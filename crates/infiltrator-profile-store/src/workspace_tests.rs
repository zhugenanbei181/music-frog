use super::{
    document_revision, publish_document_pair, source_identity, validate_workspace_bytes,
    validate_workspace_identity, workspace_from_bytes,
};
use lurker_core_contract::identity::EngineId;
use lurker_core_contract::profile_document_identity::{
    ProfileDocumentSourceIdentity, ProfileDocumentSourceKind, ProfileId, ProfileSidecarRevision,
    ProfileSourceId,
};
use lurker_core_contract::profile_document_identity::{ProfileFormatId, ProfileSchemaVersion};
use lurker_core_contract::profile_protection::ProfileWriteProtection;
use lurker_core_ports::error::PortError;
use lurker_core_ports::profile_document_codec::ProfileDocumentCodecScope;
use std::fs::{create_dir, read, write};
use tempfile::TempDir;

fn scope(engine: &str) -> ProfileDocumentCodecScope {
    ProfileDocumentCodecScope {
        engine_id: EngineId::new(engine).unwrap(),
        format_id: ProfileFormatId::new("native").unwrap(),
        schema_version: ProfileSchemaVersion::new(1).unwrap(),
    }
}

fn source() -> ProfileDocumentSourceIdentity {
    ProfileDocumentSourceIdentity::new(
        ProfileDocumentSourceKind::Local,
        ProfileSourceId::new("source-test").unwrap(),
    )
}

#[test]
fn workspace_revision_tracks_raw_bytes_and_distinguishes_absent_sidecar() {
    let bytes = b"native document".to_vec();
    let workspace = workspace_from_bytes(
        &scope("mihomo"),
        ProfileId::new("main").unwrap(),
        source(),
        ProfileWriteProtection::Editable,
        bytes.clone(),
        None,
    );

    assert_eq!(
        workspace.document.identity.revision,
        document_revision(&bytes)
    );
    assert_eq!(workspace.sidecar_revision, ProfileSidecarRevision::Absent);
    validate_workspace_bytes(&workspace).unwrap();

    let mut forged = workspace;
    forged.document.raw_bytes.push(b'!');
    assert!(matches!(
        validate_workspace_bytes(&forged),
        Err(PortError::Rejected(_))
    ));
}

#[test]
fn codec_scope_rejects_a_foreign_engine_before_store_access() {
    let workspace = workspace_from_bytes(
        &scope("sing-box"),
        ProfileId::new("main").unwrap(),
        source(),
        ProfileWriteProtection::Editable,
        b"{}".to_vec(),
        Some(Vec::new()),
    );

    assert!(matches!(
        validate_workspace_identity(&scope("mihomo"), &workspace.identity()),
        Err(PortError::Unsupported { .. })
    ));
}

#[test]
fn source_identity_hashes_subscription_urls_without_retaining_the_secret() {
    let profile = ProfileId::new("subscribed").unwrap();
    let url = "https://private.example/subscription?token=secret-value";
    let identity = source_identity(
        &profile,
        ProfileWriteProtection::RemoteSubscription,
        Some(url),
    )
    .unwrap();

    assert_eq!(identity.kind, ProfileDocumentSourceKind::Subscription);
    assert!(!identity.source_id.as_str().contains("secret-value"));
    assert_ne!(identity.source_id.as_str(), url);
}

#[tokio::test]
async fn failed_document_publication_restores_exact_previous_sidecar_bytes() {
    let directory = TempDir::new().unwrap();
    let sidecar = directory.path().join("options/main.json");
    create_dir(sidecar.parent().unwrap()).unwrap();
    write(&sidecar, b"previous exact bytes").unwrap();

    let result = publish_document_pair(
        &sidecar,
        Some(b"previous exact bytes"),
        Some(b"candidate bytes"),
        async { Err(PortError::Io("document write failed".to_owned())) },
    )
    .await;

    assert!(matches!(result, Err(PortError::Io(_))));
    assert_eq!(read(sidecar).unwrap(), b"previous exact bytes");
}
