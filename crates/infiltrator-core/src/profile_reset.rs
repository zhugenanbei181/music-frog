use anyhow::anyhow;
use infiltrator_ports::error::PortError;
use infiltrator_ports::profile_reset::ProfileResetPort;
use mihomo_config::port::find_available_port;

use crate::settings_io::app_config_manager;
use tokio::fs;

pub struct FileProfileReset;

impl FileProfileReset {
    pub fn current() -> Self {
        Self
    }
}

#[async_trait::async_trait]
impl ProfileResetPort for FileProfileReset {
    async fn reset_to_default(&self) -> Result<(), PortError> {
        reset_profiles_to_default_impl()
            .await
            .map_err(|error| PortError::Io(error.to_string()))
    }
}

pub async fn reset_profiles_to_default() -> anyhow::Result<()> {
    FileProfileReset::current()
        .reset_to_default()
        .await
        .map_err(|error| anyhow!(error.to_string()))
}

async fn reset_profiles_to_default_impl() -> anyhow::Result<()> {
    let manager = app_config_manager().await?;
    // 清空并重建的是解析后的 configs 目录（可能已被云同步重定向），
    // 而不是固定的 `<home>/configs`。
    let config_dir = manager.config_dir().to_path_buf();
    if config_dir.exists() {
        fs::remove_dir_all(&config_dir).await?;
    }

    let default_config = build_default_config()?;
    manager.save("default", &default_config).await?;
    manager.set_current("default").await?;
    Ok(())
}

fn build_default_config() -> anyhow::Result<String> {
    let port = find_available_port(9090)
        .ok_or_else(|| anyhow!("无法找到可用的控制接口端口（9090-9190）"))?;
    Ok(format!(
        r#"# mihomo configuration
port: 7890
socks-port: 7891
allow-lan: false
mode: rule
log-level: info
external-controller: 127.0.0.1:{}
"#,
        port
    ))
}
