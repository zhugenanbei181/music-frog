//! Shared regex compilation, filtering, highlight spans and source availability.
use crate::log_follow::LogFollowState;
use crate::search_text::project_match_ranges;
use infiltrator_contract::logs::LogLevel;
use infiltrator_contract::search_text::SearchTextRun;
use infiltrator_contract::session::SessionToken;
use infiltrator_contract::surface_snapshot::{LogSnapshot, LogsPageSnapshot, PageData, PageStatus};
use regex::{Regex, RegexBuilder};

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
#[derive(Clone, Debug, Default)]
pub struct LogSearchState {
    pub follow: LogFollowState,
    query: String,
    compiled: Option<Regex>,
    invalid_pattern: Option<String>,
    scope: Option<(u64, Option<SessionToken>)>,
    page: Option<PageData<LogsPageSnapshot>>,
    rows: Vec<LogSearchRow>,
    matched_count: usize,
}
impl LogSearchState {
    pub fn observe(
        &mut self,
        generation: u64,
        token: Option<SessionToken>,
        page: &PageData<LogsPageSnapshot>,
    ) {
        if self.scope == Some((generation, token)) && self.page.as_ref() == Some(page) {
            return;
        }
        self.scope = Some((generation, token));
        self.page = Some(page.clone());
        self.rebuild();
    }
    pub fn edit_query(&mut self, query: &str) {
        if self.query == query {
            return;
        }
        self.query = query.into();
        self.invalid_pattern = None;
        self.compiled = if query.trim().is_empty() {
            None
        } else {
            match RegexBuilder::new(query)
                .case_insensitive(true)
                .size_limit(1024 * 1024)
                .build()
            {
                Ok(regex) => Some(regex),
                Err(error) => {
                    self.invalid_pattern = Some(error.to_string());
                    None
                }
            }
        };
        self.rebuild();
    }
    fn rebuild(&mut self) {
        self.rows = self
            .page
            .as_ref()
            .and_then(|page| page.data.as_ref())
            .into_iter()
            .flat_map(|data| &data.entries)
            .map(|entry| self.project_row(entry))
            .collect();
        self.matched_count = self.rows.iter().filter(|row| row.visible).count();
    }
    fn runs(&self, value: &str) -> Vec<SearchTextRun> {
        let ranges = self
            .compiled
            .as_ref()
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
        let visible = self.invalid_pattern.is_none()
            && self
                .compiled
                .as_ref()
                .is_none_or(|regex| values.iter().any(|value| regex.is_match(value)));
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
        &self.query
    }
    pub fn invalid_pattern(&self) -> Option<&str> {
        self.invalid_pattern.as_deref()
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
        self.page
            .as_ref()
            .and_then(|page| page.data.as_ref())
            .map_or(0, |data| data.total_entries)
    }
    pub fn source_current(&self) -> bool {
        self.page
            .as_ref()
            .is_some_and(|page| matches!(page.status, PageStatus::Ready | PageStatus::Empty))
    }
    pub fn status_key(&self) -> &'static str {
        if self.invalid_pattern.is_some() {
            "logs_search_invalid"
        } else if !self.source_current() {
            "logs_search_unavailable"
        } else if self.source_count() == 0 {
            "logs_no_realtime_records"
        } else if self.matched_count == 0 {
            "logs_search_no_matches"
        } else {
            "logs_search_results"
        }
    }
}
