//! Pure derivation of the selected active outbound node from Mihomo proxies.

use std::collections::HashMap;

use infiltrator_contract::active_exit::{ActiveExitSnapshot, ActiveExitStatus};

use crate::filter::extract_country_code;
use crate::proxy::Proxy;

/// Derive the selected exit from the controller's proxy map. Selection is
/// deterministic: `PROXIES`, then `GLOBAL`, then the first sorted group.
pub fn derive(
    generation: u64,
    revision: u64,
    proxies: &HashMap<String, Proxy>,
) -> ActiveExitSnapshot {
    let Some((group_name, group)) = selected_group(proxies) else {
        return ActiveExitSnapshot {
            generation,
            revision,
            status: ActiveExitStatus::Empty,
            ..Default::default()
        };
    };
    let selected = group.now().unwrap_or_default().trim();
    if selected.is_empty() {
        return ActiveExitSnapshot {
            generation,
            revision,
            status: ActiveExitStatus::Empty,
            group: Some(group_name),
            ..Default::default()
        };
    }

    let proxy = proxies.get(selected);
    let delay_ms = proxy.and_then(|proxy| {
        proxy
            .delay()
            .or_else(|| proxy.history().last().map(|history| history.delay))
    });
    ActiveExitSnapshot {
        generation,
        revision,
        status: ActiveExitStatus::Ready,
        failure: None,
        group: Some(group_name),
        name: Some(selected.to_owned()),
        country_code: extract_country_code(selected).map(str::to_owned),
        protocol: proxy.map(|proxy| proxy.proxy_type().to_owned()),
        delay_ms,
        alive: proxy.map(Proxy::alive),
    }
}

fn selected_group(proxies: &HashMap<String, Proxy>) -> Option<(String, &Proxy)> {
    for preferred in ["PROXIES", "GLOBAL"] {
        if let Some((name, proxy)) = proxies
            .iter()
            .find(|(name, proxy)| name.eq_ignore_ascii_case(preferred) && proxy.is_group())
        {
            return Some((name.clone(), proxy));
        }
    }
    let mut groups = proxies
        .iter()
        .filter(|(_, proxy)| proxy.is_group())
        .collect::<Vec<_>>();
    groups.sort_by(|left, right| left.0.cmp(right.0));
    groups
        .first()
        .map(|(name, proxy)| ((*name).clone(), *proxy))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::proxy::{ProxyBase, ProxyGroup, Shadowsocks};

    fn proxies() -> HashMap<String, Proxy> {
        HashMap::from([
            (
                "GLOBAL".to_owned(),
                Proxy::Selector(ProxyGroup {
                    name: "GLOBAL".to_owned(),
                    now: "香港 01".to_owned(),
                    all: vec!["香港 01".to_owned()],
                    history: Vec::new(),
                }),
            ),
            (
                "香港 01".to_owned(),
                Proxy::Shadowsocks(Shadowsocks {
                    base: ProxyBase {
                        name: "香港 01".to_owned(),
                        delay: Some(42),
                        alive: true,
                        ..Default::default()
                    },
                    ..Default::default()
                }),
            ),
        ])
    }

    #[test]
    fn derives_selected_group_node_protocol_delay_and_region() {
        let snapshot = derive(3, 4, &proxies());
        assert_eq!(snapshot.status, ActiveExitStatus::Ready);
        assert_eq!(snapshot.group.as_deref(), Some("GLOBAL"));
        assert_eq!(snapshot.name.as_deref(), Some("香港 01"));
        assert_eq!(snapshot.country_code.as_deref(), Some("HK"));
        assert_eq!(snapshot.protocol.as_deref(), Some("Shadowsocks"));
        assert_eq!(snapshot.delay_ms, Some(42));
        assert_eq!(snapshot.alive, Some(true));
    }

    #[test]
    fn empty_proxy_map_is_typed_empty() {
        let snapshot = derive(1, 1, &HashMap::new());
        assert_eq!(snapshot.status, ActiveExitStatus::Empty);
        assert!(!snapshot.is_drawable());
    }
}
