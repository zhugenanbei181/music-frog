//! Host-side client for the embedded loopback Admin API.

use reqwest::Client;
use serde::Serialize;
use serde::de::DeserializeOwned;

pub struct AdminApiClient {
    base_url: String,
    client: Client,
}

impl AdminApiClient {
    pub fn new(base_url: impl Into<String>) -> anyhow::Result<Self> {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()?;
        Ok(Self {
            base_url: base_url.into().trim_end_matches('/').to_string(),
            client,
        })
    }

    pub async fn get<T: DeserializeOwned>(&self, path: &str) -> anyhow::Result<T> {
        self.client
            .get(self.url(path))
            .send()
            .await?
            .error_for_status()?
            .json::<T>()
            .await
            .map_err(Into::into)
    }

    pub async fn post<T: DeserializeOwned, B: Serialize>(
        &self,
        path: &str,
        body: &B,
    ) -> anyhow::Result<T> {
        self.client
            .post(self.url(path))
            .json(body)
            .send()
            .await?
            .error_for_status()?
            .json::<T>()
            .await
            .map_err(Into::into)
    }

    fn url(&self, path: &str) -> String {
        format!("{}/{}", self.base_url, path.trim_start_matches('/'))
    }
}
