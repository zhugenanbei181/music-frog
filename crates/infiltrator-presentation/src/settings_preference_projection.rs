//! Preference observations preserve unknown and stale facts independently of runtime controls.
use infiltrator_contract::surface_snapshot::PageStatus;
use infiltrator_shared::locales::{Lang, Localizer};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PreferencePresentation {
    pub key: &'static str,
    pub params: Vec<(&'static str, String)>,
    pub editable: bool,
}

pub fn project(value: Option<bool>, status: &PageStatus, locale: &str) -> PreferencePresentation {
    let state_key = match value {
        Some(true) => "overview_toggle_enabled",
        Some(false) => "overview_toggle_disabled",
        None => "overview_toggle_unknown",
    };
    let state = Lang(locale).tr(state_key).into_owned();
    let (key, params) = match status {
        PageStatus::Failed { failure } | PageStatus::Unavailable { failure } => (
            "settings_preference_stale",
            vec![("state", state), ("reason", failure.message.clone())],
        ),
        PageStatus::Loading => ("settings_preference_loading", vec![("state", state)]),
        PageStatus::Ready | PageStatus::Empty => (state_key, Vec::new()),
    };
    PreferencePresentation {
        key,
        params,
        editable: value.is_some() && *status == PageStatus::Ready,
    }
}
