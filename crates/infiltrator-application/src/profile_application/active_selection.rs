use super::ProfileApplication;
use infiltrator_contract::active_profile_reference::{
    ActiveProfileReferenceCasOutcome, ActiveProfileReferenceState, ActiveProfileReferenceTarget,
};
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::profile_selection_clear::{
    ClearActiveProfileSelectionOutcome, ClearActiveProfileSelectionRequest,
    UnselectedProfileSelectionReadback,
};

impl ProfileApplication {
    /// Read the current active profile selection reference.
    pub async fn read_active_profile_reference(&self) -> Result<ActiveProfileReferenceState, Failure> {
        self.store.read_active_profile_reference().await.map_err(Failure::from)
    }

    /// Clear this engine's selected profile through the owner-authorized
    /// revision CAS. The operation only changes the selection reference; it
    /// never stops the independently owned Core service or VPN.
    pub async fn clear_active_profile_selection(
        &self,
        request: ClearActiveProfileSelectionRequest,
    ) -> Result<ClearActiveProfileSelectionOutcome, Failure> {
        let engine_id = self.store.engine_id();
        if let ActiveProfileReferenceState::Selected(reference) = &request.expected
            && reference.engine_id != engine_id
        {
            return Err(Failure::new(
                ErrorCode::InvalidInput,
                format!(
                    "cannot clear profile `{}` from foreign engine `{}` through `{}`",
                    reference.profile_id.as_str(),
                    reference.engine_id.as_str(),
                    engine_id.as_str()
                ),
                false,
            ));
        }

        let observed = self
            .store
            .read_active_profile_reference()
            .await
            .map_err(Failure::from)?;
        if observed != request.expected {
            return Ok(ClearActiveProfileSelectionOutcome::Stale {
                expected: request.expected,
                observed,
            });
        }

        let ActiveProfileReferenceState::Selected(previous) = request.expected else {
            let ActiveProfileReferenceState::Unselected { revision } = observed else {
                return Err(internal_readback_failure(
                    "active profile selection changed after an equal unselected readback",
                ));
            };
            return Ok(ClearActiveProfileSelectionOutcome::AlreadyUnselected {
                current: UnselectedProfileSelectionReadback { revision },
            });
        };
        let expected = ActiveProfileReferenceState::Selected(previous.clone());
        match self
            .store
            .compare_and_set_active_profile_reference(
                expected.clone(),
                ActiveProfileReferenceTarget::Unselected,
                request.authorization,
            )
            .await
            .map_err(Failure::from)?
        {
            ActiveProfileReferenceCasOutcome::Stale { expected, observed } => {
                Ok(ClearActiveProfileSelectionOutcome::Stale { expected, observed })
            }
            ActiveProfileReferenceCasOutcome::Committed {
                previous: committed_previous,
                current,
            } => {
                if committed_previous != expected {
                    return Err(internal_readback_failure(
                        "active profile clear compare returned a different previous selection",
                    ));
                }
                let ActiveProfileReferenceState::Unselected { revision } = current else {
                    return Err(internal_readback_failure(
                        "active profile clear compare did not return an unselected readback",
                    ));
                };
                if Some(revision) != previous.revision.next() {
                    return Err(internal_readback_failure(
                        "active profile clear readback did not advance the selection revision once",
                    ));
                }
                Ok(ClearActiveProfileSelectionOutcome::Cleared {
                    previous,
                    current: UnselectedProfileSelectionReadback { revision },
                })
            }
        }
    }
}

fn internal_readback_failure(message: &str) -> Failure {
    Failure::new(ErrorCode::Internal, message.to_owned(), false)
}
