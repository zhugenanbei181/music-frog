//! HTTP adapter for public-egress IP probes.

use chrono::Utc;
use infiltrator_contract::snapshot::PublicIpSnapshot;
use infiltrator_http::HttpClient;
use infiltrator_ports::error::PortError;
use infiltrator_ports::public_ip_probe::PublicIpProbe;
use std::net::IpAddr;

pub struct HttpPublicIpProbe {
    client: HttpClient,
}

impl HttpPublicIpProbe {
    pub fn with_default_client() -> Self {
        Self {
            client: infiltrator_http::build_http_client(),
        }
    }
}

#[async_trait::async_trait]
impl PublicIpProbe for HttpPublicIpProbe {
    async fn probe(&self, proxy_endpoint: Option<String>) -> Result<PublicIpSnapshot, PortError> {
        let client = if let Some(endpoint) = proxy_endpoint {
            let proxy = infiltrator_http::reqwest::Proxy::http(format!("http://{endpoint}"))
                .map_err(|error| PortError::Failed(error.to_string()))?;
            HttpClient::builder()
                .timeout(std::time::Duration::from_secs(5))
                .proxy(proxy)
                .build()
                .map_err(|error| PortError::Failed(error.to_string()))?
        } else {
            self.client.clone()
        };

        let body = client
            .get("https://api.ipify.org")
            .send()
            .await
            .map_err(|error| PortError::Network(error.to_string()))?
            .text()
            .await
            .map_err(|error| PortError::Network(error.to_string()))?;
        let ip = body.trim();
        if ip.parse::<IpAddr>().is_err() {
            return Err(PortError::Failed(
                "public IP provider returned an invalid address".to_string(),
            ));
        }
        Ok(PublicIpSnapshot {
            ip: ip.to_string(),
            provider: "api.ipify.org".to_string(),
            checked_at_epoch_ms: Utc::now().timestamp_millis(),
        })
    }
}
