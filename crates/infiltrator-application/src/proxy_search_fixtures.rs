//! Explicit shared observations for the native search capture; never a production fallback.
use crate::proxy_inspection_fixtures::{INSPECTION_NODE, observed_proxy};
use infiltrator_domain::proxy::{Proxy, ProxyBase, ProxyGroup, Shadowsocks};
use std::collections::HashMap;

pub const SEARCH_GROUP: &str = "Search policy";
pub const SEARCH_QUERY: &str = "Inspection";
pub fn observed_proxies() -> HashMap<String, Proxy> {
    HashMap::from([
        (INSPECTION_NODE.into(), observed_proxy()),
        (
            "Other node".into(),
            Proxy::Shadowsocks(Shadowsocks {
                server: "other.example.test".into(),
                port: 443,
                cipher: "aes-256-gcm".into(),
                base: ProxyBase {
                    name: "Other node".into(),
                    alive: true,
                    delay: Some(65),
                    ..Default::default()
                },
                ..Default::default()
            }),
        ),
        (
            SEARCH_GROUP.into(),
            Proxy::Selector(ProxyGroup {
                name: SEARCH_GROUP.into(),
                all: vec![INSPECTION_NODE.into(), "Other node".into()],
                now: "Other node".into(),
                ..Default::default()
            }),
        ),
    ])
}
