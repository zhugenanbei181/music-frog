//! Shared read model and actions for Overview card layout and longitudinal reordering.

use serde::{Deserialize, Serialize};

/// Enumeration of distinct reorderable cards on the Overview page.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OverviewCardKind {
    /// Proxy running mode segmented controller (Rule / Global / Direct / Script).
    ModeSegment,
    /// Real-time dual-channel GPU bezier traffic waveform chart.
    Traffic,
    /// Six-item core resource & telemetry metrics grid.
    Metrics,
    /// System proxy and TUN mode master switch cards.
    MasterSwitches,
    /// High-fidelity active outbound exit node card.
    ActiveExit,
    /// Public IP privacy & ISP location probe card.
    PublicIp,
    /// Multi-stage traffic routing topology flow chain.
    Topology,
    /// Subscription quota usage and billing expiry dashboard.
    Quota,
}

impl OverviewCardKind {
    /// Canonical default visual order of cards on the Overview page.
    pub const DEFAULT_ORDER: [Self; 8] = [
        Self::ModeSegment,
        Self::Traffic,
        Self::Metrics,
        Self::MasterSwitches,
        Self::ActiveExit,
        Self::PublicIp,
        Self::Topology,
        Self::Quota,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ModeSegment => "mode_segment",
            Self::Traffic => "traffic",
            Self::Metrics => "metrics",
            Self::MasterSwitches => "master_switches",
            Self::ActiveExit => "active_exit",
            Self::PublicIp => "public_ip",
            Self::Topology => "topology",
            Self::Quota => "quota",
        }
    }

    pub fn from_str_loose(s: &str) -> Option<Self> {
        let clean = s.trim().to_ascii_lowercase().replace(['-', '_'], "");
        match clean.as_str() {
            "modesegment" | "mode" => Some(Self::ModeSegment),
            "traffic" | "waveform" | "chart" => Some(Self::Traffic),
            "metrics" | "stats" | "chips" => Some(Self::Metrics),
            "masterswitches" | "switches" | "masters" => Some(Self::MasterSwitches),
            "activeexit" | "exit" | "node" => Some(Self::ActiveExit),
            "publicip" | "ip" | "probe" => Some(Self::PublicIp),
            "topology" | "chain" => Some(Self::Topology),
            "quota" | "subscription" => Some(Self::Quota),
            _ => None,
        }
    }
}

/// Shared layout snapshot indicating the sequential display order of Overview cards.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OverviewLayoutSnapshot {
    pub order: Vec<OverviewCardKind>,
    pub is_customized: bool,
}

impl Default for OverviewLayoutSnapshot {
    fn default() -> Self {
        Self {
            order: OverviewCardKind::DEFAULT_ORDER.to_vec(),
            is_customized: false,
        }
    }
}

impl OverviewLayoutSnapshot {
    pub fn new(order: Vec<OverviewCardKind>) -> Self {
        let is_customized = order != OverviewCardKind::DEFAULT_ORDER.to_vec();
        Self { order, is_customized }
    }

    /// Explicit fixture for demo/screenshot hosts.
    pub fn demo_fixture() -> Self {
        Self::default()
    }

    /// Move a card up in the layout order. Returns true if position changed.
    pub fn move_up(&mut self, kind: OverviewCardKind) -> bool {
        let Some(pos) = self.order.iter().position(|&k| k == kind) else {
            return false;
        };
        if pos > 0 {
            self.order.swap(pos, pos - 1);
            self.is_customized = self.order != OverviewCardKind::DEFAULT_ORDER.to_vec();
            true
        } else {
            false
        }
    }

    /// Move a card down in the layout order. Returns true if position changed.
    pub fn move_down(&mut self, kind: OverviewCardKind) -> bool {
        let Some(pos) = self.order.iter().position(|&k| k == kind) else {
            return false;
        };
        if pos + 1 < self.order.len() {
            self.order.swap(pos, pos + 1);
            self.is_customized = self.order != OverviewCardKind::DEFAULT_ORDER.to_vec();
            true
        } else {
            false
        }
    }

    /// Reorder a card from one position to another. Returns true if valid.
    pub fn reorder(&mut self, from: usize, to: usize) -> bool {
        if from >= self.order.len() || to >= self.order.len() {
            return false;
        }
        if from == to {
            return false;
        }
        let item = self.order.remove(from);
        self.order.insert(to, item);
        self.is_customized = self.order != OverviewCardKind::DEFAULT_ORDER.to_vec();
        true
    }

    /// Reset layout back to default canonical order.
    pub fn reset_to_default(&mut self) {
        self.order = OverviewCardKind::DEFAULT_ORDER.to_vec();
        self.is_customized = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_order_covers_all_eight_overview_cards() {
        let snapshot = OverviewLayoutSnapshot::default();
        assert_eq!(snapshot.order.len(), 8);
        assert!(!snapshot.is_customized);
        assert_eq!(snapshot.order[0], OverviewCardKind::ModeSegment);
        assert_eq!(snapshot.order[7], OverviewCardKind::Quota);
    }

    #[test]
    fn reorder_and_move_operations_work() {
        let mut snapshot = OverviewLayoutSnapshot::default();
        assert!(snapshot.move_down(OverviewCardKind::ModeSegment));
        assert!(snapshot.is_customized);
        assert_eq!(snapshot.order[0], OverviewCardKind::Traffic);
        assert_eq!(snapshot.order[1], OverviewCardKind::ModeSegment);

        assert!(snapshot.move_up(OverviewCardKind::ModeSegment));
        assert!(!snapshot.is_customized);
        assert_eq!(snapshot.order[0], OverviewCardKind::ModeSegment);

        assert!(snapshot.reorder(0, 3));
        assert!(snapshot.is_customized);
        snapshot.reset_to_default();
        assert!(!snapshot.is_customized);
    }
}
