//! Object-safe engine API seam used by adapter tests and outbound discovery.
//!
//! Connection list and close operations intentionally do not appear here.
//! Production code accesses them only through the scope- and revision-bound
//! `ConnectionPort`; the raw controller calls remain private to this crate.
//! Tests can provide an in-memory implementation without starting Mihomo.

use crate::client::MihomoClient;
use crate::error::Result;
use crate::types::*;
use async_trait::async_trait;
use infiltrator_domain::proxy::Proxy;
use serde_json::Value;
use std::collections::HashMap;

#[async_trait]
pub trait MihomoApi: Send + Sync {
    async fn get_version(&self) -> Result<Version>;
    async fn get_config(&self) -> Result<ConfigResponse>;
    async fn get_rules(&self) -> Result<Vec<Rule>>;
    async fn get_proxies(&self) -> Result<HashMap<String, Proxy>>;
    async fn get_proxy(&self, name: &str) -> Result<Proxy>;
    async fn switch_proxy(&self, group: &str, name: &str) -> Result<()>;
    async fn test_delay(&self, name: &str, url: &str, timeout: u32) -> Result<u32>;
    async fn reload_config(&self, path: Option<&str>) -> Result<()>;
    async fn patch_config(&self, updates: Value) -> Result<()>;
    async fn get_proxy_providers(&self) -> Result<HashMap<String, ProxyProvider>>;
    async fn get_rule_providers(&self) -> Result<HashMap<String, RuleProvider>>;
    async fn update_proxy_provider(&self, name: &str) -> Result<()>;
    async fn update_rule_provider(&self, name: &str) -> Result<()>;
    async fn get_dns_query(&self, name: &str, q_type: &str) -> Result<Value>;
    async fn get_script(&self) -> Result<Value>;
    /// Restarts the mihomo core (`POST /restart`): mihomo reloads its
    /// in-process configuration and then restarts itself.
    ///
    /// Deliberately **no** counterpart exists for `POST /upgrade` (core
    /// self-upgrade): a mihomo self-upgrade would download and replace the
    /// core binary outside this project's kernel digest verification chain
    /// (UP-001), so it is a forbidden delivery path here. Core updates must
    /// go through the `mihomo-version` download/verify flow instead; that is
    /// why this seam only offers the restart command.
    async fn restart_core(&self) -> Result<()>;

    /// Triggers the GeoIP/GeoSite database update inside the running core
    /// (`POST /upgrade/geo`).
    ///
    /// This only refreshes the geo rule databases; it is unrelated to the
    /// forbidden `POST /upgrade` core self-upgrade path (UP-001). mihomo
    /// performs the actual download asynchronously, so success means the
    /// trigger was accepted, not that fresh databases are installed yet.
    async fn upgrade_geo(&self) -> Result<()>;

    /// Triggers an on-demand health check for one proxy provider
    /// (`GET /providers/proxies/{provider}/healthcheck`).
    async fn provider_healthcheck(&self, provider: &str) -> Result<()>;

    /// Returns the FakeIP cache table (`GET /cache/fakeip`) as raw JSON.
    ///
    /// mihomo keys the response map by dynamically cached domain names, so
    /// the payload is kept as [`serde_json::Value`] to preserve its original
    /// shape.
    async fn fakeip_cache(&self) -> Result<Value>;

    /// Runs a delay test against every proxy in a group
    /// (`GET /group/{group}/delay?url={url}&timeout={timeout_ms}`).
    ///
    /// Map values are the measured delays in milliseconds, but a proxy that
    /// failed the test yields an error string (for example
    /// `"An error occurred in the delay test"`) instead of a number — hence
    /// [`serde_json::Value`]; callers must handle both variants.
    async fn group_delay(
        &self,
        group: &str,
        url: &str,
        timeout_ms: u32,
    ) -> Result<HashMap<String, Value>>;
}

#[async_trait]
impl MihomoApi for MihomoClient {
    async fn get_version(&self) -> Result<Version> {
        MihomoClient::get_version(self).await
    }

    async fn get_config(&self) -> Result<ConfigResponse> {
        MihomoClient::get_config(self).await
    }

    async fn get_rules(&self) -> Result<Vec<Rule>> {
        MihomoClient::get_rules(self).await
    }

    async fn get_proxies(&self) -> Result<HashMap<String, Proxy>> {
        MihomoClient::get_proxies(self).await
    }

    async fn get_proxy(&self, name: &str) -> Result<Proxy> {
        MihomoClient::get_proxy(self, name).await
    }

    async fn switch_proxy(&self, group: &str, name: &str) -> Result<()> {
        MihomoClient::switch_proxy(self, group, name).await
    }

    async fn test_delay(&self, name: &str, url: &str, timeout: u32) -> Result<u32> {
        MihomoClient::test_delay(self, name, url, timeout).await
    }

    async fn reload_config(&self, path: Option<&str>) -> Result<()> {
        MihomoClient::reload_config(self, path).await
    }

    async fn patch_config(&self, updates: Value) -> Result<()> {
        MihomoClient::patch_config(self, updates).await
    }

    async fn get_proxy_providers(&self) -> Result<HashMap<String, ProxyProvider>> {
        MihomoClient::get_proxy_providers(self).await
    }

    async fn get_rule_providers(&self) -> Result<HashMap<String, RuleProvider>> {
        MihomoClient::get_rule_providers(self).await
    }

    async fn update_proxy_provider(&self, name: &str) -> Result<()> {
        MihomoClient::update_proxy_provider(self, name).await
    }

    async fn update_rule_provider(&self, name: &str) -> Result<()> {
        MihomoClient::update_rule_provider(self, name).await
    }

    async fn get_dns_query(&self, name: &str, q_type: &str) -> Result<Value> {
        MihomoClient::get_dns_query(self, name, q_type).await
    }

    async fn get_script(&self) -> Result<Value> {
        MihomoClient::get_script(self).await
    }

    async fn restart_core(&self) -> Result<()> {
        MihomoClient::restart_core(self).await
    }

    async fn upgrade_geo(&self) -> Result<()> {
        MihomoClient::upgrade_geo(self).await
    }

    async fn provider_healthcheck(&self, provider: &str) -> Result<()> {
        MihomoClient::provider_healthcheck(self, provider).await
    }

    async fn fakeip_cache(&self) -> Result<Value> {
        MihomoClient::fakeip_cache(self).await
    }

    async fn group_delay(
        &self,
        group: &str,
        url: &str,
        timeout_ms: u32,
    ) -> Result<HashMap<String, Value>> {
        MihomoClient::group_delay(self, group, url, timeout_ms).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::MihomoError;
    #[cfg(test)]
    use std::sync::Arc;

    fn unsupported<T>() -> Result<T> {
        Err(MihomoError::Config(
            "mock: endpoint not stubbed for this test".to_string(),
        ))
    }

    /// In-memory mock proving the seam is object-safe and mockable.
    struct MockMihomoApi {
        version: String,
        proxies: Vec<String>,
    }

    #[async_trait]
    impl MihomoApi for MockMihomoApi {
        async fn get_version(&self) -> Result<Version> {
            Ok(Version {
                version: self.version.clone(),
                premium: false,
            })
        }

        async fn get_proxies(&self) -> Result<HashMap<String, Proxy>> {
            let mut map = HashMap::new();
            for name in &self.proxies {
                map.insert(name.clone(), Proxy::Unknown);
            }
            Ok(map)
        }

        async fn get_config(&self) -> Result<ConfigResponse> {
            unsupported()
        }
        async fn get_rules(&self) -> Result<Vec<Rule>> {
            unsupported()
        }
        async fn get_proxy(&self, _name: &str) -> Result<Proxy> {
            unsupported()
        }
        async fn switch_proxy(&self, _group: &str, _name: &str) -> Result<()> {
            unsupported()
        }
        async fn test_delay(&self, _name: &str, _url: &str, _timeout: u32) -> Result<u32> {
            unsupported()
        }
        async fn reload_config(&self, _path: Option<&str>) -> Result<()> {
            unsupported()
        }
        async fn patch_config(&self, _updates: Value) -> Result<()> {
            unsupported()
        }
        async fn get_proxy_providers(&self) -> Result<HashMap<String, ProxyProvider>> {
            unsupported()
        }
        async fn get_rule_providers(&self) -> Result<HashMap<String, RuleProvider>> {
            unsupported()
        }
        async fn update_proxy_provider(&self, _name: &str) -> Result<()> {
            unsupported()
        }
        async fn update_rule_provider(&self, _name: &str) -> Result<()> {
            unsupported()
        }
        async fn get_dns_query(&self, _name: &str, _q_type: &str) -> Result<Value> {
            unsupported()
        }
        async fn get_script(&self) -> Result<Value> {
            unsupported()
        }
        async fn restart_core(&self) -> Result<()> {
            unsupported()
        }
        async fn upgrade_geo(&self) -> Result<()> {
            unsupported()
        }
        async fn provider_healthcheck(&self, _provider: &str) -> Result<()> {
            unsupported()
        }
        async fn fakeip_cache(&self) -> Result<Value> {
            unsupported()
        }
        async fn group_delay(
            &self,
            _group: &str,
            _url: &str,
            _timeout_ms: u32,
        ) -> Result<HashMap<String, Value>> {
            unsupported()
        }
    }

    // Generic consumer: written once against `impl MihomoApi`, works with the
    // real `MihomoClient` and any mock alike.
    async fn describe<C: MihomoApi + ?Sized>(api: &C) -> Result<String> {
        let version = api.get_version().await?.version;
        let proxies = api.get_proxies().await?;
        let names: Vec<&String> = proxies.keys().collect();
        Ok(format!("{} ({})", version, names.len()))
    }

    #[tokio::test]
    async fn mock_impl_can_stand_in_for_mihomo_client() {
        let mock = MockMihomoApi {
            version: "mock-1.0".to_string(),
            proxies: vec!["A".to_string(), "B".to_string()],
        };
        assert_eq!(
            describe(&mock).await.unwrap(),
            "mock-1.0 (2)",
            "generic code must consume the mock impl"
        );

        // The seam is also usable behind a dyn trait object.
        let boxed: Arc<dyn MihomoApi> = Arc::new(MockMihomoApi {
            version: "dyn-2.0".to_string(),
            proxies: vec![],
        });
        assert_eq!(describe(boxed.as_ref()).await.unwrap(), "dyn-2.0 (0)");
    }
}
