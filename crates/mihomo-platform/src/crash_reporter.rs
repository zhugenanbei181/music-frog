use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use regex::Regex;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

static PROCESS_START_TIME: OnceLock<Instant> = OnceLock::new();
type CleanupHookFn = Box<dyn Fn() + Send + Sync + 'static>;
static CLEANUP_HOOKS: Mutex<Vec<(&'static str, CleanupHookFn)>> = Mutex::new(Vec::new());
static CLEANUP_PERFORMED: AtomicBool = AtomicBool::new(false);

/// Returns the uptime of the current process in seconds.
pub fn get_uptime_secs() -> u64 {
    PROCESS_START_TIME
        .get_or_init(Instant::now)
        .elapsed()
        .as_secs()
}

/// Initializes the process start timer.
pub fn init_process_timer() {
    PROCESS_START_TIME.get_or_init(Instant::now);
}

/// Structured crash report capturing system OS, core version, uptime, panic or fatal signal details.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct CrashReport {
    pub timestamp_secs: u64,
    pub os_info: String,
    pub panic_reason: String,
    pub backtrace_summary: Option<String>,
    pub client_version: String,
    #[serde(default)]
    pub core_version: Option<String>,
    #[serde(default)]
    pub uptime_secs: u64,
    #[serde(default)]
    pub fatal_signal: Option<String>,
    #[serde(default)]
    pub sanitized: bool,
}

impl CrashReport {
    /// Computes a unique SHA-256 fingerprint for deduplicating identical crash stacks.
    pub fn fingerprint(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(self.panic_reason.as_bytes());
        if let Some(ref bt) = self.backtrace_summary {
            for line in bt.lines().take(3) {
                hasher.update(line.trim().as_bytes());
            }
        }
        let result = hasher.finalize();
        let mut hex = String::with_capacity(result.len() * 2);
        for byte in result {
            use std::fmt::Write;
            let _ = write!(hex, "{:02x}", byte);
        }
        hex
    }
}

fn default_heartbeat_timeout_secs() -> u64 {
    30
}

/// Sentinel state recording active DNS and system proxy interception.
/// Persisted to disk atomically to ensure crash detection and automatic reversion.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct DnsStateSentinel {
    pub original_dns_servers: Vec<String>,
    pub current_dns_servers: Vec<String>,
    pub system_proxy_original: Option<String>,
    #[serde(default)]
    pub system_proxy_current: Option<String>,
    pub daemon_pid: u32,
    #[serde(default)]
    pub interface_name: Option<String>,
    pub created_at_secs: u64,
    pub heartbeat_secs: u64,
    #[serde(default = "default_heartbeat_timeout_secs")]
    pub heartbeat_timeout_secs: u64,
    pub is_active: bool,
    #[serde(default)]
    pub metadata: std::collections::HashMap<String, String>,
}

impl DnsStateSentinel {
    pub fn new(
        daemon_pid: u32,
        original_dns: Vec<String>,
        current_dns: Vec<String>,
        original_proxy: Option<String>,
    ) -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        Self {
            original_dns_servers: original_dns,
            current_dns_servers: current_dns,
            system_proxy_original: original_proxy,
            system_proxy_current: None,
            daemon_pid,
            interface_name: None,
            created_at_secs: now,
            heartbeat_secs: now,
            heartbeat_timeout_secs: default_heartbeat_timeout_secs(),
            is_active: true,
            metadata: std::collections::HashMap::new(),
        }
    }

    pub fn with_interface(mut self, interface: impl Into<String>) -> Self {
        self.interface_name = Some(interface.into());
        self
    }

    pub fn with_heartbeat_timeout(mut self, timeout_secs: u64) -> Self {
        self.heartbeat_timeout_secs = timeout_secs;
        self
    }

    pub fn with_current_proxy(mut self, current_proxy: Option<String>) -> Self {
        self.system_proxy_current = current_proxy;
        self
    }

    pub fn with_meta(mut self, key: impl Into<String>, val: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), val.into());
        self
    }

    pub fn touch_heartbeat(&mut self) {
        self.heartbeat_secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
    }

    pub fn is_heartbeat_expired(&self, now_secs: u64) -> bool {
        self.is_active
            && self.heartbeat_timeout_secs > 0
            && now_secs.saturating_sub(self.heartbeat_secs) > self.heartbeat_timeout_secs
    }

    pub fn mark_inactive(&mut self) {
        self.is_active = false;
        self.touch_heartbeat();
    }

    pub fn is_stale_or_dead(&self, now_secs: u64) -> bool {
        if !self.is_active {
            return false;
        }
        !DnsCrashWatchdog::is_process_alive(self.daemon_pid)
            || self.is_heartbeat_expired(now_secs)
    }
}

/// Action performed by the DNS Crash Watchdog upon finding an orphaned state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum WatchdogRecoveryAction {
    RestoredDns {
        previous_dns: Vec<String>,
        restored_to: Vec<String>,
        #[serde(default)]
        interface: Option<String>,
    },
    RestoredProxy {
        previous_proxy: Option<String>,
        #[serde(default)]
        restored_to: Option<String>,
    },
    CleanedOrphanedSentinel {
        daemon_pid: u32,
        reason: String,
    },
    KilledZombieProcess {
        pid: u32,
    },
    NoActionRequired,
}

/// Summary of watchdog recovery execution.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WatchdogRecoveryResult {
    pub sentinel: DnsStateSentinel,
    pub actions: Vec<WatchdogRecoveryAction>,
    pub recovered_at_secs: u64,
}

/// Ultra-lightweight standalone watchdog for inspecting orphaned DNS / system proxy state
/// left behind after core crashes, sudden power-loss, or SIGKILL.
pub struct DnsCrashWatchdog;

impl DnsCrashWatchdog {
    pub const SENTINEL_FILE: &'static str = ".dns_state_sentinel.json";

    /// Writes active sentinel to disk atomically.
    pub fn write_sentinel(home_dir: &Path, sentinel: &DnsStateSentinel) -> anyhow::Result<PathBuf> {
        let path = home_dir.join(Self::SENTINEL_FILE);
        let temp_path = home_dir.join(format!("{}.tmp", Self::SENTINEL_FILE));
        let json = serde_json::to_string_pretty(sentinel)?;
        std::fs::write(&temp_path, json)?;
        std::fs::rename(&temp_path, &path)?;
        Ok(path)
    }

    /// Updates heartbeat timestamp on the sentinel file.
    pub fn touch_heartbeat(home_dir: &Path) -> anyhow::Result<()> {
        if let Some(mut sentinel) = Self::read_sentinel(home_dir)? {
            sentinel.touch_heartbeat();
            Self::write_sentinel(home_dir, &sentinel)?;
        }
        Ok(())
    }

    /// Reads existing sentinel if present.
    pub fn read_sentinel(home_dir: &Path) -> anyhow::Result<Option<DnsStateSentinel>> {
        let path = home_dir.join(Self::SENTINEL_FILE);
        if !path.exists() {
            return Ok(None);
        }
        let content = std::fs::read_to_string(&path)?;
        let sentinel = serde_json::from_str::<DnsStateSentinel>(&content)?;
        Ok(Some(sentinel))
    }

    /// Removes sentinel on clean shutdown.
    pub fn remove_sentinel(home_dir: &Path) -> anyhow::Result<()> {
        let path = home_dir.join(Self::SENTINEL_FILE);
        if path.exists() {
            std::fs::remove_file(path)?;
        }
        Ok(())
    }

    /// Checks if a process PID is currently alive on the host.
    pub fn is_process_alive(pid: u32) -> bool {
        if pid == 0 {
            return false;
        }
        #[cfg(unix)]
        {
            unsafe { libc::kill(pid as libc::pid_t, 0) == 0 }
        }
        #[cfg(windows)]
        {
            use windows_sys::Win32::Foundation::CloseHandle;
            use windows_sys::Win32::System::Threading::{OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION};
            unsafe {
                let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
                if !handle.is_null() {
                    CloseHandle(handle);
                    true
                } else {
                    false
                }
            }
        }
        #[cfg(not(any(unix, windows)))]
        {
            false
        }
    }

    /// Checks whether an orphaned DNS state exists (sentinel exists, is active, but daemon PID is dead).
    pub fn check_orphaned_state(home_dir: &Path) -> Option<DnsStateSentinel> {
        let sentinel = Self::read_sentinel(home_dir).ok().flatten()?;
        if sentinel.is_active && !Self::is_process_alive(sentinel.daemon_pid) {
            Some(sentinel)
        } else {
            None
        }
    }

    /// Checks whether a sentinel is stale (dead process OR expired heartbeat while active).
    pub fn check_stale_or_orphaned(home_dir: &Path, now_secs: u64) -> Option<DnsStateSentinel> {
        let sentinel = Self::read_sentinel(home_dir).ok().flatten()?;
        if sentinel.is_stale_or_dead(now_secs) {
            Some(sentinel)
        } else {
            None
        }
    }

    /// Executes recovery if orphaned state is detected, invoking supplied restoration handlers.
    pub fn recover_orphaned_state<F1, F2>(
        home_dir: &Path,
        restore_dns_fn: F1,
        restore_proxy_fn: F2,
    ) -> Option<WatchdogRecoveryResult>
    where
        F1: FnOnce(&[String]) -> anyhow::Result<()>,
        F2: FnOnce(Option<&str>) -> anyhow::Result<()>,
    {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        Self::recover_stale_or_orphaned_state(home_dir, now, restore_dns_fn, restore_proxy_fn)
    }

    /// Executes recovery if stale or orphaned state is detected, invoking supplied restoration handlers.
    pub fn recover_stale_or_orphaned_state<F1, F2>(
        home_dir: &Path,
        now_secs: u64,
        restore_dns_fn: F1,
        restore_proxy_fn: F2,
    ) -> Option<WatchdogRecoveryResult>
    where
        F1: FnOnce(&[String]) -> anyhow::Result<()>,
        F2: FnOnce(Option<&str>) -> anyhow::Result<()>,
    {
        let sentinel = Self::check_stale_or_orphaned(home_dir, now_secs)?;
        let mut actions = Vec::new();

        if !sentinel.original_dns_servers.is_empty()
            && let Ok(()) = restore_dns_fn(&sentinel.original_dns_servers)
        {
            actions.push(WatchdogRecoveryAction::RestoredDns {
                previous_dns: sentinel.current_dns_servers.clone(),
                restored_to: sentinel.original_dns_servers.clone(),
                interface: sentinel.interface_name.clone(),
            });
        }

        if let Ok(()) = restore_proxy_fn(sentinel.system_proxy_original.as_deref()) {
            actions.push(WatchdogRecoveryAction::RestoredProxy {
                previous_proxy: sentinel.system_proxy_current.clone().or_else(|| {
                    if sentinel.system_proxy_original.is_some() {
                        Some("active".to_string())
                    } else {
                        None
                    }
                }),
                restored_to: sentinel.system_proxy_original.clone(),
            });
        }

        actions.push(WatchdogRecoveryAction::CleanedOrphanedSentinel {
            daemon_pid: sentinel.daemon_pid,
            reason: if !Self::is_process_alive(sentinel.daemon_pid) {
                format!("Process PID {} terminated abnormally", sentinel.daemon_pid)
            } else {
                "Heartbeat expired while active".to_string()
            },
        });

        let _ = Self::remove_sentinel(home_dir);

        let recovered_at_secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        Some(WatchdogRecoveryResult {
            sentinel,
            actions,
            recovered_at_secs,
        })
    }
}

/// Standalone watchdog supervisor running independently to monitor DNS & system proxy state,
/// automatically reverting hijacked settings if the main daemon crashes or locks up.
pub struct StandaloneDnsWatchdog {
    pub home_dir: PathBuf,
    pub poll_interval: std::time::Duration,
    pub heartbeat_timeout_secs: u64,
    pub is_running: Arc<AtomicBool>,
}

impl StandaloneDnsWatchdog {
    /// Default watchdog poll interval (2 seconds).
    pub const DEFAULT_POLL_INTERVAL: std::time::Duration = std::time::Duration::from_secs(2);
    /// Default heartbeat expiry threshold (30 seconds).
    pub const DEFAULT_HEARTBEAT_TIMEOUT: u64 = 30;

    /// Creates a new StandaloneDnsWatchdog targeting the specified home directory.
    pub fn new(home_dir: PathBuf) -> Self {
        Self {
            home_dir,
            poll_interval: Self::DEFAULT_POLL_INTERVAL,
            heartbeat_timeout_secs: Self::DEFAULT_HEARTBEAT_TIMEOUT,
            is_running: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Sets custom poll interval.
    pub fn with_poll_interval(mut self, interval: std::time::Duration) -> Self {
        self.poll_interval = interval;
        self
    }

    /// Sets custom heartbeat timeout.
    pub fn with_heartbeat_timeout(mut self, timeout_secs: u64) -> Self {
        self.heartbeat_timeout_secs = timeout_secs;
        self
    }

    /// Performs a single inspection and recovery pass.
    pub fn check_and_recover<F1, F2>(
        &self,
        restore_dns: F1,
        restore_proxy: F2,
    ) -> Option<WatchdogRecoveryResult>
    where
        F1: FnOnce(&[String]) -> anyhow::Result<()>,
        F2: FnOnce(Option<&str>) -> anyhow::Result<()>,
    {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        DnsCrashWatchdog::recover_stale_or_orphaned_state(
            &self.home_dir,
            now,
            restore_dns,
            restore_proxy,
        )
    }

    /// Runs the watchdog polling loop asynchronously until stopped or receiver signaled.
    pub async fn run_loop<F1, F2>(
        &self,
        restore_dns: F1,
        restore_proxy: F2,
        mut stop_rx: tokio::sync::watch::Receiver<bool>,
    ) -> usize
    where
        F1: Fn(&[String]) -> anyhow::Result<()> + Send + Sync + 'static,
        F2: Fn(Option<&str>) -> anyhow::Result<()> + Send + Sync + 'static,
    {
        self.is_running.store(true, Ordering::SeqCst);
        let mut recoveries = 0;
        let mut interval = tokio::time::interval(self.poll_interval);
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        let restore_dns_arc = Arc::new(restore_dns);
        let restore_proxy_arc = Arc::new(restore_proxy);

        loop {
            tokio::select! {
                _ = interval.tick() => {
                    if !self.is_running.load(Ordering::SeqCst) {
                        break;
                    }
                    let now = SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs();

                    let dns_fn = restore_dns_arc.clone();
                    let proxy_fn = restore_proxy_arc.clone();

                    if let Some(res) = DnsCrashWatchdog::recover_stale_or_orphaned_state(
                        &self.home_dir,
                        now,
                        move |dns| dns_fn(dns),
                        move |proxy| proxy_fn(proxy),
                    ) {
                        log::warn!(
                            "[DNS_WATCHDOG] Recovered orphaned DNS state from PID {}: {:?}",
                            res.sentinel.daemon_pid,
                            res.actions
                        );
                        recoveries += 1;
                    }
                }
                _ = stop_rx.changed() => {
                    if *stop_rx.borrow() {
                        break;
                    }
                }
            }
        }

        self.is_running.store(false, Ordering::SeqCst);
        recoveries
    }

    /// Generates platform-specific CLI command strings for restoring DNS servers.
    pub fn generate_platform_dns_restore_commands(
        interface: Option<&str>,
        servers: &[String],
    ) -> Vec<String> {
        let iface = interface.unwrap_or("eth0");
        let mut commands = Vec::new();

        if cfg!(target_os = "linux") {
            if servers.is_empty() {
                commands.push(format!("resolvectl revert {}", iface));
            } else {
                commands.push(format!("resolvectl dns {} {}", iface, servers.join(" ")));
            }
        } else if cfg!(target_os = "macos") {
            let iface_mac = interface.unwrap_or("Wi-Fi");
            if servers.is_empty() {
                commands.push(format!("networksetup -setdnsservers {} Empty", iface_mac));
            } else {
                commands.push(format!(
                    "networksetup -setdnsservers {} {}",
                    iface_mac,
                    servers.join(" ")
                ));
            }
        } else if cfg!(windows) {
            let iface_win = interface.unwrap_or("Ethernet");
            if servers.is_empty() {
                commands.push(format!("netsh interface ip set dns name=\"{}\" source=dhcp", iface_win));
            } else {
                if let Some(primary) = servers.first() {
                    commands.push(format!(
                        "netsh interface ip set dns name=\"{}\" static {} primary",
                        iface_win, primary
                    ));
                }
                for (idx, secondary) in servers.iter().skip(1).enumerate() {
                    commands.push(format!(
                        "netsh interface ip add dns name=\"{}\" {} index={}",
                        iface_win,
                        secondary,
                        idx + 2
                    ));
                }
            }
        } else {
            commands.push(format!("# generic restore for {}: {}", iface, servers.join(",")));
        }

        commands
    }

    /// Stops the running watchdog loop.
    pub fn stop(&self) {
        self.is_running.store(false, Ordering::SeqCst);
    }
}

/// Utility for capturing, sanitizing, serializing, saving, and rotating crash reports.
pub struct CrashReporter;

impl CrashReporter {
    /// Creates a new crash report with client version, panic reason, and optional backtrace.
    pub fn new_report(
        panic_reason: &str,
        client_version: &str,
        backtrace: Option<&str>,
    ) -> CrashReport {
        Self::new_full_report(
            panic_reason,
            client_version,
            Some(client_version),
            get_uptime_secs(),
            None,
            backtrace,
        )
    }

    /// Creates a full crash report with explicit core version, uptime, signal, and backtrace.
    pub fn new_full_report(
        panic_reason: &str,
        client_version: &str,
        core_version: Option<&str>,
        uptime_secs: u64,
        fatal_signal: Option<&str>,
        backtrace: Option<&str>,
    ) -> CrashReport {
        let timestamp_secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let os_info = std::env::consts::OS.to_string();

        CrashReport {
            timestamp_secs,
            os_info,
            panic_reason: panic_reason.to_string(),
            backtrace_summary: backtrace.map(|s| s.to_string()),
            client_version: client_version.to_string(),
            core_version: core_version.map(|s| s.to_string()),
            uptime_secs,
            fatal_signal: fatal_signal.map(|s| s.to_string()),
            sanitized: false,
        }
    }

    /// Creates a report for fatal OS signals (SIGTERM, SIGINT, SIGSEGV, etc.).
    pub fn new_signal_report(
        signal_name: &str,
        client_version: &str,
        core_version: Option<&str>,
    ) -> CrashReport {
        Self::new_full_report(
            &format!("Fatal signal received: {}", signal_name),
            client_version,
            core_version,
            get_uptime_secs(),
            Some(signal_name),
            None,
        )
    }

    /// Sanitizes sensitive information (Bearer tokens, secrets, private keys, JWTs, home paths) from the report.
    pub fn sanitize_report(report: &mut CrashReport) {
        let mut texts = vec![&mut report.panic_reason];
        if let Some(ref mut bt) = report.backtrace_summary {
            texts.push(bt);
        }
        if let Some(ref mut sig) = report.fatal_signal {
            texts.push(sig);
        }

        let home_from_paths = crate::paths::get_home_dir()
            .ok()
            .map(|p| p.to_string_lossy().to_string());
        let home_from_dirs = dirs::home_dir().map(|p| p.to_string_lossy().to_string());

        let token_bearer_re = Regex::new(r"(?i)(bearer\s+)[a-zA-Z0-9_\-\.]+").unwrap();
        let token_param_re = Regex::new(r"(?i)(token[=:]\s*)[^\s&,;\x22\x27]+").unwrap();
        let secret_re = Regex::new(r"(?i)(secret[=:]\s*)[^\s&,;\x22\x27]+").unwrap();
        let password_re = Regex::new(r"(?i)(password[=:]\s*)[^\s&,;\x22\x27]+").unwrap();
        let apikey_re = Regex::new(r"(?i)(api[-_]?key[=:]\s*)[^\s&,;\x22\x27]+").unwrap();
        let url_cred_re = Regex::new(r"(?i)(https?://)([^:]+):([^@]+)@").unwrap();
        let jwt_re = Regex::new(r"eyJ[A-Za-z0-9_\-]{10,}\.[A-Za-z0-9_\-]{10,}\.[A-Za-z0-9_\-]{10,}").unwrap();
        let private_key_re = Regex::new(r"-----BEGIN[ A-Z0-9_\-]+PRIVATE KEY-----[\s\S]*?-----END[ A-Z0-9_\-]+PRIVATE KEY-----").unwrap();

        let unix_home_re = Regex::new(r"/home/[^/\s\x22\x27,:]+").unwrap();
        let mac_home_re = Regex::new(r"/Users/[^/\s\x22\x27,:]+").unwrap();
        let win_home_re = Regex::new(r"(?i)[a-zA-Z]:\\Users\\[^\\\s\x22\x27,:]+").unwrap();

        for text in texts {
            if let Some(ref home) = home_from_paths {
                *text = text.replace(home, "<REDACTED_HOME>");
            }
            if let Some(ref home) = home_from_dirs {
                *text = text.replace(home, "<REDACTED_HOME>");
            }

            *text = unix_home_re
                .replace_all(text, "<REDACTED_HOME>")
                .to_string();
            *text = mac_home_re.replace_all(text, "<REDACTED_HOME>").to_string();
            *text = win_home_re.replace_all(text, "<REDACTED_HOME>").to_string();

            *text = token_bearer_re
                .replace_all(text, "${1}<REDACTED_TOKEN>")
                .to_string();
            *text = token_param_re
                .replace_all(text, "${1}<REDACTED_TOKEN>")
                .to_string();
            *text = secret_re
                .replace_all(text, "${1}<REDACTED_SECRET>")
                .to_string();
            *text = password_re
                .replace_all(text, "${1}<REDACTED_SECRET>")
                .to_string();
            *text = apikey_re
                .replace_all(text, "${1}<REDACTED_SECRET>")
                .to_string();
            *text = url_cred_re
                .replace_all(text, "${1}<REDACTED_USER>:<REDACTED_PASS>@")
                .to_string();
            *text = jwt_re.replace_all(text, "<REDACTED_JWT>").to_string();
            *text = private_key_re.replace_all(text, "<REDACTED_PRIVATE_KEY>").to_string();
        }

        report.sanitized = true;
    }

    /// Serializes a crash report to a formatted JSON string.
    pub fn serialize_report(report: &CrashReport) -> anyhow::Result<String> {
        let json = serde_json::to_string_pretty(report)?;
        Ok(json)
    }

    /// Parses a crash report from a JSON string.
    pub fn parse_report(json_str: &str) -> anyhow::Result<CrashReport> {
        let report = serde_json::from_str(json_str)?;
        Ok(report)
    }

    /// Saves a sanitized crash report dump to `<home>/crash_reports/crash_<timestamp>.json`.
    pub fn save_crash_dump(
        report: &CrashReport,
        base_dir_override: Option<&Path>,
    ) -> anyhow::Result<PathBuf> {
        let mut sanitized_report = report.clone();
        if !sanitized_report.sanitized {
            Self::sanitize_report(&mut sanitized_report);
        }

        let base_dir = match base_dir_override {
            Some(dir) => dir.to_path_buf(),
            None => crate::paths::get_home_dir().or_else(|_| {
                dirs::home_dir()
                    .ok_or_else(|| anyhow::anyhow!("Unable to determine home directory"))
            })?,
        };

        let reports_dir = base_dir.join("crash_reports");
        std::fs::create_dir_all(&reports_dir)?;

        let filename = format!("crash_{}.json", sanitized_report.timestamp_secs);
        let report_file = reports_dir.join(filename);

        let json = Self::serialize_report(&sanitized_report)?;
        std::fs::write(&report_file, json)?;

        // Auto-rotate older dumps keeping latest 20
        let _ = Self::rotate_crash_dumps(&reports_dir, 20);

        Ok(report_file)
    }

    /// Rotates old crash dumps in the designated directory, pruning to keep at most `max_keep` files.
    pub fn rotate_crash_dumps(reports_dir: &Path, max_keep: usize) -> anyhow::Result<usize> {
        if !reports_dir.is_dir() {
            return Ok(0);
        }

        let mut entries = Vec::new();
        for entry in std::fs::read_dir(reports_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_file()
                && path.extension().is_some_and(|ext| ext == "json")
                && path.file_name().is_some_and(|n| n.to_string_lossy().starts_with("crash_"))
                && let Ok(metadata) = entry.metadata()
            {
                let modified = metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH);
                entries.push((modified, path));
            }
        }

        if entries.len() <= max_keep {
            return Ok(0);
        }

        entries.sort_by_key(|(m, _)| *m);

        let remove_count = entries.len() - max_keep;
        let mut removed = 0;

        for (_, path) in entries.iter().take(remove_count) {
            if std::fs::remove_file(path).is_ok() {
                removed += 1;
            }
        }

        Ok(removed)
    }
}

/// Clean exit hook manager: registers termination and panic handlers to ensure
/// system proxy and TUN routes are restored on unexpected panic or exit.
pub struct CleanExitHook;

impl CleanExitHook {
    /// Registers a generic named cleanup action.
    pub fn register_cleanup(name: &'static str, hook: impl Fn() + Send + Sync + 'static) {
        if let Ok(mut hooks) = CLEANUP_HOOKS.lock() {
            hooks.push((name, Box::new(hook)));
        }
    }

    /// Registers a handler to restore system proxy settings on panic or exit.
    pub fn register_proxy_restore(hook: impl Fn() + Send + Sync + 'static) {
        Self::register_cleanup("system_proxy_restore", hook);
    }

    /// Registers a handler to restore TUN routes and network tables on panic or exit.
    pub fn register_tun_route_restore(hook: impl Fn() + Send + Sync + 'static) {
        Self::register_cleanup("tun_routes_restore", hook);
    }

    /// Executes all registered cleanup hooks in sequence, catching any unwinds.
    pub fn run_emergency_cleanup() {
        CLEANUP_PERFORMED.store(true, Ordering::SeqCst);

        let hooks: Vec<(&'static str, CleanupHookFn)> = match CLEANUP_HOOKS.lock() {
            Ok(mut h) => std::mem::take(&mut *h),
            Err(e) => std::mem::take(&mut *e.into_inner()),
        };

        for (name, hook) in hooks {
            let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                hook();
            }));
            if res.is_err() {
                eprintln!(
                    "[CRASH_CLEANUP] Cleanup hook '{}' panicked during emergency exit",
                    name
                );
            }
        }
    }

    /// Returns whether emergency cleanup has been performed.
    pub fn is_cleanup_performed() -> bool {
        CLEANUP_PERFORMED.load(Ordering::SeqCst)
    }

    /// Resets the cleanup registry and state (for test isolation).
    pub fn reset_for_tests() {
        CLEANUP_PERFORMED.store(false, Ordering::SeqCst);
        if let Ok(mut hooks) = CLEANUP_HOOKS.lock() {
            hooks.clear();
        }
    }

    /// Installs a panic hook that automatically records a sanitized crash dump
    /// and runs emergency cleanup (restoring proxy and TUN routes).
    pub fn register_panic_hook(client_version: &str, core_version: Option<&str>) {
        let client_ver = client_version.to_string();
        let core_ver = core_version.map(|s| s.to_string());

        let default_hook = std::panic::take_hook();

        std::panic::set_hook(Box::new(move |panic_info| {
            let panic_message = panic_info.to_string();
            let backtrace = std::backtrace::Backtrace::force_capture().to_string();

            let mut report = CrashReporter::new_full_report(
                &panic_message,
                &client_ver,
                core_ver.as_deref(),
                get_uptime_secs(),
                None,
                Some(&backtrace),
            );

            CrashReporter::sanitize_report(&mut report);
            let _ = CrashReporter::save_crash_dump(&report, None);

            // Execute emergency restoration of system proxy and TUN routes
            CleanExitHook::run_emergency_cleanup();

            default_hook(panic_info);
        }));
    }
}

#[cfg(test)]
#[path = "crash_reporter_test.rs"]
mod tests;
