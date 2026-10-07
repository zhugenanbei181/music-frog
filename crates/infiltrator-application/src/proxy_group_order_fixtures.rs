//! Explicit common observations for both native order-editor captures.
use crate::proxy_inspection_fixtures::{INSPECTION_NODE, observed_proxy};
use infiltrator_domain::proxy::{Proxy, ProxyGroup};
use std::collections::HashMap;
pub const FIRST_GROUP: &str = "A-Preferred";
pub const MOVED_GROUP: &str = "B-Streaming";
pub const LAST_GROUP: &str = "C-Fallback";
pub fn observed_groups() -> HashMap<String, Proxy> {
    let mut proxies = HashMap::from([(INSPECTION_NODE.into(), observed_proxy())]);
    for name in [FIRST_GROUP, MOVED_GROUP, LAST_GROUP] {
        proxies.insert(
            name.into(),
            Proxy::Selector(ProxyGroup {
                name: name.into(),
                all: vec![INSPECTION_NODE.into()],
                now: INSPECTION_NODE.into(),
                ..Default::default()
            }),
        );
    }
    proxies
}
