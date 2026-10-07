//! The single proxy fact and preference fold; peer renderers replay its row order and states.
use crate::proxy_search_projection::search_node;
use infiltrator_contract::proxies::{
    ProxyFilterAliveSnapshot, ProxyGroupClassification, ProxyUiPreferences,
};
use infiltrator_contract::surface_snapshot::{ProxyGroupSnapshot, ProxyNodeSnapshot};
use infiltrator_domain::proxy::Proxy;
use std::collections::HashMap;

/// Preserve controller health before applying view preferences. A cached delay is not a liveness flag.
pub fn project_groups_snapshot(
    proxies: &HashMap<String, Proxy>,
    preferences: &ProxyUiPreferences,
) -> (Vec<ProxyGroupSnapshot>, ProxyFilterAliveSnapshot) {
    let mut candidates: HashMap<&str, Option<bool>> = proxies
        .iter()
        .filter(|(_, proxy)| !proxy.is_group())
        .map(|(name, proxy)| (name.as_str(), proxy.health_observation()))
        .collect();
    for member in proxies.values().filter_map(Proxy::all).flatten() {
        if !proxies.contains_key(member) {
            candidates.entry(member).or_insert(None);
        }
    }
    let alive = ProxyFilterAliveSnapshot::derive_from_candidates(
        candidates.into_values(),
        preferences.filter_alive,
    );
    let groups = proxies
        .iter()
        .filter_map(|(name, proxy)| {
            let members = proxy.all()?;
            let current = proxy.now().unwrap_or_default().to_owned();
            Some(ProxyGroupSnapshot {
                name: name.clone(),
                group_type: proxy.proxy_type().into(),
                classification: ProxyGroupClassification::from_str_loose(proxy.proxy_type()),
                current: current.clone(),
                expanded: true,
                proxies: members
                    .iter()
                    .map(|member| {
                        let proxy = proxies.get(member);
                        ProxyNodeSnapshot {
                            name: member.clone(),
                            node_type: proxy.map(Proxy::proxy_type).unwrap_or("Unknown").into(),
                            delay_ms: proxy.and_then(Proxy::delay),
                            alive: proxy.and_then(Proxy::health_observation),
                            selected: member == &current,
                            favorite: false,
                            features: proxy
                                .is_some_and(Proxy::udp)
                                .then(|| "UDP".to_owned())
                                .into_iter()
                                .collect(),
                        }
                    })
                    .collect(),
            })
        })
        .collect();
    (project_proxy_groups(groups, preferences), alive)
}

/// One fold for group identity, expansion, favorites, order and live-node filtering.
pub fn project_proxy_groups(
    mut groups: Vec<ProxyGroupSnapshot>,
    preferences: &ProxyUiPreferences,
) -> Vec<ProxyGroupSnapshot> {
    for group in &mut groups {
        group.expanded = preferences.is_group_expanded(&group.name);
        for node in &mut group.proxies {
            node.favorite = preferences.is_favorite(&node.name);
        }
        if preferences.filter_alive {
            group
                .proxies
                .retain(|node| ProxyFilterAliveSnapshot::is_node_alive(node.alive));
        }
        if !preferences.search_query.trim().is_empty()
            && !group
                .name
                .to_ascii_lowercase()
                .contains(&preferences.search_query.trim().to_ascii_lowercase())
        {
            group
                .proxies
                .retain(|node| search_node(node, &preferences.search_query).matches);
        }
        group.proxies.sort_by(|left, right| {
            preferences.sort_order.compare_candidates(
                &left.name,
                left.delay_ms,
                left.favorite,
                &right.name,
                right.delay_ms,
                right.favorite,
            )
        });
    }
    let rank = |name: &str| {
        preferences
            .custom_group_order
            .iter()
            .position(|entry| entry == name)
            .unwrap_or(usize::MAX)
    };
    groups.sort_by(|left, right| {
        rank(&left.name)
            .cmp(&rank(&right.name))
            .then_with(|| left.name.cmp(&right.name))
    });
    groups
}

#[cfg(test)]
mod tests {
    use super::*;
    use infiltrator_domain::proxy::{ProxyBase, ProxyGroup, Shadowsocks};
    use infiltrator_domain::proxy_observation::RuntimeProxyObservation;
    use serde_json::from_str;

    #[test]
    fn unreported_group_members_keep_their_identity_and_unknown_facts_in_rows_and_inspection() {
        use crate::proxy_inspection_projection::inspection_catalogue;
        let proxies = HashMap::from([(
            "group".into(),
            Proxy::Selector(ProxyGroup {
                name: "group".into(),
                now: "missing-node".into(),
                all: vec!["missing-node".into()],
                ..Default::default()
            }),
        )]);
        let mut prefs = ProxyUiPreferences {
            search_query: "missing".into(),
            ..Default::default()
        };
        let (groups, counts) = project_groups_snapshot(&proxies, &prefs);
        assert_eq!(
            (
                counts.total_nodes,
                counts.unknown_nodes,
                counts.alive_nodes,
                counts.dead_nodes
            ),
            (1, 1, 0, 0)
        );
        let node = &groups[0].proxies[0];
        assert_eq!(node.name, "missing-node");
        assert_eq!(node.node_type, "Unknown");
        assert_eq!(node.alive, None);
        assert_eq!(node.delay_ms, None);
        assert!(node.selected);
        let details = inspection_catalogue(&proxies);
        let detail = details
            .iter()
            .find(|detail| detail.name == "missing-node")
            .unwrap();
        assert!(!detail.can_probe);
        assert_eq!(
            (
                detail.server.as_deref(),
                detail.port,
                detail.udp,
                detail.alive
            ),
            (None, None, None, None)
        );
        prefs.filter_alive = true;
        let (groups, _) = project_groups_snapshot(&proxies, &prefs);
        assert!(groups[0].proxies.is_empty());
        assert_eq!(groups[0].current, "missing-node");
    }

    #[test]
    fn absent_health_stays_unknown_and_a_cached_latency_cannot_make_it_alive() {
        let node: ProxyNodeSnapshot = from_str(
            r#"{"name":"legacy-node","node_type":"VLESS","delay_ms":25,"selected":true,"favorite":false,"features":[]}"#,
        ).unwrap();
        assert_eq!(node.alive, None);
        assert_eq!(node.delay_ms, Some(25));
        let group = ProxyGroupSnapshot {
            name: "group".into(),
            group_type: "Selector".into(),
            classification: None,
            current: node.name.clone(),
            expanded: true,
            proxies: vec![node],
        };
        let mut preferences = ProxyUiPreferences::default();
        let unfiltered = project_proxy_groups(vec![group.clone()], &preferences);
        assert_eq!(unfiltered[0].proxies[0].alive, None);
        assert_eq!(unfiltered[0].proxies[0].delay_ms, Some(25));
        preferences.set_filter_alive(true);
        let filtered = project_proxy_groups(vec![group], &preferences);
        assert!(filtered[0].proxies.is_empty());
        assert_eq!(filtered[0].current, "legacy-node");
    }

    #[test]
    fn nested_groups_and_unknown_protocols_do_not_inherit_a_synthetic_healthy_flag() {
        let nested = Proxy::Selector(ProxyGroup {
            name: "nested".into(),
            all: vec!["unknown".into()],
            now: "unknown".into(),
            ..Default::default()
        });
        let proxies = HashMap::from([
            ("unknown".into(), Proxy::Unknown),
            ("nested".into(), nested),
            (
                "parent".into(),
                Proxy::Selector(ProxyGroup {
                    name: "parent".into(),
                    all: vec!["nested".into(), "unknown".into()],
                    now: "nested".into(),
                    ..Default::default()
                }),
            ),
        ]);
        let mut preferences = ProxyUiPreferences::default();
        let (groups, facts) = project_groups_snapshot(&proxies, &preferences);
        assert_eq!(facts.total_nodes, 1);
        assert_eq!(facts.unknown_nodes, 1);
        assert_eq!(facts.alive_nodes, 0);
        assert_eq!(facts.dead_nodes, 0);
        let parent = groups.iter().find(|group| group.name == "parent").unwrap();
        assert_eq!(parent.current, "nested");
        assert_eq!(parent.proxies.len(), 2);
        assert!(parent.proxies.iter().all(|node| node.alive.is_none()));
        preferences.set_filter_alive(true);
        let (groups, _) = project_groups_snapshot(&proxies, &preferences);
        let parent = groups.iter().find(|group| group.name == "parent").unwrap();
        assert!(parent.proxies.is_empty());
        assert_eq!(parent.current, "nested");
    }

    #[test]
    fn stale_positive_latency_does_not_override_the_controllers_dead_flag() {
        let leaf = |name: &str, alive, delay| {
            Proxy::Shadowsocks(Shadowsocks {
                base: ProxyBase {
                    name: name.into(),
                    alive,
                    delay,
                    ..Default::default()
                },
                ..Default::default()
            })
        };
        let proxies = HashMap::from([
            ("dead".into(), leaf("dead", false, Some(25))),
            ("live".into(), leaf("live", true, Some(70))),
            ("untested".into(), leaf("untested", true, None)),
            (
                "group".into(),
                Proxy::Selector(ProxyGroup {
                    name: "group".into(),
                    now: "dead".into(),
                    all: vec!["dead".into(), "live".into(), "untested".into()],
                    ..Default::default()
                }),
            ),
        ]);
        let mut preferences = ProxyUiPreferences::default();
        let (groups, facts) = project_groups_snapshot(&proxies, &preferences);
        assert_eq!(
            (facts.total_nodes, facts.alive_nodes, facts.dead_nodes),
            (3, 2, 1)
        );
        let dead = groups[0]
            .proxies
            .iter()
            .find(|node| node.name == "dead")
            .unwrap();
        assert_eq!(dead.delay_ms, Some(25));
        assert_eq!(dead.alive, Some(false));
        assert!(dead.selected);
        preferences.set_filter_alive(true);
        let (filtered, facts) = project_groups_snapshot(&proxies, &preferences);
        assert!(facts.enabled);
        assert_eq!(
            (facts.total_nodes, facts.alive_nodes, facts.dead_nodes),
            (3, 2, 1)
        );
        assert_eq!(
            filtered[0].current, "dead",
            "filtering cannot change the controller selection"
        );
        assert_eq!(filtered[0].proxies.len(), 2);
        assert_eq!(filtered[0].proxies[0].name, "live");
        assert!(!filtered[0].proxies[0].selected);
    }

    #[test]
    fn health_filter_uses_reported_flags_and_keeps_unknown_counts_separate() {
        let observation = |name: &str, alive: Option<bool>, delay: Option<u32>| {
            let facts: RuntimeProxyObservation = from_str(
                &serde_json::json!({
                    "name":name,"type":"Shadowsocks","alive":alive,"delay":delay
                })
                .to_string(),
            )
            .unwrap();
            Proxy::Observed(facts)
        };
        let proxies = HashMap::from([
            (
                "unmeasured".into(),
                observation("unmeasured", Some(true), None),
            ),
            ("zero".into(), observation("zero", Some(true), Some(0))),
            ("unknown".into(), observation("unknown", None, Some(20))),
            ("dead".into(), observation("dead", Some(false), Some(30))),
            (
                "group".into(),
                Proxy::Selector(ProxyGroup {
                    name: "group".into(),
                    now: "unknown".into(),
                    all: vec![
                        "unmeasured".into(),
                        "zero".into(),
                        "unknown".into(),
                        "dead".into(),
                    ],
                    ..Default::default()
                }),
            ),
        ]);
        let mut prefs = ProxyUiPreferences::default();
        prefs.set_filter_alive(true);
        let (groups, counts) = project_groups_snapshot(&proxies, &prefs);
        assert_eq!(
            (
                counts.total_nodes,
                counts.alive_nodes,
                counts.dead_nodes,
                counts.unknown_nodes
            ),
            (4, 2, 1, 1)
        );
        assert_eq!(groups[0].current, "unknown");
        let names: Vec<_> = groups[0]
            .proxies
            .iter()
            .map(|node| node.name.as_str())
            .collect();
        assert_eq!(names, vec!["unmeasured", "zero"]);
        assert!(
            groups[0]
                .proxies
                .iter()
                .all(|node| node.alive == Some(true))
        );
    }
}
