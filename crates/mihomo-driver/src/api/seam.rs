//! Private controller seam used only by the typed Mihomo outbound adapter.
//!
//! Raw name-based controller operations stay behind this module; product
//! consumers use the scope- and revision-bound `OutboundPort`.

use crate::api::client::MihomoClient;
use crate::api::proxy::ProxyObservation;
use crate::error::Result;
use async_trait::async_trait;
use std::collections::HashMap;

#[async_trait]
pub(crate) trait MihomoOutboundApi: Send + Sync {
    async fn get_proxies(&self) -> Result<HashMap<String, ProxyObservation>>;
    async fn switch_proxy(&self, group: &str, name: &str) -> Result<()>;
    async fn test_delay(&self, name: &str, url: &str, timeout: u32) -> Result<u32>;
}

#[async_trait]
impl MihomoOutboundApi for MihomoClient {
    async fn get_proxies(&self) -> Result<HashMap<String, ProxyObservation>> {
        MihomoClient::get_proxies(self).await
    }

    async fn switch_proxy(&self, group: &str, name: &str) -> Result<()> {
        MihomoClient::switch_proxy(self, group, name).await
    }

    async fn test_delay(&self, name: &str, url: &str, timeout: u32) -> Result<u32> {
        MihomoClient::test_delay(self, name, url, timeout).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    struct MockMihomoOutboundApi {
        names: Vec<String>,
    }

    #[async_trait]
    impl MihomoOutboundApi for MockMihomoOutboundApi {
        async fn get_proxies(&self) -> Result<HashMap<String, ProxyObservation>> {
            Ok(self
                .names
                .iter()
                .cloned()
                .map(|name| {
                    (
                        name.clone(),
                        ProxyObservation {
                            name,
                            proxy_type: "Unknown".to_owned(),
                            ..ProxyObservation::default()
                        },
                    )
                })
                .collect())
        }

        async fn switch_proxy(&self, _group: &str, _name: &str) -> Result<()> {
            Ok(())
        }

        async fn test_delay(&self, _name: &str, _url: &str, _timeout: u32) -> Result<u32> {
            Ok(42)
        }
    }

    async fn inventory_count(api: &dyn MihomoOutboundApi) -> Result<usize> {
        Ok(api.get_proxies().await?.len())
    }

    #[tokio::test]
    async fn private_outbound_adapter_seam_is_object_safe_and_mockable() {
        let mock = Arc::new(MockMihomoOutboundApi {
            names: vec!["A".to_owned(), "B".to_owned()],
        });
        assert_eq!(inventory_count(mock.as_ref()).await.unwrap(), 2);

        let erased: Arc<dyn MihomoOutboundApi> =
            Arc::new(MockMihomoOutboundApi { names: Vec::new() });
        assert_eq!(inventory_count(erased.as_ref()).await.unwrap(), 0);
    }
}
