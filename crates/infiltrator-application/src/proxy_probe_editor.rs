//! Neutral editing state for latency parameters; a draft becomes applied only after durable success.
use crate::proxy_probe_options_projection::parse_draft;
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::proxy_probe_options::{
    ProxyProbeDraft, ProxyProbeOptions, ProxyProbeSettingsSnapshot,
};

#[derive(Clone, Debug)]
pub struct PendingProbeOptions {
    pub token: u64,
    pub options: ProxyProbeOptions,
}
#[derive(Clone, Debug, Default)]
pub struct ProxyProbeEditor {
    pub draft: ProxyProbeDraft,
    pub applied: Option<ProxyProbeOptions>,
    pub can_persist: bool,
    pub dirty: bool,
    pub validation: Option<Failure>,
    pub failure: Option<Failure>,
    pub pending: Option<PendingProbeOptions>,
    awaiting: Option<ProxyProbeOptions>,
    next_token: u64,
}
impl ProxyProbeEditor {
    pub fn observe(&mut self, snapshot: &ProxyProbeSettingsSnapshot) {
        self.can_persist = snapshot.can_persist;
        if let Some(expected) = &self.awaiting {
            if snapshot.options.as_ref() != Some(expected) {
                return;
            }
            self.awaiting = None;
        }
        if snapshot.failure.is_some() && snapshot.options.is_none() {
            self.failure = snapshot.failure.clone();
            return;
        }
        self.applied = snapshot.options.clone();
        if !self.dirty && self.pending.is_none() {
            self.draft = self
                .applied
                .as_ref()
                .map(ProxyProbeDraft::from)
                .unwrap_or_default();
            self.validation = snapshot.failure.clone();
        }
    }
    fn edited(&mut self) {
        self.dirty = true;
        self.failure = None;
        self.validation = parse_draft(&self.draft).err();
    }
    pub fn edit_url(&mut self, value: String) {
        if self.pending.is_some() {
            return;
        }
        self.draft.test_url = value;
        self.edited();
    }
    pub fn edit_timeout(&mut self, value: String) {
        if self.pending.is_some() {
            return;
        }
        self.draft.timeout_ms = value;
        self.edited();
    }
    pub fn cancel(&mut self) {
        if self.pending.is_some() {
            return;
        }
        self.draft = self
            .applied
            .as_ref()
            .map(ProxyProbeDraft::from)
            .unwrap_or_default();
        self.validation = parse_draft(&self.draft).err();
        self.failure = None;
        self.dirty = false;
    }
    pub fn can_apply(&self) -> bool {
        self.can_persist && self.dirty && self.pending.is_none() && self.validation.is_none()
    }
    pub fn begin(&mut self) -> Result<PendingProbeOptions, Failure> {
        if !self.can_persist {
            return Err(Failure::new(
                ErrorCode::NotReady,
                "probe settings store is not available",
                true,
            ));
        }
        if self.pending.is_some() {
            return Err(Failure::new(
                ErrorCode::InvalidState,
                "probe settings write is already pending",
                false,
            ));
        }
        if !self.dirty {
            return Err(Failure::new(
                ErrorCode::InvalidState,
                "probe settings draft has no changes",
                false,
            ));
        }
        let options = parse_draft(&self.draft)?;
        self.next_token = self.next_token.wrapping_add(1);
        let pending = PendingProbeOptions {
            token: self.next_token,
            options,
        };
        self.pending = Some(pending.clone());
        self.failure = None;
        Ok(pending)
    }
    pub fn finish(&mut self, token: u64, result: Result<(), Failure>) -> bool {
        let Some(pending) = &self.pending else {
            return false;
        };
        if pending.token != token {
            return false;
        }
        let options = pending.options.clone();
        self.pending = None;
        match result {
            Ok(()) => {
                self.applied = Some(options.clone());
                self.awaiting = Some(options.clone());
                self.draft = ProxyProbeDraft::from(&options);
                self.dirty = false;
                self.validation = None;
                self.failure = None;
            }
            Err(failure) => self.failure = Some(failure),
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cancel_validation_failure_and_stale_results_cannot_publish_applied_options() {
        let mut editor = ProxyProbeEditor::default();
        let snapshot = ProxyProbeSettingsSnapshot {
            can_persist: true,
            ..Default::default()
        };
        editor.observe(&snapshot);
        let original = editor.applied.clone();
        editor.edit_timeout("60000".into());
        assert!(editor.validation.is_some());
        assert!(!editor.can_apply());
        assert!(editor.begin().is_err());
        assert_eq!(editor.applied, original);
        editor.cancel();
        assert!(!editor.dirty);
        editor.edit_url("https://probe.example.test/check".into());
        let pending = editor.begin().unwrap();
        assert!(editor.begin().is_err());
        assert!(!editor.finish(pending.token + 1, Ok(())));
        let failure = Failure::new(ErrorCode::Storage, "write denied", true);
        assert!(editor.finish(pending.token, Err(failure.clone())));
        assert_eq!(editor.failure, Some(failure));
        assert!(editor.draft.test_url.contains("probe.example.test"));
        assert_eq!(editor.applied, original);
        let retry = editor.begin().unwrap();
        assert!(editor.finish(retry.token, Ok(())));
        let committed = editor.applied.clone();
        editor.observe(&snapshot);
        assert_eq!(
            editor.applied, committed,
            "a delayed reader cannot undo a completed write"
        );
        let confirmed = ProxyProbeSettingsSnapshot {
            options: committed.clone(),
            can_persist: true,
            failure: None,
        };
        editor.observe(&confirmed);
        assert_eq!(editor.applied, committed);
    }
}
