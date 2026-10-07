//! Immutable search decisions and original text; no toolkit or UI identity.
use crate::search_text::SearchTextRun;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConnectionSearchRow {
    pub visible: bool,
    pub endpoint: Vec<SearchTextRun>,
    pub process_name: Vec<SearchTextRun>,
    pub process_rule: Vec<SearchTextRun>,
    /// Original matching observation, including metadata absent from the normal headline.
    pub matched_term: Vec<SearchTextRun>,
}
