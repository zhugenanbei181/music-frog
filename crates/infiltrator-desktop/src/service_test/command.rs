//! Behavior cases for command.
//! test-intent: behavior

use super::*;

#[tokio::test]
async fn test_command_sequences_with_mock_service_harness() {
    use super::state_machine::CommandSequence;

    let harness = MockServiceHarness::with_privilege(PrivilegeLevel::Admin);

    // 1. Execute TUN Startup Sequence
    let tun_seq = CommandSequence::tun_startup_sequence(
        Some("tun0".to_string()),
        Some("/etc/mihomo/config.yaml".to_string()),
    );
    assert_eq!(tun_seq.commands.len(), 3);
    let res = harness.execute_sequence(&tun_seq).await;
    assert!(res.all_successful());
    assert_eq!(res.step_results.len(), 3);

    // 2. Execute System Proxy Sequence
    let proxy_seq =
        CommandSequence::system_proxy_sequence("127.0.0.1:7890", Some("localhost".to_string()));
    let res_proxy = harness.execute_sequence(&proxy_seq).await;
    assert!(res_proxy.all_successful());
    assert_eq!(res_proxy.step_results.len(), 3);

    // 3. Execute Teardown Sequence
    let teardown_seq = CommandSequence::teardown_sequence();
    let res_teardown = harness.execute_sequence(&teardown_seq).await;
    assert!(res_teardown.all_successful());
    assert_eq!(res_teardown.step_results.len(), 3);

    // 4. Custom Sequence
    let custom_seq = CommandSequence::new("PingPongStatus")
        .then(ServiceCommand::Ping { nonce: 112233 })
        .then(ServiceCommand::QueryStatus);
    let res_custom = harness.execute_sequence(&custom_seq).await;
    assert!(res_custom.all_successful());
    assert_eq!(res_custom.step_results.len(), 2);
}
