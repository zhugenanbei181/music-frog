//! Desktop physical-link observation and safe TUN route-anchor repair.

use async_trait::async_trait;
use infiltrator_contract::network_roaming::{
    NetworkInterfaceKind, NetworkInterfaceSnapshot, NetworkObservation,
    NetworkRoamingRepairRequest, NetworkRoamingRepairResult,
};
use infiltrator_ports::error::PortError;
use infiltrator_ports::network_roaming::NetworkRoamingPort;
use mihomo_platform::interface_watcher::{InterfaceType, NetworkInterfaceWatcher};
use std::net::IpAddr;
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

static NEXT_ROUTE_GENERATION: AtomicU64 = AtomicU64::new(1);
static SHARED_REPAIR_STATE: OnceLock<Arc<Mutex<RepairState>>> = OnceLock::new();

#[derive(Clone, Debug)]
pub struct DesktopNetworkRoamingPort {
    repair_state: Arc<Mutex<RepairState>>,
}

impl Default for DesktopNetworkRoamingPort {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Debug, Default)]
struct RepairState {
    last_key: Option<RepairKey>,
    last_result: Option<NetworkRoamingRepairResult>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct RepairKey {
    physical_interface: String,
    gateway_ip: Option<String>,
    tun_interface: String,
    strict_route: bool,
}

impl DesktopNetworkRoamingPort {
    pub fn new() -> Self {
        Self {
            repair_state: Arc::new(Mutex::new(RepairState::default())),
        }
    }

    pub fn shared() -> Self {
        Self {
            repair_state: SHARED_REPAIR_STATE
                .get_or_init(|| Arc::new(Mutex::new(RepairState::default())))
                .clone(),
        }
    }

    fn observe_sync() -> Result<NetworkObservation, PortError> {
        let mut interfaces = NetworkInterfaceWatcher::poll_interfaces();
        let mtus = crate::mtu::link_mtu_table();
        crate::mtu::apply_link_mtu(&mut interfaces, &mtus);
        let routes = default_routes()?;
        apply_default_routes(&mut interfaces, &routes);

        Ok(NetworkObservation {
            interfaces: interfaces.into_iter().map(to_contract_interface).collect(),
            observed_at_epoch_ms: epoch_ms(),
        })
    }

    fn repair_sync(
        &self,
        request: NetworkRoamingRepairRequest,
    ) -> Result<NetworkRoamingRepairResult, PortError> {
        validate_repair_request(&request)?;
        let key = RepairKey {
            physical_interface: request.physical_interface.clone(),
            gateway_ip: request.gateway_ip.clone(),
            tun_interface: request.tun_interface.clone(),
            strict_route: request.strict_route,
        };
        let mut repair_state = self.repair_state.lock().expect("network repair state lock");
        if repair_state.last_key.as_ref() == Some(&key)
            && let Some(result) = repair_state.last_result.clone()
        {
            return Ok(NetworkRoamingRepairResult {
                detail: format!("already read back by this host: {}", result.detail),
                ..result
            });
        }
        let generation = NEXT_ROUTE_GENERATION.fetch_add(1, Ordering::Relaxed);
        let Some(gateway_ip) = request.gateway_ip.as_deref() else {
            let result = NetworkRoamingRepairResult {
                route_generation: generation,
                detail: format!(
                    "{} has no explicit next-hop; the OS link route remains authoritative",
                    request.physical_interface
                ),
            };
            repair_state.last_key = Some(key);
            repair_state.last_result = Some(result.clone());
            return Ok(result);
        };
        let gateway_ip = gateway_ip.parse::<IpAddr>().map_err(|_| {
            PortError::Failed(format!(
                "default gateway is not an IP address: {gateway_ip}"
            ))
        })?;

        let detail = repair_route_anchor(&request, gateway_ip)?;
        let result = NetworkRoamingRepairResult {
            route_generation: generation,
            detail,
        };
        repair_state.last_key = Some(key);
        repair_state.last_result = Some(result.clone());
        Ok(result)
    }
}

#[async_trait]
impl NetworkRoamingPort for DesktopNetworkRoamingPort {
    async fn observe(&self) -> Result<NetworkObservation, PortError> {
        tokio::task::spawn_blocking(Self::observe_sync)
            .await
            .map_err(|error| PortError::Io(format!("network observation worker failed: {error}")))?
    }

    async fn repair(
        &self,
        request: NetworkRoamingRepairRequest,
    ) -> Result<NetworkRoamingRepairResult, PortError> {
        let port = self.clone();
        tokio::task::spawn_blocking(move || port.repair_sync(request))
            .await
            .map_err(|error| PortError::Io(format!("network repair worker failed: {error}")))?
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct DefaultRoute {
    interface: String,
    gateway_ip: Option<String>,
    metric: Option<u32>,
}

#[allow(clippy::needless_return)]
fn default_routes() -> Result<Vec<DefaultRoute>, PortError> {
    #[cfg(target_os = "linux")]
    {
        let output = command_output("ip", &["route", "show", "default"])?;
        return Ok(parse_linux_default_routes(&output));
    }

    #[cfg(target_os = "macos")]
    {
        let output = command_output("route", &["-n", "get", "default"])?;
        return parse_macos_default_route(&output)
            .map(|route| vec![route])
            .ok_or_else(|| PortError::NotFound("macOS did not report a default route".to_owned()));
    }

    #[cfg(target_os = "windows")]
    {
        let script = "Get-NetRoute -AddressFamily IPv4 -DestinationPrefix '0.0.0.0/0' | Sort-Object RouteMetric | Select-Object -First 16 | ForEach-Object { '{0}|{1}|{2}' -f $_.InterfaceAlias,$_.NextHop,$_.RouteMetric }";
        let output = command_output(
            "powershell.exe",
            &["-NoProfile", "-NonInteractive", "-Command", script],
        )?;
        return Ok(parse_windows_default_routes(&output));
    }

    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    Err(PortError::unsupported(
        infiltrator_contract::capability::Capability::NetworkRoaming,
        "desktop default-route observation is not implemented for this OS",
    ))
}

fn command_output(program: &str, args: &[&str]) -> Result<String, PortError> {
    let output = Command::new(program)
        .args(args)
        .output()
        .map_err(|error| PortError::Io(format!("failed to run {program}: {error}")))?;
    if !output.status.success() {
        return Err(PortError::Failed(format!(
            "{program} exited with {:?}: {}",
            output.status.code(),
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

fn parse_linux_default_routes(output: &str) -> Vec<DefaultRoute> {
    let mut routes = output
        .lines()
        .filter_map(|line| {
            let fields: Vec<&str> = line.split_whitespace().collect();
            if fields.first().copied() != Some("default") {
                return None;
            }
            let mut interface = None;
            let mut gateway_ip = None;
            let mut metric = None;
            let mut index = 1;
            while index < fields.len() {
                match fields[index] {
                    "dev" => interface = fields.get(index + 1).map(|value| (*value).to_owned()),
                    "via" => gateway_ip = fields.get(index + 1).map(|value| (*value).to_owned()),
                    "metric" => metric = fields.get(index + 1).and_then(|value| value.parse().ok()),
                    _ => {}
                }
                index += 1;
            }
            Some(DefaultRoute {
                interface: interface?,
                gateway_ip,
                metric,
            })
        })
        .collect::<Vec<_>>();
    routes.sort_by_key(|route| (route.metric.unwrap_or(u32::MAX), route.interface.clone()));
    routes
}

#[allow(dead_code)]
fn parse_macos_default_route(output: &str) -> Option<DefaultRoute> {
    let mut gateway_ip = None;
    let mut interface = None;
    for line in output.lines() {
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        match key.trim() {
            "gateway" => gateway_ip = Some(value.trim().to_owned()),
            "interface" => interface = Some(value.trim().to_owned()),
            _ => {}
        }
    }
    Some(DefaultRoute {
        interface: interface?,
        gateway_ip,
        metric: None,
    })
}

#[allow(dead_code)]
fn parse_windows_default_routes(output: &str) -> Vec<DefaultRoute> {
    output
        .lines()
        .filter_map(|line| {
            let mut fields = line.trim().splitn(3, '|');
            let interface = fields.next()?.trim();
            let gateway = fields.next()?.trim();
            let metric = fields.next()?.trim().parse().ok();
            if interface.is_empty() {
                return None;
            }
            Some(DefaultRoute {
                interface: interface.to_owned(),
                gateway_ip: (!gateway.is_empty() && gateway != "0.0.0.0")
                    .then(|| gateway.to_owned()),
                metric,
            })
        })
        .collect()
}

fn apply_default_routes(
    interfaces: &mut [mihomo_platform::interface_watcher::NetworkInterfaceSnapshot],
    routes: &[DefaultRoute],
) {
    for route in routes {
        if let Some(interface) = interfaces
            .iter_mut()
            .find(|item| item.name == route.interface)
        {
            interface.is_default_gateway = true;
            interface.gateway_ip = route.gateway_ip.clone();
            interface.metric = route.metric;
        }
    }
}

fn to_contract_interface(
    interface: mihomo_platform::interface_watcher::NetworkInterfaceSnapshot,
) -> NetworkInterfaceSnapshot {
    let kind = interface.inferred_type();
    NetworkInterfaceSnapshot {
        name: interface.name,
        kind: match kind {
            InterfaceType::Ethernet => NetworkInterfaceKind::Ethernet,
            InterfaceType::WiFi => NetworkInterfaceKind::Wifi,
            InterfaceType::Cellular => NetworkInterfaceKind::Cellular,
            InterfaceType::Tun => NetworkInterfaceKind::Tun,
            InterfaceType::Loopback => NetworkInterfaceKind::Loopback,
            InterfaceType::Bridge => NetworkInterfaceKind::Bridge,
            InterfaceType::Other => NetworkInterfaceKind::Other,
        },
        is_up: interface.is_up,
        is_default_gateway: interface.is_default_gateway,
        gateway_ip: interface.gateway_ip,
        ip_addresses: interface.ip_addresses,
        mtu: interface.mtu,
        metric: interface.metric,
        dns_servers: interface.dns_servers,
    }
}

fn validate_repair_request(request: &NetworkRoamingRepairRequest) -> Result<(), PortError> {
    if request.physical_interface.trim().is_empty()
        || request.tun_interface.trim().is_empty()
        || request.physical_interface.contains('\0')
        || request.tun_interface.contains('\0')
    {
        return Err(PortError::NotFound(
            "route repair requires non-empty interface names".to_owned(),
        ));
    }
    if request
        .physical_interface
        .eq_ignore_ascii_case(&request.tun_interface)
    {
        return Err(PortError::Failed(
            "refusing to repair a route through the TUN interface itself".to_owned(),
        ));
    }
    Ok(())
}

#[allow(clippy::needless_return)]
fn repair_route_anchor(
    request: &NetworkRoamingRepairRequest,
    gateway_ip: IpAddr,
) -> Result<String, PortError> {
    #[cfg(target_os = "linux")]
    {
        let prefix = match gateway_ip {
            IpAddr::V4(_) => format!("{gateway_ip}/32"),
            IpAddr::V6(_) => format!("{gateway_ip}/128"),
        };
        let args = if gateway_ip.is_ipv4() {
            vec![
                "route",
                "replace",
                prefix.as_str(),
                "dev",
                request.physical_interface.as_str(),
            ]
        } else {
            vec![
                "-6",
                "route",
                "replace",
                prefix.as_str(),
                "dev",
                request.physical_interface.as_str(),
            ]
        };
        command_status("ip", &args)?;
        let gateway = gateway_ip.to_string();
        let readback_args = if gateway_ip.is_ipv4() {
            vec!["route", "get", gateway.as_str()]
        } else {
            vec!["-6", "route", "get", gateway.as_str()]
        };
        let readback = command_output("ip", &readback_args)?;
        if parse_route_get_interface(&readback).as_deref()
            != Some(request.physical_interface.as_str())
        {
            return Err(PortError::Failed(format!(
                "route readback did not select {}",
                request.physical_interface
            )));
        }
        return Ok(format!(
            "replaced {} route anchor via {} (strict-route={})",
            request.physical_interface, gateway_ip, request.strict_route
        ));
    }

    #[cfg(target_os = "macos")]
    {
        let gateway = gateway_ip.to_string();
        let host_flag = if gateway_ip.is_ipv4() {
            "-host"
        } else {
            "-inet6"
        };
        let args = [
            "-n",
            "change",
            host_flag,
            gateway.as_str(),
            "-interface",
            request.physical_interface.as_str(),
        ];
        if command_status("route", &args).is_err() {
            let add_args = [
                "-n",
                "add",
                host_flag,
                gateway.as_str(),
                "-interface",
                request.physical_interface.as_str(),
            ];
            command_status("route", &add_args)?;
        }
        let readback = command_output("route", &["-n", "get", gateway.as_str()])?;
        if parse_macos_route_get_interface(&readback).as_deref()
            != Some(request.physical_interface.as_str())
        {
            return Err(PortError::Failed(format!(
                "route readback did not select {}",
                request.physical_interface
            )));
        }
        return Ok(format!(
            "reconciled {} route anchor via {} (strict-route={})",
            request.physical_interface, gateway, request.strict_route
        ));
    }

    #[cfg(target_os = "windows")]
    {
        let gateway = gateway_ip.to_string();
        let script = "$gateway=$env:MF_NETWORK_ROAM_GATEWAY; $interface=$env:MF_NETWORK_ROAM_INTERFACE; $family=if ($gateway.Contains(':')) { 'IPv6' } else { 'IPv4' }; $prefix=if ($family -eq 'IPv6') { \"$gateway/128\" } else { \"$gateway/32\" }; $nextHop=if ($family -eq 'IPv6') { '::' } else { '0.0.0.0' }; New-NetRoute -AddressFamily $family -DestinationPrefix $prefix -InterfaceAlias $interface -NextHop $nextHop -PolicyStore ActiveStore -RouteMetric 1 -ErrorAction Stop | Out-Null";
        let mut command = Command::new("powershell.exe");
        command.args(["-NoProfile", "-NonInteractive", "-Command", script]);
        command.env("MF_NETWORK_ROAM_GATEWAY", &gateway);
        command.env("MF_NETWORK_ROAM_INTERFACE", &request.physical_interface);
        command_status_command(&mut command)?;
        let readback_script = "$gateway=$env:MF_NETWORK_ROAM_GATEWAY; $interface=$env:MF_NETWORK_ROAM_INTERFACE; $family=if ($gateway.Contains(':')) { 'IPv6' } else { 'IPv4' }; $prefix=if ($family -eq 'IPv6') { \"$gateway/128\" } else { \"$gateway/32\" }; Get-NetRoute -AddressFamily $family -DestinationPrefix $prefix -ErrorAction SilentlyContinue | Where-Object InterfaceAlias -eq $interface | Select-Object -First 1 -ExpandProperty InterfaceAlias";
        let mut readback_command = Command::new("powershell.exe");
        readback_command.args(["-NoProfile", "-NonInteractive", "-Command", readback_script]);
        readback_command.env("MF_NETWORK_ROAM_GATEWAY", &gateway);
        readback_command.env("MF_NETWORK_ROAM_INTERFACE", &request.physical_interface);
        let readback = command_output_command(&mut readback_command)?;
        if readback.trim() != request.physical_interface {
            return Err(PortError::Failed(format!(
                "route readback did not select {}",
                request.physical_interface
            )));
        }
        return Ok(format!(
            "reconciled {} route anchor via {} (strict-route={})",
            request.physical_interface, gateway, request.strict_route
        ));
    }

    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    {
        let _ = (request, gateway_ip);
        Err(PortError::unsupported(
            infiltrator_contract::capability::Capability::NetworkRoaming,
            "desktop TUN route repair is not implemented for this OS",
        ))
    }
}

fn command_status(program: &str, args: &[&str]) -> Result<(), PortError> {
    let mut command = Command::new(program);
    command.args(args);
    command_status_command(&mut command)
}

fn command_status_command(command: &mut Command) -> Result<(), PortError> {
    let output = command
        .output()
        .map_err(|error| PortError::Io(format!("failed to run route command: {error}")))?;
    if output.status.success() {
        Ok(())
    } else {
        Err(PortError::PermissionDenied(
            String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        ))
    }
}

#[allow(dead_code)]
fn command_output_command(command: &mut Command) -> Result<String, PortError> {
    let output: Output = command
        .output()
        .map_err(|error| PortError::Io(format!("failed to run route command: {error}")))?;
    if !output.status.success() {
        return Err(PortError::Failed(format!(
            "route command exited with {:?}: {}",
            output.status.code(),
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

fn parse_route_get_interface(output: &str) -> Option<String> {
    let fields: Vec<&str> = output.split_whitespace().collect();
    fields
        .windows(2)
        .find(|pair| pair[0] == "dev")
        .map(|pair| pair[1].to_owned())
}

#[allow(dead_code)]
fn parse_macos_route_get_interface(output: &str) -> Option<String> {
    output.lines().find_map(|line| {
        let (key, value) = line.split_once(':')?;
        (key.trim() == "interface").then(|| value.trim().to_owned())
    })
}

fn epoch_ms() -> Option<i64> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|duration| i64::try_from(duration.as_millis()).ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linux_default_route_parser_ranks_by_metric_and_keeps_next_hop() {
        let routes = parse_linux_default_routes(
            "default via 192.168.2.1 dev wlan0 proto dhcp metric 600\ndefault via 192.168.1.1 dev eth0 proto dhcp metric 100\n",
        );
        assert_eq!(routes[0].interface, "eth0");
        assert_eq!(routes[0].gateway_ip.as_deref(), Some("192.168.1.1"));
        assert_eq!(routes[1].interface, "wlan0");
    }

    #[test]
    fn linux_default_route_parser_supports_a_direct_link_route() {
        let routes =
            parse_linux_default_routes("default dev eth0 proto kernel scope link metric 50\n");
        assert_eq!(routes[0].interface, "eth0");
        assert_eq!(routes[0].gateway_ip, None);
        assert_eq!(routes[0].metric, Some(50));
    }

    #[test]
    fn macos_default_route_parser_reads_gateway_and_interface() {
        let route = parse_macos_default_route("gateway: 192.0.2.1\ninterface: en0\n").unwrap();
        assert_eq!(route.interface, "en0");
        assert_eq!(route.gateway_ip.as_deref(), Some("192.0.2.1"));
    }

    #[test]
    fn windows_default_route_parser_rejects_empty_and_zero_next_hops() {
        let routes = parse_windows_default_routes("Ethernet|0.0.0.0|25\nWi-Fi|192.0.2.1|50\n");
        assert_eq!(routes[0].gateway_ip, None);
        assert_eq!(routes[1].gateway_ip.as_deref(), Some("192.0.2.1"));
    }

    #[test]
    fn repair_request_rejects_tun_loop_and_empty_names() {
        let base = NetworkRoamingRepairRequest {
            physical_interface: "eth0".to_owned(),
            gateway_ip: Some("192.0.2.1".to_owned()),
            tun_interface: "Meta".to_owned(),
            strict_route: true,
        };
        assert!(validate_repair_request(&base).is_ok());
        assert!(
            validate_repair_request(&NetworkRoamingRepairRequest {
                physical_interface: "Meta".to_owned(),
                ..base.clone()
            })
            .is_err()
        );
        assert!(
            validate_repair_request(&NetworkRoamingRepairRequest {
                physical_interface: String::new(),
                ..base
            })
            .is_err()
        );
    }

    #[tokio::test]
    async fn repeated_no_next_hop_repair_is_idempotent_on_one_host() {
        let port = DesktopNetworkRoamingPort::new();
        let request = NetworkRoamingRepairRequest {
            physical_interface: "eth0".to_owned(),
            gateway_ip: None,
            tun_interface: "Meta".to_owned(),
            strict_route: true,
        };
        let first = port.repair(request.clone()).await.expect("first repair");
        let second = port.repair(request).await.expect("second repair");
        assert_eq!(first.route_generation, second.route_generation);
        assert!(second.detail.contains("already read back"));
    }
}
