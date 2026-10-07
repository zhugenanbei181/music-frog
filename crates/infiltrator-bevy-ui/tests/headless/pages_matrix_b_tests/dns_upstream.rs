//! Behavior cases for dns upstream.
//! test-intent: behavior

use super::*;

#[test]
fn test_dns_upstream_list_edit_submits_shared_patch() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_b_app(Arc::clone(&sink));
    navigate_to(&mut app, Route::Dns);

    set_dns_field(
        &mut app,
        DnsFormField::Nameserver,
        "https://dns.google/dns-query, quic://dns.adguard.com",
    );
    trigger_dns_edit_apply(&mut app);

    let submitted = sink.submitted();
    assert_eq!(submitted.len(), 1);
    match &submitted[0] {
        UiCommand::ApplyDnsSettings { patch } => {
            assert_eq!(
                patch.nameserver.as_deref(),
                Some(
                    &[
                        "https://dns.google/dns-query".to_owned(),
                        "quic://dns.adguard.com".to_owned()
                    ][..]
                )
            );
            // The whole workbench patch is submitted, including the fallback tier.
            assert!(patch.fallback.is_some());
            assert!(patch.fallback_policy.is_some());
        }
        other => panic!("expected ApplyDnsSettings, got {:?}", other),
    }
}
