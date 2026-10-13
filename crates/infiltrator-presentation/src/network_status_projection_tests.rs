//! test-intent: behavior
use super::{
    roaming_event, roaming_interfaces, roaming_mtu, roaming_route, roaming_status,
    system_proxy_status,
};
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::network_roaming::{
    NetworkRoamingEvent, NetworkRoamingSnapshot, NetworkRoamingStatus,
};
use infiltrator_contract::system_proxy::{
    SystemProxyOwnership, SystemProxyRecoverySnapshot, SystemProxyRecoveryStatus,
    SystemProxySnapshot, SystemProxyStatus,
};

#[test]
fn missing_network_facts_are_unknown_and_observed_zero_is_preserved() {
    let mut snapshot = NetworkRoamingSnapshot::default();
    assert_eq!(
        roaming_mtu(&snapshot, "en-US"),
        "Physical MTU — → TUN — · MSS —"
    );
    assert_eq!(
        roaming_interfaces(&snapshot, "en-US"),
        "Interface facts not observed"
    );
    assert_eq!(
        roaming_route(&snapshot, "en-US"),
        "Active interface — · Gateway — · Physical MTU — → TUN — · MSS — · No recent event"
    );
    snapshot.physical_mtu = Some(1500);
    snapshot.recommended_tun_mtu = Some(1420);
    snapshot.tcp_mss = Some(0);
    assert_eq!(
        roaming_mtu(&snapshot, "en-US"),
        "Physical MTU 1500 → TUN 1420 · MSS 0"
    );
    assert_eq!(
        roaming_mtu(&snapshot, "zh-CN"),
        "物理 MTU 1500 → TUN 1420 · MSS 0"
    );
    assert_eq!(snapshot.tcp_mss, Some(0));
}

#[test]
fn gateway_changes_keep_both_interfaces_and_addresses_without_reinterpreting_data() {
    let event = NetworkRoamingEvent::GatewayChanged {
        old_interface: Some("eth{gateway}".into()),
        new_interface: Some("wlan{event}".into()),
        old_gateway_ip: Some("192.0.2.1".into()),
        new_gateway_ip: Some("192.0.2.2".into()),
    };
    assert_eq!(
        roaming_event(&event, "en-US"),
        "Gateway changed eth{gateway} → wlan{event} (192.0.2.1 → 192.0.2.2)"
    );
    let snapshot = NetworkRoamingSnapshot {
        last_event: Some(event),
        active_interface: Some("{mtu}".into()),
        ..Default::default()
    };
    assert_eq!(
        roaming_route(&snapshot, "en-US"),
        "Active interface {mtu} · Gateway — · Physical MTU — → TUN — · MSS — · Gateway changed eth{gateway} → wlan{event} (192.0.2.1 → 192.0.2.2)"
    );
}

#[test]
fn network_failure_reason_is_redacted_and_unsupported_stays_distinct() {
    let failure = Failure::new(ErrorCode::Network, "token=privatevalue {status}", true);
    assert_eq!(
        roaming_status(
            &NetworkRoamingStatus::Failed {
                failure: failure.clone()
            },
            "en-US"
        ),
        "Repair failed · token=*** {status}"
    );
    assert_eq!(
        roaming_status(
            &NetworkRoamingStatus::Unsupported {
                reason: "adapter missing".into()
            },
            "en-US"
        ),
        "Host unsupported · adapter missing"
    );
    assert_eq!(
        roaming_event(&NetworkRoamingEvent::RepairFailed { failure }, "en-US"),
        "Repair failed · token=*** {status}"
    );
}

#[test]
fn system_proxy_ownership_and_recovery_replay_the_same_facts_in_both_locales() {
    let snapshot = SystemProxySnapshot {
        status: SystemProxyStatus::Enabled,
        ownership: SystemProxyOwnership::Owned,
        endpoint: Some("127.0.0.1:7890".into()),
        ..Default::default()
    };
    let mut recovery = SystemProxyRecoverySnapshot::default();
    assert_eq!(
        system_proxy_status(&snapshot, &recovery, "en-US"),
        "Owned · 127.0.0.1:7890"
    );
    assert_eq!(
        system_proxy_status(&snapshot, &recovery, "zh-CN"),
        "已接管 · 127.0.0.1:7890"
    );
    recovery.status = SystemProxyRecoveryStatus::Failed {
        failure: Failure::new(ErrorCode::Storage, "password=privatevalue {status}", false),
    };
    assert_eq!(
        system_proxy_status(&snapshot, &recovery, "en-US"),
        "Startup recovery failed · password=*** {status}"
    );
    assert_eq!(
        system_proxy_status(
            &SystemProxySnapshot::default(),
            &SystemProxyRecoverySnapshot::default(),
            "en-US"
        ),
        "Not probed"
    );
}
