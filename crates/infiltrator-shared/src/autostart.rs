//! Cross-platform per-user autostart support.
//!
//! * Windows: the per-user `Run` registry key. Registry entries are keyed by
//!   `name`, so each desktop client manages its own entry and coexists with
//!   the other: the iced client registers as `MusicFrogInfiltrator`, the
//!   legacy Tauri client as `MihomoDespicableInfiltrator`.
//! * Linux: an XDG autostart desktop entry at
//!   `<config-dir>/autostart/<name>.desktop`, where `<config-dir>` honors
//!   `XDG_CONFIG_HOME` and falls back to `$HOME/.config`.
//! * macOS: a launchd agent plist at `~/Library/LaunchAgents/<name>.plist`.
//!   Implemented for parity, but not yet verified on real macOS hardware.
//!
//! On every platform the entry points at the current executable with
//! `--autostart`. Tests can inject a base directory instead of touching
//! environment variables.

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;
#[cfg(target_os = "windows")]
use std::process::Command;

#[cfg(target_os = "windows")]
use anyhow::anyhow;

#[cfg(target_os = "windows")]
const REG_RUN_KEY: &str = "HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Run";

#[cfg(target_os = "windows")]
use windows_sys::Win32::System::Threading::CREATE_NO_WINDOW;

#[cfg(target_os = "windows")]
fn new_hidden_command(program: &str) -> Command {
    let mut command = Command::new(program);
    command.creation_flags(CREATE_NO_WINDOW);
    command
}

/// Returns whether an autostart entry registered under `name` exists.
#[cfg(target_os = "windows")]
pub fn is_autostart_enabled(name: &str) -> bool {
    let output = new_hidden_command("reg")
        .args(["query", REG_RUN_KEY, "/v", name])
        .output();
    output.map(|o| o.status.success()).unwrap_or(false)
}

/// Creates or removes the autostart entry registered under `name`, pointing
/// it at the current executable with `--autostart`.
#[cfg(target_os = "windows")]
pub fn set_autostart_enabled(name: &str, enabled: bool) -> anyhow::Result<()> {
    if enabled {
        let exe = std::env::current_exe()?;
        let task_cmd = format!("\"{}\" --autostart", exe.to_string_lossy());
        let status = new_hidden_command("reg")
            .args([
                "add",
                REG_RUN_KEY,
                "/v",
                name,
                "/t",
                "REG_SZ",
                "/d",
                &task_cmd,
                "/f",
            ])
            .status()?;
        if !status.success() {
            return Err(anyhow!("Failed to create registry autostart entry"));
        }
    } else if is_autostart_enabled(name) {
        let status = new_hidden_command("reg")
            .args(["delete", REG_RUN_KEY, "/v", name, "/f"])
            .status()?;
        if !status.success() {
            return Err(anyhow!("Failed to delete registry autostart entry"));
        }
    }
    Ok(())
}

/// Linux backend: XDG autostart (`<config-dir>/autostart/<name>.desktop`).
#[cfg(target_os = "linux")]
mod xdg {
    use anyhow::anyhow;
    use std::env;
    use std::fs;
    use std::path::{Path, PathBuf};

    /// Resolves the XDG config dir: explicit `base_dir` (test injection)
    /// first, then a non-empty `XDG_CONFIG_HOME`, then `$HOME/.config`.
    pub(super) fn resolve_config_dir(
        base_dir: Option<&Path>,
        xdg_config_home: Option<&str>,
        home: Option<&str>,
    ) -> anyhow::Result<PathBuf> {
        if let Some(dir) = base_dir {
            return Ok(dir.to_path_buf());
        }
        if let Some(dir) = xdg_config_home.filter(|value| !value.is_empty()) {
            return Ok(PathBuf::from(dir));
        }
        let home = home.ok_or_else(|| anyhow!("neither XDG_CONFIG_HOME nor HOME is set"))?;
        Ok(PathBuf::from(home).join(".config"))
    }

    fn autostart_dir(base_dir: Option<&Path>) -> anyhow::Result<PathBuf> {
        let config_dir = resolve_config_dir(
            base_dir,
            env::var("XDG_CONFIG_HOME").ok().as_deref(),
            env::var("HOME").ok().as_deref(),
        )?;
        Ok(config_dir.join("autostart"))
    }

    fn entry_path(base_dir: Option<&Path>, name: &str) -> anyhow::Result<PathBuf> {
        Ok(autostart_dir(base_dir)?.join(format!("{name}.desktop")))
    }

    pub fn is_enabled(base_dir: Option<&Path>, name: &str) -> bool {
        entry_path(base_dir, name)
            .map(|path| path.is_file())
            .unwrap_or(false)
    }

    pub fn set_enabled(base_dir: Option<&Path>, name: &str, enabled: bool) -> anyhow::Result<()> {
        let dir = autostart_dir(base_dir)?;
        let path = dir.join(format!("{name}.desktop"));
        if enabled {
            let exe = std::env::current_exe()?;
            let exec = format!("\"{}\" --autostart", exe.to_string_lossy());
            let content = format!(
                "[Desktop Entry]\nType=Application\nName={name}\nExec={exec}\nTerminal=false\n"
            );
            fs::create_dir_all(&dir)?;
            fs::write(&path, content)?;
        } else if path.exists() {
            fs::remove_file(&path)?;
        }
        Ok(())
    }
}

/// macOS backend: launchd agent plist (`~/Library/LaunchAgents/<name>.plist`).
/// Implemented for parity, but not yet verified on real macOS hardware.
#[cfg(target_os = "macos")]
mod launchd {
    use anyhow::anyhow;
    use std::env;
    use std::fs;
    use std::path::{Path, PathBuf};

    fn agents_dir(base_dir: Option<&Path>) -> anyhow::Result<PathBuf> {
        if let Some(dir) = base_dir {
            return Ok(dir.to_path_buf());
        }
        let home = env::var("HOME").map_err(|_| anyhow!("HOME is not set"))?;
        Ok(PathBuf::from(home).join("Library").join("LaunchAgents"))
    }

    pub fn is_enabled(base_dir: Option<&Path>, name: &str) -> bool {
        agents_dir(base_dir)
            .map(|dir| dir.join(format!("{name}.plist")).is_file())
            .unwrap_or(false)
    }

    pub fn set_enabled(base_dir: Option<&Path>, name: &str, enabled: bool) -> anyhow::Result<()> {
        let dir = agents_dir(base_dir)?;
        let path = dir.join(format!("{name}.plist"));
        if enabled {
            let exe = std::env::current_exe()?;
            let mut program_arguments = String::new();
            program_arguments.push_str(&format!(
                "        <string>{}</string>\n",
                exe.to_string_lossy()
            ));
            program_arguments.push_str("        <string>--autostart</string>\n");
            let content = format!(
                "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
                <!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n\
                <plist version=\"1.0\">\n\
                <dict>\n\
                    <key>Label</key>\n\
                    <string>{name}</string>\n\
                    <key>ProgramArguments</key>\n\
                    <array>\n\
                {program_arguments}                </array>\n\
                    <key>RunAtLoad</key>\n\
                    <true/>\n\
                </dict>\n\
                </plist>\n"
            );
            fs::create_dir_all(&dir)?;
            fs::write(&path, content)?;
        } else if path.exists() {
            fs::remove_file(&path)?;
        }
        Ok(())
    }
}

/// Returns whether an autostart entry registered under `name` exists.
#[cfg(target_os = "linux")]
pub fn is_autostart_enabled(name: &str) -> bool {
    xdg::is_enabled(None, name)
}

/// Creates or removes the XDG autostart entry registered under `name`,
/// pointing it at the current executable with `--autostart`.
#[cfg(target_os = "linux")]
pub fn set_autostart_enabled(name: &str, enabled: bool) -> anyhow::Result<()> {
    xdg::set_enabled(None, name, enabled)
}

/// Returns whether an autostart entry registered under `name` exists.
///
/// The macOS launchd backend is implemented but not yet verified on real
/// hardware.
#[cfg(target_os = "macos")]
pub fn is_autostart_enabled(name: &str) -> bool {
    launchd::is_enabled(None, name)
}

/// Creates or removes the launchd agent plist registered under `name`,
/// pointing it at the current executable with `--autostart`.
#[cfg(target_os = "macos")]
pub fn set_autostart_enabled(name: &str, enabled: bool) -> anyhow::Result<()> {
    launchd::set_enabled(None, name, enabled)
}

/// Returns whether an autostart entry registered under `name` exists.
#[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
pub fn is_autostart_enabled(name: &str) -> bool {
    let _ = name;
    false
}

/// Creates or removes the autostart entry registered under `name`.
#[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
pub fn set_autostart_enabled(name: &str, enabled: bool) -> anyhow::Result<()> {
    let _ = (name, enabled);
    Err(anyhow::anyhow!(
        "Autostart is only supported on Windows, Linux and macOS"
    ))
}

#[cfg(all(test, any(target_os = "linux", target_os = "macos")))]
mod tests {
    use super::*;
    use std::fs;
    use std::path::{Path, PathBuf};

    /// `tempfile` is not a dev-dependency of this crate, so allocate a
    /// process-unique directory under the system temp dir instead.
    fn temp_base_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "infiltrator-shared-autostart-{}-{tag}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn xdg_autostart_roundtrip_with_injected_dir() {
        let dir = temp_base_dir("xdg-roundtrip");
        let base = Some(dir.as_path());
        let name = "MusicFrogInfiltratorTest";

        assert!(!xdg::is_enabled(base, name));
        xdg::set_enabled(base, name, true).unwrap();
        assert!(xdg::is_enabled(base, name));

        let desktop = dir.join("autostart").join(format!("{name}.desktop"));
        let content = fs::read_to_string(&desktop).unwrap();
        assert!(content.starts_with("[Desktop Entry]"));
        assert!(content.contains(&format!("Name={name}")));
        assert!(content.contains("Exec="));
        assert!(content.contains("--autostart"));

        xdg::set_enabled(base, name, false).unwrap();
        assert!(!desktop.exists());
        assert!(!xdg::is_enabled(base, name));

        let _ = fs::remove_dir_all(&dir);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn xdg_config_dir_resolution_prefers_xdg_config_home() {
        let injected = Path::new("/tmp/injected");

        // An injected base dir (test support) wins over the environment.
        assert_eq!(
            xdg::resolve_config_dir(Some(injected), Some("/xdg/custom"), Some("/home/user"))
                .unwrap(),
            injected
        );
        // A non-empty XDG_CONFIG_HOME wins over HOME.
        assert_eq!(
            xdg::resolve_config_dir(None, Some("/xdg/custom"), Some("/home/user")).unwrap(),
            PathBuf::from("/xdg/custom")
        );
        // Empty or missing XDG_CONFIG_HOME falls back to $HOME/.config.
        assert_eq!(
            xdg::resolve_config_dir(None, Some(""), Some("/home/user")).unwrap(),
            PathBuf::from("/home/user/.config")
        );
        assert_eq!(
            xdg::resolve_config_dir(None, None, Some("/home/user")).unwrap(),
            PathBuf::from("/home/user/.config")
        );
        // No inputs at all is a typed error, not a panic.
        assert!(xdg::resolve_config_dir(None, None, None).is_err());
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn launchd_autostart_roundtrip_with_injected_dir() {
        let dir = temp_base_dir("launchd-roundtrip");
        let base = Some(dir.as_path());
        let name = "MusicFrogInfiltratorTest";

        assert!(!launchd::is_enabled(base, name));
        launchd::set_enabled(base, name, true).unwrap();
        assert!(launchd::is_enabled(base, name));

        let plist = dir.join(format!("{name}.plist"));
        let content = fs::read_to_string(&plist).unwrap();
        assert!(content.contains(&format!("<string>{name}</string>")));
        assert!(content.contains("<string>--autostart</string>"));
        assert!(content.contains("<key>RunAtLoad</key>"));

        launchd::set_enabled(base, name, false).unwrap();
        assert!(!plist.exists());
        assert!(!launchd::is_enabled(base, name));

        let _ = fs::remove_dir_all(&dir);
    }
}
