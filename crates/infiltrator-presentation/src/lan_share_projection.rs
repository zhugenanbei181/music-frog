//! Presentation read-model projection for LAN proxy sharing and multi-device setup guides (FLPARITY-026).

use infiltrator_contract::lan_share::{
    LanDeviceGuideStep, LanDevicePlatform, LanShareWizardSnapshot,
};

/// Presentation model for the LAN sharing QR modal dialog.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LanShareModalProjection {
    pub is_enabled: bool,
    pub title: String,
    pub mixed_port: u16,
    pub primary_ip: String,
    pub primary_http_endpoint: String,
    pub primary_socks5_endpoint: String,
    pub primary_pac_url: Option<String>,
    pub qr_payload: String,
    pub interface_options: Vec<(String, String)>,
    pub active_guide: LanDeviceGuideStep,
    pub copy_bash_command: String,
    pub copy_cmd_command: String,
}

impl LanShareModalProjection {
    pub fn of(
        snapshot: &LanShareWizardSnapshot,
        selected_platform: Option<LanDevicePlatform>,
    ) -> Self {
        let platform = selected_platform.unwrap_or(LanDevicePlatform::Ios);
        let primary = snapshot.primary_endpoint.as_ref();

        let primary_ip = primary
            .map(|e| e.ip.clone())
            .unwrap_or_else(|| "127.0.0.1".to_owned());

        let primary_http_endpoint = primary
            .map(|e| e.http_proxy_url.clone())
            .unwrap_or_else(|| format!("http://127.0.0.1:{}", snapshot.mixed_port));

        let primary_socks5_endpoint = primary
            .map(|e| e.socks5_proxy_url.clone())
            .unwrap_or_else(|| format!("socks5://127.0.0.1:{}", snapshot.mixed_port));

        let primary_pac_url = primary.and_then(|e| e.pac_url.clone());

        let qr_payload = primary_pac_url
            .clone()
            .unwrap_or_else(|| primary_http_endpoint.clone());

        let interface_options: Vec<(String, String)> = snapshot
            .endpoints
            .iter()
            .map(|e| (e.interface_name.clone(), e.ip.clone()))
            .collect();

        let active_guide = snapshot.build_guide(platform, primary);

        let copy_bash_command = primary
            .map(|e| e.bash_export_command.clone())
            .unwrap_or_else(|| {
                format!(
                    "export http_proxy=http://127.0.0.1:{} https_proxy=http://127.0.0.1:{}",
                    snapshot.mixed_port, snapshot.mixed_port
                )
            });

        let copy_cmd_command = primary
            .map(|e| e.cmd_export_command.clone())
            .unwrap_or_else(|| {
                format!(
                    "set http_proxy=http://127.0.0.1:{} && set https_proxy=http://127.0.0.1:{}",
                    snapshot.mixed_port, snapshot.mixed_port
                )
            });

        Self {
            is_enabled: snapshot.is_lan_enabled,
            title: "局域网共享与设备接入向导".to_owned(),
            mixed_port: snapshot.mixed_port,
            primary_ip,
            primary_http_endpoint,
            primary_socks5_endpoint,
            primary_pac_url,
            qr_payload,
            interface_options,
            active_guide,
            copy_bash_command,
            copy_cmd_command,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use infiltrator_contract::lan_share::NetworkInterfaceInfo;

    #[test]
    fn projection_builds_correctly_with_endpoints() {
        let ifaces = vec![NetworkInterfaceInfo {
            name: "wlan0".to_owned(),
            ip_v4: Some("192.168.1.120".to_owned()),
            ip_v6: None,
            is_loopback: false,
            is_up: true,
        }];

        let snapshot = LanShareWizardSnapshot::of(true, 7890, ifaces);
        let proj = LanShareModalProjection::of(&snapshot, Some(LanDevicePlatform::NintendoSwitch));

        assert!(proj.is_enabled);
        assert_eq!(proj.primary_ip, "192.168.1.120");
        assert_eq!(proj.primary_http_endpoint, "http://192.168.1.120:7890");
        assert_eq!(
            proj.active_guide.platform,
            LanDevicePlatform::NintendoSwitch
        );
        assert!(proj.qr_payload.contains("192.168.1.120"));
        assert!(proj.copy_bash_command.contains("192.168.1.120:7890"));
    }
}
