//! One state machine for all filter fields, source changes and correlated command outcomes.
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::subscription_filter_form::{FilterField, FilterObservation};
use infiltrator_contract::subscription_filter_result::{FilterReport, SubscriptionFilterApplied};
use infiltrator_contract::subscription_import::{SubscriptionFilterDedup, SubscriptionFilterDraft};
use infiltrator_domain::filter_policy_form::filter_spec_from_draft;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PendingFilterChange {
    pub token: u64,
    pub source: FilterObservation,
    pub draft: SubscriptionFilterDraft,
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SubscriptionFilterEditor {
    pub draft: SubscriptionFilterDraft,
    base: Option<FilterObservation>,
    latest: Option<FilterObservation>,
    read_failure: Option<Failure>,
    pub failure: Option<Failure>,
    pub pending: Option<PendingFilterChange>,
    next_token: u64,
    target_profile: Option<String>,
    touched: bool,
    pub applied: bool,
    pub report: Option<FilterReport>,
}
impl SubscriptionFilterEditor {
    pub fn new(observation: Option<FilterObservation>) -> Self {
        let mut editor = Self::default();
        editor.observe(Ok(observation));
        editor
    }
    pub fn observe(&mut self, observation: Result<Option<FilterObservation>, Failure>) {
        match observation {
            Err(failure) => self.read_failure = Some(failure),
            Ok(latest) => {
                self.target_profile = latest.as_ref().map(|value| value.source.profile.clone());
                self.read_failure = None;
                if self.pending.is_some() {
                    self.latest = latest;
                    return;
                }
                if !self.dirty() {
                    if self.base.as_ref().map(|value| &value.source)
                        != latest.as_ref().map(|value| &value.source)
                    {
                        self.applied = false;
                        self.report = None;
                    }
                    self.base = latest.clone();
                    self.draft = latest
                        .as_ref()
                        .map(|source| source.filter.clone())
                        .unwrap_or_default();
                }
                self.latest = latest;
            }
        }
    }
    pub fn dirty(&self) -> bool {
        self.touched
            || self
                .base
                .as_ref()
                .is_some_and(|source| self.draft != source.filter)
    }
    pub fn observe_profile(
        &mut self,
        profile: &str,
        observation: Result<FilterObservation, Failure>,
    ) {
        self.observe(observation.and_then(|observed| {
            if observed.source.profile == profile {
                Ok(Some(observed))
            } else {
                Err(Failure::new(
                    ErrorCode::NotReady,
                    "Filter observation belongs to another profile",
                    true,
                ))
            }
        }));
    }
    pub fn bind_profile(&mut self, profile: Option<&str>) {
        if self.target_profile.as_deref() == profile {
            return;
        }
        self.target_profile = profile.map(str::to_owned);
        if self.base.as_ref().map(|base| base.source.profile.as_str()) != profile {
            self.read_failure = Some(Failure::new(
                ErrorCode::NotReady,
                "Waiting for the selected profile options",
                true,
            ));
        }
    }
    pub fn current(&self) -> bool {
        self.read_failure.is_none()
            && self
                .base
                .as_ref()
                .zip(self.latest.as_ref())
                .is_some_and(|(base, latest)| base.source == latest.source)
            && self.base.as_ref().is_some_and(|base| {
                Some(base.source.profile.as_str()) == self.target_profile.as_deref()
            })
    }
    pub fn can_edit(&self) -> bool {
        self.current() && self.pending.is_none()
    }
    pub fn stale(&self) -> bool {
        self.base.is_some() && !self.current()
    }
    pub fn selected(&self) -> Option<SubscriptionFilterDedup> {
        SubscriptionFilterDedup::from_index(self.draft.dedup_index)
    }
    pub fn value_for(&self, profile: &str) -> Option<SubscriptionFilterDedup> {
        (self.can_edit()
            && self
                .base
                .as_ref()
                .is_some_and(|base| base.source.profile == profile))
        .then(|| self.selected())
        .flatten()
    }
    pub fn edit(&mut self, field: FilterField, value: String) -> bool {
        if !self.can_edit() {
            return false;
        }
        if field.value(&self.draft) != value {
            field.set(&mut self.draft, value);
            self.touched = true;
            self.failure = None;
            self.applied = false;
        }
        true
    }
    pub fn pick(&mut self, mode: SubscriptionFilterDedup) -> bool {
        if !self.can_edit() {
            return false;
        }
        if self.draft.dedup_index != mode.index() {
            self.draft.dedup_index = mode.index();
            self.touched = true;
            self.failure = None;
            self.applied = false;
        }
        true
    }
    pub fn cancel(&mut self) -> bool {
        if self.pending.is_some() {
            return false;
        }
        self.base = self.latest.clone();
        self.draft = self
            .base
            .as_ref()
            .map(|source| source.filter.clone())
            .unwrap_or_default();
        self.failure = None;
        self.touched = false;
        self.applied = false;
        self.report = None;
        true
    }
    pub fn source_profile(&self) -> Option<&str> {
        self.base.as_ref().map(|base| base.source.profile.as_str())
    }
    pub fn composing(&mut self) {
        if self.can_edit() {
            self.touched = true;
        }
    }
    pub fn latest_profile(&self) -> Option<&str> {
        self.latest
            .as_ref()
            .map(|latest| latest.source.profile.as_str())
    }
    pub fn read_failure(&self) -> Option<&Failure> {
        self.read_failure.as_ref()
    }
    pub fn begin(&mut self) -> Result<PendingFilterChange, Failure> {
        let failure = if !self.can_edit() {
            Some(Failure::new(
                ErrorCode::NotReady,
                "The filter form has no current source or a write is pending",
                true,
            ))
        } else {
            filter_spec_from_draft(&self.draft)
                .err()
                .map(|error| Failure::new(ErrorCode::InvalidInput, error.to_string(), false))
        };
        if let Some(failure) = failure {
            self.failure = Some(failure.clone());
            return Err(failure);
        }
        self.next_token += 1;
        let pending = PendingFilterChange {
            token: self.next_token,
            source: self.base.clone().expect("validated source"),
            draft: self.draft.clone(),
        };
        self.pending = Some(pending.clone());
        self.failure = None;
        self.applied = false;
        Ok(pending)
    }
    pub fn finish(
        &mut self,
        token: u64,
        result: Result<SubscriptionFilterApplied, Failure>,
    ) -> bool {
        let Some(pending) = self
            .pending
            .as_ref()
            .filter(|pending| pending.token == token)
            .cloned()
        else {
            return false;
        };
        self.pending = None;
        match result {
            Ok(applied) if applied.source.profile == pending.source.source.profile => {
                let observed = FilterObservation {
                    source: applied.source,
                    filter: pending.draft,
                };
                self.base = Some(observed.clone());
                if self.latest.as_ref() == Some(&pending.source) {
                    self.latest = Some(observed);
                }
                self.failure = None;
                self.touched = false;
                self.applied = true;
                self.report = Some(applied.report);
            }
            Ok(_) => {
                self.failure = Some(Failure::new(
                    ErrorCode::InvalidState,
                    "Filter receipt belongs to another profile",
                    false,
                ))
            }
            Err(failure) => self.failure = Some(failure),
        }
        true
    }
}

#[cfg(test)]
#[path = "subscription_filter_editor_tests.rs"]
mod tests;
