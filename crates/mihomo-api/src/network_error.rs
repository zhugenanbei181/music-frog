use crate::error::MihomoError;
use infiltrator_ports::error::PortError;
use lurker_core_contract::error::{ErrorCode, Failure};

pub(crate) fn network_error(error: MihomoError) -> PortError {
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
