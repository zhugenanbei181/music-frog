use crate::proxy::{Proxies, Proxy, ProxyGroup, ProxyHistory};
use crate::{MihomoClient, Result};

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone, Default)]
pub struct ProxyNode {
    pub name: String,
    pub proxy_type: String,
    pub udp: bool,
    pub history: Vec<ProxyHistory>,
    pub delay: Option<u32>,
    pub alive: bool,
}

pub struct ProxyManager {
    client: MihomoClient,
}

impl ProxyManager {
    pub fn new(client: MihomoClient) -> Self {
        Self { client }
    }

    pub async fn list_proxies(&self) -> Result<Vec<ProxyNode>> {
        let proxies: Proxies = self.client.get_proxies().await?;
        let mut nodes = vec![];

        for (name, info) in proxies {
            let info: Proxy = info;
            if !info.is_group() {
                let history = info.history().to_vec();
                let delay = info.delay();
                nodes.push(ProxyNode {
                    name,
                    proxy_type: info.proxy_type().to_string(),
                    udp: info.udp(),
                    history,
                    delay,
                    alive: info.alive(),
                });
            }
        }

        log::debug!("Filtered {} proxy nodes from all proxies", nodes.len());
        nodes.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(nodes)
    }

    pub async fn list_groups(&self) -> Result<Vec<ProxyGroup>> {
        let proxies: Proxies = self.client.get_proxies().await?;
        let mut groups = vec![];

        for (name, info) in proxies {
            let info: Proxy = info;
            if info.is_group() {
                groups.push(ProxyGroup {
                    name,
                    now: info.now().unwrap_or_default().to_string(),
                    all: info.all().unwrap_or_default().to_vec(),
                    history: info.history().to_vec(),
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
        let info: Proxy = self.client.get_proxy(group).await?;
        Ok(info.now().unwrap_or_default().to_string())
    }

    pub async fn get_all_proxies(&self) -> Result<Proxies> {
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
                    "name": "DIRECT",
                    "udp": true,
                    "history": [],
                    "alive": true
                },
                "Proxy-A": {
                    "type": "Shadowsocks",
                    "name": "Proxy-A",
                    "udp": true,
                    "history": [{"time": "2024-01-01T00:00:00Z", "delay": 100}],
                    "alive": true,
                    "delay": 100,
                    "server": "1.2.3.4",
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
        let manager = ProxyManager::new(client);
        let proxies = manager.list_proxies().await.unwrap();

        mock.assert_async().await;
        // DIRECT is now considered a node (not a group in is_group)
        assert_eq!(proxies.len(), 2);
        let proxy_a = proxies.iter().find(|p| p.name == "Proxy-A").unwrap();
        assert_eq!(proxy_a.delay, Some(100));
        assert!(proxy_a.alive);
    }

    #[tokio::test]
    async fn test_list_groups() {
        let mut server = Server::new_async().await;
        let body = serde_json::json!({
            "proxies": {
                "GLOBAL": {
                    "type": "Selector",
                    "name": "GLOBAL",
                    "now": "Proxy-A",
                    "all": ["Proxy-A", "Proxy-B"],
                    "history": []
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
        let manager = ProxyManager::new(client);
        let groups = manager.list_groups().await.unwrap();

        mock.assert_async().await;
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].name, "GLOBAL");
        assert_eq!(groups[0].now, "Proxy-A");
        assert_eq!(groups[0].all.len(), 2);
    }
}
