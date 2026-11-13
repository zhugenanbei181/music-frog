//! The one owner of the connections page's control chrome.
//!
//! Which controls the page mounts, and which of them share a row, is a page
//! fact rather than a surface fact. A surface that answers it on its own
//! shows the reader a different page for the same state, and a narrow window
//! pays for the difference in the only currency it has: vertical space. A
//! control band the reader cannot see is a band that pushes the rows
//! themselves below the fold, so the arrangement is published once here from
//! the shared viewport ladder, and a surface only maps the published bands to
//! widgets and dispatches the typed intents back.

use infiltrator_contract::connection_view_preference::ConnectionViewMode;
use infiltrator_contract::responsive_viewport::ViewportTier;

/// One control band of the connections page.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ConnectionControlBand {
    /// The Active / Closed sub-tabs.
    Tabs,
    /// The grouping control.
    Grouping,
    /// The search field.
    Search,
    /// The List / Table morphology control.
    Morphology,
    /// The close-filtered action.
    CloseFiltered,
}

impl ConnectionControlBand {
    /// Every band the page can mount, in the order the page reads them.
    pub const ALL: [ConnectionControlBand; 5] = [
        ConnectionControlBand::Tabs,
        ConnectionControlBand::Grouping,
        ConnectionControlBand::Search,
        ConnectionControlBand::Morphology,
        ConnectionControlBand::CloseFiltered,
    ];
}

/// The control bands of the connections page, grouped into the rows they share.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConnectionControlStack {
    rows: Vec<Vec<ConnectionControlBand>>,
}

impl ConnectionControlStack {
    /// The rows of bands, in the order the page stacks them.
    pub fn rows(&self) -> &[Vec<ConnectionControlBand>] {
        &self.rows
    }

    /// The number of rows the stack spends on the page's vertical budget.
    pub fn row_count(&self) -> usize {
        self.rows.len()
    }

    /// The number of bands the stack spends.
    pub fn band_count(&self) -> usize {
        self.rows.iter().map(Vec::len).sum()
    }

    /// The row a band shares, or `None` when the band is not mounted.
    pub fn row_of(&self, band: ConnectionControlBand) -> Option<usize> {
        self.rows.iter().position(|row| row.contains(&band))
    }

    /// Whether two mounted bands share a row.
    pub fn shares_row_with(
        &self,
        band: ConnectionControlBand,
        other: ConnectionControlBand,
    ) -> bool {
        self.row_of(band)
            .is_some_and(|row| self.rows[row].contains(&other))
    }
}

/// Whether the grouping control is mounted for the given morphology.
///
/// The table shows one row per connection, so the grouping buckets have no
/// place there: a grouping control in the table morphology would fold rows
/// the table cannot show. The rule belongs to the page, not to a surface.
pub const fn grouping_control_mounted(mode: ConnectionViewMode) -> bool {
    matches!(mode, ConnectionViewMode::List)
}

/// Whether the column chooser is mounted for the given morphology.
///
/// The chooser picks the columns of a table, so the list morphology has no
/// columns to pick.
pub const fn column_chooser_mounted(mode: ConnectionViewMode) -> bool {
    matches!(mode, ConnectionViewMode::Table)
}

/// The control stack the connections page presents at a viewport tier.
///
/// The tier is the shared ladder's fact; the arrangement that follows from it
/// is published here so that no surface keeps a second copy of the rule. A
/// wide window has the vertical room for one band per control, while a narrow
/// window folds the tabs, the morphology and the filtered-close action into a
/// single row and keeps the search field on a row of its own, because a
/// search field is the one band the reader cannot do without.
pub fn connection_control_stack(
    tier: ViewportTier,
    mode: ConnectionViewMode,
) -> ConnectionControlStack {
    let mut rows = Vec::new();
    if tier.is_narrow() {
        rows.push(vec![
            ConnectionControlBand::Tabs,
            ConnectionControlBand::Morphology,
            ConnectionControlBand::CloseFiltered,
        ]);
        if grouping_control_mounted(mode) {
            rows.push(vec![ConnectionControlBand::Grouping]);
        }
        rows.push(vec![ConnectionControlBand::Search]);
    } else {
        let mut row = vec![
            ConnectionControlBand::Tabs,
            ConnectionControlBand::Search,
            ConnectionControlBand::Morphology,
            ConnectionControlBand::CloseFiltered,
        ];
        if grouping_control_mounted(mode) {
            row.insert(1, ConnectionControlBand::Grouping);
        }
        rows.push(row);
    }
    ConnectionControlStack { rows }
}

/// Where the traffic summary rides in the connections page.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConnectionTrafficPlacement {
    /// The summary keeps a row of its own under the page header.
    OwnRow,
    /// The summary rides in the page header band, beside the title.
    HeaderBand,
}

/// How many rows the column chooser takes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConnectionChooserPlacement {
    /// The chooser's title sits above the scrolling column strip.
    TwoRows,
    /// The chooser's title rides on the strip's own row.
    OneRow,
}

/// The chrome decisions a viewport tier makes for the connections page.
///
/// A narrow window cannot pay for a band per fact. The facts themselves stay
/// published - the traffic summary is still on the page, the column chooser
/// is still reachable - but they share the rows they can share, because the
/// rows the reader cannot see are the rows the table does not get.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ConnectionPageChrome {
    /// Where the traffic summary rides.
    pub traffic: ConnectionTrafficPlacement,
    /// How many rows the column chooser takes.
    pub chooser: ConnectionChooserPlacement,
}

impl ConnectionPageChrome {
    /// The chrome a wide window presents.
    pub const fn wide() -> Self {
        Self {
            traffic: ConnectionTrafficPlacement::OwnRow,
            chooser: ConnectionChooserPlacement::TwoRows,
        }
    }

    /// The chrome a narrow window presents.
    pub const fn narrow() -> Self {
        Self {
            traffic: ConnectionTrafficPlacement::HeaderBand,
            chooser: ConnectionChooserPlacement::OneRow,
        }
    }
}

/// The page chrome a viewport tier presents for the connections page.
pub const fn connection_page_chrome(tier: ViewportTier) -> ConnectionPageChrome {
    if tier.is_narrow() {
        ConnectionPageChrome::narrow()
    } else {
        ConnectionPageChrome::wide()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_narrow_window_folds_the_control_stack_into_fewer_rows() {
        let table = connection_control_stack(ViewportTier::Compact, ConnectionViewMode::Table);
        assert_eq!(table.row_count(), 2);
        assert_eq!(table.band_count(), 4);
        let list = connection_control_stack(ViewportTier::Compact, ConnectionViewMode::List);
        assert_eq!(list.row_count(), 3);
        assert_eq!(list.band_count(), 5);
    }

    #[test]
    fn a_narrow_window_keeps_the_search_field_on_its_own_row() {
        let stack = connection_control_stack(ViewportTier::Compact, ConnectionViewMode::Table);
        assert_eq!(stack.row_of(ConnectionControlBand::Search), Some(1));
        assert!(!stack.shares_row_with(ConnectionControlBand::Search, ConnectionControlBand::Tabs));
    }

    #[test]
    fn the_morphology_and_the_filtered_close_share_one_row_when_narrow() {
        let stack = connection_control_stack(ViewportTier::Compact, ConnectionViewMode::Table);
        assert!(stack.shares_row_with(
            ConnectionControlBand::Morphology,
            ConnectionControlBand::CloseFiltered
        ));
        assert!(stack.shares_row_with(
            ConnectionControlBand::Morphology,
            ConnectionControlBand::Tabs
        ));
    }

    #[test]
    fn a_wide_window_keeps_every_band_on_one_row() {
        let stack = connection_control_stack(ViewportTier::Expanded, ConnectionViewMode::List);
        assert_eq!(stack.row_count(), 1);
        assert_eq!(stack.band_count(), 5);
        assert!(stack.shares_row_with(
            ConnectionControlBand::Grouping,
            ConnectionControlBand::Search
        ));
    }

    #[test]
    fn the_grouping_band_is_mounted_by_the_list_morphology_only() {
        let list = connection_control_stack(ViewportTier::Expanded, ConnectionViewMode::List);
        let table = connection_control_stack(ViewportTier::Expanded, ConnectionViewMode::Table);
        assert!(list.row_of(ConnectionControlBand::Grouping).is_some());
        assert!(table.row_of(ConnectionControlBand::Grouping).is_none());
        assert!(grouping_control_mounted(ConnectionViewMode::List));
        assert!(!grouping_control_mounted(ConnectionViewMode::Table));
    }

    #[test]
    fn the_column_chooser_is_mounted_by_the_table_morphology_only() {
        assert!(column_chooser_mounted(ConnectionViewMode::Table));
        assert!(!column_chooser_mounted(ConnectionViewMode::List));
    }

    #[test]
    fn every_band_is_mounted_exactly_once() {
        for tier in [
            ViewportTier::Compact,
            ViewportTier::Medium,
            ViewportTier::Expanded,
            ViewportTier::Ultra,
        ] {
            for mode in [ConnectionViewMode::List, ConnectionViewMode::Table] {
                let stack = connection_control_stack(tier, mode);
                let mut mounted = Vec::new();
                for row in stack.rows() {
                    for band in row {
                        assert!(!mounted.contains(band), "band mounted twice: {band:?}");
                        mounted.push(*band);
                    }
                }
                for band in ConnectionControlBand::ALL {
                    if band == ConnectionControlBand::Grouping {
                        assert_eq!(mounted.contains(&band), grouping_control_mounted(mode));
                        continue;
                    }
                    assert!(mounted.contains(&band), "band not mounted: {band:?}");
                }
            }
        }
    }

    #[test]
    fn a_narrow_window_folds_the_traffic_summary_into_the_header_band() {
        let chrome = connection_page_chrome(ViewportTier::Compact);
        assert_eq!(chrome.traffic, ConnectionTrafficPlacement::HeaderBand);
        assert_eq!(
            connection_page_chrome(ViewportTier::Medium).traffic,
            ConnectionTrafficPlacement::HeaderBand
        );
    }

    #[test]
    fn a_narrow_window_folds_the_column_chooser_into_one_row() {
        assert_eq!(
            connection_page_chrome(ViewportTier::Compact).chooser,
            ConnectionChooserPlacement::OneRow
        );
    }

    #[test]
    fn a_wide_window_gives_every_band_a_row_of_its_own() {
        for tier in [ViewportTier::Expanded, ViewportTier::Ultra] {
            let chrome = connection_page_chrome(tier);
            assert_eq!(chrome.traffic, ConnectionTrafficPlacement::OwnRow);
            assert_eq!(chrome.chooser, ConnectionChooserPlacement::TwoRows);
        }
    }
}
