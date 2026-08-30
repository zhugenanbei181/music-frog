use std::path::{Path, PathBuf};
use std::io::Result;
use std::fs;
use std::env;

#[cfg(target_os = "windows")]
use std::io::{Error, ErrorKind};

pub struct AutostartManager {
    base_dir_override: Option<PathBuf>,
}

impl Default for AutostartManager {
    fn default() -> Self {
        Self::new()
    }
}

impl AutostartManager {
    pub fn new() -> Self {
        Self { base_dir_override: None }
    }

    pub fn with_base_dir(base_dir: PathBuf) -> Self {
        Self { base_dir_override: Some(base_dir) }
    }

    #[cfg(target_os = "windows")]
    pub fn enable(&self, app_name: &str, exec_path: &Path, args: &[&str]) -> Result<()> {
        use winreg::enums::*;
        use winreg::RegKey;

        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let path = Path::new("Software").join("Microsoft").join("Windows").join("CurrentVersion").join("Run");
        let (key, _) = hkcu.create_subkey(&path)?;

        let mut command = format!("\"{}\"", exec_path.display());
        for arg in args {
            command.push_str(&format!(" {}", arg));
        }

        key.set_value(app_name, &command)
    }

    #[cfg(target_os = "windows")]
    pub fn disable(&self, app_name: &str) -> Result<()> {
        use winreg::enums::*;
        use winreg::RegKey;

        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let path = Path::new("Software").join("Microsoft").join("Windows").join("CurrentVersion").join("Run");
        let key = hkcu.open_subkey_with_flags(&path, KEY_WRITE)?;

        match key.delete_value(app_name) {
            Ok(_) => Ok(()),
            Err(e) if e.kind() == ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e),
        }
    }

    #[cfg(target_os = "windows")]
    pub fn is_enabled(&self, app_name: &str) -> Result<bool> {
        use winreg::enums::*;
        use winreg::RegKey;

        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let path = Path::new("Software").join("Microsoft").join("Windows").join("CurrentVersion").join("Run");
        let key = match hkcu.open_subkey_with_flags(&path, KEY_READ) {
            Ok(k) => k,
            Err(_) => return Ok(false),
        };

        let val: std::result::Result<String, _> = key.get_value(app_name);
        Ok(val.is_ok())
    }

    #[cfg(target_os = "linux")]
    fn get_autostart_dir(&self) -> Result<PathBuf> {
        if let Some(ref d) = self.base_dir_override {
            return Ok(d.clone());
        }
        let home = env::var("HOME").map_err(|e| std::io::Error::new(std::io::ErrorKind::NotFound, e))?;
        Ok(PathBuf::from(home).join(".config").join("autostart"))
    }

    #[cfg(target_os = "linux")]
    pub fn enable(&self, app_name: &str, exec_path: &Path, args: &[&str]) -> Result<()> {
        let dir = self.get_autostart_dir()?;
        fs::create_dir_all(&dir)?;

        let mut command = format!("\"{}\"", exec_path.display());
        for arg in args {
            command.push_str(&format!(" {}", arg));
        }

        let content = format!(
            "[Desktop Entry]\n\
            Type=Application\n\
            Name={}\n\
            Exec={}\n\
            Terminal=false\n",
            app_name, command
        );

        let file_path = dir.join(format!("{}.desktop", app_name));
        fs::write(file_path, content)
    }

    #[cfg(target_os = "linux")]
    pub fn disable(&self, app_name: &str) -> Result<()> {
        let dir = self.get_autostart_dir()?;
        let file_path = dir.join(format!("{}.desktop", app_name));
        if file_path.exists() {
            fs::remove_file(file_path)?;
        }
        Ok(())
    }

    #[cfg(target_os = "linux")]
    pub fn is_enabled(&self, app_name: &str) -> Result<bool> {
        let dir = self.get_autostart_dir()?;
        let file_path = dir.join(format!("{}.desktop", app_name));
        Ok(file_path.exists())
    }

    #[cfg(target_os = "macos")]
    fn get_autostart_dir(&self) -> Result<PathBuf> {
        if let Some(ref d) = self.base_dir_override {
            return Ok(d.clone());
        }
        let home = env::var("HOME").map_err(|e| std::io::Error::new(std::io::ErrorKind::NotFound, e))?;
        Ok(PathBuf::from(home).join("Library").join("LaunchAgents"))
    }

    #[cfg(target_os = "macos")]
    pub fn enable(&self, app_name: &str, exec_path: &Path, args: &[&str]) -> Result<()> {
        let dir = self.get_autostart_dir()?;
        fs::create_dir_all(&dir)?;

        let mut args_xml = String::new();
        args_xml.push_str(&format!("        <string>{}</string>\n", exec_path.display()));
        for arg in args {
            args_xml.push_str(&format!("        <string>{}</string>\n", arg));
        }

        let content = format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
            <!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n\
            <plist version=\"1.0\">\n\
            <dict>\n\
                <key>Label</key>\n\
                <string>{}</string>\n\
                <key>ProgramArguments</key>\n\
                <array>\n\
{}                </array>\n\
                <key>RunAtLoad</key>\n\
                <true/>\n\
            </dict>\n\
            </plist>\n",
            app_name, args_xml
        );

        let file_path = dir.join(format!("{}.plist", app_name));
        fs::write(file_path, content)
    }

    #[cfg(target_os = "macos")]
    pub fn disable(&self, app_name: &str) -> Result<()> {
        let dir = self.get_autostart_dir()?;
        let file_path = dir.join(format!("{}.plist", app_name));
        if file_path.exists() {
            fs::remove_file(file_path)?;
        }
        Ok(())
    }

    #[cfg(target_os = "macos")]
    pub fn is_enabled(&self, app_name: &str) -> Result<bool> {
        let dir = self.get_autostart_dir()?;
        let file_path = dir.join(format!("{}.plist", app_name));
        Ok(file_path.exists())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    #[cfg(target_os = "linux")]
    fn test_linux_autostart() {
        let dir = tempdir().unwrap();
        let manager = AutostartManager::with_base_dir(dir.path().to_path_buf());
        let app_name = "test_app";
        let exec_path = Path::new("/usr/bin/test");
        let args = ["--arg1", "value"];

        assert!(!manager.is_enabled(app_name).unwrap());
        manager.enable(app_name, exec_path, &args).unwrap();
        assert!(manager.is_enabled(app_name).unwrap());

        let desktop_file = dir.path().join(format!("{}.desktop", app_name));
        let content = fs::read_to_string(&desktop_file).unwrap();
        assert!(content.contains("Name=test_app"));
        assert!(content.contains("Exec=\"/usr/bin/test\" --arg1 value"));

        manager.disable(app_name).unwrap();
        assert!(!manager.is_enabled(app_name).unwrap());
        assert!(!desktop_file.exists());
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn test_macos_autostart() {
        let dir = tempdir().unwrap();
        let manager = AutostartManager::with_base_dir(dir.path().to_path_buf());
        let app_name = "test_app";
        let exec_path = Path::new("/usr/bin/test");
        let args = ["--arg1", "value"];

        assert!(!manager.is_enabled(app_name).unwrap());
        manager.enable(app_name, exec_path, &args).unwrap();
        assert!(manager.is_enabled(app_name).unwrap());

        let plist_file = dir.path().join(format!("{}.plist", app_name));
        let content = fs::read_to_string(&plist_file).unwrap();
        assert!(content.contains("<string>test_app</string>"));
        assert!(content.contains("<string>/usr/bin/test</string>"));
        assert!(content.contains("<string>--arg1</string>"));
        assert!(content.contains("<string>value</string>"));

        manager.disable(app_name).unwrap();
        assert!(!manager.is_enabled(app_name).unwrap());
        assert!(!plist_file.exists());
    }
}
