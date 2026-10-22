//! One semantic fold for live rule rows and unsaved editor rows on either UI.
//!
//! A rule row carries two facts of its own: the local trace's **count** and
//! the row's **status** (enabled, disabled, shadowed). They are two typed
//! facts ([`RuleRowHits`] and [`RuleRowStatus`]) so a surface can draw them
//! as two chips; the older single-sentence copy stays for the surfaces that
//! still read both facts from one string.
use infiltrator_domain::rules::types::{RuleType, parse_rule_str};
use infiltrator_domain::sub_rules::format_ast;
use lurker_core_contract::error::{ErrorCode, Failure};
use lurker_ui_presentation::copy::TextCatalog;
use lurker_ui_presentation::copy::interpolate;

pub struct RuleRowFacts {
    pub rule_type: String,
    pub payload: String,
    pub target: String,
    pub no_resolve: bool,
    pub source_ip: bool,
    pub failure: Option<Failure>,
}
pub fn rule_row(raw: &str) -> RuleRowFacts {
    match parse_rule_str(raw) {
        Ok(parsed) => RuleRowFacts {
            rule_type: parsed.rule_type.name().to_owned(),
            payload: match &parsed.rule_type {
                RuleType::Logical(rule) => format_ast(&rule.payload),
                kind => kind.payload().unwrap_or_default().to_owned(),
            },
            target: parsed.target,
            no_resolve: parsed.no_resolve,
            source_ip: parsed.source_ip,
            failure: None,
        },
        Err(error) => RuleRowFacts {
            rule_type: raw.split(',').next().unwrap_or_default().trim().to_owned(),
            payload: raw.to_owned(),
            target: String::new(),
            no_resolve: false,
            source_ip: false,
            failure: Some(Failure::new(
                ErrorCode::Configuration,
                error.to_string(),
                false,
            )),
        },
    }
}
pub fn row_detail(
    source_ip: bool,
    no_resolve: bool,
    failure: Option<&Failure>,
    catalog: &dyn TextCatalog,
) -> String {
    let mut details = Vec::new();
    if source_ip {
        details.push(catalog.text("rule_param_source_ip").to_string());
    }
    if no_resolve {
        details.push(catalog.text("rule_param_no_resolve").to_string());
    }
    if let Some(failure) = failure {
        details.push(interpolate(
            catalog.text("rule_row_invalid").as_ref(),
            &[("reason", &failure.message)],
        ));
    }
    details.join(" · ")
}
pub fn row_hits_key(count: Option<u64>, enabled: bool, shadowed: bool) -> &'static str {
    if count.is_none() && !enabled {
        "rule_local_hits_unobserved_disabled"
    } else if count.is_none() && shadowed {
        "rule_local_hits_unobserved_shadowed"
    } else if count.is_none() {
        "shell_readout_unknown"
    } else if !enabled {
        "rule_local_hits_disabled"
    } else if shadowed {
        "rule_local_hits_shadowed"
    } else {
        "rule_local_hits"
    }
}
pub fn row_hits_copy(
    count: Option<u64>,
    enabled: bool,
    shadowed: bool,
    catalog: &dyn TextCatalog,
) -> String {
    interpolate(
        catalog
            .text(row_hits_key(count, enabled, shadowed))
            .as_ref(),
        &[(
            "count",
            &count.map(|value| value.to_string()).unwrap_or_default(),
        )],
    )
}

/// The local trace's count fact for one rule row.
///
/// The count is one fact of its own: a surface draws it as the row's count
/// chip, beside the row's status chip, instead of reading both out of one
/// mixed sentence.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RuleRowHits {
    /// The local trace never observed this row.
    Unobserved,
    /// The local trace observed this row the given number of times.
    Observed { count: u64 },
}

impl RuleRowHits {
    /// The locale key of the count chip.
    ///
    /// The key is the shared fact, so a surface that renders the chip live
    /// (the Bevy row restamps its text) reads the same key the shared copy
    /// uses instead of naming one of its own.
    pub const fn key(self) -> &'static str {
        match self {
            Self::Unobserved => "rule_hits_unobserved",
            Self::Observed { .. } => "rule_hits_count",
        }
    }

    /// The count the chip's copy interpolates, when it has one.
    pub const fn count(self) -> Option<u64> {
        match self {
            Self::Unobserved => None,
            Self::Observed { count } => Some(count),
        }
    }
}

/// The status fact of one rule row: whether the row still decides traffic,
/// and whether a later rule shadows it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RuleRowStatus {
    /// The row is enabled and nothing shadows it: the plain case.
    Enabled,
    /// The row is disabled and decides nothing.
    Disabled,
    /// A later rule shadows this row.
    Shadowed,
}

impl RuleRowStatus {
    /// The locale key of the status chip.
    ///
    /// A plain row carries no status fact of its own, so it has no status
    /// chip: the key is `None` and the surface draws no chip.
    pub const fn key(self) -> Option<&'static str> {
        match self {
            Self::Enabled => None,
            Self::Disabled => Some("rule_draft_disabled"),
            Self::Shadowed => Some("rule_status_shadowed"),
        }
    }
}

/// The two facts one rule row's chips carry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RuleRowChips {
    pub hits: RuleRowHits,
    pub status: RuleRowStatus,
}

impl RuleRowChips {
    /// Split one row's facts into its count chip and its status chip.
    pub fn of(count: Option<u64>, enabled: bool, shadowed: bool) -> Self {
        Self {
            hits: match count {
                Some(count) => RuleRowHits::Observed { count },
                None => RuleRowHits::Unobserved,
            },
            status: if !enabled {
                RuleRowStatus::Disabled
            } else if shadowed {
                RuleRowStatus::Shadowed
            } else {
                RuleRowStatus::Enabled
            },
        }
    }
}

/// The count chip's copy: the observed count or the plain "not observed".
pub fn row_hits_chip(hits: RuleRowHits, catalog: &dyn TextCatalog) -> String {
    match hits.count() {
        None => catalog.text(hits.key()).to_string(),
        Some(count) => {
            let count = count.to_string();
            interpolate(
                catalog.text(hits.key()).as_ref(),
                &[("count", count.as_str())],
            )
        }
    }
}

/// The status chip's copy.
///
/// A plain row carries no status fact of its own, so it has no status chip:
/// the surface draws the chip only when there is something to say.
pub fn row_status_chip(status: RuleRowStatus, catalog: &dyn TextCatalog) -> Option<String> {
    status.key().map(|key| catalog.text(key).to_string())
}

#[cfg(test)]
#[path = "rule_row_projection_test.rs"]
mod tests;
