//! Explicit isolated profile ports for Hosts behavior and capture; no filesystem or network fallback.
use async_trait::async_trait;
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_domain::profile_options::ProfileOptions;
use infiltrator_domain::profiles::{ProfileInfo, ProfileMetadata};
use infiltrator_ports::error::PortError;
use infiltrator_ports::profile_store::ProfileStore;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{
    Mutex,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};
pub const HOSTS_PROFILE: &str = "hosts-work";
pub const ORIGINAL_HOSTS: &str = "mixed-port: 7890\ndns:\n  enable: false\n  future-setting: preserve-me\n  hosts:\n    legacy.test: 1.1.1.1\nhosts:\n  live.test: 9.9.9.9\n  second.test: 8.8.8.8\n";
pub struct HostsCaptureStore {
    current: Mutex<String>,
    files: Mutex<BTreeMap<String, String>>,
    pub deny_save: AtomicBool,
    pub deny_read: AtomicBool,
    pub writes: AtomicUsize,
}
impl Default for HostsCaptureStore {
    fn default() -> Self {
        Self {
            current: Mutex::new(HOSTS_PROFILE.into()),
            files: Mutex::new(BTreeMap::from([
                (HOSTS_PROFILE.into(), ORIGINAL_HOSTS.into()),
                ("other".into(), "hosts:\n  other.test: 4.4.4.4\n".into()),
            ])),
            deny_save: AtomicBool::new(false),
            deny_read: AtomicBool::new(false),
            writes: AtomicUsize::new(0),
        }
    }
}
impl HostsCaptureStore {
    pub fn content(&self) -> String {
        self.files.lock().expect("isolated file state")[HOSTS_PROFILE].clone()
    }
    pub fn replace(&self, content: &str) {
        self.files
            .lock()
            .expect("isolated file state")
            .insert(HOSTS_PROFILE.into(), content.into());
    }
}
#[async_trait]
impl ProfileStore for HostsCaptureStore {
    fn config_dir(&self) -> PathBuf {
        PathBuf::from("/isolated-hosts")
    }
    async fn list_profiles(&self) -> Result<Vec<ProfileInfo>, PortError> {
        let current = self.current.lock().expect("isolated current").clone();
        Ok(self
            .files
            .lock()
            .expect("isolated files")
            .keys()
            .map(|name| ProfileInfo {
                name: name.clone(),
                active: name == &current,
                ..Default::default()
            })
            .collect())
    }
    async fn get_current(&self) -> Result<String, PortError> {
        Ok(self.current.lock().expect("isolated current").clone())
    }
    async fn set_current(&self, profile: &str) -> Result<(), PortError> {
        *self.current.lock().expect("isolated current") = profile.into();
        Ok(())
    }
    async fn load(&self, profile: &str) -> Result<String, PortError> {
        if self.deny_read.load(Ordering::SeqCst) {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::Permission,
                "Allow profile access, then retry the Hosts operation.",
                true,
            )));
        }
        self.files
            .lock()
            .expect("isolated files")
            .get(profile)
            .cloned()
            .ok_or_else(|| PortError::NotFound(profile.into()))
    }
    async fn save(&self, profile: &str, content: &str) -> Result<(), PortError> {
        self.writes.fetch_add(1, Ordering::SeqCst);
        if self.deny_save.load(Ordering::SeqCst) {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::Permission,
                "Allow profile writes, then retry the Hosts operation.",
                true,
            )));
        }
        self.files
            .lock()
            .expect("isolated files")
            .insert(profile.into(), content.into());
        Ok(())
    }
    async fn delete_profile(&self, profile: &str) -> Result<(), PortError> {
        self.files.lock().expect("isolated files").remove(profile);
        Ok(())
    }
    async fn get_profile_metadata(&self, _: &str) -> Result<ProfileMetadata, PortError> {
        Ok(ProfileMetadata::default())
    }
    async fn update_profile_metadata(&self, _: &str, _: &ProfileMetadata) -> Result<(), PortError> {
        Err(PortError::NotFound(
            "metadata write is outside this isolated Hosts port".into(),
        ))
    }
    async fn delete_subscription_credential(&self, _: &str) -> Result<(), PortError> {
        Err(PortError::NotFound(
            "subscription credentials are outside this isolated Hosts port".into(),
        ))
    }
    async fn load_options(&self, _: &str) -> Result<ProfileOptions, PortError> {
        Ok(ProfileOptions::default())
    }
    async fn save_options(&self, _: &str, _: &ProfileOptions) -> Result<(), PortError> {
        Err(PortError::NotFound(
            "option write is outside this isolated Hosts port".into(),
        ))
    }
    async fn delete_options(&self, _: &str) -> Result<(), PortError> {
        Err(PortError::NotFound(
            "option removal is outside this isolated Hosts port".into(),
        ))
    }
    async fn clear_backup(&self, _: &str) -> Result<(), PortError> {
        Ok(())
    }
    async fn restore_backup(&self, _: &str) -> Result<bool, PortError> {
        Ok(false)
    }
}
