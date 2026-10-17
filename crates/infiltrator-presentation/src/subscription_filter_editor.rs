//! Shared subscription filter draft and correlated command state.
use infiltrator_contract::subscription_filter_form::{FilterField, FilterObservation};
use infiltrator_contract::subscription_filter_result::{FilterReport, SubscriptionFilterApplied};
use infiltrator_contract::subscription_import::{SubscriptionFilterDedup, SubscriptionFilterDraft};
use infiltrator_domain::filter_policy_form::filter_spec_from_draft;
use lurker_core_contract::error::{ErrorCode, Failure};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PendingFilterChange {
    pub token: u64,
    pub source: FilterObservation,
    pub draft: SubscriptionFilterDraft,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SubscriptionFilterEditor {
    pub draft: SubscriptionFilterDraft,
    source: FilterSourceState,
    read: FilterReadState,
    draft_state: FilterDraftState,
    action: FilterActionState,
    target: FilterTarget,
    next_token: u64,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
enum FilterSourceState {
    #[default]
    Unobserved,
    Observed {
        baseline: FilterObservation,
        latest: FilterObservation,
    },
    MissingLatest {
        baseline: FilterObservation,
    },
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
enum FilterReadState {
    #[default]
    Unobserved,
    Ready,
    Waiting(Failure),
    Failed(Failure),
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum FilterDraftState {
    #[default]
    Pristine,
    Modified,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
enum FilterActionState {
    #[default]
    Ready,
    Pending(PendingFilterChange),
    Rejected(Failure),
    Failed(Failure),
    Applied(FilterReport),
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
enum FilterTarget {
    #[default]
    Unselected,
    Selected(String),
}

impl SubscriptionFilterEditor {
    pub fn new(observation: Option<FilterObservation>) -> Self {
        let mut editor = Self::default();
        editor.observe(Ok(observation));
        editor
    }

    pub fn observe(&mut self, observation: Result<Option<FilterObservation>, Failure>) {
        let latest = match observation {
            Err(failure) => {
                self.read = FilterReadState::Failed(failure);
                return;
            }
            Ok(latest) => latest,
        };
        self.target = target_from(latest.as_ref());
        self.read = match latest.as_ref() {
            Some(_) => FilterReadState::Ready,
            None => FilterReadState::Unobserved,
        };

        if matches!(&self.action, FilterActionState::Pending(_)) {
            self.source = self.source.with_latest(latest);
            return;
        }

        if self.dirty() {
            self.source = self.source.with_latest(latest);
            return;
        }

        let previous_identity = self
            .source
            .baseline()
            .map(|value| &value.workspace_identity);
        let latest_identity = latest.as_ref().map(|value| &value.workspace_identity);
        if previous_identity != latest_identity
            && matches!(&self.action, FilterActionState::Applied(_))
        {
            self.action = FilterActionState::Ready;
        }
        self.draft = latest
            .as_ref()
            .map(|source| source.filter.clone())
            .unwrap_or_default();
        self.source = FilterSourceState::at(latest);
        self.draft_state = FilterDraftState::Pristine;
    }

    pub fn dirty(&self) -> bool {
        matches!(self.draft_state, FilterDraftState::Modified)
            || self
                .source
                .baseline()
                .is_some_and(|source| self.draft != source.filter)
    }

    pub fn observe_profile(
        &mut self,
        profile: &str,
        observation: Result<FilterObservation, Failure>,
    ) {
        self.observe(observation.and_then(|observed| {
            if observed.workspace_identity.document.profile_id.as_str() == profile {
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
        if self.target.profile() == profile {
            return;
        }
        self.target = profile.map_or(FilterTarget::Unselected, |profile| {
            FilterTarget::Selected(profile.to_owned())
        });
        if self
            .source
            .baseline()
            .map(|base| base.workspace_identity.document.profile_id.as_str())
            != profile
        {
            self.read = FilterReadState::Waiting(Failure::new(
                ErrorCode::NotReady,
                "Waiting for the selected profile options",
                true,
            ));
        }
    }

    pub fn current(&self) -> bool {
        matches!(&self.read, FilterReadState::Ready)
            && matches!(
                &self.source,
                FilterSourceState::Observed { baseline, latest }
                    if baseline.workspace_identity == latest.workspace_identity
                        && self.target.profile()
                            == Some(baseline.workspace_identity.document.profile_id.as_str())
            )
    }

    pub fn can_edit(&self) -> bool {
        self.current() && !matches!(&self.action, FilterActionState::Pending(_))
    }

    pub fn stale(&self) -> bool {
        self.source.baseline().is_some() && !self.current()
    }

    pub fn selected(&self) -> Option<SubscriptionFilterDedup> {
        SubscriptionFilterDedup::from_index(self.draft.dedup_index)
    }

    pub fn value_for(&self, profile: &str) -> Option<SubscriptionFilterDedup> {
        (self.can_edit()
            && self.source.baseline().is_some_and(|base| {
                base.workspace_identity.document.profile_id.as_str() == profile
            }))
        .then(|| self.selected())
        .flatten()
    }

    pub fn edit(&mut self, field: FilterField, value: String) -> bool {
        if !self.can_edit() {
            return false;
        }
        if field.value(&self.draft) != value {
            field.set(&mut self.draft, value);
            self.draft_state = FilterDraftState::Modified;
            self.action = FilterActionState::Ready;
        }
        true
    }

    pub fn pick(&mut self, mode: SubscriptionFilterDedup) -> bool {
        if !self.can_edit() {
            return false;
        }
        if self.draft.dedup_index != mode.index() {
            self.draft.dedup_index = mode.index();
            self.draft_state = FilterDraftState::Modified;
            self.action = FilterActionState::Ready;
        }
        true
    }

    pub fn cancel(&mut self) -> bool {
        if matches!(&self.action, FilterActionState::Pending(_)) {
            return false;
        }
        let latest = self.source.latest().cloned();
        self.source = FilterSourceState::at(latest.clone());
        self.draft = latest.map(|source| source.filter).unwrap_or_default();
        self.action = FilterActionState::Ready;
        self.draft_state = FilterDraftState::Pristine;
        true
    }

    pub fn source_profile(&self) -> Option<&str> {
        self.source
            .baseline()
            .map(|base| base.workspace_identity.document.profile_id.as_str())
    }

    pub fn composing(&mut self) {
        if self.can_edit() {
            self.draft_state = FilterDraftState::Modified;
        }
    }

    pub fn latest_profile(&self) -> Option<&str> {
        self.source
            .latest()
            .map(|latest| latest.workspace_identity.document.profile_id.as_str())
    }

    pub fn read_failure(&self) -> Option<&Failure> {
        match &self.read {
            FilterReadState::Waiting(failure) | FilterReadState::Failed(failure) => Some(failure),
            FilterReadState::Unobserved | FilterReadState::Ready => None,
        }
    }

    pub fn failure(&self) -> Option<&Failure> {
        match &self.action {
            FilterActionState::Rejected(failure) | FilterActionState::Failed(failure) => {
                Some(failure)
            }
            FilterActionState::Ready
            | FilterActionState::Pending(_)
            | FilterActionState::Applied(_) => None,
        }
    }

    pub fn pending(&self) -> Option<&PendingFilterChange> {
        match &self.action {
            FilterActionState::Pending(pending) => Some(pending),
            FilterActionState::Ready
            | FilterActionState::Rejected(_)
            | FilterActionState::Failed(_)
            | FilterActionState::Applied(_) => None,
        }
    }

    pub fn applied(&self) -> bool {
        matches!(&self.action, FilterActionState::Applied(_))
    }

    pub fn report(&self) -> Option<&FilterReport> {
        match &self.action {
            FilterActionState::Applied(report) => Some(report),
            FilterActionState::Ready
            | FilterActionState::Pending(_)
            | FilterActionState::Rejected(_)
            | FilterActionState::Failed(_) => None,
        }
    }

    pub fn begin(&mut self) -> Result<PendingFilterChange, Failure> {
        if !self.can_edit() {
            let failure = Failure::new(
                ErrorCode::NotReady,
                "The filter form has no current source or a write is pending",
                true,
            );
            self.action = FilterActionState::Rejected(failure.clone());
            return Err(failure);
        }
        if let Err(error) = filter_spec_from_draft(&self.draft) {
            let failure = Failure::new(ErrorCode::InvalidInput, error.to_string(), false);
            self.action = FilterActionState::Rejected(failure.clone());
            return Err(failure);
        }
        self.next_token = self.next_token.checked_add(1).ok_or_else(|| {
            let failure = Failure::new(ErrorCode::InvalidState, "Filter token exhausted", false);
            self.action = FilterActionState::Rejected(failure.clone());
            failure
        })?;
        let pending = PendingFilterChange {
            token: self.next_token,
            source: self.source.baseline().cloned().expect("editable source"),
            draft: self.draft.clone(),
        };
        self.action = FilterActionState::Pending(pending.clone());
        Ok(pending)
    }

    pub fn finish(
        &mut self,
        token: u64,
        result: Result<SubscriptionFilterApplied, Failure>,
    ) -> bool {
        let Some(pending) = self
            .pending()
            .filter(|pending| pending.token == token)
            .cloned()
        else {
            return false;
        };
        match result {
            Ok(applied)
                if applied.workspace_identity.document.profile_id
                    == pending.source.workspace_identity.document.profile_id =>
            {
                let observed = FilterObservation {
                    workspace_identity: applied.workspace_identity,
                    filter: pending.draft,
                };
                let latest = self
                    .source
                    .latest()
                    .filter(|latest| *latest == &pending.source)
                    .map(|_| observed.clone())
                    .or_else(|| self.source.latest().cloned());
                self.source = FilterSourceState::with_pair(observed, latest);
                self.draft_state = FilterDraftState::Pristine;
                self.action = FilterActionState::Applied(applied.report);
            }
            Ok(_) => {
                self.action = FilterActionState::Failed(Failure::new(
                    ErrorCode::InvalidState,
                    "Filter receipt belongs to another profile",
                    false,
                ));
            }
            Err(failure) => self.action = FilterActionState::Failed(failure),
        }
        true
    }
}

impl FilterSourceState {
    fn at(observation: Option<FilterObservation>) -> Self {
        match observation {
            Some(observation) => Self::Observed {
                baseline: observation.clone(),
                latest: observation,
            },
            None => Self::Unobserved,
        }
    }

    fn with_pair(baseline: FilterObservation, latest: Option<FilterObservation>) -> Self {
        match latest {
            Some(latest) => Self::Observed { baseline, latest },
            None => Self::MissingLatest { baseline },
        }
    }

    fn with_latest(&self, latest: Option<FilterObservation>) -> Self {
        let baseline = self.baseline().cloned();
        match (baseline, latest) {
            (Some(baseline), Some(latest)) => Self::Observed { baseline, latest },
            (Some(baseline), None) => Self::MissingLatest { baseline },
            (None, Some(latest)) => Self::at(Some(latest)),
            (None, None) => Self::Unobserved,
        }
    }

    fn baseline(&self) -> Option<&FilterObservation> {
        match self {
            Self::Unobserved => None,
            Self::Observed { baseline, .. } | Self::MissingLatest { baseline } => Some(baseline),
        }
    }

    fn latest(&self) -> Option<&FilterObservation> {
        match self {
            Self::Unobserved | Self::MissingLatest { .. } => None,
            Self::Observed { latest, .. } => Some(latest),
        }
    }
}

impl FilterTarget {
    fn profile(&self) -> Option<&str> {
        match self {
            Self::Unselected => None,
            Self::Selected(profile) => Some(profile),
        }
    }
}

fn target_from(observation: Option<&FilterObservation>) -> FilterTarget {
    observation.map_or(FilterTarget::Unselected, |observation| {
        FilterTarget::Selected(
            observation
                .workspace_identity
                .document
                .profile_id
                .as_str()
                .to_owned(),
        )
    })
}

#[cfg(test)]
#[path = "subscription_filter_editor_tests.rs"]
mod tests;
