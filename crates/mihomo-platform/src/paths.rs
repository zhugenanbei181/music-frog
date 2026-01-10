use mihomo_api::MihomoError;
use std::path::PathBuf;
use std::sync::OnceLock;

static DATA_DIR_OVERRIDE: OnceLock<PathBuf> = OnceLock::new();

pub fn set_home_dir_override(path: PathBuf) -> bool {
    DATA_DIR_OVERRIDE.set(path).is_ok()
}

pub fn get_home_dir() -> Result<PathBuf, MihomoError> {
    if let Some(path) = DATA_DIR_OVERRIDE.get() {
        log::debug!("Using data dir override: {}", path.display());
        return Ok(path.clone());
    }

    if let Ok(home) = std::env::var("MIHOMO_HOME") {
        log::debug!("Using MIHOMO_HOME: {}", home);
        return Ok(PathBuf::from(home));
    }

    let home = dirs::home_dir()
        .ok_or_else(|| MihomoError::Config("Could not determine home directory".to_string()))?;

    let mihomo_home = home.join(".config/mihomo-rs");
    log::debug!("Using default home: {}", mihomo_home.display());
    Ok(mihomo_home)
}
