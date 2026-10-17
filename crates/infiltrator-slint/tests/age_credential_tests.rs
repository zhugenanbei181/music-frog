//! SLINT-014: Slint Age credential editing and encrypted import verification.
//!
//! Validates:
//! - Configured / NotConfigured credential reading
//! - Read / Set / Clear mutations
//! - Encrypted and unencrypted subscription import
//! - Stale owner scope and stale selection fencing
//! - AgePrivateKey memory scrubbing and redaction
//! - Old unsupported callbacks and error strings are cleared

use infiltrator_slint::age_action::execute_slint_age_action;

use infiltrator_contract::active_profile_reference::{
    ActiveProfileReference, ActiveProfileReferenceRevision, ActiveProfileReferenceState,
};
use infiltrator_contract::command::CommandIntent;
use infiltrator_contract::profile_age_credential::{
    AgePrivateKey, AgePrivateKeyError, ProfileAgeCredentialMutation,
    ProfileAgeCredentialObservation, ProfileAgeCredentialOwnerReadback,
    ProfileAgeCredentialOwnerRequest, ProfileAgeCredentialReadOutcome,
    ProfileAgeCredentialRevision, ProfileAgeCredentialSecret, ProfileAgeCredentialState,
};
use infiltrator_contract::profile_document_identity::ProfileId;
use infiltrator_contract::subscription_import::SubscriptionImportChannel;
use infiltrator_contract::surface::{HostKind, SurfaceKind};
use infiltrator_contract::surface_snapshot::{
    PageData, ProfileSnapshot, ProfilesPageSnapshot, SurfaceSnapshot,
};
use infiltrator_slint::{SlintAction, SlintProductHost};
use lurker_core_contract::error::{ErrorCode, Failure};
use lurker_core_contract::identity::{
    CoreGeneration, CoreIdentity, CoreInstanceId, CoreScope, EngineId,
};
use serde_json::json;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

struct MockSlintHost {
    scope: CoreScope,
    stale_scope: bool,
    stale_selection: bool,
    credential: Mutex<Option<ProfileAgeCredentialObservation>>,
    selection: Mutex<ActiveProfileReferenceState>,
    import_calls: AtomicUsize,
    last_imported: Mutex<Option<(String, String)>>,
}

impl MockSlintHost {
    fn new(scope: CoreScope) -> Self {
        let engine_id = scope.identity.engine.clone();
        let profile_id = ProfileId::new("alpha").unwrap();
        let initial_obs = ProfileAgeCredentialObservation::new(
            engine_id.clone(),
            profile_id.clone(),
            ProfileAgeCredentialRevision::new(1),
            ProfileAgeCredentialState::NotConfigured,
        );
        let initial_selection = ActiveProfileReferenceState::Selected(ActiveProfileReference::new(
            engine_id,
            profile_id,
            ActiveProfileReferenceRevision::new(1),
        ));
        Self {
            scope,
            stale_scope: false,
            stale_selection: false,
            credential: Mutex::new(Some(initial_obs)),
            selection: Mutex::new(initial_selection),
            import_calls: AtomicUsize::new(0),
            last_imported: Mutex::new(None),
        }
    }

    fn make_snapshot(&self) -> SurfaceSnapshot {
        let mut snapshot = SurfaceSnapshot::unavailable(
            SurfaceKind::SlintDesktop,
            HostKind::Desktop,
            Failure::new(ErrorCode::NotReady, "initial", true),
        );
        snapshot.core.identity = Some(self.scope.identity.clone());
        snapshot.core.generation = self.scope.generation.value();
        snapshot.core.session_token = self.scope.session_token;
        let profile: ProfileSnapshot = serde_json::from_value(json!({
            "id": "alpha",
            "name": "Alpha Profile",
            "url": "https://example.com/sub.yaml",
            "updated_at": "",
            "is_active": true
        }))
        .expect("profile snapshot JSON");
        let page: ProfilesPageSnapshot = serde_json::from_value(json!({
            "profiles": [profile],
            "auto_update_interval_hours": 24,
            "updating": false
        }))
        .expect("profiles page JSON");
        snapshot.pages.profiles = PageData::ready(page);
        snapshot
    }
}

impl SlintProductHost for MockSlintHost {
    fn read_snapshot(&self) -> Result<SurfaceSnapshot, Failure> {
        Ok(self.make_snapshot())
    }

    fn execute(
        &self,
        snapshot: Option<&SurfaceSnapshot>,
        action: SlintAction,
    ) -> Result<(), Failure> {
        match action {
            SlintAction::Command(CommandIntent::ImportSubscription {
                profile_id,
                channel: SubscriptionImportChannel::Url,
                source,
            }) => {
                self.import_calls.fetch_add(1, Ordering::SeqCst);
                *self.last_imported.lock().unwrap() = Some((profile_id, source));
                Ok(())
            }
            action @ (SlintAction::ImportEncryptedSubscription { .. }
            | SlintAction::SetProfileAgeCredential { .. }
            | SlintAction::ClearProfileAgeCredential { .. }
            | SlintAction::SaveProfileSettings { .. }) => {
                execute_slint_age_action(self, snapshot, action)
            }
            _ => Err(Failure::unsupported("unsupported in mock")),
        }
    }

    fn execute_age_credential_request(
        &self,
        request: ProfileAgeCredentialOwnerRequest,
    ) -> Result<ProfileAgeCredentialOwnerReadback, Failure> {
        if self.stale_scope || request.scope() != &self.scope {
            return Err(Failure::new(
                ErrorCode::NotReady,
                "Age credential request belongs to a stale Core scope",
                true,
            ));
        }

        let selection = self.selection.lock().unwrap().clone();

        match request {
            ProfileAgeCredentialOwnerRequest::Read { profile_id, .. } => {
                if self.stale_selection {
                    return Ok(ProfileAgeCredentialOwnerReadback::Read(
                        ProfileAgeCredentialReadOutcome::StaleSelection {
                            expected: selection.clone(),
                            observed: selection,
                        },
                    ));
                }
                let mut guard = self.credential.lock().unwrap();
                let current = guard.get_or_insert_with(|| {
                    ProfileAgeCredentialObservation::new(
                        self.scope.identity.engine.clone(),
                        profile_id,
                        ProfileAgeCredentialRevision::new(1),
                        ProfileAgeCredentialState::NotConfigured,
                    )
                });
                Ok(ProfileAgeCredentialOwnerReadback::Read(
                    ProfileAgeCredentialReadOutcome::Observed {
                        current: current.clone(),
                        selection,
                    },
                ))
            }
            ProfileAgeCredentialOwnerRequest::Set {
                expected,
                expected_selection,
                ..
            } => {
                if self.stale_selection || expected_selection != selection {
                    return Ok(ProfileAgeCredentialOwnerReadback::Mutation(
                        ProfileAgeCredentialMutation::StaleSelection {
                            expected: expected_selection,
                            observed: selection,
                        },
                    ));
                }
                let mut guard = self.credential.lock().unwrap();
                let previous = guard.clone().unwrap_or(expected.clone());
                let next_rev = ProfileAgeCredentialRevision::new(previous.revision.value() + 1);
                let current = ProfileAgeCredentialObservation::new(
                    expected.engine_id,
                    expected.profile_id,
                    next_rev,
                    ProfileAgeCredentialState::Configured,
                );
                *guard = Some(current.clone());
                Ok(ProfileAgeCredentialOwnerReadback::Mutation(
                    ProfileAgeCredentialMutation::Updated { previous, current },
                ))
            }
            ProfileAgeCredentialOwnerRequest::Clear {
                expected,
                expected_selection,
                ..
            } => {
                if self.stale_selection || expected_selection != selection {
                    return Ok(ProfileAgeCredentialOwnerReadback::Mutation(
                        ProfileAgeCredentialMutation::StaleSelection {
                            expected: expected_selection,
                            observed: selection,
                        },
                    ));
                }
                let mut guard = self.credential.lock().unwrap();
                let previous = guard.clone().unwrap_or(expected.clone());
                let next_rev = ProfileAgeCredentialRevision::new(previous.revision.value() + 1);
                let current = ProfileAgeCredentialObservation::new(
                    expected.engine_id,
                    expected.profile_id,
                    next_rev,
                    ProfileAgeCredentialState::NotConfigured,
                );
                *guard = Some(current.clone());
                Ok(ProfileAgeCredentialOwnerReadback::Mutation(
                    ProfileAgeCredentialMutation::Cleared { previous, current },
                ))
            }
        }
    }
}

fn test_scope() -> CoreScope {
    CoreScope {
        identity: CoreIdentity::new(
            EngineId::mihomo(),
            CoreInstanceId::new("test").unwrap(),
            None,
        ),
        generation: CoreGeneration::new(1),
        session_token: None,
    }
}

const TEST_KEY: &str =
    "AGE-SECRET-KEY-1QQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQ000000";

#[test]
fn test_age_credential_read_unconfigured_and_configured() {
    let host = MockSlintHost::new(test_scope());
    let snapshot = host.make_snapshot();

    // 1. Initial read should report NotConfigured
    let readback = host
        .execute_age_credential_request(ProfileAgeCredentialOwnerRequest::Read {
            scope: host.scope.clone(),
            profile_id: ProfileId::new("alpha").unwrap(),
        })
        .expect("read age credential");

    let ProfileAgeCredentialOwnerReadback::Read(outcome) = readback else {
        panic!("expected Read readback");
    };
    let ProfileAgeCredentialReadOutcome::Observed { current, .. } = outcome else {
        panic!("expected Observed outcome");
    };
    assert_eq!(current.state, ProfileAgeCredentialState::NotConfigured);

    // 2. Mutate to set a key
    let key = AgePrivateKey::new(TEST_KEY.to_string()).expect("valid key");
    let action = SlintAction::SetProfileAgeCredential {
        profile_id: "alpha".to_string(),
        private_key: key,
    };
    execute_slint_age_action(&host, Some(&snapshot), action).expect("set credential");

    // 3. Second read should report Configured
    let readback2 = host
        .execute_age_credential_request(ProfileAgeCredentialOwnerRequest::Read {
            scope: host.scope.clone(),
            profile_id: ProfileId::new("alpha").unwrap(),
        })
        .expect("read age credential again");

    let ProfileAgeCredentialOwnerReadback::Read(outcome2) = readback2 else {
        panic!("expected Read readback");
    };
    let ProfileAgeCredentialReadOutcome::Observed {
        current: current2, ..
    } = outcome2
    else {
        panic!("expected Observed outcome");
    };
    assert_eq!(current2.state, ProfileAgeCredentialState::Configured);
    assert_eq!(current2.revision.value(), 2);
}

#[test]
fn test_age_credential_set_and_clear() {
    let host = MockSlintHost::new(test_scope());
    let snapshot = host.make_snapshot();

    // 1. Set key
    let key = AgePrivateKey::new(TEST_KEY.to_string()).expect("valid key");
    execute_slint_age_action(
        &host,
        Some(&snapshot),
        SlintAction::SetProfileAgeCredential {
            profile_id: "alpha".to_string(),
            private_key: key,
        },
    )
    .expect("set key");

    assert_eq!(
        host.credential.lock().unwrap().as_ref().unwrap().state,
        ProfileAgeCredentialState::Configured
    );

    // 2. Clear key
    execute_slint_age_action(
        &host,
        Some(&snapshot),
        SlintAction::ClearProfileAgeCredential {
            profile_id: "alpha".to_string(),
        },
    )
    .expect("clear key");

    assert_eq!(
        host.credential.lock().unwrap().as_ref().unwrap().state,
        ProfileAgeCredentialState::NotConfigured
    );
}

#[test]
fn test_encrypted_subscription_import_success_and_plain_import() {
    let host = MockSlintHost::new(test_scope());
    let snapshot = host.make_snapshot();

    // 1. Encrypted import with key sets the credential and executes import
    let key = AgePrivateKey::new(TEST_KEY.to_string()).expect("valid key");
    execute_slint_age_action(
        &host,
        Some(&snapshot),
        SlintAction::ImportEncryptedSubscription {
            name: "encrypted-sub".to_string(),
            url: "https://example.com/sub.enc".to_string(),
            private_key: Some(key),
        },
    )
    .expect("encrypted import");

    assert_eq!(host.import_calls.load(Ordering::SeqCst), 1);
    assert_eq!(
        host.last_imported.lock().unwrap().as_ref(),
        Some(&(
            "encrypted-sub".to_string(),
            "https://example.com/sub.enc".to_string()
        ))
    );
    assert_eq!(
        host.credential.lock().unwrap().as_ref().unwrap().state,
        ProfileAgeCredentialState::Configured
    );

    // 2. Plain import without key imports directly without setting credential
    execute_slint_age_action(
        &host,
        Some(&snapshot),
        SlintAction::ImportEncryptedSubscription {
            name: "plain-sub".to_string(),
            url: "https://example.com/plain.yaml".to_string(),
            private_key: None,
        },
    )
    .expect("plain import");

    assert_eq!(host.import_calls.load(Ordering::SeqCst), 2);
}

#[test]
fn test_stale_owner_scope_rejected() {
    let mut host = MockSlintHost::new(test_scope());
    host.stale_scope = true;
    let snapshot = host.make_snapshot();

    let key = AgePrivateKey::new(TEST_KEY.to_string()).expect("valid key");
    let failure = execute_slint_age_action(
        &host,
        Some(&snapshot),
        SlintAction::SetProfileAgeCredential {
            profile_id: "alpha".to_string(),
            private_key: key,
        },
    )
    .expect_err("stale scope must be rejected");

    assert_eq!(failure.code, ErrorCode::NotReady);
}

#[test]
fn test_stale_selection_rejected() {
    let mut host = MockSlintHost::new(test_scope());
    host.stale_selection = true;
    let snapshot = host.make_snapshot();

    let key = AgePrivateKey::new(TEST_KEY.to_string()).expect("valid key");
    let failure = execute_slint_age_action(
        &host,
        Some(&snapshot),
        SlintAction::SetProfileAgeCredential {
            profile_id: "alpha".to_string(),
            private_key: key,
        },
    )
    .expect_err("stale selection must be rejected");

    assert_eq!(failure.code, ErrorCode::NotReady);
}

#[test]
fn test_age_private_key_validation_and_scrub() {
    // Empty key rejected
    assert!(matches!(
        AgePrivateKey::new("".to_string()),
        Err(AgePrivateKeyError::Empty)
    ));
    assert!(matches!(
        AgePrivateKey::new("   ".to_string()),
        Err(AgePrivateKeyError::Empty)
    ));

    // Untrimmed key rejected
    assert!(matches!(
        AgePrivateKey::new(" key\n".to_string()),
        Err(AgePrivateKeyError::InvalidWhitespace)
    ));

    // Valid key is accepted
    let key = AgePrivateKey::new(TEST_KEY.to_string()).expect("valid");
    assert_eq!(key.as_str(), TEST_KEY);

    // Secret Debug is strictly redacted
    let secret = ProfileAgeCredentialSecret::new(TEST_KEY.as_bytes().to_vec()).unwrap();
    let debug_repr = format!("{secret:?}");
    assert_eq!(debug_repr, "ProfileAgeCredentialSecret([REDACTED])");
    assert!(!debug_repr.contains(TEST_KEY));
}

#[test]
fn test_old_unsupported_strings_removed() {
    let source = include_str!("../src/callbacks/profiles.rs");
    assert!(
        !source.contains("Age-encrypted subscription import is not exposed"),
        "old unsupported string must be removed"
    );
    assert!(
        !source.contains("Changing an Age key requires the shared credential editor"),
        "old unsupported string must be removed"
    );
}
