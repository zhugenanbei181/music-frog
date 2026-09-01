//! TUN mode privilege / background-service management.
//!
//! Platform contracts (see `docs/PLATFORM_CONTRACTS.md` §2.5):
//!
//! - **Linux — capability model.** Install = `pkexec setcap
//!   cap_net_admin,cap_net_bind_service+ep <core-binary>`; uninstall = the exact
//!   inverse, `pkexec setcap -r <core-binary>`, targeting the *same* core binary
//!   that was passed to [`TunServiceManager::install_service`] (never the GUI
//!   executable). There is no daemon in this model, so start/stop are documented
//!   no-ops.
//! - **Windows — `sc.exe` service management** (unchanged; cannot be compiled or
//!   verified on the current Linux host).
//! - **macOS — explicitly unsupported in 0.20.** A real macOS TUN route needs a
//!   Network Extension (or a `nerds`-style privileged helper campaign); that work
//!   is out of scope for 0.20. The previous `sudo launchctl` calls were a fake
//!   path (no tty in a GUI session, guaranteed to fail, sometimes silently) and
//!   have been removed: every macOS verb returns [`UnsupportedPlatformError`]
//!   and status checks return [`ServiceModeStatus::Unsupported`].

use anyhow::{Context, Result};
use std::fmt;
use std::path::Path;
#[cfg_attr(target_os = "macos", allow(unused_imports))]
use std::process::Command;

/// Typed error for platforms where a TUN privilege route is intentionally not
/// implemented (currently macOS). Callers can downcast to detect "honest
/// unsupported" instead of a generic failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UnsupportedPlatformError {
    /// The service verb that was attempted, e.g. `"install_service"`.
    pub action: &'static str,
    /// The platform the verb is unsupported on, e.g. `"macOS"`.
    pub platform: &'static str,
}

impl fmt::Display for UnsupportedPlatformError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "TUN service verb `{}` is not supported on {} (requires Network Extension or a privileged helper; not implemented in 0.20)",
            self.action, self.platform
        )
    }
}

impl std::error::Error for UnsupportedPlatformError {}

/// Represents the status of the TUN service or privilege mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServiceModeStatus {
    InstalledAndRunning,
    InstalledStopped,
    NotInstalled,
    MissingPrivilege,
    Unsupported,
}

impl fmt::Display for ServiceModeStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let msg = match self {
            Self::InstalledAndRunning => "Installed and Running",
            Self::InstalledStopped => "Installed but Stopped",
            Self::NotInstalled => "Not Installed",
            Self::MissingPrivilege => "Missing Privilege",
            Self::Unsupported => "Unsupported OS",
        };
        write!(f, "{}", msg)
    }
}

/// Controller for TUN Mode privileges and background services.
pub struct TunServiceManager;

impl TunServiceManager {
    pub const SERVICE_NAME: &'static str = "MusicFrogInfiltratorService";

    /// Checks the current status of the service mode.
    pub fn check_status() -> ServiceModeStatus {
        let exe = match std::env::current_exe() {
            Ok(path) => path,
            Err(_) => return ServiceModeStatus::MissingPrivilege,
        };
        Self::check_status_for(&exe)
    }

    /// Checks the service/capability state for the actual mihomo binary.
    /// This is different from [`Self::check_status`] in packaged desktops:
    /// the GUI executable and the core executable are separate files.
    pub fn check_status_for(binary_path: &Path) -> ServiceModeStatus {
        #[cfg(target_os = "windows")]
        {
            let _ = binary_path;
            Self::check_status_windows()
        }
        #[cfg(target_os = "linux")]
        {
            Self::check_status_linux(binary_path)
        }
        #[cfg(target_os = "macos")]
        {
            let _ = binary_path;
            Self::check_status_macos()
        }
        #[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
        {
            let _ = binary_path;
            ServiceModeStatus::Unsupported
        }
    }

    /// Installs the background service or grants necessary capabilities.
    ///
    /// On Linux this grants `cap_net_admin,cap_net_bind_service+ep` on
    /// `service_bin_path` (the mihomo core binary). [`Self::uninstall_service`]
    /// is the exact inverse of this call.
    pub fn install_service(service_bin_path: &Path) -> Result<()> {
        #[cfg(target_os = "windows")]
        {
            let _ = service_bin_path;
            Self::install_service_windows(service_bin_path)
        }
        #[cfg(target_os = "linux")]
        {
            Self::install_service_linux(service_bin_path)
        }
        #[cfg(target_os = "macos")]
        {
            let _ = service_bin_path;
            Self::install_service_macos(service_bin_path)
        }
        #[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
        {
            let _ = service_bin_path;
            anyhow::bail!("Unsupported OS for service installation");
        }
    }

    /// Uninstalls the service or removes capabilities.
    ///
    /// On Linux this is the *exact inverse* of [`Self::install_service`]: it
    /// removes the capabilities (`pkexec setcap -r`) from the very same core
    /// binary that install granted them to. It must never target the GUI
    /// executable.
    pub fn uninstall_service(service_bin_path: &Path) -> Result<()> {
        #[cfg(target_os = "windows")]
        {
            let _ = service_bin_path;
            Self::uninstall_service_windows()
        }
        #[cfg(target_os = "linux")]
        {
            Self::uninstall_service_linux(service_bin_path)
        }
        #[cfg(target_os = "macos")]
        {
            let _ = service_bin_path;
            Self::uninstall_service_macos()
        }
        #[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
        {
            let _ = service_bin_path;
            anyhow::bail!("Unsupported OS for service uninstallation");
        }
    }

    /// Starts the installed service.
    ///
    /// On Linux the capability model has no daemon, so this is a documented
    /// no-op: granting capabilities is the whole "installation".
    pub fn start_service() -> Result<()> {
        #[cfg(target_os = "windows")]
        {
            Self::start_service_windows()
        }
        #[cfg(target_os = "linux")]
        {
            Self::start_service_linux()
        }
        #[cfg(target_os = "macos")]
        {
            Self::start_service_macos()
        }
        #[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
        {
            anyhow::bail!("Unsupported OS for service start");
        }
    }

    /// Stops the currently running service.
    ///
    /// On Linux the capability model has no daemon, so this is a documented
    /// no-op.
    pub fn stop_service() -> Result<()> {
        #[cfg(target_os = "windows")]
        {
            Self::stop_service_windows()
        }
        #[cfg(target_os = "linux")]
        {
            Self::stop_service_linux()
        }
        #[cfg(target_os = "macos")]
        {
            Self::stop_service_macos()
        }
        #[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
        {
            anyhow::bail!("Unsupported OS for service stop");
        }
    }

    // =========================================================================
    // WINDOWS IMPLEMENTATION
    // =========================================================================
    // NOTE: kept as-is; this host cannot compile or test the Windows arm.

    #[cfg(target_os = "windows")]
    fn check_status_windows() -> ServiceModeStatus {
        let output = Command::new("sc.exe")
            .arg("query")
            .arg(Self::SERVICE_NAME)
            .output();

        if let Ok(out) = output {
            let stdout = String::from_utf8_lossy(&out.stdout);
            if stdout.contains("1060") || stdout.contains("does not exist") {
                return ServiceModeStatus::NotInstalled;
            } else if stdout.contains("RUNNING") {
                return ServiceModeStatus::InstalledAndRunning;
            } else if stdout.contains("STOPPED") {
                return ServiceModeStatus::InstalledStopped;
            } else if !out.status.success() {
                return ServiceModeStatus::MissingPrivilege;
            }
        }
        ServiceModeStatus::MissingPrivilege
    }

    #[cfg(target_os = "windows")]
    fn install_service_windows(bin_path: &Path) -> Result<()> {
        let path_str = bin_path.to_str().context("Invalid binary path")?;
        let bin_path_arg = format!("\"{}\"", path_str);
        let status = Command::new("sc.exe")
            .args([
                "create",
                Self::SERVICE_NAME,
                "binPath=",
                &bin_path_arg,
                "start=",
                "auto",
            ])
            .status()?;

        if !status.success() {
            anyhow::bail!("Failed to create Windows service. Requires Administrator privileges.");
        }
        Ok(())
    }

    #[cfg(target_os = "windows")]
    fn uninstall_service_windows() -> Result<()> {
        let _ = Self::stop_service_windows();
        let status = Command::new("sc.exe")
            .args(["delete", Self::SERVICE_NAME])
            .status()?;

        if !status.success() {
            anyhow::bail!("Failed to delete Windows service. Requires Administrator privileges.");
        }
        Ok(())
    }

    #[cfg(target_os = "windows")]
    fn start_service_windows() -> Result<()> {
        let status = Command::new("sc.exe")
            .args(["start", Self::SERVICE_NAME])
            .status()?;

        if !status.success() {
            anyhow::bail!("Failed to start Windows service. Requires Administrator privileges.");
        }
        Ok(())
    }

    #[cfg(target_os = "windows")]
    fn stop_service_windows() -> Result<()> {
        let status = Command::new("sc.exe")
            .args(["stop", Self::SERVICE_NAME])
            .status()?;

        if !status.success() {
            anyhow::bail!("Failed to stop Windows service.");
        }
        Ok(())
    }

    // =========================================================================
    // LINUX IMPLEMENTATION
    // =========================================================================

    #[cfg(target_os = "linux")]
    fn check_status_linux(exe: &Path) -> ServiceModeStatus {
        let output = match Command::new("getcap").arg(exe).output() {
            Ok(output) => output,
            Err(_) => return ServiceModeStatus::MissingPrivilege,
        };

        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            if stdout.contains("cap_net_admin") && stdout.contains("cap_net_bind_service") {
                return ServiceModeStatus::InstalledAndRunning;
            }
        }
        ServiceModeStatus::NotInstalled
    }

    /// Builds the argv handed to `pkexec` for a setcap grant (`remove = false`)
    /// or a setcap removal (`remove = true`). Pure helper, unit-tested so the
    /// argv contract cannot drift between install and uninstall.
    #[cfg(target_os = "linux")]
    fn setcap_argv(bin_path: &Path, remove: bool) -> Vec<std::ffi::OsString> {
        let mut argv = vec![std::ffi::OsString::from("setcap")];
        if remove {
            argv.push(std::ffi::OsString::from("-r"));
        } else {
            argv.push(std::ffi::OsString::from("cap_net_admin,cap_net_bind_service+ep"));
        }
        argv.push(bin_path.as_os_str().to_os_string());
        argv
    }

    /// Runs `pkexec setcap [...] <bin_path>` and fails loudly on any non-zero
    /// exit or spawn error. `pkexec_program` is injectable so tests can point
    /// it at a fake recorder binary.
    #[cfg(target_os = "linux")]
    fn run_pkexec_setcap(pkexec_program: &Path, bin_path: &Path, remove: bool) -> Result<()> {
        let argv = Self::setcap_argv(bin_path, remove);
        let output = Command::new(pkexec_program)
            .args(&argv)
            .output()
            .with_context(|| {
                format!(
                    "failed to spawn pkexec for setcap on {}",
                    bin_path.display()
                )
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!(
                "Failed to {} capabilities via pkexec on {} (exit {:?}): {}",
                if remove { "remove" } else { "grant" },
                bin_path.display(),
                output.status.code(),
                stderr.trim()
            );
        }
        Ok(())
    }

    #[cfg(target_os = "linux")]
    fn install_service_linux(bin_path: &Path) -> Result<()> {
        // Do not invoke a shell here. The binary path is user/package
        // controlled and `pkexec setcap ... <path>` preserves spaces and
        // punctuation without introducing shell-quoting hazards.
        Self::run_pkexec_setcap(Path::new("pkexec"), bin_path, false)
    }

    /// Exact inverse of [`Self::install_service_linux`]: removes the very
    /// capabilities that install granted, from the same core binary.
    #[cfg(target_os = "linux")]
    fn uninstall_service_linux(bin_path: &Path) -> Result<()> {
        Self::run_pkexec_setcap(Path::new("pkexec"), bin_path, true)
    }

    // Capability model: there is no daemon to start or stop.
    #[cfg(target_os = "linux")]
    fn start_service_linux() -> Result<()> {
        Ok(())
    }

    #[cfg(target_os = "linux")]
    fn stop_service_linux() -> Result<()> {
        Ok(())
    }

    // =========================================================================
    // MACOS IMPLEMENTATION — honestly unsupported in 0.20
    // =========================================================================
    //
    // A real macOS TUN route requires a Network Extension (or a nerds-style
    // privileged helper). The previous implementation was a fake sudo path:
    // `sudo launchctl` cannot prompt in a GUI session, so it always failed,
    // sometimes while pretending to succeed. All verbs now fail honestly with
    // `UnsupportedPlatformError`, and status checks report `Unsupported`.

    #[cfg(target_os = "macos")]
    fn unsupported(action: &'static str) -> anyhow::Error {
        UnsupportedPlatformError {
            action,
            platform: "macOS",
        }
        .into()
    }

    #[cfg(target_os = "macos")]
    fn check_status_macos() -> ServiceModeStatus {
        ServiceModeStatus::Unsupported
    }

    #[cfg(target_os = "macos")]
    fn install_service_macos(_bin_path: &Path) -> Result<()> {
        Err(Self::unsupported("install_service"))
    }

    #[cfg(target_os = "macos")]
    fn uninstall_service_macos() -> Result<()> {
        Err(Self::unsupported("uninstall_service"))
    }

    #[cfg(target_os = "macos")]
    fn start_service_macos() -> Result<()> {
        Err(Self::unsupported("start_service"))
    }

    #[cfg(target_os = "macos")]
    fn stop_service_macos() -> Result<()> {
        Err(Self::unsupported("stop_service"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_status_display() {
        assert_eq!(
            ServiceModeStatus::InstalledAndRunning.to_string(),
            "Installed and Running"
        );
        assert_eq!(
            ServiceModeStatus::InstalledStopped.to_string(),
            "Installed but Stopped"
        );
        assert_eq!(ServiceModeStatus::NotInstalled.to_string(), "Not Installed");
        assert_eq!(
            ServiceModeStatus::MissingPrivilege.to_string(),
            "Missing Privilege"
        );
        assert_eq!(ServiceModeStatus::Unsupported.to_string(), "Unsupported OS");
    }

    #[test]
    fn test_status_transitions() {
        let mut status = ServiceModeStatus::NotInstalled;
        assert_eq!(status, ServiceModeStatus::NotInstalled);
        status = ServiceModeStatus::InstalledAndRunning;
        assert_eq!(status, ServiceModeStatus::InstalledAndRunning);
    }

    #[test]
    fn test_service_manager_dummy_check() {
        let status = TunServiceManager::check_status();
        assert!(matches!(
            status,
            ServiceModeStatus::InstalledAndRunning
                | ServiceModeStatus::InstalledStopped
                | ServiceModeStatus::NotInstalled
                | ServiceModeStatus::MissingPrivilege
                | ServiceModeStatus::Unsupported
        ));
    }

    #[test]
    fn test_unsupported_platform_error_display() {
        let err = UnsupportedPlatformError {
            action: "install_service",
            platform: "macOS",
        };
        assert!(err.to_string().contains("install_service"));
        assert!(err.to_string().contains("macOS"));
    }
}

#[cfg(all(test, target_os = "linux"))]
mod linux_tests {
    use super::*;
    use std::ffi::OsString;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use tempfile::TempDir;

    /// Creates a fake `pkexec` executable that records its argv (one arg per
    /// line) into `args_file` and exits with `exit_code`.
    fn write_fake_pkexec(dir: &Path, exit_code: i32, args_file: &Path) -> std::path::PathBuf {
        let script = dir.join("pkexec");
        let body = format!(
            "#!/bin/sh\nprintf '%s\\n' \"$@\" > {}\nexit {}\n",
            args_file.display(),
            exit_code
        );
        fs::write(&script, body).expect("write fake pkexec script");
        fs::set_permissions(&script, fs::Permissions::from_mode(0o755))
            .expect("chmod fake pkexec");
        script
    }

    fn fake_core_binary(dir: &Path) -> std::path::PathBuf {
        let core = dir.join("mihomo-core");
        fs::write(&core, b"fake-elf").expect("write fake core binary");
        core
    }

    #[test]
    fn test_tun_setcap_argv_grant_and_remove() {
        let bin = Path::new("/opt/music-frog/mihomo core");
        assert_eq!(
            TunServiceManager::setcap_argv(bin, false),
            vec![
                OsString::from("setcap"),
                OsString::from("cap_net_admin,cap_net_bind_service+ep"),
                OsString::from("/opt/music-frog/mihomo core"),
            ]
        );
        assert_eq!(
            TunServiceManager::setcap_argv(bin, true),
            vec![
                OsString::from("setcap"),
                OsString::from("-r"),
                OsString::from("/opt/music-frog/mihomo core"),
            ]
        );
    }

    #[test]
    fn test_tun_linux_install_grants_caps_on_core_binary() {
        let dir = TempDir::new().expect("tempdir");
        let args_file = dir.path().join("args.txt");
        let fake_pkexec = write_fake_pkexec(dir.path(), 0, &args_file);
        let core = fake_core_binary(dir.path());

        TunServiceManager::run_pkexec_setcap(&fake_pkexec, &core, false)
            .expect("install (setcap grant) should succeed");

        let recorded = fs::read_to_string(&args_file).expect("read recorded argv");
        let expected = format!(
            "setcap\ncap_net_admin,cap_net_bind_service+ep\n{}\n",
            core.display()
        );
        assert_eq!(recorded, expected);
    }

    #[test]
    fn test_tun_linux_uninstall_is_exact_inverse_of_install() {
        let dir = TempDir::new().expect("tempdir");
        let args_file = dir.path().join("args.txt");
        let fake_pkexec = write_fake_pkexec(dir.path(), 0, &args_file);
        let core = fake_core_binary(dir.path());

        TunServiceManager::run_pkexec_setcap(&fake_pkexec, &core, true)
            .expect("uninstall (setcap -r) should succeed");

        // Regression for the old bug: uninstall must target the core binary
        // passed in, never `current_exe()` (the GUI executable), and must use
        // `setcap -r` — the exact inverse of the install argv.
        let recorded = fs::read_to_string(&args_file).expect("read recorded argv");
        let expected = format!("setcap\n-r\n{}\n", core.display());
        assert_eq!(recorded, expected);
        let gui_exe = std::env::current_exe().expect("current_exe");
        assert!(
            !recorded.contains(gui_exe.to_string_lossy().trim()),
            "uninstall must not target the GUI executable: {recorded}"
        );
    }

    #[test]
    fn test_tun_linux_pkexec_nonzero_exit_returns_err() {
        let dir = TempDir::new().expect("tempdir");
        let args_file = dir.path().join("args.txt");
        let fake_pkexec = write_fake_pkexec(dir.path(), 3, &args_file);
        let core = fake_core_binary(dir.path());

        for remove in [false, true] {
            let err = TunServiceManager::run_pkexec_setcap(&fake_pkexec, &core, remove)
                .expect_err("non-zero exit must return Err, never silent success");
            let msg = err.to_string();
            assert!(msg.contains("pkexec"), "error should mention pkexec: {msg}");
            assert!(msg.contains("3"), "error should carry the exit code: {msg}");
        }
    }

    #[test]
    fn test_tun_linux_pkexec_spawn_failure_returns_err() {
        let dir = TempDir::new().expect("tempdir");
        let core = fake_core_binary(dir.path());
        let missing = dir.path().join("no-such-pkexec");

        let err = TunServiceManager::run_pkexec_setcap(&missing, &core, true)
            .expect_err("spawn failure must return Err");
        assert!(
            err.to_string().contains("failed to spawn pkexec"),
            "unexpected error: {err}"
        );
    }

}

#[cfg(all(test, target_os = "macos"))]
mod macos_tests {
    use super::*;

    #[test]
    fn test_tun_macos_status_is_unsupported() {
        assert_eq!(
            TunServiceManager::check_status_for(Path::new("/opt/mihomo/core")),
            ServiceModeStatus::Unsupported
        );
        assert_eq!(
            TunServiceManager::check_status(),
            ServiceModeStatus::Unsupported
        );
    }

    #[test]
    fn test_tun_macos_verbs_return_typed_unsupported() {
        let results = vec![
            TunServiceManager::install_service(Path::new("/opt/mihomo/core")),
            TunServiceManager::uninstall_service(Path::new("/opt/mihomo/core")),
            TunServiceManager::start_service(),
            TunServiceManager::stop_service(),
        ];
        for res in results {
            let err = res.expect_err("macOS verbs must fail honestly");
            let typed = err
                .downcast_ref::<UnsupportedPlatformError>()
                .expect("error must be the typed UnsupportedPlatformError");
            assert_eq!(typed.platform, "macOS");
        }
    }
}
