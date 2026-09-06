//! Explicit host adapter namespace for the Iced desktop product.
//!
//! Page/view/update code calls this module instead of reaching into
//! `infiltrator-desktop` directly. The concrete dependency remains confined
//! to this composition boundary while the public UI model stays expressed in
//! application/contract values.

pub mod desktop {
    pub fn system_proxy_port() -> std::sync::Arc<dyn infiltrator_ports::system_proxy::SystemProxyPort> {
        std::sync::Arc::new(infiltrator_desktop::system_proxy::DesktopSystemProxy::new())
    }

    pub fn read_system_proxy_state() -> anyhow::Result<infiltrator_desktop::proxy::SystemProxyState>
    {
        infiltrator_desktop::proxy::read_system_proxy_state()
    }

    pub fn uwp_loopback_application() -> infiltrator_application::uwp_loopback_application::UwpLoopbackApplication {
        infiltrator_application::uwp_loopback_application::UwpLoopbackApplication::new(
            std::sync::Arc::new(infiltrator_desktop::uwp_loopback_port::DesktopUwpLoopbackPort),
        )
    }

}

pub mod process_enumerator {
    pub type ExtendedProcessInfo = infiltrator_desktop::process_enumerator::ExtendedProcessInfo;
    pub type ProcessCategory = infiltrator_desktop::process_enumerator::ProcessCategory;

    pub fn enumerate_extended_processes() -> anyhow::Result<Vec<ExtendedProcessInfo>> {
        infiltrator_desktop::process_enumerator::enumerate_extended_processes()
    }
}

pub mod clipboard_helper {
    pub type ClipboardContentType = infiltrator_desktop::clipboard_helper::ClipboardContentType;
    pub type ClipboardHelper = infiltrator_desktop::clipboard_helper::ClipboardHelper;
}

pub mod admin_client {
    pub type AdminApiClient = infiltrator_desktop::admin_client::AdminApiClient;
}

pub mod tun_service {
    pub type TunServiceManager = infiltrator_desktop::tun_service::TunServiceManager;
    pub type ServiceModeStatus = infiltrator_desktop::tun_service::ServiceModeStatus;
}

pub mod editor {
    pub async fn open_profile_in_editor(
        editor_path: Option<String>,
        profile_name: &str,
    ) -> anyhow::Result<()> {
        infiltrator_desktop::editor::open_profile_in_editor(editor_path, profile_name).await
    }
}

pub mod boot {
    pub type BootError = infiltrator_desktop::boot::BootError;

    pub async fn bootstrap_host_runtime_from_current_home(
        use_bundled: bool,
        bundled_candidates: &[std::path::PathBuf],
    ) -> anyhow::Result<(
        std::sync::Arc<dyn infiltrator_ports::host_runtime::HostRuntime>,
        bool,
    )> {
        infiltrator_desktop::boot::bootstrap_host_runtime_from_current_home(
            use_bundled,
            bundled_candidates,
        )
        .await
    }
}

pub mod storage {
    use infiltrator_domain::profile_options::ProfileOptions;
    use infiltrator_domain::snapshots::SnapshotMeta;
    use infiltrator_ports::app_routing_store::AppRoutingStore;
    use infiltrator_ports::doctor::DoctorPort;
    use infiltrator_ports::fake_ip_cache::FakeIpCachePort;
    use infiltrator_ports::profile_reset::ProfileResetPort;
    use infiltrator_ports::profile_store::ProfileStore;
    use infiltrator_ports::public_ip_probe::PublicIpProbe;
    use infiltrator_ports::settings_store::SettingsStore;
    use infiltrator_ports::snapshot_store::SnapshotStore;
    use infiltrator_ports::subscription_source::SubscriptionSource;
    use infiltrator_ports::sync::SyncPort;
    use infiltrator_ports::version::VersionPort;
    use std::path::Path;
    use std::sync::Arc;

    pub fn home_dir() -> anyhow::Result<std::path::PathBuf> {
        infiltrator_desktop::storage::home_dir()
    }

    pub async fn profile_store() -> anyhow::Result<Arc<dyn ProfileStore>> {
        infiltrator_desktop::storage::profile_store().await
    }

    pub async fn profile_controller_url() -> anyhow::Result<String> {
        infiltrator_desktop::storage::profile_controller_url().await
    }

    pub async fn settings_store() -> anyhow::Result<Arc<dyn SettingsStore>> {
        infiltrator_desktop::storage::settings_store().await
    }

    pub async fn save_webdav_password(password: &str) -> anyhow::Result<()> {
        infiltrator_desktop::storage::save_webdav_password(password).await
    }

    pub async fn webdav_password() -> Option<String> {
        infiltrator_desktop::storage::webdav_password().await
    }

    pub async fn clear_webdav_password() {
        infiltrator_desktop::storage::clear_webdav_password().await
    }

    pub fn subscription_source() -> impl SubscriptionSource {
        infiltrator_desktop::storage::subscription_source()
    }

    pub fn sync() -> anyhow::Result<impl SyncPort> {
        infiltrator_desktop::storage::sync()
    }

    pub async fn snapshot_store() -> anyhow::Result<impl SnapshotStore> {
        infiltrator_desktop::storage::snapshot_store().await
    }

    pub fn version() -> anyhow::Result<impl VersionPort> {
        infiltrator_desktop::storage::version()
    }

    pub fn port_conflict() -> anyhow::Result<impl infiltrator_ports::port_conflict::PortConflictPort> {
        infiltrator_desktop::storage::port_conflict()
    }

    pub fn public_ip_probe() -> impl PublicIpProbe {
        infiltrator_desktop::storage::public_ip_probe()
    }

    pub fn doctor() -> anyhow::Result<impl DoctorPort> {
        infiltrator_desktop::storage::doctor()
    }

    pub fn profile_reset() -> impl ProfileResetPort {
        infiltrator_desktop::storage::profile_reset()
    }

    pub fn fake_ip_cache() -> impl FakeIpCachePort {
        infiltrator_desktop::storage::fake_ip_cache()
    }

    pub fn app_routing_store() -> anyhow::Result<impl AppRoutingStore> {
        infiltrator_desktop::storage::app_routing_store()
    }

    pub async fn reset_profiles_to_default() -> anyhow::Result<()> {
        infiltrator_desktop::storage::reset_profiles_to_default().await
    }

    pub fn factory_reset(home: &Path, configs_dir: Option<&Path>) -> anyhow::Result<Vec<String>> {
        infiltrator_desktop::storage::factory_reset(home, configs_dir)
    }

    pub async fn load_profile_options(
        config_dir: &Path,
        profile: &str,
    ) -> anyhow::Result<ProfileOptions> {
        infiltrator_desktop::storage::load_profile_options(config_dir, profile).await
    }

    pub async fn save_profile_options(
        config_dir: &Path,
        profile: &str,
        options: &ProfileOptions,
    ) -> anyhow::Result<()> {
        infiltrator_desktop::storage::save_profile_options(config_dir, profile, options).await
    }

    pub async fn list_profile_snapshots(
        config_dir: &Path,
        profile: &str,
    ) -> anyhow::Result<Vec<SnapshotMeta>> {
        infiltrator_desktop::storage::list_profile_snapshots(config_dir, profile).await
    }

    pub async fn read_profile_snapshot(path: &Path) -> anyhow::Result<String> {
        infiltrator_desktop::storage::read_profile_snapshot(path).await
    }
}
