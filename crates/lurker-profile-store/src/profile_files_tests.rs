use crate::profile_files::{ProfileDeletionParticipant, ProfileDeletionRollback};
use crate::store::ProfileDocumentExtension;
use crate::test_core::TestCore;
use async_trait::async_trait;
use lurker_core_contract::active_profile_reference::{
    ActiveProfileReference, ActiveProfileReferenceRevision, ActiveProfileReferenceState,
};
use lurker_core_contract::identity::EngineId;
use lurker_core_contract::profile_document_identity::ProfileId;
use lurker_core_ports::error::PortError;
use std::sync::Mutex;
use tokio::fs;

#[derive(Default)]
struct FakeParticipant {
    retired: Mutex<Vec<String>>,
    completed: Mutex<Vec<String>>,
    rolled_back: Mutex<Vec<String>>,
    fail_completion: bool,
    refuse_retirement: bool,
}

impl FakeParticipant {
    fn failing_completion() -> Self {
        Self {
            fail_completion: true,
            ..Self::default()
        }
    }

    fn refusing_retirement() -> Self {
        Self {
            refuse_retirement: true,
            ..Self::default()
        }
    }

    fn retired(&self) -> Vec<String> {
        self.retired.lock().expect("retired lock").clone()
    }

    fn completed(&self) -> Vec<String> {
        self.completed.lock().expect("completed lock").clone()
    }

    fn rolled_back(&self) -> Vec<String> {
        self.rolled_back.lock().expect("rolled back lock").clone()
    }
}

struct FakeRollback<'a> {
    participant: &'a FakeParticipant,
    profile: String,
}

#[async_trait]
impl ProfileDeletionParticipant for FakeParticipant {
    async fn retire_for_deletion<'a>(
        &'a self,
        profile: &ProfileId,
    ) -> Result<Box<dyn ProfileDeletionRollback + 'a>, PortError> {
        if self.refuse_retirement {
            return Err(PortError::Failed("product state refused retirement".into()));
        }
        self.retired
            .lock()
            .expect("retired lock")
            .push(profile.as_str().to_owned());
        Ok(Box::new(FakeRollback {
            participant: self,
            profile: profile.as_str().to_owned(),
        }))
    }

    async fn complete_deletion(&self, profile: &ProfileId) -> Result<(), PortError> {
        self.completed
            .lock()
            .expect("completed lock")
            .push(profile.as_str().to_owned());
        if self.fail_completion {
            return Err(PortError::Failed("product cleanup refused".into()));
        }
        Ok(())
    }
}

#[async_trait]
impl ProfileDeletionRollback for FakeRollback<'_> {
    async fn rollback(self: Box<Self>) -> Result<(), PortError> {
        self.participant
            .rolled_back
            .lock()
            .expect("rolled back lock")
            .push(self.profile);
        Ok(())
    }
}

fn active_profile(engine: &str, profile: &str) -> ActiveProfileReferenceState {
    ActiveProfileReferenceState::Selected(ActiveProfileReference::new(
        EngineId::new(engine).unwrap(),
        ProfileId::new(profile).unwrap(),
        ActiveProfileReferenceRevision::new(3),
    ))
}

#[tokio::test]
async fn validated_write_preserves_the_exact_previous_document_as_backup() {
    let fixture = TestCore::new(ProfileDocumentExtension::Yaml);
    let core = fixture.core();
    let profile = ProfileId::new("main").unwrap();

    assert!(!core.profile_document_has_backup(&profile).await.unwrap());
    core.write_profile_document_locked(&profile, b"port: 7890\n")
        .await
        .unwrap();
    core.write_profile_document_locked(&profile, b"port: 7891\n")
        .await
        .unwrap();

    assert!(core.profile_document_has_backup(&profile).await.unwrap());
    assert_eq!(
        core.read_profile_document_backup(&profile).await.unwrap(),
        Some("port: 7890\n".to_owned())
    );
    assert!(
        core.restore_profile_document_backup_locked(&profile)
            .await
            .unwrap()
    );
    assert_eq!(
        core.read_profile_document(&profile).await.unwrap(),
        "port: 7890\n"
    );
    core.clear_profile_document_backup(&profile).await.unwrap();
    assert!(!core.profile_document_has_backup(&profile).await.unwrap());
}

#[tokio::test]
async fn deletion_removes_document_sidecar_and_backup_and_notifies_the_participant() {
    let fixture = TestCore::new(ProfileDocumentExtension::Yaml);
    let core = fixture.core();
    let profile = ProfileId::new("primary").unwrap();
    let document = core.profile_document_path(&profile).await.unwrap();
    let backup = fixture.layout().document_backup_path(&document);
    let sidecar = fixture
        .layout()
        .options_path(fixture.config_dir(), profile.as_str());
    let participant = FakeParticipant::default();

    core.write_profile_document_locked(&profile, b"mode: rule\n")
        .await
        .unwrap();
    fs::write(&backup, b"old document\n").await.unwrap();
    fs::create_dir_all(sidecar.parent().unwrap()).await.unwrap();
    fs::write(&sidecar, b"profile options\n").await.unwrap();

    core.delete_profile_locked(&profile, &participant)
        .await
        .unwrap();

    assert!(!document.exists());
    assert!(!backup.exists());
    assert!(!sidecar.exists());
    assert_eq!(participant.retired(), vec!["primary".to_owned()]);
    assert_eq!(participant.completed(), vec!["primary".to_owned()]);
    assert!(participant.rolled_back().is_empty());
}

#[tokio::test]
async fn refused_product_retirement_aborts_deletion_before_touching_the_document() {
    let fixture = TestCore::new(ProfileDocumentExtension::Yaml);
    let core = fixture.core();
    let profile = ProfileId::new("primary").unwrap();
    let document = core.profile_document_path(&profile).await.unwrap();
    let previous = b"mode: rule\n";
    let participant = FakeParticipant::refusing_retirement();

    core.write_profile_document_locked(&profile, previous)
        .await
        .unwrap();

    assert!(
        core.delete_profile_locked(&profile, &participant)
            .await
            .is_err()
    );
    assert_eq!(fs::read(document).await.unwrap(), previous);
    assert!(participant.retired().is_empty());
    assert!(participant.rolled_back().is_empty());
    assert!(participant.completed().is_empty());
}

#[tokio::test]
async fn failed_product_cleanup_still_removes_engine_metadata() {
    let fixture = TestCore::new(ProfileDocumentExtension::Yaml);
    let core = fixture.core();
    let profile = ProfileId::new("primary").unwrap();
    let participant = FakeParticipant::failing_completion();

    core.write_profile_document_locked(&profile, b"mode: rule\n")
        .await
        .unwrap();
    core.delete_profile_locked(&profile, &participant)
        .await
        .unwrap();

    assert!(!core.profile_document_path(&profile).await.unwrap().exists());
    assert_eq!(participant.completed(), vec!["primary".to_owned()]);
}

#[tokio::test]
async fn inactive_profile_operations_reject_the_selected_profile() {
    let fixture = TestCore::new(ProfileDocumentExtension::Yaml);
    let core = fixture.core();
    let profile = ProfileId::new("main").unwrap();
    let selected = active_profile("fixture-engine", "main");

    core.write_profile_document_locked(&profile, b"port: 7890\n")
        .await
        .unwrap();

    assert!(matches!(
        core.save_inactive_profile_under_selection_locked(&profile, b"port: 1\n", &selected)
            .await,
        Err(PortError::Rejected(_))
    ));
    assert!(matches!(
        core.restore_inactive_profile_backup_under_selection_locked(&profile, &selected)
            .await,
        Err(PortError::Rejected(_))
    ));
    assert!(matches!(
        core.delete_profile_under_selection_locked(
            &profile,
            &selected,
            &FakeParticipant::default()
        )
        .await,
        Err(PortError::Rejected(_))
    ));
    // The selection of another engine does not block an inactive operation.
    core.save_inactive_profile_under_selection_locked(
        &profile,
        b"port: 2\n",
        &active_profile("other-engine", "main"),
    )
    .await
    .unwrap();
}
