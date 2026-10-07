//! Neutral original text runs shared by native search presentations.
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchTextRun {
    pub text: String,
    pub highlighted: bool,
}
