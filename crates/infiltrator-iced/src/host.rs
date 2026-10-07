//! Explicit host adapter namespace for the Iced desktop product.
//!
//! Page/view/update code calls this module instead of reaching into
//! `infiltrator-desktop` directly. The concrete dependency remains confined
//! to this composition boundary while the public UI model stays expressed in
//! application/contract values.

pub mod desktop {
    use infiltrator_application::uwp_loopback_application::UwpLoopbackApplication;
    use infiltrator_desktop::proxy;
    use infiltrator_desktop::proxy::SystemProxyState;
    use infiltrator_desktop::system_proxy::DesktopSystemProxy;
    use infiltrator_desktop::uwp_loopback_port::DesktopUwpLoopbackPort;
    use infiltrator_ports::system_proxy::SystemProxyPort;
    use std::sync::Arc;
    pub fn system_proxy_port() -> Arc<dyn SystemProxyPort> {
        Arc::new(DesktopSystemProxy::new())
    }

    pub fn read_system_proxy_state() -> anyhow::Result<SystemProxyState> {
        proxy::read_system_proxy_state()
    }

    pub fn uwp_loopback_application() -> UwpLoopbackApplication {
        UwpLoopbackApplication::new(Arc::new(DesktopUwpLoopbackPort))
    }
}

pub mod mini_hud {
    use infiltrator_desktop::mini_hud_window::DesktopMiniHudWindow;
    use infiltrator_ports::mini_hud_window::{MiniHudWindowHandle, MiniHudWindowPort};
    use std::sync::Arc;
    /// The desktop host's floating-window capability adapter. It is a process
    /// singleton bound to this surface's window handle via
    /// [`bind_window_handle`]; with no live handle it answers typed
    /// unsupported (never a fake "Applied").
    pub fn window_port() -> Arc<dyn MiniHudWindowPort> {
        Arc::new(DesktopMiniHudWindow::shared())
    }

    /// Register the surface-owned window handle with the desktop host adapter.
    pub fn bind_window_handle(handle: Arc<dyn MiniHudWindowHandle>) {
        DesktopMiniHudWindow::shared().bind(handle);
    }
}

pub mod process_enumerator {
    use infiltrator_desktop::process_enumerator;
    use infiltrator_desktop::process_enumerator::ExtendedProcessInfo;

    pub fn enumerate_extended_processes() -> anyhow::Result<Vec<ExtendedProcessInfo>> {
        process_enumerator::enumerate_extended_processes()
    }
}

pub mod editor {
    use infiltrator_desktop::editor;
    pub async fn open_profile_in_editor(
        editor_path: Option<String>,
        profile_name: &str,
    ) -> anyhow::Result<()> {
        editor::open_profile_in_editor(editor_path, profile_name).await
    }
}

pub mod boot {
    use infiltrator_desktop::boot;
    use infiltrator_ports::host_runtime::HostRuntime;
    use std::path::PathBuf;
    use std::sync::Arc;

    pub async fn bootstrap_host_runtime_from_current_home(
        use_bundled: bool,
        bundled_candidates: &[PathBuf],
    ) -> anyhow::Result<(Arc<dyn HostRuntime>, bool)> {
        boot::bootstrap_host_runtime_from_current_home(use_bundled, bundled_candidates).await
    }
}

/// DUAL-07-05: executor-neutral delay seam the shared subscription refresh
/// uses for exponential backoff. The Iced worker already runs on Tokio, so the
/// composition root's runtime is reused instead of Iced importing an executor
/// into the application layer.
pub mod runtime {
    use infiltrator_desktop::subscription_notification_port::DesktopSubscriptionNotificationPort;
    use infiltrator_ports::application_runtime::ApplicationRuntime;
    use infiltrator_ports::subscription_notification::SubscriptionNotificationPort;
    use std::sync::{Arc, OnceLock};
    pub fn application_runtime() -> Arc<dyn ApplicationRuntime> {
        static RUNTIME: OnceLock<Arc<dyn ApplicationRuntime>> = OnceLock::new();
        RUNTIME
            .get_or_init(|| {
                infiltrator_composition::tokio_application_runtime()
                    .expect("Tokio application runtime must be constructible")
            })
            .clone()
    }

    /// DUAL-07-10: the desktop system-notification adapter the shared
    /// subscription refresh emits through.
    pub fn subscription_notifier() -> Arc<dyn SubscriptionNotificationPort> {
        Arc::new(DesktopSubscriptionNotificationPort)
    }
}

pub mod storage {
    use infiltrator_desktop::storage;
    use infiltrator_desktop::subscription_import_port::DesktopSubscriptionImportPort;
    use infiltrator_domain::profile_options::ProfileOptions;
    use infiltrator_domain::snapshots::SnapshotMeta;
    use infiltrator_ports::app_routing_store::AppRoutingStore;
    use infiltrator_ports::doctor::DoctorPort;
    use infiltrator_ports::fake_ip_cache::FakeIpCachePort;
    use infiltrator_ports::port_conflict::PortConflictPort;
    use infiltrator_ports::profile_reset::ProfileResetPort;
    use infiltrator_ports::profile_store::ProfileStore;
    use infiltrator_ports::public_ip_probe::PublicIpProbe;
    use infiltrator_ports::settings_store::SettingsStore;
    use infiltrator_ports::snapshot_store::SnapshotStore;
    use infiltrator_ports::subscription_import::SubscriptionImportPort;
    use infiltrator_ports::subscription_source::SubscriptionSource;
    use infiltrator_ports::sync::SyncPort;
    use infiltrator_ports::version::VersionPort;
    use std::path::{Path, PathBuf};
    use std::sync::Arc;

    pub fn home_dir() -> anyhow::Result<PathBuf> {
        storage::home_dir()
    }

    pub async fn profile_store() -> anyhow::Result<Arc<dyn ProfileStore>> {
        storage::profile_store().await
    }

    pub async fn profile_controller_url() -> anyhow::Result<String> {
        storage::profile_controller_url().await
    }

    pub async fn settings_store() -> anyhow::Result<Arc<dyn SettingsStore>> {
        storage::settings_store().await
    }

    pub async fn save_webdav_password(password: &str) -> anyhow::Result<()> {
        storage::save_webdav_password(password).await
    }

    pub async fn webdav_password() -> Option<String> {
        storage::webdav_password().await
    }

    pub async fn clear_webdav_password() {
        storage::clear_webdav_password().await
    }

    pub fn subscription_source() -> impl SubscriptionSource {
        storage::subscription_source()
    }

    /// DUAL-07-01: the desktop host's local-file / clipboard import port.
    pub fn subscription_import_port() -> Arc<dyn SubscriptionImportPort> {
        Arc::new(DesktopSubscriptionImportPort)
    }

    pub fn sync() -> anyhow::Result<impl SyncPort> {
        storage::sync()
    }

    pub async fn snapshot_store() -> anyhow::Result<impl SnapshotStore> {
        storage::snapshot_store().await
    }

    pub fn version() -> anyhow::Result<impl VersionPort> {
        storage::version()
    }

    pub fn port_conflict() -> anyhow::Result<impl PortConflictPort> {
        storage::port_conflict()
    }

    pub fn public_ip_probe() -> impl PublicIpProbe {
        storage::public_ip_probe()
    }

    pub fn doctor() -> anyhow::Result<impl DoctorPort> {
        storage::doctor()
    }

    pub fn profile_reset() -> impl ProfileResetPort {
        storage::profile_reset()
    }

    pub fn fake_ip_cache() -> impl FakeIpCachePort {
        storage::fake_ip_cache()
    }

    pub fn app_routing_store() -> anyhow::Result<impl AppRoutingStore> {
        storage::app_routing_store()
    }

    pub async fn reset_profiles_to_default() -> anyhow::Result<()> {
        storage::reset_profiles_to_default().await
    }

    pub fn factory_reset(home: &Path, configs_dir: Option<&Path>) -> anyhow::Result<Vec<String>> {
        storage::factory_reset(home, configs_dir)
    }

    pub async fn load_profile_options(
        config_dir: &Path,
        profile: &str,
    ) -> anyhow::Result<ProfileOptions> {
        storage::load_profile_options(config_dir, profile).await
    }

    pub async fn save_profile_options(
        config_dir: &Path,
        profile: &str,
        options: &ProfileOptions,
    ) -> anyhow::Result<()> {
        storage::save_profile_options(config_dir, profile, options).await
    }

    pub async fn list_profile_snapshots(
        config_dir: &Path,
        profile: &str,
    ) -> anyhow::Result<Vec<SnapshotMeta>> {
        storage::list_profile_snapshots(config_dir, profile).await
    }

    pub async fn read_profile_snapshot(path: &Path) -> anyhow::Result<String> {
        storage::read_profile_snapshot(path).await
    }
}
