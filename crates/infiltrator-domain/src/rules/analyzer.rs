use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet, VecDeque};
use serde::{Deserialize, Serialize};

use super::RuleEntry;
use super::types::{ParsedRule, RuleType, parse_rule_str};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShadowedRuleWarning {
    pub index: usize,
    pub rule: String,
    pub shadowed_by_index: usize,
    pub shadowed_by_rule: String,
    pub reason: ShadowReason,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ShadowReason {
    UnreachableAfterMatch,
    DuplicateRule,
    DomainShadowedBySuffix,
    DomainSuffixShadowedBySuffix,
    DomainShadowedByKeyword,
    IpCidrShadowedByCidr,
    Other(String),
}

impl std::fmt::Display for ShadowReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ShadowReason::UnreachableAfterMatch => {
                write!(
                    f,
                    "Rule is unreachable because an earlier MATCH rule matches all traffic"
                )
            }
            ShadowReason::DuplicateRule => {
                write!(
                    f,
                    "Duplicate identical rule exists earlier in the rule list"
                )
            }
            ShadowReason::DomainShadowedBySuffix => {
                write!(
                    f,
                    "Domain is shadowed by an earlier broader DOMAIN-SUFFIX rule"
                )
            }
            ShadowReason::DomainSuffixShadowedBySuffix => {
                write!(
                    f,
                    "Domain suffix is shadowed by an earlier broader DOMAIN-SUFFIX rule"
                )
            }
            ShadowReason::DomainShadowedByKeyword => {
                write!(f, "Domain is shadowed by an earlier DOMAIN-KEYWORD rule")
            }
            ShadowReason::IpCidrShadowedByCidr => {
                write!(f, "IP CIDR is shadowed by an earlier broader IP-CIDR rule")
            }
            ShadowReason::Other(msg) => write!(f, "{msg}"),
        }
    }
}

/// Helper function to parse an IP CIDR string into (IpAddr, prefix_len).
fn parse_cidr(cidr_str: &str) -> Option<(std::net::IpAddr, u8)> {
    let trimmed = cidr_str.trim();
    if let Some((ip_str, prefix_str)) = trimmed.split_once('/') {
        let ip = ip_str.trim().parse::<std::net::IpAddr>().ok()?;
        let prefix = prefix_str.trim().parse::<u8>().ok()?;
        Some((ip, prefix))
    } else {
        let ip = trimmed.parse::<std::net::IpAddr>().ok()?;
        let prefix = match ip {
            std::net::IpAddr::V4(_) => 32,
            std::net::IpAddr::V6(_) => 128,
        };
        Some((ip, prefix))
    }
}

/// Returns true if `broader` CIDR strictly or equally contains `narrower` CIDR.
fn cidr_contains(broader: &str, narrower: &str) -> bool {
    let Some((b_ip, b_prefix)) = parse_cidr(broader) else {
        return false;
    };
    let Some((n_ip, n_prefix)) = parse_cidr(narrower) else {
        return false;
    };

    if b_prefix > n_prefix {
        return false;
    }

    match (b_ip, n_ip) {
        (std::net::IpAddr::V4(b_net), std::net::IpAddr::V4(n_net)) => {
            if b_prefix == 0 {
                return true;
            }
            let mask = !0u32 << (32 - b_prefix);
            (u32::from(b_net) & mask) == (u32::from(n_net) & mask)
        }
        (std::net::IpAddr::V6(b_net), std::net::IpAddr::V6(n_net)) => {
            if b_prefix == 0 {
                return true;
            }
            let mask = !0u128 << (128 - b_prefix);
            (u128::from(b_net) & mask) == (u128::from(n_net) & mask)
        }
        _ => false,
    }
}

/// Checks if an earlier rule shadows a subsequent rule.
fn check_shadow(
    earlier_entry: &RuleEntry,
    earlier_parsed: &ParsedRule,
    later_entry: &RuleEntry,
    later_parsed: &ParsedRule,
) -> Option<ShadowReason> {
    // 1. Unreachable after MATCH
    if earlier_parsed.rule_type == RuleType::Match {
        return Some(ShadowReason::UnreachableAfterMatch);
    }

    // 2. Duplicate rule
    if earlier_entry
        .rule
        .trim()
        .eq_ignore_ascii_case(later_entry.rule.trim())
        || (earlier_parsed.rule_type == later_parsed.rule_type
            && earlier_parsed
                .target
                .eq_ignore_ascii_case(&later_parsed.target)
            && earlier_parsed.no_resolve == later_parsed.no_resolve)
    {
        return Some(ShadowReason::DuplicateRule);
    }

    match (&earlier_parsed.rule_type, &later_parsed.rule_type) {
        // 3. Domain shadowed by DOMAIN-SUFFIX
        (RuleType::DomainSuffix(suffix_j), RuleType::Domain(domain_i)) => {
            let s = suffix_j.trim_start_matches('.').to_ascii_lowercase();
            let d = domain_i.trim_start_matches('.').to_ascii_lowercase();
            if d == s || d.ends_with(&format!(".{s}")) {
                return Some(ShadowReason::DomainShadowedBySuffix);
            }
        }
        // 4. Domain suffix shadowed by broader DOMAIN-SUFFIX
        (RuleType::DomainSuffix(suffix_j), RuleType::DomainSuffix(suffix_i)) => {
            let s_j = suffix_j.trim_start_matches('.').to_ascii_lowercase();
            let s_i = suffix_i.trim_start_matches('.').to_ascii_lowercase();
            if s_i != s_j && s_i.ends_with(&format!(".{s_j}")) {
                return Some(ShadowReason::DomainSuffixShadowedBySuffix);
            }
        }
        // 5. Domain shadowed by DOMAIN-KEYWORD
        (RuleType::DomainKeyword(kw_j), RuleType::Domain(domain_i)) => {
            let kw = kw_j.to_ascii_lowercase();
            let d = domain_i.to_ascii_lowercase();
            if d.contains(&kw) {
                return Some(ShadowReason::DomainShadowedByKeyword);
            }
        }
        (RuleType::DomainKeyword(kw_j), RuleType::DomainSuffix(suffix_i)) => {
            let kw = kw_j.to_ascii_lowercase();
            let s = suffix_i.to_ascii_lowercase();
            if s.contains(&kw) {
                return Some(ShadowReason::DomainShadowedByKeyword);
            }
        }
        // 6. IP CIDR shadowed by broader IP CIDR
        (RuleType::IpCidr(cidr_j), RuleType::IpCidr(cidr_i)) => {
            if cidr_contains(cidr_j, cidr_i) && cidr_j != cidr_i {
                return Some(ShadowReason::IpCidrShadowedByCidr);
            }
        }
        (RuleType::IpCidr6(cidr_j), RuleType::IpCidr6(cidr_i)) => {
            if cidr_contains(cidr_j, cidr_i) && cidr_j != cidr_i {
                return Some(ShadowReason::IpCidrShadowedByCidr);
            }
        }
        (RuleType::SrcIpCidr(cidr_j), RuleType::SrcIpCidr(cidr_i))
            if cidr_contains(cidr_j, cidr_i) && cidr_j != cidr_i =>
        {
            return Some(ShadowReason::IpCidrShadowedByCidr);
        }
        _ => {}
    }

    None
}

/// Static analysis for rule lists: scans active rules for shadowing, duplicates, and unreachability.
pub fn find_shadowed_rules(rules: &[RuleEntry]) -> Vec<ShadowedRuleWarning> {
    let mut warnings = Vec::new();
    let mut parsed_rules: Vec<Option<ParsedRule>> = Vec::with_capacity(rules.len());

    for entry in rules {
        if !entry.enabled {
            parsed_rules.push(None);
            continue;
        }
        parsed_rules.push(parse_rule_str(&entry.rule).ok());
    }

    for (i, entry_i) in rules.iter().enumerate() {
        if !entry_i.enabled {
            continue;
        }
        let Some(ref parsed_i) = parsed_rules[i] else {
            continue;
        };

        for j in 0..i {
            if !rules[j].enabled {
                continue;
            }
            let Some(ref parsed_j) = parsed_rules[j] else {
                continue;
            };

            if let Some(reason) = check_shadow(&rules[j], parsed_j, entry_i, parsed_i) {
                warnings.push(ShadowedRuleWarning {
                    index: i,
                    rule: entry_i.rule.clone(),
                    shadowed_by_index: j,
                    shadowed_by_rule: rules[j].rule.clone(),
                    reason,
                });
                break;
            }
        }
    }

    warnings
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum VisitState {
    Unvisited,
    Visiting,
    Visited,
}

/// Topology analyzer and cycle detector for proxy group dependencies.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ProxyGroupTopology;

impl ProxyGroupTopology {
    /// Detects dependency cycles among proxy groups (e.g. `GroupA -> GroupB -> GroupA`)
    /// using depth-first search cycle detection.
    ///
    /// Returns a list of detected cycles where each cycle is represented as a closed path
    /// (e.g., `["GroupA", "GroupB", "GroupA"]`).
    pub fn detect_group_cycles(groups: &BTreeMap<String, Vec<String>>) -> Vec<Vec<String>> {
        let mut state: HashMap<&str, VisitState> = groups
            .keys()
            .map(|k| (k.as_str(), VisitState::Unvisited))
            .collect();
        let mut cycles = Vec::new();
        let mut stack: Vec<&str> = Vec::new();

        for group_name in groups.keys() {
            if state.get(group_name.as_str()) == Some(&VisitState::Unvisited) {
                Self::dfs_cycle(
                    group_name.as_str(),
                    groups,
                    &mut state,
                    &mut stack,
                    &mut cycles,
                );
            }
        }

        cycles
    }

    fn dfs_cycle<'a>(
        current: &'a str,
        groups: &'a BTreeMap<String, Vec<String>>,
        state: &mut HashMap<&'a str, VisitState>,
        stack: &mut Vec<&'a str>,
        cycles: &mut Vec<Vec<String>>,
    ) {
        state.insert(current, VisitState::Visiting);
        stack.push(current);

        if let Some(members) = groups.get(current) {
            let mut seen_neighbors = HashSet::new();
            for member in members {
                let target = member.as_str();
                if !groups.contains_key(target) || !seen_neighbors.insert(target) {
                    continue;
                }

                match state.get(target) {
                    Some(VisitState::Visiting) => {
                        if let Some(pos) = stack.iter().position(|&x| x == target) {
                            let mut cycle: Vec<String> =
                                stack[pos..].iter().map(|&s| s.to_string()).collect();
                            cycle.push(target.to_string());
                            cycles.push(cycle);
                        }
                    }
                    Some(VisitState::Unvisited) => {
                        Self::dfs_cycle(target, groups, state, stack, cycles);
                    }
                    Some(VisitState::Visited) | None => {}
                }
            }
        }

        stack.pop();
        state.insert(current, VisitState::Visited);
    }

    /// Performs a topological sort of proxy groups to determine the valid evaluation order
    /// (dependencies evaluated before dependent groups).
    ///
    /// If dependency cycles exist, returns `Err(cycle_nodes)` containing the names of nodes
    /// participating in the cycle(s).
    pub fn topological_sort_groups(
        groups: &BTreeMap<String, Vec<String>>,
    ) -> Result<Vec<String>, Vec<String>> {
        let cycles = Self::detect_group_cycles(groups);
        if !cycles.is_empty() {
            let mut cycle_nodes = BTreeSet::new();
            for cycle in cycles {
                for node in cycle {
                    cycle_nodes.insert(node);
                }
            }
            return Err(cycle_nodes.into_iter().collect());
        }

        let mut dep_count: HashMap<&str, usize> = HashMap::new();
        let mut dependents: HashMap<&str, Vec<&str>> = HashMap::new();

        for (group_name, members) in groups {
            let mut distinct_deps = HashSet::new();
            for member in members {
                let target = member.as_str();
                if groups.contains_key(target) && target != group_name.as_str() {
                    distinct_deps.insert(target);
                }
            }

            dep_count.insert(group_name.as_str(), distinct_deps.len());
            for dep in distinct_deps {
                dependents
                    .entry(dep)
                    .or_default()
                    .push(group_name.as_str());
            }
        }

        let mut ready: BTreeSet<&str> = dep_count
            .iter()
            .filter(|&(_, &count)| count == 0)
            .map(|(&name, _)| name)
            .collect();

        let mut order = Vec::with_capacity(groups.len());

        while let Some(&curr) = ready.iter().next() {
            ready.remove(curr);
            order.push(curr.to_string());

            if let Some(deps) = dependents.get(curr) {
                for &dependent in deps {
                    if let Some(count) = dep_count.get_mut(dependent) {
                        *count = count.saturating_sub(1);
                        if *count == 0 {
                            ready.insert(dependent);
                        }
                    }
                }
            }
        }

        if order.len() == groups.len() {
            Ok(order)
        } else {
            let remaining: Vec<String> = groups
                .keys()
                .filter(|k| !order.contains(k))
                .cloned()
                .collect();
            Err(remaining)
        }
    }

    /// Identifies proxy nodes or groups in `all_nodes` that cannot be reached
    /// through any dependency traversal starting from `root`.
    pub fn find_unreachable_nodes(
        groups: &BTreeMap<String, Vec<String>>,
        root: &str,
        all_nodes: &[String],
    ) -> Vec<String> {
        let mut reachable: HashSet<&str> = HashSet::new();
        let mut visited_groups: HashSet<&str> = HashSet::new();
        let mut queue: VecDeque<&str> = VecDeque::new();

        if groups.contains_key(root) {
            reachable.insert(root);
            visited_groups.insert(root);
            queue.push_back(root);

            while let Some(curr_group) = queue.pop_front() {
                if let Some(members) = groups.get(curr_group) {
                    for member in members {
                        let member_str = member.as_str();
                        reachable.insert(member_str);
                        if groups.contains_key(member_str) && visited_groups.insert(member_str) {
                            queue.push_back(member_str);
                        }
                    }
                }
            }
        } else if !root.is_empty() {
            reachable.insert(root);
        }

        all_nodes
            .iter()
            .filter(|node| !reachable.contains(node.as_str()))
            .cloned()
            .collect()
    }
}

/// Helper alias for detecting dependency cycles among proxy groups.
pub fn detect_group_cycles(groups: &BTreeMap<String, Vec<String>>) -> Vec<Vec<String>> {
    ProxyGroupTopology::detect_group_cycles(groups)
}

/// Helper alias for topologically sorting proxy groups into evaluation order.
pub fn topological_sort_groups(
    groups: &BTreeMap<String, Vec<String>>,
) -> Result<Vec<String>, Vec<String>> {
    ProxyGroupTopology::topological_sort_groups(groups)
}

/// Helper alias for finding unreachable nodes from a root group.
pub fn find_unreachable_nodes(
    groups: &BTreeMap<String, Vec<String>>,
    root: &str,
    all_nodes: &[String],
) -> Vec<String> {
    ProxyGroupTopology::find_unreachable_nodes(groups, root, all_nodes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_domain_shadowed_by_suffix() {
        let rules = vec![
            RuleEntry {
                rule: "DOMAIN-SUFFIX,google.com,PROXY".into(),
                enabled: true,
            },
            RuleEntry {
                rule: "DOMAIN,www.google.com,DIRECT".into(),
                enabled: true,
            },
            RuleEntry {
                rule: "DOMAIN,mail.google.com,REJECT".into(),
                enabled: true,
            },
            RuleEntry {
                rule: "DOMAIN,example.com,DIRECT".into(),
                enabled: true,
            },
        ];

        let warnings = find_shadowed_rules(&rules);
        assert_eq!(warnings.len(), 2);
        assert_eq!(warnings[0].index, 1);
        assert_eq!(warnings[0].shadowed_by_index, 0);
        assert_eq!(warnings[0].reason, ShadowReason::DomainShadowedBySuffix);
        assert_eq!(warnings[1].index, 2);
        assert_eq!(warnings[1].shadowed_by_index, 0);
        assert_eq!(warnings[1].reason, ShadowReason::DomainShadowedBySuffix);
    }

    #[test]
    fn test_suffix_shadowed_by_suffix() {
        let rules = vec![
            RuleEntry {
                rule: "DOMAIN-SUFFIX,google.com,PROXY".into(),
                enabled: true,
            },
            RuleEntry {
                rule: "DOMAIN-SUFFIX,mail.google.com,DIRECT".into(),
                enabled: true,
            },
        ];

        let warnings = find_shadowed_rules(&rules);
        assert_eq!(warnings.len(), 1);
        assert_eq!(warnings[0].index, 1);
        assert_eq!(
            warnings[0].reason,
            ShadowReason::DomainSuffixShadowedBySuffix
        );
    }

    #[test]
    fn test_unreachable_after_match() {
        let rules = vec![
            RuleEntry {
                rule: "MATCH,DIRECT".into(),
                enabled: true,
            },
            RuleEntry {
                rule: "DOMAIN,google.com,PROXY".into(),
                enabled: true,
            },
            RuleEntry {
                rule: "IP-CIDR,1.1.1.1/32,PROXY".into(),
                enabled: true,
            },
        ];

        let warnings = find_shadowed_rules(&rules);
        assert_eq!(warnings.len(), 2);
        assert_eq!(warnings[0].index, 1);
        assert_eq!(warnings[0].reason, ShadowReason::UnreachableAfterMatch);
        assert_eq!(warnings[1].index, 2);
        assert_eq!(warnings[1].reason, ShadowReason::UnreachableAfterMatch);
    }

    #[test]
    fn test_duplicate_rules() {
        let rules = vec![
            RuleEntry {
                rule: "DOMAIN,example.com,PROXY".into(),
                enabled: true,
            },
            RuleEntry {
                rule: "DOMAIN,example.com,PROXY".into(),
                enabled: true,
            },
        ];

        let warnings = find_shadowed_rules(&rules);
        assert_eq!(warnings.len(), 1);
        assert_eq!(warnings[0].index, 1);
        assert_eq!(warnings[0].reason, ShadowReason::DuplicateRule);
    }

    #[test]
    fn test_ip_cidr_shadowing() {
        let rules = vec![
            RuleEntry {
                rule: "IP-CIDR,10.0.0.0/8,DIRECT".into(),
                enabled: true,
            },
            RuleEntry {
                rule: "IP-CIDR,10.1.2.0/24,PROXY".into(),
                enabled: true,
            },
            RuleEntry {
                rule: "IP-CIDR,192.168.1.0/24,PROXY".into(),
                enabled: true,
            },
        ];

        let warnings = find_shadowed_rules(&rules);
        assert_eq!(warnings.len(), 1);
        assert_eq!(warnings[0].index, 1);
        assert_eq!(warnings[0].reason, ShadowReason::IpCidrShadowedByCidr);
    }

    #[test]
    fn test_disabled_rules_ignored() {
        let rules = vec![
            RuleEntry {
                rule: "MATCH,DIRECT".into(),
                enabled: false,
            },
            RuleEntry {
                rule: "DOMAIN,google.com,PROXY".into(),
                enabled: true,
            },
        ];

        let warnings = find_shadowed_rules(&rules);
        assert!(warnings.is_empty());
    }

    #[test]
    fn test_detect_group_cycles_none() {
        let mut groups = BTreeMap::new();
        groups.insert("Proxy".into(), vec!["Auto".into(), "HK 01".into()]);
        groups.insert("Auto".into(), vec!["US 01".into(), "US 02".into()]);
        groups.insert("Fallback".into(), vec!["Auto".into(), "JP 01".into()]);

        let cycles = ProxyGroupTopology::detect_group_cycles(&groups);
        assert!(cycles.is_empty());
    }

    #[test]
    fn test_detect_group_cycles_direct() {
        let mut groups = BTreeMap::new();
        groups.insert("GroupA".into(), vec!["GroupB".into()]);
        groups.insert("GroupB".into(), vec!["GroupA".into()]);

        let cycles = ProxyGroupTopology::detect_group_cycles(&groups);
        assert_eq!(cycles.len(), 1);
        assert_eq!(cycles[0], vec!["GroupA", "GroupB", "GroupA"]);
    }

    #[test]
    fn test_detect_group_cycles_self_loop() {
        let mut groups = BTreeMap::new();
        groups.insert("GroupA".into(), vec!["GroupA".into()]);

        let cycles = ProxyGroupTopology::detect_group_cycles(&groups);
        assert_eq!(cycles.len(), 1);
        assert_eq!(cycles[0], vec!["GroupA", "GroupA"]);
    }

    #[test]
    fn test_detect_group_cycles_indirect_and_leaf_nodes() {
        let mut groups = BTreeMap::new();
        groups.insert("G1".into(), vec!["NodeA".into(), "G2".into()]);
        groups.insert("G2".into(), vec!["NodeB".into(), "G3".into()]);
        groups.insert("G3".into(), vec!["NodeC".into(), "G1".into()]);

        let cycles = ProxyGroupTopology::detect_group_cycles(&groups);
        assert_eq!(cycles.len(), 1);
        assert_eq!(cycles[0], vec!["G1", "G2", "G3", "G1"]);
    }

    #[test]
    fn test_detect_group_cycles_multiple() {
        let mut groups = BTreeMap::new();
        groups.insert("A".into(), vec!["B".into()]);
        groups.insert("B".into(), vec!["A".into()]);
        groups.insert("X".into(), vec!["Y".into()]);
        groups.insert("Y".into(), vec!["X".into()]);

        let cycles = ProxyGroupTopology::detect_group_cycles(&groups);
        assert_eq!(cycles.len(), 2);
        assert_eq!(cycles[0], vec!["A", "B", "A"]);
        assert_eq!(cycles[1], vec!["X", "Y", "X"]);
    }

    #[test]
    fn test_topological_sort_groups_success() {
        let mut groups = BTreeMap::new();
        groups.insert("Proxy".into(), vec!["Auto".into(), "Fallback".into()]);
        groups.insert("Auto".into(), vec!["HK 01".into(), "HK 02".into()]);
        groups.insert("Fallback".into(), vec!["Auto".into(), "US 01".into()]);

        let order = ProxyGroupTopology::topological_sort_groups(&groups).expect("topological sort");
        assert_eq!(order, vec!["Auto", "Fallback", "Proxy"]);
    }

    #[test]
    fn test_topological_sort_groups_independent() {
        let mut groups = BTreeMap::new();
        groups.insert("GroupB".into(), vec!["Node1".into()]);
        groups.insert("GroupA".into(), vec!["Node2".into()]);

        let order = ProxyGroupTopology::topological_sort_groups(&groups).expect("topological sort");
        assert_eq!(order, vec!["GroupA", "GroupB"]);
    }

    #[test]
    fn test_topological_sort_groups_empty() {
        let groups = BTreeMap::new();
        let order = ProxyGroupTopology::topological_sort_groups(&groups).expect("empty sort");
        assert!(order.is_empty());
    }

    #[test]
    fn test_topological_sort_groups_cycle_error() {
        let mut groups = BTreeMap::new();
        groups.insert("GroupA".into(), vec!["GroupB".into()]);
        groups.insert("GroupB".into(), vec!["GroupA".into()]);
        groups.insert("Other".into(), vec!["Node1".into()]);

        let err = ProxyGroupTopology::topological_sort_groups(&groups).expect_err("cycle error");
        assert_eq!(err, vec!["GroupA", "GroupB"]);
    }

    #[test]
    fn test_find_unreachable_nodes_basic() {
        let mut groups = BTreeMap::new();
        groups.insert("PROXY".into(), vec!["Auto".into(), "Direct".into()]);
        groups.insert("Auto".into(), vec!["Node-US".into(), "Node-HK".into()]);
        groups.insert("OrphanGroup".into(), vec!["Node-JP".into()]);

        let all_nodes = vec![
            "Node-US".to_string(),
            "Node-HK".to_string(),
            "Node-JP".to_string(),
            "Node-SG".to_string(),
        ];

        let unreachable = ProxyGroupTopology::find_unreachable_nodes(&groups, "PROXY", &all_nodes);
        assert_eq!(unreachable, vec!["Node-JP", "Node-SG"]);
    }

    #[test]
    fn test_find_unreachable_nodes_with_cycles() {
        let mut groups = BTreeMap::new();
        groups.insert("G1".into(), vec!["G2".into()]);
        groups.insert("G2".into(), vec!["G1".into(), "Node-A".into()]);

        let all_nodes = vec!["Node-A".to_string(), "Node-B".to_string()];

        let unreachable = ProxyGroupTopology::find_unreachable_nodes(&groups, "G1", &all_nodes);
        assert_eq!(unreachable, vec!["Node-B"]);
    }

    #[test]
    fn test_find_unreachable_nodes_unknown_root() {
        let mut groups = BTreeMap::new();
        groups.insert("PROXY".into(), vec!["Node-US".into()]);

        let all_nodes = vec!["Node-US".to_string(), "Node-HK".to_string()];

        let unreachable = ProxyGroupTopology::find_unreachable_nodes(&groups, "NonExistent", &all_nodes);
        assert_eq!(unreachable, vec!["Node-US", "Node-HK"]);
    }

    #[test]
    fn test_find_unreachable_nodes_all_reachable() {
        let mut groups = BTreeMap::new();
        groups.insert("PROXY".into(), vec!["Node-US".into(), "Node-HK".into()]);

        let all_nodes = vec!["Node-US".to_string(), "Node-HK".to_string()];

        let unreachable = ProxyGroupTopology::find_unreachable_nodes(&groups, "PROXY", &all_nodes);
        assert!(unreachable.is_empty());
    }

    #[test]
    fn test_helper_aliases() {
        let mut groups = BTreeMap::new();
        groups.insert("A".into(), vec!["Node1".into()]);
        assert_eq!(detect_group_cycles(&groups).len(), 0);
        assert_eq!(topological_sort_groups(&groups).unwrap(), vec!["A"]);
        assert!(find_unreachable_nodes(&groups, "A", &["Node1".into()]).is_empty());
    }
}
