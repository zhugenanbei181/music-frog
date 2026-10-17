//! Injectable, loopback-only HTTP boundary for sing-box's Clash API.

use async_trait::async_trait;
use lurker_core_ports::error::PortError;
use reqwest::header::CONTENT_TYPE;
use reqwest::redirect::Policy;
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
            // The controller is loopback-only: never let an ambient HTTP proxy
            // (HTTP_PROXY/ALL_PROXY) take over the request and see the secret.
            .no_proxy()
            // Never follow redirects: a squatter on the controller port could
            // otherwise redirect a bearer-authenticated request off-host.
            .redirect(Policy::none())
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
            Some(body) => request.header(CONTENT_TYPE, "application/json").body(body),
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

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;
    use tokio::time::timeout;

    #[test]
    fn loopback_endpoint_accepts_only_explicit_http_loopback() {
        for accepted in [
            "http://127.0.0.1:9090",
            "http://127.0.0.1:1",
            "http://[::1]:9090",
        ] {
            assert!(parse_loopback_endpoint(accepted).is_ok(), "{accepted}");
        }
        for rejected in [
            "https://127.0.0.1:9090",
            "http://192.168.1.5:9090",
            "http://0.0.0.0:9090",
            "http://localhost:9090",
            "http://user:pass@127.0.0.1:9090",
            "http://127.0.0.1:9090/proxies",
            "http://127.0.0.1:9090?x=1",
            "http://127.0.0.1:9090#frag",
            "http://127.0.0.1",
        ] {
            assert!(parse_loopback_endpoint(rejected).is_err(), "{rejected}");
        }
    }

    #[tokio::test]
    async fn redirects_are_never_followed() {
        let sink = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let sink_address = sink.local_addr().unwrap();
        let target = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let target_address = target.local_addr().unwrap();

        let redirect = tokio::spawn(async move {
            let (mut socket, _) = target.accept().await.unwrap();
            let mut buffer = [0u8; 1024];
            let _ = socket.read(&mut buffer).await;
            let response = format!(
                "HTTP/1.1 302 Found\r\nLocation: http://{sink_address}/steal\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
            );
            socket.write_all(response.as_bytes()).await.unwrap();
        });

        let url = parse_loopback_endpoint(&format!("http://{target_address}")).unwrap();
        let response = ReqwestSingBoxClashHttpPort
            .request(
                &url,
                SingBoxClashMethod::Get,
                &SingBoxControllerAuthorization::BearerToken("secret".to_owned()),
                None,
                Duration::from_secs(5),
            )
            .await
            .unwrap();
        assert_eq!(response.status, 302);
        redirect.await.unwrap();

        timeout(Duration::from_millis(200), sink.accept())
            .await
            .expect_err("a redirect must not be followed");
    }
}
