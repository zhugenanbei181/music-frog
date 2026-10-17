//! sing-box-specific Clash API client and live version observation.

use crate::controller_configuration::SingBoxControllerConfiguration;
use crate::http::{
    ReqwestSingBoxClashHttpPort, SingBoxClashHttpPort, SingBoxClashMethod, SingBoxClashResponse,
    SingBoxControllerAuthorization, endpoint_with_path, parse_loopback_endpoint,
};
use lurker_core_contract::identity::CoreVersion;
use lurker_core_ports::error::PortError;
use serde::de::DeserializeOwned;
use std::sync::Arc;
use std::time::Duration;
use url::Url;

const API_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Clone)]
pub struct SingBoxClashApiClient {
    base_url: Url,
    authorization: SingBoxControllerAuthorization,
    http: Arc<dyn SingBoxClashHttpPort>,
}

impl SingBoxClashApiClient {
    pub fn with_reqwest(controller: SingBoxControllerConfiguration) -> Result<Self, PortError> {
        Self::new(controller, Arc::new(ReqwestSingBoxClashHttpPort))
    }

    pub fn new(
        controller: SingBoxControllerConfiguration,
        http: Arc<dyn SingBoxClashHttpPort>,
    ) -> Result<Self, PortError> {
        Ok(Self {
            base_url: parse_loopback_endpoint(&controller.endpoint())?,
            authorization: controller.authorization().clone(),
            http,
        })
    }

    pub fn endpoint(&self) -> &Url {
        &self.base_url
    }

    pub(crate) async fn get_json<T: DeserializeOwned>(
        &self,
        segments: &[&str],
    ) -> Result<T, PortError> {
        let response = self.send(SingBoxClashMethod::Get, segments, None).await?;
        require_status(&response, 200, "GET")?;
        serde_json::from_slice(&response.body)
            .map_err(|_| PortError::Network("sing-box Clash API JSON was invalid".to_owned()))
    }

    pub(crate) async fn put_json(
        &self,
        segments: &[&str],
        body: serde_json::Value,
    ) -> Result<(), PortError> {
        let body = serde_json::to_vec(&body).map_err(|_| {
            PortError::Failed("sing-box request JSON could not be encoded".to_owned())
        })?;
        let response = self
            .send(SingBoxClashMethod::Put, segments, Some(body))
            .await?;
        require_status(&response, 204, "PUT")
    }

    async fn send(
        &self,
        method: SingBoxClashMethod,
        segments: &[&str],
        body: Option<Vec<u8>>,
    ) -> Result<SingBoxClashResponse, PortError> {
        let url = endpoint_with_path(&self.base_url, segments)?;
        self.http
            .request(&url, method, &self.authorization, body, API_TIMEOUT)
            .await
    }
}

/// Reads the running sing-box version from the live Clash API `/version`.
///
/// This value is never inferred from the locked download manifest or the
/// executable path. The API reports `sing-box <version>`; the core identity
/// stores the canonical `v<version>` form used by operation evidence locks.
#[derive(Clone)]
pub struct SingBoxCoreVersionProvider {
    api: SingBoxClashApiClient,
}

impl SingBoxCoreVersionProvider {
    pub fn new(api: SingBoxClashApiClient) -> Self {
        Self { api }
    }

    pub async fn observe(&self) -> Result<CoreVersion, PortError> {
        let response: VersionResponse = self.api.get_json(&["version"]).await?;
        parse_core_version(&response.version)
    }
}

#[derive(serde::Deserialize)]
struct VersionResponse {
    version: String,
}

fn parse_core_version(value: &str) -> Result<CoreVersion, PortError> {
    let version = value.strip_prefix("sing-box ").ok_or_else(|| {
        PortError::Network("sing-box /version returned an unexpected engine identity".to_owned())
    })?;
    if version.is_empty()
        || version.trim() != version
        || !version.as_bytes()[0].is_ascii_digit()
        || !version
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b".-+".contains(&byte))
    {
        return Err(PortError::Network(
            "sing-box /version returned a non-canonical version".to_owned(),
        ));
    }
    CoreVersion::new(format!("v{version}"))
        .map_err(|_| PortError::Network("sing-box /version returned an empty version".to_owned()))
}

fn require_status(
    response: &SingBoxClashResponse,
    expected: u16,
    method: &str,
) -> Result<(), PortError> {
    if response.status == expected {
        Ok(())
    } else {
        Err(PortError::Network(format!(
            "sing-box Clash API {method} returned HTTP {}",
            response.status
        )))
    }
}

#[cfg(test)]
mod version_tests {
    use super::parse_core_version;

    #[test]
    fn live_version_parser_requires_the_sing_box_engine_prefix() {
        assert_eq!(
            parse_core_version("sing-box 1.14.3").unwrap().raw(),
            "v1.14.3"
        );
        assert!(parse_core_version("mihomo v1.19.32").is_err());
        assert!(parse_core_version("sing-box ").is_err());
        assert!(parse_core_version("sing-box 1.14.3 unexpected").is_err());
    }
}
