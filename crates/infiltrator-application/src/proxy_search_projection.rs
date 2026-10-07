//! One neutral search decision for names, protocol, tags, latency and phonetic/region aliases.
use infiltrator_contract::search_text::SearchTextRun;
use infiltrator_contract::surface_snapshot::ProxyGroupSnapshot;
use infiltrator_contract::surface_snapshot::ProxyNodeSnapshot;
use infiltrator_shared::fuzzy_search::pinyin_fuzzy_match;
use std::collections::BTreeMap;

pub fn project_name_runs(
    groups: &[ProxyGroupSnapshot],
    query: &str,
) -> BTreeMap<String, Vec<SearchTextRun>> {
    groups
        .iter()
        .flat_map(|group| &group.proxies)
        .map(|node| {
            let decision = search_node(node, query);
            let mut runs = Vec::new();
            let mut offset = 0;
            for (start, end) in decision.name_ranges {
                if start > offset {
                    runs.push(SearchTextRun {
                        text: node.name[offset..start].into(),
                        highlighted: false,
                    });
                }
                runs.push(SearchTextRun {
                    text: node.name[start..end].into(),
                    highlighted: true,
                });
                offset = end;
            }
            if offset < node.name.len() {
                runs.push(SearchTextRun {
                    text: node.name[offset..].into(),
                    highlighted: false,
                });
            }
            (node.name.clone(), runs)
        })
        .collect()
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SearchDecision {
    pub matches: bool,
    /// UTF-8 byte ranges into the original name, safe for native text runs.
    pub name_ranges: Vec<(usize, usize)>,
}

pub fn search_node(node: &ProxyNodeSnapshot, query: &str) -> SearchDecision {
    let query = query.trim().to_ascii_lowercase();
    if query.is_empty() {
        return SearchDecision {
            matches: true,
            name_ranges: vec![],
        };
    }
    let name = node.name.to_ascii_lowercase();
    let ranges: Vec<_> = name
        .match_indices(&query)
        .map(|(start, value)| (start, start + value.len()))
        .collect();
    if !ranges.is_empty() {
        return SearchDecision {
            matches: true,
            name_ranges: ranges,
        };
    }
    if let Some(value) = query.strip_prefix('<').or_else(|| query.strip_prefix('>'))
        && let Ok(limit) = value.trim().parse::<u32>()
    {
        let matches = node
            .delay_ms
            .filter(|value| *value > 0)
            .is_some_and(|value| {
                if query.starts_with('<') {
                    value < limit
                } else {
                    value > limit
                }
            });
        return SearchDecision {
            matches,
            name_ranges: vec![],
        };
    }
    let protocol = node.node_type.to_ascii_lowercase();
    let metadata_match = protocol.contains(&query)
        || (query == "ss" && protocol == "shadowsocks")
        || (matches!(query.as_str(), "hy" | "hy2") && protocol == "hysteria2")
        || node
            .features
            .iter()
            .any(|tag| tag.to_ascii_lowercase().contains(&query));
    let name_match = pinyin_fuzzy_match(&node.name, &query);
    SearchDecision {
        matches: metadata_match || name_match,
        // A phonetic/region alias identifies the name as a whole, never invented character offsets.
        name_ranges: if name_match {
            vec![(0, node.name.len())]
        } else {
            vec![]
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn matching_preserves_unicode_ranges_and_keeps_zero_and_absence_out_of_numeric_success() {
        let mut node = ProxyNodeSnapshot {
            name: "香港 IEPL-01".into(),
            node_type: "Shadowsocks".into(),
            delay_ms: Some(42),
            alive: Some(true),
            selected: true,
            favorite: false,
            features: vec!["UDP".into()],
        };
        let matched = search_node(&node, "IEPL");
        assert!(matched.matches);
        assert_eq!(
            &node.name[matched.name_ranges[0].0..matched.name_ranges[0].1],
            "IEPL"
        );
        for query in ["xg", "hk", "ss", "udp", "<100"] {
            assert!(search_node(&node, query).matches, "query {query}");
        }
        assert!(!search_node(&node, "missing").matches);
        node.delay_ms = Some(0);
        assert!(!search_node(&node, "<100").matches);
        node.delay_ms = None;
        assert!(!search_node(&node, ">0").matches);
        assert!(search_node(&node, " ").matches);
        assert!(node.selected);
    }

    #[test]
    fn shared_search_round_trip_filters_and_highlights_without_changing_controller_selection() {
        use crate::proxy_projection::project_proxy_groups;
        use infiltrator_contract::proxies::ProxyUiPreferences;
        let node = |name: &str, delay| ProxyNodeSnapshot {
            name: name.into(),
            node_type: "Shadowsocks".into(),
            delay_ms: delay,
            alive: Some(true),
            selected: name == "Tokyo IEPL",
            favorite: false,
            features: vec![],
        };
        let input = vec![ProxyGroupSnapshot {
            name: "Policy".into(),
            group_type: "Selector".into(),
            classification: None,
            current: "Tokyo IEPL".into(),
            expanded: true,
            proxies: vec![
                node("香港 IEPL", Some(42)),
                node("Tokyo IEPL", Some(65)),
                node("Zero", Some(0)),
            ],
        }];
        let mut preferences = ProxyUiPreferences::default();
        for query in ["iepl", "hk", "<50", "missing", ""] {
            preferences.search_query = query.into();
            let groups = project_proxy_groups(input.clone(), &preferences);
            assert_eq!(groups[0].current, "Tokyo IEPL");
            let expected = match query {
                "iepl" => 2,
                "hk" | "<50" => 1,
                "missing" => 0,
                _ => 3,
            };
            assert_eq!(groups[0].proxies.len(), expected);
            let runs = project_name_runs(&groups, query);
            for node in &groups[0].proxies {
                let runs = &runs[&node.name];
                assert_eq!(
                    runs.iter().map(|run| run.text.as_str()).collect::<String>(),
                    node.name
                );
                if query == "iepl" {
                    assert!(runs.iter().any(|run| run.highlighted && run.text == "IEPL"));
                }
                if query.is_empty() {
                    assert!(runs.iter().all(|run| !run.highlighted));
                }
            }
        }
        let mut refreshed = input;
        refreshed[0].proxies[0].name = "Updated IEPL".into();
        refreshed[0].proxies[0].delay_ms = Some(99);
        preferences.search_query = "iepl".into();
        let projected = project_proxy_groups(refreshed, &preferences);
        let runs = project_name_runs(&projected, "iepl");
        assert!(!runs.contains_key("香港 IEPL"));
        assert!(
            runs["Updated IEPL"]
                .iter()
                .any(|run| run.highlighted && run.text == "IEPL")
        );
        assert_eq!(
            projected[0]
                .proxies
                .iter()
                .find(|node| node.name == "Updated IEPL")
                .unwrap()
                .delay_ms,
            Some(99)
        );
        assert_eq!(projected[0].current, "Tokyo IEPL");
    }
}
