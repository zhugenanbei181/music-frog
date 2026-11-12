use super::{DEMO_ASN, DEMO_IP, DEMO_ISP, PublicIpProbeOwner};
use infiltrator_contract::public_ip::{PublicIpProbeSnapshot, PublicIpProbeStatus};
use infiltrator_contract::public_ip_probe::{
    PublicIpProbeAnswer, PublicIpProbeError, PublicIpProbePhase, PublicIpProbeStamp,
};
use infiltrator_shared::locales::{Lang, Localizer};

const PROBE_FAILURE_KEY: &str = "public_ip_failed";

#[test]
fn initial_readback_equals_the_shared_demo_fixture() {
    let owner = PublicIpProbeOwner::default();

    assert_eq!(owner.peek_snapshot(), PublicIpProbeSnapshot::demo_fixture());
    assert_eq!(owner.state().phase(), PublicIpProbePhase::Succeeded);
}

#[test]
fn refresh_publishes_probing_then_a_typed_failure() {
    let mut owner = PublicIpProbeOwner::default();
    let answer = owner.begin_refresh("en").expect("refresh begins");
    assert!(matches!(answer, PublicIpProbeAnswer::Failed { .. }));

    let probing = owner.snapshot();
    assert_eq!(probing.status, PublicIpProbeStatus::Probing);
    assert_eq!(probing.failure, None);
    assert_eq!(probing.ip, None);
    assert!(!probing.is_drawable());
    assert_eq!(owner.state().phase(), PublicIpProbePhase::Probing);

    let failed = owner.snapshot();
    assert_eq!(failed.status, PublicIpProbeStatus::Failed);
    assert_eq!(
        failed.failure.as_deref(),
        Some(Lang("en").tr(PROBE_FAILURE_KEY).as_ref())
    );
    assert_eq!(failed.ip, None);
    assert!(!failed.is_drawable());
    assert_eq!(owner.state().phase(), PublicIpProbePhase::Failed);
}

#[test]
fn a_second_refresh_publishes_probing_then_a_typed_success() {
    let mut owner = PublicIpProbeOwner::default();
    let first = owner.begin_refresh("zh-CN").expect("first refresh begins");
    let first_probing = owner.snapshot();
    let first_failed = owner.snapshot();
    assert_eq!(
        first_failed.failure.as_deref(),
        Some(Lang("zh-CN").tr(PROBE_FAILURE_KEY).as_ref())
    );

    let second = owner.begin_refresh("en").expect("second refresh begins");
    assert!(matches!(second, PublicIpProbeAnswer::Succeeded { .. }));
    assert_ne!(first, second);
    assert_eq!(first_probing.status, owner.peek_snapshot().status);
    let probing = owner.snapshot();
    assert_eq!(probing.status, PublicIpProbeStatus::Probing);
    assert!(!probing.is_drawable());

    let ready = owner.snapshot();
    assert_eq!(ready.status, PublicIpProbeStatus::Ready);
    assert_eq!(ready.failure, None);
    assert_eq!(ready.ip.as_deref(), Some(DEMO_IP));
    assert_eq!(ready.isp.as_deref(), Some(DEMO_ISP));
    assert_eq!(ready.asn, Some(DEMO_ASN));
    assert!(ready.is_drawable());
    assert_ne!(
        ready.revision,
        PublicIpProbeSnapshot::demo_fixture().revision
    );
    assert_eq!(owner.state().phase(), PublicIpProbePhase::Succeeded);
}

#[test]
fn stale_answer_does_not_change_the_current_state() {
    let mut owner = PublicIpProbeOwner::default();
    let answer = owner.begin_refresh("en").expect("refresh begins");
    let stale = match answer {
        PublicIpProbeAnswer::Failed { stamp, reason } => PublicIpProbeAnswer::Failed {
            stamp: PublicIpProbeStamp {
                request_id: stamp.request_id + 100,
                ..stamp
            },
            reason,
        },
        other => panic!("the first demo refresh must be a failure, got {other:?}"),
    };
    let before = owner.state().clone();

    assert_eq!(
        owner.accept_answer(stale).unwrap_err(),
        PublicIpProbeError::StaleAnswer
    );
    assert_eq!(*owner.state(), before);
    assert_eq!(owner.peek_snapshot().status, PublicIpProbeStatus::Probing);
}

#[test]
fn failure_reason_comes_from_the_shared_language_catalog() {
    let mut owner = PublicIpProbeOwner::default();
    owner.begin_refresh("en").expect("refresh begins");
    owner.snapshot();
    let failed = owner.snapshot();

    assert_eq!(
        failed.failure.as_deref(),
        Some(Lang("en").tr(PROBE_FAILURE_KEY).as_ref())
    );
}
