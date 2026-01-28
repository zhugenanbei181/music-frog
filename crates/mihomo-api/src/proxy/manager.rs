use crate::{MihomoClient, ProxyGroup, ProxyInfo, ProxyNode, Result};
use std::collections::HashMap;

pub struct ProxyManager {
    client: MihomoClient,
}

impl ProxyManager {
    pub fn new(client: MihomoClient) -> Self {
        Self { client }
    }

    pub async fn list_proxies(&self) -> Result<Vec<ProxyNode>> {
        let proxies = self.client.get_proxies().await?;
        let mut nodes = vec![];

        for (name, info) in proxies {
            let is_group = matches!(
                info.proxy_type.as_str(),
                "Selector"
                    | "URLTest"
                    | "Fallback"
                    | "LoadBalance"
                    | "Relay"
                    | "Direct"
                    | "Reject"
                    | "Pass"
                    | "Compatible"
                    | "RejectDrop"
            );

            if !is_group {
                let delay = info.history.first().map(|h| h.delay);
                nodes.push(ProxyNode {
                    name,
                    proxy_type: info.proxy_type,
                    delay,
                    alive: delay.is_some(),
                });
            }
        }

        log::debug!("Filtered {} proxy nodes from all proxies", nodes.len());
        nodes.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(nodes)
    }

    pub async fn list_groups(&self) -> Result<Vec<ProxyGroup>> {
        let proxies = self.client.get_proxies().await?;
        let mut groups = vec![];

        for (name, info) in proxies {
            let is_group = matches!(
                info.proxy_type.as_str(),
                "Selector" | "URLTest" | "Fallback" | "LoadBalance" | "Relay"
            );

            if is_group {
                groups.push(ProxyGroup {
                    name,
                    group_type: info.proxy_type,
                    now: info.now.unwrap_or_default(),
                    all: info.all.unwrap_or_default(),
                });
            }
        }

        groups.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(groups)
    }

    pub async fn switch(&self, group: &str, proxy: &str) -> Result<()> {
        self.client.switch_proxy(group, proxy).await
    }

    pub async fn get_current(&self, group: &str) -> Result<String> {
        let info = self.client.get_proxy(group).await?;
        Ok(info.now.unwrap_or_default())
    }

    pub async fn get_all_proxies(&self) -> Result<HashMap<String, ProxyInfo>> {
        self.client.get_proxies().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    use mockito::Server;

    #[test]
    fn test_proxy_manager_new() {
        let client = MihomoClient::new("http://127.0.0.1:9090", None).unwrap();
        let _ = ProxyManager::new(client);
    }

    #[tokio::test]
    async fn test_list_proxies() {
        let mut server = Server::new_async().await;
        let body = serde_json::json!({
            "proxies": {
                "DIRECT": {
                    "type": "Direct",
                    "udp": true,
                    "history": []
                },
                "Proxy-A": {
                    "type": "Shadowsocks",
                    "udp": true,
                    "history": [{"time": "2024-01-01T00:00:00Z", "delay": 100}]
                }
            }
        });
        
        let mock = server.mock("GET", "/proxies")
            .with_status(200)
            .with_body(serde_json::to_string(&body).unwrap())
            .create_async().await;

        let client = MihomoClient::new(&server.url(), None).unwrap();
        let manager = ProxyManager::new(client);
        let proxies = manager.list_proxies().await.unwrap();

        mock.assert_async().await;
        assert_eq!(proxies.len(), 1); // Only Proxy-A, DIRECT is considered a group-like here or filtered
        assert_eq!(proxies[0].name, "Proxy-A");
        assert_eq!(proxies[0].delay, Some(100));
        assert!(proxies[0].alive);
    }

    #[tokio::test]
    async fn test_list_groups() {
        let mut server = Server::new_async().await;
        let body = serde_json::json!({
            "proxies": {
                "GLOBAL": {
                    "type": "Selector",
                    "now": "Proxy-A",
                    "all": ["Proxy-A", "Proxy-B"],
                    "history": []
                }
            }
        });
        
        let mock = server.mock("GET", "/proxies")
            .with_status(200)
            .with_body(serde_json::to_string(&body).unwrap())
            .create_async().await;

        let client = MihomoClient::new(&server.url(), None).unwrap();
        let manager = ProxyManager::new(client);
        let groups = manager.list_groups().await.unwrap();

        mock.assert_async().await;
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].name, "GLOBAL");
        assert_eq!(groups[0].now, "Proxy-A");
        assert_eq!(groups[0].all.len(), 2);
    }
}
