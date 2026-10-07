//! Toolkit-neutral fields and observations of the subscription filter form.
use crate::profile_source::ProfileSourceIdentity;
use crate::subscription_import::SubscriptionFilterDraft;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum FilterField {
    #[default]
    Include,
    Exclude,
    Protocols,
    Renames,
    Advanced,
}
impl FilterField {
    pub const ALL: [Self; 5] = [
        Self::Include,
        Self::Exclude,
        Self::Protocols,
        Self::Renames,
        Self::Advanced,
    ];
    pub fn value(self, draft: &SubscriptionFilterDraft) -> &str {
        match self {
            Self::Include => &draft.include,
            Self::Exclude => &draft.exclude,
            Self::Protocols => &draft.exclude_types,
            Self::Renames => &draft.renames,
            Self::Advanced => draft.advanced_policy.as_deref().unwrap_or(""),
        }
    }
    pub fn set(self, draft: &mut SubscriptionFilterDraft, value: String) {
        match self {
            Self::Include => draft.include = value,
            Self::Exclude => draft.exclude = value,
            Self::Protocols => draft.exclude_types = value,
            Self::Renames => draft.renames = value,
            Self::Advanced => draft.advanced_policy = Some(value),
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FilterObservation {
    pub source: ProfileSourceIdentity,
    pub filter: SubscriptionFilterDraft,
}
