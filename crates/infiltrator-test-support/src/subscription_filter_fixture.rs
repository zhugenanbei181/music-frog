//! Explicit demo/test observations derived from their supplied documents.
use infiltrator_contract::profile_document_identity::{
    ProfileDocumentIdentity, ProfileDocumentRevision, ProfileDocumentSourceIdentity,
    ProfileDocumentSourceKind, ProfileId, ProfileSidecarRevision, ProfileSourceId,
    ProfileWorkspaceIdentity,
};
use infiltrator_contract::subscription_filter_form::FilterObservation;
use infiltrator_contract::subscription_import::SubscriptionFilterDraft;
use infiltrator_domain::filter_policy_form::filter_spec_from_draft;
use infiltrator_domain::profile_options::ProfileOptions;
use infiltrator_domain::profile_source::{hash_bytes, hash_document_bytes};
use lurker_core_contract::error::{ErrorCode, Failure};
use lurker_core_contract::identity::EngineId;
use lurker_core_contract::profile_document_identity::{ProfileFormatId, ProfileSchemaVersion};

pub const FIXTURE_DOCUMENT: &str = "mode: rule\n";

pub fn observation(
    profile: &str,
    content: &str,
    filter: SubscriptionFilterDraft,
) -> Result<FilterObservation, Failure> {
    let options = (!filter.is_empty())
        .then(|| {
            filter_spec_from_draft(&filter).and_then(|spec| {
                serde_yaml_ng::to_string(&ProfileOptions {
                    filter: Some(spec),
                    ..Default::default()
                })
                .map_err(Into::into)
            })
        })
        .transpose()
        .map_err(|error| Failure::new(ErrorCode::InvalidInput, error.to_string(), false))?;
    let profile_id = ProfileId::new(profile)
        .map_err(|error| Failure::new(ErrorCode::InvalidInput, error.to_string(), false))?;
    let engine_id = EngineId::new("mihomo")
        .map_err(|error| Failure::new(ErrorCode::InvalidInput, error.to_string(), false))?;
    let source_id = ProfileSourceId::new(format!("local-{}", hash_document_bytes(profile)))
        .map_err(|error| Failure::new(ErrorCode::InvalidInput, error.to_string(), false))?;
    let document = ProfileDocumentIdentity::new(
        profile_id,
        engine_id,
        ProfileDocumentSourceIdentity::new(ProfileDocumentSourceKind::Local, source_id),
        ProfileDocumentRevision::new(hash_document_bytes(content))
            .map_err(|error| Failure::new(ErrorCode::InvalidInput, error.to_string(), false))?,
        ProfileFormatId::new("mihomo-yaml")
            .map_err(|error| Failure::new(ErrorCode::InvalidInput, error.to_string(), false))?,
        ProfileSchemaVersion::new(1)
            .map_err(|error| Failure::new(ErrorCode::InvalidInput, error.to_string(), false))?,
    );
    let sidecar_revision = options
        .as_deref()
        .map(str::as_bytes)
        .map(hash_bytes)
        .map(|revision| {
            ProfileDocumentRevision::new(revision)
                .map(ProfileSidecarRevision::Present)
                .map_err(|error| Failure::new(ErrorCode::InvalidInput, error.to_string(), false))
        })
        .transpose()?
        .unwrap_or(ProfileSidecarRevision::Absent);
    Ok(FilterObservation {
        workspace_identity: ProfileWorkspaceIdentity::new(document, sidecar_revision),
        filter,
    })
}
