use crate::active_reference_cas::{
    ActiveProfileReferenceCasReady, ActiveProfileReferenceOwnerFence,
};
use infiltrator_contract::active_profile_reference::{
    ActiveProfileReferenceRevision, ActiveProfileReferenceState, ActiveProfileReferenceTarget,
};
use infiltrator_contract::core_identity::{CoreGeneration, CoreIdentity, CoreInstanceId, EngineId};
use infiltrator_contract::error::ErrorCode;
use infiltrator_contract::profile_document_identity::ProfileId;
use infiltrator_ports::error::PortError;

fn unselected(revision: u64) -> ActiveProfileReferenceState {
    ActiveProfileReferenceState::Unselected {
        revision: ActiveProfileReferenceRevision::new(revision),
    }
}

fn identity(engine: &str) -> CoreIdentity {
    CoreIdentity::new(
        EngineId::new(engine).unwrap(),
        CoreInstanceId::new("core-1").unwrap(),
        None,
    )
}

#[test]
fn revision_compare_produces_a_typed_transition_witness() {
    let ready = ActiveProfileReferenceCasReady::compare(
        &unselected(4),
        ActiveProfileReferenceRevision::new(4),
    )
    .unwrap();
    assert_eq!(ready.previous(), &unselected(4));
    let next = ready
        .transition(
            ActiveProfileReferenceTarget::selected(
                EngineId::mihomo(),
                ProfileId::new("main").unwrap(),
            ),
            ActiveProfileReferenceOwnerFence::Unowned,
        )
        .unwrap();

    assert_eq!(next.revision(), ActiveProfileReferenceRevision::new(5));
}

#[test]
fn stale_revision_fails_before_transition() {
    let error = ActiveProfileReferenceCasReady::compare(
        &unselected(5),
        ActiveProfileReferenceRevision::new(4),
    )
    .unwrap_err();

    assert!(matches!(
        error,
        PortError::Rejected(ref failure) if failure.code == ErrorCode::NotReady
    ));
}

#[test]
fn bound_owner_can_reselect_within_its_engine_but_cannot_clear_or_switch_engines() {
    let owner = ActiveProfileReferenceOwnerFence::Bound {
        identity: identity("mihomo"),
        generation: CoreGeneration::new(8),
    };
    let ready = ActiveProfileReferenceCasReady::compare(
        &unselected(1),
        ActiveProfileReferenceRevision::new(1),
    )
    .unwrap();
    assert!(
        ready
            .clone()
            .transition(
                ActiveProfileReferenceTarget::selected(
                    EngineId::mihomo(),
                    ProfileId::new("next").unwrap(),
                ),
                owner.clone(),
            )
            .is_ok()
    );
    assert!(
        ready
            .clone()
            .transition(ActiveProfileReferenceTarget::Unselected, owner.clone())
            .is_err()
    );
    assert!(
        ready
            .transition(
                ActiveProfileReferenceTarget::selected(
                    EngineId::sing_box(),
                    ProfileId::new("next").unwrap(),
                ),
                owner,
            )
            .is_err()
    );
}

#[test]
fn switching_owner_rejects_reference_mutations() {
    let ready = ActiveProfileReferenceCasReady::compare(
        &unselected(1),
        ActiveProfileReferenceRevision::new(1),
    )
    .unwrap();
    let error = ready
        .transition(
            ActiveProfileReferenceTarget::Unselected,
            ActiveProfileReferenceOwnerFence::Switching,
        )
        .unwrap_err();

    assert!(matches!(
        error,
        PortError::Rejected(ref failure) if failure.code == ErrorCode::NotReady
    ));
}
