//! Conversion of driver failures into the neutral network failure class.

use crate::error::MihomoError;
use lurker_core_contract::error::{ErrorCode, Failure};
use lurker_core_ports::error::PortError;

/// Converts a driver failure into the neutral network-facing port failure.
///
/// The HTTP status decides the category: a rejected request, a missing
/// authentication, a denied permission and a missing resource keep their own
/// class; every other failure is a network failure.
pub fn network_error(error: MihomoError) -> PortError {
    if let MihomoError::Http(http) = &error {
        match http.status().map(|status| status.as_u16()) {
            Some(400 | 422) => {
                return PortError::Rejected(Failure::new(
                    ErrorCode::InvalidInput,
                    error.to_string(),
                    false,
                ));
            }
            Some(401) => {
                return PortError::Rejected(Failure::new(
                    ErrorCode::Authentication,
                    error.to_string(),
                    false,
                ));
            }
            Some(403) => return PortError::PermissionDenied(error.to_string()),
            Some(404) => return PortError::NotFound(error.to_string()),
            _ => {}
        }
    }
    PortError::Network(error.to_string())
}
