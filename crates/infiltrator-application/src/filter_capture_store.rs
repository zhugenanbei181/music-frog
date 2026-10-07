//! Isolated complete profile/options transactions for native tests and captures.
use async_trait::async_trait;
use infiltrator_contract::capability::Capability;
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::profile_protection::ProfileWriteProtection;
use infiltrator_contract::profile_source::ProfileSourceIdentity;
use infiltrator_domain::profile_options::{FilterSpec, ProfileOptions};
use infiltrator_domain::profile_source::identify_profile_source;
use infiltrator_domain::profiles::{ProfileInfo, ProfileMetadata};
use infiltrator_ports::error::PortError;
use infiltrator_ports::profile_store::ProfileStore;
use infiltrator_ports::profile_workspace::{ProfileWorkspace, ProfileWorkspaceUpdate};
use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

pub const FILTER_PROFILE: &str = "filter-source";
pub const FILTER_DOCUMENT: &str = "mixed-port: 7890\nmode: rule\nproxies:\n  - {name: HK-public, type: ss, server: 1.1.1.1, port: 443, cipher: aes-128-gcm, password: fixture}\n  - {name: HK-private, type: ss, server: 192.168.1.1, port: 443, cipher: aes-128-gcm, password: fixture}\n  - {name: US-public, type: ss, server: 8.8.8.8, port: 80, cipher: aes-128-gcm, password: fixture}\n";
pub const FILTER_POLICY: &str = "{drop-private-ip: true, remove-emojis: true, sort-by: name-desc}";

pub struct FilterCaptureStore {
    workspace: Mutex<ProfileWorkspace>,
    pub deny_write: AtomicBool,
    pub deny_read: AtomicBool,
    pub writes: AtomicUsize,
}
fn workspace(content: String, options: ProfileOptions) -> Result<ProfileWorkspace, PortError> {
    let raw = if options.is_empty() {
        None
    } else {
        Some(serde_yaml_ng::to_string(&options).map_err(|error| PortError::Io(error.to_string()))?)
    };
    Ok(ProfileWorkspace {
        write_protection: ProfileWriteProtection::Editable,
        source: identify_profile_source(FILTER_PROFILE.into(), &content, raw.as_deref()),
        content,
        options,
        options_document: raw,
    })
}
impl Default for FilterCaptureStore {
    fn default() -> Self {
        Self {
            workspace: Mutex::new(
                workspace(
                    FILTER_DOCUMENT.into(),
                    ProfileOptions {
                        filter: Some(FilterSpec {
                            drop_private_ip: true,
                            ..Default::default()
                        }),
                        ..Default::default()
                    },
                )
                .expect("explicit filter fixture"),
            ),
            deny_write: AtomicBool::new(false),
            deny_read: AtomicBool::new(false),
            writes: AtomicUsize::new(0),
        }
    }
}
impl FilterCaptureStore {
    pub fn observed(&self) -> ProfileWorkspace {
        self.workspace
            .lock()
            .expect("isolated filter store")
            .clone()
    }
    fn check_profile(profile: &str) -> Result<(), PortError> {
        if profile == FILTER_PROFILE {
            Ok(())
        } else {
            Err(PortError::NotFound(profile.into()))
        }
    }
}
fn outside() -> PortError {
    PortError::unsupported(
        Capability::Profiles,
        "operation is outside the isolated filter transaction",
    )
}
#[async_trait]
impl ProfileStore for FilterCaptureStore {
    fn config_dir(&self) -> PathBuf {
        PathBuf::from("/isolated-filter")
    }
    async fn list_profiles(&self) -> Result<Vec<ProfileInfo>, PortError> {
        Ok(vec![ProfileInfo {
            name: FILTER_PROFILE.into(),
            active: true,
            ..Default::default()
        }])
    }
    async fn get_current(&self) -> Result<String, PortError> {
        Ok(FILTER_PROFILE.into())
    }
    async fn set_current(&self, _: &str) -> Result<(), PortError> {
        Err(outside())
    }
    async fn load(&self, profile: &str) -> Result<String, PortError> {
        Self::check_profile(profile)?;
        Ok(self.observed().content)
    }
    async fn save(&self, _: &str, _: &str) -> Result<(), PortError> {
        Err(outside())
    }
    async fn delete_profile(&self, _: &str) -> Result<(), PortError> {
        Err(outside())
    }
    async fn get_profile_metadata(&self, profile: &str) -> Result<ProfileMetadata, PortError> {
        Self::check_profile(profile)?;
        Ok(ProfileMetadata::default())
    }
    async fn update_profile_metadata(&self, _: &str, _: &ProfileMetadata) -> Result<(), PortError> {
        Err(outside())
    }
    async fn delete_subscription_credential(&self, _: &str) -> Result<(), PortError> {
        Err(outside())
    }
    async fn load_options(&self, profile: &str) -> Result<ProfileOptions, PortError> {
        Self::check_profile(profile)?;
        Ok(self.observed().options)
    }
    async fn save_options(&self, _: &str, _: &ProfileOptions) -> Result<(), PortError> {
        Err(outside())
    }
    async fn delete_options(&self, _: &str) -> Result<(), PortError> {
        Err(outside())
    }
    async fn load_workspace(&self, profile: &str) -> Result<ProfileWorkspace, PortError> {
        Self::check_profile(profile)?;
        if self.deny_read.load(Ordering::SeqCst) {
            return Err(PortError::PermissionDenied(
                "isolated editor read denied".into(),
            ));
        }
        Ok(self.observed())
    }
    async fn load_active_workspace(&self) -> Result<ProfileWorkspace, PortError> {
        Ok(self.observed())
    }
    async fn compare_and_save_workspace(
        &self,
        expected: &ProfileSourceIdentity,
        update: &ProfileWorkspaceUpdate,
    ) -> Result<ProfileWorkspace, PortError> {
        let mut current = self.workspace.lock().expect("isolated filter store");
        if current.source != *expected {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::NotReady,
                "The profile or options changed",
                true,
            )));
        }
        if self.deny_write.load(Ordering::SeqCst) {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::Permission,
                "Allow profile write access, then retry",
                true,
            )));
        }
        let committed = workspace(update.content.clone(), update.options.clone())?;
        *current = committed.clone();
        self.writes.fetch_add(1, Ordering::SeqCst);
        Ok(committed)
    }
    async fn clear_backup(&self, profile: &str) -> Result<(), PortError> {
        Self::check_profile(profile)
    }
    async fn restore_backup(&self, _: &str) -> Result<bool, PortError> {
        Err(outside())
    }
}
