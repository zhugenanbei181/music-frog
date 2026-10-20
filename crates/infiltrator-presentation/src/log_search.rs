//! Shared regex compilation, filtering, highlight spans and source availability.
use infiltrator_contract::logs::LogLevel;
use infiltrator_contract::surface_snapshot::{LogSnapshot, LogsPageSnapshot};
use lurker_core_contract::page::{PageData, PageStatus};
use lurker_core_contract::session::SessionToken;
use lurker_ui_presentation::log_follow::LogFollowState;
use regex::{Regex, RegexBuilder};
use zhuge_ui_core::text::SearchTextRun;
use zhuge_ui_core::text::project_match_ranges;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LogSearchRow {
    pub id: u64,
    pub visible: bool,
    pub level: LogLevel,
    pub timestamp: Vec<SearchTextRun>,
    pub level_label: Vec<SearchTextRun>,
    pub tag: Vec<SearchTextRun>,
    pub message: Vec<SearchTextRun>,
}

/// The typed outcome of the logs search fold.
///
/// Every surface renders the search state from this one enum, so "the pattern
/// is broken" can never look like "nothing matched" and "the page is gone" can
/// never look like an empty result.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LogSearchOutcome {
    /// The query is not a usable regular expression.
    InvalidPattern {
        /// Why the pattern did not compile.
        reason: String,
    },
    /// The observed page is not currently readable.
    SourceUnavailable,
    /// The owner reported no records at all.
    NoRecords,
    /// The query is valid but no record matches it.
    NoMatches {
        /// Rows that matched.
        matched: usize,
        /// Rows the owner reported.
        source: usize,
    },
    /// The query matched at least one record.
    Results {
        /// Rows that matched.
        matched: usize,
        /// Rows the owner reported.
        source: usize,
    },
}

#[derive(Clone, Debug, Default)]
pub struct LogSearchState {
    pub follow: LogFollowState,
    query: LogQueryState,
    source: LogSourceObservation,
    rows: Vec<LogSearchRow>,
    matched_count: usize,
}

#[derive(Clone, Debug)]
enum LogQueryState {
    Empty { source: String },
    Valid { source: String, regex: Regex },
    Invalid { source: String, error: String },
}

impl Default for LogQueryState {
    fn default() -> Self {
        Self::Empty {
            source: String::new(),
        }
    }
}

#[derive(Clone, Debug, Default)]
enum LogSourceObservation {
    #[default]
    Unobserved,
    Observed {
        generation: u64,
        token: Option<SessionToken>,
        page: PageData<LogsPageSnapshot>,
    },
}

impl LogSearchState {
    pub fn observe(
        &mut self,
        generation: u64,
        token: Option<SessionToken>,
        page: &PageData<LogsPageSnapshot>,
    ) {
        if matches!(
            &self.source,
            LogSourceObservation::Observed {
                generation: previous_generation,
                token: previous_token,
                page: previous_page,
            } if *previous_generation == generation
                && previous_token == &token
                && previous_page == page
        ) {
            return;
        }
        self.source = LogSourceObservation::Observed {
            generation,
            token,
            page: page.clone(),
        };
        self.rebuild();
    }

    pub fn edit_query(&mut self, query: &str) {
        if self.query.source() == query {
            return;
        }
        self.query = if query.trim().is_empty() {
            LogQueryState::Empty {
                source: query.into(),
            }
        } else {
            match RegexBuilder::new(query)
                .case_insensitive(true)
                .size_limit(1024 * 1024)
                .build()
            {
                Ok(regex) => LogQueryState::Valid {
                    source: query.into(),
                    regex,
                },
                Err(error) => LogQueryState::Invalid {
                    source: query.into(),
                    error: error.to_string(),
                },
            }
        };
        self.rebuild();
    }

    fn rebuild(&mut self) {
        let entries = match &self.source {
            LogSourceObservation::Unobserved => &[][..],
            LogSourceObservation::Observed { page, .. } => page
                .data
                .as_ref()
                .map_or(&[][..], |data| data.entries.as_slice()),
        };
        self.rows = entries
            .iter()
            .map(|entry| self.project_row(entry))
            .collect();
        self.matched_count = self.rows.iter().filter(|row| row.visible).count();
    }

    fn runs(&self, value: &str) -> Vec<SearchTextRun> {
        let ranges = self
            .query
            .regex()
            .into_iter()
            .flat_map(|regex| regex.find_iter(value))
            .filter(|matched| !matched.is_empty())
            .map(|matched| (matched.start(), matched.end()));
        project_match_ranges(value, ranges)
    }

    fn project_row(&self, entry: &LogSnapshot) -> LogSearchRow {
        let level = LogLevel::from_identifier(&entry.level);
        let label = format!("[{}]", level.label());
        let tag = if entry.tag.is_empty() {
            String::new()
        } else {
            format!("[{}]", entry.tag)
        };
        let values = [&entry.timestamp, &label, &tag, &entry.message];
        let visible = match &self.query {
            LogQueryState::Empty { .. } => true,
            LogQueryState::Valid { regex, .. } => values.iter().any(|value| regex.is_match(value)),
            LogQueryState::Invalid { .. } => false,
        };
        LogSearchRow {
            id: entry.id,
            visible,
            level,
            timestamp: self.runs(&entry.timestamp),
            level_label: self.runs(&label),
            tag: self.runs(&tag),
            message: self.runs(&entry.message),
        }
    }

    pub fn query(&self) -> &str {
        self.query.source()
    }

    pub fn invalid_pattern(&self) -> Option<&str> {
        self.query.invalid_error()
    }

    pub fn rows(&self) -> &[LogSearchRow] {
        &self.rows
    }

    pub fn row(&self, id: u64) -> Option<&LogSearchRow> {
        self.rows.iter().find(|row| row.id == id)
    }

    pub fn matched_count(&self) -> usize {
        self.matched_count
    }

    pub fn source_count(&self) -> usize {
        match &self.source {
            LogSourceObservation::Unobserved => 0,
            LogSourceObservation::Observed { page, .. } => {
                page.data.as_ref().map_or(0, |data| data.total_entries)
            }
        }
    }

    pub fn source_current(&self) -> bool {
        matches!(
            &self.source,
            LogSourceObservation::Observed { page, .. }
                if matches!(page.status, PageStatus::Ready | PageStatus::Empty)
        )
    }

    pub fn status_key(&self) -> &'static str {
        match self.outcome() {
            LogSearchOutcome::InvalidPattern { .. } => "logs_search_invalid",
            LogSearchOutcome::SourceUnavailable => "logs_search_unavailable",
            LogSearchOutcome::NoRecords => "logs_no_realtime_records",
            LogSearchOutcome::NoMatches { .. } => "logs_search_no_matches",
            LogSearchOutcome::Results { .. } => "logs_search_results",
        }
    }

    /// The typed outcome of the search fold.
    pub fn outcome(&self) -> LogSearchOutcome {
        if let Some(reason) = self.query.invalid_error() {
            return LogSearchOutcome::InvalidPattern {
                reason: reason.to_owned(),
            };
        }
        if !self.source_current() {
            return LogSearchOutcome::SourceUnavailable;
        }
        if self.source_count() == 0 {
            return LogSearchOutcome::NoRecords;
        }
        if self.matched_count == 0 {
            return LogSearchOutcome::NoMatches {
                matched: 0,
                source: self.source_count(),
            };
        }
        LogSearchOutcome::Results {
            matched: self.matched_count,
            source: self.source_count(),
        }
    }
}

impl LogQueryState {
    fn source(&self) -> &str {
        match self {
            Self::Empty { source } | Self::Valid { source, .. } | Self::Invalid { source, .. } => {
                source
            }
        }
    }

    fn regex(&self) -> Option<&Regex> {
        match self {
            Self::Valid { regex, .. } => Some(regex),
            Self::Empty { .. } | Self::Invalid { .. } => None,
        }
    }

    fn invalid_error(&self) -> Option<&str> {
        match self {
            Self::Invalid { error, .. } => Some(error),
            Self::Empty { .. } | Self::Valid { .. } => None,
        }
    }
}
