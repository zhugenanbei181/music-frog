//! test-intent: real_process

use infiltrator_application::runtime_configuration_application::RuntimeConfigurationApplication;
use infiltrator_contract::core_identity::{
    CoreGeneration, CoreIdentity, CoreInstanceId, CoreScope, CoreVersion, EngineId,
};
use infiltrator_contract::tun::TunStack;
use infiltrator_ports::runtime_configuration::ObservedValue;
use mihomo_api::client::MihomoClient;
use mihomo_api::runtime_configuration::{MihomoRuntimeConfigurationAdapter, RuntimeScopeResolver};
use std::env;
use std::sync::Arc;

fn required_environment(name: &str) -> String {
    env::var(name).unwrap_or_else(|_| panic!("{name} must be set by the explicit Mihomo smoke"))
}

#[tokio::test]
#[ignore = "requires the explicit SHA-locked Mihomo real-process smoke"]
async fn runtime_configuration_applies_and_reads_back_tun_stack_while_tun_stays_disabled() {
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
            CoreInstanceId::new("core-008-tun-config-real-process").expect("instance ID"),
            Some(CoreVersion::new(expected_version).expect("locked Mihomo version")),
        ),
        generation: CoreGeneration::new(1),
        session_token: None,
    };
    let resolver_scope = scope.clone();
    let scope_resolver: RuntimeScopeResolver = Arc::new(move || Ok(resolver_scope.clone()));
    let application = RuntimeConfigurationApplication::new(
        scope.clone(),
        MihomoRuntimeConfigurationAdapter::ports(client, Some(scope_resolver)),
    );

    let before = application
        .observe_tun()
        .await
        .expect("observe the real TUN configuration");
    assert_eq!(before.scope, scope);
    assert_eq!(before.value.enabled, ObservedValue::Reported(false));
    assert_eq!(
        before.value.stack,
        ObservedValue::Reported(TunStack::System.as_str().to_owned())
    );

    let changed = application
        .set_tun_stack(TunStack::Mixed)
        .await
        .expect("apply a live TUN stack change through the application and adapter");
    assert_eq!(changed.scope, scope);
    assert_eq!(
        changed.value.stack,
        ObservedValue::Reported(TunStack::Mixed.as_str().to_owned())
    );
    assert_eq!(changed.value.enabled, ObservedValue::Reported(false));

    let restored = application
        .set_tun_stack(TunStack::System)
        .await
        .expect("restore the original TUN stack through the typed application");
    assert_eq!(restored.scope, scope);
    assert_eq!(restored.value.stack, before.value.stack);
    assert_eq!(restored.value.enabled, ObservedValue::Reported(false));

    let final_readback = application
        .observe_tun()
        .await
        .expect("confirm restored TUN configuration from the live controller");
    assert_eq!(final_readback, restored);
}
