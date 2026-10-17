use crate::document_store::write_document_bytes_durable;
use crate::store::ProfileDocumentExtension;
use crate::test_core::TestCore;
use crate::workspace::{ProfileSourceFacts, ProfileWorkspaceRecord};
use crate::workspace_store::{ProfileWorkspaceEdit, ProfileWorkspacePublicationTarget};
use async_trait::async_trait;
use lurker_core_contract::active_profile_reference::{
    ActiveProfileReference, ActiveProfileReferenceRevision, ActiveProfileReferenceState,
};
use lurker_core_contract::error::ErrorCode;
use lurker_core_contract::profile_document_identity::{ProfileDocumentSourceKind, ProfileId};
use lurker_core_contract::profile_protection::{ProfileWriteAuthorization, ProfileWriteProtection};
use lurker_core_ports::error::PortError;
use tokio::fs;

struct EditableFacts;

#[async_trait]
impl ProfileSourceFacts for EditableFacts {
    async fn write_protection(
        &self,
        _profile: &ProfileId,
    ) -> Result<ProfileWriteProtection, PortError> {
        Ok(ProfileWriteProtection::Editable)
    }

    async fn subscription_url(&self, _profile: &ProfileId) -> Result<Option<String>, PortError> {
        Ok(None)
    }
}

struct SubscribedFacts;

#[async_trait]
impl ProfileSourceFacts for SubscribedFacts {
    async fn write_protection(
        &self,
        _profile: &ProfileId,
    ) -> Result<ProfileWriteProtection, PortError> {
        Ok(ProfileWriteProtection::RemoteSubscription)
    }

    async fn subscription_url(&self, _profile: &ProfileId) -> Result<Option<String>, PortError> {
        Ok(Some("https://private.example/subscription".to_owned()))
    }
}

async fn seeded(fixture: &TestCore, profile: &ProfileId) -> ProfileWorkspaceRecord {
    fixture
        .core()
        .write_profile_document_locked(profile, br#"{"revision":1}"#)
        .await
        .unwrap();
    let sidecar = fixture
        .layout()
        .options_path(fixture.config_dir(), profile.as_str());
    write_document_bytes_durable(&sidecar, b"sidecar-v1")
        .await
        .unwrap();
    fixture
        .core()
        .read_profile_workspace(profile, &EditableFacts)
        .await
        .unwrap()
}

#[tokio::test]
async fn workspace_reads_expose_exact_document_sidecar_and_source_identity() {
    let fixture = TestCore::new(ProfileDocumentExtension::Yaml);
    let profile = ProfileId::new("main").unwrap();
    let workspace = seeded(&fixture, &profile).await;

    assert_eq!(workspace.document.raw_bytes, br#"{"revision":1}"#);
    assert_eq!(
        workspace.sidecar_bytes.as_deref(),
        Some(b"sidecar-v1".as_slice())
    );
    assert_eq!(workspace.write_protection, ProfileWriteProtection::Editable);
    assert_eq!(
        workspace.document.identity.source.kind,
        ProfileDocumentSourceKind::Local
    );
}

#[tokio::test]
async fn publication_cas_writes_exact_document_and_sidecar_bytes() {
    let fixture = TestCore::new(ProfileDocumentExtension::Yaml);
    let core = fixture.core();
    let profile = ProfileId::new("main").unwrap();
    let previous = seeded(&fixture, &profile).await;
    let expected = previous.identity();

    let guard = core.lock_profile_writes().await.unwrap();
    let published = core
        .publish_profile_workspace_locked(
            &EditableFacts,
            &expected,
            ProfileWorkspaceEdit::Derived,
            br#"{"revision":2}"#,
            Some(b"sidecar-v2"),
            ProfileWorkspacePublicationTarget::AuthorizedActive,
        )
        .await
        .unwrap();
    drop(guard);

    assert_eq!(published.document.raw_bytes, br#"{"revision":2}"#);
    assert_eq!(
        published.sidecar_bytes.as_deref(),
        Some(b"sidecar-v2".as_slice())
    );
    assert_eq!(
        core.read_profile_workspace(&profile, &EditableFacts)
            .await
            .unwrap(),
        published
    );
}

#[tokio::test]
async fn publication_rejects_stale_identity_without_writes() {
    let fixture = TestCore::new(ProfileDocumentExtension::Yaml);
    let core = fixture.core();
    let profile = ProfileId::new("main").unwrap();
    let stale = seeded(&fixture, &profile).await;
    let stale_identity = stale.identity();

    let guard = core.lock_profile_writes().await.unwrap();
    core.publish_profile_workspace_locked(
        &EditableFacts,
        &stale_identity,
        ProfileWorkspaceEdit::Derived,
        br#"{"revision":2}"#,
        Some(b"sidecar-v2"),
        ProfileWorkspacePublicationTarget::AuthorizedActive,
    )
    .await
    .unwrap();
    let error = core
        .publish_profile_workspace_locked(
            &EditableFacts,
            &stale_identity,
            ProfileWorkspaceEdit::Derived,
            br#"{"revision":3}"#,
            Some(b"sidecar-v3"),
            ProfileWorkspacePublicationTarget::AuthorizedActive,
        )
        .await
        .unwrap_err();
    drop(guard);

    assert!(matches!(error, PortError::Rejected(failure) if failure.code == ErrorCode::NotReady));
    let observed = core
        .read_profile_workspace(&profile, &EditableFacts)
        .await
        .unwrap();
    assert_eq!(observed.document.raw_bytes, br#"{"revision":2}"#);
    assert_eq!(
        observed.sidecar_bytes.as_deref(),
        Some(b"sidecar-v2".as_slice())
    );
}

#[tokio::test]
async fn direct_edit_of_a_protected_profile_requires_an_explicit_unlock() {
    let fixture = TestCore::new(ProfileDocumentExtension::Yaml);
    let core = fixture.core();
    let profile = ProfileId::new("main").unwrap();
    seeded(&fixture, &profile).await;
    let protected = core
        .read_profile_workspace(&profile, &SubscribedFacts)
        .await
        .unwrap();
    assert_eq!(
        protected.write_protection,
        ProfileWriteProtection::RemoteSubscription
    );
    let expected = protected.identity();

    let guard = core.lock_profile_writes().await.unwrap();
    let error = core
        .publish_profile_workspace_locked(
            &SubscribedFacts,
            &expected,
            ProfileWorkspaceEdit::DirectEdit {
                authorization: ProfileWriteAuthorization::RequireUnlock,
            },
            br#"{"revision":2}"#,
            None,
            ProfileWorkspacePublicationTarget::AuthorizedActive,
        )
        .await
        .unwrap_err();
    drop(guard);

    assert!(
        matches!(error, PortError::Rejected(failure) if failure.code == ErrorCode::Configuration)
    );
    assert_eq!(
        fs::read(core.profile_document_path(&profile).await.unwrap())
            .await
            .unwrap(),
        br#"{"revision":1}"#
    );
}

#[tokio::test]
async fn inactive_publication_target_rejects_the_selected_profile() {
    let fixture = TestCore::new(ProfileDocumentExtension::Yaml);
    let core = fixture.core();
    let profile = ProfileId::new("main").unwrap();
    let previous = seeded(&fixture, &profile).await;
    let expected = previous.identity();
    let selection = ActiveProfileReferenceState::Selected(ActiveProfileReference::new(
        core.engine_id(),
        profile.clone(),
        ActiveProfileReferenceRevision::new(1),
    ));

    let guard = core.lock_profile_writes().await.unwrap();
    let error = core
        .publish_profile_workspace_locked(
            &EditableFacts,
            &expected,
            ProfileWorkspaceEdit::Derived,
            br#"{"revision":2}"#,
            None,
            ProfileWorkspacePublicationTarget::InactiveSelection(&selection),
        )
        .await
        .unwrap_err();
    drop(guard);

    assert!(matches!(error, PortError::Rejected(failure) if failure.code == ErrorCode::NotReady));
}

#[tokio::test]
async fn restore_workspace_puts_the_exact_previous_bytes_back() {
    let fixture = TestCore::new(ProfileDocumentExtension::Yaml);
    let core = fixture.core();
    let profile = ProfileId::new("main").unwrap();
    let previous = seeded(&fixture, &profile).await;

    let guard = core.lock_profile_writes().await.unwrap();
    let published = core
        .publish_profile_workspace_locked(
            &EditableFacts,
            &previous.identity(),
            ProfileWorkspaceEdit::Derived,
            br#"{"revision":2}"#,
            Some(b"sidecar-v2"),
            ProfileWorkspacePublicationTarget::AuthorizedActive,
        )
        .await
        .unwrap();
    let restored = core
        .restore_profile_workspace_locked(
            &EditableFacts,
            &published.identity(),
            &previous,
            ProfileWorkspacePublicationTarget::AuthorizedActive,
        )
        .await
        .unwrap();
    drop(guard);

    assert_eq!(restored, previous);
    assert_eq!(
        core.read_profile_workspace(&profile, &EditableFacts)
            .await
            .unwrap(),
        previous
    );
}
