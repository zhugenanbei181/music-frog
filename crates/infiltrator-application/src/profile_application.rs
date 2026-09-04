//! Profile use-cases over the runtime-neutral profile store port.
//!
//! This module owns profile identity validation, projections, activation, and
//! the boundary between a stored profile and a live managed runtime. Concrete
//! config managers and filesystem details stay in outbound adapters.

use chrono::Utc;
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_domain::apply::ApplyStrategy;
use infiltrator_domain::profiles::{
    ProfileDetail, ProfileInfo, ProfileMetadata, sanitize_profile_name,
};
use infiltrator_domain::subscription::CheckedSubscriptionUrl;
use infiltrator_ports::profile_store::ProfileStore;
use infiltrator_ports::runtime_gateway::ManagedRuntime;
use infiltrator_ports::subscription_source::SubscriptionSource;
use std::fmt::Display;
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Clone)]
pub struct ProfileApplication {
    store: Arc<dyn ProfileStore>,
}

impl ProfileApplication {
    pub fn new(store: Arc<dyn ProfileStore>) -> Self {
        Self { store }
    }

    pub fn config_dir(&self) -> PathBuf {
        self.store.config_dir()
    }

    pub async fn list_profiles(&self) -> Result<Vec<ProfileInfo>, Failure> {
        self.store.list_profiles().await.map_err(Failure::from)
    }

    pub async fn current_profile(&self) -> Result<String, Failure> {
        self.store.get_current().await.map_err(Failure::from)
    }

    pub async fn current_content(&self) -> Result<(String, String), Failure> {
        let profile = self.current_profile().await?;
        let content = self.store.load(&profile).await.map_err(Failure::from)?;
        Ok((profile, content))
    }

    pub async fn load_profile_info(&self, name: &str) -> Result<ProfileInfo, Failure> {
        let name = valid_name(name)?;
        self.list_profiles()
            .await?
            .into_iter()
            .find(|profile| profile.name == name)
            .ok_or_else(|| {
                Failure::new(
                    ErrorCode::Configuration,
                    format!("profile `{name}` was not found"),
                    false,
                )
            })
    }

    pub async fn load_profile_detail(&self, name: &str) -> Result<ProfileDetail, Failure> {
        let profile = self.load_profile_info(name).await?;
        let content = self
            .store
            .load(&profile.name)
            .await
            .map_err(Failure::from)?;
        Ok(ProfileDetail {
            name: profile.name,
            active: profile.active,
            path: profile.path,
            content,
            subscription_url: profile.subscription_url,
            auto_update_enabled: profile.auto_update_enabled,
            update_interval_hours: profile.update_interval_hours,
            last_updated: profile.last_updated,
            next_update: profile.next_update,
            traffic_upload: profile.traffic_upload,
            traffic_download: profile.traffic_download,
            traffic_total: profile.traffic_total,
            expire_at: profile.expire_at,
        })
    }

    pub async fn select_profile(&self, name: &str) -> Result<ProfileInfo, Failure> {
        let name = valid_name(name)?;
        self.store.set_current(&name).await.map_err(Failure::from)?;
        self.load_profile_info(&name).await
    }

    pub async fn delete_profile(&self, name: &str) -> Result<(), Failure> {
        let name = valid_name(name)?;
        self.store
            .delete_profile(&name)
            .await
            .map_err(Failure::from)?;
        self.store
            .delete_options(&name)
            .await
            .map_err(Failure::from)
    }

    pub async fn load_metadata(&self, name: &str) -> Result<ProfileMetadata, Failure> {
        let name = valid_name(name)?;
        self.store
            .get_profile_metadata(&name)
            .await
            .map_err(Failure::from)
    }

    pub async fn update_metadata(
        &self,
        name: &str,
        metadata: &ProfileMetadata,
    ) -> Result<(), Failure> {
        let name = valid_name(name)?;
        self.store
            .update_profile_metadata(&name, metadata)
            .await
            .map_err(Failure::from)
    }

    pub async fn save_profile(&self, name: &str, content: &str) -> Result<(), Failure> {
        let name = valid_name(name)?;
        self.store.save(&name, content).await.map_err(Failure::from)
    }

    pub async fn clear_backup(&self, name: &str) -> Result<(), Failure> {
        let name = valid_name(name)?;
        self.store.clear_backup(&name).await.map_err(Failure::from)
    }

    pub async fn delete_subscription_credential(&self, name: &str) -> Result<(), Failure> {
        let name = valid_name(name)?;
        self.store
            .delete_subscription_credential(&name)
            .await
            .map_err(Failure::from)
    }

    pub async fn import_subscription<S: SubscriptionSource + ?Sized>(
        &self,
        source: &S,
        name: &str,
        url: &str,
    ) -> Result<ProfileInfo, Failure> {
        let name = valid_name(name)?;
        let url = CheckedSubscriptionUrl::parse(url)
            .map_err(|error| Failure::new(ErrorCode::InvalidInput, error.to_string(), false))?;
        let document = source.fetch(&name, &url).await.map_err(Failure::from)?;
        commit_subscription_document(&self.store, &name, url.as_str(), document).await?;
        self.load_profile_info(&name).await
    }

    pub async fn update_subscription<S: SubscriptionSource + ?Sized>(
        &self,
        source: &S,
        name: &str,
    ) -> Result<ProfileInfo, Failure> {
        let name = valid_name(name)?;
        let mut metadata = self.load_metadata(&name).await?;
        let url = metadata.subscription_url.as_deref().ok_or_else(|| {
            Failure::new(
                ErrorCode::Configuration,
                "subscription URL is missing",
                false,
            )
        })?;
        let url = CheckedSubscriptionUrl::parse(url)
            .map_err(|error| Failure::new(ErrorCode::InvalidInput, error.to_string(), false))?;
        let document = source.fetch(&name, &url).await.map_err(Failure::from)?;
        self.store
            .save(&name, &document.content)
            .await
            .map_err(Failure::from)?;

        let now = Utc::now();
        apply_subscription_metadata(&mut metadata, document.userinfo, now);
        self.update_metadata(&name, &metadata).await?;
        self.load_profile_info(&name).await
    }

    /// Commit an arbitrary profile document through the managed runtime when
    /// the document belongs to the active profile. Inactive profiles use the
    /// store writer and clear their transient backup after a successful save.
    pub async fn save_profile_content<R: ManagedRuntime + ?Sized>(
        &self,
        runtime: Option<Arc<R>>,
        profile: String,
        content: String,
        strategy: ApplyStrategy,
    ) -> Result<(), Failure> {
        let profile = valid_name(&profile)?;
        let current = self.store.get_current().await.map_err(Failure::from)?;
        if let Some(runtime) = runtime
            && current == profile
        {
            runtime
                .apply_profile_content(&content, strategy)
                .await
                .map(|_| ())
                .map_err(Failure::from)
        } else {
            self.store
                .save(&profile, &content)
                .await
                .map_err(Failure::from)?;
            self.store
                .clear_backup(&profile)
                .await
                .map_err(Failure::from)
        }
    }

    pub async fn save_current_profile_content<F, E, R>(
        &self,
        runtime: Option<Arc<R>>,
        strategy: ApplyStrategy,
        transform: F,
    ) -> Result<(), Failure>
    where
        F: FnOnce(&str) -> Result<String, E> + Send + 'static,
        E: Display + Send + 'static,
        R: ManagedRuntime + ?Sized,
    {
        let profile = self.store.get_current().await.map_err(Failure::from)?;
        let content = self.store.load(&profile).await.map_err(Failure::from)?;
        let updated = transform(&content)
            .map_err(|error| Failure::new(ErrorCode::Configuration, error.to_string(), false))?;
        self.save_profile_content(runtime, profile, updated, strategy)
            .await
    }

    /// Switch the active profile and re-apply it to a running core. A failed
    /// apply restores both the profile pointer and the previous live config.
    pub async fn activate_profile<R: ManagedRuntime + ?Sized>(
        &self,
        runtime: Option<Arc<R>>,
        profile: &str,
    ) -> Result<bool, Failure> {
        let profile = valid_name(profile)?;
        let previous = self.store.get_current().await.map_err(Failure::from)?;
        if previous == profile {
            return Ok(runtime.is_some());
        }

        self.store
            .set_current(&profile)
            .await
            .map_err(Failure::from)?;

        let Some(runtime) = runtime else {
            return Ok(false);
        };

        if let Err(error) = runtime
            .apply_current_config(ApplyStrategy::AlwaysRestart)
            .await
        {
            let _ = self.store.set_current(&previous).await;
            if let Err(recovery) = runtime
                .apply_current_config(ApplyStrategy::AlwaysRestart)
                .await
            {
                let _ = self.store.clear_backup(&profile).await;
                return Err(Failure::new(
                    ErrorCode::Internal,
                    format!("profile switch failed: {error}; recovery also failed: {recovery}"),
                    false,
                ));
            }
            let _ = self.store.clear_backup(&profile).await;
            return Err(Failure::new(
                ErrorCode::Internal,
                format!("profile switch failed; previous profile restored: {error}"),
                false,
            ));
        }

        Ok(true)
    }
}

fn valid_name(name: &str) -> Result<String, Failure> {
    sanitize_profile_name(name)
        .map_err(|error| Failure::new(ErrorCode::InvalidInput, error.to_string(), false))
}

async fn commit_subscription_document(
    store: &Arc<dyn ProfileStore>,
    name: &str,
    source_url: &str,
    document: infiltrator_ports::subscription_source::SubscriptionDocument,
) -> Result<(), Failure> {
    if document.content.trim().is_empty() {
        return Err(Failure::new(
            ErrorCode::Configuration,
            "subscription returned empty content",
            false,
        ));
    }
    infiltrator_domain::config::validate_yaml(&document.content)
        .map_err(|error| Failure::new(ErrorCode::Configuration, error.to_string(), false))?;
    store
        .save(name, &document.content)
        .await
        .map_err(Failure::from)?;

    let mut metadata = store
        .get_profile_metadata(name)
        .await
        .map_err(Failure::from)?;
    metadata.subscription_url = Some(source_url.to_owned());
    apply_subscription_metadata(&mut metadata, document.userinfo, Utc::now());
    store
        .update_profile_metadata(name, &metadata)
        .await
        .map_err(Failure::from)
}

fn apply_subscription_metadata(
    metadata: &mut infiltrator_domain::profiles::ProfileMetadata,
    userinfo: Option<infiltrator_domain::subscription::SubscriptionUserInfo>,
    now: chrono::DateTime<Utc>,
) {
    if let Some(info) = userinfo {
        metadata.traffic_upload = info.upload;
        metadata.traffic_download = info.download;
        metadata.traffic_total = info.total;
        metadata.expire_at = info.expire;
    }
    metadata.last_updated = Some(now);
    metadata.next_update = if metadata.auto_update_enabled {
        metadata
            .update_interval_hours
            .map(|hours| now + chrono::Duration::hours(hours as i64))
    } else {
        None
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use infiltrator_domain::profiles::ProfileMetadata;
    use infiltrator_ports::error::PortError;
    use std::collections::BTreeMap;
    use std::sync::Mutex;

    #[derive(Default)]
    struct FakeStore {
        current: Mutex<String>,
        profiles: Mutex<BTreeMap<String, (String, ProfileMetadata)>>,
        deleted_options: Mutex<Vec<String>>,
        cleared_backups: Mutex<Vec<String>>,
    }

    impl FakeStore {
        fn with_profile(name: &str, content: &str, active: bool) -> Self {
            let mut profiles = BTreeMap::new();
            profiles.insert(
                name.to_string(),
                (content.to_string(), ProfileMetadata::default()),
            );
            Self {
                current: Mutex::new(if active {
                    name.to_string()
                } else {
                    String::new()
                }),
                profiles: Mutex::new(profiles),
                ..Self::default()
            }
        }
    }

    #[async_trait]
    impl ProfileStore for FakeStore {
        fn config_dir(&self) -> PathBuf {
            PathBuf::from("/fake/configs")
        }

        async fn list_profiles(&self) -> Result<Vec<ProfileInfo>, PortError> {
            let current = self.current.lock().expect("current lock").clone();
            Ok(self
                .profiles
                .lock()
                .expect("profiles lock")
                .iter()
                .map(|(name, (_content, metadata))| ProfileInfo {
                    name: name.clone(),
                    active: current == *name,
                    path: format!("/fake/configs/{name}.yaml"),
                    subscription_url: metadata.subscription_url.clone(),
                    auto_update_enabled: metadata.auto_update_enabled,
                    update_interval_hours: metadata.update_interval_hours,
                    last_updated: metadata.last_updated,
                    next_update: metadata.next_update,
                    traffic_upload: metadata.traffic_upload,
                    traffic_download: metadata.traffic_download,
                    traffic_total: metadata.traffic_total,
                    expire_at: metadata.expire_at,
                    controller_url: None,
                    controller_changed: None,
                })
                .collect())
        }

        async fn get_current(&self) -> Result<String, PortError> {
            Ok(self.current.lock().expect("current lock").clone())
        }

        async fn set_current(&self, profile: &str) -> Result<(), PortError> {
            if !self
                .profiles
                .lock()
                .expect("profiles lock")
                .contains_key(profile)
            {
                return Err(PortError::NotFound(profile.to_string()));
            }
            *self.current.lock().expect("current lock") = profile.to_string();
            Ok(())
        }

        async fn load(&self, profile: &str) -> Result<String, PortError> {
            self.profiles
                .lock()
                .expect("profiles lock")
                .get(profile)
                .map(|(content, _)| content.clone())
                .ok_or_else(|| PortError::NotFound(profile.to_string()))
        }

        async fn save(&self, profile: &str, content: &str) -> Result<(), PortError> {
            self.profiles
                .lock()
                .expect("profiles lock")
                .entry(profile.to_string())
                .or_insert_with(|| (String::new(), ProfileMetadata::default()))
                .0 = content.to_string();
            Ok(())
        }

        async fn delete_profile(&self, profile: &str) -> Result<(), PortError> {
            self.profiles
                .lock()
                .expect("profiles lock")
                .remove(profile)
                .map(|_| ())
                .ok_or_else(|| PortError::NotFound(profile.to_string()))
        }

        async fn get_profile_metadata(&self, profile: &str) -> Result<ProfileMetadata, PortError> {
            self.profiles
                .lock()
                .expect("profiles lock")
                .get(profile)
                .map(|(_, metadata)| metadata.clone())
                .ok_or_else(|| PortError::NotFound(profile.to_string()))
        }

        async fn update_profile_metadata(
            &self,
            profile: &str,
            metadata: &ProfileMetadata,
        ) -> Result<(), PortError> {
            self.profiles
                .lock()
                .expect("profiles lock")
                .get_mut(profile)
                .map(|(_, current)| *current = metadata.clone())
                .ok_or_else(|| PortError::NotFound(profile.to_string()))
        }

        async fn delete_subscription_credential(&self, _profile: &str) -> Result<(), PortError> {
            Ok(())
        }

        async fn delete_options(&self, profile: &str) -> Result<(), PortError> {
            self.deleted_options
                .lock()
                .expect("options lock")
                .push(profile.to_string());
            Ok(())
        }

        async fn clear_backup(&self, profile: &str) -> Result<(), PortError> {
            self.cleared_backups
                .lock()
                .expect("backup lock")
                .push(profile.to_string());
            Ok(())
        }

        async fn restore_backup(&self, _profile: &str) -> Result<bool, PortError> {
            Ok(false)
        }
    }

    #[tokio::test]
    async fn list_and_detail_are_projected_from_the_store() {
        let store = Arc::new(FakeStore::with_profile("main", "mode: rule\n", true));
        let application = ProfileApplication::new(store);

        let profiles = application.list_profiles().await.expect("list");
        assert_eq!(profiles.len(), 1);
        assert!(profiles[0].active);

        let detail = application
            .load_profile_detail("main")
            .await
            .expect("detail");
        assert_eq!(detail.content, "mode: rule\n");
        assert!(detail.active);
    }

    #[tokio::test]
    async fn invalid_names_are_rejected_before_store_access() {
        let application = ProfileApplication::new(Arc::new(FakeStore::default()));
        let failure = application
            .load_profile_info("../outside")
            .await
            .expect_err("path-like name must fail");
        assert_eq!(failure.code, ErrorCode::InvalidInput);
    }

    #[tokio::test]
    async fn selection_updates_the_current_profile() {
        let store = Arc::new(FakeStore::with_profile("main", "mode: rule\n", false));
        let application = ProfileApplication::new(Arc::clone(&store) as Arc<dyn ProfileStore>);

        let selected = application.select_profile("main").await.expect("select");
        assert!(selected.active);
        assert_eq!(
            application.current_profile().await.expect("current"),
            "main"
        );
    }

    #[tokio::test]
    async fn deletion_cleans_the_profile_sidecar() {
        let store = Arc::new(FakeStore::with_profile("main", "mode: rule\n", false));
        let application = ProfileApplication::new(Arc::clone(&store) as Arc<dyn ProfileStore>);

        application.delete_profile("main").await.expect("delete");
        assert_eq!(
            store
                .deleted_options
                .lock()
                .expect("options lock")
                .as_slice(),
            &["main".to_string()]
        );
    }

    #[tokio::test]
    async fn inactive_save_clears_the_transient_backup() {
        let store = Arc::new(FakeStore::with_profile("main", "mode: rule\n", false));
        let application = ProfileApplication::new(Arc::clone(&store) as Arc<dyn ProfileStore>);
        let runtime: Option<Arc<dyn ManagedRuntime>> = None;

        application
            .save_profile_content(
                runtime,
                "main".to_string(),
                "mode: direct\n".to_string(),
                ApplyStrategy::PreferReload,
            )
            .await
            .expect("save");
        assert_eq!(store.load("main").await.expect("load"), "mode: direct\n");
        assert_eq!(
            store
                .cleared_backups
                .lock()
                .expect("backup lock")
                .as_slice(),
            &["main".to_string()]
        );
    }
}
