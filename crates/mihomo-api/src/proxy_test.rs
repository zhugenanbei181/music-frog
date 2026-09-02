/// ProxyManager 高层业务逻辑测试
///
/// ProxyManager 封装了对 Mihomo API 代理相关接口的访问。
/// 核心职责：
///   - list_proxies()：列出所有非组节点，携带延迟和存活信息
///   - list_groups()：列出所有策略组，携带成员列表和当前选中节点
///   - switch()：切换策略组的当前节点
///   - get_current()：查询策略组当前选中的节点名
#[cfg(test)]
mod tests {
    use crate::client::MihomoClient;
    use crate::proxy::manager::ProxyManager;
    use mockito::Server;

    // ──────────────────────────────────────────────
    // list_proxies：节点列表过滤与属性提取
    // ──────────────────────────────────────────────

    #[tokio::test]
    async fn list_proxies_excludes_groups_and_includes_leaf_nodes() {
        // Selector / URLTest 等策略组不应出现在节点列表中，只返回可用的叶节点
        let mut server = Server::new_async().await;
        let body = serde_json::json!({
            "proxies": {
                "GLOBAL": {
                    "type": "Selector",
                    "name": "GLOBAL",
                    "now": "HK-1",
                    "all": ["HK-1", "HK-2"],
                    "history": []
                },
                "HK-1": {
                    "type": "Shadowsocks",
                    "name": "HK-1",
                    "udp": true,
                    "history": [{ "time": "2024-01-01T00:00:00Z", "delay": 45 }],
                    "alive": true,
                    "delay": 45,
                    "server": "hk1.example.com",
                    "port": 443,
                    "cipher": "aes-256-gcm"
                },
                "HK-2": {
                    "type": "Shadowsocks",
                    "name": "HK-2",
                    "udp": true,
                    "history": [],
                    "alive": false,
                    "server": "hk2.example.com",
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
        let nodes = manager.list_proxies().await.unwrap();

        mock.assert_async().await;
        // GLOBAL（策略组）不出现在结果中
        assert!(
            nodes.iter().all(|n| n.name != "GLOBAL"),
            "策略组不应出现在节点列表中"
        );
        assert_eq!(nodes.len(), 2, "应返回 HK-1 和 HK-2 两个叶节点");

        // HK-1 有延迟记录，HK-2 无延迟（未测速或超时）
        let hk1 = nodes.iter().find(|n| n.name == "HK-1").unwrap();
        assert_eq!(hk1.delay, Some(45));
        assert!(hk1.alive);

        let hk2 = nodes.iter().find(|n| n.name == "HK-2").unwrap();
        assert_eq!(hk2.delay, None);
        assert!(!hk2.alive);
    }

    #[tokio::test]
    async fn list_proxies_result_is_sorted_by_name() {
        // 节点列表按名称升序排列，方便 UI 稳定展示
        let mut server = Server::new_async().await;
        let body = serde_json::json!({
            "proxies": {
                "Z-Node": { "type": "Shadowsocks", "name": "Z-Node", "udp": false, "history": [], "alive": true, "server": "1.1.1.1", "port": 443, "cipher": "aes-256-gcm" },
                "A-Node": { "type": "Shadowsocks", "name": "A-Node", "udp": false, "history": [], "alive": true, "server": "2.2.2.2", "port": 443, "cipher": "aes-256-gcm" },
                "M-Node": { "type": "Shadowsocks", "name": "M-Node", "udp": false, "history": [], "alive": true, "server": "3.3.3.3", "port": 443, "cipher": "aes-256-gcm" }
            }
        });

        server
            .mock("GET", "/proxies")
            .with_status(200)
            .with_body(serde_json::to_string(&body).unwrap())
            .create_async()
            .await;

        let client = MihomoClient::new(&server.url(), None).unwrap();
        let nodes = ProxyManager::new(client).list_proxies().await.unwrap();

        assert_eq!(nodes[0].name, "A-Node");
        assert_eq!(nodes[1].name, "M-Node");
        assert_eq!(nodes[2].name, "Z-Node");
    }

    // ──────────────────────────────────────────────
    // list_groups：策略组列表
    // ──────────────────────────────────────────────

    #[tokio::test]
    async fn list_groups_returns_only_group_types_with_members() {
        // 只有 Selector / URLTest / Fallback / LoadBalance 是策略组，
        // 叶节点（SS 等）不应出现在组列表中
        let mut server = Server::new_async().await;
        let body = serde_json::json!({
            "proxies": {
                "GLOBAL": {
                    "type": "Selector",
                    "name": "GLOBAL",
                    "now": "HK-Group",
                    "all": ["HK-Group", "DIRECT"],
                    "history": []
                },
                "HK-Group": {
                    "type": "URLTest",
                    "name": "HK-Group",
                    "now": "HK-1",
                    "all": ["HK-1", "HK-2"],
                    "history": []
                },
                "HK-1": {
                    "type": "Shadowsocks",
                    "name": "HK-1",
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
        let groups = ProxyManager::new(client).list_groups().await.unwrap();

        mock.assert_async().await;
        assert_eq!(groups.len(), 2, "GLOBAL 和 HK-Group 应被识别为策略组");
        assert!(
            groups.iter().all(|g| g.name != "HK-1"),
            "叶节点不应出现在组列表中"
        );

        let hk_group = groups.iter().find(|g| g.name == "HK-Group").unwrap();
        assert_eq!(hk_group.now, "HK-1");
        assert_eq!(hk_group.all.len(), 2);
    }

    // ──────────────────────────────────────────────
    // switch：节点切换
    // ──────────────────────────────────────────────

    #[tokio::test]
    async fn switch_sends_put_request_to_correct_group_endpoint() {
        // 切换策略组节点时，应向 PUT /proxies/{group} 发送请求
        let mut server = Server::new_async().await;
        let mock = server
            .mock("PUT", "/proxies/GLOBAL")
            .match_body(mockito::Matcher::JsonString(
                r#"{"name":"HK-1"}"#.to_string(),
            ))
            .with_status(204)
            .create_async()
            .await;

        let client = MihomoClient::new(&server.url(), None).unwrap();
        let result = ProxyManager::new(client).switch("GLOBAL", "HK-1").await;

        mock.assert_async().await;
        assert!(result.is_ok(), "切换节点应成功");
    }

    // ──────────────────────────────────────────────
    // get_current：查询当前节点
    // ──────────────────────────────────────────────

    #[tokio::test]
    async fn get_current_returns_active_node_name_for_selector_group() {
        // 查询策略组当前节点，返回 now 字段的值
        let mut server = Server::new_async().await;
        let body = serde_json::json!({
            "type": "Selector",
            "name": "GLOBAL",
            "now": "HK-1",
            "all": ["HK-1", "HK-2"],
            "history": []
        });

        let mock = server
            .mock("GET", "/proxies/GLOBAL")
            .with_status(200)
            .with_body(serde_json::to_string(&body).unwrap())
            .create_async()
            .await;

        let client = MihomoClient::new(&server.url(), None).unwrap();
        let current = ProxyManager::new(client)
            .get_current("GLOBAL")
            .await
            .unwrap();

        mock.assert_async().await;
        assert_eq!(current, "HK-1");
    }
}
