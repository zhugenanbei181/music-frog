//! Injectable, loopback-only HTTP boundary for sing-box's Clash API.

use async_trait::async_trait;
use infiltrator_ports::error::PortError;
use reqwest::header::CONTENT_TYPE;
use std::fmt::{self, Debug, Formatter};
use std::time::Duration;
use url::{Host, Url};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SingBoxClashMethod {
    Get,
    Put,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SingBoxClashResponse {
    pub status: u16,
    pub body: Vec<u8>,
}

#[derive(Clone, PartialEq, Eq)]
pub enum SingBoxControllerAuthorization {
    NoSecret,
    BearerToken(String),
}

impl Debug for SingBoxControllerAuthorization {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoSecret => formatter.write_str("NoSecret"),
            Self::BearerToken(_) => formatter.write_str("BearerToken(<redacted>)"),
        }
    }
}

#[async_trait]
pub trait SingBoxClashHttpPort: Send + Sync {
    async fn request(
        &self,
        url: &Url,
        method: SingBoxClashMethod,
        authorization: &SingBoxControllerAuthorization,
        body: Option<Vec<u8>>,
        timeout: Duration,
    ) -> Result<SingBoxClashResponse, PortError>;
}

#[derive(Clone, Copy, Debug, Default)]
pub struct ReqwestSingBoxClashHttpPort;

#[async_trait]
impl SingBoxClashHttpPort for ReqwestSingBoxClashHttpPort {
    async fn request(
        &self,
        url: &Url,
        method: SingBoxClashMethod,
        authorization: &SingBoxControllerAuthorization,
        body: Option<Vec<u8>>,
        timeout: Duration,
    ) -> Result<SingBoxClashResponse, PortError> {
        let client = reqwest::Client::builder()
            .timeout(timeout)
            .build()
            .map_err(|_| PortError::Network("sing-box HTTP client failed".to_owned()))?;
        let request = match method {
            SingBoxClashMethod::Get => client.get(url.clone()),
            SingBoxClashMethod::Put => client.put(url.clone()),
        };
        let request = match authorization {
            SingBoxControllerAuthorization::NoSecret => request,
            SingBoxControllerAuthorization::BearerToken(token) => request.bearer_auth(token),
        };
        let request = match body {
            Some(body) => request
                .header(CONTENT_TYPE, "application/json")
                .body(body),
            None => request,
        };
        let response = request
            .send()
            .await
            .map_err(|_| PortError::Network("sing-box HTTP request failed".to_owned()))?;
        let status = response.status().as_u16();
        let body = response
            .bytes()
            .await
            .map_err(|_| PortError::Network("sing-box HTTP response read failed".to_owned()))?
            .to_vec();
        Ok(SingBoxClashResponse { status, body })
    }
}

pub(crate) fn parse_loopback_endpoint(endpoint: &str) -> Result<Url, PortError> {
    let url = Url::parse(endpoint)
        .map_err(|_| PortError::Failed("sing-box controller URL is invalid".to_owned()))?;
    let loopback_host = match url.host() {
        Some(Host::Ipv4(address)) => address.is_loopback(),
        Some(Host::Ipv6(address)) => address.is_loopback(),
        Some(Host::Domain(_)) | None => false,
    };

    if url.scheme() != "http"
        || !loopback_host
        || !url.username().is_empty()
        || url.password().is_some()
        || (url.path() != "/" && !url.path().is_empty())
        || url.query().is_some()
        || url.fragment().is_some()
        || url.port().is_none()
    {
        return Err(PortError::Failed(
            "sing-box controller must be an explicit HTTP loopback endpoint".to_owned(),
        ));
    }

    Ok(url)
}

pub(crate) fn endpoint_with_path(base: &Url, segments: &[&str]) -> Result<Url, PortError> {
    let mut url = base.clone();
    {
        let mut path = url
            .path_segments_mut()
            .map_err(|()| PortError::Failed("controller URL cannot be a base".to_owned()))?;
        path.clear();
        for segment in segments {
            path.push(segment);
        }
    }
    Ok(url)
}
