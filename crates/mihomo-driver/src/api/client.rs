use crate::api::proxy::{ProxiesResponse, ProxyObservation};
use crate::error::{MihomoError, Result};
use crate::types::*;
use lurker_core_contract::error::Failure;
use reqwest::Client;
use reqwest::redirect::Policy;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use url::{Host, Url};

/// The largest delay timeout the Mihomo controller accepts, in milliseconds.
///
/// The controller wire type is a signed 16-bit integer, so a larger value is
/// refused before the request is sent.
pub const MAX_MIHOMO_DELAY_TIMEOUT_MS: u32 = i16::MAX as u32;

pub(crate) fn validate_mihomo_delay_timeout(timeout_ms: u32) -> Result<()> {
    if timeout_ms == 0 || timeout_ms > MAX_MIHOMO_DELAY_TIMEOUT_MS {
        return Err(MihomoError::Config(format!(
            "Mihomo delay timeout must be in 1..={MAX_MIHOMO_DELAY_TIMEOUT_MS} milliseconds"
        )));
    }
    Ok(())
}

/// Lifecycle-aware item emitted by a controller WebSocket stream.
#[derive(Debug)]
pub enum StreamEvent<T> {
    /// The stream is establishing its WebSocket connection.
    Connecting,
    /// The stream is connected and may deliver items.
    Connected,
    /// One item the controller published.
    Item(T),
    /// The stream lost its connection and is reconnecting.
    Reconnecting(Failure),
    /// The stream ended; no further item will arrive.
    Terminal(StreamTerminal),
}

/// Terminal outcome of a controller WebSocket stream.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StreamTerminal {
    /// The caller cancelled the stream.
    Cancelled,
    /// The stream failed and will not be retried.
    Failed(Failure),
    /// The controller does not support this stream.
    Unsupported(Failure),
}

/// The controller is the engine's loopback admin API.
///
/// Accepting any other URL would send the controller secret in cleartext to a
/// LAN or internet host, and accepting a hostname would reopen DNS rebinding.
/// Only an explicit `http` URL whose host is an IP literal in `127.0.0.0/8` or
/// `::1`, with a non-zero port and no userinfo/path/query/fragment, is valid.
pub(crate) fn validate_loopback_controller(base_url: &Url) -> Result<()> {
    let loopback_host = match base_url.host() {
        Some(Host::Ipv4(address)) => address.is_loopback(),
        Some(Host::Ipv6(address)) => address.is_loopback(),
        Some(Host::Domain(_)) | None => false,
    };
    if base_url.scheme() != "http"
        || !loopback_host
        || !base_url.username().is_empty()
        || base_url.password().is_some()
        || (base_url.path() != "/" && !base_url.path().is_empty())
        || base_url.query().is_some()
        || base_url.fragment().is_some()
        || base_url.port().is_none()
        || base_url.port() == Some(0)
    {
        return Err(MihomoError::Config(
            "Mihomo controller must be an explicit HTTP loopback endpoint".to_owned(),
        ));
    }
    Ok(())
}

/// The Mihomo controller client.
///
/// The client only ever talks to an explicit HTTP loopback endpoint: any
/// other URL would send the controller secret in cleartext to a LAN or
/// internet host.
#[derive(Clone)]
pub struct MihomoClient {
    pub(crate) client: Client,
    pub(crate) base_url: Url,
    pub(crate) secret: Option<String>,
    configuration_write_lock: Arc<Mutex<()>>,
}

impl MihomoClient {
    /// Creates a controller client for one loopback endpoint.
    ///
    /// Fails with a `Config` error when the endpoint is not an explicit HTTP
    /// loopback URL with a non-zero port.
    pub fn new(base_url: &str, secret: Option<String>) -> Result<Self> {
        let base_url = Url::parse(base_url).map_err(|e| MihomoError::Config(e.to_string()))?;
        validate_loopback_controller(&base_url)?;
        let client = Client::builder()
            .timeout(Duration::from_secs(10))
            // The controller is loopback-only: an ambient HTTP proxy must never
            // take over the request and observe the controller secret.
            .no_proxy()
            // Never follow redirects: a squatter on the controller port could
            // otherwise redirect a bearer-authenticated request off-host.
            .redirect(Policy::none())
            .build()?;
        Ok(Self {
            client,
            base_url,
            secret,
            configuration_write_lock: Arc::new(Mutex::new(())),
        })
    }

    /// Shared configuration-write serialization for one controller endpoint.
    /// Product configuration ports that must not interleave patches clone this
    /// lock from the same client.
    pub fn configuration_write_lock(&self) -> Arc<Mutex<()>> {
        self.configuration_write_lock.clone()
    }

    pub(crate) fn build_url(&self, path: &str) -> Result<Url> {
        self.base_url
            .join(path.trim_start_matches('/'))
            .map_err(|e| MihomoError::Config(e.to_string()))
    }

    pub(crate) fn build_url_with_query(&self, path: &str, query: &[(&str, String)]) -> Result<Url> {
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
    pub(crate) fn build_url_with_segments(&self, segments: &[&str]) -> Result<Url> {
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

    pub(crate) fn add_auth(&self, mut req: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        if let Some(secret) = &self.secret {
            req = req.bearer_auth(secret);
        }
        req
    }

    /// Reads the version of the running core.
    pub async fn get_version(&self) -> Result<Version> {
        let url = self.build_url("/version")?;
        let req = self.client.get(url);
        let req = self.add_auth(req);
        let resp = req.send().await?.error_for_status()?;
        Ok(resp.json().await?)
    }

    /// Reads the effective configuration of the running core.
    pub async fn get_config(&self) -> Result<ConfigResponse> {
        let url = self.build_url("/configs")?;
        let req = self.client.get(url);
        let req = self.add_auth(req);
        let resp = req.send().await?.error_for_status()?;
        Ok(resp.json().await?)
    }

    /// Reads the routing rules of the running core, in controller order.
    pub async fn get_rules(&self) -> Result<Vec<Rule>> {
        let url = self.build_url("/rules")?;
        let req = self.client.get(url);
        let req = self.add_auth(req);
        let resp = req.send().await?.error_for_status()?;
        let list: RuleList = resp.json().await?;
        Ok(list.rules)
    }

    /// Reads every outbound the controller knows, keyed by its name.
    pub async fn get_proxies(&self) -> Result<HashMap<String, ProxyObservation>> {
        let url = self.build_url("/proxies")?;
        log::debug!("Fetching proxies from: {}", url);
        let req = self.client.get(url);
        let req = self.add_auth(req);
        let resp = req.send().await?.error_for_status()?;
        let data: ProxiesResponse = resp.json().await?;
        log::debug!("Received {} proxies", data.proxies.len());
        Ok(data.proxies)
    }

    /// Selects `name` as the active member of the group `group`.
    pub async fn switch_proxy(&self, group: &str, name: &str) -> Result<()> {
        let url = self.build_url_with_segments(&["proxies", group])?;
        let req = self.client.put(url).json(&json!({ "name": name }));
        let req = self.add_auth(req);
        req.send().await?.error_for_status()?;
        Ok(())
    }

    /// Measures the delay of one outbound against `url`.
    ///
    /// Fails with a `Config` error when the timeout is zero or above
    /// [`MAX_MIHOMO_DELAY_TIMEOUT_MS`].
    pub async fn test_delay(&self, name: &str, url: &str, timeout: u32) -> Result<u32> {
        validate_mihomo_delay_timeout(timeout)?;
        let mut request_url = self.build_url_with_segments(&["proxies", name, "delay"])?;
        request_url
            .query_pairs_mut()
            .append_pair("url", url)
            .append_pair("timeout", &timeout.to_string());
        let req = self.client.get(request_url);
        let req = self.add_auth(req);
        let resp = req.send().await?.error_for_status()?;
        let data: DelayTestResponse = resp.json().await?;
        Ok(data.delay)
    }

    /// Reloads the configuration of the running core.
    ///
    /// Without a path the core reloads the configuration it was started with.
    pub async fn reload_config(&self, path: Option<&str>) -> Result<()> {
        let url = self.build_url_with_query("/configs", &[("force", "true".to_owned())])?;
        let mut req = self.client.put(url);
        if let Some(path) = path {
            req = req.json(&json!({ "path": path }));
        }
        let req = self.add_auth(req);
        req.send().await?.error_for_status()?;
        Ok(())
    }

    /// Applies a partial configuration patch to the running core.
    pub async fn patch_config(&self, updates: Value) -> Result<()> {
        let url = self.build_url("/configs")?;
        let req = self.client.patch(url).json(&updates);
        let req = self.add_auth(req);
        req.send().await?.error_for_status()?;
        Ok(())
    }

    /// Reads every rule provider, keyed by its name.
    pub async fn get_rule_providers(&self) -> Result<HashMap<String, RuleProvider>> {
        let url = self.build_url("/providers/rules")?;
        let req = self.client.get(url);
        let req = self.add_auth(req);
        let resp = req.send().await?.error_for_status()?;
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
        let resp = req.send().await?.error_for_status()?;
        let list: RuleProviderList = resp.json().await?;
        Ok(list
            .providers
            .get(name)
            .map(|provider| provider.payload.clone())
            .filter(|payload| !payload.is_empty()))
    }

    /// Triggers an update of one rule provider.
    pub async fn update_rule_provider(&self, name: &str) -> Result<()> {
        let url = self.build_url_with_segments(&["providers", "rules", name])?;
        let req = self.client.put(url);
        let req = self.add_auth(req);
        req.send().await?.error_for_status()?;
        Ok(())
    }

    /// Flushes the FakeIP cache of the running core.
    pub async fn flush_fakeip_cache(&self) -> Result<()> {
        let url = self.build_url("/cache/fakeip/flush")?;
        let req = self.client.post(url);
        let req = self.add_auth(req);
        req.send().await?.error_for_status()?;
        Ok(())
    }

    /// Resolves one DNS name through the resolver of the running core.
    pub async fn get_dns_query(&self, name: &str, q_type: &str) -> Result<Value> {
        let mut url = self.build_url("/dns/query")?;
        url.query_pairs_mut()
            .append_pair("name", name)
            .append_pair("type", q_type);
        let req = self.client.get(url);
        let req = self.add_auth(req);
        let resp = req.send().await?.error_for_status()?;
        Ok(resp.json().await?)
    }

    /// Reads the script configuration of the running core.
    pub async fn get_script(&self) -> Result<Value> {
        let url = self.build_url("/script")?;
        let req = self.client.get(url);
        let req = self.add_auth(req);
        let resp = req.send().await?.error_for_status()?;
        Ok(resp.json().await?)
    }

    /// Trigger Mihomo's active garbage collector.
    pub async fn trigger_gc(&self) -> Result<()> {
        let url = self.build_url("/debug/gc")?;
        let req = self.add_auth(self.client.put(url));
        req.send().await?.error_for_status()?;
        Ok(())
    }

    /// Reads the observed connections and the traffic counters.
    pub async fn get_connections(&self) -> Result<ConnectionsResponse> {
        let url = self.build_url("/connections")?;
        log::debug!("Fetching connections from: {}", url);
        let req = self.client.get(url);
        let req = self.add_auth(req);
        let resp = req.send().await?.error_for_status()?;
        let data: ConnectionsResponse = resp.json().await?;
        log::debug!("Received {} connections", data.connections.len());
        Ok(data)
    }

    /// Closes one observed connection.
    pub async fn close_connection(&self, id: &str) -> Result<()> {
        let url = self.build_url(&format!("/connections/{}", id))?;
        let req = self.client.delete(url);
        let req = self.add_auth(req);
        req.send().await?.error_for_status()?;
        Ok(())
    }

    /// Closes every observed connection.
    pub async fn close_all_connections(&self) -> Result<()> {
        let url = self.build_url("/connections")?;
        let req = self.client.delete(url);
        let req = self.add_auth(req);
        req.send().await?.error_for_status()?;
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
    /// Note: `POST /upgrade` (core self-upgrade) is intentionally not exposed;
    /// Core delivery stays in the versioned download and verification flow.
    pub async fn restart_core(&self) -> Result<()> {
        let url = self.build_url("/restart")?;
        let req = self.client.post(url);
        let req = self.add_auth(req);
        req.send().await?.error_for_status()?;
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
        req.send().await?.error_for_status()?;
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
        req.send().await?.error_for_status()?;
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
        let resp = req.send().await?.error_for_status()?;
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
        let resp = req.send().await?.error_for_status()?;
        Ok(resp.json().await?)
    }
}
