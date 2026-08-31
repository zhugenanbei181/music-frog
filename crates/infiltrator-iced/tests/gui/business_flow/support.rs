//! Shared support for the headless business-journey tests.
//!
//! Journey pattern used throughout (identical to `options_flow_tests.rs`):
//! 1. drive the real `AppState::update()` state machine with user messages,
//! 2. iced `Task`s are lazy in tests — where a journey needs the async leg,
//!    the exact task body (the same product functions, same order) is run on
//!    the test thread via `block_on`, and its result message is fed back
//!    through `update()` (异步结果回灌),
//! 3. every filesystem-touching journey runs inside a [`TempHome`] (global
//!    `mihomo_platform` home override + mutex) so no test ever touches the
//!    real user directory; remote interactions either fail fast on a closed
//!    localhost port or never run at all.
//!
//! test-intent: behavior

use crate::state::AppState;
use crate::tray::spec::{TrayController, TraySpec};
use crate::types::app::ToastStatus;
use crate::types::message::Message;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

/// Drive one message through the real state machine and report the work
/// units of the (lazily executed) task so journeys can assert "task armed"
/// (`>= 1`) vs `Task::none()` (`== 0`).
pub fn feed(state: &mut AppState, message: Message) -> usize {
    state.update(message).units()
}

/// Run an async task body synchronously (the iced task itself is dropped).
pub fn block_on<F: std::future::Future>(future: F) -> F::Output {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(future)
}

/// Fresh production-shaped state: no tray, no runtime, no fixture data.
pub fn fresh_state() -> AppState {
    let (state, _) = AppState::new();
    state
}

/// Serializes every test that flips the global home override, and provides
/// RAII cleanup. `mihomo_platform::paths::get_home_dir()` (and therefore
/// `crate::configs_dir`, `VersionManager::new`, settings load/save and the
/// factory-reset paths) resolves to this directory for the journey's
/// duration.
static HOME_LOCK: Mutex<()> = Mutex::new(());

pub struct TempHome {
    dir: PathBuf,
    _lock: MutexGuard<'static, ()>,
}

impl TempHome {
    pub fn acquire(tag: &str) -> Self {
        let lock = HOME_LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let dir = std::env::temp_dir().join(format!(
            "iced-business-{tag}-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(dir.join("configs")).unwrap();
        assert!(
            mihomo_platform::paths::set_home_dir_override(dir.clone()),
            "home override must install while HOME_LOCK is held"
        );
        Self { dir, _lock: lock }
    }

    pub fn configs(&self) -> PathBuf {
        self.dir.join("configs")
    }

    /// Seed one profile through the real config manager and make it current.
    pub fn seed_profile(&self, name: &str, yaml: &str) {
        block_on(async {
            let manager = crate::configs_dir::config_manager().await.unwrap();
            manager.save(name, yaml).await.unwrap();
            manager.set_current(name).await.unwrap();
        });
    }
}

impl Drop for TempHome {
    fn drop(&mut self) {
        mihomo_platform::paths::clear_home_dir_override();
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

impl std::ops::Deref for TempHome {
    type Target = Path;
    fn deref(&self) -> &Path {
        &self.dir
    }
}

/// Counting tray controller: records every spec push so journeys can assert
/// on tray refresh behaviour (throttling, refresh_tray no-panic) without a
/// D-Bus session.
#[derive(Clone)]
pub struct FakeTray {
    count: Arc<AtomicUsize>,
}

impl FakeTray {
    /// Install into the shell domain, replacing the default (absent) tray.
    pub fn install(state: &mut AppState) -> Self {
        let fake = Self {
            count: Arc::new(AtomicUsize::new(0)),
        };
        state.shell.tray_controller = Some(Box::new(fake.clone()));
        fake
    }

    pub fn count(&self) -> usize {
        self.count.load(Ordering::SeqCst)
    }
}

impl TrayController for FakeTray {
    fn update_spec(&self, _spec: TraySpec) {
        self.count.fetch_add(1, Ordering::SeqCst);
    }

    fn shutdown(&mut self) {}
}

/// Minimal but valid mihomo document used by the editor/filter/diff journeys.
pub const SAMPLE_PROFILE_YAML: &str = r#"mixed-port: 7890
mode: rule
log-level: info
proxies:
  - name: HK-1
    type: ss
    server: hk.example.com
    port: 8388
    cipher: aes-256-gcm
    password: pw1
  - name: JP-1
    type: ss
    server: jp.example.com
    port: 8388
    cipher: aes-256-gcm
    password: pw2
  - name: US-1
    type: ss
    server: us.example.com
    port: 8388
    cipher: aes-256-gcm
    password: pw3
proxy-groups:
  - name: PROXY
    type: select
    proxies: [HK-1, JP-1, US-1]
rules:
  - MATCH,PROXY
"#;

/// Status of the most recently raised toast (the single toast sink is
/// `Message::ShowToast`).
pub fn last_toast(state: &AppState) -> Option<(String, ToastStatus)> {
    state.shell.toasts.last().cloned()
}

/// Named profile with subscription metadata, for in-memory state domains.
pub fn subscribed_profile(name: &str, active: bool, url: Option<&str>) -> mihomo_config::profile::Profile {
    let mut profile = mihomo_config::profile::Profile::new(
        name.to_string(),
        PathBuf::from(format!("/configs/{name}.yaml")),
        active,
    );
    profile.subscription_url = url.map(str::to_string);
    profile.auto_update_enabled = url.is_some();
    profile.update_interval_hours = url.is_some().then_some(24);
    profile
}
