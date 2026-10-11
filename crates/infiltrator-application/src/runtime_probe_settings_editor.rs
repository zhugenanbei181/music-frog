//! Explicit editor states for the Core-owned runtime probe settings.

use crate::runtime_probe_settings::parse_draft;
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::runtime_settings::{RuntimeProbeSettings, RuntimeProbeSettingsState};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RuntimeProbeSettingsDraft {
    pub url: String,
    pub timeout_ms: String,
}

impl From<&RuntimeProbeSettings> for RuntimeProbeSettingsDraft {
    fn from(settings: &RuntimeProbeSettings) -> Self {
        Self {
            url: settings.url().to_owned(),
            timeout_ms: settings.timeout_ms().to_string(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PendingRuntimeProbeSettings {
    pub token: u64,
    pub settings: RuntimeProbeSettings,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RuntimeProbeSettingsWriteOutcome {
    Stale,
    Saved,
    Failed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Checkpoint {
    applied: RuntimeProbeSettings,
    draft: RuntimeProbeSettingsDraft,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum EditorState {
    Unavailable {
        failure: Failure,
        previous: Option<Checkpoint>,
    },
    Ready {
        applied: RuntimeProbeSettings,
        draft: RuntimeProbeSettingsDraft,
    },
    Editing {
        applied: RuntimeProbeSettings,
        draft: RuntimeProbeSettingsDraft,
    },
    Saving {
        token: u64,
        applied: RuntimeProbeSettings,
        draft: RuntimeProbeSettingsDraft,
        submitted: RuntimeProbeSettings,
    },
    AwaitingReadback {
        expected: RuntimeProbeSettings,
        draft: RuntimeProbeSettingsDraft,
    },
    WriteFailed {
        applied: RuntimeProbeSettings,
        draft: RuntimeProbeSettingsDraft,
        failure: Failure,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RuntimeProbeSettingsEditor {
    state: EditorState,
    next_token: u64,
}

impl Default for RuntimeProbeSettingsEditor {
    fn default() -> Self {
        Self {
            state: EditorState::Unavailable {
                failure: Failure::unsupported("runtime probe settings have not been observed"),
                previous: None,
            },
            next_token: 0,
        }
    }
}

impl RuntimeProbeSettingsEditor {
    pub fn observe(&mut self, observation: &RuntimeProbeSettingsState) {
        match observation {
            RuntimeProbeSettingsState::Available { settings } => match &self.state {
                EditorState::Saving { .. } | EditorState::Editing { .. } => {}
                EditorState::AwaitingReadback { expected, .. } if expected == settings => {
                    self.state = EditorState::Ready {
                        applied: settings.clone(),
                        draft: RuntimeProbeSettingsDraft::from(settings),
                    };
                }
                EditorState::AwaitingReadback { .. } => {}
                EditorState::WriteFailed { .. } => {}
                EditorState::Unavailable {
                    previous: Some(checkpoint),
                    ..
                } => {
                    self.state = EditorState::Editing {
                        applied: settings.clone(),
                        draft: checkpoint.draft.clone(),
                    };
                }
                EditorState::Unavailable { previous: None, .. } | EditorState::Ready { .. } => {
                    self.state = EditorState::Ready {
                        applied: settings.clone(),
                        draft: RuntimeProbeSettingsDraft::from(settings),
                    };
                }
            },
            RuntimeProbeSettingsState::Unavailable { failure } => match &self.state {
                EditorState::Saving { .. } | EditorState::AwaitingReadback { .. } => {}
                EditorState::Unavailable { previous, .. } => {
                    self.state = EditorState::Unavailable {
                        failure: failure.clone(),
                        previous: previous.clone(),
                    };
                }
                EditorState::Ready { applied, draft }
                | EditorState::Editing { applied, draft }
                | EditorState::WriteFailed {
                    applied,
                    draft,
                    failure: _,
                } => {
                    self.state = EditorState::Unavailable {
                        failure: failure.clone(),
                        previous: Some(Checkpoint {
                            applied: applied.clone(),
                            draft: draft.clone(),
                        }),
                    };
                }
            },
        }
    }

    pub fn draft(&self) -> Option<&RuntimeProbeSettingsDraft> {
        match &self.state {
            EditorState::Unavailable { previous, .. } => {
                previous.as_ref().map(|entry| &entry.draft)
            }
            EditorState::Ready { draft, .. }
            | EditorState::Editing { draft, .. }
            | EditorState::Saving { draft, .. }
            | EditorState::WriteFailed { draft, .. } => Some(draft),
            EditorState::AwaitingReadback { draft, .. } => Some(draft),
        }
    }

    pub fn applied(&self) -> Option<&RuntimeProbeSettings> {
        match &self.state {
            EditorState::Unavailable { previous, .. } => {
                previous.as_ref().map(|entry| &entry.applied)
            }
            EditorState::Ready { applied, .. }
            | EditorState::Editing { applied, .. }
            | EditorState::Saving { applied, .. }
            | EditorState::WriteFailed { applied, .. } => Some(applied),
            EditorState::AwaitingReadback { expected, .. } => Some(expected),
        }
    }

    pub fn failure(&self) -> Option<&Failure> {
        match &self.state {
            EditorState::Unavailable { failure, .. } | EditorState::WriteFailed { failure, .. } => {
                Some(failure)
            }
            _ => None,
        }
    }

    pub fn record_failure(&mut self, failure: Failure) {
        let (applied, draft) = match &self.state {
            EditorState::Editing { applied, draft }
            | EditorState::WriteFailed { applied, draft, .. } => (applied.clone(), draft.clone()),
            _ => return,
        };
        self.state = EditorState::WriteFailed {
            applied,
            draft,
            failure,
        };
    }

    pub fn validation(&self) -> Option<Failure> {
        match &self.state {
            EditorState::Ready { draft, .. }
            | EditorState::Editing { draft, .. }
            | EditorState::WriteFailed { draft, .. }
            | EditorState::Unavailable {
                previous: Some(Checkpoint { draft, .. }),
                ..
            } => parse_draft(draft).err(),
            _ => None,
        }
    }

    pub fn pending(&self) -> Option<PendingRuntimeProbeSettings> {
        match &self.state {
            EditorState::Saving {
                token, submitted, ..
            } => Some(PendingRuntimeProbeSettings {
                token: *token,
                settings: submitted.clone(),
            }),
            _ => None,
        }
    }

    pub fn is_pending(&self) -> bool {
        matches!(
            &self.state,
            EditorState::Saving { .. } | EditorState::AwaitingReadback { .. }
        )
    }

    pub fn can_apply(&self) -> bool {
        match &self.state {
            EditorState::Editing { applied, draft }
            | EditorState::WriteFailed { applied, draft, .. } => {
                RuntimeProbeSettingsDraft::from(applied) != *draft && parse_draft(draft).is_ok()
            }
            _ => false,
        }
    }

    fn edit(&mut self, update: impl FnOnce(&mut RuntimeProbeSettingsDraft)) {
        let (applied, mut draft) = match &self.state {
            EditorState::Ready { applied, draft }
            | EditorState::Editing { applied, draft }
            | EditorState::WriteFailed { applied, draft, .. } => (applied.clone(), draft.clone()),
            _ => return,
        };
        update(&mut draft);
        self.state = EditorState::Editing { applied, draft };
    }

    pub fn edit_url(&mut self, value: String) {
        self.edit(|draft| draft.url = value);
    }

    pub fn edit_timeout(&mut self, value: String) {
        self.edit(|draft| draft.timeout_ms = value);
    }

    pub fn cancel(&mut self) {
        let checkpoint = match &self.state {
            EditorState::Ready { .. }
            | EditorState::Saving { .. }
            | EditorState::AwaitingReadback { .. } => return,
            EditorState::Unavailable { .. } => return,
            EditorState::Editing { applied, .. } | EditorState::WriteFailed { applied, .. } => {
                Some(Checkpoint {
                    applied: applied.clone(),
                    draft: RuntimeProbeSettingsDraft::from(applied),
                })
            }
        };
        self.state = match checkpoint {
            Some(checkpoint) => EditorState::Ready {
                applied: checkpoint.applied,
                draft: checkpoint.draft,
            },
            None => EditorState::Unavailable {
                failure: Failure::unsupported("runtime probe settings have not been observed"),
                previous: None,
            },
        };
    }

    pub fn begin(&mut self) -> Result<PendingRuntimeProbeSettings, Failure> {
        let (applied, draft) = match &self.state {
            EditorState::Editing { applied, draft }
            | EditorState::WriteFailed { applied, draft, .. } => (applied.clone(), draft.clone()),
            EditorState::Unavailable { .. } => {
                return Err(Failure::new(
                    ErrorCode::NotReady,
                    "runtime probe settings are unavailable",
                    true,
                ));
            }
            EditorState::Ready { .. } => {
                return Err(Failure::new(
                    ErrorCode::InvalidState,
                    "runtime probe settings have no changes",
                    false,
                ));
            }
            EditorState::Saving { .. } | EditorState::AwaitingReadback { .. } => {
                return Err(Failure::new(
                    ErrorCode::InvalidState,
                    "runtime probe settings write is already pending",
                    false,
                ));
            }
        };
        if RuntimeProbeSettingsDraft::from(&applied) == draft {
            return Err(Failure::new(
                ErrorCode::InvalidState,
                "runtime probe settings have no changes",
                false,
            ));
        }
        let settings = parse_draft(&draft)?;
        self.next_token = self.next_token.checked_add(1).ok_or_else(|| {
            Failure::new(
                ErrorCode::InvalidState,
                "runtime probe settings request sequence exhausted",
                false,
            )
        })?;
        let pending = PendingRuntimeProbeSettings {
            token: self.next_token,
            settings: settings.clone(),
        };
        self.state = EditorState::Saving {
            token: pending.token,
            applied,
            draft,
            submitted: settings,
        };
        Ok(pending)
    }

    pub fn finish(
        &mut self,
        token: u64,
        result: Result<(), Failure>,
    ) -> RuntimeProbeSettingsWriteOutcome {
        let (expected_token, applied, draft, submitted) = match &self.state {
            EditorState::Saving {
                token,
                applied,
                draft,
                submitted,
            } => (*token, applied.clone(), draft.clone(), submitted.clone()),
            _ => return RuntimeProbeSettingsWriteOutcome::Stale,
        };
        if expected_token != token {
            return RuntimeProbeSettingsWriteOutcome::Stale;
        }
        match result {
            Ok(()) => {
                self.state = EditorState::AwaitingReadback {
                    expected: submitted.clone(),
                    draft: RuntimeProbeSettingsDraft::from(&submitted),
                };
                RuntimeProbeSettingsWriteOutcome::Saved
            }
            Err(failure) => {
                self.state = EditorState::WriteFailed {
                    applied,
                    draft,
                    failure,
                };
                RuntimeProbeSettingsWriteOutcome::Failed
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn available(settings: RuntimeProbeSettings) -> RuntimeProbeSettingsState {
        RuntimeProbeSettingsState::Available { settings }
    }

    #[test]
    fn invalid_draft_and_stale_write_receipt_never_change_the_applied_value() {
        let original = RuntimeProbeSettings::default();
        let mut editor = RuntimeProbeSettingsEditor::default();
        editor.observe(&available(original.clone()));
        editor.edit_timeout("0".into());
        assert!(editor.validation().is_some());
        assert!(!editor.can_apply());
        assert_eq!(editor.applied(), Some(&original));

        editor.cancel();
        editor.edit_url("https://probe.example.test/check".into());
        let pending = editor.begin().unwrap();
        assert!(editor.begin().is_err());
        assert_eq!(
            editor.finish(pending.token + 1, Ok(())),
            RuntimeProbeSettingsWriteOutcome::Stale
        );
        assert_eq!(editor.applied(), Some(&original));
        assert_eq!(
            editor.finish(pending.token, Err(Failure::unsupported("write denied"))),
            RuntimeProbeSettingsWriteOutcome::Failed
        );
        assert_eq!(editor.applied(), Some(&original));
        assert!(editor.failure().is_some());
    }

    #[test]
    fn successful_write_waits_for_matching_owner_readback_before_becoming_ready() {
        let mut editor = RuntimeProbeSettingsEditor::default();
        editor.observe(&available(RuntimeProbeSettings::default()));
        editor.edit_url("https://probe.example.test/check".into());
        let pending = editor.begin().unwrap();
        assert_eq!(
            editor.finish(pending.token, Ok(())),
            RuntimeProbeSettingsWriteOutcome::Saved
        );
        assert!(editor.is_pending());
        editor.observe(&available(RuntimeProbeSettings::default()));
        assert!(
            editor.is_pending(),
            "a stale snapshot cannot undo the write"
        );
        editor.observe(&available(pending.settings.clone()));
        assert!(!editor.is_pending());
        assert_eq!(editor.applied(), Some(&pending.settings));
    }
}
