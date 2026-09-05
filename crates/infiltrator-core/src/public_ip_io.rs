//! HTTP adapter for public-egress IP probes.

use chrono::Utc;
use infiltrator_contract::snapshot::PublicIpSnapshot;
use infiltrator_http::HttpClient;
use infiltrator_ports::error::PortError;
use infiltrator_ports::public_ip_probe::PublicIpProbe;
use serde::Deserialize;
use std::net::IpAddr;

pub struct HttpPublicIpProbe {
    client: HttpClient,
    provider: PublicIpProvider,
}

#[derive(Clone, Copy)]
enum PublicIpProvider {
    Ipify,
    IpApi,
}

#[derive(Deserialize)]
struct IpApiResponse {
    ip: Option<String>,
    #[serde(rename = "country_name")]
    country: Option<String>,
    region: Option<String>,
    city: Option<String>,
}

impl HttpPublicIpProbe {
    pub fn with_default_client() -> Self {
        Self {
            client: infiltrator_http::build_http_client(),
            provider: PublicIpProvider::Ipify,
        }
    }

    /// Android keeps the legacy location fields while still going through
    /// the shared `PublicIpProbe` port. Desktop/Iced use the smaller ipify
    /// response by default.
    pub fn with_geolocation_client() -> Self {
        Self {
            client: infiltrator_http::build_http_client(),
            provider: PublicIpProvider::IpApi,
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

        let (endpoint, provider) = match self.provider {
            PublicIpProvider::Ipify => ("https://api.ipify.org", "api.ipify.org"),
            PublicIpProvider::IpApi => ("https://ipapi.co/json/", "ipapi.co"),
        };
        let response = client
            .get(endpoint)
            .send()
            .await
            .map_err(|error| PortError::Network(error.to_string()))?;
        if !response.status().is_success() {
            return Err(PortError::Network(format!(
                "public IP provider returned HTTP {}",
                response.status()
            )));
        }
        let (ip, country, region, city) = match self.provider {
            PublicIpProvider::Ipify => {
                let body = response
                    .text()
                    .await
                    .map_err(|error| PortError::Network(error.to_string()))?;
                (body.trim().to_string(), None, None, None)
            }
            PublicIpProvider::IpApi => {
                let body = response
                    .json::<IpApiResponse>()
                    .await
                    .map_err(|error| PortError::Network(error.to_string()))?;
                (
                    body.ip.unwrap_or_default(),
                    body.country,
                    body.region,
                    body.city,
                )
            }
        };
        let ip = ip.trim();
        if ip.parse::<IpAddr>().is_err() {
            return Err(PortError::Failed(
                "public IP provider returned an invalid address".to_string(),
            ));
        }
        Ok(PublicIpSnapshot {
            ip: ip.to_string(),
            provider: provider.to_string(),
            checked_at_epoch_ms: Utc::now().timestamp_millis(),
            country,
            region,
            city,
        })
    }
}
