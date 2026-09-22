//! DUAL-05-09/10: shared dialer-proxy chain and loop vocabulary.
//!
//! `dialer-proxy:` names either a node or a proxy group (upstream docs:
//! `值可以为策略组/出站代理的 name`). The shared application resolves the
//! static chain `node -> dialer -> dialer's dialer` and detects dependency
//! loops over the combined dialer + proxy-group graph
//! (`infiltrator-domain::proxy_nodes::dialer`). This module owns the read model
//! both surfaces render, so Iced and Bevy cannot disagree about a chain.
//!
//! Hard rule: a loop is never a valid chain. [`DialerChainView::valid`] is
//! `false` for every loop and every dangling reference, and
//! [`DialerChainView::chain_line`] renders the real loop path with a warning
//! marker instead of a clean arrow chain.

use serde::{Deserialize, Serialize};

use crate::protocol_fidelity::ProtocolFamily;

/// What a chain hop points at.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DialerHopKind {
    #[default]
    Node,
    Group,
}

impl DialerHopKind {
    pub const fn label_zh(self) -> &'static str {
        match self {
            Self::Node => "节点",
            Self::Group => "策略组",
        }
    }
}

/// One hop of a resolved chain, as both surfaces render it.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DialerHopView {
    pub name: String,
    pub kind: DialerHopKind,
    /// mihomo `type` string (`ss`, `vless`, `select`, `url-test`, ...).
    pub node_type: String,
    /// Shared family label for a node hop; the group `type` for a group hop.
    pub type_label: String,
    /// Static members of a group hop (runtime selection stays unknown).
    pub candidates: Vec<String>,
}

impl DialerHopView {
    /// `nas(vless)` / `dialer(select: hk, jp)`.
    pub fn chip(&self) -> String {
        if self.kind == DialerHopKind::Group && !self.candidates.is_empty() {
            return format!(
                "{}({}: {})",
                self.name,
                self.type_label,
                self.candidates.join(", ")
            );
        }
        format!("{}({})", self.name, self.type_label)
    }
}

/// Where a static chain ends.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DialerChainEnd {
    /// The last hop declares no `dialer-proxy`.
    #[default]
    Complete,
    /// `dialer-proxy` names something the document does not declare.
    MissingTarget { name: String },
    /// The next hop is a proxy group; the member is a runtime fact.
    GroupBoundary { name: String },
    /// The chain closes a dependency loop. Never a valid chain.
    Cycle { path: Vec<String> },
}

impl DialerChainEnd {
    pub fn label_zh(&self) -> String {
        match self {
            Self::Complete => "链路完整".to_string(),
            Self::MissingTarget { name } => format!("跳板 `{name}` 未声明"),
            Self::GroupBoundary { name } => format!("策略组 `{name}` 成员由运行时选择"),
            Self::Cycle { path } => format!("依赖环路 {}", path.join(" → ")),
        }
    }
}

/// A chain rooted at one node, as both surfaces render it.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DialerChainView {
    pub root: String,
    pub hops: Vec<DialerHopView>,
    pub end: DialerChainEnd,
}

impl DialerChainView {
    /// `false` for loops and dangling references. A group boundary stays valid:
    /// the member is a runtime fact, not a defect.
    pub fn valid(&self) -> bool {
        !self.has_loop() && !matches!(self.end, DialerChainEnd::MissingTarget { .. })
    }

    pub fn has_loop(&self) -> bool {
        matches!(self.end, DialerChainEnd::Cycle { .. })
    }

    /// The chain line both surfaces render. A loop renders the real path with a
    /// warning marker, never a clean chain.
    pub fn chain_line(&self) -> String {
        let mut parts: Vec<String> = self.hops.iter().map(DialerHopView::chip).collect();
        match &self.end {
            DialerChainEnd::Complete => {}
            DialerChainEnd::MissingTarget { name } => parts.push(format!("{name}(缺失)")),
            DialerChainEnd::GroupBoundary { name } => parts.push(format!("{name}(策略组)")),
            DialerChainEnd::Cycle { path } => {
                return format!("{} ⛔ 环路 {}", parts.join(" → "), path.join(" → "));
            }
        }
        parts.join(" → ")
    }

    pub fn warning_lines(&self) -> Vec<String> {
        match &self.end {
            DialerChainEnd::MissingTarget { name } => vec![format!(
                "`dialer-proxy: {name}` does not name a declared node or proxy group; the core would fail to build this chain"
            )],
            DialerChainEnd::Cycle { path } => vec![format!(
                "the chain is a dependency loop (`{}`); it is not a usable chain",
                path.join(" -> ")
            )],
            DialerChainEnd::GroupBoundary { name } => vec![format!(
                "the chain continues through proxy group `{name}`; the selected member is a runtime fact"
            )],
            DialerChainEnd::Complete => Vec::new(),
        }
    }

    pub fn chips(&self) -> Vec<String> {
        let mut chips = vec![format!("chain:{}", self.hops.len())];
        if let Some(second) = self.hops.get(1) {
            chips.push(format!("via:{}", second.type_label));
        }
        if self.has_loop() {
            chips.push("loop:yes".to_string());
        }
        chips
    }

    /// One-line zh summary for compact slots.
    pub fn summary_zh(&self) -> String {
        format!("{} · {}", self.chain_line(), self.end.label_zh())
    }
}

/// Kind of dependency loop found on the dialer/relay graph.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DialerLoopKind {
    /// `A -> A`.
    SelfLoop,
    /// `A -> B -> A`.
    #[default]
    Mutual,
    /// A loop that only walks `proxy-groups[].proxies` member edges.
    RelayGroupCycle,
    /// Any other loop on the combined dialer/relay graph.
    Circuit,
}

impl DialerLoopKind {
    pub const fn label_zh(self) -> &'static str {
        match self {
            Self::SelfLoop => "自引用",
            Self::Mutual => "互相引用",
            Self::RelayGroupCycle => "策略组环",
            Self::Circuit => "多跳环路",
        }
    }
}

/// One typed loop finding. Both surfaces render [`DialerLoopFinding::message`].
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DialerLoopFinding {
    pub kind: DialerLoopKind,
    /// Closed path, e.g. `["A", "B", "A"]`.
    pub path: Vec<String>,
    /// `true` when at least one edge is a `dialer-proxy` edge.
    pub spans_dialer_proxy: bool,
    pub message: String,
}

impl DialerLoopFinding {
    pub fn chip(&self) -> String {
        format!("loop:{}", self.path.join("→"))
    }
}

/// DUAL-05-09/10 read model: chains + loops for one profile document.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DialerChainReport {
    #[serde(default)]
    pub chains: Vec<DialerChainView>,
    #[serde(default)]
    pub loops: Vec<DialerLoopFinding>,
    /// Document-level facts (removed `relay` groups, provider-only groups).
    #[serde(default)]
    pub warnings: Vec<String>,
}

impl DialerChainReport {
    pub fn is_empty(&self) -> bool {
        self.chains.is_empty() && self.loops.is_empty() && self.warnings.is_empty()
    }

    pub fn chain_for(&self, node: &str) -> Option<&DialerChainView> {
        self.chains.iter().find(|chain| chain.root == node)
    }

    pub fn chain_for_mut(&mut self, node: &str) -> Option<&mut DialerChainView> {
        self.chains.iter_mut().find(|chain| chain.root == node)
    }

    /// Valid chains only: a loop is never presented as a valid chain.
    pub fn valid_chains(&self) -> Vec<&DialerChainView> {
        self.chains.iter().filter(|chain| chain.valid()).collect()
    }

    pub fn loop_for(&self, node: &str) -> Option<&DialerLoopFinding> {
        self.loops
            .iter()
            .find(|finding| finding.path.iter().any(|entry| entry == node))
    }

    pub fn has_loops(&self) -> bool {
        !self.loops.is_empty()
    }

    pub fn loop_lines(&self) -> Vec<String> {
        self.loops
            .iter()
            .map(|loop_| loop_.message.clone())
            .collect()
    }

    /// Chips for the first valid chain (if any) + one per loop.
    pub fn chips(&self) -> Vec<String> {
        let mut chips = Vec::new();
        if let Some(chain) = self.valid_chains().first() {
            chips.extend(chain.chips());
        }
        for finding in &self.loops {
            chips.push(finding.chip());
        }
        chips
    }

    pub fn summary_zh(&self) -> String {
        if self.is_empty() {
            return "无前置跳板链路".to_string();
        }
        format!(
            "跳板链路 {} 条（可用 {}）· 环路 {} 处 · 提示 {} 条",
            self.chains.len(),
            self.valid_chains().len(),
            self.loops.len(),
            self.warnings.len()
        )
    }
}

/// DUAL-05-13: `true` when the draft's `dialer_proxy` names a group instead of
/// a node, given the profile's group names — surfaces use it to label the
/// chain boundary honestly. Kept here so both surfaces share the rule.
pub fn target_is_group(group_names: &[String], target: &str) -> bool {
    group_names.iter().any(|name| name == target)
}
/// Family label used by chain hops; a single shared mapping.
pub fn hop_type_label(node_type: &str, is_group: bool) -> String {
    if is_group {
        return node_type.to_string();
    }
    ProtocolFamily::from_type_str(node_type)
        .label_zh()
        .to_string()
}

/// `true` when the mihomo `type` string is a proxy-group type rather than a
/// node protocol; used when a hop's kind has to be recovered from text alone.
pub fn is_group_type(node_type: &str) -> bool {
    matches!(
        node_type.trim(),
        "select" | "url-test" | "fallback" | "load-balance" | "relay" | "compatible"
    )
}

/// DUAL-05-10: the finding a loop produces must never read like a valid chain;
/// this helper is the one place both surfaces build the zh warning text.
pub fn loop_message(kind: DialerLoopKind, path: &[String]) -> String {
    format!(
        "检测到{}依赖环路：{}；该链路不可用",
        kind.label_zh(),
        path.join(" → ")
    )
}

#[cfg(test)]
#[path = "dialer_chain_test.rs"]
mod tests;
