//! Memory-port fixtures retain typed document and sidecar identity semantics.
use infiltrator_domain::profile_options::ProfileOptions;
use infiltrator_domain::profile_source::hash_bytes;
use infiltrator_ports::profile_workspace::ProfileWorkspace;
use lurker_core_contract::identity::EngineId;
use lurker_core_contract::profile_document_identity::{
    ProfileDocument, ProfileDocumentIdentity, ProfileDocumentRevision,
    ProfileDocumentSourceIdentity, ProfileDocumentSourceKind, ProfileId, ProfileSidecarRevision,
    ProfileSourceId,
};
use lurker_core_contract::profile_document_identity::{ProfileFormatId, ProfileSchemaVersion};
use lurker_core_contract::profile_protection::ProfileWriteProtection;
use lurker_core_ports::error::PortError;

pub fn workspace_with_origin(
    profile: &str,
    document_bytes: &[u8],
    options: Option<&ProfileOptions>,
    subscription_url: Option<&str>,
) -> Result<ProfileWorkspace, PortError> {
    let sidecar_bytes = options
        .map(serde_yaml_ng::to_string)
        .transpose()
        .map_err(|error| PortError::Io(error.to_string()))?
        .map(String::into_bytes);
    workspace_with_raw_sidecar(profile, document_bytes, sidecar_bytes, subscription_url)
}

pub fn workspace_with_raw_sidecar(
    profile: &str,
    document_bytes: &[u8],
    sidecar_bytes: Option<Vec<u8>>,
    subscription_url: Option<&str>,
) -> Result<ProfileWorkspace, PortError> {
    let (kind, source_key) = match subscription_url.filter(|url| !url.trim().is_empty()) {
        Some(url) => (ProfileDocumentSourceKind::Subscription, url.as_bytes()),
        None => (ProfileDocumentSourceKind::Local, profile.as_bytes()),
    };
    let source_hash = hash_bytes(source_key);
    let source_prefix = match kind {
        ProfileDocumentSourceKind::Local => "local-",
        ProfileDocumentSourceKind::Subscription => "subscription-",
        ProfileDocumentSourceKind::Imported => "imported-",
        ProfileDocumentSourceKind::Synchronized => "sync-",
    };
    let source_id = ProfileSourceId::new(format!("{source_prefix}{source_hash}"))
        .map_err(|error| PortError::Io(error.to_string()))?;
    let document_revision = ProfileDocumentRevision::new(hash_bytes(document_bytes))
        .map_err(|error| PortError::Io(error.to_string()))?;
    let engine_id = EngineId::new("mihomo").map_err(|error| PortError::Io(error.to_string()))?;
    let profile_id = ProfileId::new(profile).map_err(|error| PortError::Io(error.to_string()))?;
    let identity = ProfileDocumentIdentity::new(
        profile_id,
        engine_id,
        ProfileDocumentSourceIdentity::new(kind, source_id),
        document_revision,
        ProfileFormatId::new("mihomo-yaml").expect("valid Mihomo format ID"),
        ProfileSchemaVersion::new(1).expect("positive Mihomo schema version"),
    );
    let sidecar_revision = sidecar_bytes
        .as_deref()
        .map(hash_bytes)
        .map(|revision| {
            ProfileDocumentRevision::new(revision)
                .map(ProfileSidecarRevision::Present)
                .map_err(|error| PortError::Io(error.to_string()))
        })
        .transpose()?
        .unwrap_or(ProfileSidecarRevision::Absent);
    Ok(ProfileWorkspace {
        document: ProfileDocument::new(identity, document_bytes.to_vec()),
        sidecar_revision,
        write_protection: ProfileWriteProtection::Editable,
        sidecar_bytes,
    })
}
