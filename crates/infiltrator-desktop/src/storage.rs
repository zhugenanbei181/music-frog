//! Desktop composition helpers for application storage ports.

use infiltrator_domain::profile_options::ProfileOptions;
use infiltrator_domain::snapshots::SnapshotMeta;
use infiltrator_ports::profile_store::ProfileStore;
use infiltrator_ports::public_ip_probe::PublicIpProbe;
use infiltrator_ports::settings_store::SettingsStore;
use infiltrator_ports::subscription_source::SubscriptionSource;
use infiltrator_ports::doctor::DoctorPort;
use std::path::Path;
use std::sync::Arc;

pub fn home_dir() -> anyhow::Result<std::path::PathBuf> {
    infiltrator_core::host_io::home_dir()
}

pub async fn profile_store() -> anyhow::Result<Arc<dyn ProfileStore>> {
    infiltrator_core::profile_store_io::open().await
}

pub async fn settings_store() -> anyhow::Result<Arc<dyn SettingsStore>> {
    let store = infiltrator_core::settings_store::for_current_home()?;
    Ok(Arc::new(store))
}

pub async fn save_webdav_password(password: &str) -> anyhow::Result<()> {
    infiltrator_core::host_io::save_webdav_password(password).await
}

pub async fn clear_webdav_password() {
    infiltrator_core::host_io::clear_webdav_password().await;
}

pub fn subscription_source() -> impl SubscriptionSource {
    infiltrator_core::subscription_io::HttpSubscriptionSource::with_default_clients()
}

pub fn public_ip_probe() -> impl PublicIpProbe {
    infiltrator_core::public_ip_io::HttpPublicIpProbe::with_default_client()
}

pub fn doctor() -> anyhow::Result<impl DoctorPort> {
    infiltrator_core::doctor_port::MihomoDoctor::detect()
}

pub async fn reset_profiles_to_default() -> anyhow::Result<()> {
    infiltrator_core::profile_reset::reset_profiles_to_default().await
}

pub fn factory_reset(home: &Path, configs_dir: Option<&Path>) -> anyhow::Result<Vec<String>> {
    infiltrator_core::factory_reset::execute(home, configs_dir).map(|report| report.warnings)
}

pub async fn load_profile_options(
    config_dir: &Path,
    profile: &str,
) -> anyhow::Result<ProfileOptions> {
    infiltrator_core::profile_options_io::load_options(config_dir, profile).await
}

pub async fn save_profile_options(
    config_dir: &Path,
    profile: &str,
    options: &ProfileOptions,
) -> anyhow::Result<()> {
    infiltrator_core::profile_options_io::save_options(config_dir, profile, options).await
}

pub async fn list_profile_snapshots(
    config_dir: &Path,
    profile: &str,
) -> anyhow::Result<Vec<SnapshotMeta>> {
    infiltrator_core::history::list_snapshots(config_dir, profile)
        .await
        .map_err(|error| anyhow::anyhow!(error.to_string()))
}

pub async fn read_profile_snapshot(path: &Path) -> anyhow::Result<String> {
    infiltrator_core::history::read_snapshot(path)
        .await
        .map_err(|error| anyhow::anyhow!(error.to_string()))
}
