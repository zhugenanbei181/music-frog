//! Shared subscription status copy derived from observed metadata, never an editor draft.
use infiltrator_contract::error::Failure;
use infiltrator_contract::subscription_import::{SubscriptionFilterDedup, SubscriptionFilterDraft};
use infiltrator_domain::filter_policy_form::filter_spec_from_draft;
use infiltrator_domain::redact::redact_line;
use infiltrator_shared::i18n_interpolator::localize;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SubscriptionCopyProjection {
    pub key: &'static str,
    pub params: Vec<(&'static str, String)>,
}
impl SubscriptionCopyProjection {
    fn plain(key: &'static str) -> Self {
        Self {
            key,
            params: Vec::new(),
        }
    }
    pub fn render(&self, locale: &str) -> String {
        localize(locale, self.key, &self.params)
    }
    pub fn no_profile() -> Self {
        Self::plain("subscription_no_profile")
    }
}

pub fn conditional_request(
    etag: Option<&str>,
    last_modified: Option<&str>,
) -> SubscriptionCopyProjection {
    if etag.is_none() && last_modified.is_none() {
        return SubscriptionCopyProjection::plain("profiles_conditional_request_empty");
    }
    SubscriptionCopyProjection {
        key: "subscription_conditional_cached",
        params: vec![
            ("etag", etag.unwrap_or("—").into()),
            ("modified", last_modified.unwrap_or("—").into()),
        ],
    }
}
pub fn backup_status(available: Option<bool>) -> SubscriptionCopyProjection {
    SubscriptionCopyProjection::plain(match available {
        Some(true) => "profiles_backup_available",
        Some(false) => "profiles_backup_none",
        None => "subscription_no_profile",
    })
}
pub fn filter_status(filter: Option<&SubscriptionFilterDraft>) -> SubscriptionCopyProjection {
    let Some(filter) = filter else {
        return SubscriptionCopyProjection::plain("subscription_no_profile");
    };
    let spec = match filter_spec_from_draft(filter) {
        Ok(spec) => spec,
        Err(error) => {
            return SubscriptionCopyProjection {
                key: "subscription_filter_invalid_policy",
                params: vec![("reason", redact_line(&error.to_string(), &[]))],
            };
        }
    };
    if spec.is_empty() {
        return SubscriptionCopyProjection::plain("subscription_filter_inactive");
    }
    let Some(dedup) = SubscriptionFilterDedup::from_index(filter.dedup_index) else {
        return SubscriptionCopyProjection::plain("subscription_filter_invalid_mode");
    };
    let mut copy = SubscriptionCopyProjection {
        key: match (filter.renames.trim().is_empty(), dedup) {
            (true, SubscriptionFilterDedup::Disabled) => "subscription_filter_no_rename_no_dedup",
            (true, SubscriptionFilterDedup::KeepFirst) => {
                "subscription_filter_no_rename_keep_first"
            }
            (true, SubscriptionFilterDedup::KeepLast) => "subscription_filter_no_rename_keep_last",
            (true, SubscriptionFilterDedup::AppendIndex) => {
                "subscription_filter_no_rename_append_index"
            }
            (false, SubscriptionFilterDedup::Disabled) => "subscription_filter_rename_no_dedup",
            (false, SubscriptionFilterDedup::KeepFirst) => "subscription_filter_rename_keep_first",
            (false, SubscriptionFilterDedup::KeepLast) => "subscription_filter_rename_keep_last",
            (false, SubscriptionFilterDedup::AppendIndex) => {
                "subscription_filter_rename_append_index"
            }
        },
        params: vec![
            ("include", filter.include.clone()),
            ("exclude", filter.exclude.clone()),
            ("protocols", filter.exclude_types.clone()),
        ],
    };
    if let Some(policy) = filter
        .advanced_policy
        .as_deref()
        .filter(|policy| !policy.trim().is_empty() && policy.trim() != "{}")
    {
        copy.key = match copy.key {
            "subscription_filter_no_rename_no_dedup" => {
                "subscription_filter_no_rename_no_dedup_advanced"
            }
            "subscription_filter_no_rename_keep_first" => {
                "subscription_filter_no_rename_keep_first_advanced"
            }
            "subscription_filter_no_rename_keep_last" => {
                "subscription_filter_no_rename_keep_last_advanced"
            }
            "subscription_filter_no_rename_append_index" => {
                "subscription_filter_no_rename_append_index_advanced"
            }
            "subscription_filter_rename_no_dedup" => "subscription_filter_rename_no_dedup_advanced",
            "subscription_filter_rename_keep_first" => {
                "subscription_filter_rename_keep_first_advanced"
            }
            "subscription_filter_rename_keep_last" => {
                "subscription_filter_rename_keep_last_advanced"
            }
            "subscription_filter_rename_append_index" => {
                "subscription_filter_rename_append_index_advanced"
            }
            key => key,
        };
        copy.params.push(("policy", policy.into()));
    }
    copy
}

pub fn observed_filter_status(
    filter: Option<&SubscriptionFilterDraft>,
    failure: Option<&Failure>,
) -> SubscriptionCopyProjection {
    match failure {
        Some(failure) => SubscriptionCopyProjection {
            key: "filter_form_read_failed",
            params: vec![("reason", redact_line(&failure.message, &[]))],
        },
        None => filter_status(filter),
    }
}
pub fn schedule_status(
    enabled: Option<bool>,
    cron: Option<&str>,
    interval: Option<u32>,
) -> SubscriptionCopyProjection {
    let Some(enabled) = enabled else {
        return SubscriptionCopyProjection::plain("subscription_no_profile");
    };
    let cron = cron.filter(|cron| !cron.trim().is_empty());
    let (key, params) = if let Some(cron) = cron {
        (
            if enabled {
                "subscription_schedule_cron"
            } else {
                "subscription_schedule_paused_cron"
            },
            vec![("cron", cron.into())],
        )
    } else if let Some(hours) = interval {
        (
            if enabled {
                "subscription_schedule_interval"
            } else {
                "subscription_schedule_paused_interval"
            },
            vec![("hours", hours.to_string())],
        )
    } else {
        (
            if enabled {
                "subscription_schedule_unobserved"
            } else {
                "subscription_schedule_manual"
            },
            Vec::new(),
        )
    };
    SubscriptionCopyProjection { key, params }
}

pub fn reload_status(enabled: Option<bool>) -> SubscriptionCopyProjection {
    SubscriptionCopyProjection::plain(match enabled {
        Some(true) => "subscription_reload_enabled",
        Some(false) => "subscription_reload_disabled",
        None => "subscription_reload_unknown",
    })
}

#[cfg(test)]
#[path = "subscription_status_projection_tests.rs"]
mod tests;
