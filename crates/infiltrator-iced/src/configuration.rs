//! Iced-side access to the application configuration facade.

use crate::configs_dir::config_manager;
use infiltrator_application::configuration_application::ConfigurationApplication;
use infiltrator_contract::error::InfiltratorError;

pub async fn application() -> Result<ConfigurationApplication, InfiltratorError> {
    let store = config_manager().await?;
    Ok(ConfigurationApplication::new(store))
}
