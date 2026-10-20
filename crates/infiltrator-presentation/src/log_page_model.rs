//! The one typed page model of the logs page, shared by every surface.
//!
//! The logs page has exactly one state derivation: from the accepted contract
//! page observation to what the page shows. This module owns that derivation —
//! the page availability, the stream phase, the level filter and the search
//! fold — so a surface renders the model and dispatches typed intents back
//! instead of re-deriving the same state from the snapshot a second time.
//!
//! The raw observation stays with the owner that produced it; everything the
//! page *renders* about it is derived here, once.

use crate::log_search::{LogSearchOutcome, LogSearchRow, LogSearchState};
use infiltrator_contract::logs::{LogLevel, LogStreamState};
use infiltrator_contract::surface_snapshot::LogsPageSnapshot;
use lurker_core_contract::error::Failure;
use lurker_core_contract::page::{PageData, PageStatus};
use lurker_core_contract::session::SessionToken;
use lurker_ui_presentation::log_follow::LogFollowState;

/// The typed availability of the logs page.
///
/// A surface never infers availability from an empty entry list: the owner
/// states it, and this enum keeps the reason attached to the state.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum LogPageStatus {
    /// No page observation has arrived yet.
    #[default]
    Unobserved,
    /// The owner has not produced the first answer yet.
    Loading,
    /// The owner produced a readable page.
    Ready,
    /// The owner produced a readable but empty page.
    Empty,
    /// The host does not offer the logs page; the failure explains why.
    Unavailable {
        /// Typed reason the page is absent.
        failure: Failure,
    },
    /// The owner failed while producing the page.
    Failed {
        /// Typed reason the read failed.
        failure: Failure,
    },
}

impl LogPageStatus {
    /// Fold the contract page status into the typed page availability.
    pub fn from_page_status(status: &PageStatus) -> Self {
        match status {
            PageStatus::Loading => Self::Loading,
            PageStatus::Ready => Self::Ready,
            PageStatus::Empty => Self::Empty,
            PageStatus::Unavailable { failure } => Self::Unavailable {
                failure: failure.clone(),
            },
            PageStatus::Failed { failure } => Self::Failed {
                failure: failure.clone(),
            },
        }
    }

    /// Whether the page currently carries a readable observation.
    pub fn is_readable(&self) -> bool {
        matches!(self, Self::Ready | Self::Empty)
    }

    /// The failure that explains why the page is not readable, if any.
    pub fn failure(&self) -> Option<&Failure> {
        match self {
            Self::Unavailable { failure } | Self::Failed { failure } => Some(failure),
            Self::Unobserved | Self::Loading | Self::Ready | Self::Empty => None,
        }
    }
}

/// The typed level filter of the logs page.
///
/// A filter is either "every level" or one concrete level. There is no third
/// state, so a surface can no longer hold a level string that names no level.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LogLevelFilter {
    /// Every level is shown.
    #[default]
    All,
    /// Only records of this level are shown.
    Level(LogLevel),
}

impl LogLevelFilter {
    /// Parse the owner's active-level identifier.
    ///
    /// An identifier that names no level is not a filter: it never degrades
    /// into an "unknown" level, because `unknown` is a real record level and
    /// must not stand in for a filter nobody asked for.
    pub fn from_identifier(value: &str) -> Self {
        match value.trim().to_ascii_lowercase().as_str() {
            "debug" | "dbg" => Self::Level(LogLevel::Debug),
            "info" | "inf" => Self::Level(LogLevel::Info),
            "warn" | "warning" | "wrn" => Self::Level(LogLevel::Warn),
            "error" | "err" | "fatal" => Self::Level(LogLevel::Error),
            "unknown" => Self::Level(LogLevel::Unknown),
            _ => Self::All,
        }
    }

    /// The typed filter the owner's reported active level expresses.
    pub fn from_active_level(active: Option<&str>) -> Self {
        match active {
            Some(value) => Self::from_identifier(value),
            None => Self::All,
        }
    }

    /// The concrete level this filter selects, if it selects one.
    pub fn level(self) -> Option<LogLevel> {
        match self {
            Self::All => None,
            Self::Level(level) => Some(level),
        }
    }

    /// The owner-facing identifier of this filter.
    pub fn identifier(self) -> Option<String> {
        self.level().map(|level| level.label().to_string())
    }
}

/// The level filter of the logs page and the authority it comes from.
///
/// The owner is authoritative for the filter; a surface may select one before
/// the owner has confirmed it. Keeping both in one typed enum means the page
/// can never show a filter nobody chose and never lose the authority that
/// produced it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LogLevelSelection {
    /// Every level is shown; no level filter is applied.
    #[default]
    All,
    /// The owner confirmed this level filter.
    Confirmed(LogLevel),
    /// The surface selected this level filter; the owner has not confirmed it.
    Selected(LogLevel),
}

impl LogLevelSelection {
    /// The level the page filters by, if it filters by one.
    pub fn level(self) -> Option<LogLevel> {
        match self {
            Self::All => None,
            Self::Confirmed(level) | Self::Selected(level) => Some(level),
        }
    }

    /// The typed filter this selection expresses.
    pub fn filter(self) -> LogLevelFilter {
        match self.level() {
            Some(level) => LogLevelFilter::Level(level),
            None => LogLevelFilter::All,
        }
    }

    /// Fold the owner's level-filter readback into the selection.
    pub fn observe_owner(self, filter: LogLevelFilter) -> Self {
        match filter {
            LogLevelFilter::All => Self::All,
            LogLevelFilter::Level(level) => Self::Confirmed(level),
        }
    }

    /// Fold the surface's own level selection into the selection.
    pub fn select(self, filter: LogLevelFilter) -> Self {
        match filter {
            LogLevelFilter::All => Self::All,
            LogLevelFilter::Level(level) => Self::Selected(level),
        }
    }
}

/// The observed state of the logs page.
#[derive(Clone, Debug, Default)]
enum LogPageObservation {
    #[default]
    Unobserved,
    Observed {
        generation: u64,
        token: Option<SessionToken>,
        status: LogPageStatus,
        stream: LogStreamState,
        total_entries: usize,
    },
}

/// The one page model of the logs page.
///
/// A surface renders the logs page from this model alone: the availability,
/// the stream phase, the entry count, the level filter and every search row
/// come from here, and the surface only dispatches typed intents back.
#[derive(Clone, Debug, Default)]
pub struct LogsPageModel {
    search: LogSearchState,
    page: LogPageObservation,
    level: LogLevelSelection,
}

impl LogsPageModel {
    /// Fold one accepted contract page observation into the model.
    ///
    /// The level filter is *not* touched here: it is a separate owner
    /// readback, so a surface that observes log records without an owner (the
    /// standalone log stream) never silently drops the level the user chose.
    pub fn observe(
        &mut self,
        generation: u64,
        token: Option<SessionToken>,
        page: &PageData<LogsPageSnapshot>,
    ) {
        self.search.observe(generation, token, page);
        self.page = LogPageObservation::Observed {
            generation,
            token,
            status: LogPageStatus::from_page_status(&page.status),
            stream: page
                .data
                .as_ref()
                .map(|data| data.stream.clone())
                .unwrap_or_default(),
            total_entries: page
                .data
                .as_ref()
                .map(|data| data.total_entries)
                .unwrap_or_default(),
        };
    }

    /// Fold the owner's level-filter readback into the model.
    pub fn observe_level(&mut self, filter: LogLevelFilter) {
        self.level = self.level.observe_owner(filter);
    }

    /// Select the level filter the page applies.
    pub fn select_level(&mut self, filter: LogLevelFilter) {
        self.level = self.level.select(filter);
    }

    /// Edit the search query the page applies.
    pub fn edit_query(&mut self, query: &str) {
        self.search.edit_query(query);
    }

    /// The typed availability of the page.
    pub fn status(&self) -> LogPageStatus {
        match &self.page {
            LogPageObservation::Unobserved => LogPageStatus::Unobserved,
            LogPageObservation::Observed { status, .. } => status.clone(),
        }
    }

    /// The stream phase the owner reported.
    pub fn stream(&self) -> LogStreamState {
        match &self.page {
            LogPageObservation::Unobserved => LogStreamState::default(),
            LogPageObservation::Observed { stream, .. } => stream.clone(),
        }
    }

    /// The number of records the owner reported before any level filter.
    pub fn total_entries(&self) -> usize {
        match &self.page {
            LogPageObservation::Unobserved => 0,
            LogPageObservation::Observed { total_entries, .. } => *total_entries,
        }
    }

    /// The core generation the observed page belongs to.
    pub fn generation(&self) -> Option<u64> {
        match &self.page {
            LogPageObservation::Unobserved => None,
            LogPageObservation::Observed { generation, .. } => Some(*generation),
        }
    }

    /// The session token the observed page belongs to.
    pub fn session_token(&self) -> Option<SessionToken> {
        match &self.page {
            LogPageObservation::Unobserved => None,
            LogPageObservation::Observed { token, .. } => token.clone(),
        }
    }

    /// The level filter of the page and the authority it comes from.
    pub fn level_selection(&self) -> LogLevelSelection {
        self.level
    }

    /// The typed level filter of the page.
    pub fn level_filter(&self) -> LogLevelFilter {
        self.level.filter()
    }

    /// The concrete level the page filters by, if it filters by one.
    pub fn active_level(&self) -> Option<LogLevel> {
        self.level.level()
    }

    /// The shared search fold of the page.
    pub fn search(&self) -> &LogSearchState {
        &self.search
    }

    /// The search rows of the page, in observation order.
    pub fn rows(&self) -> &[LogSearchRow] {
        self.search.rows()
    }

    /// One search row of the page.
    pub fn row(&self, id: u64) -> Option<&LogSearchRow> {
        self.search.row(id)
    }

    /// The current search query.
    pub fn query(&self) -> &str {
        self.search.query()
    }

    /// The reason the current query is not a usable pattern.
    pub fn invalid_pattern(&self) -> Option<&str> {
        self.search.invalid_pattern()
    }

    /// The number of rows that survive the current query.
    pub fn matched_count(&self) -> usize {
        self.search.matched_count()
    }

    /// The number of rows the owner reported.
    pub fn source_count(&self) -> usize {
        self.search.source_count()
    }

    /// Whether the observed page is currently readable.
    pub fn source_current(&self) -> bool {
        self.search.source_current()
    }

    /// The typed outcome of the search fold.
    pub fn search_outcome(&self) -> LogSearchOutcome {
        self.search.outcome()
    }

    /// The catalog key of the search outcome.
    pub fn status_key(&self) -> &'static str {
        self.search.status_key()
    }

    /// The viewport follow state of the page.
    pub fn follow(&self) -> &LogFollowState {
        &self.search.follow
    }

    /// The viewport follow state of the page, for the fold that changes it.
    pub fn follow_mut(&mut self) -> &mut LogFollowState {
        &mut self.search.follow
    }
}

#[cfg(test)]
#[path = "log_page_model_tests.rs"]
mod tests;
