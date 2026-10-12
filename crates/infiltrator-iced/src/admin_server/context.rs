use super::*;
use infiltrator_shared::i18n_interpolator::localize;
use infiltrator_shared::locales::{Lang, Localizer};

impl IcedAdminContext {
    pub fn new(shared: AdminSharedRuntime) -> Self {
        Self { shared }
    }

    async fn load_settings(&self) -> AppSettings {
        load_settings_from_disk().await.unwrap_or_default()
    }

    async fn localized_message(&self, key: &str) -> String {
        let settings = self.load_settings().await;
        Lang(&settings.language).tr(key).into_owned()
    }

    async fn update_settings(&self, apply: impl FnOnce(&mut AppSettings)) -> anyhow::Result<()> {
        let application = settings_application().await?;
        application
            .update(apply)
            .await
            .map_err(|failure| anyhow!(failure.message))
    }
}

#[async_trait::async_trait]
impl AdminApiContext for IcedAdminContext {
    async fn profile_application(&self) -> anyhow::Result<ProfileApplication> {
        let store = profile_store().await?;
        Ok(ProfileApplication::new(store))
    }

    async fn configuration_application(&self) -> anyhow::Result<ConfigurationApplication> {
        Ok(crate::configuration::for_mihomo_profiles(
            self.profile_application().await?,
        ))
    }

    async fn doctor_application(&self) -> anyhow::Result<DoctorApplication> {
        let doctor = doctor()?;
        Ok(DoctorApplication::new(Arc::new(doctor)))
    }

    async fn profile_controller_url(&self) -> anyhow::Result<Option<String>> {
        Ok(profile_controller_url().await.ok())
    }

    async fn profile_reset_application(&self) -> anyhow::Result<ProfileResetApplication> {
        let reset = profile_reset();
        Ok(ProfileResetApplication::new(Arc::new(reset)))
    }

    async fn cache_application(&self) -> anyhow::Result<CacheApplication> {
        let cache = fake_ip_cache();
        Ok(CacheApplication::new(Arc::new(cache)))
    }

    async fn subscription_source(&self) -> anyhow::Result<Arc<dyn SubscriptionSource>> {
        Ok(Arc::new(subscription_source()))
    }

    async fn sync_application(&self) -> anyhow::Result<SyncApplication> {
        let sync = sync()?;
        Ok(SyncApplication::new(Arc::new(sync)))
    }

    async fn version_application(&self) -> anyhow::Result<VersionApplication> {
        application().map_err(|error| anyhow::anyhow!(error.to_string()))
    }

    async fn network_application(&self) -> anyhow::Result<NetworkApplication> {
        Ok(NetworkApplication::new(Arc::new(public_ip_probe())))
    }

    async fn routing_intent_snapshot(
        &self,
    ) -> Result<(Option<CoreScope>, RoutingIntentSnapshot), Failure> {
        let application = self.shared.application().ok_or_else(|| {
            Failure::new(
                ErrorCode::NotReady,
                "Core application is unavailable for routing intent observation",
                true,
            )
        })?;
        let scope = application.snapshot().scope();
        let Some(scope) = scope else {
            return Ok((None, RoutingIntentSnapshot::default()));
        };
        let outcome = application
            .execute_scoped(CoreCommandRequest {
                scope: scope.clone(),
                intent: CommandIntent::GetRoutingIntent,
            })
            .await;
        let snapshot = match outcome.result.into_output()? {
            CommandOutput::RoutingIntentObserved { snapshot } => snapshot,
            _ => {
                return Err(Failure::new(
                    ErrorCode::InvalidState,
                    "Core returned no typed routing intent observation",
                    true,
                ));
            }
        };
        if outcome.scope.as_ref() != Some(&scope) {
            return Err(Failure::new(
                ErrorCode::InvalidState,
                "Core scope changed before the admin routing intent observation was returned",
                true,
            ));
        }
        Ok((outcome.scope, snapshot))
    }

    async fn apply_routing_intent(
        &self,
        scope: CoreScope,
        intent: RoutingIntent,
    ) -> Result<(Option<CoreScope>, RoutingIntentSnapshot), Failure> {
        let application = self.shared.application().ok_or_else(|| {
            Failure::new(
                ErrorCode::NotReady,
                "Core application is unavailable for routing intent control",
                true,
            )
        })?;
        if application.snapshot().scope().as_ref() != Some(&scope) {
            return Err(Failure::new(
                ErrorCode::InvalidState,
                "routing intent request used a stale Core scope",
                false,
            ));
        }
        let expected_scope = scope.clone();

        let outcome = application
            .execute_scoped(CoreCommandRequest {
                scope,
                intent: CommandIntent::SetRoutingIntent {
                    intent: intent.clone(),
                },
            })
            .await;
        let observed = match outcome.result.into_output()? {
            CommandOutput::RoutingIntentApplied { observed } if observed == intent => observed,
            CommandOutput::RoutingIntentApplied { observed } => {
                return Err(Failure::new(
                    ErrorCode::InvalidState,
                    format!("Core observed {observed:?} after requesting {intent:?}"),
                    true,
                ));
            }
            _ => {
                return Err(Failure::new(
                    ErrorCode::InvalidState,
                    "Core returned no routing intent readback",
                    true,
                ));
            }
        };
        if outcome.scope.as_ref() != Some(&expected_scope) {
            return Err(Failure::new(
                ErrorCode::InvalidState,
                "Core scope changed before the admin routing intent readback was returned",
                true,
            ));
        }
        Ok((
            outcome.scope,
            RoutingIntentSnapshot {
                current: Some(observed),
                status: RoutingIntentStatus::Ready,
                failure: None,
            },
        ))
    }

    async fn webdav_password(&self) -> Option<String> {
        webdav_password().await
    }

    async fn set_webdav_password(&self, password: &str) -> anyhow::Result<()> {
        save_webdav_password(password).await
    }

    async fn apply_core_owner_action(&self, action: CoreOwnerAction) -> anyhow::Result<()> {
        let session = self
            .shared
            .product_session()
            .ok_or_else(|| anyhow!("desktop Core owner session is unavailable"))?;
        let desktop_action = match action.clone() {
            CoreOwnerAction::StartCore => DesktopProductSessionAction::StartCore,
            CoreOwnerAction::StopCore => DesktopProductSessionAction::StopCore,
            CoreOwnerAction::RestartCore => DesktopProductSessionAction::RestartCore,
            CoreOwnerAction::SelectProfile(target) => {
                DesktopProductSessionAction::SelectProfile(target)
            }
            CoreOwnerAction::RecomposeSelectedProfile => {
                DesktopProductSessionAction::RecomposeSelectedProfile
            }
            CoreOwnerAction::RetryOwnerSwitch => DesktopProductSessionAction::RetryOwnerSwitch,
        };
        let result = tokio::task::spawn_blocking(move || session.apply(desktop_action)).await;
        let binding = match result {
            Ok(Ok(binding)) => binding,
            Ok(Err(error)) => {
                self.shared.send(AdminHostCommand::ProductSessionChanged);
                return Err(anyhow!(error.to_string()));
            }
            Err(error) => {
                self.shared.send(AdminHostCommand::ProductSessionChanged);
                return Err(anyhow!("desktop Core owner action worker failed: {error}"));
            }
        };
        let proxy_cleanup = match action {
            CoreOwnerAction::StopCore
                if read_system_proxy_state()
                    .map(|state| state.enabled)
                    .unwrap_or(false) =>
            {
                let proxy_port = binding
                    .host_runtime
                    .system_proxy_port()
                    .unwrap_or_else(system_proxy_port);
                SystemProxyApplication::new(proxy_port)
                    .set_enabled(false, None, None)
                    .await
                    .map_err(|failure| anyhow!(failure.message))
            }
            CoreOwnerAction::StartCore
            | CoreOwnerAction::StopCore
            | CoreOwnerAction::RestartCore
            | CoreOwnerAction::SelectProfile(_)
            | CoreOwnerAction::RecomposeSelectedProfile
            | CoreOwnerAction::RetryOwnerSwitch => Ok(()),
        };
        self.shared.send(AdminHostCommand::ProductSessionChanged);
        proxy_cleanup
    }

    async fn set_use_bundled_core(&self, enabled: bool) {
        if let Err(err) = self
            .update_settings(|settings| settings.use_bundled_core = enabled)
            .await
        {
            log::warn!("failed to persist use_bundled_core: {err:#}");
        }
    }

    async fn refresh_core_version_info(&self) {
        self.shared.send(AdminHostCommand::CoreVersionsChanged);
    }

    async fn notify_subscription_update(
        &self,
        profile: String,
        success: bool,
        message: Option<String>,
    ) {
        let status = if success {
            ToastStatus::Success
        } else {
            ToastStatus::Error
        };
        let language = self.load_settings().await.language;
        let text = match message {
            Some(detail) => localize(
                &language,
                "admin_subscription_update_detail",
                &[("profile", profile), ("detail", detail)],
            ),
            None => localize(
                &language,
                "admin_subscription_update",
                &[("profile", profile)],
            ),
        };
        self.shared
            .send(AdminHostCommand::Toast(text.clone(), status));
        // 0.20: mirror the toast as an OS notification — the scheduler fires
        // while the window may be hidden (success Low / failure Critical).
        // `title` is a locale key resolved on the consumer side.
        self.shared.send(AdminHostCommand::SystemNotify {
            title: if success {
                "notify_sub_auto_updated".to_string()
            } else {
                "notify_sub_update_failed".to_string()
            },
            body: text,
            urgency: if success {
                NotifyUrgency::Low
            } else {
                NotifyUrgency::Critical
            },
        });
    }

    async fn editor_path(&self) -> Option<String> {
        self.load_settings().await.editor_path
    }

    async fn set_editor_path(&self, path: Option<String>) {
        if let Err(err) = self
            .update_settings(|settings| settings.editor_path = path)
            .await
        {
            log::warn!("failed to persist editor path: {err:#}");
        }
    }

    async fn pick_editor_path(&self) -> Option<String> {
        let (tx, rx) = oneshot::channel();
        self.shared
            .send(AdminHostCommand::PickEditorPath(Arc::new(tx)));
        // The dialog runs on the main thread; give the user ample time.
        timeout(time::Duration::from_secs(300), rx)
            .await
            .ok()?
            .ok()?
    }

    async fn open_profile_in_editor(&self, profile_name: &str) -> anyhow::Result<()> {
        let editor_path = self.editor_path().await;
        open_profile_in_editor(editor_path, profile_name).await
    }

    async fn get_app_settings(&self) -> AppSettings {
        self.load_settings().await
    }

    async fn save_app_settings(&self, settings: AppSettings) -> anyhow::Result<()> {
        save_settings_to_disk(&settings).await?;
        self.shared
            .event_bus()
            .publish(AdminEvent::new(EVENT_SETTINGS_CHANGED));
        self.shared.send(AdminHostCommand::SettingsSavedExternally);
        Ok(())
    }

    async fn runtime_running(&self) -> bool {
        match self.shared.runtime() {
            Some(runtime) => ManagedRuntime::is_running(runtime.as_ref()).await,
            None => false,
        }
    }

    async fn runtime_controller_url(&self) -> Option<String> {
        self.shared
            .runtime()
            .map(|runtime| runtime.controller_url())
    }

    async fn runtime_gateway(&self) -> anyhow::Result<Arc<dyn RuntimeGateway>> {
        let core_not_running = self.localized_message("core_not_running").await;
        let runtime = self
            .shared
            .runtime()
            .ok_or_else(|| anyhow!(core_not_running))?;
        Ok(runtime)
    }

    async fn outbound_owner(&self) -> anyhow::Result<AdminOutboundOwner> {
        let core_not_running = self.localized_message("core_not_running").await;
        let runtime = self
            .shared
            .runtime()
            .ok_or_else(|| anyhow!(core_not_running))?;
        let port = runtime
            .outbound_port()
            .ok_or_else(|| anyhow!("typed outbound owner is not composed"))?;
        let scope_source = runtime
            .outbound_scope_source()
            .ok_or_else(|| anyhow!("typed Core scope source is not composed"))?;
        Ok(AdminOutboundOwner::new(port, scope_source))
    }

    async fn system_proxy_enabled(&self) -> bool {
        read_system_proxy_state()
            .map(|state| state.enabled)
            .unwrap_or(false)
    }

    async fn set_system_proxy_enabled(&self, enabled: bool) -> anyhow::Result<()> {
        if enabled {
            let settings = self.load_settings().await;
            let language = settings.language;
            let runtime = self
                .shared
                .runtime()
                .ok_or_else(|| anyhow!(Lang(&language).tr("core_not_running").into_owned()))?;
            let endpoint = ManagedRuntime::http_proxy_endpoint(runtime.as_ref())
                .await?
                .ok_or_else(|| {
                    anyhow!(Lang(&language).tr("system_proxy_missing_port").into_owned())
                })?;
            let proxy_port = runtime
                .system_proxy_port()
                .unwrap_or_else(system_proxy_port);
            SystemProxyApplication::new(proxy_port)
                .set_enabled(true, Some(endpoint), None)
                .await
                .map_err(|failure| anyhow!(failure.message))?;
        } else {
            SystemProxyApplication::new(system_proxy_port())
                .set_enabled(false, None, None)
                .await
                .map_err(|failure| anyhow!(failure.message))?;
        }
        Ok(())
    }

    async fn autostart_enabled(&self) -> bool {
        is_autostart_enabled(crate::AUTOSTART_REG_NAME)
    }

    async fn set_autostart_enabled(&self, enabled: bool) -> anyhow::Result<()> {
        Ok(set_autostart_enabled(crate::AUTOSTART_REG_NAME, enabled)?)
    }

    fn supports_system_proxy_control(&self) -> bool {
        cfg!(target_os = "windows")
    }

    fn supports_autostart_control(&self) -> bool {
        cfg!(target_os = "windows")
    }
}
