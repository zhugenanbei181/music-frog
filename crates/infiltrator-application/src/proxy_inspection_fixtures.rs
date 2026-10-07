//! Explicit demo/headless observations shared by both peer inspection scenarios.
use infiltrator_domain::proxy::{Proxy, ProxyBase, ProxyHistory, Shadowsocks};

pub const INSPECTION_NODE: &str = "Inspection node";
pub const INSPECTION_GROUP: &str = "Inspection group";
pub fn observed_proxy() -> Proxy {
    Proxy::Shadowsocks(Shadowsocks {
        server: "node.example.test".into(),
        port: 443,
        cipher: "aes-256-gcm".into(),
        base: ProxyBase {
            name: INSPECTION_NODE.into(),
            udp: true,
            alive: true,
            delay: Some(42),
            history: vec![
                ProxyHistory {
                    time: "2026-10-05T00:00:00Z".into(),
                    delay: 18,
                },
                ProxyHistory {
                    time: "2026-10-05T00:01:00Z".into(),
                    delay: 0,
                },
                ProxyHistory {
                    time: "2026-10-05T00:02:00Z".into(),
                    delay: 42,
                },
            ],
        },
        ..Default::default()
    })
}
