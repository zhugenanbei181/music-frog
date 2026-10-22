//! The incremental list filter and the four-dimension connection order
//! (EFFECTS-009 #33, #34).
//!
//! A live list changes one row at a time, so re-filtering the whole list on
//! every refresh is wasted work. The projector therefore owns the *index* of
//! the rows that matched and the owner tells it what changed: one row that
//! changed is re-evaluated, the rest keep the decision they already got.
//!
//! The projector copies nothing. It holds the decisions it made and the
//! indices they belong to; the rows themselves stay where the owner keeps
//! them, so there is no second copy of a row fact anywhere.

use crate::filter_query::FilterQuery;
use std::cmp::Ordering;

/// The facts one row exposes to the shared filter.
pub trait FilterRow {
    /// The stable identity of the row.
    fn row_id(&self) -> &str;

    /// The text the query matches against.
    fn row_text(&self) -> &str;

    /// Whether the row satisfies the query.
    fn row_matches(&self, query: &FilterQuery) -> bool;
}

/// What one refresh of the projector did.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FilterRefresh {
    /// How many rows were evaluated again.
    pub reevaluated: usize,
    /// How many rows matched after the refresh.
    pub matched: usize,
}

/// The projector of a filtered list with an index cache.
#[derive(Clone, Debug, Default)]
pub struct FilterProjector {
    query: FilterQuery,
    decisions: Vec<bool>,
    matched: Vec<usize>,
}

impl FilterProjector {
    /// A projector that keeps every row.
    pub fn new() -> Self {
        Self::default()
    }

    /// The query the projector applies.
    pub fn query(&self) -> &FilterQuery {
        &self.query
    }

    /// Apply another query.
    ///
    /// A new query invalidates every decision, so the next refresh evaluates
    /// the whole list again.
    pub fn set_query(&mut self, query: FilterQuery) {
        if self.query != query {
            self.query = query;
            self.decisions.clear();
            self.matched.clear();
        }
    }

    /// The indices of the rows that match, in list order.
    pub fn matched(&self) -> &[usize] {
        &self.matched
    }

    /// Whether the row at `index` matches.
    pub fn is_matched(&self, index: usize) -> bool {
        self.decisions.get(index).copied().unwrap_or(false)
    }

    /// Evaluate every row again.
    pub fn reset<R: FilterRow>(&mut self, rows: &[R]) -> FilterRefresh {
        self.decisions.clear();
        self.matched.clear();
        let mut refresh = FilterRefresh::default();
        for (index, row) in rows.iter().enumerate() {
            self.evaluate(index, row, &mut refresh);
        }
        refresh.matched = self.matched.len();
        refresh
    }

    /// Evaluate one row again after it changed.
    ///
    /// The rows before and after the changed row keep the decisions they
    /// already have, which is what makes a live refresh cheap.
    pub fn upsert<R: FilterRow>(&mut self, rows: &[R], index: usize) -> FilterRefresh {
        let mut refresh = FilterRefresh::default();
        let Some(row) = rows.get(index) else {
            refresh.matched = self.matched.len();
            return refresh;
        };
        self.evaluate(index, row, &mut refresh);
        refresh.matched = self.matched.len();
        refresh
    }

    /// Drop one row that the owner removed.
    pub fn remove(&mut self, index: usize) -> FilterRefresh {
        if index >= self.decisions.len() {
            return FilterRefresh {
                reevaluated: 0,
                matched: self.matched.len(),
            };
        }
        self.decisions.remove(index);
        self.matched.retain(|value| *value != index);
        for value in &mut self.matched {
            if *value > index {
                *value -= 1;
            }
        }
        FilterRefresh {
            reevaluated: 0,
            matched: self.matched.len(),
        }
    }

    fn evaluate<R: FilterRow>(&mut self, index: usize, row: &R, refresh: &mut FilterRefresh) {
        refresh.reevaluated += 1;
        let matched = row.row_matches(&self.query);
        let previous = self.decisions.get(index).copied();
        if previous == Some(matched) {
            return;
        }
        if index >= self.decisions.len() {
            self.decisions.resize(index + 1, false);
        }
        self.decisions[index] = matched;
        if matched && !previous.unwrap_or(false) {
            let position = self.matched.partition_point(|value| *value < index);
            self.matched.insert(position, index);
        } else if !matched
            && previous.unwrap_or(false)
            && let Ok(position) = self.matched.binary_search(&index)
        {
            self.matched.remove(position);
        }
    }
}

/// The direction of one sort dimension.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SortDirection {
    /// The smallest value first.
    Ascending,
    /// The largest value first.
    Descending,
}

/// The four dimensions the connections list is ordered by.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConnectionSortDimension {
    /// The total traffic of the connection.
    Traffic,
    /// The instantaneous rate of the connection.
    Rate,
    /// The destination host of the connection.
    Host,
    /// The moment the connection started.
    Started,
}

/// One row of the connections list, as the list orders it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConnectionOrderRow {
    /// The display name of the connection.
    pub name: String,
    /// The destination host of the connection.
    pub host: String,
    /// The total traffic of the connection, in bytes.
    pub traffic_bytes: u64,
    /// The instantaneous rate of the connection, in bytes per second.
    pub rate_bytes_per_second: u64,
    /// The moment the connection started, in epoch milliseconds.
    pub started_at_ms: i64,
}

/// Order a list of connections by one of the four dimensions.
///
/// The other three dimensions are the tie-break chain, and the chain is the
/// same on every surface: the total traffic, then the instantaneous rate,
/// then the destination host, then the display name. A list ordered by one
/// fact still has to break the ties of that fact, and the tie-break must not
/// depend on which surface drew it.
pub fn sort_connections(
    rows: &mut [ConnectionOrderRow],
    dimension: ConnectionSortDimension,
    direction: SortDirection,
) {
    rows.sort_by(|left, right| compare_rows(left, right, dimension, direction));
}

fn compare_rows(
    left: &ConnectionOrderRow,
    right: &ConnectionOrderRow,
    dimension: ConnectionSortDimension,
    direction: SortDirection,
) -> Ordering {
    let primary = match dimension {
        ConnectionSortDimension::Traffic => left.traffic_bytes.cmp(&right.traffic_bytes),
        ConnectionSortDimension::Rate => {
            left.rate_bytes_per_second.cmp(&right.rate_bytes_per_second)
        }
        ConnectionSortDimension::Host => left.host.cmp(&right.host),
        ConnectionSortDimension::Started => left.started_at_ms.cmp(&right.started_at_ms),
    };
    let primary = match direction {
        SortDirection::Ascending => primary,
        SortDirection::Descending => primary.reverse(),
    };
    primary
        .then_with(|| right.traffic_bytes.cmp(&left.traffic_bytes))
        .then_with(|| right.rate_bytes_per_second.cmp(&left.rate_bytes_per_second))
        .then_with(|| left.host.cmp(&right.host))
        .then_with(|| left.name.cmp(&right.name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone)]
    struct Row {
        id: String,
        text: String,
    }

    impl FilterRow for Row {
        fn row_id(&self) -> &str {
            &self.id
        }

        fn row_text(&self) -> &str {
            &self.text
        }

        fn row_matches(&self, query: &FilterQuery) -> bool {
            match query {
                FilterQuery::MatchAll => true,
                FilterQuery::Text(text) => self.text.contains(text.as_str()),
                FilterQuery::Latency(_) => true,
            }
        }
    }

    fn row(id: &str, text: &str) -> Row {
        Row {
            id: id.to_owned(),
            text: text.to_owned(),
        }
    }

    #[test]
    fn the_projector_indexes_the_matching_rows() {
        let rows = vec![row("a", "alpha"), row("b", "kernel"), row("c", "gamma")];
        let mut projector = FilterProjector::new();
        projector.set_query(FilterQuery::Text("a".to_owned()));
        let refresh = projector.reset(&rows);
        assert_eq!(refresh.reevaluated, 3);
        assert_eq!(refresh.matched, 2);
        assert_eq!(projector.matched(), &[0, 2]);
    }

    #[test]
    fn a_refresh_that_changes_one_row_evaluates_one_row() {
        let rows = vec![row("a", "alpha"), row("b", "kernel"), row("c", "gamma")];
        let mut projector = FilterProjector::new();
        projector.set_query(FilterQuery::Text("a".to_owned()));
        projector.reset(&rows);

        let mut changed = rows.clone();
        changed[1].text = "alpha".to_owned();
        let refresh = projector.upsert(&changed, 1);
        assert_eq!(
            refresh.reevaluated, 1,
            "only the changed row may be evaluated again"
        );
        assert_eq!(refresh.matched, 3);
        assert_eq!(projector.matched(), &[0, 1, 2]);
    }

    #[test]
    fn a_new_query_invalidates_every_decision() {
        let rows = vec![row("a", "alpha"), row("b", "kernel")];
        let mut projector = FilterProjector::new();
        projector.set_query(FilterQuery::Text("a".to_owned()));
        projector.reset(&rows);
        assert_eq!(projector.matched(), &[0]);

        projector.set_query(FilterQuery::Text("k".to_owned()));
        let refresh = projector.reset(&rows);
        assert_eq!(refresh.reevaluated, 2);
        assert_eq!(projector.matched(), &[1]);
    }

    #[test]
    fn a_removed_row_leaves_the_index() {
        let rows = vec![row("a", "alpha"), row("b", "kernel"), row("c", "gamma")];
        let mut projector = FilterProjector::new();
        projector.set_query(FilterQuery::Text("a".to_owned()));
        projector.reset(&rows);
        assert_eq!(projector.matched(), &[0, 2]);

        projector.remove(0);
        assert_eq!(projector.matched(), &[1]);
        assert!(!projector.is_matched(0));
    }

    #[test]
    fn the_connection_order_uses_the_four_dimensions() {
        let mut rows = vec![
            ConnectionOrderRow {
                name: "b".to_owned(),
                host: "example.com".to_owned(),
                traffic_bytes: 10,
                rate_bytes_per_second: 1,
                started_at_ms: 30,
            },
            ConnectionOrderRow {
                name: "a".to_owned(),
                host: "example.com".to_owned(),
                traffic_bytes: 10,
                rate_bytes_per_second: 1,
                started_at_ms: 20,
            },
            ConnectionOrderRow {
                name: "c".to_owned(),
                host: "another.com".to_owned(),
                traffic_bytes: 99,
                rate_bytes_per_second: 1,
                started_at_ms: 10,
            },
        ];
        sort_connections(
            &mut rows,
            ConnectionSortDimension::Traffic,
            SortDirection::Descending,
        );
        assert_eq!(rows[0].name, "c");
        // The tie-break chain is traffic, rate, host, name.
        assert_eq!(rows[1].name, "a");
        assert_eq!(rows[2].name, "b");

        sort_connections(
            &mut rows,
            ConnectionSortDimension::Host,
            SortDirection::Ascending,
        );
        assert_eq!(rows[0].host, "another.com");
        assert_eq!(rows[1].host, "example.com");
        assert_eq!(rows[2].host, "example.com");

        sort_connections(
            &mut rows,
            ConnectionSortDimension::Started,
            SortDirection::Descending,
        );
        assert_eq!(rows[0].started_at_ms, 30);
        assert_eq!(rows[1].started_at_ms, 20);
        assert_eq!(rows[2].started_at_ms, 10);
    }
}
