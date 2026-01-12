use std::time::Duration;

use log::warn;

pub use reqwest;
pub type HttpClient = reqwest::Client;

pub fn build_http_client() -> HttpClient {
    HttpClient::builder()
        .user_agent("MusicFrog-Despicable-Infiltrator")
        .timeout(Duration::from_secs(30))
        .build()
        .unwrap_or_else(|err| {
            warn!("failed to build http client: {err}");
            HttpClient::new()
        })
}

pub fn build_raw_http_client(default_client: &HttpClient) -> HttpClient {
    HttpClient::builder()
        .user_agent("MusicFrog-Despicable-Infiltrator")
        .timeout(Duration::from_secs(30))
        .no_gzip()
        .no_brotli()
        .no_deflate()
        .no_zstd()
        .build()
        .unwrap_or_else(|err| {
            warn!("failed to build raw http client: {err}");
            default_client.clone()
        })
}
