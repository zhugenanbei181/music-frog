//! Typed preflight for the loopback Clash API embedded in a sing-box profile.

use crate::api::http::SingBoxControllerAuthorization;
use serde_json::Value;
use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::net::SocketAddr;

/// Why the controller configuration of a sing-box profile was rejected.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SingBoxControllerConfigurationError {
    /// The profile is not valid JSON.
    InvalidJson,
    /// The profile does not enable the experimental Clash API.
    ClashApiDisabled,
    /// The external controller is not a socket address.
    InvalidExternalController,
    /// The external controller does not bind to loopback.
    ExternalControllerNotLoopback,
    /// The external controller uses the ephemeral port zero.
    ExternalControllerHasEphemeralPort,
    /// The Clash API secret is not a string.
    InvalidSecret,
}

impl Display for SingBoxControllerConfigurationError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidJson => formatter.write_str("sing-box profile is not valid JSON"),
            Self::ClashApiDisabled => {
                formatter.write_str("sing-box profile does not enable experimental.clash_api")
            }
            Self::InvalidExternalController => {
                formatter.write_str("sing-box external_controller is not a socket address")
            }
            Self::ExternalControllerNotLoopback => {
                formatter.write_str("sing-box external_controller must bind to loopback")
            }
            Self::ExternalControllerHasEphemeralPort => {
                formatter.write_str("sing-box external_controller must use a fixed, non-zero port")
            }
            Self::InvalidSecret => {
                formatter.write_str("sing-box Clash API secret must be a string")
            }
        }
    }
}

impl Error for SingBoxControllerConfigurationError {}

/// The validated loopback controller configuration of a sing-box profile.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SingBoxControllerConfiguration {
    address: SocketAddr,
    authorization: SingBoxControllerAuthorization,
}

impl SingBoxControllerConfiguration {
    /// Reads the controller configuration out of a sing-box profile.
    ///
    /// The profile must enable the experimental Clash API, bind the external
    /// controller to loopback and use a fixed, non-zero port.
    pub fn from_profile_json(
        raw_bytes: &[u8],
    ) -> Result<Self, SingBoxControllerConfigurationError> {
        let document: Value = serde_json::from_slice(raw_bytes)
            .map_err(|_| SingBoxControllerConfigurationError::InvalidJson)?;
        let clash_api = document
            .get("experimental")
            .and_then(Value::as_object)
            .and_then(|experimental| experimental.get("clash_api"))
            .and_then(Value::as_object)
            .ok_or(SingBoxControllerConfigurationError::ClashApiDisabled)?;
        let external_controller = clash_api
            .get("external_controller")
            .and_then(Value::as_str)
            .filter(|address| !address.trim().is_empty())
            .ok_or(SingBoxControllerConfigurationError::ClashApiDisabled)?;
        let address = external_controller
            .parse::<SocketAddr>()
            .map_err(|_| SingBoxControllerConfigurationError::InvalidExternalController)?;
        if !address.ip().is_loopback() {
            return Err(SingBoxControllerConfigurationError::ExternalControllerNotLoopback);
        }
        if address.port() == 0 {
            return Err(SingBoxControllerConfigurationError::ExternalControllerHasEphemeralPort);
        }
        let authorization = match clash_api.get("secret") {
            None | Some(Value::Null) => SingBoxControllerAuthorization::NoSecret,
            Some(Value::String(secret)) if secret.is_empty() => {
                SingBoxControllerAuthorization::NoSecret
            }
            Some(Value::String(secret)) => {
                SingBoxControllerAuthorization::BearerToken(secret.clone())
            }
            Some(_) => return Err(SingBoxControllerConfigurationError::InvalidSecret),
        };
        Ok(Self {
            address,
            authorization,
        })
    }

    /// Returns the socket address the controller binds to.
    pub fn address(&self) -> SocketAddr {
        self.address
    }

    /// Returns the explicit HTTP loopback endpoint of the controller.
    pub fn endpoint(&self) -> String {
        format!("http://{}", self.address)
    }

    /// Returns the authorization the controller expects.
    pub fn authorization(&self) -> &SingBoxControllerAuthorization {
        &self.authorization
    }
}

#[cfg(test)]
#[path = "controller_configuration_tests.rs"]
mod tests;
