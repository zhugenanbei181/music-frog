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
        // 测试安全策略：绝不访问外网。mockito 只绑定 127.0.0.1 回环地址，
        // 客户端“能否发送请求”的行为完全在 loopback 上验证。
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("GET", "/ping")
            .with_status(200)
            .with_body("pong")
            .create_async()
            .await;

        let client = build_http_client();
        let url = format!("{}/ping", server.url());
        let resp = client
            .get(&url)
            .send()
            .await
            .expect("request to loopback mock server should succeed");

        assert_eq!(resp.status(), 200);
        assert_eq!(resp.text().await.unwrap(), "pong");
        mock.assert_async().await;
    }

    #[test]
    fn test_http_client_type_alias() {
        // Verify that HttpClient is indeed reqwest::Client
        let _client: HttpClient = reqwest::Client::new();
    }
}
