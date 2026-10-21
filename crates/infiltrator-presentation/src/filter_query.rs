//! The one typed filter query the shared page models own.
//!
//! Every page that filters a list needs the same two answers: what did the
//! user type, and what does that text mean. Before this module the proxies
//! page kept one private shape and the rules page kept none at all, so each
//! surface held a private string filter and two surfaces could disagree
//! about what a query matches.
//!
//! This module owns the parse. The raw text becomes one typed value, and
//! every page renders from the parsed value instead of from a private
//! string.

/// The latency bound a filter query can carry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LatencyBound {
    /// Keep the rows that are faster than the bound.
    FasterThan(u32),
    /// Keep the rows that are slower than the bound.
    SlowerThan(u32),
}

/// The typed filter query the shared page models own.
///
/// The variants are exclusive: a query is either empty, free text, or a
/// latency bound. A page whose rows carry no latency never produces the
/// latency variant.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum FilterQuery {
    /// The query is empty: every row matches.
    #[default]
    MatchAll,
    /// The query is free text.
    Text(String),
    /// The query is a latency bound.
    Latency(LatencyBound),
}

impl FilterQuery {
    /// Parse the raw search text of a list whose rows carry a latency.
    ///
    /// A bound is only a bound when the remainder is a real millisecond
    /// count; `<abc` stays free text so a row named `<abc` is still
    /// reachable.
    pub fn parse(raw: &str) -> Self {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Self::MatchAll;
        }
        // The operator is one character, not one byte: a name or a query may
        // start with a character that is not ASCII.
        let mut characters = trimmed.chars();
        let operator = characters.next();
        let rest = characters.as_str();
        if matches!(operator, Some('<' | '>')) {
            if let Ok(milliseconds) = rest.trim().parse::<u32>() {
                return Self::Latency(if operator == Some('<') {
                    LatencyBound::FasterThan(milliseconds)
                } else {
                    LatencyBound::SlowerThan(milliseconds)
                });
            }
        }
        Self::text_query(trimmed)
    }

    /// Parse the raw search text of a list whose rows carry no latency.
    ///
    /// The latency bound does not apply to such a list, so the parse stays
    /// textual: a query that reads like a bound is free text here, never an
    /// empty result.
    pub fn parse_text(raw: &str) -> Self {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Self::MatchAll;
        }
        Self::text_query(trimmed)
    }

    fn text_query(trimmed: &str) -> Self {
        Self::Text(trimmed.to_ascii_lowercase())
    }

    /// The free-text form of the query.
    pub fn text(&self) -> &str {
        match self {
            Self::MatchAll => "",
            Self::Text(text) => text,
            Self::Latency(_) => "",
        }
    }

    /// The latency bound of the query, when it is one.
    pub fn latency_bound(&self) -> Option<LatencyBound> {
        match self {
            Self::Latency(bound) => Some(*bound),
            Self::MatchAll | Self::Text(_) => None,
        }
    }

    /// Whether the query keeps every row.
    pub fn matches_all(&self) -> bool {
        matches!(self, Self::MatchAll)
    }
}

#[cfg(test)]
#[path = "filter_query_tests.rs"]
mod tests;
