//! Pure validation for VpnService start requests.

use infiltrator_contract::vpn::{
    MAX_VPN_MTU_BYTES, MIN_VPN_MTU_BYTES, VpnConfiguration, VpnStartRequest,
};
use std::net::IpAddr;
use url::Url;

pub fn validate_start_request(request: &VpnStartRequest) -> Result<(), String> {
    if request.tun_fd <= 0 {
        return Err("VpnService supplied an invalid tunnel file descriptor".to_owned());
    }
    if request.prepared.preparation_id.value() == 0 || request.prepared.vpn_generation.value() == 0
    {
        return Err("VPN start requires a non-zero prepared transaction identity".to_owned());
    }
    if !request
        .prepared
        .core_scope
        .session_token
        .is_some_and(|token| token.is_valid())
    {
        return Err("VPN start requires an active Core session scope".to_owned());
    }
    validate_configuration(request.configuration())
}

pub fn validate_configuration(configuration: &VpnConfiguration) -> Result<(), String> {
    if !(MIN_VPN_MTU_BYTES..=MAX_VPN_MTU_BYTES).contains(&configuration.mtu) {
        return Err(format!(
            "VPN MTU must be within {MIN_VPN_MTU_BYTES}..={MAX_VPN_MTU_BYTES}"
        ));
    }
    let endpoint = Url::parse(configuration.proxy_endpoint.trim())
        .map_err(|error| format!("invalid VPN proxy endpoint: {error}"))?;
    if !matches!(endpoint.scheme(), "socks5" | "socks" | "http" | "https") {
        return Err(format!(
            "unsupported VPN proxy scheme: {}",
            endpoint.scheme()
        ));
    }
    if endpoint.host_str().is_none() || endpoint.port().is_none() {
        return Err("VPN proxy endpoint must include host and port".to_owned());
    }
    if configuration.routes.is_empty() {
        return Err("VpnService requires at least one route".to_owned());
    }
    for route in &configuration.routes {
        let address = route
            .address
            .parse::<IpAddr>()
            .map_err(|_| format!("invalid VPN route address: {}", route.address))?;
        let max_prefix = if address.is_ipv4() { 32 } else { 128 };
        if u16::from(route.prefix) > max_prefix {
            return Err(format!(
                "VPN route prefix {} exceeds {max_prefix}",
                route.prefix
            ));
        }
    }
    if configuration.dns_servers.len() > 16 {
        return Err("VPN DNS server list exceeds 16 entries".to_owned());
    }
    for server in &configuration.dns_servers {
        server
            .parse::<IpAddr>()
            .map_err(|_| format!("invalid VPN DNS server: {server}"))?;
    }
    if !configuration.foreground_requested {
        return Err("Android VPN sessions must request foreground service mode".to_owned());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use infiltrator_contract::core_identity::{
        CoreGeneration, CoreIdentity, CoreInstanceId, CoreScope, CoreVersion, EngineId,
    };
    use infiltrator_contract::vpn::{
        PreparedVpnConfiguration, VpnGeneration, VpnPreparationId, VpnRoute, VpnStartRequest,
    };
    use lurker_core_contract::session::SessionToken;

    fn request() -> VpnStartRequest {
        VpnStartRequest {
            tun_fd: 7,
            prepared: PreparedVpnConfiguration {
                preparation_id: VpnPreparationId::new(1),
                vpn_generation: VpnGeneration::new(1),
                owner_data_revision: 1,
                core_scope: CoreScope {
                    identity: CoreIdentity::new(
                        EngineId::new("mihomo").expect("engine ID"),
                        CoreInstanceId::new("primary").expect("instance ID"),
                        Some(CoreVersion::new("1.19.12").expect("Core version")),
                    ),
                    generation: CoreGeneration::new(9),
                    session_token: Some(SessionToken::new(71)),
                },
                configuration: VpnConfiguration {
                    proxy_endpoint: "socks5://127.0.0.1:7891".to_owned(),
                    mtu: 1500,
                    routes: vec![VpnRoute {
                        address: "0.0.0.0".to_owned(),
                        prefix: 0,
                        exclude: false,
                    }],
                    dns_servers: vec!["1.1.1.1".to_owned()],
                    ipv6: true,
                    foreground_requested: true,
                },
            },
        }
    }

    #[test]
    fn valid_request_contains_fd_proxy_route_and_foreground_proof() {
        assert!(validate_start_request(&request()).is_ok());
    }

    #[test]
    fn invalid_fd_and_non_foreground_requests_fail_closed() {
        let mut value = request();
        value.tun_fd = 0;
        assert!(validate_start_request(&value).is_err());
        value = request();
        value.prepared.configuration.foreground_requested = false;
        assert!(validate_start_request(&value).is_err());
    }

    #[test]
    fn route_and_dns_values_are_validated_as_ip_literals() {
        let mut value = request();
        value.prepared.configuration.routes[0].address = "not-an-ip".to_owned();
        assert!(validate_start_request(&value).is_err());
        value = request();
        value.prepared.configuration.dns_servers = vec!["dns.example".to_owned()];
        assert!(validate_start_request(&value).is_err());
    }
}
