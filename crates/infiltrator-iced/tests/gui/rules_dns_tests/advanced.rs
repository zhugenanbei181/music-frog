//! Behavior cases for advanced.
//! test-intent: behavior

use super::*;
use infiltrator_domain::dns::DnsConfig;
use infiltrator_domain::fake_ip::FakeIpConfig;
use infiltrator_domain::tun::TunConfig;

#[test]
fn test_advanced_bundle_load_applies_form_drafts() {
    let (mut state, _) = AppState::new();
    let bundle = AdvancedConfigsBundle {
        dns_json: "{}".to_string(),
        fake_ip_json: "{}".to_string(),
        tun_json: "{}".to_string(),
        dns: DnsConfig {
            enable: Some(true),
            nameserver: Some(vec!["https://dns.google/dns-query".to_string()]),
            enhanced_mode: Some("fake-ip".to_string()),
            ..Default::default()
        },
        fake_ip: FakeIpConfig {
            fake_ip_range: Some("198.18.0.1/16".to_string()),
            store_fake_ip: Some(true),
            ..Default::default()
        },
        tun: TunConfig {
            enable: Some(true),
            stack: Some("gvisor".to_string()),
            mtu: Some(1500),
            ..Default::default()
        },
    };
    let _ = state.update(Message::AdvancedConfigsBundleLoaded(Ok(Box::new(bundle))));
    assert!(state.editor.dns_form.switches.enable);
    assert_eq!(
        state.editor.dns_form.nameserver,
        "https://dns.google/dns-query".to_string()
    );
    assert_eq!(
        state.editor.fake_ip_form.fake_ip_range,
        "198.18.0.1/16".to_string()
    );
    assert!(state.editor.fake_ip_form.store_fake_ip);
    assert!(state.editor.tun_form.enable);
    assert_eq!(state.editor.tun_form.stack, "gvisor".to_string());
    assert_eq!(state.editor.tun_form.mtu, "1500".to_string());
}
