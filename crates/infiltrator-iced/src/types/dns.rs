//! Advanced-config (DNS / Fake-IP / TUN / sniffer) form types: page tabs,
//! form/edit-mode selection, form drafts and validation state.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DnsTab {
    #[default]
    Dns,
    FakeIp,
    Tun,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AdvancedEditMode {
    #[default]
    Form,
    Json,
}

#[derive(Debug, Clone, Default)]
pub struct DnsFormDraft {
    pub enable: bool,
    pub nameserver: String,
    pub fallback: String,
    pub enhanced_mode: String,
    pub fake_ip_range: String,
    pub fake_ip_filter: String,
    pub ipv6: bool,
    pub cache: bool,
    pub use_hosts: bool,
    pub use_system_hosts: bool,
    pub respect_rules: bool,
    pub proxy_server_nameserver: String,
    pub direct_nameserver: String,
}

#[derive(Debug, Clone, Default)]
pub struct FakeIpFormDraft {
    pub fake_ip_range: String,
    pub fake_ip_filter: String,
    pub store_fake_ip: bool,
}

#[derive(Debug, Clone, Default)]
pub struct TunFormDraft {
    pub enable: bool,
    pub stack: String,
    pub mtu: String,
    pub dns_hijack: String,
    pub auto_route: bool,
    pub auto_detect_interface: bool,
    pub strict_route: bool,
}

#[derive(Debug, Clone, Default)]
pub struct AdvancedValidationState {
    pub dns: Option<String>,
    pub fake_ip: Option<String>,
    pub tun: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct AdvancedConfigsBundle {
    pub dns_json: String,
    pub fake_ip_json: String,
    pub tun_json: String,
    pub dns: infiltrator_core::dns::DnsConfig,
    pub fake_ip: infiltrator_core::fake_ip::FakeIpConfig,
    pub tun: infiltrator_core::tun::TunConfig,
}
