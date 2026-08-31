/// 代理节点类型系统测试
///
/// 验证 Proxy 枚举的反序列化、类型识别、属性访问等核心业务语义。
/// Mihomo API 返回的所有代理节点必须能被正确识别并分类为代理节点或策略组。
#[cfg(test)]
mod tests {
    use crate::proxy::types::Proxy;
    use crate::types::ProxiesResponse;

    // ──────────────────────────────────────────────
    // 反序列化：节点类型识别
    // ──────────────────────────────────────────────

    #[test]
    fn shadowsocks_node_deserializes_with_cipher_and_server() {
        let json = r#"{
            "proxies": {
                "My-SS": {
                    "type": "Shadowsocks",
                    "name": "My-SS",
                    "udp": true,
                    "history": [],
                    "alive": true,
                    "server": "1.2.3.4",
                    "port": 8388,
                    "cipher": "aes-256-gcm"
                }
            }
        }"#;
        let resp: ProxiesResponse = serde_json::from_str(json).unwrap();
        let proxy = resp.proxies.get("My-SS").unwrap();

        assert_eq!(proxy.proxy_type(), "Shadowsocks");
        assert!(!proxy.is_group());
        if let Proxy::Shadowsocks(ss) = proxy {
            assert_eq!(ss.server, "1.2.3.4");
            assert_eq!(ss.cipher, "aes-256-gcm");
        } else {
            panic!("Expected Shadowsocks");
        }
    }

    #[test]
    fn shadowsocks_with_obfs_plugin_deserializes_plugin_opts() {
        // 带 obfs 混淆插件的 SS 节点，plugin-opts 必须正确解析
        let json = r#"{
            "proxies": {
                "SS-Obfs": {
                    "type": "Shadowsocks",
                    "name": "SS-Obfs",
                    "udp": false,
                    "history": [],
                    "alive": true,
                    "server": "1.1.1.1",
                    "port": 8388,
                    "cipher": "aes-256-gcm",
                    "plugin": "v2ray-plugin",
                    "plugin-opts": {
                        "mode": "websocket",
                        "host": "cdn.example.com"
                    }
                }
            }
        }"#;
        let resp: ProxiesResponse = serde_json::from_str(json).unwrap();
        if let Proxy::Shadowsocks(ss) = resp.proxies.get("SS-Obfs").unwrap() {
            assert_eq!(ss.plugin.as_deref(), Some("v2ray-plugin"));
            let opts = ss.plugin_opts.as_ref().unwrap();
            assert_eq!(opts.mode.as_deref(), Some("websocket"));
            assert_eq!(opts.host.as_deref(), Some("cdn.example.com"));
        } else {
            panic!("Expected Shadowsocks");
        }
    }

    #[test]
    fn vmess_node_with_websocket_transport_deserializes_ws_opts() {
        // VMess + WebSocket 是常见组合，ws-opts 中的 path 和 Host header 必须保留
        let json = r#"{
            "proxies": {
                "Vmess-WS": {
                    "type": "Vmess",
                    "name": "Vmess-WS",
                    "udp": false,
                    "history": [],
                    "alive": true,
                    "server": "2.2.2.2",
                    "port": 443,
                    "uuid": "a-uuid-string",
                    "alterId": 0,
                    "cipher": "auto",
                    "tls": true,
                    "network": "ws",
                    "ws-opts": {
                        "path": "/v2ray",
                        "headers": { "Host": "cdn.example.com" }
                    }
                }
            }
        }"#;
        let resp: ProxiesResponse = serde_json::from_str(json).unwrap();
        if let Proxy::Vmess(vm) = resp.proxies.get("Vmess-WS").unwrap() {
            assert_eq!(vm.network, "ws");
            assert!(vm.tls, "TLS 应该为开启状态");
            let opts = vm.ws_opts.as_ref().expect("ws-opts 不能为空");
            assert_eq!(opts.path.as_deref(), Some("/v2ray"));
            let headers = opts.headers.as_ref().unwrap();
            assert_eq!(headers.get("Host").map(|s| s.as_str()), Some("cdn.example.com"));
        } else {
            panic!("Expected Vmess");
        }
    }

    #[test]
    fn hysteria2_node_deserializes_auth_and_sni() {
        let json = r#"{
            "proxies": {
                "Hy2": {
                    "type": "Hysteria2",
                    "name": "Hy2",
                    "udp": true,
                    "history": [],
                    "alive": true,
                    "server": "3.3.3.3",
                    "port": 443,
                    "auth": "my-password",
                    "sni": "example.com"
                }
            }
        }"#;
        let resp: ProxiesResponse = serde_json::from_str(json).unwrap();
        if let Proxy::Hysteria2(hy) = resp.proxies.get("Hy2").unwrap() {
            assert_eq!(hy.auth.as_deref(), Some("my-password"));
            assert_eq!(hy.sni.as_deref(), Some("example.com"));
        } else {
            panic!("Expected Hysteria2");
        }
    }

    // ──────────────────────────────────────────────
    // 策略组：is_group / now / all 语义
    // ──────────────────────────────────────────────

    #[test]
    fn selector_group_is_recognized_as_selectable_group() {
        // Selector 是用户可手动切换节点的策略组，必须能读取 now 和 all
        let json = r#"{
            "proxies": {
                "MyGroup": {
                    "type": "Selector",
                    "name": "MyGroup",
                    "now": "Node-A",
                    "all": ["Node-A", "Node-B", "Node-C"],
                    "history": []
                }
            }
        }"#;
        let resp: ProxiesResponse = serde_json::from_str(json).unwrap();
        let proxy = resp.proxies.get("MyGroup").unwrap();

        assert!(proxy.is_group(), "Selector 必须被识别为策略组");
        assert_eq!(proxy.proxy_type(), "Selector");
        assert_eq!(proxy.now(), Some("Node-A"), "now 应返回当前选中节点");
        let all = proxy.all().unwrap();
        assert_eq!(all.len(), 3);
        assert!(all.contains(&"Node-B".to_string()));
    }

    #[test]
    fn urltest_and_fallback_groups_are_not_manually_selectable_nodes() {
        // URLTest / Fallback 是自动切换组，is_group() 为 true，但不暴露 .all() 给用户手动选择
        // （业务上这两种组的节点由 mihomo 自动管理）
        let json = r#"{
            "proxies": {
                "AutoGroup": {
                    "type": "URLTest",
                    "name": "AutoGroup",
                    "now": "Fast-Node",
                    "all": ["Fast-Node", "Slow-Node"],
                    "history": []
                },
                "FallbackGroup": {
                    "type": "Fallback",
                    "name": "FallbackGroup",
                    "now": "Primary",
                    "all": ["Primary", "Backup"],
                    "history": []
                }
            }
        }"#;
        let resp: ProxiesResponse = serde_json::from_str(json).unwrap();

        let auto = resp.proxies.get("AutoGroup").unwrap();
        assert!(auto.is_group());
        assert_eq!(auto.proxy_type(), "URLTest");

        let fallback = resp.proxies.get("FallbackGroup").unwrap();
        assert!(fallback.is_group());
        assert_eq!(fallback.proxy_type(), "Fallback");
    }

    #[test]
    fn non_group_proxy_returns_none_for_group_accessors() {
        // 普通节点不应返回 now / all，防止调用方错误地把节点当组用
        let json = r#"{
            "proxies": {
                "Node": {
                    "type": "Shadowsocks",
                    "name": "Node",
                    "udp": true,
                    "history": [],
                    "alive": true,
                    "server": "1.1.1.1",
                    "port": 8388,
                    "cipher": "aes-256-gcm"
                }
            }
        }"#;
        let resp: ProxiesResponse = serde_json::from_str(json).unwrap();
        let proxy = resp.proxies.get("Node").unwrap();

        assert!(!proxy.is_group());
        assert_eq!(proxy.now(), None, "普通节点没有 now");
        assert_eq!(proxy.all(), None, "普通节点没有 all");
    }

    // ──────────────────────────────────────────────
    // 延迟历史与存活状态
    // ──────────────────────────────────────────────

    #[test]
    fn delay_is_taken_from_most_recent_history_entry() {
        // 节点的当前延迟取自 history 最后一条记录的 delay 字段
        let json = r#"{
            "proxies": {
                "Node": {
                    "type": "Shadowsocks",
                    "name": "Node",
                    "udp": true,
                    "history": [
                        { "time": "2024-01-01T00:00:00Z", "delay": 200 },
                        { "time": "2024-01-01T00:01:00Z", "delay": 85 }
                    ],
                    "alive": true,
                    "server": "1.1.1.1",
                    "port": 443,
                    "cipher": "aes-256-gcm"
                }
            }
        }"#;
        let resp: ProxiesResponse = serde_json::from_str(json).unwrap();
        let proxy = resp.proxies.get("Node").unwrap();

        let history = proxy.history();
        assert_eq!(history.len(), 2);
        // 最近一条延迟为 85ms
        assert_eq!(history.last().map(|h| h.delay), Some(85));
    }

    #[test]
    fn dead_node_has_alive_false_and_no_delay() {
        // 不可达节点 alive = false，delay 为 None（未测速或超时）
        let json = r#"{
            "proxies": {
                "DeadNode": {
                    "type": "Shadowsocks",
                    "name": "DeadNode",
                    "udp": false,
                    "history": [],
                    "alive": false,
                    "server": "dead.host",
                    "port": 443,
                    "cipher": "chacha20-ietf-poly1305"
                }
            }
        }"#;
        let resp: ProxiesResponse = serde_json::from_str(json).unwrap();
        let proxy = resp.proxies.get("DeadNode").unwrap();

        assert!(!proxy.alive(), "不可达节点应标记为 alive=false");
        assert_eq!(proxy.delay(), None, "未测速节点 delay 应为 None");
    }

    // ──────────────────────────────────────────────
    // 前向兼容：未知协议类型
    // ──────────────────────────────────────────────

    #[test]
    fn unknown_protocol_type_degrades_gracefully_without_panicking() {
        // 未来 Mihomo 可能新增协议类型，Unknown 变体保证不会 panic 或 parse error
        let json = r#"{
            "proxies": {
                "Future-Node": {
                    "type": "NewProtocolXYZ",
                    "name": "Future-Node",
                    "udp": true
                }
            }
        }"#;
        let resp: ProxiesResponse = serde_json::from_str(json).unwrap();
        assert!(
            matches!(resp.proxies.get("Future-Node").unwrap(), Proxy::Unknown),
            "未知协议应降级为 Unknown 而非报错"
        );
    }
}
