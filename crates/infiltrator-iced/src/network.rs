//! Iced-side access to the application network facade.

use crate::host::storage::public_ip_probe;
use infiltrator_application::network_application::NetworkApplication;
use std::sync::Arc;

pub fn application() -> NetworkApplication {
    NetworkApplication::new(Arc::new(public_ip_probe()))
}
