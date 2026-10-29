//! Shared live contract for LAN proxy sharing QR code and multi-device setup guides (FLPARITY-026).

use serde::{Deserialize, Serialize};

/// Basic network interface information for host binding inspection.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NetworkInterfaceInfo {
    pub name: String,
    pub ip_v4: Option<String>,
    pub ip_v6: Option<String>,
    pub is_loopback: bool,
    pub is_up: bool,
}

impl NetworkInterfaceInfo {
    pub fn is_usable_lan_v4(&self) -> bool {
        self.is_up
            && !self.is_loopback
            && self
                .ip_v4
                .as_ref()
                .is_some_and(|ip| !ip.starts_with("127."))
    }
}

/// Target platform for LAN proxy configuration guides.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LanDevicePlatform {
    Ios,
    Android,
    NintendoSwitch,
    Playstation,
    TerminalCli,
}

impl LanDevicePlatform {
    pub const ALL: [Self; 5] = [
        Self::Ios,
        Self::Android,
        Self::NintendoSwitch,
        Self::Playstation,
        Self::TerminalCli,
    ];

    pub const fn label_zh(self) -> &'static str {
        match self {
            Self::Ios => "iOS / iPadOS",
            Self::Android => "Android 安卓",
            Self::NintendoSwitch => "Nintendo Switch",
            Self::Playstation => "PlayStation 4/5",
            Self::TerminalCli => "命令行 / 终端",
        }
    }
}

/// Step-by-step setup instructions for a specific device platform.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LanDeviceGuideStep {
    pub platform: LanDevicePlatform,
    pub platform_label: &'static str,
    pub steps: Vec<String>,
    pub copyable_command: Option<String>,
}

/// Aggregated proxy endpoint representation for an IP and port pair.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LanProxyEndpointModel {
    pub interface_name: String,
    pub ip: String,
    pub port: u16,
    pub http_proxy_url: String,
    pub socks5_proxy_url: String,
    pub pac_url: Option<String>,
    pub bash_export_command: String,
    pub cmd_export_command: String,
}

impl LanProxyEndpointModel {
    pub fn new(interface_name: impl Into<String>, ip: impl Into<String>, port: u16) -> Self {
        let ip = ip.into();
        let interface_name = interface_name.into();
        let http_proxy_url = format!("http://{ip}:{port}");
        let socks5_proxy_url = format!("socks5://{ip}:{port}");
        let pac_url = Some(format!("http://{ip}:{port}/ui/proxy.pac"));
        let bash_export_command = format!(
            "export http_proxy={http_proxy_url} https_proxy={http_proxy_url} all_proxy={socks5_proxy_url}"
        );
        let cmd_export_command =
            format!("set http_proxy={http_proxy_url} && set https_proxy={http_proxy_url}");

        Self {
            interface_name,
            ip,
            port,
            http_proxy_url,
            socks5_proxy_url,
            pac_url,
            bash_export_command,
            cmd_export_command,
        }
    }
}

/// Complete snapshot for LAN share wizard modal.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LanShareWizardSnapshot {
    pub is_lan_enabled: bool,
    pub mixed_port: u16,
    pub interfaces: Vec<NetworkInterfaceInfo>,
    pub endpoints: Vec<LanProxyEndpointModel>,
    pub primary_endpoint: Option<LanProxyEndpointModel>,
}

impl LanShareWizardSnapshot {
    pub fn of(
        is_lan_enabled: bool,
        mixed_port: u16,
        interfaces: Vec<NetworkInterfaceInfo>,
    ) -> Self {
        let endpoints: Vec<LanProxyEndpointModel> = interfaces
            .iter()
            .filter(|iface| iface.is_usable_lan_v4())
            .filter_map(|iface| {
                let ip = iface.ip_v4.as_ref()?;
                Some(LanProxyEndpointModel::new(&iface.name, ip, mixed_port))
            })
            .collect();

        let primary_endpoint = endpoints.first().cloned();

        Self {
            is_lan_enabled,
            mixed_port,
            interfaces,
            endpoints,
            primary_endpoint,
        }
    }

    /// Build device setup guide for a selected platform and endpoint.
    pub fn build_guide(
        &self,
        platform: LanDevicePlatform,
        endpoint: Option<&LanProxyEndpointModel>,
    ) -> LanDeviceGuideStep {
        let ip = endpoint.map(|e| e.ip.as_str()).unwrap_or("192.168.x.x");
        let port = endpoint.map(|e| e.port).unwrap_or(self.mixed_port);

        let (steps, copyable_command) = match platform {
            LanDevicePlatform::Ios => (
                vec![
                    "打开「设置」→「无线局域网」，点击已连接 Wi-Fi 旁的「i」图标。".to_owned(),
                    "滑动至最下方，点击「配置代理」选择「手动」。".to_owned(),
                    format!("在「服务器」输入 {ip}，在「端口」输入 {port}，点击右上角「存储」。"),
                ],
                None,
            ),
            LanDevicePlatform::Android => (
                vec![
                    "打开「设置」→「WLAN」，长按或点击当前连接的 Wi-Fi 齿轮图标。".to_owned(),
                    "点击「修改网络」或「高级选项」，将「代理」设为「手动」。".to_owned(),
                    format!(
                        "「代理服务器主机名」输入 {ip}，「代理服务器端口」输入 {port}，保存连接。"
                    ),
                ],
                None,
            ),
            LanDevicePlatform::NintendoSwitch => (
                vec![
                    "进入 Switch「系统设置」→「互联网」→「互联网设置」。".to_owned(),
                    "选择当前连接的 Wi-Fi 网络，点击「更改设置」。".to_owned(),
                    "找到「代理服务器设置」，选择「开启」。".to_owned(),
                    format!("「服务器」输入 {ip}，「端口」输入 {port}，点击「保存」。"),
                ],
                None,
            ),
            LanDevicePlatform::Playstation => (
                vec![
                    "进入 PS4/PS5「设定」→「网络」→「设定互联网连接」。".to_owned(),
                    "选择当前网络进入「高级设置」，找到「代理服务器」选择「使用」。".to_owned(),
                    format!("「地址」填入 {ip}，「端口号」填入 {port}，保存完成设置。"),
                ],
                None,
            ),
            LanDevicePlatform::TerminalCli => {
                let cmd = endpoint
                    .map(|e| e.bash_export_command.clone())
                    .unwrap_or_else(|| {
                        format!(
                            "export http_proxy=http://{ip}:{port} https_proxy=http://{ip}:{port}"
                        )
                    });
                (
                    vec![
                        "在同一局域网下的电脑终端执行环境变量配置命令，即可为当前终端会话注入代理：".to_owned(),
                        cmd.clone(),
                    ],
                    Some(cmd),
                )
            }
        };

        LanDeviceGuideStep {
            platform,
            platform_label: platform.label_zh(),
            steps,
            copyable_command,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn usable_lan_v4_filters_loopback_and_down_interfaces() {
        let lo = NetworkInterfaceInfo {
            name: "lo".to_owned(),
            ip_v4: Some("127.0.0.1".to_owned()),
            ip_v6: Some("::1".to_owned()),
            is_loopback: true,
            is_up: true,
        };
        assert!(!lo.is_usable_lan_v4());

        let wlan0 = NetworkInterfaceInfo {
            name: "wlan0".to_owned(),
            ip_v4: Some("192.168.1.105".to_owned()),
            ip_v6: None,
            is_loopback: false,
            is_up: true,
        };
        assert!(wlan0.is_usable_lan_v4());
    }

    #[test]
    fn endpoint_model_formats_proxy_and_export_commands() {
        let ep = LanProxyEndpointModel::new("wlan0", "192.168.1.50", 7890);
        assert_eq!(ep.http_proxy_url, "http://192.168.1.50:7890");
        assert_eq!(ep.socks5_proxy_url, "socks5://192.168.1.50:7890");
        assert_eq!(
            ep.pac_url.as_deref(),
            Some("http://192.168.1.50:7890/ui/proxy.pac")
        );
        assert!(ep.bash_export_command.contains("http://192.168.1.50:7890"));
    }

    #[test]
    fn snapshot_generates_device_guides_for_all_platforms() {
        let ifaces = vec![NetworkInterfaceInfo {
            name: "wlan0".to_owned(),
            ip_v4: Some("192.168.31.200".to_owned()),
            ip_v6: None,
            is_loopback: false,
            is_up: true,
        }];
        let snapshot = LanShareWizardSnapshot::of(true, 7890, ifaces);
        assert_eq!(snapshot.endpoints.len(), 1);
        let ep = snapshot.primary_endpoint.as_ref().unwrap();

        for platform in LanDevicePlatform::ALL {
            let guide = snapshot.build_guide(platform, Some(ep));
            assert!(!guide.steps.is_empty());
            assert!(
                guide
                    .steps
                    .iter()
                    .any(|s| s.contains("192.168.31.200") || s.contains("代理"))
            );
        }
    }
}
