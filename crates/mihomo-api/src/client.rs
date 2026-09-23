use crate::error::{MihomoError, Result};
use crate::types::*;
use infiltrator_domain::proxy::Proxy;
use reqwest::Client;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::time::Duration;
use url::Url;

mod stream;

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
        req.send().await?.error_for_status()?;
        Ok(())
    }

    pub async fn patch_config(&self, updates: Value) -> Result<()> {
        let url = self.build_url("/configs")?;
        let req = self.client.patch(url).json(&updates);
        let req = self.add_auth(req);
        req.send().await?.error_for_status()?;
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

    /// DUAL-11-06: the payload the kernel publishes for one rule provider.
    ///
    /// mihomo's `/providers/rules` handler serialises `payload` only for
    /// `type: inline` providers (`Provider` structs built from a vehicle use
    /// `payload,omitempty` and never fill it), and `/providers/rules/{name}`
    /// is `PUT`-only. `Ok(None)` therefore means the kernel genuinely does not
    /// publish this provider's rules; the caller must not invent them.
    pub async fn get_rule_provider_payload(&self, name: &str) -> Result<Option<Vec<String>>> {
        let url = self.build_url("/providers/rules")?;
        let req = self.client.get(url);
        let req = self.add_auth(req);
        let resp = req.send().await?;
        let list: RuleProviderList = resp.json().await?;
        Ok(list
            .providers
            .get(name)
            .map(|provider| provider.payload.clone())
            .filter(|payload| !payload.is_empty()))
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

    pub async fn get_memory(&self) -> Result<MemoryData> {
        let url = self.build_url("/memory")?;
        let req = self.client.get(url);
        let req = self.add_auth(req);
        let resp = req.send().await?;
        Ok(resp.json().await?)
    }

    /// Trigger Mihomo's active garbage collector.
    pub async fn trigger_gc(&self) -> Result<()> {
        let url = self.build_url("/debug/gc")?;
        let req = self.add_auth(self.client.put(url));
        req.send().await?.error_for_status()?;
        Ok(())
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

    /// Triggers the GeoIP/GeoSite database update inside the running core
    /// (`POST /upgrade/geo`).
    ///
    /// mihomo performs the actual download asynchronously; a success response
    /// means the core accepted the trigger, not that fresh databases are
    /// already installed. Following the fire-and-forget style of the other
    /// command methods here, any success response is treated as `Ok(())`
    /// without inspecting the body.
    pub async fn upgrade_geo(&self) -> Result<()> {
        let url = self.build_url("/upgrade/geo")?;
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
#[path = "client_auth_test.rs"]
mod auth_tests;

#[cfg(test)]
#[path = "client_tests.rs"]
mod tests;
