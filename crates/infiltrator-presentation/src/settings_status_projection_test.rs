//! test-intent: behavior
use super::*;
use infiltrator_contract::core_identity::EngineId;
use infiltrator_contract::core_identity::{
    CoreGeneration, CoreIdentity, CoreInstanceId, CoreScope, CoreVersion,
};
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::resources::ResourceAvailability;

#[test]
fn resource_copy_uses_the_observed_limit_and_distinguishes_zero_missing_and_unsampled() {
    let snapshot = CoreResourceSnapshot::Observed {
        scope: Box::new(CoreScope {
            identity: CoreIdentity::new(
                EngineId::mihomo(),
                CoreInstanceId::new("projection-test").expect("instance"),
                Some(CoreVersion::new("test").expect("version")),
            ),
            generation: CoreGeneration::new(1),
            session_token: None,
        }),
        memory: ResourceAvailability::Available(0),
        cpu_percent: ResourceAvailability::Available(0.0),
        memory_soft_limit_bytes: 128 * 1024 * 1024,
        gc: Box::new(CoreGcStatus::Collected {
            before_bytes: 1,
            after_bytes: 0,
        }),
    };
    let en = format_core_resources(&snapshot, "en-US");
    let zh = format_core_resources(&snapshot, "zh-CN");
    assert_eq!(
        en,
        "Memory=0.0 MiB · CPU=0.0% · limit=128.0 MiB · GC=Collected, after=0.0 MiB"
    );
    assert_eq!(
        zh,
        "内存=0.0 MiB · CPU=0.0% · 上限=128.0 MiB · GC=已回收，读回=0.0 MiB"
    );
    assert_eq!(
        format_core_resources(&CoreResourceSnapshot::default(), "en-US"),
        "Memory=Not observed · CPU=Not observed · limit=512.0 MiB · GC=Not attempted"
    );
}

#[test]
fn ready_mtu_with_missing_observations_never_reports_zero_or_claims_application() {
    let snapshot = MtuNegotiationSnapshot {
        state: MtuProbeState::Ready,
        ..Default::default()
    };
    let en = format_mtu(&snapshot, "en-US");
    assert_eq!(
        en,
        "Active link: physical=? → TUN=? · MSS=? · overhead=80 · applied=Not applied"
    );
    assert!(!en.contains("=0"));
}

#[test]
fn status_failure_is_redacted_and_user_braces_are_not_reinterpreted_in_either_locale() {
    let rejected = CoreArtifactVerification::Rejected {
        engine_id: EngineId::new("mihomo").unwrap(),
        version: "v1 {message}".into(),
        failure: Failure::new(
            ErrorCode::Authentication,
            "https://example.test/?token=private&reason={version}",
            true,
        ),
    };
    for locale in ["zh-CN", "en-US"] {
        let text = format_integrity(&rejected, locale);
        assert!(!text.contains("private"));
        assert!(text.contains("v1 {message}"));
        assert!(text.contains("reason={version}"));
    }
}
