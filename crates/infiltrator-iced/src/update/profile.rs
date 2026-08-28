use crate::state::AppState;
use crate::types::{InfiltratorError, Message, ToastStatus};
use chrono::Utc;
use dav_client::{DavClient, client::WebDavClient};
use iced::Task;
use iced::widget::text_editor;
use infiltrator_core::settings::{AppSettings, WebDavConfig};
use mihomo_config::ConfigManager;

impl AppState {
    fn sync_subscription_editor(&mut self) {
        if self.profiles.is_empty() {
            self.subscription_profile_name.clear();
            self.subscription_url.clear();
            self.subscription_auto_update_enabled = false;
            self.subscription_update_interval_hours.clear();
            return;
        }

        let selected_name = if !self.subscription_profile_name.is_empty()
            && self
                .profiles
                .iter()
                .any(|p| p.name == self.subscription_profile_name)
        {
            self.subscription_profile_name.clone()
        } else if let Some(active) = self.profiles.iter().find(|p| p.active) {
            active.name.clone()
        } else {
            self.profiles[0].name.clone()
        };

        self.subscription_profile_name = selected_name.clone();
        if let Some(profile) = self.profiles.iter().find(|p| p.name == selected_name) {
            self.subscription_url = profile.subscription_url.clone().unwrap_or_default();
            self.subscription_auto_update_enabled = profile.auto_update_enabled;
            self.subscription_update_interval_hours = profile
                .update_interval_hours
                .map(|hours| hours.to_string())
                .unwrap_or_else(|| "24".to_string());
        }
    }

    fn invalidate_rules_dns_views(&mut self) {
        self.rules_loaded_once = false;
        self.advanced_configs_loaded_once = false;
        self.rules_render_cache.clear();
        self.rules_filtered_indices.clear();
        self.rules_page = 0;
        self.rules_heavy_ready = false;
        self.dns_heavy_ready = false;
        self.rule_providers_editor_state = crate::types::EditorLazyState::Unloaded;
        self.proxy_providers_editor_state = crate::types::EditorLazyState::Unloaded;
        self.sniffer_editor_state = crate::types::EditorLazyState::Unloaded;
        self.dns_editor_state = crate::types::EditorLazyState::Unloaded;
        self.fake_ip_editor_state = crate::types::EditorLazyState::Unloaded;
        self.tun_editor_state = crate::types::EditorLazyState::Unloaded;
    }

    pub fn update_profile(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::LoadProfiles => {
                self.is_loading_profiles = true;
                Task::perform(
                    async {
                        let cm = ConfigManager::new()
                            .map_err(|e: mihomo_api::MihomoError| InfiltratorError::from(e))?;
                        cm.list_profiles()
                            .await
                            .map_err(|e: mihomo_api::MihomoError| InfiltratorError::from(e))
                    },
                    Message::ProfilesLoaded,
                )
            }
            Message::ProfilesLoaded(result) => {
                self.is_loading_profiles = false;
                match result {
                    Ok(profiles) => {
                        self.profiles = profiles;
                        self.sync_subscription_editor();
                        Task::none()
                    }
                    Err(e) => {
                        self.error_msg = Some(e.to_string());
                        Task::none()
                    }
                }
            }
            Message::UpdateProfilesFilter(filter) => {
                self.profiles_filter = filter;
                Task::none()
            }
            Message::ClearProfiles => {
                self.error_msg = None;
                self.is_loading_profiles = true;
                Task::perform(
                    async {
                        infiltrator_core::profiles::reset_profiles_to_default()
                            .await
                            .map_err(|e| InfiltratorError::Config(e.to_string()))?;
                        Ok(())
                    },
                    Message::ProfilesCleared,
                )
            }
            Message::ProfilesCleared(result) => {
                self.is_loading_profiles = false;
                match result {
                    Ok(_) => {
                        self.invalidate_rules_dns_views();
                        self.profiles_filter.clear();
                        Task::batch(vec![
                            Task::done(Message::LoadProfiles),
                            Task::done(Message::StartProxy),
                            Task::done(Message::ShowToast(
                                "Profiles reset to default".to_string(),
                                ToastStatus::Success,
                            )),
                        ])
                    }
                    Err(e) => {
                        self.error_msg = Some(e.to_string());
                        Task::done(Message::ShowToast(e.to_string(), ToastStatus::Error))
                    }
                }
            }
            Message::SetActiveProfile(name) => {
                self.error_msg = None;
                self.invalidate_rules_dns_views();
                Task::perform(
                    async move {
                        let cm = ConfigManager::new()
                            .map_err(|e: mihomo_api::MihomoError| InfiltratorError::from(e))?;
                        cm.set_current(&name)
                            .await
                            .map_err(|e: mihomo_api::MihomoError| InfiltratorError::from(e))?;
                        Ok(())
                    },
                    |result: Result<(), InfiltratorError>| match result {
                        Ok(_) => Message::StartProxy,
                        Err(e) => Message::ShowToast(e.to_string(), ToastStatus::Error),
                    },
                )
            }
            Message::UpdateImportUrl(url) => {
                self.import_url = url;
                Task::none()
            }
            Message::UpdateImportName(name) => {
                self.import_name = name;
                Task::none()
            }
            Message::UpdateImportActivate(enabled) => {
                self.import_activate = enabled;
                Task::none()
            }
            Message::ImportProfile => {
                let url = self.import_url.trim().to_string();
                let name = self.import_name.trim().to_string();
                let activate = self.import_activate;
                if name.is_empty() || url.is_empty() {
                    return Task::done(Message::ShowToast(
                        "Name and subscription URL are required".to_string(),
                        ToastStatus::Error,
                    ));
                }

                self.is_importing = true;
                Task::perform(
                    async move {
                        let profile_name = infiltrator_core::profiles::sanitize_profile_name(&name)
                            .map_err(|e| InfiltratorError::Config(e.to_string()))?;
                        infiltrator_core::profiles::create_profile_from_url(&profile_name, &url)
                            .await
                            .map_err(|e| InfiltratorError::Config(e.to_string()))?;

                        if activate {
                            let cm = ConfigManager::new().map_err(InfiltratorError::from)?;
                            cm.set_current(&profile_name)
                                .await
                                .map_err(InfiltratorError::from)?;
                        }
                        Ok(())
                    },
                    Message::ProfileImported,
                )
            }
            Message::ProfileImported(result) => {
                self.is_importing = false;
                match result {
                    Ok(_) => {
                        self.invalidate_rules_dns_views();
                        let activate = self.import_activate;
                        self.import_url.clear();
                        self.import_name.clear();
                        self.import_activate = false;
                        let mut tasks = vec![
                            Task::done(Message::LoadProfiles),
                            Task::done(Message::ShowToast(
                                "Profile imported successfully".to_string(),
                                ToastStatus::Success,
                            )),
                        ];
                        if activate {
                            tasks.push(Task::done(Message::StartProxy));
                        }
                        Task::batch(tasks)
                    }
                    Err(e) => {
                        self.error_msg = Some(e.to_string());
                        Task::done(Message::ShowToast(e.to_string(), ToastStatus::Error))
                    }
                }
            }
            Message::DeleteProfile(name) => Task::perform(
                async move {
                    let cm = ConfigManager::new().map_err(InfiltratorError::from)?;
                    cm.delete_profile(&name)
                        .await
                        .map_err(InfiltratorError::from)?;
                    Ok(())
                },
                Message::ProfileDeleted,
            ),
            Message::ProfileDeleted(result) => match result {
                Ok(_) => {
                    self.invalidate_rules_dns_views();
                    Task::batch(vec![
                        Task::done(Message::LoadProfiles),
                        Task::done(Message::ShowToast(
                            "Profile deleted".to_string(),
                            ToastStatus::Success,
                        )),
                    ])
                }
                Err(e) => {
                    self.error_msg = Some(e.to_string());
                    Task::done(Message::ShowToast(e.to_string(), ToastStatus::Error))
                }
            },
            Message::BrowseLocalImportFile => Task::perform(
                async {
                    tokio::task::spawn_blocking(|| {
                        rfd::FileDialog::new()
                            .add_filter("YAML", &["yaml", "yml"])
                            .pick_file()
                    })
                    .await
                    .ok()
                    .flatten()
                },
                Message::LocalImportFilePicked,
            ),
            Message::LocalImportFilePicked(path) => {
                if let Some(path) = path {
                    self.local_import_path = path.to_string_lossy().to_string();
                    if self.local_import_name.trim().is_empty()
                        && let Some(stem) = path.file_stem().and_then(|s| s.to_str())
                    {
                        self.local_import_name = stem.to_string();
                    }
                }
                Task::none()
            }
            Message::UpdateLocalImportPath(path) => {
                self.local_import_path = path;
                if self.local_import_name.trim().is_empty()
                    && let Some(stem) = std::path::Path::new(&self.local_import_path)
                        .file_stem()
                        .and_then(|s| s.to_str())
                {
                    self.local_import_name = stem.to_string();
                }
                Task::none()
            }
            Message::UpdateLocalImportName(name) => {
                self.local_import_name = name;
                Task::none()
            }
            Message::UpdateLocalImportActivate(enabled) => {
                self.local_import_activate = enabled;
                Task::none()
            }
            Message::ImportLocalProfile => {
                let path = self.local_import_path.trim().to_string();
                let name = self.local_import_name.trim().to_string();
                let activate = self.local_import_activate;
                if path.is_empty() || name.is_empty() {
                    return Task::done(Message::ShowToast(
                        "Local path and profile name are required".to_string(),
                        ToastStatus::Error,
                    ));
                }

                self.is_importing_local = true;
                Task::perform(
                    async move {
                        let profile_name = infiltrator_core::profiles::sanitize_profile_name(&name)
                            .map_err(|e| InfiltratorError::Config(e.to_string()))?;
                        let content = tokio::fs::read_to_string(&path)
                            .await
                            .map_err(|e| InfiltratorError::Io(e.to_string()))?;
                        infiltrator_core::config::validate_yaml(&content)
                            .map_err(|e| InfiltratorError::Config(e.to_string()))?;

                        let cm = ConfigManager::new().map_err(InfiltratorError::from)?;
                        cm.save(&profile_name, &content)
                            .await
                            .map_err(InfiltratorError::from)?;
                        if activate {
                            cm.set_current(&profile_name)
                                .await
                                .map_err(InfiltratorError::from)?;
                        }
                        Ok(())
                    },
                    Message::LocalProfileImported,
                )
            }
            Message::LocalProfileImported(result) => {
                self.is_importing_local = false;
                match result {
                    Ok(_) => {
                        self.invalidate_rules_dns_views();
                        let activate = self.local_import_activate;
                        self.local_import_path.clear();
                        self.local_import_name.clear();
                        self.local_import_activate = false;
                        let mut tasks = vec![
                            Task::done(Message::LoadProfiles),
                            Task::done(Message::ShowToast(
                                "Local profile imported".to_string(),
                                ToastStatus::Success,
                            )),
                        ];
                        if activate {
                            tasks.push(Task::done(Message::StartProxy));
                        }
                        Task::batch(tasks)
                    }
                    Err(e) => {
                        self.error_msg = Some(e.to_string());
                        Task::done(Message::ShowToast(e.to_string(), ToastStatus::Error))
                    }
                }
            }
            Message::SelectSubscriptionProfile(name) => {
                self.subscription_profile_name = name;
                self.sync_subscription_editor();
                Task::none()
            }
            Message::UpdateSubscriptionUrl(url) => {
                self.subscription_url = url;
                Task::none()
            }
            Message::UpdateSubscriptionAutoUpdate(enabled) => {
                self.subscription_auto_update_enabled = enabled;
                Task::none()
            }
            Message::UpdateSubscriptionInterval(interval) => {
                self.subscription_update_interval_hours = interval;
                Task::none()
            }
            Message::SaveSubscriptionSettings => {
                let profile_name = self.subscription_profile_name.clone();
                let url = self.subscription_url.trim().to_string();
                let auto_update = self.subscription_auto_update_enabled;
                let interval_raw = self.subscription_update_interval_hours.trim().to_string();

                if profile_name.is_empty() {
                    return Task::done(Message::ShowToast(
                        "Please select a profile".to_string(),
                        ToastStatus::Error,
                    ));
                }
                if auto_update && url.is_empty() {
                    return Task::done(Message::ShowToast(
                        "Subscription URL is required when auto update is enabled".to_string(),
                        ToastStatus::Error,
                    ));
                }
                let interval_hours = if auto_update {
                    let normalized = if interval_raw.is_empty() {
                        "24".to_string()
                    } else {
                        interval_raw
                    };
                    match normalized.parse::<u32>() {
                        Ok(v) if v > 0 => Some(v),
                        _ => {
                            return Task::done(Message::ShowToast(
                                "Update interval must be a positive number".to_string(),
                                ToastStatus::Error,
                            ));
                        }
                    }
                } else {
                    None
                };

                self.is_saving_subscription = true;
                Task::perform(
                    async move {
                        let cm = ConfigManager::new().map_err(InfiltratorError::from)?;
                        let mut metadata = cm
                            .get_profile_metadata(&profile_name)
                            .await
                            .map_err(InfiltratorError::from)?;

                        if url.is_empty() {
                            metadata.subscription_url = None;
                            metadata.auto_update_enabled = false;
                            metadata.update_interval_hours = None;
                            metadata.last_updated = None;
                            metadata.next_update = None;
                        } else {
                            metadata.subscription_url = Some(url);
                            metadata.auto_update_enabled = auto_update;
                            metadata.update_interval_hours = interval_hours;
                            metadata.next_update = None;
                        }

                        cm.update_profile_metadata(&profile_name, &metadata)
                            .await
                            .map_err(InfiltratorError::from)?;
                        Ok(())
                    },
                    Message::SubscriptionSettingsSaved,
                )
            }
            Message::SubscriptionSettingsSaved(result) => {
                self.is_saving_subscription = false;
                match result {
                    Ok(_) => Task::batch(vec![
                        Task::done(Message::LoadProfiles),
                        Task::done(Message::ShowToast(
                            "Subscription settings saved".to_string(),
                            ToastStatus::Success,
                        )),
                    ]),
                    Err(e) => {
                        self.error_msg = Some(e.to_string());
                        Task::done(Message::ShowToast(e.to_string(), ToastStatus::Error))
                    }
                }
            }
            Message::UpdateSubscriptionNow => {
                let profile_name = self.subscription_profile_name.clone();
                if profile_name.is_empty() {
                    return Task::done(Message::ShowToast(
                        "Please select a profile".to_string(),
                        ToastStatus::Error,
                    ));
                }
                self.is_updating_subscription_now = true;
                Task::perform(
                    async move {
                        infiltrator_core::profiles::update_profile(&profile_name)
                            .await
                            .map_err(|e| InfiltratorError::Config(e.to_string()))?;
                        Ok(())
                    },
                    Message::SubscriptionUpdatedNow,
                )
            }
            Message::SubscriptionUpdatedNow(result) => {
                self.is_updating_subscription_now = false;
                match result {
                    Ok(_) => Task::batch(vec![
                        Task::done(Message::LoadProfiles),
                        Task::done(Message::ShowToast(
                            "Subscription updated".to_string(),
                            ToastStatus::Success,
                        )),
                    ]),
                    Err(e) => {
                        self.error_msg = Some(e.to_string());
                        Task::done(Message::ShowToast(e.to_string(), ToastStatus::Error))
                    }
                }
            }
            Message::TickSubUpdate => Task::perform(
                async move {
                    let manager = ConfigManager::new().map_err(InfiltratorError::from)?;
                    let profiles = manager
                        .list_profiles()
                        .await
                        .map_err(InfiltratorError::from)?;
                    let now = Utc::now();
                    let mut updated_names = Vec::new();
                    let mut active_updated = false;

                    for profile in profiles {
                        if !profile.auto_update_enabled {
                            continue;
                        }
                        let Some(url) = profile.subscription_url.as_deref() else {
                            continue;
                        };
                        if url.trim().is_empty() {
                            continue;
                        }

                        let due = profile.next_update.is_none_or(|next| next <= now);
                        if !due {
                            continue;
                        }

                        infiltrator_core::profiles::update_profile(&profile.name)
                            .await
                            .map_err(|e| InfiltratorError::Config(e.to_string()))?;
                        if profile.active {
                            active_updated = true;
                        }
                        updated_names.push(profile.name);
                    }

                    Ok((updated_names, active_updated))
                },
                Message::SubscriptionAutoUpdated,
            ),
            Message::SubscriptionAutoUpdated(result) => match result {
                Ok((updated_profiles, active_updated)) => {
                    if updated_profiles.is_empty() {
                        return Task::none();
                    }
                    let mut tasks = vec![
                        Task::done(Message::LoadProfiles),
                        Task::done(Message::ShowToast(
                            format!("Auto-updated: {}", updated_profiles.join(", ")),
                            ToastStatus::Success,
                        )),
                    ];
                    if active_updated {
                        tasks.push(Task::done(Message::StartProxy));
                    }
                    Task::batch(tasks)
                }
                Err(e) => {
                    self.error_msg = Some(e.to_string());
                    Task::done(Message::ShowToast(
                        format!("Auto update failed: {}", e),
                        ToastStatus::Warning,
                    ))
                }
            },
            Message::EditProfile(path) => {
                let p = path.clone();
                Task::perform(
                    async move {
                        let content = tokio::fs::read_to_string(&p)
                            .await
                            .map_err(|e| InfiltratorError::Io(e.to_string()))?;
                        Ok((p, content))
                    },
                    Message::ProfileContentLoaded,
                )
            }
            Message::ProfileContentLoaded(result) => match result {
                Ok((path, content)) => {
                    self.editor_path = Some(path);
                    self.editor_content = text_editor::Content::with_text(&content);
                    Task::done(Message::Navigate(crate::types::Route::Editor))
                }
                Err(e) => {
                    self.error_msg = Some(e.to_string());
                    Task::none()
                }
            },
            Message::EditorAction(action) => {
                self.editor_content.perform(action);
                Task::none()
            }
            Message::SaveProfile => {
                if let Some(path) = self.editor_path.clone() {
                    let content = self.editor_content.text();
                    Task::perform(
                        async move {
                            tokio::fs::write(&path, content)
                                .await
                                .map_err(|e| InfiltratorError::Io(e.to_string()))?;
                            Ok(())
                        },
                        Message::ProfileSaved,
                    )
                } else {
                    Task::none()
                }
            }
            Message::ProfileSaved(result) => match result {
                Ok(_) => {
                    self.invalidate_rules_dns_views();
                    Task::done(Message::ShowToast(
                        "Profile saved".to_string(),
                        ToastStatus::Success,
                    ))
                }
                Err(e) => {
                    self.error_msg = Some(e.to_string());
                    Task::done(Message::ShowToast(e.to_string(), ToastStatus::Error))
                }
            },
            Message::UpdateWebDavUrl(url) => {
                self.webdav_url = url;
                Task::none()
            }
            Message::UpdateWebDavUser(user) => {
                self.webdav_user = user;
                Task::none()
            }
            Message::UpdateWebDavPass(pass) => {
                self.webdav_pass = pass;
                Task::none()
            }
            Message::UpdateWebDavEnabled(enabled) => {
                self.webdav_enabled = enabled;
                Task::none()
            }
            Message::UpdateWebDavSyncInterval(v) => {
                self.webdav_sync_interval_mins = v;
                Task::none()
            }
            Message::UpdateWebDavSyncOnStartup(enabled) => {
                self.webdav_sync_on_startup = enabled;
                Task::none()
            }
            Message::UpdateEditorPathSetting(path) => {
                self.editor_path_setting = path;
                Task::none()
            }
            Message::SaveAppSettings => {
                let interval =
                    if !self.webdav_enabled && self.webdav_sync_interval_mins.trim().is_empty() {
                        60
                    } else {
                        match self.webdav_sync_interval_mins.trim().parse::<u32>() {
                            Ok(v) if v > 0 => v,
                            _ => {
                                return Task::done(Message::ShowToast(
                                    "WebDAV sync interval must be a positive integer".to_string(),
                                    ToastStatus::Error,
                                ));
                            }
                        }
                    };

                self.is_saving_app_settings = true;
                let language = self.lang.clone();
                let theme = if self.theme == iced::Theme::Light {
                    "light".to_string()
                } else {
                    "dark".to_string()
                };
                let editor_path = if self.editor_path_setting.trim().is_empty() {
                    None
                } else {
                    Some(self.editor_path_setting.trim().to_string())
                };
                let webdav = WebDavConfig {
                    enabled: self.webdav_enabled,
                    url: self.webdav_url.clone(),
                    username: self.webdav_user.clone(),
                    password: self.webdav_pass.clone(),
                    sync_interval_mins: interval,
                    sync_on_startup: self.webdav_sync_on_startup,
                };

                Task::perform(
                    async move {
                        let base_dir =
                            mihomo_platform::get_home_dir().map_err(InfiltratorError::from)?;
                        let settings_path = infiltrator_core::settings::settings_path(&base_dir)
                            .map_err(|e| InfiltratorError::Config(e.to_string()))?;
                        let mut settings =
                            infiltrator_core::settings::load_settings(&settings_path)
                                .await
                                .unwrap_or_else(|_| AppSettings::default());
                        settings.language = language;
                        settings.theme = theme;
                        settings.editor_path = editor_path;
                        settings.webdav = webdav;
                        infiltrator_core::settings::save_settings(&settings_path, &settings)
                            .await
                            .map_err(|e| InfiltratorError::Config(e.to_string()))?;
                        Ok(())
                    },
                    Message::AppSettingsSaved,
                )
            }
            Message::AppSettingsSaved(result) => {
                self.is_saving_app_settings = false;
                match result {
                    Ok(_) => {
                        self.editor_path = if self.editor_path_setting.trim().is_empty() {
                            None
                        } else {
                            Some(std::path::PathBuf::from(self.editor_path_setting.trim()))
                        };
                        Task::done(Message::ShowToast(
                            "App settings saved".to_string(),
                            ToastStatus::Success,
                        ))
                    }
                    Err(e) => {
                        self.error_msg = Some(e.to_string());
                        Task::done(Message::ShowToast(e.to_string(), ToastStatus::Error))
                    }
                }
            }
            Message::SyncUpload => {
                let url = self.webdav_url.clone();
                let user = self.webdav_user.clone();
                let pass = self.webdav_pass.clone();
                self.is_syncing = true;
                Task::perform(
                    async move {
                        let client = WebDavClient::new(&url, &user, &pass)
                            .map_err(|e| InfiltratorError::Sync(e.to_string()))?;
                        let cm = ConfigManager::new()
                            .map_err(|e: mihomo_api::MihomoError| InfiltratorError::from(e))?;
                        let profiles = cm
                            .list_profiles()
                            .await
                            .map_err(|e: mihomo_api::MihomoError| InfiltratorError::from(e))?;

                        for profile in profiles {
                            let content = tokio::fs::read_to_string(&profile.path)
                                .await
                                .map_err(|e| InfiltratorError::Io(e.to_string()))?;
                            client
                                .put(&format!("{}.yaml", profile.name), content.as_bytes(), None)
                                .await
                                .map_err(|e| InfiltratorError::Sync(e.to_string()))?;
                        }
                        Ok(())
                    },
                    Message::SyncFinished,
                )
            }
            Message::SyncDownload => {
                let url = self.webdav_url.clone();
                let user = self.webdav_user.clone();
                let pass = self.webdav_pass.clone();
                self.is_syncing = true;
                Task::perform(
                    async move {
                        let client = WebDavClient::new(&url, &user, &pass)
                            .map_err(|e| InfiltratorError::Sync(e.to_string()))?;
                        let files = client
                            .list("")
                            .await
                            .map_err(|e| InfiltratorError::Sync(e.to_string()))?;

                        for file in files {
                            if file.path.ends_with(".yaml") {
                                let content = client
                                    .get(&file.path)
                                    .await
                                    .map_err(|e| InfiltratorError::Sync(e.to_string()))?;
                                let data_dir = mihomo_platform::get_home_dir().map_err(
                                    |e: mihomo_api::MihomoError| InfiltratorError::from(e),
                                )?;
                                let path = data_dir.join("configs").join(file.path);
                                tokio::fs::write(path, content)
                                    .await
                                    .map_err(|e| InfiltratorError::Io(e.to_string()))?;
                            }
                        }
                        Ok(())
                    },
                    Message::SyncFinished,
                )
            }
            Message::SyncFinished(result) => {
                self.is_syncing = false;
                match result {
                    Ok(_) => Task::done(Message::ShowToast(
                        "Sync completed".to_string(),
                        ToastStatus::Success,
                    )),
                    Err(e) => {
                        self.error_msg = Some(e.to_string());
                        Task::done(Message::ShowToast(e.to_string(), ToastStatus::Error))
                    }
                }
            }
            Message::TickWebDavSync => {
                if self.webdav_enabled
                    && !self.webdav_url.is_empty()
                    && !self.webdav_user.is_empty()
                {
                    return Task::done(Message::SyncUpload);
                }
                Task::none()
            }
            _ => Task::none(),
        }
    }
}
