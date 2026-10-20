//! Preference observations preserve unknown and stale facts independently of runtime controls.
use crate::copy::TextCatalog;
use lurker_core_contract::page::PageStatus;

/// The presentation of one preference observation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PreferencePresentation {
    /// The catalog key the surface renders.
    pub key: &'static str,
    /// The interpolation parameters of the catalog key.
    pub params: Vec<(&'static str, String)>,
    /// Whether the surface may change the preference right now.
    pub editable: bool,
}

/// Projects one preference observation into its presentation.
///
/// An unknown value, a stale read and a failed read stay distinguishable: the
/// projection reports the state text and the reason separately, and only a
/// ready observation of a known value is editable.
pub fn project(
    value: Option<bool>,
    status: &PageStatus,
    catalog: &dyn TextCatalog,
) -> PreferencePresentation {
    let state_key = match value {
        Some(true) => "overview_toggle_enabled",
        Some(false) => "overview_toggle_disabled",
        None => "overview_toggle_unknown",
    };
    let state = catalog.text(state_key).into_owned();
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
