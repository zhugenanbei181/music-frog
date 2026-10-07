//! Iced host access to the shared per-app routing application.

use crate::host::storage::app_routing_store;
use infiltrator_application::routing_application::RoutingApplication;
use infiltrator_contract::error::InfiltratorError;
use std::sync::Arc;

pub async fn application() -> Result<RoutingApplication, InfiltratorError> {
    let store = app_routing_store().map_err(|error| InfiltratorError::Config(error.to_string()))?;
    Ok(RoutingApplication::new(Arc::new(store)))
}
