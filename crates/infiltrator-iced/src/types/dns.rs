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
    /// DUAL-14-11: shared `dns.hosts` editor validation message.
    pub dns_hosts: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct AdvancedConfigsBundle {
    pub dns_json: String,
    pub fake_ip_json: String,
    pub tun_json: String,
    pub dns: infiltrator_domain::dns::DnsConfig,
    pub fake_ip: infiltrator_domain::fake_ip::FakeIpConfig,
    pub tun: infiltrator_domain::tun::TunConfig,
}

/// Configuration and negotiation state for TUN network stacks and MTU probe.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TunStackConfig {
    pub active_stack: String,
    pub negotiated_mtu: u32,
    pub is_probing_mtu: bool,
    pub probe_result_summary: Option<String>,
}
