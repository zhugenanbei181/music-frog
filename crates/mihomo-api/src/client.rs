use crate::Proxy;
use crate::error::{MihomoError, Result};
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

                let mut request = match ws_url_str.as_str().into_client_request() {
                    Ok(req) => req,
                    Err(_) => break,
                };
                if let Some(s) = &secret {
                    request
                        .headers_mut()
                        .insert("Authorization", format!("Bearer {}", s).parse().unwrap());
                }

                if let Ok((ws_stream, _)) = connect_async(request).await {
                    backoff = Duration::from_secs(1);
                    let (_, mut read) = ws_stream.split();
                    while let Some(msg) = read.next().await {
                        match msg {
                            Ok(Message::Text(text)) => {
                                if let Some(item) = parse(text.as_ref())
                                    && tx.send(item).is_err()
                                {
                                    return;
                                }
                            }
                            Ok(Message::Close(_)) | Err(_) => break,
                            _ => {}
                        }
                    }
                }

                tokio::time::sleep(backoff).await;
                backoff = std::cmp::min(backoff * 2, Duration::from_secs(30));
            }
        });

        Ok(rx)
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
            if let Ok((stream, _)) = server.accept().await {
                if let Ok(mut ws_stream) = tokio_tungstenite::accept_async(stream).await {
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
            if let Ok((stream, _)) = server.accept().await {
                if let Ok(mut ws_stream) = tokio_tungstenite::accept_async(stream).await {
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
            }
        });

        let client = MihomoClient::new(&format!("http://{}", addr), None).unwrap();
        let mut rx = client.stream_connections().await.unwrap();

        let data = tokio::time::timeout(std::time::Duration::from_secs(1), rx.recv())
            .await
            .unwrap();
        assert!(data.is_some());
    }
}
