//! DUAL-05-09/10: static `dialer-proxy` (前置跳板) chain resolution and the
//! dialer/relay dependency graph.
//!
//! mihomo's `dialer-proxy:` value may name either a proxy group or an outbound
//! node (upstream docs: `值可以为策略组/出站代理的 name`), and `proxy-groups`
//! members form the relay half of the same graph. This module owns the *static*
//! facts only:
//!
//! * [`DialerGraphFacts`] — nodes with their `dialer-proxy`, groups with their
//!   members (`proxies:`) and provider references (`use:`);
//! * [`DialerTopology::detect_cycles`] — cycle detection over the combined
//!   dialer + group-member graph. It delegates to the same DFS
//!   [`ProxyGroupTopology`] uses for proxy groups (group 11 semantics), so the
//!   two analyzers cannot drift apart;
//! * [`DialerTopology::resolve_chain`] — `node -> dialer -> dialer's dialer`
//!   hops. The chain stops honestly at a proxy group because the static
//!   document cannot know which member the runtime selects; if the node
//!   already participates in a detected cycle the chain carries that loop
//!   instead of pretending to be a valid chain.
//!
//! Nothing here reads the network or the filesystem, and nothing guesses a
//! runtime selection.

use std::collections::{BTreeMap, BTreeSet};

use serde_yaml_ng::Value;

use super::model::ProxyNode;
use super::profile_yaml::parse_profile_yaml;
use crate::rules::analyzer::ProxyGroupTopology;

/// One `proxy-groups:` declaration that participates in dialer resolution.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DialerGroup {
    pub name: String,
    /// `type:` — `select` / `url-test` / `fallback` / `load-balance` / ...
    pub group_type: String,
    /// `proxies:` members, in document order.
    pub members: Vec<String>,
    /// `use:` provider names. Providers are resolved at runtime, so a group
    /// that only lists providers contributes no static member edges.
    pub providers: Vec<String>,
}

impl DialerGroup {
    fn is_relay_group(&self) -> bool {
        self.group_type.trim().eq_ignore_ascii_case("relay")
    }
}

/// What kind of dependency loop a finding describes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DialerCycleKind {
    /// `A -> A`.
    SelfLoop,
    /// `A -> B -> A`.
    Mutual,
    /// A cycle that only walks `proxy-groups[].proxies` member edges.
    GroupCycle,
    /// Any other cycle on the combined dialer/relay graph.
    Circuit,
}

/// One detected dependency cycle on the combined graph.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DialerCycle {
    pub kind: DialerCycleKind,
    /// Closed path, e.g. `["A", "B", "A"]`.
    pub path: Vec<String>,
    /// `true` when at least one edge of the cycle is a `dialer-proxy` edge.
    pub spans_dialer_proxy: bool,
}

/// Where a statically resolved chain ends.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DialerChainEnd {
    /// The last hop declares no `dialer-proxy` at all.
    Complete,
    /// `dialer-proxy` names something the profile does not declare.
    MissingTarget(String),
    /// The next hop is a proxy group; the runtime picks the member.
    GroupBoundary(String),
    /// The chain (or one of its group candidates) closes a loop.
    Cycle(Vec<String>),
}

/// One hop of a resolved chain.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DialerChainHop {
    pub name: String,
    /// mihomo `type` string for a node; the group `type` for a group hop.
    pub node_type: String,
    pub is_group: bool,
    /// Static candidates of a group hop (its `proxies:` members).
    pub candidates: Vec<String>,
}

/// A chain rooted at one node, resolved from the document only.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedDialerChain {
    pub root: String,
    pub hops: Vec<DialerChainHop>,
    pub end: DialerChainEnd,
}

impl ResolvedDialerChain {
    /// `true` when no loop and no dangling reference was found. A
    /// [`DialerChainEnd::GroupBoundary`] stays valid: the group member is a
    /// runtime fact, not a defect.
    pub fn is_valid(&self) -> bool {
        !matches!(
            self.end,
            DialerChainEnd::Cycle(_) | DialerChainEnd::MissingTarget(_)
        )
    }

    /// `true` when the chain shows a dependency loop.
    pub fn is_loop(&self) -> bool {
        matches!(self.end, DialerChainEnd::Cycle(_))
    }
}

/// Static dialer/relay facts of one profile document.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DialerGraphFacts {
    /// node name -> mihomo `type` string.
    pub node_types: BTreeMap<String, String>,
    /// node name -> `dialer-proxy` value (only nodes that declare one).
    pub dialers: BTreeMap<String, String>,
    /// group name -> declaration.
    pub groups: BTreeMap<String, DialerGroup>,
}

impl DialerGraphFacts {
    /// Collect dialer/group facts from parsed domain nodes.
    pub fn from_nodes(nodes: &[ProxyNode]) -> Self {
        let mut facts = Self::default();
        for node in nodes {
            let name = node.name().trim().to_string();
            if name.is_empty() {
                continue;
            }
            facts
                .node_types
                .insert(name.clone(), node.type_name().to_string());
            if let Some(dialer) = dialer_of(node).map(str::trim)
                && !dialer.is_empty()
            {
                facts.dialers.insert(name, dialer.to_string());
            }
        }
        facts
    }

    /// Parse a profile document into dialer/group facts. A profile without a
    /// `proxies:` section is an empty graph, not an error.
    pub fn from_profile_yaml(text: &str) -> anyhow::Result<Self> {
        let nodes = parse_profile_yaml(text)?;
        let mut facts = Self::from_nodes(&nodes);
        let document: Value = serde_yaml_ng::from_str(text)?;
        for group in groups_from_document(&document) {
            facts.groups.insert(group.name.clone(), group);
        }
        Ok(facts)
    }

    /// `true` when no node declares a `dialer-proxy` and no group lists members
    /// or providers; callers use it to stay honestly empty.
    pub fn is_empty(&self) -> bool {
        let no_group_edges = self
            .groups
            .values()
            .all(|group| group.members.is_empty() && group.providers.is_empty());
        self.dialers.is_empty() && no_group_edges
    }

    /// The combined graph: `dialer-proxy` edges plus group member edges. Every
    /// known name is a key so the shared DFS can traverse it.
    pub fn dependency_graph(&self) -> BTreeMap<String, Vec<String>> {
        let mut graph: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for name in self.node_types.keys() {
            graph.entry(name.clone()).or_default();
        }
        for name in self.groups.keys() {
            graph.entry(name.clone()).or_default();
        }
        for (node, dialer) in &self.dialers {
            graph.entry(node.clone()).or_default().push(dialer.clone());
            graph.entry(dialer.clone()).or_default();
        }
        for (name, group) in &self.groups {
            if group.members.is_empty() {
                continue;
            }
            let entry = graph.entry(name.clone()).or_default();
            entry.extend(group.members.iter().cloned());
            for member in &group.members {
                graph.entry(member.clone()).or_default();
            }
        }
        graph
    }

    /// Group-member edges as `(group, member)` pairs, used to classify a cycle
    /// as a pure proxy-group cycle.
    fn group_edges(&self) -> BTreeSet<(String, String)> {
        let mut edges = BTreeSet::new();
        for (name, group) in &self.groups {
            for member in &group.members {
                edges.insert((name.clone(), member.clone()));
            }
        }
        edges
    }
}

fn dialer_of(node: &ProxyNode) -> Option<&str> {
    match node {
        ProxyNode::Vless(node) => node.dialer_proxy.as_deref(),
        ProxyNode::Hysteria2(node) => node.dialer_proxy.as_deref(),
        ProxyNode::Tuic(node) => node.dialer_proxy.as_deref(),
        ProxyNode::WireGuard(node) => node.dialer_proxy.as_deref(),
        ProxyNode::Shadowsocks(node) => node.dialer_proxy.as_deref(),
        ProxyNode::Anytls(node) => node.dialer_proxy.as_deref(),
        ProxyNode::Trojan(node) => node.dialer_proxy.as_deref(),
        ProxyNode::Vmess(node) => node.dialer_proxy.as_deref(),
        ProxyNode::Ssh(node) => node.dialer_proxy.as_deref(),
        ProxyNode::Other(other) => other.fields.get("dialer-proxy").and_then(Value::as_str),
    }
}

/// Read the `proxy-groups:` section into typed declarations. Entries without a
/// usable name are skipped (mihomo rejects them too and the caller reports the
/// document's own validation problem, not a fabricated group).
pub fn groups_from_document(document: &Value) -> Vec<DialerGroup> {
    let Some(Value::Sequence(entries)) = document.get("proxy-groups") else {
        return Vec::new();
    };
    let mut groups = Vec::new();
    for entry in entries {
        let Some(mapping) = entry.as_mapping() else {
            continue;
        };
        let get = |key: &str| mapping.get(Value::String(key.to_string()));
        let Some(name) = get("name").and_then(Value::as_str) else {
            continue;
        };
        let name = name.trim().to_string();
        if name.is_empty() {
            continue;
        }
        let group_type = get("type")
            .and_then(Value::as_str)
            .unwrap_or("select")
            .trim()
            .to_string();
        let string_list = |key: &str| -> Vec<String> {
            match get(key) {
                Some(Value::Sequence(items)) => items
                    .iter()
                    .filter_map(Value::as_str)
                    .map(|item| item.trim().to_string())
                    .filter(|item| !item.is_empty())
                    .collect(),
                Some(Value::String(item)) => vec![item.trim().to_string()],
                _ => Vec::new(),
            }
        };
        groups.push(DialerGroup {
            members: string_list("proxies"),
            providers: string_list("use"),
            name,
            group_type,
        });
    }
    groups
}

/// Static topology analyzer for the dialer/relay graph.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DialerTopology;

impl DialerTopology {
    /// Detect every dependency cycle the shared proxy-group DFS finds on the
    /// combined dialer + relay graph.
    pub fn detect_cycles(facts: &DialerGraphFacts) -> Vec<DialerCycle> {
        let graph = facts.dependency_graph();
        let group_edges = facts.group_edges();
        ProxyGroupTopology::detect_group_cycles(&graph)
            .into_iter()
            .map(|path| classify_cycle(&path, &group_edges))
            .collect()
    }

    /// Cycles that involve `node` (a chain rendered for `node` must never look
    /// valid while a loop through it exists).
    pub fn cycles_containing(facts: &DialerGraphFacts, node: &str) -> Vec<DialerCycle> {
        Self::detect_cycles(facts)
            .into_iter()
            .filter(|cycle| cycle.path.iter().any(|entry| entry == node))
            .collect()
    }

    /// Resolve the static chain of one node.
    pub fn resolve_chain(facts: &DialerGraphFacts, root: &str) -> ResolvedDialerChain {
        let mut hops = vec![hop_for(facts, root)];
        let mut seen = vec![root.to_string()];
        let mut current = root.to_string();
        let mut end = DialerChainEnd::Complete;

        // A cycle through this node makes every chain that starts here a loop;
        // report the real path instead of a chain that looks valid.
        if let Some(cycle) = Self::cycles_containing(facts, root).into_iter().next() {
            let mut path = cycle.path.clone();
            if !path.iter().any(|entry| entry == root) {
                path.push(root.to_string());
            }
            return ResolvedDialerChain {
                root: root.to_string(),
                hops,
                end: DialerChainEnd::Cycle(path),
            };
        }

        while let Some(dialer) = facts.dialers.get(&current).cloned() {
            if seen.contains(&dialer) {
                let mut path: Vec<String> = seen
                    .iter()
                    .skip_while(|entry| *entry != &dialer)
                    .cloned()
                    .collect();
                path.push(dialer.clone());
                end = DialerChainEnd::Cycle(path);
                break;
            }
            if let Some(group) = facts.groups.get(&dialer) {
                hops.push(DialerChainHop {
                    name: group.name.clone(),
                    node_type: group.group_type.clone(),
                    is_group: true,
                    candidates: group.members.clone(),
                });
                end = DialerChainEnd::GroupBoundary(dialer.clone());
                break;
            }
            if !facts.node_types.contains_key(&dialer) {
                end = DialerChainEnd::MissingTarget(dialer.clone());
                break;
            }
            hops.push(hop_for(facts, &dialer));
            seen.push(dialer.clone());
            current = dialer;
        }

        ResolvedDialerChain {
            root: root.to_string(),
            hops,
            end,
        }
    }

    /// Resolve a chain for every declared node, in name order.
    pub fn resolve_all(facts: &DialerGraphFacts) -> Vec<ResolvedDialerChain> {
        facts
            .node_types
            .keys()
            .map(|name| Self::resolve_chain(facts, name))
            .collect()
    }

    /// Document-level warnings the surfaces must show instead of silently
    /// accepting a removed construct: `type: relay` groups were removed from
    /// the core in favour of `dialer-proxy`.
    pub fn group_warnings(facts: &DialerGraphFacts) -> Vec<String> {
        let mut warnings = Vec::new();
        for group in facts.groups.values() {
            if group.is_relay_group() {
                warnings.push(format!(
                    "proxy group `{}` uses the removed `relay` type; the pinned core rejects it, migrate to `dialer-proxy`",
                    group.name
                ));
            }
            if group.members.is_empty() && !group.providers.is_empty() {
                warnings.push(format!(
                    "proxy group `{}` lists only providers (`use:`); its dialer chain candidates are runtime facts",
                    group.name
                ));
            }
        }
        warnings
    }
}

fn hop_for(facts: &DialerGraphFacts, name: &str) -> DialerChainHop {
    DialerChainHop {
        name: name.to_string(),
        node_type: facts
            .node_types
            .get(name)
            .cloned()
            .unwrap_or_else(|| "group".to_string()),
        is_group: false,
        candidates: Vec::new(),
    }
}

fn classify_cycle(path: &[String], group_edges: &BTreeSet<(String, String)>) -> DialerCycle {
    let spans_dialer_proxy = path
        .windows(2)
        .any(|edge| !group_edges.contains(&(edge[0].clone(), edge[1].clone())));
    let kind = if path.len() == 2 && path[0] == path[1] {
        DialerCycleKind::SelfLoop
    } else if !spans_dialer_proxy {
        DialerCycleKind::GroupCycle
    } else if path.len() == 3 && path[0] == path[2] {
        DialerCycleKind::Mutual
    } else {
        DialerCycleKind::Circuit
    };
    DialerCycle {
        kind,
        path: path.to_vec(),
        spans_dialer_proxy,
    }
}

#[cfg(test)]
#[path = "dialer_test.rs"]
mod tests;
