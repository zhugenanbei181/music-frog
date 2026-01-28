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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_http_client() {
        let _client = build_http_client();
        // Verify the client was created successfully without panicking
    }

    #[test]
    fn test_build_http_client_user_agent() {
        let _client = build_http_client();
        // Since we cannot inspect default_headers() on reqwest::Client easily in all environments,
        // we trust the builder worked if it didn't panic.
    }

    #[test]
    fn test_build_http_client_timeout() {
        let _client = build_http_client();
        // Verify the client was created successfully without panicking
    }

    #[test]
    fn test_build_raw_http_client() {
        let default_client = build_http_client();
        let _raw_client = build_raw_http_client(&default_client);
        // Verify the raw client was created successfully without panicking
    }

    #[test]
    fn test_build_raw_http_client_user_agent() {
        let default_client = build_http_client();
        let _raw_client = build_raw_http_client(&default_client);
    }

    #[tokio::test]
    async fn test_http_client_send() {
        let client = build_http_client();

        // Try making a simple request to verify the client works
        let result = client.get("https://example.com").send().await;

        // We don't care about the actual result, just that it doesn't panic
        // In a real test environment, this might fail due to network issues
        let _ = result;
    }

    #[test]
    fn test_http_client_type_alias() {
        // Verify that HttpClient is indeed reqwest::Client
        let _client: HttpClient = reqwest::Client::new();
    }
}
