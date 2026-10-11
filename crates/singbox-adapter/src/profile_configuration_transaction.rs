//! Scope-correct Core transaction declaration for the uncomposed sing-box owner.

use infiltrator_contract::profile_document_identity::{ProfileFormatId, ProfileSchemaVersion};
use infiltrator_ports::error::PortError;
use infiltrator_ports::profile_configuration_transaction::{
    ProfileConfigurationTransactionOutcome, ProfileConfigurationTransactionPort,
    ProfileConfigurationTransactionRequest, ProfileConfigurationTransactionScope,
    UnavailableProfileConfigurationTransaction,
};
use infiltrator_ports::profile_document_codec::ProfileDocumentCodecScope;

/// The standalone sing-box file adapter cannot yet provide this Core
/// transaction: it has no engine-owned profile-store CAS binding or lifecycle
/// session for restart/readiness readback. Keep the exact JSON scope visible
/// and reject the operation explicitly until a sing-box product owner exists.
pub struct SingBoxProfileConfigurationTransactionAdapter {
    unavailable: UnavailableProfileConfigurationTransaction,
}

impl SingBoxProfileConfigurationTransactionAdapter {
    pub fn unavailable() -> Self {
        let scope = ProfileConfigurationTransactionScope::new(ProfileDocumentCodecScope {
            engine_id: crate::engine_id(),
            format_id: ProfileFormatId::new(crate::profile_document_codec::PROFILE_FORMAT)
                .expect("static sing-box format identifier is valid"),
            schema_version: ProfileSchemaVersion::new(
                crate::profile_document_codec::PROFILE_SCHEMA_VERSION,
            )
            .expect("sing-box schema version is positive"),
        });
        Self {
            unavailable: UnavailableProfileConfigurationTransaction::new(
                scope,
                "sing-box workspace transaction is unavailable until an engine-owned CAS store and live lifecycle owner are composed",
            ),
        }
    }
}

#[async_trait::async_trait]
impl ProfileConfigurationTransactionPort for SingBoxProfileConfigurationTransactionAdapter {
    fn scope(&self) -> ProfileConfigurationTransactionScope {
        ProfileConfigurationTransactionPort::scope(&self.unavailable)
    }

    async fn transact(
        &self,
        request: ProfileConfigurationTransactionRequest,
    ) -> Result<ProfileConfigurationTransactionOutcome, PortError> {
        self.unavailable.transact(request).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use infiltrator_contract::profile_document_identity::{
        ProfileDocumentIdentity, ProfileDocumentRevision, ProfileDocumentSourceIdentity,
        ProfileDocumentSourceKind, ProfileId, ProfileSidecarRevision, ProfileSourceId,
        ProfileWorkspaceIdentity,
    };
    use infiltrator_domain::apply::ApplyStrategy;
    use infiltrator_ports::profile_workspace::{ProfileWorkspacePurpose, ProfileWorkspaceUpdate};

    fn request(
        scope: &ProfileConfigurationTransactionScope,
    ) -> ProfileConfigurationTransactionRequest {
        ProfileConfigurationTransactionRequest {
            expected: ProfileWorkspaceIdentity::new(
                ProfileDocumentIdentity::new(
                    ProfileId::new("main").unwrap(),
                    scope.document.engine_id.clone(),
                    ProfileDocumentSourceIdentity::new(
                        ProfileDocumentSourceKind::Local,
                        ProfileSourceId::new("main-local").unwrap(),
                    ),
                    ProfileDocumentRevision::new("revision-1").unwrap(),
                    scope.document.format_id.clone(),
                    scope.document.schema_version,
                ),
                ProfileSidecarRevision::Absent,
            ),
            update: ProfileWorkspaceUpdate {
                purpose: ProfileWorkspacePurpose::Derived,
                document_bytes: br#"{"inbounds":[]}"#.to_vec(),
                sidecar_bytes: None,
            },
            strategy: ApplyStrategy::PreferReload,
        }
    }

    #[tokio::test]
    async fn workspace_transaction_is_explicitly_unsupported_in_this_scope() {
        let adapter = SingBoxProfileConfigurationTransactionAdapter::unavailable();
        let scope = adapter.scope();

        assert_eq!(scope.document.engine_id, crate::engine_id());
        assert_eq!(
            scope.document.format_id.as_str(),
            crate::profile_document_codec::PROFILE_FORMAT
        );
        assert!(matches!(
            adapter.transact(request(&scope)).await,
            Err(PortError::Unsupported { .. })
        ));
    }
}
