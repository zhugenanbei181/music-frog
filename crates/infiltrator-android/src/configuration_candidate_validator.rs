//! Stopped-Core native configuration validation through the Android owner host.

use async_trait::async_trait;
use infiltrator_contract::android_control::{
    AndroidHostProcessRole, CoreLaunchConfigurationIdentity,
};
use infiltrator_contract::capability::Capability;
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::profile_document_identity::{
    ProfileDocumentRevision, ProfileWorkspaceIdentity,
};
use infiltrator_platform::android_bridge::{
    AndroidBridge, AndroidCoreConfigurationValidationOutcome,
    AndroidCoreConfigurationValidationRequest,
};
use infiltrator_ports::error::PortError;
use infiltrator_ports::profile_configuration_transaction::{
    ProfileConfigurationCandidateValidatorPort, ProfileConfigurationTransactionScope,
};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::fs::OpenOptions;
use tokio::io::AsyncWriteExt;

const VALIDATION_TIMEOUT_MILLIS: u64 = 30_000;
static REQUEST_SEQUENCE: AtomicU64 = AtomicU64::new(1);

#[async_trait]
pub(crate) trait AndroidCoreConfigurationValidationClient: Send + Sync {
    fn process_role(&self) -> AndroidHostProcessRole;

    async fn validate_configuration(
        &self,
        request: &AndroidCoreConfigurationValidationRequest,
    ) -> Result<AndroidCoreConfigurationValidationOutcome, String>;
}

pub(crate) struct AndroidBridgeConfigurationValidationClient {
    bridge: Arc<dyn AndroidBridge>,
}

impl AndroidBridgeConfigurationValidationClient {
    pub(crate) fn new(bridge: Arc<dyn AndroidBridge>) -> Self {
        Self { bridge }
    }
}

#[async_trait]
impl AndroidCoreConfigurationValidationClient for AndroidBridgeConfigurationValidationClient {
    fn process_role(&self) -> AndroidHostProcessRole {
        self.bridge.process_role()
    }

    async fn validate_configuration(
        &self,
        request: &AndroidCoreConfigurationValidationRequest,
    ) -> Result<AndroidCoreConfigurationValidationOutcome, String> {
        self.bridge
            .core_validate_configuration(request)
            .await
            .map_err(|error| error.to_string())
    }
}

pub(crate) struct AndroidProfileConfigurationCandidateValidator {
    scope: ProfileConfigurationTransactionScope,
    config_directory: PathBuf,
    working_directory: PathBuf,
    client: Arc<dyn AndroidCoreConfigurationValidationClient>,
}

impl AndroidProfileConfigurationCandidateValidator {
    pub(crate) fn new(
        scope: ProfileConfigurationTransactionScope,
        config_directory: PathBuf,
        working_directory: PathBuf,
        client: Arc<dyn AndroidCoreConfigurationValidationClient>,
    ) -> Self {
        Self {
            scope,
            config_directory,
            working_directory,
            client,
        }
    }
}

#[async_trait]
impl ProfileConfigurationCandidateValidatorPort for AndroidProfileConfigurationCandidateValidator {
    fn scope(&self) -> ProfileConfigurationTransactionScope {
        self.scope.clone()
    }

    async fn validate_candidate(
        &self,
        expected: &ProfileWorkspaceIdentity,
        document_bytes: &[u8],
    ) -> Result<(), PortError> {
        if !self.scope.supports(expected) {
            return Err(PortError::unsupported(
                Capability::Profiles,
                "Android native validation request has a foreign engine, format, or schema",
            ));
        }
        if self.client.process_role() != AndroidHostProcessRole::CoreOwner {
            return Err(PortError::unsupported(
                Capability::Profiles,
                "stopped-Core configuration validation is available only in Android :vpn owner",
            ));
        }
        if !self.config_directory.is_absolute() || !self.working_directory.is_absolute() {
            return Err(PortError::Failed(
                "Android Core validation directories must be absolute host paths".to_owned(),
            ));
        }

        let request_id = next_request_id()?;
        let candidate_digest = sha256_digest(document_bytes);
        let candidate_revision = ProfileDocumentRevision::new(
            candidate_digest
                .strip_prefix("sha256:")
                .expect("SHA-256 digest has its prefix"),
        )
        .map_err(|error| PortError::Failed(error.to_string()))?;
        let identity = CoreLaunchConfigurationIdentity {
            engine_id: expected.document.engine_id.clone(),
            profile_id: expected.document.profile_id.clone(),
            document_revision: candidate_revision,
        };
        let (candidate, mut file) = reserve_candidate(
            &self.config_directory,
            identity.profile_id.as_str(),
            request_id,
        )
        .await?;
        file.write_all(document_bytes)
            .await
            .map_err(|error| PortError::Io(error.to_string()))?;
        file.flush()
            .await
            .map_err(|error| PortError::Io(error.to_string()))?;
        drop(file);

        let candidate_path = utf8_path(&candidate.0)?;
        let working_directory = tokio::fs::canonicalize(&self.working_directory)
            .await
            .map_err(|error| PortError::Io(error.to_string()))?;
        let working_directory = utf8_path(&working_directory)?;
        let request = AndroidCoreConfigurationValidationRequest {
            request_id,
            timeout_millis: VALIDATION_TIMEOUT_MILLIS,
            configuration_identity: identity,
            candidate_path,
            candidate_digest,
            working_directory,
        };
        let outcome = self
            .client
            .validate_configuration(&request)
            .await
            .map_err(PortError::Failed)?;
        map_outcome(outcome)
    }
}

struct CandidateFile(PathBuf);

impl Drop for CandidateFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

async fn reserve_candidate(
    config_directory: &Path,
    profile_id: &str,
    request_id: u64,
) -> Result<(CandidateFile, tokio::fs::File), PortError> {
    tokio::fs::create_dir_all(config_directory)
        .await
        .map_err(|error| PortError::Io(error.to_string()))?;
    let config_directory = tokio::fs::canonicalize(config_directory)
        .await
        .map_err(|error| PortError::Io(error.to_string()))?;
    let path = config_directory.join(format!(
        ".{profile_id}.yaml.tmp-{}-{request_id}",
        std::process::id(),
    ));
    match OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .await
    {
        Ok(file) => Ok((CandidateFile(path), file)),
        Err(error) => Err(PortError::Io(error.to_string())),
    }
}

fn next_request_id() -> Result<u64, PortError> {
    REQUEST_SEQUENCE
        .try_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
            current.checked_add(1)
        })
        .map_err(|_| PortError::Failed("Android Core validation request IDs are exhausted".into()))
}

fn sha256_digest(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut encoded = String::with_capacity(7 + digest.len() * 2);
    encoded.push_str("sha256:");
    for byte in digest {
        use std::fmt::Write as _;
        write!(encoded, "{byte:02x}").expect("writing to a string cannot fail");
    }
    encoded
}

fn utf8_path(path: &Path) -> Result<PathBuf, PortError> {
    path.to_str().map(PathBuf::from).ok_or_else(|| {
        PortError::Failed("Android Core validation path is not valid UTF-8".to_owned())
    })
}

fn map_outcome(outcome: AndroidCoreConfigurationValidationOutcome) -> Result<(), PortError> {
    match outcome {
        AndroidCoreConfigurationValidationOutcome::Validated => Ok(()),
        AndroidCoreConfigurationValidationOutcome::ConfigurationRejected { exit_code } => {
            Err(PortError::Rejected(Failure::new(
                ErrorCode::Configuration,
                format!("Mihomo rejected the candidate configuration (exit code {exit_code})"),
                false,
            )))
        }
        AndroidCoreConfigurationValidationOutcome::CoreActive => {
            Err(PortError::Rejected(Failure::new(
                ErrorCode::NotReady,
                "Mihomo must be stopped before native configuration validation".to_owned(),
                true,
            )))
        }
        AndroidCoreConfigurationValidationOutcome::NotVpnOwner => Err(PortError::unsupported(
            Capability::Profiles,
            "Android configuration validation was not executed by the :vpn owner",
        )),
        other => Err(PortError::Failed(format!(
            "Android native Mihomo configuration validation ended with {other:?}"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use infiltrator_contract::core_identity::EngineId;
    use infiltrator_contract::profile_document_identity::{
        ProfileDocumentIdentity, ProfileDocumentRevision, ProfileDocumentSourceIdentity,
        ProfileDocumentSourceKind, ProfileFormatId, ProfileId, ProfileSchemaVersion,
        ProfileSidecarRevision, ProfileWorkspaceIdentity,
    };
    use infiltrator_ports::profile_configuration_transaction::ProfileConfigurationCandidateValidatorPort;
    use infiltrator_ports::profile_document_codec::ProfileDocumentCodecPort;
    use std::sync::Mutex;

    struct FakeClient {
        role: AndroidHostProcessRole,
        outcome: AndroidCoreConfigurationValidationOutcome,
        request: Mutex<Option<AndroidCoreConfigurationValidationRequest>>,
    }

    #[async_trait]
    impl AndroidCoreConfigurationValidationClient for FakeClient {
        fn process_role(&self) -> AndroidHostProcessRole {
            self.role
        }

        async fn validate_configuration(
            &self,
            request: &AndroidCoreConfigurationValidationRequest,
        ) -> Result<AndroidCoreConfigurationValidationOutcome, String> {
            *self.request.lock().expect("request mutex") = Some(request.clone());
            let candidate =
                std::fs::read(&request.candidate_path).map_err(|error| error.to_string())?;
            if sha256_digest(&candidate) != request.candidate_digest {
                return Err("candidate digest did not match its bytes".to_owned());
            }
            Ok(self.outcome.clone())
        }
    }

    fn workspace_identity() -> ProfileWorkspaceIdentity {
        ProfileWorkspaceIdentity::new(
            ProfileDocumentIdentity::new(
                ProfileId::new("primary").expect("profile ID"),
                EngineId::new("mihomo").expect("engine ID"),
                ProfileDocumentSourceIdentity::new(
                    ProfileDocumentSourceKind::Local,
                    infiltrator_contract::profile_document_identity::ProfileSourceId::new(
                        "local-primary",
                    )
                    .expect("source ID"),
                ),
                ProfileDocumentRevision::new("old-revision").expect("old revision"),
                ProfileFormatId::new("mihomo-yaml").expect("format ID"),
                ProfileSchemaVersion::new(1).expect("schema version"),
            ),
            ProfileSidecarRevision::Absent,
        )
    }

    fn validator(
        directory: &Path,
        client: Arc<dyn AndroidCoreConfigurationValidationClient>,
    ) -> AndroidProfileConfigurationCandidateValidator {
        AndroidProfileConfigurationCandidateValidator::new(
            ProfileConfigurationTransactionScope::new(
                mihomo_config::profile_document_codec::MihomoProfileDocumentCodec.scope(),
            ),
            directory.join("files/engines/mihomo"),
            directory.join("files"),
            client,
        )
    }

    #[tokio::test]
    async fn stages_candidate_with_content_revision_and_removes_it_after_owner_receipt() {
        let directory = tempfile::tempdir().expect("temporary Android files root");
        let bytes = b"mixed-port: 7890\nmode: rule\n";
        let client = Arc::new(FakeClient {
            role: AndroidHostProcessRole::CoreOwner,
            outcome: AndroidCoreConfigurationValidationOutcome::Validated,
            request: Mutex::new(None),
        });
        let client_port: Arc<dyn AndroidCoreConfigurationValidationClient> = client.clone();
        let validator = validator(directory.path(), client_port);

        validator
            .validate_candidate(&workspace_identity(), bytes)
            .await
            .expect("native owner validation");

        let request = client
            .request
            .lock()
            .expect("request mutex")
            .clone()
            .expect("request");
        assert_eq!(request.timeout_millis, VALIDATION_TIMEOUT_MILLIS);
        assert_eq!(
            request.configuration_identity.profile_id.as_str(),
            "primary"
        );
        assert_eq!(
            request.configuration_identity.document_revision.as_str(),
            request.candidate_digest.trim_start_matches("sha256:"),
        );
        assert_eq!(
            std::fs::read(&request.candidate_path).unwrap_err().kind(),
            std::io::ErrorKind::NotFound
        );
        assert_eq!(request.working_directory, directory.path().join("files"));
        assert!(
            request
                .candidate_path
                .to_string_lossy()
                .contains(".primary.yaml.tmp-")
        );
    }

    #[tokio::test]
    async fn non_owner_cannot_create_or_validate_a_candidate() {
        let directory = tempfile::tempdir().expect("temporary Android files root");
        let client: Arc<dyn AndroidCoreConfigurationValidationClient> = Arc::new(FakeClient {
            role: AndroidHostProcessRole::UiClient,
            outcome: AndroidCoreConfigurationValidationOutcome::Validated,
            request: Mutex::new(None),
        });
        let validator = validator(directory.path(), client);

        let error = validator
            .validate_candidate(&workspace_identity(), b"candidate")
            .await
            .expect_err("UI process must fail closed");

        assert!(matches!(error, PortError::Unsupported { .. }));
        assert!(!directory.path().join("files/engines/mihomo").exists());
    }

    #[tokio::test]
    async fn native_rejection_is_a_typed_configuration_failure() {
        let directory = tempfile::tempdir().expect("temporary Android files root");
        let client = Arc::new(FakeClient {
            role: AndroidHostProcessRole::CoreOwner,
            outcome: AndroidCoreConfigurationValidationOutcome::ConfigurationRejected {
                exit_code: 1,
            },
            request: Mutex::new(None),
        });
        let client_port: Arc<dyn AndroidCoreConfigurationValidationClient> = client.clone();
        let validator = validator(directory.path(), client_port);

        let error = validator
            .validate_candidate(&workspace_identity(), b"invalid")
            .await
            .expect_err("Mihomo rejection must fail the transaction");

        assert!(matches!(
            error,
            PortError::Rejected(Failure {
                code: ErrorCode::Configuration,
                ..
            })
        ));
        let request = client
            .request
            .lock()
            .expect("request mutex")
            .clone()
            .expect("native validation request");
        assert_eq!(
            std::fs::read(&request.candidate_path).unwrap_err().kind(),
            std::io::ErrorKind::NotFound
        );
    }

    struct UnavailableClient {
        request: Mutex<Option<AndroidCoreConfigurationValidationRequest>>,
    }

    #[async_trait]
    impl AndroidCoreConfigurationValidationClient for UnavailableClient {
        fn process_role(&self) -> AndroidHostProcessRole {
            AndroidHostProcessRole::CoreOwner
        }

        async fn validate_configuration(
            &self,
            request: &AndroidCoreConfigurationValidationRequest,
        ) -> Result<AndroidCoreConfigurationValidationOutcome, String> {
            *self.request.lock().expect("request mutex") = Some(request.clone());
            Err("owner validation IPC is unavailable".to_owned())
        }
    }

    #[tokio::test]
    async fn owner_ipc_failure_is_typed_and_removes_the_staged_candidate() {
        let directory = tempfile::tempdir().expect("temporary Android files root");
        let client = Arc::new(UnavailableClient {
            request: Mutex::new(None),
        });
        let client_port: Arc<dyn AndroidCoreConfigurationValidationClient> = client.clone();
        let validator = validator(directory.path(), client_port);

        let error = validator
            .validate_candidate(&workspace_identity(), b"candidate")
            .await
            .expect_err("missing owner IPC must fail closed");

        assert!(
            matches!(error, PortError::Failed(message) if message.contains("IPC is unavailable"))
        );
        let request = client
            .request
            .lock()
            .expect("request mutex")
            .clone()
            .expect("staged request");
        assert_eq!(
            std::fs::read(&request.candidate_path).unwrap_err().kind(),
            std::io::ErrorKind::NotFound
        );
    }
}
