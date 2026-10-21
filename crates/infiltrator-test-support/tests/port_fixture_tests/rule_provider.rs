//! Behaviour of the isolated rule-provider runtime and cache port fixtures.

use infiltrator_contract::provider_cache::{ProviderContentOrigin, RuleProviderCacheState};
use infiltrator_contract::rule_provider_runtime::{
    RuleProviderId, RuleProviderPayload, RuleProviderRevision,
};
use infiltrator_domain::rules::provider_store::{
    ProviderBehavior, ProviderFormat, RuleProviderDeclaration,
};
use infiltrator_ports::rule_provider_cache::RuleProviderCachePort;
use infiltrator_ports::rule_provider_runtime::RuleProviderPort;
use infiltrator_test_support::rule_provider_fixtures::{
    CAPTURE_ENGINE, CAPTURE_PROVIDER, CAPTURE_PROVIDER_RULES, CAPTURE_SECOND_PROVIDER,
    RuleProviderCacheCapture, RuleProviderCapturePort,
};
use lurker_core_contract::identity::{
    CoreGeneration, CoreIdentity, CoreInstanceId, CoreScope, CoreVersion, EngineId,
};
use lurker_core_ports::error::PortError;

fn capture_scope() -> CoreScope {
    CoreScope {
        identity: CoreIdentity::new(
            EngineId::new(CAPTURE_ENGINE).expect("valid engine ID"),
            CoreInstanceId::new("capture-core").expect("valid instance ID"),
            Some(CoreVersion::new("1.19.30").expect("valid version")),
        ),
        generation: CoreGeneration::new(1),
        session_token: None,
    }
}

fn provider_id(name: &str) -> RuleProviderId {
    RuleProviderId::new(
        EngineId::new(CAPTURE_ENGINE).expect("valid engine ID"),
        name,
    )
    .expect("valid provider ID")
}

fn declaration(name: &str) -> RuleProviderDeclaration {
    RuleProviderDeclaration {
        name: name.to_owned(),
        provider_type: "http".to_owned(),
        behavior: ProviderBehavior::Domain,
        format: ProviderFormat::Yaml,
        url: Some(format!("https://provider.example.com/{name}.yaml")),
        path: None,
        payload: Vec::new(),
    }
}

#[tokio::test]
async fn the_inventory_publishes_the_observed_providers() {
    let port = RuleProviderCapturePort::new();
    let inventory = port
        .inventory(capture_scope())
        .await
        .expect("provider inventory");

    assert_eq!(inventory.providers().len(), 2);
    assert_eq!(inventory.revision(), RuleProviderRevision::initial());
    assert!(inventory.provider(&provider_id(CAPTURE_PROVIDER)).is_some());
    assert!(
        inventory
            .provider(&provider_id(CAPTURE_SECOND_PROVIDER))
            .is_some()
    );
}

#[tokio::test]
async fn a_refresh_advances_the_inventory_revision() {
    let port = RuleProviderCapturePort::new();
    let scope = capture_scope();
    let inventory = port.inventory(scope.clone()).await.expect("inventory");

    let receipt = port
        .refresh(scope, inventory.revision(), provider_id(CAPTURE_PROVIDER))
        .await
        .expect("refresh receipt");

    assert_eq!(receipt.requested_revision(), inventory.revision());
    assert_eq!(
        receipt.inventory().revision(),
        RuleProviderRevision::initial()
            .next()
            .expect("next revision")
    );
    assert_eq!(port.current_revision(), receipt.inventory().revision());
}

#[tokio::test]
async fn a_stale_revision_is_refused_instead_of_refreshing() {
    let port = RuleProviderCapturePort::new();
    let scope = capture_scope();
    let inventory = port.inventory(scope.clone()).await.expect("inventory");
    let stale = inventory.revision();

    port.refresh(scope.clone(), stale, provider_id(CAPTURE_PROVIDER))
        .await
        .expect("first refresh");

    let error = port
        .refresh(scope, stale, provider_id(CAPTURE_PROVIDER))
        .await
        .expect_err("the stale refresh must fail");

    assert!(matches!(error, PortError::Rejected(_)));
}

#[tokio::test]
async fn a_failing_refresh_is_a_typed_failure() {
    let port = RuleProviderCapturePort::new();
    port.fail_refresh("the provider host refused the download");
    let scope = capture_scope();
    let inventory = port.inventory(scope.clone()).await.expect("inventory");

    let error = port
        .refresh(scope, inventory.revision(), provider_id(CAPTURE_PROVIDER))
        .await
        .expect_err("the refresh must fail");

    assert!(matches!(error, PortError::Network(_)));
    assert_eq!(port.current_revision(), inventory.revision());
}

#[tokio::test]
async fn an_unknown_provider_is_not_found() {
    let port = RuleProviderCapturePort::new();
    let scope = capture_scope();
    let inventory = port.inventory(scope.clone()).await.expect("inventory");

    let error = port
        .refresh(scope, inventory.revision(), provider_id("never-declared"))
        .await
        .expect_err("an unknown provider must not be found");

    assert!(matches!(error, PortError::NotFound(_)));
}

#[tokio::test]
async fn the_payload_readback_publishes_the_published_rules() {
    let port = RuleProviderCapturePort::new();
    let scope = capture_scope();
    let inventory = port.inventory(scope.clone()).await.expect("inventory");

    let payload = port
        .payload(scope, inventory.revision(), provider_id(CAPTURE_PROVIDER))
        .await
        .expect("provider payload");

    match payload.payload() {
        RuleProviderPayload::Published(rules) => {
            assert_eq!(rules.len(), 2);
            assert_eq!(rules[0], CAPTURE_PROVIDER_RULES[0]);
            assert_eq!(rules[1], CAPTURE_PROVIDER_RULES[1]);
        }
        other => panic!("expected a published payload, got {other:?}"),
    }
}

#[tokio::test]
async fn the_cache_publishes_the_real_cached_bytes() {
    let cache = RuleProviderCacheCapture::new();

    let entry = cache
        .read_provider(&declaration(CAPTURE_PROVIDER))
        .await
        .expect("provider cache entry")
        .expect("the capture provider has a cached file");
    assert_eq!(entry.origin, ProviderContentOrigin::KernelCacheFile);
    assert!(!entry.bytes.is_empty());

    let fact = cache
        .fingerprint(&declaration(CAPTURE_PROVIDER))
        .await
        .expect("provider fingerprint")
        .expect("the capture provider has a fingerprint");
    assert_eq!(fact.fingerprint.size_bytes, entry.bytes.len() as u64);
    assert!(!fact.fingerprint.sha256.is_empty());
}

#[tokio::test]
async fn a_provider_without_a_cached_file_answers_with_no_entry() {
    let cache = RuleProviderCacheCapture::new();

    let entry = cache
        .read_provider(&declaration("never-downloaded"))
        .await
        .expect("provider cache entry");

    assert!(entry.is_none());
    assert!(
        cache
            .fingerprint(&declaration("never-downloaded"))
            .await
            .expect("provider fingerprint")
            .is_none()
    );
}

#[tokio::test]
async fn the_purge_reports_the_real_counts() {
    let cache = RuleProviderCacheCapture::new();
    let before = cache.snapshot().await.expect("cache snapshot");
    assert_eq!(before.state, RuleProviderCacheState::Ready);
    assert_eq!(before.file_count, 1);

    let purge = cache.purge().await.expect("cache purge");

    assert_eq!(purge.files_removed, 1);
    assert!(purge.bytes_freed > 0);
    let after = cache.snapshot().await.expect("cache snapshot");
    assert_eq!(after.state, RuleProviderCacheState::Empty);
    assert_eq!(after.file_count, 0);
}
