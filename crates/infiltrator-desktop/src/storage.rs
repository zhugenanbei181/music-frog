//! Desktop composition helpers for application storage ports.

use crate::port_conflict::DesktopPortConflict;
use crate::speedtest_history_store::FileSpeedtestHistoryStore;
use infiltrator_core::app_routing_io::FileAppRoutingStore;
use infiltrator_core::doctor_port::MihomoDoctor;
use infiltrator_core::factory_reset::execute;
use infiltrator_core::fake_ip_cache_io::FileFakeIpCache;
use infiltrator_core::history::{list_snapshots, read_snapshot};
use infiltrator_core::profile_reset::FileProfileReset;
use infiltrator_core::profile_store_io::open;
use infiltrator_core::public_ip_io::HttpPublicIpProbe;
use infiltrator_core::settings_io::app_config_manager;
use infiltrator_core::settings_store::for_current_home;
use infiltrator_core::snapshot_io::FileSnapshotStore;
use infiltrator_core::subscription_io::HttpSubscriptionSource;
use infiltrator_core::sync_port::FileWebDavSync;
use infiltrator_core::version_port::MihomoVersionPort;
use infiltrator_core::{host_io, profile_reset};
use infiltrator_domain::profile_options::ProfileOptions;
use infiltrator_domain::snapshots::SnapshotMeta;
use infiltrator_ports::app_routing_store::AppRoutingStore;
use infiltrator_ports::doctor::DoctorPort;
use infiltrator_ports::endpoint::EndpointSource;
use infiltrator_ports::fake_ip_cache::FakeIpCachePort;
use infiltrator_ports::port_conflict::PortConflictPort;
use infiltrator_ports::profile_reset::ProfileResetPort;
use infiltrator_ports::profile_store::ProfileStore;
use infiltrator_ports::public_ip_probe::PublicIpProbe;
use infiltrator_ports::settings_store::SettingsStore;
use infiltrator_ports::snapshot_store::SnapshotStore;
use infiltrator_ports::speedtest_history::SpeedtestHistoryStore;
use infiltrator_ports::subscription_source::SubscriptionSource;
use infiltrator_ports::sync::SyncPort;
use infiltrator_ports::version::VersionPort;
use mihomo_config::endpoint::ProfileEndpointSource;
use mihomo_config::profile_option_store::{load_options, save_options};
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub fn home_dir() -> anyhow::Result<PathBuf> {
    host_io::home_dir()
}

pub async fn profile_store() -> anyhow::Result<Arc<dyn ProfileStore>> {
    open().await
}

pub async fn profile_controller_url() -> anyhow::Result<String> {
    let manager = app_config_manager().await?;
    manager
        .get_external_controller()
        .await
        .map_err(|error| anyhow::anyhow!(error.to_string()))
}

pub async fn settings_store() -> anyhow::Result<Arc<dyn SettingsStore>> {
    let store = for_current_home()?;
    Ok(Arc::new(store))
}

pub async fn endpoint_source() -> anyhow::Result<impl EndpointSource> {
    let manager = app_config_manager().await?;
    Ok(ProfileEndpointSource::new(Arc::new(manager)))
}

pub async fn save_webdav_password(password: &str) -> anyhow::Result<()> {
    host_io::save_webdav_password(password).await
}

pub async fn webdav_password() -> Option<String> {
    let store = settings_store().await.ok()?;
    store.load_hydrated().await.ok()?.webdav.password.into()
}

pub async fn clear_webdav_password() {
    host_io::clear_webdav_password().await;
}

pub fn subscription_source() -> impl SubscriptionSource {
    HttpSubscriptionSource::with_default_clients()
}

pub fn sync() -> anyhow::Result<impl SyncPort> {
    FileWebDavSync::current()
}

pub async fn snapshot_store() -> anyhow::Result<impl SnapshotStore> {
    FileSnapshotStore::current().await
}

pub fn port_conflict() -> anyhow::Result<impl PortConflictPort> {
    Ok(DesktopPortConflict::new(home_dir()?))
}

pub fn version() -> anyhow::Result<impl VersionPort> {
    MihomoVersionPort::current()
}

pub fn public_ip_probe() -> impl PublicIpProbe {
    HttpPublicIpProbe::with_default_client()
}

pub fn doctor() -> anyhow::Result<impl DoctorPort> {
    MihomoDoctor::detect()
}

pub fn profile_reset() -> impl ProfileResetPort {
    FileProfileReset::current()
}

pub fn fake_ip_cache() -> impl FakeIpCachePort {
    FileFakeIpCache::current()
}

pub fn speedtest_history_store() -> anyhow::Result<impl SpeedtestHistoryStore> {
    FileSpeedtestHistoryStore::current()
}

pub fn app_routing_store() -> anyhow::Result<impl AppRoutingStore> {
    FileAppRoutingStore::current()
}

pub async fn reset_profiles_to_default() -> anyhow::Result<()> {
    profile_reset::reset_profiles_to_default().await
}

pub fn factory_reset(home: &Path, configs_dir: Option<&Path>) -> anyhow::Result<Vec<String>> {
    execute(home, configs_dir).map(|report| report.warnings)
}

pub async fn load_profile_options(
    config_dir: &Path,
    profile: &str,
) -> anyhow::Result<ProfileOptions> {
    Ok(load_options(config_dir, profile).await?)
}

pub async fn save_profile_options(
    config_dir: &Path,
    profile: &str,
    options: &ProfileOptions,
) -> anyhow::Result<()> {
    Ok(save_options(config_dir, profile, options).await?)
}

pub async fn list_profile_snapshots(
    config_dir: &Path,
    profile: &str,
) -> anyhow::Result<Vec<SnapshotMeta>> {
    list_snapshots(config_dir, profile)
        .await
        .map_err(|error| anyhow::anyhow!(error.to_string()))
}

pub async fn read_profile_snapshot(path: &Path) -> anyhow::Result<String> {
    read_snapshot(path)
        .await
        .map_err(|error| anyhow::anyhow!(error.to_string()))
}
