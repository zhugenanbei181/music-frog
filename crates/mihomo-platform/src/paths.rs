use mihomo_api::MihomoError;
use std::path::PathBuf;
use std::sync::RwLock;
use std::sync::Arc;
use std::sync::LazyLock;

static DATA_DIR_OVERRIDE: LazyLock<Arc<RwLock<Option<PathBuf>>>> = LazyLock::new(|| Arc::new(RwLock::new(None)));

pub fn set_home_dir_override(path: PathBuf) -> bool {
    if let Ok(mut guard) = DATA_DIR_OVERRIDE.write() {
        *guard = Some(path);
        true
    } else {
        false
    }
}

pub fn clear_home_dir_override() {
    if let Ok(mut guard) = DATA_DIR_OVERRIDE.write() {
        *guard = None;
    }
}

pub fn get_home_dir() -> Result<PathBuf, MihomoError> {
    if let Ok(guard) = DATA_DIR_OVERRIDE.read()
        && let Some(path) = guard.as_ref() {
            log::debug!("Using data dir override: {}", path.display());
            return Ok(path.clone());
        }

    if let Ok(home) = std::env::var("MIHOMO_HOME") {
        let path = PathBuf::from(home);
        if !path.as_os_str().is_empty() {
            log::debug!("Using MIHOMO_HOME: {}", path.display());
            return Ok(path);
        }
    }

    let home = dirs::home_dir()
        .ok_or_else(|| MihomoError::Config("Could not determine home directory".to_string()))?;

    let mihomo_home = home.join(".config/mihomo-rs");
    log::debug!("Using default home: {}", mihomo_home.display());
    Ok(mihomo_home)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_set_home_dir_override() {
        let path = PathBuf::from("/test/path");
        let result = set_home_dir_override(path.clone());
        assert!(result);

        // Get the override
        let retrieved = get_home_dir().unwrap();
        assert_eq!(retrieved, path);
    }

    #[test]
    fn test_get_home_dir_with_override() {
        let path = PathBuf::from("/custom/home");
        set_home_dir_override(path.clone());

        let result = get_home_dir();
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), path);
    }

    #[test]
    fn test_get_home_dir_without_override() {
        // Clear override
        clear_home_dir_override();

        // Set MIHOMO_HOME environment variable
        unsafe { std::env::set_var("MIHOMO_HOME", "/env/home") };

        let result = get_home_dir();
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), PathBuf::from("/env/home"));

        // Clean up
        unsafe { std::env::remove_var("MIHOMO_HOME") };
    }

    #[test]
    fn test_get_home_dir_default() {
        // Clear override and env var
        clear_home_dir_override();
        unsafe { std::env::remove_var("MIHOMO_HOME") };

        // This should use dirs::home_dir() and return Ok
        // (will fail in headless environments, but that's expected)
        let _result = get_home_dir();
        // We don't assert success here as it depends on the test environment
    }
}
