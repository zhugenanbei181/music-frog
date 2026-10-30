//! Application service for encrypted backup export, verification, local/WebDAV delivery, and atomic restore (FLPARITY-013).

use infiltrator_contract::backup_restore::{
    BackupBundleHeader, BackupDeliveryTarget, BackupOperationReceipt, BackupRestoreScope,
    BackupRestoreSnapshot, BackupVerificationFailure,
};
use infiltrator_domain::backup::{
    BackupBundle, BackupError, export_encrypted_bundle, import_encrypted_bundle,
};
use infiltrator_ports::backup_restore::BackupRestorePort;
use std::fmt::{self, Debug, Formatter};
use std::sync::{Arc, Mutex};

/// Minimum required passphrase length for AES-256-GCM backup archives.
pub const MIN_BACKUP_PASSPHRASE_LEN: usize = 6;
/// Currently supported major bundle schema version.
pub const SUPPORTED_BUNDLE_MAJOR_VERSION: u32 = 1;

#[derive(Debug)]
struct InnerState {
    revision: u64,
    selected_target: BackupDeliveryTarget,
    selected_scope: BackupRestoreScope,
    last_receipt: BackupOperationReceipt,
    available_bundles: Vec<BackupBundleHeader>,
}

impl Default for InnerState {
    fn default() -> Self {
        Self {
            revision: 1,
            selected_target: BackupDeliveryTarget::LocalFile,
            selected_scope: BackupRestoreScope::FullData,
            last_receipt: BackupOperationReceipt::Idle,
            available_bundles: Vec::new(),
        }
    }
}

/// Parse the major version number from a semantic version string (`"1.0.0"` -> `Some(1)`).
pub fn parse_schema_major(version: &str) -> Option<u32> {
    version.trim().split('.').next()?.parse::<u32>().ok()
}

/// Validate a decoded `BackupBundle` for schema version compatibility and SHA-256 payload integrity.
pub fn verify_decoded_bundle(bundle: &BackupBundle) -> Result<(), BackupVerificationFailure> {
    let major = parse_schema_major(&bundle.manifest.version).ok_or_else(|| {
        BackupVerificationFailure::CorruptedPayload {
            detail: format!(
                "invalid manifest version string: {}",
                bundle.manifest.version
            ),
        }
    })?;
    if major != SUPPORTED_BUNDLE_MAJOR_VERSION {
        return Err(BackupVerificationFailure::IncompatibleSchemaVersion {
            bundle_version: bundle.manifest.version.clone(),
            supported_major: SUPPORTED_BUNDLE_MAJOR_VERSION,
        });
    }
    let actual = bundle.compute_digest();
    if bundle.manifest.sha256_digest != actual {
        return Err(BackupVerificationFailure::IntegrityDigestMismatch {
            expected_sha256: bundle.manifest.sha256_digest.clone(),
            actual_sha256: actual,
        });
    }
    Ok(())
}

/// Map a domain `BackupError` into the contract `BackupVerificationFailure`.
pub fn map_domain_backup_error(err: BackupError) -> BackupVerificationFailure {
    match err {
        BackupError::EmptyPassword => BackupVerificationFailure::PassphraseTooShort {
            min_len: MIN_BACKUP_PASSPHRASE_LEN,
        },
        BackupError::DecryptionFailed => BackupVerificationFailure::WrongPassphraseOrAuthTag,
        BackupError::IntegrityMismatch { expected, actual } => {
            BackupVerificationFailure::IntegrityDigestMismatch {
                expected_sha256: expected,
                actual_sha256: actual,
            }
        }
        BackupError::InvalidFormat(detail) | BackupError::EncryptionFailed(detail) => {
            BackupVerificationFailure::CorruptedPayload { detail }
        }
        BackupError::Json(err) => BackupVerificationFailure::CorruptedPayload {
            detail: err.to_string(),
        },
        BackupError::Zip(err) => BackupVerificationFailure::CorruptedPayload {
            detail: err.to_string(),
        },
        BackupError::Io(err) => BackupVerificationFailure::CorruptedPayload {
            detail: err.to_string(),
        },
    }
}

/// Synchronously encrypt a verified `BackupBundle` and produce the export receipt.
pub fn encrypt_verified_bundle(
    bundle: &BackupBundle,
    passphrase: &str,
    destination: impl Into<String>,
    target: BackupDeliveryTarget,
    now_unix_ms: u64,
) -> Result<(Vec<u8>, BackupOperationReceipt), BackupOperationReceipt> {
    let trimmed = passphrase.trim();
    if trimmed.chars().count() < MIN_BACKUP_PASSPHRASE_LEN {
        return Err(BackupOperationReceipt::Failed {
            failure: BackupVerificationFailure::PassphraseTooShort {
                min_len: MIN_BACKUP_PASSPHRASE_LEN,
            },
            attempted_at_unix_ms: now_unix_ms,
        });
    }
    if let Err(failure) = verify_decoded_bundle(bundle) {
        return Err(BackupOperationReceipt::Failed {
            failure,
            attempted_at_unix_ms: now_unix_ms,
        });
    }
    match export_encrypted_bundle(bundle, trimmed) {
        Ok(bytes) => {
            let receipt = BackupOperationReceipt::ExportSucceeded {
                target,
                destination: destination.into(),
                bundle_version: bundle.manifest.version.clone(),
                sha256_digest: bundle.manifest.sha256_digest.clone(),
                profile_count: bundle.profiles.len(),
                exported_at_unix_ms: now_unix_ms,
            };
            Ok((bytes, receipt))
        }
        Err(err) => Err(BackupOperationReceipt::Failed {
            failure: map_domain_backup_error(err),
            attempted_at_unix_ms: now_unix_ms,
        }),
    }
}

/// Synchronously decrypt and verify an encrypted backup archive.
pub fn decrypt_and_verify_bundle(
    encrypted_bytes: &[u8],
    passphrase: &str,
    now_unix_ms: u64,
) -> Result<BackupBundle, BackupOperationReceipt> {
    let trimmed = passphrase.trim();
    if trimmed.chars().count() < MIN_BACKUP_PASSPHRASE_LEN {
        return Err(BackupOperationReceipt::Failed {
            failure: BackupVerificationFailure::PassphraseTooShort {
                min_len: MIN_BACKUP_PASSPHRASE_LEN,
            },
            attempted_at_unix_ms: now_unix_ms,
        });
    }
    // Check magic framing and version header before AEAD decryption so corrupted or incompatible
    // headers are classified deterministically.
    if encrypted_bytes.len() < 48 || &encrypted_bytes[0..16] != b"IFTR_BACKUP_V1\x00\x00" {
        return Err(BackupOperationReceipt::Failed {
            failure: BackupVerificationFailure::CorruptedPayload {
                detail: "missing or invalid IFTR_BACKUP_V1 framing header".to_string(),
            },
            attempted_at_unix_ms: now_unix_ms,
        });
    }
    match import_encrypted_bundle(encrypted_bytes, trimmed) {
        Ok(bundle) => {
            if let Err(failure) = verify_decoded_bundle(&bundle) {
                return Err(BackupOperationReceipt::Failed {
                    failure,
                    attempted_at_unix_ms: now_unix_ms,
                });
            }
            Ok(bundle)
        }
        Err(err) => Err(BackupOperationReceipt::Failed {
            failure: map_domain_backup_error(err),
            attempted_at_unix_ms: now_unix_ms,
        }),
    }
}

/// Authoritative application owner for encrypted backup and restore.
#[derive(Clone)]
pub struct BackupRestoreApplication {
    port: Arc<dyn BackupRestorePort>,
    state: Arc<Mutex<InnerState>>,
}

impl Debug for BackupRestoreApplication {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("BackupRestoreApplication").finish()
    }
}

impl BackupRestoreApplication {
    /// Construct a new backup/restore application over the given delivery port.
    pub fn new(port: Arc<dyn BackupRestorePort>) -> Self {
        Self {
            port,
            state: Arc::new(Mutex::new(InnerState::default())),
        }
    }

    /// Select the delivery target (`LocalFile` or `WebDav`).
    pub fn set_delivery_target(&self, target: BackupDeliveryTarget) -> BackupDeliveryTarget {
        if let Ok(mut guard) = self.state.lock()
            && guard.selected_target != target
        {
            guard.selected_target = target;
            guard.revision = guard.revision.saturating_add(1);
        }
        target
    }

    /// Select the restore scope (`ConfigOnly` or `FullData`).
    pub fn set_restore_scope(&self, scope: BackupRestoreScope) -> BackupRestoreScope {
        if let Ok(mut guard) = self.state.lock()
            && guard.selected_scope != scope
        {
            guard.selected_scope = scope;
            guard.revision = guard.revision.saturating_add(1);
        }
        scope
    }

    /// Capture, encrypt, verify, and deliver a backup archive to the currently selected target.
    pub async fn export_backup(
        &self,
        passphrase: &str,
        destination_hint: &str,
        now_unix_ms: u64,
    ) -> BackupOperationReceipt {
        let target = self
            .state
            .lock()
            .map(|g| g.selected_target)
            .unwrap_or(BackupDeliveryTarget::LocalFile);

        let trimmed = passphrase.trim();
        if trimmed.chars().count() < MIN_BACKUP_PASSPHRASE_LEN {
            return self.record_receipt(BackupOperationReceipt::Failed {
                failure: BackupVerificationFailure::PassphraseTooShort {
                    min_len: MIN_BACKUP_PASSPHRASE_LEN,
                },
                attempted_at_unix_ms: now_unix_ms,
            });
        }

        let bundle = match self.port.capture_current_bundle().await {
            Ok(b) => b,
            Err(err) => {
                return self.record_receipt(BackupOperationReceipt::Failed {
                    failure: BackupVerificationFailure::DeliveryFailed {
                        target,
                        detail: err.to_string(),
                    },
                    attempted_at_unix_ms: now_unix_ms,
                });
            }
        };

        let (encrypted_bytes, _) = match encrypt_verified_bundle(
            &bundle,
            trimmed,
            destination_hint,
            target,
            now_unix_ms,
        ) {
            Ok(pair) => pair,
            Err(failed_receipt) => return self.record_receipt(failed_receipt),
        };

        match self
            .port
            .deliver_encrypted_archive(target, destination_hint, &encrypted_bytes)
            .await
        {
            Ok(canonical_dest) => {
                let header = BackupBundleHeader {
                    schema_version: bundle.manifest.version.clone(),
                    created_at_unix_ms: now_unix_ms,
                    client_version: bundle.manifest.client_version.clone(),
                    sha256_digest: bundle.manifest.sha256_digest.clone(),
                    profile_count: bundle.profiles.len(),
                };
                let receipt = BackupOperationReceipt::ExportSucceeded {
                    target,
                    destination: canonical_dest,
                    bundle_version: bundle.manifest.version,
                    sha256_digest: bundle.manifest.sha256_digest,
                    profile_count: bundle.profiles.len(),
                    exported_at_unix_ms: now_unix_ms,
                };
                if let Ok(mut guard) = self.state.lock() {
                    guard.available_bundles.push(header);
                    guard.last_receipt = receipt.clone();
                    guard.revision = guard.revision.saturating_add(1);
                }
                receipt
            }
            Err(err) => self.record_receipt(BackupOperationReceipt::Failed {
                failure: BackupVerificationFailure::DeliveryFailed {
                    target,
                    detail: err.to_string(),
                },
                attempted_at_unix_ms: now_unix_ms,
            }),
        }
    }

    /// Fetch, decrypt, verify schema + digest, and atomically restore a backup archive.
    pub async fn restore_backup(
        &self,
        passphrase: &str,
        source_uri: &str,
        now_unix_ms: u64,
    ) -> BackupOperationReceipt {
        let (target, scope) = self
            .state
            .lock()
            .map(|g| (g.selected_target, g.selected_scope))
            .unwrap_or((
                BackupDeliveryTarget::LocalFile,
                BackupRestoreScope::FullData,
            ));

        let trimmed = passphrase.trim();
        if trimmed.chars().count() < MIN_BACKUP_PASSPHRASE_LEN {
            return self.record_receipt(BackupOperationReceipt::Failed {
                failure: BackupVerificationFailure::PassphraseTooShort {
                    min_len: MIN_BACKUP_PASSPHRASE_LEN,
                },
                attempted_at_unix_ms: now_unix_ms,
            });
        }

        let encrypted_bytes = match self.port.fetch_encrypted_archive(target, source_uri).await {
            Ok(bytes) => bytes,
            Err(err) => {
                return self.record_receipt(BackupOperationReceipt::Failed {
                    failure: BackupVerificationFailure::DeliveryFailed {
                        target,
                        detail: err.to_string(),
                    },
                    attempted_at_unix_ms: now_unix_ms,
                });
            }
        };

        let bundle = match decrypt_and_verify_bundle(&encrypted_bytes, trimmed, now_unix_ms) {
            Ok(bundle) => bundle,
            Err(failed_receipt) => return self.record_receipt(failed_receipt),
        };

        match self.port.apply_restored_bundle(&bundle, scope).await {
            Ok(restored_profiles) => {
                self.record_receipt(BackupOperationReceipt::RestoreSucceeded {
                    target,
                    scope,
                    restored_profiles,
                    restored_at_unix_ms: now_unix_ms,
                })
            }
            Err(err) => self.record_receipt(BackupOperationReceipt::Failed {
                failure: BackupVerificationFailure::PartialRestoreAborted {
                    stage: scope.label_en().to_string(),
                    detail: err.to_string(),
                },
                attempted_at_unix_ms: now_unix_ms,
            }),
        }
    }

    /// Produce an authoritative snapshot for UI projection.
    pub fn snapshot(&self) -> BackupRestoreSnapshot {
        let Ok(guard) = self.state.lock() else {
            return BackupRestoreSnapshot::default();
        };
        BackupRestoreSnapshot {
            revision: guard.revision,
            selected_target: guard.selected_target,
            selected_scope: guard.selected_scope,
            last_receipt: guard.last_receipt.clone(),
            available_bundles: guard.available_bundles.clone(),
        }
    }

    fn record_receipt(&self, receipt: BackupOperationReceipt) -> BackupOperationReceipt {
        if let Ok(mut guard) = self.state.lock() {
            guard.last_receipt = receipt.clone();
            guard.revision = guard.revision.saturating_add(1);
        }
        receipt
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use infiltrator_domain::backup::ProfileBackupItem;
    use lurker_core_ports::error::PortError;
    use std::collections::HashMap;

    struct FakeBackupPort {
        live_settings: Mutex<String>,
        live_mixin: Mutex<String>,
        live_profiles: Mutex<Vec<ProfileBackupItem>>,
        local_store: Mutex<HashMap<String, Vec<u8>>>,
        webdav_store: Mutex<HashMap<String, Vec<u8>>>,
        fail_apply: Mutex<bool>,
    }

    impl FakeBackupPort {
        fn new() -> Self {
            Self {
                live_settings: Mutex::new("mixed-port = 7890\n".to_string()),
                live_mixin: Mutex::new("dns: { enable: true }\n".to_string()),
                live_profiles: Mutex::new(vec![
                    ProfileBackupItem::new("alpha", "proxies: []\n"),
                    ProfileBackupItem::new("beta", "proxies: []\n"),
                ]),
                local_store: Mutex::new(HashMap::new()),
                webdav_store: Mutex::new(HashMap::new()),
                fail_apply: Mutex::new(false),
            }
        }
    }

    #[async_trait]
    impl BackupRestorePort for FakeBackupPort {
        async fn capture_current_bundle(&self) -> Result<BackupBundle, PortError> {
            let profiles = self.live_profiles.lock().unwrap().clone();
            let settings = self.live_settings.lock().unwrap().clone();
            let mixin = self.live_mixin.lock().unwrap().clone();
            Ok(BackupBundle::new(profiles, settings, mixin))
        }

        async fn deliver_encrypted_archive(
            &self,
            target: BackupDeliveryTarget,
            destination_hint: &str,
            encrypted_bytes: &[u8],
        ) -> Result<String, PortError> {
            let store = match target {
                BackupDeliveryTarget::LocalFile => &self.local_store,
                BackupDeliveryTarget::WebDav => &self.webdav_store,
            };
            store
                .lock()
                .unwrap()
                .insert(destination_hint.to_string(), encrypted_bytes.to_vec());
            Ok(destination_hint.to_string())
        }

        async fn fetch_encrypted_archive(
            &self,
            target: BackupDeliveryTarget,
            source_uri: &str,
        ) -> Result<Vec<u8>, PortError> {
            let store = match target {
                BackupDeliveryTarget::LocalFile => &self.local_store,
                BackupDeliveryTarget::WebDav => &self.webdav_store,
            };
            store
                .lock()
                .unwrap()
                .get(source_uri)
                .cloned()
                .ok_or_else(|| PortError::NotFound(format!("archive not found: {source_uri}")))
        }

        async fn apply_restored_bundle(
            &self,
            bundle: &BackupBundle,
            scope: BackupRestoreScope,
        ) -> Result<usize, PortError> {
            if *self.fail_apply.lock().unwrap() {
                return Err(PortError::Failed(
                    "disk quota exceeded while writing profile".to_string(),
                ));
            }
            *self.live_settings.lock().unwrap() = bundle.settings_toml.clone();
            *self.live_mixin.lock().unwrap() = bundle.mixin_yaml.clone();
            if scope.includes_profiles() {
                *self.live_profiles.lock().unwrap() = bundle.profiles.clone();
                Ok(bundle.profiles.len())
            } else {
                Ok(0)
            }
        }
    }

    #[tokio::test]
    async fn local_and_webdav_export_and_scoped_restore_work_end_to_end() {
        let port = Arc::new(FakeBackupPort::new());
        let app = BackupRestoreApplication::new(port.clone());

        app.set_delivery_target(BackupDeliveryTarget::WebDav);
        let export_receipt = app
            .export_backup(
                "CorrectHorseBatteryStaple",
                "webdav://backups/v1.encpkg",
                1000,
            )
            .await;
        assert!(matches!(
            export_receipt,
            BackupOperationReceipt::ExportSucceeded {
                target: BackupDeliveryTarget::WebDav,
                profile_count: 2,
                ..
            }
        ));

        // Mutate live state before restoring
        *port.live_settings.lock().unwrap() = "mixed-port = 9999\n".to_string();
        *port.live_profiles.lock().unwrap() =
            vec![ProfileBackupItem::new("only_local", "proxies: []")];

        // 1. ConfigOnly restore updates settings but keeps existing local profiles
        app.set_restore_scope(BackupRestoreScope::ConfigOnly);
        let r1 = app
            .restore_backup(
                "CorrectHorseBatteryStaple",
                "webdav://backups/v1.encpkg",
                2000,
            )
            .await;
        assert_eq!(
            r1,
            BackupOperationReceipt::RestoreSucceeded {
                target: BackupDeliveryTarget::WebDav,
                scope: BackupRestoreScope::ConfigOnly,
                restored_profiles: 0,
                restored_at_unix_ms: 2000,
            }
        );
        assert_eq!(*port.live_settings.lock().unwrap(), "mixed-port = 7890\n");
        assert_eq!(port.live_profiles.lock().unwrap().len(), 1);

        // 2. FullData restore also restores all profile items
        app.set_restore_scope(BackupRestoreScope::FullData);
        let r2 = app
            .restore_backup(
                "CorrectHorseBatteryStaple",
                "webdav://backups/v1.encpkg",
                3000,
            )
            .await;
        assert_eq!(
            r2,
            BackupOperationReceipt::RestoreSucceeded {
                target: BackupDeliveryTarget::WebDav,
                scope: BackupRestoreScope::FullData,
                restored_profiles: 2,
                restored_at_unix_ms: 3000,
            }
        );
        assert_eq!(port.live_profiles.lock().unwrap().len(), 2);
    }

    #[tokio::test]
    async fn wrong_password_corrupted_bundle_and_partial_failure_never_mutate_state() {
        let port = Arc::new(FakeBackupPort::new());
        let app = BackupRestoreApplication::new(port.clone());

        // Short passphrase rejected
        let short = app.export_backup("123", "local.encpkg", 100).await;
        assert!(matches!(
            short,
            BackupOperationReceipt::Failed {
                failure: BackupVerificationFailure::PassphraseTooShort { min_len: 6 },
                ..
            }
        ));

        // Valid export
        app.export_backup("MasterSecret2026", "local.encpkg", 200)
            .await;

        // Mutate live state to verify failed restores do not touch it
        *port.live_settings.lock().unwrap() = "untouched = true\n".to_string();

        // Wrong password rejected
        let wrong_pass = app
            .restore_backup("WrongPassphrase999", "local.encpkg", 300)
            .await;
        assert_eq!(
            wrong_pass,
            BackupOperationReceipt::Failed {
                failure: BackupVerificationFailure::WrongPassphraseOrAuthTag,
                attempted_at_unix_ms: 300,
            }
        );
        assert_eq!(*port.live_settings.lock().unwrap(), "untouched = true\n");

        // Corrupted payload rejected
        port.local_store
            .lock()
            .unwrap()
            .insert("corrupt.encpkg".to_string(), b"not-an-iftr-bundle".to_vec());
        let corrupted = app
            .restore_backup("MasterSecret2026", "corrupt.encpkg", 400)
            .await;
        assert!(matches!(
            corrupted,
            BackupOperationReceipt::Failed {
                failure: BackupVerificationFailure::CorruptedPayload { .. },
                ..
            }
        ));

        // Partial restore failure aborted atomically
        *port.fail_apply.lock().unwrap() = true;
        let aborted = app
            .restore_backup("MasterSecret2026", "local.encpkg", 500)
            .await;
        assert!(matches!(
            aborted,
            BackupOperationReceipt::Failed {
                failure: BackupVerificationFailure::PartialRestoreAborted { .. },
                ..
            }
        ));
        assert_eq!(*port.live_settings.lock().unwrap(), "untouched = true\n");
    }

    #[test]
    fn incompatible_schema_version_and_digest_mismatch_are_rejected() {
        let mut bundle = BackupBundle::new(
            vec![ProfileBackupItem::new("p1", "proxies: []")],
            "port = 7890".to_string(),
            String::new(),
        );
        bundle.manifest.version = "2.0.0".to_string();
        assert_eq!(
            verify_decoded_bundle(&bundle),
            Err(BackupVerificationFailure::IncompatibleSchemaVersion {
                bundle_version: "2.0.0".to_string(),
                supported_major: 1,
            })
        );

        bundle.manifest.version = "1.0.0".to_string();
        bundle.manifest.sha256_digest = "0000000000000000".to_string();
        assert!(matches!(
            verify_decoded_bundle(&bundle),
            Err(BackupVerificationFailure::IntegrityDigestMismatch { .. })
        ));
    }
}
