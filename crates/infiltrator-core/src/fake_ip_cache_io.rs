//! Host filesystem adapter for the Mihomo Fake-IP cache.

use anyhow::Context;

pub async fn clear_fake_ip_cache() -> anyhow::Result<bool> {
    let manager = crate::settings_io::app_config_manager()
        .await
        .context("init config manager")?;
    let profile_path = manager
        .get_current_path()
        .await
        .context("load current profile path")?;
    let config_dir = profile_path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("profile path has no parent directory"))?;
    let cache_path = config_dir.join("fake-ip-cache");
    if tokio::fs::try_exists(&cache_path)
        .await
        .context("check fake-ip cache")?
    {
        tokio::fs::remove_file(&cache_path)
            .await
            .context("remove fake-ip cache")?;
        return Ok(true);
    }
    Ok(false)
}
