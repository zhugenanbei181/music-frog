//! Iced-side access to the application network facade.

use infiltrator_application::network_application::NetworkApplication;
use std::sync::Arc;

pub fn application() -> NetworkApplication {
    NetworkApplication::new(Arc::new(infiltrator_desktop::storage::public_ip_probe()))
}
