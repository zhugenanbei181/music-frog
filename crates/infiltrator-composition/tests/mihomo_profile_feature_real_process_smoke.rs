//! test-intent: real_process

use infiltrator_application::runtime_configuration_application::RuntimeConfigurationApplication;
use infiltrator_ports::runtime_configuration::ObservedValue;
use lurker_core_contract::identity::{
    CoreGeneration, CoreIdentity, CoreInstanceId, CoreScope, CoreVersion, EngineId,
};
use lurker_core_contract::session::SessionToken;
use mihomo_api::runtime_configuration::{MihomoRuntimeConfigurationAdapter, RuntimeScopeResolver};
use mihomo_driver::api::client::MihomoClient;
use std::env;
use std::sync::Arc;

fn required_environment(name: &str) -> String {
    env::var(name).unwrap_or_else(|_| panic!("{name} must be set by the explicit Mihomo smoke"))
}

#[tokio::test]
#[ignore = "requires the explicit SHA-locked Mihomo real-process smoke"]
async fn profile_apply_feature_ipv6_write_reads_back_and_restores_with_tun_disabled() {
    let controller_url = required_environment("MIHOMO_REAL_CONTROLLER_URL");
    let controller_secret = required_environment("MIHOMO_REAL_CONTROLLER_SECRET");
    let expected_version = required_environment("MIHOMO_REAL_VERSION");
    let client = MihomoClient::new(&controller_url, Some(controller_secret))
        .expect("construct production Mihomo controller client");
    let version = client
        .get_version()
        .await
        .expect("read live Mihomo version");
    assert_eq!(version.version, expected_version);

    let scope = CoreScope {
        identity: CoreIdentity::new(
            EngineId::new("mihomo").expect("Mihomo engine ID"),
            CoreInstanceId::new("core-008-profile-feature-real-process").expect("instance ID"),
            Some(CoreVersion::new(expected_version).expect("locked Mihomo version")),
        ),
        generation: CoreGeneration::new(1),
        session_token: Some(SessionToken::new(1)),
    };
    let resolver_scope = scope.clone();
    let scope_resolver: RuntimeScopeResolver = Arc::new(move || Ok(resolver_scope.clone()));
    let application = RuntimeConfigurationApplication::new(
        scope.clone(),
        MihomoRuntimeConfigurationAdapter::ports(client, Some(scope_resolver)),
    );

    let before = application
        .observe_ipv6()
        .await
        .expect("observe the current live IPv6 profile feature");
    assert_eq!(before.scope, scope);
    assert_eq!(before.value.enabled, ObservedValue::Reported(false));
    assert_eq!(before.value.tun_enabled, ObservedValue::Reported(false));

    let enabled = application
        .set_ipv6_routing(true)
        .await
        .expect("apply the typed Profile.ApplyFeature IPv6 command");
    assert!(enabled.enabled);
    assert!(!enabled.tun_enabled);

    let restored = application
        .set_ipv6_routing(false)
        .await
        .expect("restore the original IPv6 profile feature");
    assert!(!restored.enabled);
    assert!(!restored.tun_enabled);

    let final_readback = application
        .observe_ipv6()
        .await
        .expect("confirm the live controller restored the original values");
    assert_eq!(final_readback.scope, scope);
    assert_eq!(final_readback.value.enabled, ObservedValue::Reported(false));
    assert_eq!(
        final_readback.value.tun_enabled,
        ObservedValue::Reported(false)
    );
}
