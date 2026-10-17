//! Iced-side access to the application configuration facade.

use crate::configs_dir::config_manager;
use infiltrator_application::configuration_application::ConfigurationApplication;
use infiltrator_composition::mihomo_configuration::for_mihomo_store;
use lurker_core_contract::error::InfiltratorError;

pub async fn application() -> Result<ConfigurationApplication, InfiltratorError> {
    let store = config_manager().await?;
    Ok(for_mihomo_store(store))
}
