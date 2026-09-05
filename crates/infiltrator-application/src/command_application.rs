//! Shared command routing for inbound surfaces.
//!
//! CoreApplication owns lifecycle state and its private worker. This module
//! owns the rest of the application use-case dispatch and is installed by a
//! host composition when that surface wants the full command vocabulary.

use infiltrator_contract::command::CommandIntent;
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_domain::app_routing::{AppRoutingMode, AppRoutingRule};
use infiltrator_domain::proxy::Proxy;
use infiltrator_ports::runtime_gateway::{ManagedRuntime, RuntimeGateway};
use infiltrator_ports::subscription_source::SubscriptionSource;
use std::collections::HashSet;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use crate::doctor_application::DoctorApplication;
use crate::profile_application::ProfileApplication;
use crate::routing_application::RoutingApplication;
use crate::settings_application::SettingsApplication;
use crate::snapshot_application::SnapshotApplication;
use crate::sync_application::SyncApplication;

pub type CommandFuture = Pin<Box<dyn Future<Output = Result<(), Failure>> + Send + 'static>>;

/// Extension point consumed by CoreApplication for commands beyond lifecycle
/// and proxy mode.
pub trait CommandHandler: Send + Sync {
    fn handle(&self, intent: CommandIntent) -> CommandFuture;
}

#[derive(Clone, Default)]
pub struct CommandApplication {
    profile: Option<ProfileApplication>,
    runtime: Option<Arc<dyn RuntimeGateway>>,
    managed_runtime: Option<Arc<dyn ManagedRuntime>>,
    subscription_source: Option<Arc<dyn SubscriptionSource>>,
    doctor: Option<DoctorApplication>,
    routing: Option<RoutingApplication>,
    sync: Option<SyncApplication>,
    settings: Option<SettingsApplication>,
    snapshots: Option<SnapshotApplication>,
}

impl CommandApplication {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_profile(mut self, application: ProfileApplication) -> Self {
        self.profile = Some(application);
        self
    }

    pub fn with_runtime(mut self, runtime: Arc<dyn RuntimeGateway>) -> Self {
        self.runtime = Some(runtime);
        self
    }

    pub fn with_managed_runtime(mut self, runtime: Arc<dyn ManagedRuntime>) -> Self {
        self.managed_runtime = Some(runtime);
        self
    }

    pub fn with_subscription_source(mut self, source: Arc<dyn SubscriptionSource>) -> Self {
        self.subscription_source = Some(source);
        self
    }

    pub fn with_doctor(mut self, application: DoctorApplication) -> Self {
        self.doctor = Some(application);
        self
    }

    pub fn with_routing(mut self, application: RoutingApplication) -> Self {
        self.routing = Some(application);
        self
    }

    pub fn with_sync(mut self, application: SyncApplication) -> Self {
        self.sync = Some(application);
        self
    }

    pub fn with_settings(mut self, application: SettingsApplication) -> Self {
        self.settings = Some(application);
        self
    }

    pub fn with_snapshots(mut self, application: SnapshotApplication) -> Self {
        self.snapshots = Some(application);
        self
    }

    pub async fn execute(&self, intent: CommandIntent) -> Result<(), Failure> {
        match intent {
            CommandIntent::SwitchProfile { profile_id } => {
                let profile = self.profile()?;
                if let Some(runtime) = self.managed_runtime.clone() {
                    profile
                        .activate_profile(Some(runtime), &profile_id)
                        .await
                        .map(|_| ())
                } else {
                    profile.select_profile(&profile_id).await.map(|_| ())
                }
            }
            CommandIntent::SelectProxyNode { group, node } => self
                .runtime()?
                .switch_proxy(&group, &node)
                .await
                .map_err(Failure::from),
            CommandIntent::TestDelay { group } => {
                let runtime = self.runtime()?;
                let proxies = runtime.get_proxies().await.map_err(Failure::from)?;
                let candidates = delay_candidates(&proxies, group.as_deref())?;
                let _ = crate::proxy_application::test_proxy_delays(
                    runtime,
                    candidates,
                    DEFAULT_DELAY_TEST_URL.to_string(),
                    DEFAULT_DELAY_TIMEOUT_MS,
                    DEFAULT_DELAY_CONCURRENCY,
                )
                .await;
                Ok(())
            }
            CommandIntent::UpdateProfile { profile_id } => {
                let profile = self.profile()?;
                let source = self.subscription_source()?;
                profile
                    .update_subscription(source.as_ref(), &profile_id)
                    .await
                    .map(|_| ())
            }
            CommandIntent::DeleteProfile { profile_id } => {
                self.profile()?.delete_profile(&profile_id).await
            }
            CommandIntent::RefreshRuleProviders => {
                let runtime = self.runtime()?;
                let providers = runtime
                    .get_rule_providers()
                    .await
                    .map_err(Failure::from)?;
                for provider in providers {
                    runtime
                        .update_rule_provider(&provider.name)
                        .await
                        .map_err(Failure::from)?;
                }
                Ok(())
            }
            CommandIntent::CloseConnection { id } => self
                .runtime()?
                .close_connection(&id)
                .await
                .map_err(Failure::from),
            CommandIntent::CloseAllConnections => self
                .runtime()?
                .close_all_connections()
                .await
                .map_err(Failure::from),
            CommandIntent::ClearDnsCache => self
                .runtime()?
                .flush_fakeip_cache()
                .await
                .map_err(Failure::from),
            CommandIntent::RunDoctorDiagnostics => self.doctor()?.run(None).await.map(|_| ()),
            CommandIntent::RepairDoctorIssue { check_id } => {
                self.doctor()?.fix(Some(check_id)).await.map(|_| ())
            }
            CommandIntent::RepairAllDoctorIssues => self.doctor()?.fix(None).await.map(|_| ()),
            CommandIntent::ToggleAppRouting { app_id, enabled } => {
                self.routing()?.set_package_enabled(&app_id, enabled)
            }
            CommandIntent::SetAppRoutingMode { mode } => {
                let mode = parse_routing_mode(&mode)?;
                self.routing()?.set_mode(mode)
            }
            CommandIntent::SetAppRule { app_id, rule } => {
                let rule = parse_routing_rule(&rule)?;
                self.routing()?.set_rule(&app_id, rule)
            }
            CommandIntent::SyncNow => {
                let settings = self.settings()?.load_hydrated().await?;
                self.sync()?
                    .sync(settings.webdav, settings.configs_dir)
                    .await
                    .map(|_| ())
            }
            CommandIntent::CreateBackupSnapshot => {
                self.snapshots()?.create_current().await.map(|_| ())
            }
            CommandIntent::RestoreSnapshot { id } => {
                let profile = self.profile()?.current_profile().await?;
                let path = std::path::PathBuf::from(id);
                self.snapshots()?
                    .restore(self.managed_runtime.clone(), &profile, &path)
                    .await
            }
            CommandIntent::UpdateSetting { key, value } => self.update_setting(&key, &value).await,
            CommandIntent::SetProxyMode { mode } => self
                .runtime()?
                .set_proxy_mode(mode)
                .await
                .map_err(Failure::from),
            CommandIntent::StartCore
            | CommandIntent::StopCore
            | CommandIntent::RestartCore
            | CommandIntent::ClearLogs
            | CommandIntent::SetLogLevelFilter { .. }
            | CommandIntent::TestDnsLatency
            | CommandIntent::ToggleTun { .. }
            | CommandIntent::SetSystemProxy { .. }
            | CommandIntent::ToggleIncludeSystemApps { .. }
            | CommandIntent::ResolveConflictKeepLocal
            | CommandIntent::ResolveConflictTakeRemote
            | CommandIntent::CheckUpdates => Err(unsupported()),
        }
    }

    async fn update_setting(&self, key: &str, value: &str) -> Result<(), Failure> {
        let key = key.trim();
        let value = value.trim();
        if !matches!(
            key,
            "language" | "theme" | "notifications_enabled" | "close_to_tray"
        ) {
            return Err(Failure::new(
                ErrorCode::InvalidInput,
                format!("unknown setting {key}"),
                false,
            ));
        }
        let parsed_bool = match key {
            "notifications_enabled" | "close_to_tray" => Some(value.parse::<bool>().map_err(
                |_| Failure::new(ErrorCode::InvalidInput, format!("invalid boolean {value}"), false),
            )?),
            _ => None,
        };
        self.settings()?
            .update(|settings| match key {
                "language" => settings.language = value.to_string(),
                "theme" => settings.theme = value.to_string(),
                "notifications_enabled" => {
                    settings.notifications_enabled = parsed_bool.unwrap_or(false)
                }
                "close_to_tray" => settings.close_to_tray = parsed_bool.unwrap_or(false),
                _ => {}
            })
            .await
    }

    fn profile(&self) -> Result<ProfileApplication, Failure> {
        self.profile.clone().ok_or_else(|| missing("profile application"))
    }

    fn runtime(&self) -> Result<Arc<dyn RuntimeGateway>, Failure> {
        self.runtime
            .clone()
            .ok_or_else(|| missing("runtime gateway"))
    }

    fn subscription_source(&self) -> Result<Arc<dyn SubscriptionSource>, Failure> {
        self.subscription_source
            .clone()
            .ok_or_else(|| missing("subscription source"))
    }

    fn doctor(&self) -> Result<DoctorApplication, Failure> {
        self.doctor.clone().ok_or_else(|| missing("doctor application"))
    }

    fn routing(&self) -> Result<RoutingApplication, Failure> {
        self.routing.clone().ok_or_else(|| missing("routing application"))
    }

    fn sync(&self) -> Result<SyncApplication, Failure> {
        self.sync.clone().ok_or_else(|| missing("sync application"))
    }

    fn settings(&self) -> Result<SettingsApplication, Failure> {
        self.settings.clone().ok_or_else(|| missing("settings application"))
    }

    fn snapshots(&self) -> Result<SnapshotApplication, Failure> {
        self.snapshots
            .clone()
            .ok_or_else(|| missing("snapshot application"))
    }
}

impl CommandHandler for CommandApplication {
    fn handle(&self, intent: CommandIntent) -> CommandFuture {
        let application = self.clone();
        Box::pin(async move { application.execute(intent).await })
    }
}

const DEFAULT_DELAY_TEST_URL: &str = "http://www.gstatic.com/generate_204";
const DEFAULT_DELAY_TIMEOUT_MS: u32 = 5000;
const DEFAULT_DELAY_CONCURRENCY: usize = 30;

fn delay_candidates(
    proxies: &std::collections::HashMap<String, Proxy>,
    group: Option<&str>,
) -> Result<Vec<String>, Failure> {
    let candidates = match group {
        Some(group) => match proxies.get(group) {
            Some(Proxy::Selector(value))
            | Some(Proxy::URLTest(value))
            | Some(Proxy::Fallback(value))
            | Some(Proxy::LoadBalance(value)) => value.all.clone(),
            Some(_) => {
                return Err(Failure::new(
                    ErrorCode::InvalidInput,
                    format!("{group} is not a proxy group"),
                    false,
                ));
            }
            None => {
                return Err(Failure::new(
                    ErrorCode::InvalidInput,
                    format!("proxy group {group} was not found"),
                    false,
                ));
            }
        },
        None => proxies
            .values()
            .filter(|proxy| {
                !matches!(
                    proxy,
                    Proxy::Selector(_)
                        | Proxy::URLTest(_)
                        | Proxy::Fallback(_)
                        | Proxy::LoadBalance(_)
                        | Proxy::Unknown
                )
            })
            .map(|proxy| proxy.name().to_string())
            .filter(|name| !name.is_empty())
            .collect(),
    };
    let mut unique = HashSet::new();
    Ok(candidates
        .into_iter()
        .filter(|candidate| unique.insert(candidate.clone()))
        .collect())
}

fn parse_routing_mode(value: &str) -> Result<AppRoutingMode, Failure> {
    match value.trim().to_ascii_lowercase().as_str() {
        "proxy_all" | "global" => Ok(AppRoutingMode::ProxyAll),
        "proxy_selected" | "whitelist" => Ok(AppRoutingMode::ProxySelected),
        "bypass_selected" | "blacklist" => Ok(AppRoutingMode::BypassSelected),
        _ => Err(Failure::new(
            ErrorCode::InvalidInput,
            format!("unknown app routing mode {value}"),
            false,
        )),
    }
}

fn parse_routing_rule(value: &str) -> Result<AppRoutingRule, Failure> {
    match value.trim().to_ascii_lowercase().as_str() {
        "proxy" => Ok(AppRoutingRule::Proxy),
        "direct" => Ok(AppRoutingRule::Direct),
        "block" => Ok(AppRoutingRule::Block),
        _ => Err(Failure::new(
            ErrorCode::InvalidInput,
            format!("unknown app routing rule {value}"),
            false,
        )),
    }
}

fn missing(capability: &str) -> Failure {
    Failure::new(
        ErrorCode::NotReady,
        format!("{capability} is not configured for this host"),
        false,
    )
}

fn unsupported() -> Failure {
    Failure::unsupported("command has no host port in this composition")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_android_and_desktop_routing_vocabulary() {
        assert_eq!(parse_routing_mode("global"), Ok(AppRoutingMode::ProxyAll));
        assert_eq!(
            parse_routing_mode("proxy_selected"),
            Ok(AppRoutingMode::ProxySelected)
        );
        assert_eq!(parse_routing_rule("block"), Ok(AppRoutingRule::Block));
        assert!(parse_routing_rule("drop").is_err());
    }
}
