//! test-intent: behavior
use super::*;
use infiltrator_contract::error::{ErrorCode, Failure};

#[test]
fn resource_copy_uses_the_observed_limit_and_distinguishes_zero_missing_and_unsampled() {
    let snapshot = CoreResourceSnapshot {
        memory_bytes: Some(0),
        cpu_percent: Some(0.0),
        memory_soft_limit_bytes: 128 * 1024 * 1024,
        gc: CoreGcStatus::Triggered {
            before_bytes: 1,
            after_bytes: Some(0),
        },
    };
    let en = format_core_resources(&snapshot, "en-US");
    let zh = format_core_resources(&snapshot, "zh-CN");
    assert_eq!(
        en,
        "Memory=0.0 MiB · CPU=0.0% · limit=128.0 MiB · GC=Triggered, after=0.0 MiB"
    );
    assert_eq!(
        zh,
        "内存=0.0 MiB · CPU=0.0% · 上限=128.0 MiB · GC=已触发，之后=0.0 MiB"
    );
    assert_eq!(
        format_core_resources(&CoreResourceSnapshot::default(), "en-US"),
        "Memory=? MiB · CPU=?% · limit=512.0 MiB · GC=Not sampled"
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
