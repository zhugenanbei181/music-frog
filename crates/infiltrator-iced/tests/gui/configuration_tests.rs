use super::*;
use infiltrator_application::dns_hosts_fixtures::{HOSTS_PROFILE, HostsCaptureStore};
use infiltrator_contract::dns_hosts::DnsHostEntry;

#[tokio::test]
async fn host_configuration_factory_binds_the_mihomo_codec() {
    let store = Arc::new(HostsCaptureStore::default());
    let application = for_mihomo_store(store);

    let observed = application.load_hosts_profile().await.unwrap();
    assert_eq!(observed.profile, HOSTS_PROFILE);
    assert_eq!(
        observed.entries,
        vec![
            DnsHostEntry {
                domain: "live.test".into(),
                address: "9.9.9.9".into(),
            },
            DnsHostEntry {
                domain: "second.test".into(),
                address: "8.8.8.8".into(),
            },
        ]
    );
}
