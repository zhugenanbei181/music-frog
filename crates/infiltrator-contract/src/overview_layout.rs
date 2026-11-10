//! Shared read model and actions for Overview card layout and longitudinal reordering.

use serde::{Deserialize, Serialize};

/// Regions that own a shell control or a reorderable Overview card.
///
/// The split is about *who lays the entry out*, not about what it shows: the
/// dashboard band is the owner's reorderable content, while a fixed control is
/// mounted by a surface wherever that surface puts its controls. The name
/// `ShellSidebar` was retired because it was read as a location: the Bevy
/// surface took it literally and painted the routing selector and the master
/// switches in the sidebar *and* in the dashboard band, and the sidebar copies
/// were the ones the shared layout order could not reach. Slint, the surface
/// the space-utilization judge measures, has always mounted both in the
/// dashboard (`ui/pages/home_desktop_grid.slint`), and the desktop reference
/// records that the sidebar carries navigation only
/// (`docs/reference/FLCLASH_DESKTOP_REFERENCE.md` §2, `chrome_band` 0%).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum OverviewCardRegion {
    /// The card belongs to the Overview page's reorderable band.
    Overview,
    /// The control is a fixed dashboard control: the Overview page mounts it,
    /// outside the reorderable band, and no other region may mount a second
    /// copy of it.
    DashboardFixed,
}

/// Enumeration of distinct shell and Overview controls represented by the
/// shared layout contract.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OverviewCardKind {
    /// Shared traffic routing intent selector.
    RoutingIntentSegment,
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
    pub const DEFAULT_ORDER: [Self; 6] = [
        Self::Traffic,
        Self::Metrics,
        Self::ActiveExit,
        Self::PublicIp,
        Self::Topology,
        Self::Quota,
    ];

    /// The dashboard's fixed controls: the Overview page mounts them, outside
    /// the reorderable band, and nowhere else. They are excluded from
    /// [`Self::DEFAULT_ORDER`] because the owner's order cannot move a control
    /// that is not part of the band — not because a sidebar owns them.
    pub const DASHBOARD_FIXED_ORDER: [Self; 2] = [Self::RoutingIntentSegment, Self::MasterSwitches];

    /// The single region decision for one contract entry.
    pub const fn region(self) -> OverviewCardRegion {
        match self {
            Self::RoutingIntentSegment | Self::MasterSwitches => OverviewCardRegion::DashboardFixed,
            _ => OverviewCardRegion::Overview,
        }
    }

    /// Whether the entry may be mounted as an Overview card.
    pub const fn is_overview_card(self) -> bool {
        matches!(self.region(), OverviewCardRegion::Overview)
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::RoutingIntentSegment => "routing_intent_segment",
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
            "traffic" | "waveform" | "chart" => Some(Self::Traffic),
            "metrics" | "stats" | "chips" => Some(Self::Metrics),
            "activeexit" | "exit" | "node" => Some(Self::ActiveExit),
            "publicip" | "ip" | "probe" => Some(Self::PublicIp),
            "topology" | "chain" => Some(Self::Topology),
            "quota" | "subscription" => Some(Self::Quota),
            _ => None,
        }
    }
}

/// The direction of a one-step Overview card reorder.
///
/// The surface names the card and the direction; the shared owner applies the
/// swap against its own snapshot, so no surface derives the next order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OverviewCardMoveDirection {
    /// Swap the card with the entry directly above it.
    Up,
    /// Swap the card with the entry directly below it.
    Down,
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
        let order: Vec<_> = order
            .into_iter()
            .filter(|kind| kind.is_overview_card())
            .collect();
        let is_customized = order != OverviewCardKind::DEFAULT_ORDER.to_vec();
        Self {
            order,
            is_customized,
        }
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

    /// Move a card one step in the requested direction. Returns true if the
    /// position changed.
    pub fn move_card(
        &mut self,
        kind: OverviewCardKind,
        direction: OverviewCardMoveDirection,
    ) -> bool {
        match direction {
            OverviewCardMoveDirection::Up => self.move_up(kind),
            OverviewCardMoveDirection::Down => self.move_down(kind),
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
    fn default_order_covers_the_six_overview_cards() {
        let snapshot = OverviewLayoutSnapshot::default();
        assert_eq!(snapshot.order.len(), 6);
        assert!(!snapshot.is_customized);
        assert_eq!(snapshot.order[0], OverviewCardKind::Traffic);
        assert_eq!(snapshot.order[5], OverviewCardKind::Quota);
        assert_eq!(
            OverviewCardKind::DASHBOARD_FIXED_ORDER
                .iter()
                .filter(|kind| kind.is_overview_card())
                .count(),
            0
        );
    }

    #[test]
    fn reorder_and_move_operations_work() {
        let mut snapshot = OverviewLayoutSnapshot::default();
        assert!(snapshot.move_down(OverviewCardKind::Traffic));
        assert!(snapshot.is_customized);
        assert_eq!(snapshot.order[0], OverviewCardKind::Metrics);
        assert_eq!(snapshot.order[1], OverviewCardKind::Traffic);

        assert!(snapshot.move_up(OverviewCardKind::Traffic));
        assert!(!snapshot.is_customized);
        assert_eq!(snapshot.order[0], OverviewCardKind::Traffic);

        assert!(snapshot.reorder(0, 3));
        assert!(snapshot.is_customized);
        snapshot.reset_to_default();
        assert!(!snapshot.is_customized);
    }

    #[test]
    fn legacy_sidebar_controls_are_dropped_from_the_overview_order() {
        let mut snapshot = OverviewLayoutSnapshot::new(vec![
            OverviewCardKind::RoutingIntentSegment,
            OverviewCardKind::Traffic,
            OverviewCardKind::MasterSwitches,
        ]);

        assert_eq!(snapshot.order, vec![OverviewCardKind::Traffic]);
        assert!(!snapshot.move_card(
            OverviewCardKind::RoutingIntentSegment,
            OverviewCardMoveDirection::Down
        ));
    }
}
