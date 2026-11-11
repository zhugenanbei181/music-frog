//! The traffic audit's reader vocabulary (TRAFFICAUDIT-001).
//!
//! The ranges and the dimensions are the domain's typed values; what lives
//! here is *which* of them a surface offers, in which order, and which copy
//! key names each one. A second surface therefore cannot offer a different set
//! of ranges, order them differently, or name the same range with different
//! words - and the index a segmented control reports maps back to the owner's
//! value in exactly one place.
//!
//! The order and the defaults are the reference product's own: `1h / 24h / 7d /
//! 30d` with `24h` selected, and `sourceIP / host / outbound / process`.

use infiltrator_domain::traffic_audit::{TrafficDimension, TrafficWindow};

/// Every range a surface offers, in the reference product's order.
pub const TRAFFIC_AUDIT_WINDOWS: [TrafficWindow; 4] = [
    TrafficWindow::LastHour,
    TrafficWindow::LastDay,
    TrafficWindow::LastWeek,
    TrafficWindow::LastMonth,
];

/// Every dimension a surface offers, in the reference product's order.
///
/// The owner records two more dimensions (the policy group and the destination
/// country); the reference product's audit offers neither, so neither is a tab
/// here. They stay recorded and readable, not offered.
pub const TRAFFIC_AUDIT_DIMENSIONS: [TrafficDimension; 4] = [
    TrafficDimension::Device,
    TrafficDimension::Domain,
    TrafficDimension::Node,
    TrafficDimension::Process,
];

/// The range at a control's index, clamped to the offered set.
pub fn traffic_audit_window_at(index: usize) -> TrafficWindow {
    TRAFFIC_AUDIT_WINDOWS[index.min(TRAFFIC_AUDIT_WINDOWS.len() - 1)]
}

/// The dimension at a control's index, clamped to the offered set.
pub fn traffic_audit_dimension_at(index: usize) -> TrafficDimension {
    TRAFFIC_AUDIT_DIMENSIONS[index.min(TRAFFIC_AUDIT_DIMENSIONS.len() - 1)]
}

/// The index a range's control shows as selected.
pub fn traffic_audit_window_index(window: TrafficWindow) -> usize {
    TRAFFIC_AUDIT_WINDOWS
        .iter()
        .position(|offered| *offered == window)
        .unwrap_or(0)
}

/// The index a dimension's control shows as selected.
pub fn traffic_audit_dimension_index(dimension: TrafficDimension) -> usize {
    TRAFFIC_AUDIT_DIMENSIONS
        .iter()
        .position(|offered| *offered == dimension)
        .unwrap_or(0)
}

/// The copy key that names a range.
pub fn traffic_audit_window_label_key(window: TrafficWindow) -> &'static str {
    match window {
        TrafficWindow::LastHour => "traffic_1h",
        TrafficWindow::LastDay => "traffic_24h",
        TrafficWindow::LastWeek => "traffic_7d",
        TrafficWindow::LastMonth => "traffic_30d",
    }
}

/// The copy key that names an offered dimension.
///
/// A dimension the surface does not offer has no key: the caller is asking to
/// name a tab that does not exist.
pub fn traffic_audit_dimension_label_key(dimension: TrafficDimension) -> Option<&'static str> {
    match dimension {
        TrafficDimension::Device => Some("traffic_dim_devices"),
        TrafficDimension::Domain => Some("traffic_dim_domains"),
        TrafficDimension::Node => Some("traffic_dim_proxies"),
        TrafficDimension::Process => Some("traffic_dim_processes"),
        TrafficDimension::Group | TrafficDimension::Country => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // An index from a control must come back as the same value, for every
    // position the control can report.
    #[test]
    fn every_offered_index_round_trips() {
        for (index, window) in TRAFFIC_AUDIT_WINDOWS.iter().enumerate() {
            assert_eq!(traffic_audit_window_at(index), *window);
            assert_eq!(traffic_audit_window_index(*window), index);
        }
        for (index, dimension) in TRAFFIC_AUDIT_DIMENSIONS.iter().enumerate() {
            assert_eq!(traffic_audit_dimension_at(index), *dimension);
            assert_eq!(traffic_audit_dimension_index(*dimension), index);
        }
    }

    // The reference product's order and defaults.
    #[test]
    fn the_offer_is_the_reference_order() {
        assert_eq!(
            TRAFFIC_AUDIT_WINDOWS
                .iter()
                .map(|window| traffic_audit_window_label_key(*window))
                .collect::<Vec<_>>(),
            vec!["traffic_1h", "traffic_24h", "traffic_7d", "traffic_30d"]
        );
        assert_eq!(
            TRAFFIC_AUDIT_DIMENSIONS
                .iter()
                .map(|dimension| traffic_audit_dimension_label_key(*dimension))
                .collect::<Vec<_>>(),
            vec![
                Some("traffic_dim_devices"),
                Some("traffic_dim_domains"),
                Some("traffic_dim_proxies"),
                Some("traffic_dim_processes"),
            ]
        );
    }

    // A dimension the surface does not offer has no name, and an index past the
    // end clamps instead of panicking on a stale selection.
    #[test]
    fn an_unoffered_dimension_has_no_key_and_an_out_of_range_index_clamps() {
        assert_eq!(
            traffic_audit_dimension_label_key(TrafficDimension::Country),
            None
        );
        assert_eq!(
            traffic_audit_dimension_label_key(TrafficDimension::Group),
            None
        );
        assert_eq!(traffic_audit_window_at(99), TrafficWindow::LastMonth);
        assert_eq!(traffic_audit_dimension_at(99), TrafficDimension::Process);
        // A selection that is not offered reads as the first tab, so a control
        // never draws with no tab selected.
        assert_eq!(traffic_audit_dimension_index(TrafficDimension::Country), 0);
    }
}
