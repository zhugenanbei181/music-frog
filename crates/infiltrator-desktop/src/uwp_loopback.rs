//! Windows UWP AppContainer loopback exemption utility.
//!
//! Provides discovery of installed UWP/AppContainer packages via Windows Registry
//! (or mock backend for testing/cross-platform) and management of loopback isolation
//! exemptions using `CheckNetIsolation.exe`.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::sync::{Arc, OnceLock, RwLock};
use std::time::{SystemTime, UNIX_EPOCH};

/// Preset selection of common UWP / AppContainer applications.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UwpPreset {
    /// All detected UWP packages
    All,
    /// WSLg, Windows Terminal, and Linux GUI integrations
    WslAndTerminal,
    /// Xbox App, Gaming Services, Minecraft, and Game Bar
    XboxAndGaming,
    /// Microsoft Edge UWP, Spotify, Netflix, and Media Players
    MediaAndBrowsers,
    /// Developer Tools, VSCode Remote, PowerToys
    DeveloperTools,
}

impl fmt::Display for UwpPreset {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::All => write!(f, "All UWP Applications"),
            Self::WslAndTerminal => write!(f, "WSL & Windows Terminal"),
            Self::XboxAndGaming => write!(f, "Xbox & Gaming Services"),
            Self::MediaAndBrowsers => write!(f, "Media Players & Browsers"),
            Self::DeveloperTools => write!(f, "Developer Tools & PowerToys"),
        }
    }
}

/// Represents an installed UWP / AppContainer application package.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppContainerPackage {
    pub display_name: String,
    pub app_name: String,
    pub package_family_name: String,
    pub sid: String,
    pub loopback_exempt: bool,
}

/// Health and status snapshot of Windows AppContainer loopback isolation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UwpIsolationHealth {
    pub total_packages: usize,
    pub exempt_packages: usize,
    pub is_supported: bool,
}

/// Persistent snapshot of loopback exemptions for state rollback and auditing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UwpExemptionSnapshot {
    pub timestamp_epoch_secs: u64,
    pub description: String,
    pub exempt_sids: Vec<String>,
}

impl UwpExemptionSnapshot {
    /// Creates a snapshot from currently exempt SIDs.
    pub fn new(exempt_sids: Vec<String>, description: impl Into<String>) -> Self {
        let timestamp_epoch_secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        Self {
            timestamp_epoch_secs,
            description: description.into(),
            exempt_sids,
        }
    }
}

/// Legacy representation for backward compatibility.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UwpAppInfo {
    pub sid: String,
    pub name: String,
    pub display_name: String,
    pub is_exempt: bool,
}

impl From<AppContainerPackage> for UwpAppInfo {
    fn from(pkg: AppContainerPackage) -> Self {
        Self {
            sid: pkg.sid,
            name: pkg.app_name,
            display_name: pkg.display_name,
            is_exempt: pkg.loopback_exempt,
        }
    }
}

impl From<UwpAppInfo> for AppContainerPackage {
    fn from(info: UwpAppInfo) -> Self {
        Self {
            sid: info.sid.clone(),
            app_name: info.name.clone(),
            package_family_name: info.name,
            display_name: info.display_name,
            loopback_exempt: info.is_exempt,
        }
    }
}

/// Abstraction for AppContainer scanning and loopback exemption mutation.
pub trait AppContainerBackend: Send + Sync {
    /// Scans and returns all registered AppContainer packages.
    fn scan_containers(&self) -> Result<Vec<AppContainerPackage>>;

    /// Grants or revokes loopback exemption for a single SID.
    fn set_exempt(&self, sid: &str, exempt: bool) -> Result<()>;

    /// Grants or revokes loopback exemption for a batch of SIDs.
    fn batch_set_exempt(&self, sids: &[String], exempt: bool) -> Result<()>;

    /// Exempts all discovered AppContainers from loopback isolation.
    fn exempt_all(&self) -> Result<()>;

    /// Clears all loopback exemptions.
    fn clear_all(&self) -> Result<()>;

    /// Applies a curated preset of exemptions.
    fn apply_preset(&self, preset: UwpPreset) -> Result<Vec<String>> {
        let packages = self.scan_containers()?;
        let target_sids: Vec<String> = packages
            .into_iter()
            .filter(|pkg| matches_uwp_preset(pkg, preset))
            .map(|pkg| pkg.sid)
            .collect();

        self.batch_set_exempt(&target_sids, true)?;
        Ok(target_sids)
    }

    /// Captures a snapshot of currently exempt SIDs.
    fn capture_snapshot(&self, description: &str) -> Result<UwpExemptionSnapshot> {
        let packages = self.scan_containers()?;
        let exempt_sids: Vec<String> = packages
            .into_iter()
            .filter(|p| p.loopback_exempt)
            .map(|p| p.sid)
            .collect();
        Ok(UwpExemptionSnapshot::new(exempt_sids, description))
    }

    /// Restores loopback exemptions from a snapshot.
    fn restore_snapshot(&self, snapshot: &UwpExemptionSnapshot) -> Result<()> {
        self.clear_all()?;
        self.batch_set_exempt(&snapshot.exempt_sids, true)?;
        Ok(())
    }
}

/// Evaluates if a package matches the given preset category.
pub fn matches_uwp_preset(pkg: &AppContainerPackage, preset: UwpPreset) -> bool {
    let lower_name = pkg.app_name.to_ascii_lowercase();
    let lower_family = pkg.package_family_name.to_ascii_lowercase();
    let lower_display = pkg.display_name.to_ascii_lowercase();

    let matches_any = |keywords: &[&str]| -> bool {
        keywords.iter().any(|&k| {
            lower_name.contains(k) || lower_family.contains(k) || lower_display.contains(k)
        })
    };

    match preset {
        UwpPreset::All => true,
        UwpPreset::WslAndTerminal => matches_any(&[
            "terminal",
            "wsl",
            "wslg",
            "ubuntu",
            "debian",
            "arch",
            "opensuse",
            "kali",
            "powershell",
        ]),
        UwpPreset::XboxAndGaming => matches_any(&[
            "xbox",
            "gaming",
            "gamebar",
            "minecraft",
            "gamingservices",
            "store",
            "windowsstore",
        ]),
        UwpPreset::MediaAndBrowsers => matches_any(&[
            "edge", "spotify", "netflix", "music", "video", "zune", "photo", "browser",
        ]),
        UwpPreset::DeveloperTools => matches_any(&[
            "powertoys",
            "vscode",
            "visualstudio",
            "git",
            "docker",
            "insomnia",
            "postman",
        ]),
    }
}

/// In-memory mock backend for testing and simulation.
#[derive(Debug, Default, Clone)]
pub struct MockAppContainerBackend {
    packages: Arc<RwLock<Vec<AppContainerPackage>>>,
}

impl MockAppContainerBackend {
    /// Creates an empty mock backend.
    pub fn new() -> Self {
        Self {
            packages: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Creates a mock backend initialized with a predefined set of packages.
    pub fn with_packages(packages: Vec<AppContainerPackage>) -> Self {
        Self {
            packages: Arc::new(RwLock::new(packages)),
        }
    }

    /// Adds a package to the mock registry.
    pub fn add_package(&self, package: AppContainerPackage) {
        if let Ok(mut pkgs) = self.packages.write() {
            pkgs.push(package);
        }
    }

    /// Returns the current list of packages in the mock registry.
    pub fn get_packages(&self) -> Vec<AppContainerPackage> {
        self.packages.read().map(|p| p.clone()).unwrap_or_default()
    }
}

impl AppContainerBackend for MockAppContainerBackend {
    fn scan_containers(&self) -> Result<Vec<AppContainerPackage>> {
        Ok(self.get_packages())
    }

    fn set_exempt(&self, sid: &str, exempt: bool) -> Result<()> {
        let mut pkgs = self
            .packages
            .write()
            .map_err(|e| anyhow::anyhow!("Lock poisoned: {e}"))?;
        let mut found = false;
        for pkg in pkgs.iter_mut() {
            if pkg.sid == sid {
                pkg.loopback_exempt = exempt;
                found = true;
            }
        }
        if !found {
            pkgs.push(AppContainerPackage {
                display_name: sid.to_string(),
                app_name: sid.to_string(),
                package_family_name: sid.to_string(),
                sid: sid.to_string(),
                loopback_exempt: exempt,
            });
        }
        Ok(())
    }

    fn batch_set_exempt(&self, sids: &[String], exempt: bool) -> Result<()> {
        for sid in sids {
            self.set_exempt(sid, exempt)?;
        }
        Ok(())
    }

    fn exempt_all(&self) -> Result<()> {
        let mut pkgs = self
            .packages
            .write()
            .map_err(|e| anyhow::anyhow!("Lock poisoned: {e}"))?;
        for pkg in pkgs.iter_mut() {
            pkg.loopback_exempt = true;
        }
        Ok(())
    }

    fn clear_all(&self) -> Result<()> {
        let mut pkgs = self
            .packages
            .write()
            .map_err(|e| anyhow::anyhow!("Lock poisoned: {e}"))?;
        for pkg in pkgs.iter_mut() {
            pkg.loopback_exempt = false;
        }
        Ok(())
    }
}

/// Native backend interacting with Windows Registry and CheckNetIsolation.exe on Windows,
/// and returning graceful empty fallbacks on non-Windows platforms.
#[derive(Debug, Default, Clone)]
pub struct NativeAppContainerBackend;

#[cfg(windows)]
impl NativeAppContainerBackend {
    fn query_exempt_sids(&self) -> Result<HashSet<String>> {
        let output = std::process::Command::new("CheckNetIsolation.exe")
            .args(["LoopbackExempt", "-s"])
            .output()
            .context("Failed to execute CheckNetIsolation.exe -s")?;

        if !output.status.success() {
            return Ok(HashSet::new());
        }

        let text = String::from_utf8_lossy(&output.stdout);
        let packages = UwpLoopbackManager::parse_loopback_status_output(&text);
        Ok(packages.into_iter().map(|p| p.sid).collect())
    }

    fn scan_registry(&self, exempt_sids: &HashSet<String>) -> Result<Vec<AppContainerPackage>> {
        use winreg::RegKey;
        use winreg::enums::{HKEY_CURRENT_USER, KEY_READ};

        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let mappings_path = r"Software\Classes\Local Settings\Software\Microsoft\Windows\CurrentVersion\AppContainer\Mappings";
        let mappings = match hkcu.open_subkey_with_flags(mappings_path, KEY_READ) {
            Ok(k) => k,
            Err(e) => {
                log::debug!("AppContainer mappings key not accessible: {e}");
                return Ok(Vec::new());
            }
        };

        let mut packages = Vec::new();
        for key_name in mappings.enum_keys().filter_map(|r| r.ok()) {
            if let Ok(subkey) = mappings.open_subkey_with_flags(&key_name, KEY_READ) {
                let sid = key_name.clone();
                let moniker: String = subkey.get_value("Moniker").unwrap_or_default();
                let display_name: String = subkey.get_value("DisplayName").unwrap_or_default();
                let family_name: String = subkey
                    .get_value("ParentMoniker")
                    .unwrap_or_else(|_| moniker.clone());

                let clean_display = clean_resource_display_name(&display_name, &moniker);
                let is_exempt = exempt_sids.contains(&sid);

                packages.push(AppContainerPackage {
                    sid: sid.clone(),
                    app_name: if moniker.is_empty() { sid } else { moniker },
                    display_name: clean_display,
                    package_family_name: family_name,
                    loopback_exempt: is_exempt,
                });
            }
        }

        packages.sort_by(|a, b| {
            a.display_name
                .to_lowercase()
                .cmp(&b.display_name.to_lowercase())
        });
        Ok(packages)
    }

    fn run_check_net_isolation(&self, args: &[&str]) -> Result<()> {
        let status = std::process::Command::new("CheckNetIsolation.exe")
            .args(args)
            .status()
            .context("Failed to execute CheckNetIsolation.exe")?;

        if !status.success() {
            anyhow::bail!("CheckNetIsolation.exe exited with non-zero status: {status}");
        }
        Ok(())
    }
}

#[cfg(windows)]
impl AppContainerBackend for NativeAppContainerBackend {
    fn scan_containers(&self) -> Result<Vec<AppContainerPackage>> {
        let exempt_sids = self.query_exempt_sids().unwrap_or_default();
        self.scan_registry(&exempt_sids)
    }

    fn set_exempt(&self, sid: &str, exempt: bool) -> Result<()> {
        let op = if exempt { "-a" } else { "-d" };
        let param = format!("-p={sid}");
        self.run_check_net_isolation(&["LoopbackExempt", op, &param])
    }

    fn batch_set_exempt(&self, sids: &[String], exempt: bool) -> Result<()> {
        for sid in sids {
            self.set_exempt(sid, exempt)?;
        }
        Ok(())
    }

    fn exempt_all(&self) -> Result<()> {
        let containers = self.scan_containers()?;
        for container in containers {
            if !container.loopback_exempt {
                let _ = self.set_exempt(&container.sid, true);
            }
        }
        Ok(())
    }

    fn clear_all(&self) -> Result<()> {
        self.run_check_net_isolation(&["LoopbackExempt", "-c"])
    }
}

#[cfg(not(windows))]
impl AppContainerBackend for NativeAppContainerBackend {
    fn scan_containers(&self) -> Result<Vec<AppContainerPackage>> {
        log::debug!("UWP AppContainer scanning is only supported on Windows; returning empty list");
        Ok(Vec::new())
    }

    fn set_exempt(&self, sid: &str, exempt: bool) -> Result<()> {
        log::debug!("UWP loopback exempt mutation ({sid} -> {exempt}) is a no-op on non-Windows");
        Ok(())
    }

    fn batch_set_exempt(&self, _sids: &[String], _exempt: bool) -> Result<()> {
        log::debug!("UWP loopback batch exempt mutation is a no-op on non-Windows");
        Ok(())
    }

    fn exempt_all(&self) -> Result<()> {
        log::debug!("UWP loopback exempt_all is a no-op on non-Windows");
        Ok(())
    }

    fn clear_all(&self) -> Result<()> {
        log::debug!("UWP loopback clear_all is a no-op on non-Windows");
        Ok(())
    }
}

static GLOBAL_BACKEND: OnceLock<RwLock<Option<Box<dyn AppContainerBackend>>>> = OnceLock::new();

fn get_backend_registry() -> &'static RwLock<Option<Box<dyn AppContainerBackend>>> {
    GLOBAL_BACKEND.get_or_init(|| RwLock::new(None))
}

/// Sets a custom backend (e.g. `MockAppContainerBackend`) for testing or specialized environments.
pub fn set_custom_backend(backend: Box<dyn AppContainerBackend>) {
    if let Ok(mut lock) = get_backend_registry().write() {
        *lock = Some(backend);
    }
}

/// Resets the backend to use the native platform implementation.
pub fn reset_backend() {
    if let Ok(mut lock) = get_backend_registry().write() {
        *lock = None;
    }
}

fn with_backend<F, R>(f: F) -> Result<R>
where
    F: FnOnce(&dyn AppContainerBackend) -> Result<R>,
{
    let lock = get_backend_registry()
        .read()
        .map_err(|e| anyhow::anyhow!("Backend lock poisoned: {e}"))?;
    if let Some(backend) = lock.as_ref() {
        f(backend.as_ref())
    } else {
        let native = NativeAppContainerBackend;
        f(&native)
    }
}

/// Lists all installed AppContainers.
pub fn list_app_containers() -> Vec<AppContainerPackage> {
    with_backend(|b| b.scan_containers()).unwrap_or_default()
}

/// Grants or revokes loopback exemption for a specific AppContainer SID.
pub fn set_loopback_exempt(sid: &str, exempt: bool) -> Result<()> {
    with_backend(|b| b.set_exempt(sid, exempt))
}

/// Grants or revokes loopback exemptions for a batch of SIDs.
pub fn batch_set_loopback_exempt(sids: &[String], exempt: bool) -> Result<()> {
    with_backend(|b| b.batch_set_exempt(sids, exempt))
}

/// Grants loopback exemptions for all discovered AppContainers.
pub fn exempt_all() -> Result<()> {
    with_backend(|b| b.exempt_all())
}

/// Clears all loopback exemptions.
pub fn clear_all() -> Result<()> {
    with_backend(|b| b.clear_all())
}

/// Applies a preset to exempt a curated subset of AppContainers.
pub fn apply_preset(preset: UwpPreset) -> Result<Vec<String>> {
    with_backend(|b| b.apply_preset(preset))
}

/// Sanitizes and extracts a human-readable display name from raw registry strings.
pub fn clean_resource_display_name(raw: &str, moniker: &str) -> String {
    let trimmed = raw.trim();
    if !trimmed.is_empty() && !trimmed.starts_with('@') && !trimmed.starts_with("ms-resource:") {
        return trimmed.to_string();
    }

    if !moniker.is_empty() {
        let base = if let Some((prefix, _)) = moniker.split_once('_') {
            prefix
        } else {
            moniker
        };

        let formatted = base
            .split('.')
            .map(|part| part.trim())
            .filter(|p| !p.is_empty())
            .collect::<Vec<_>>()
            .join(" ");

        if !formatted.is_empty() {
            return formatted;
        }
        return moniker.to_string();
    }

    if !trimmed.is_empty() {
        return trimmed.to_string();
    }

    "Unknown AppContainer".to_string()
}

pub struct UwpLoopbackManager;

impl UwpLoopbackManager {
    /// Lists all installed AppContainers.
    pub fn list_app_containers() -> Vec<AppContainerPackage> {
        list_app_containers()
    }

    /// Sets exemption for a given SID.
    pub fn set_loopback_exempt(sid: &str, exempt: bool) -> Result<()> {
        set_loopback_exempt(sid, exempt)
    }

    /// Batch sets exemptions for multiple SIDs.
    pub fn batch_set_loopback_exempt(sids: &[String], exempt: bool) -> Result<()> {
        batch_set_loopback_exempt(sids, exempt)
    }

    /// Exempts all AppContainers.
    pub fn exempt_all() -> Result<()> {
        exempt_all()
    }

    /// Clears all exemptions.
    pub fn clear_all() -> Result<()> {
        clear_all()
    }

    /// Applies a curated preset of exemptions.
    pub fn apply_preset(preset: UwpPreset) -> Result<Vec<String>> {
        apply_preset(preset)
    }

    /// Evaluates current AppContainer isolation health.
    pub fn health() -> UwpIsolationHealth {
        let packages = Self::list_app_containers();
        let exempt_count = packages.iter().filter(|p| p.loopback_exempt).count();
        UwpIsolationHealth {
            total_packages: packages.len(),
            exempt_packages: exempt_count,
            is_supported: cfg!(windows),
        }
    }

    /// Formats the command to grant loopback exemption for a specific UWP package SID.
    pub fn format_enable_command(sid: &str) -> String {
        format!("CheckNetIsolation.exe LoopbackExempt -a -p={sid}")
    }

    /// Formats the command to revoke loopback exemption for a specific UWP package SID.
    pub fn format_disable_command(sid: &str) -> String {
        format!("CheckNetIsolation.exe LoopbackExempt -d -p={sid}")
    }

    /// Formats the command to exempt all AppContainers.
    pub fn format_exempt_all_command() -> &'static str {
        "CheckNetIsolation.exe LoopbackExempt -c"
    }

    /// Formats the command to clear all loopback exemptions.
    pub fn format_clear_all_command() -> &'static str {
        "CheckNetIsolation.exe LoopbackExempt -c"
    }

    /// Parses output from `CheckNetIsolation.exe LoopbackExempt -s`.
    pub fn parse_loopback_status_output(output: &str) -> Vec<AppContainerPackage> {
        let mut apps = Vec::new();
        let mut current_sid = String::new();
        let mut current_name = String::new();
        let mut current_display = String::new();

        for line in output.lines() {
            let trimmed = line.trim();
            if let Some(sid) = trimmed.strip_prefix("SID:") {
                if !current_sid.is_empty() {
                    let display = if current_display.is_empty() {
                        clean_resource_display_name(&current_name, &current_name)
                    } else {
                        current_display.clone()
                    };
                    apps.push(AppContainerPackage {
                        sid: current_sid.clone(),
                        app_name: if current_name.is_empty() {
                            current_sid.clone()
                        } else {
                            current_name.clone()
                        },
                        package_family_name: if current_name.is_empty() {
                            current_sid.clone()
                        } else {
                            current_name.clone()
                        },
                        display_name: display,
                        loopback_exempt: true,
                    });
                    current_name.clear();
                    current_display.clear();
                }
                current_sid = sid.trim().to_string();
            } else if let Some(name) = trimmed.strip_prefix("Name:") {
                current_name = name.trim().to_string();
            } else if let Some(disp) = trimmed.strip_prefix("DisplayName:") {
                current_display = disp.trim().to_string();
            }
        }

        if !current_sid.is_empty() {
            let display = if current_display.is_empty() {
                clean_resource_display_name(&current_name, &current_name)
            } else {
                current_display
            };
            apps.push(AppContainerPackage {
                sid: current_sid.clone(),
                app_name: if current_name.is_empty() {
                    current_sid.clone()
                } else {
                    current_name.clone()
                },
                package_family_name: if current_name.is_empty() {
                    current_sid
                } else {
                    current_name
                },
                display_name: display,
                loopback_exempt: true,
            });
        }

        apps
    }

    /// Legacy parser returning `UwpAppInfo` for compatibility.
    pub fn parse_loopback_status_legacy(output: &str) -> Vec<UwpAppInfo> {
        Self::parse_loopback_status_output(output)
            .into_iter()
            .map(UwpAppInfo::from)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_commands() {
        assert_eq!(
            UwpLoopbackManager::format_enable_command("S-1-15-2-1234"),
            "CheckNetIsolation.exe LoopbackExempt -a -p=S-1-15-2-1234"
        );
        assert_eq!(
            UwpLoopbackManager::format_disable_command("S-1-15-2-1234"),
            "CheckNetIsolation.exe LoopbackExempt -d -p=S-1-15-2-1234"
        );
        assert_eq!(
            UwpLoopbackManager::format_exempt_all_command(),
            "CheckNetIsolation.exe LoopbackExempt -c"
        );
        assert_eq!(
            UwpLoopbackManager::format_clear_all_command(),
            "CheckNetIsolation.exe LoopbackExempt -c"
        );
    }

    #[test]
    fn test_clean_resource_display_name() {
        assert_eq!(
            clean_resource_display_name("Xbox App", "Microsoft.XboxApp_8wekyb3d8bbwe"),
            "Xbox App"
        );
        assert_eq!(
            clean_resource_display_name(
                "@{Microsoft.WindowsStore_12105.1001.21.0_x64__8wekyb3d8bbwe?ms-resource://...}",
                "Microsoft.WindowsStore_8wekyb3d8bbwe"
            ),
            "Microsoft WindowsStore"
        );
        assert_eq!(
            clean_resource_display_name(
                "ms-resource:AppDisplayName",
                "SpotifyAB.SpotifyMusic_zpdnekdrzrea0"
            ),
            "SpotifyAB SpotifyMusic"
        );
        assert_eq!(
            clean_resource_display_name("", "TelegramMessengerLLP.TelegramDesktop_t4vj0pshhgkwm"),
            "TelegramMessengerLLP TelegramDesktop"
        );
        assert_eq!(clean_resource_display_name("", ""), "Unknown AppContainer");
    }

    #[test]
    fn test_parse_loopback_status_output() {
        let sample = r#"
List of AppContainer Loopback Exemptions
---------------------------------------
SID: S-1-15-2-1001
Name: Microsoft.XboxApp_8wekyb3d8bbwe
DisplayName: Xbox App

SID: S-1-15-2-1002
Name: Microsoft.WindowsStore_8wekyb3d8bbwe
DisplayName: Microsoft Store
"#;
        let apps = UwpLoopbackManager::parse_loopback_status_output(sample);
        assert_eq!(apps.len(), 2);
        assert_eq!(apps[0].sid, "S-1-15-2-1001");
        assert_eq!(apps[0].app_name, "Microsoft.XboxApp_8wekyb3d8bbwe");
        assert_eq!(
            apps[0].package_family_name,
            "Microsoft.XboxApp_8wekyb3d8bbwe"
        );
        assert_eq!(apps[0].display_name, "Xbox App");
        assert!(apps[0].loopback_exempt);
        assert_eq!(apps[1].sid, "S-1-15-2-1002");
        assert_eq!(apps[1].display_name, "Microsoft Store");
        assert!(apps[1].loopback_exempt);

        let legacy = UwpLoopbackManager::parse_loopback_status_legacy(sample);
        assert_eq!(legacy.len(), 2);
        assert_eq!(legacy[0].sid, "S-1-15-2-1001");
        assert_eq!(legacy[0].display_name, "Xbox App");
        assert!(legacy[0].is_exempt);
    }

    #[test]
    fn test_mock_backend_operations() {
        let mock = MockAppContainerBackend::with_packages(vec![
            AppContainerPackage {
                display_name: "Xbox App".to_string(),
                app_name: "Microsoft.XboxApp".to_string(),
                package_family_name: "Microsoft.XboxApp_8wekyb3d8bbwe".to_string(),
                sid: "S-1-15-2-1001".to_string(),
                loopback_exempt: false,
            },
            AppContainerPackage {
                display_name: "Microsoft Store".to_string(),
                app_name: "Microsoft.WindowsStore".to_string(),
                package_family_name: "Microsoft.WindowsStore_8wekyb3d8bbwe".to_string(),
                sid: "S-1-15-2-1002".to_string(),
                loopback_exempt: false,
            },
        ]);

        assert_eq!(mock.scan_containers().unwrap().len(), 2);

        // Single exemption
        mock.set_exempt("S-1-15-2-1001", true).unwrap();
        let pkgs = mock.scan_containers().unwrap();
        assert!(
            pkgs.iter()
                .find(|p| p.sid == "S-1-15-2-1001")
                .unwrap()
                .loopback_exempt
        );
        assert!(
            !pkgs
                .iter()
                .find(|p| p.sid == "S-1-15-2-1002")
                .unwrap()
                .loopback_exempt
        );

        // Batch exemption
        mock.batch_set_exempt(&["S-1-15-2-1002".to_string()], true)
            .unwrap();
        let pkgs = mock.scan_containers().unwrap();
        assert!(pkgs.iter().all(|p| p.loopback_exempt));

        // Clear all
        mock.clear_all().unwrap();
        let pkgs = mock.scan_containers().unwrap();
        assert!(pkgs.iter().all(|p| !p.loopback_exempt));

        // Exempt all
        mock.exempt_all().unwrap();
        let pkgs = mock.scan_containers().unwrap();
        assert!(pkgs.iter().all(|p| p.loopback_exempt));
    }

    #[test]
    fn test_presets_and_snapshots() {
        let mock = MockAppContainerBackend::with_packages(vec![
            AppContainerPackage {
                display_name: "Windows Terminal".to_string(),
                app_name: "Microsoft.WindowsTerminal".to_string(),
                package_family_name: "Microsoft.WindowsTerminal_8wekyb3d8bbwe".to_string(),
                sid: "S-1-15-2-2001".to_string(),
                loopback_exempt: false,
            },
            AppContainerPackage {
                display_name: "Spotify".to_string(),
                app_name: "SpotifyAB.SpotifyMusic".to_string(),
                package_family_name: "SpotifyAB.SpotifyMusic_zpdnekdrzrea0".to_string(),
                sid: "S-1-15-2-2002".to_string(),
                loopback_exempt: false,
            },
        ]);

        // Preset: WSL & Terminal
        let exempted = mock.apply_preset(UwpPreset::WslAndTerminal).unwrap();
        assert_eq!(exempted, vec!["S-1-15-2-2001".to_string()]);

        let pkgs = mock.scan_containers().unwrap();
        assert!(
            pkgs.iter()
                .find(|p| p.sid == "S-1-15-2-2001")
                .unwrap()
                .loopback_exempt
        );
        assert!(
            !pkgs
                .iter()
                .find(|p| p.sid == "S-1-15-2-2002")
                .unwrap()
                .loopback_exempt
        );

        // Capture snapshot
        let snapshot = mock.capture_snapshot("before media").unwrap();
        assert_eq!(snapshot.exempt_sids, vec!["S-1-15-2-2001"]);

        // Preset: Media
        mock.apply_preset(UwpPreset::MediaAndBrowsers).unwrap();
        let pkgs2 = mock.scan_containers().unwrap();
        assert!(
            pkgs2
                .iter()
                .find(|p| p.sid == "S-1-15-2-2002")
                .unwrap()
                .loopback_exempt
        );

        // Restore snapshot
        mock.restore_snapshot(&snapshot).unwrap();
        let pkgs3 = mock.scan_containers().unwrap();
        assert!(
            pkgs3
                .iter()
                .find(|p| p.sid == "S-1-15-2-2001")
                .unwrap()
                .loopback_exempt
        );
        assert!(
            !pkgs3
                .iter()
                .find(|p| p.sid == "S-1-15-2-2002")
                .unwrap()
                .loopback_exempt
        );
    }

    #[test]
    fn test_global_custom_backend_integration() {
        let mock = MockAppContainerBackend::new();
        mock.add_package(AppContainerPackage {
            display_name: "Spotify".to_string(),
            app_name: "SpotifyAB.SpotifyMusic".to_string(),
            package_family_name: "SpotifyAB.SpotifyMusic_zpdnekdrzrea0".to_string(),
            sid: "S-1-15-2-9999".to_string(),
            loopback_exempt: false,
        });

        set_custom_backend(Box::new(mock));

        let list = list_app_containers();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].sid, "S-1-15-2-9999");
        assert!(!list[0].loopback_exempt);

        set_loopback_exempt("S-1-15-2-9999", true).unwrap();
        let updated = list_app_containers();
        assert!(updated[0].loopback_exempt);

        clear_all().unwrap();
        let cleared = list_app_containers();
        assert!(!cleared[0].loopback_exempt);

        batch_set_loopback_exempt(&["S-1-15-2-9999".to_string()], true).unwrap();
        let batched = list_app_containers();
        assert!(batched[0].loopback_exempt);

        let health = UwpLoopbackManager::health();
        assert_eq!(health.total_packages, 1);
        assert_eq!(health.exempt_packages, 1);

        reset_backend();
    }

    #[test]
    fn test_uwp_app_info_conversions() {
        let pkg = AppContainerPackage {
            display_name: "Calculator".to_string(),
            app_name: "Microsoft.WindowsCalculator".to_string(),
            package_family_name: "Microsoft.WindowsCalculator_8wekyb3d8bbwe".to_string(),
            sid: "S-1-15-2-3333".to_string(),
            loopback_exempt: true,
        };

        let info = UwpAppInfo::from(pkg.clone());
        assert_eq!(info.sid, "S-1-15-2-3333");
        assert_eq!(info.name, "Microsoft.WindowsCalculator");
        assert_eq!(info.display_name, "Calculator");
        assert!(info.is_exempt);

        let roundtrip = AppContainerPackage::from(info);
        assert_eq!(roundtrip.sid, pkg.sid);
        assert_eq!(roundtrip.display_name, pkg.display_name);
        assert_eq!(roundtrip.loopback_exempt, pkg.loopback_exempt);
    }
}
