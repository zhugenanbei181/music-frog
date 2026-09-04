use crate::error::{MihomoError, Result};
use infiltrator_domain::proxy::Proxy;
use crate::types::*;
use futures_util::StreamExt;
use reqwest::Client;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::time::Duration;
use tokio_tungstenite::{
    connect_async,
    tungstenite::{Message, client::IntoClientRequest},
};
use url::Url;

/// Lifecycle-aware item emitted by a controller WebSocket stream. The
/// existing `stream_*` methods keep returning data-only receivers for API
/// compatibility; surfaces that need an honest connection badge use the
/// `stream_*_events` variants.
#[derive(Debug)]
pub enum StreamEvent<T> {
    Connecting,
    Connected,
    Item(T),
    Reconnecting(String),
    Failed(String),
}

#[derive(Clone)]
pub struct MihomoClient {
    client: Client,
    base_url: Url,
    secret: Option<String>,
}

impl MihomoClient {
    pub fn new(base_url: &str, secret: Option<String>) -> Result<Self> {
        let client = Client::builder().timeout(Duration::from_secs(10)).build()?;
        let base_url = Url::parse(base_url).map_err(|e| MihomoError::Config(e.to_string()))?;
        Ok(Self {
            client,
            base_url,
            secret,
        })
    }

    fn build_url(&self, path: &str) -> Result<Url> {
        self.base_url
            .join(path.trim_start_matches('/'))
            .map_err(|e| MihomoError::Config(e.to_string()))
    }

    fn build_url_with_query(&self, path: &str, query: &[(&str, String)]) -> Result<Url> {
        let mut url = self.build_url(path)?;
        url.query_pairs_mut().extend_pairs(query);
        Ok(url)
    }

    /// Joins `segments` onto the base URL, percent-encoding each one as a
    /// single opaque path segment.
    ///
    /// Unlike [`Self::build_url`] on a `format!`-ed path — where characters
    /// such as `?`, `#`, or `/` inside a proxy/provider/group name would be
    /// parsed as query/fragment/segment delimiters — this goes through
    /// `Url::path_segments_mut`, so a name like `"My Group #1"` is sent
    /// verbatim as one encoded segment (`My%20Group%20%231`).
    fn build_url_with_segments(&self, segments: &[&str]) -> Result<Url> {
        let mut url = self.base_url.clone();
        {
            let mut path = url
                .path_segments_mut()
                .map_err(|()| MihomoError::Config("base URL cannot be a base".to_string()))?;
            path.clear();
            for segment in segments {
                path.push(segment);
            }
        }
        Ok(url)
    }

    fn add_auth(&self, mut req: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        if let Some(secret) = &self.secret {
            req = req.bearer_auth(secret);
        }
        req
    }

    pub async fn get_version(&self) -> Result<Version> {
        let url = self.build_url("/version")?;
        let req = self.client.get(url);
        let req = self.add_auth(req);
        let resp = req.send().await?;
        Ok(resp.json().await?)
    }

    pub async fn get_config(&self) -> Result<ConfigResponse> {
        let url = self.build_url("/configs")?;
        let req = self.client.get(url);
        let req = self.add_auth(req);
        let resp = req.send().await?;
        Ok(resp.json().await?)
    }

    pub async fn get_rules(&self) -> Result<Vec<Rule>> {
        let url = self.build_url("/rules")?;
        let req = self.client.get(url);
        let req = self.add_auth(req);
        let resp = req.send().await?;
        let list: RuleList = resp.json().await?;
        Ok(list.rules)
    }

    pub async fn get_proxies(&self) -> Result<HashMap<String, Proxy>> {
        let url = self.build_url("/proxies")?;
        log::debug!("Fetching proxies from: {}", url);
        let req = self.client.get(url);
        let req = self.add_auth(req);
        let resp = req.send().await?;
        let data: ProxiesResponse = resp.json().await?;
        log::debug!("Received {} proxies", data.proxies.len());
        Ok(data.proxies)
    }

    pub async fn get_proxy(&self, name: &str) -> Result<Proxy> {
        let url = self.build_url(&format!("/proxies/{}", name))?;
        let req = self.client.get(url);
        let req = self.add_auth(req);
        let resp = req.send().await?;
        Ok(resp.json().await?)
    }

    pub async fn switch_proxy(&self, group: &str, name: &str) -> Result<()> {
        let url = self.build_url(&format!("/proxies/{}", group))?;
        let req = self.client.put(url).json(&json!({ "name": name }));
        let req = self.add_auth(req);
        req.send().await?;
        Ok(())
    }

    pub async fn test_delay(&self, name: &str, url: &str, timeout: u32) -> Result<u32> {
        let url = self.build_url_with_query(
            &format!("/proxies/{}/delay", name),
            &[("url", url.to_string()), ("timeout", timeout.to_string())],
        )?;
        let req = self.client.get(url);
        let req = self.add_auth(req);
        let resp = req.send().await?;
        let data: DelayTestResponse = resp.json().await?;
        Ok(data.delay)
    }

    pub async fn reload_config(&self, path: Option<&str>) -> Result<()> {
        let url = self.build_url_with_query("/configs", &[("force", "true".to_string())])?;
        let mut req = self.client.put(url);
        if let Some(path) = path {
            req = req.json(&json!({ "path": path }));
        }
        let req = self.add_auth(req);
        req.send().await?;
        Ok(())
    }

    pub async fn patch_config(&self, updates: Value) -> Result<()> {
        let url = self.build_url("/configs")?;
        let req = self.client.patch(url).json(&updates);
        let req = self.add_auth(req);
        req.send().await?;
        Ok(())
    }

    pub async fn get_proxy_providers(&self) -> Result<HashMap<String, ProxyProvider>> {
        let url = self.build_url("/providers/proxies")?;
        let req = self.client.get(url);
        let req = self.add_auth(req);
        let resp = req.send().await?;
        let list: ProxyProviderList = resp.json().await?;
        Ok(list.providers)
    }

    pub async fn get_rule_providers(&self) -> Result<HashMap<String, RuleProvider>> {
        let url = self.build_url("/providers/rules")?;
        let req = self.client.get(url);
        let req = self.add_auth(req);
        let resp = req.send().await?;
        let list: RuleProviderList = resp.json().await?;
        Ok(list.providers)
    }

    pub async fn update_proxy_provider(&self, name: &str) -> Result<()> {
        let url = self.build_url(&format!("/providers/proxies/{}", name))?;
        let req = self.client.put(url);
        let req = self.add_auth(req);
        req.send().await?;
        Ok(())
    }

    pub async fn update_rule_provider(&self, name: &str) -> Result<()> {
        let url = self.build_url(&format!("/providers/rules/{}", name))?;
        let req = self.client.put(url);
        let req = self.add_auth(req);
        req.send().await?;
        Ok(())
    }

    pub async fn flush_fakeip_cache(&self) -> Result<()> {
        let url = self.build_url("/cache/fakeip/flush")?;
        let req = self.client.post(url);
        let req = self.add_auth(req);
        req.send().await?;
        Ok(())
    }

    pub async fn get_dns_query(&self, name: &str, q_type: &str) -> Result<Value> {
        let mut url = self.build_url("/dns/query")?;
        url.query_pairs_mut()
            .append_pair("name", name)
            .append_pair("type", q_type);
        let req = self.client.get(url);
        let req = self.add_auth(req);
        let resp = req.send().await?;
        Ok(resp.json().await?)
    }

    pub async fn get_script(&self) -> Result<Value> {
        let url = self.build_url("/script")?;
        let req = self.client.get(url);
        let req = self.add_auth(req);
        let resp = req.send().await?;
        Ok(resp.json().await?)
    }

    async fn spawn_reconnecting_stream_events<T, F>(
        &self,
        endpoint: &str,
        query: Option<String>,
        parse: F,
    ) -> Result<tokio::sync::mpsc::UnboundedReceiver<StreamEvent<T>>>
    where
        T: Send + 'static,
        F: Fn(&str) -> Option<T> + Send + Sync + 'static,
    {
        let mut ws_url = self.base_url.clone();
        ws_url
            .set_scheme(if ws_url.scheme() == "https" {
                "wss"
            } else {
                "ws"
            })
            .ok();
        ws_url.set_path(endpoint);
        if let Some(q) = query {
            ws_url.set_query(Some(&q));
        }

        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        let secret = self.secret.clone();
        let ws_url_str = ws_url.to_string();

        tokio::spawn(async move {
            let mut backoff = Duration::from_secs(1);
            loop {
                if tx.is_closed() {
                    break;
                }

                let _ = tx.send(StreamEvent::Connecting);

                let mut request = match ws_url_str.as_str().into_client_request() {
                    Ok(req) => req,
                    Err(error) => {
                        let _ = tx.send(StreamEvent::Failed(error.to_string()));
                        break;
                    }
                };
                if let Some(s) = &secret {
                    request
                        .headers_mut()
                        .insert("Authorization", format!("Bearer {}", s).parse().unwrap());
                }

                let reconnect_reason = match connect_async(request).await {
                    Ok((ws_stream, _)) => {
                        backoff = Duration::from_secs(1);
                        let _ = tx.send(StreamEvent::Connected);
                        let (_, mut read) = ws_stream.split();
                        let mut reason = "controller stream closed".to_string();
                        loop {
                            tokio::select! {
                                message = read.next() => match message {
                                    Some(Ok(Message::Text(text))) => {
                                        if let Some(item) = parse(text.as_ref())
                                            && tx.send(StreamEvent::Item(item)).is_err()
                                        {
                                            return;
                                        }
                                    }
                                    Some(Ok(Message::Close(_))) | None => break,
                                    Some(Err(error)) => {
                                        reason = error.to_string();
                                        break;
                                    }
                                    Some(Ok(_)) => {}
                                },
                                _ = tx.closed() => return,
                            }
                        }
                        reason
                    }
                    Err(error) => error.to_string(),
                };

                if tx
                    .send(StreamEvent::Reconnecting(reconnect_reason))
                    .is_err()
                {
                    return;
                }
                if tx.is_closed() {
                    break;
                }
                tokio::time::sleep(backoff).await;
                backoff = std::cmp::min(backoff * 2, Duration::from_secs(30));
            }
        });

        Ok(rx)
    }

    async fn spawn_reconnecting_stream<T, F>(
        &self,
        endpoint: &str,
        query: Option<String>,
        parse: F,
    ) -> Result<tokio::sync::mpsc::UnboundedReceiver<T>>
    where
        T: Send + 'static,
        F: Fn(&str) -> Option<T> + Send + Sync + 'static,
    {
        let mut events = self
            .spawn_reconnecting_stream_events(endpoint, query, parse)
            .await?;
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        tokio::spawn(async move {
            while let Some(event) = events.recv().await {
                if let StreamEvent::Item(item) = event
                    && tx.send(item).is_err()
                {
                    break;
                }
            }
        });
        Ok(rx)
    }

    pub async fn stream_logs_events(
        &self,
        level: Option<&str>,
    ) -> Result<tokio::sync::mpsc::UnboundedReceiver<StreamEvent<String>>> {
        self.spawn_reconnecting_stream_events(
            "/logs",
            level.map(|l| format!("level={}", l)),
            |text| Some(text.to_string()),
        )
        .await
    }

    pub async fn stream_traffic_events(
        &self,
    ) -> Result<tokio::sync::mpsc::UnboundedReceiver<StreamEvent<TrafficData>>> {
        self.spawn_reconnecting_stream_events("/traffic", None, |text| {
            serde_json::from_str::<TrafficData>(text).ok()
        })
        .await
    }

    pub async fn stream_connections_events(
        &self,
    ) -> Result<tokio::sync::mpsc::UnboundedReceiver<StreamEvent<ConnectionSnapshot>>> {
        self.spawn_reconnecting_stream_events("/connections", None, |text| {
            serde_json::from_str::<ConnectionSnapshot>(text).ok()
        })
        .await
    }

    pub async fn stream_logs(
        &self,
        level: Option<&str>,
    ) -> Result<tokio::sync::mpsc::UnboundedReceiver<String>> {
        self.spawn_reconnecting_stream("/logs", level.map(|l| format!("level={}", l)), |text| {
            Some(text.to_string())
        })
        .await
    }

    pub async fn stream_traffic(
        &self,
    ) -> Result<tokio::sync::mpsc::UnboundedReceiver<TrafficData>> {
        self.spawn_reconnecting_stream("/traffic", None, |text| {
            serde_json::from_str::<TrafficData>(text).ok()
        })
        .await
    }

    pub async fn stream_connections(
        &self,
    ) -> Result<tokio::sync::mpsc::UnboundedReceiver<ConnectionSnapshot>> {
        self.spawn_reconnecting_stream("/connections", None, |text| {
            serde_json::from_str::<ConnectionSnapshot>(text).ok()
        })
        .await
    }

    pub async fn get_memory(&self) -> Result<MemoryData> {
        let url = self.build_url("/memory")?;
        let req = self.client.get(url);
        let req = self.add_auth(req);
        let resp = req.send().await?;
        Ok(resp.json().await?)
    }

    pub async fn get_connections(&self) -> Result<ConnectionsResponse> {
        let url = self.build_url("/connections")?;
        log::debug!("Fetching connections from: {}", url);
        let req = self.client.get(url);
        let req = self.add_auth(req);
        let resp = req.send().await?;
        let data: ConnectionsResponse = resp.json().await?;
        log::debug!("Received {} connections", data.connections.len());
        Ok(data)
    }

    pub async fn close_connection(&self, id: &str) -> Result<()> {
        let url = self.build_url(&format!("/connections/{}", id))?;
        let req = self.client.delete(url);
        let req = self.add_auth(req);
        req.send().await?;
        Ok(())
    }

    pub async fn close_all_connections(&self) -> Result<()> {
        let url = self.build_url("/connections")?;
        let req = self.client.delete(url);
        let req = self.add_auth(req);
        req.send().await?;
        Ok(())
    }

    /// Restarts the mihomo core (`POST /restart`, empty body): mihomo
    /// reloads its in-process configuration and then restarts itself.
    ///
    /// The endpoint answers with an empty 204 or a JSON body depending on
    /// version, so — following the fire-and-forget style of the other
    /// command methods here — any success response is treated as `Ok(())`
    /// without inspecting the body.
    ///
    /// Note: `POST /upgrade` (core self-upgrade) is intentionally NOT
    /// wrapped by this client; see the `MihomoApi` trait docs in
    /// `crate::api` for the UP-001 rationale.
    pub async fn restart_core(&self) -> Result<()> {
        let url = self.build_url("/restart")?;
        let req = self.client.post(url);
        let req = self.add_auth(req);
        req.send().await?;
        Ok(())
    }

    /// Triggers an on-demand health check for one proxy provider
    /// (`GET /providers/proxies/{provider}/healthcheck`).
    ///
    /// The provider name is percent-encoded as a single path segment, so
    /// names containing spaces, `?`, `#`, `/`, or other reserved characters
    /// reach mihomo verbatim instead of being split into extra path/query
    /// parts.
    pub async fn provider_healthcheck(&self, provider: &str) -> Result<()> {
        let url =
            self.build_url_with_segments(&["providers", "proxies", provider, "healthcheck"])?;
        let req = self.client.get(url);
        let req = self.add_auth(req);
        req.send().await?;
        Ok(())
    }

    /// Returns the current FakeIP cache table (`GET /cache/fakeip`).
    ///
    /// mihomo answers with a JSON object whose keys are the dynamically
    /// cached domain names, so the payload is returned as a raw
    /// [`serde_json::Value`] to preserve its original shape instead of
    /// guessing a fixed struct.
    pub async fn fakeip_cache(&self) -> Result<Value> {
        let url = self.build_url("/cache/fakeip")?;
        let req = self.client.get(url);
        let req = self.add_auth(req);
        let resp = req.send().await?;
        Ok(resp.json().await?)
    }

    /// Runs a delay test against every proxy in a group
    /// (`GET /group/{group}/delay?url={url}&timeout={timeout_ms}`).
    ///
    /// The response maps proxy names to their measured delay in
    /// milliseconds, but a proxy that failed the test yields an error
    /// string (e.g. `"An error occurred in the delay test"`) instead of a
    /// number, so values are kept as [`serde_json::Value`]; callers must
    /// handle both `Value::Number` and `Value::String`.
    pub async fn group_delay(
        &self,
        group: &str,
        url: &str,
        timeout_ms: u32,
    ) -> Result<HashMap<String, Value>> {
        let mut endpoint = self.build_url_with_segments(&["group", group, "delay"])?;
        endpoint
            .query_pairs_mut()
            .append_pair("url", url)
            .append_pair("timeout", &timeout_ms.to_string());
        let req = self.client.get(endpoint);
        let req = self.add_auth(req);
        let resp = req.send().await?;
        Ok(resp.json().await?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mockito::Server;

    #[tokio::test]
    async fn test_client_new() {
        let client = MihomoClient::new("http://127.0.0.1:9090", None);
        assert!(client.is_ok());
    }

    #[tokio::test]
    async fn test_get_version() {
        let mut server = Server::new_async().await;
        let body = json!({
            "version": "v1.18.0",
            "premium": false
        });

        let mock = server
            .mock("GET", "/version")
            .with_status(200)
            .with_body(serde_json::to_string(&body).unwrap())
            .create_async()
            .await;

        let client = MihomoClient::new(&server.url(), None).unwrap();
        let version = client.get_version().await.unwrap();

        mock.assert_async().await;
        assert_eq!(version.version, "v1.18.0");
        assert!(!version.premium);
    }

    #[tokio::test]
    async fn test_get_proxies() {
        let mut server = Server::new_async().await;
        let body = json!({
            "proxies": {
                "GLOBAL": {
                    "type": "Selector",
                    "name": "GLOBAL",
                    "now": "Proxy-A",
                    "all": ["Proxy-A", "Proxy-B"],
                    "history": []
                },
                "Proxy-A": {
                    "type": "Shadowsocks",
                    "name": "Proxy-A",
                    "udp": true,
                    "history": [],
                    "alive": true,
                    "server": "1.1.1.1",
                    "port": 443,
                    "cipher": "aes-256-gcm"
                }
            }
        });

        let mock = server
            .mock("GET", "/proxies")
            .with_status(200)
            .with_body(serde_json::to_string(&body).unwrap())
            .create_async()
            .await;

        let client = MihomoClient::new(&server.url(), None).unwrap();
        let proxies = client.get_proxies().await.unwrap();

        mock.assert_async().await;
        assert!(proxies.contains_key("GLOBAL"));
        assert!(proxies.contains_key("Proxy-A"));
    }

    #[tokio::test]
    async fn test_switch_proxy() {
        let mut server = Server::new_async().await;
        let mock = server
            .mock("PUT", "/proxies/GLOBAL")
            .with_status(204)
            .create_async()
            .await;

        let client = MihomoClient::new(&server.url(), None).unwrap();
        let result = client.switch_proxy("GLOBAL", "Proxy-B").await;

        mock.assert_async().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_stream_traffic() {
        let addr = "127.0.0.1:19090";
        let server = tokio::net::TcpListener::bind(addr).await.unwrap();

        tokio::spawn(async move {
            use futures_util::SinkExt;
            if let Ok((stream, _)) = server.accept().await
                && let Ok(mut ws_stream) = tokio_tungstenite::accept_async(stream).await
            {
                let traffic = json!({
                    "up": 1024,
                    "down": 2048
                });
                let _ = ws_stream
                    .send(Message::Text(
                        serde_json::to_string(&traffic).unwrap().into(),
                    ))
                    .await;
            }
        });

        let client = MihomoClient::new(&format!("http://{}", addr), None).unwrap();
        let mut rx = client.stream_traffic().await.unwrap();

        let data = tokio::time::timeout(std::time::Duration::from_secs(1), rx.recv())
            .await
            .unwrap();
        assert!(data.is_some());
        let traffic = data.unwrap();
        assert_eq!(traffic.up, 1024);
        assert_eq!(traffic.down, 2048);
    }

    #[tokio::test]
    async fn test_stream_connections() {
        let addr = "127.0.0.1:19091";
        let server = tokio::net::TcpListener::bind(addr).await.unwrap();

        tokio::spawn(async move {
            use futures_util::SinkExt;
            if let Ok((stream, _)) = server.accept().await
                && let Ok(mut ws_stream) = tokio_tungstenite::accept_async(stream).await
            {
                let snapshot = json!({
                    "downloadTotal": 1000,
                    "uploadTotal": 2000,
                    "connections": []
                });
                let _ = ws_stream
                    .send(Message::Text(
                        serde_json::to_string(&snapshot).unwrap().into(),
                    ))
                    .await;
            }
        });

        let client = MihomoClient::new(&format!("http://{}", addr), None).unwrap();
        let mut rx = client.stream_connections().await.unwrap();

        let data = tokio::time::timeout(std::time::Duration::from_secs(1), rx.recv())
            .await
            .unwrap();
        assert!(data.is_some());
    }

    #[tokio::test]
    async fn test_stream_events_expose_lifecycle_and_data() {
        let server = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = server.local_addr().unwrap();
        tokio::spawn(async move {
            use futures_util::SinkExt;
            if let Ok((stream, _)) = server.accept().await
                && let Ok(mut ws_stream) = tokio_tungstenite::accept_async(stream).await
            {
                let traffic = TrafficData { up: 11, down: 22 };
                let _ = ws_stream
                    .send(Message::Text(
                        serde_json::to_string(&traffic).unwrap().into(),
                    ))
                    .await;
                let _ = ws_stream.close(None).await;
            }
        });

        let client = MihomoClient::new(&format!("http://{addr}"), None).unwrap();
        let mut rx = client.stream_traffic_events().await.unwrap();
        let mut saw_connecting = false;
        let mut saw_connected = false;
        let mut saw_item = false;
        for _ in 0..6 {
            let event = tokio::time::timeout(std::time::Duration::from_secs(1), rx.recv())
                .await
                .unwrap();
            let Some(event) = event else { break };
            match event {
                StreamEvent::Connecting => saw_connecting = true,
                StreamEvent::Connected => saw_connected = true,
                StreamEvent::Item(data) => {
                    assert_eq!(data.down, 22);
                    saw_item = true;
                    break;
                }
                StreamEvent::Reconnecting(_) | StreamEvent::Failed(_) => {}
            }
        }
        assert!(saw_connecting);
        assert!(saw_connected);
        assert!(saw_item);
    }

    #[tokio::test]
    async fn test_restart_core() {
        let mut server = Server::new_async().await;
        let mock = server
            .mock("POST", "/restart")
            .match_header("authorization", "Bearer test-secret")
            .with_status(204)
            .create_async()
            .await;

        let client = MihomoClient::new(&server.url(), Some("test-secret".to_string())).unwrap();
        let result = client.restart_core().await;

        mock.assert_async().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_restart_core_accepts_json_response() {
        let mut server = Server::new_async().await;
        let mock = server
            .mock("POST", "/restart")
            .with_status(200)
            .with_body(json!({ "message": "restarting" }).to_string())
            .create_async()
            .await;

        let client = MihomoClient::new(&server.url(), None).unwrap();
        let result = client.restart_core().await;

        mock.assert_async().await;
        assert!(result.is_ok(), "a JSON success body must also be Ok(())");
    }

    #[tokio::test]
    async fn test_provider_healthcheck() {
        let mut server = Server::new_async().await;
        let mock = server
            .mock("GET", "/providers/proxies/My%20Provider/healthcheck")
            .match_header("authorization", "Bearer test-secret")
            .with_status(204)
            .create_async()
            .await;

        let client = MihomoClient::new(&server.url(), Some("test-secret".to_string())).unwrap();
        let result = client.provider_healthcheck("My Provider").await;

        mock.assert_async().await;
        assert!(result.is_ok());
    }

    #[test]
    fn test_provider_name_is_percent_encoded_as_path_segment() {
        let client = MihomoClient::new("http://127.0.0.1:9090", None).unwrap();
        let url = client
            .build_url_with_segments(&["providers", "proxies", "My Provider #1?", "healthcheck"])
            .unwrap();
        assert_eq!(
            url.as_str(),
            "http://127.0.0.1:9090/providers/proxies/My%20Provider%20%231%3F/healthcheck",
            "space/#/? must be encoded inside the provider segment, not split off"
        );
    }

    #[tokio::test]
    async fn test_flush_fakeip_cache() {
        let mut server = Server::new_async().await;
        let mock = server
            .mock("POST", "/cache/fakeip/flush")
            .match_header("authorization", "Bearer test-secret")
            .with_status(204)
            .create_async()
            .await;

        let client = MihomoClient::new(&server.url(), Some("test-secret".to_string())).unwrap();
        let result = client.flush_fakeip_cache().await;

        mock.assert_async().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_fakeip_cache() {
        let mut server = Server::new_async().await;
        let body = json!({
            "music.example.org": "198.18.0.5",
            "cdn.example.net": "198.18.0.7"
        });
        let mock = server
            .mock("GET", "/cache/fakeip")
            .match_header("authorization", "Bearer test-secret")
            .with_status(200)
            .with_body(serde_json::to_string(&body).unwrap())
            .create_async()
            .await;

        let client = MihomoClient::new(&server.url(), Some("test-secret".to_string())).unwrap();
        let cache = client.fakeip_cache().await.unwrap();

        mock.assert_async().await;
        // Keys are dynamic domain names; Value keeps the original shape.
        assert_eq!(
            cache.get("music.example.org").and_then(|v| v.as_str()),
            Some("198.18.0.5")
        );
        assert!(cache.is_object());
    }

    #[tokio::test]
    async fn test_group_delay() {
        let mut server = Server::new_async().await;
        let body = json!({
            "Proxy-A": 123,
            "Proxy-B": "An error occurred in the delay test"
        });
        let mock = server
            .mock("GET", "/group/My%20Group/delay")
            .match_query("url=https%3A%2F%2Fexample.com%2Fclash&timeout=5000")
            .match_header("authorization", "Bearer test-secret")
            .with_status(200)
            .with_body(serde_json::to_string(&body).unwrap())
            .create_async()
            .await;

        let client = MihomoClient::new(&server.url(), Some("test-secret".to_string())).unwrap();
        let delays = client
            .group_delay("My Group", "https://example.com/clash", 5000)
            .await
            .unwrap();

        mock.assert_async().await;
        // Numeric delay for a reachable node...
        assert_eq!(delays.get("Proxy-A").and_then(|v| v.as_u64()), Some(123));
        // ...and an error string for one that failed the test.
        assert_eq!(
            delays.get("Proxy-B").and_then(|v| v.as_str()),
            Some("An error occurred in the delay test")
        );
    }

    #[test]
    fn test_group_name_is_percent_encoded_in_delay_url() {
        let client = MihomoClient::new("http://127.0.0.1:9090", None).unwrap();
        let mut url = client
            .build_url_with_segments(&["group", "My Group #1", "delay"])
            .unwrap();
        url.query_pairs_mut()
            .append_pair("url", "https://example.com/ping")
            .append_pair("timeout", "3000");
        assert_eq!(
            url.as_str(),
            "http://127.0.0.1:9090/group/My%20Group%20%231/delay?url=https%3A%2F%2Fexample.com%2Fping&timeout=3000",
            "group segment must be encoded while query pairs use form encoding"
        );
    }
}
