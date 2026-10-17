//! Typed command/readback state for the shared custom-protocol editor.

use crate::protocol_form::draft_from_uri;
use infiltrator_contract::command::RequestId;
use infiltrator_contract::protocol_fidelity::{ProtocolDraft, ProtocolStudioSnapshot};
use lurker_core_contract::error::{ErrorCode, Failure};
use std::mem;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum ProtocolEditorOperation {
    #[default]
    Idle,
    ImportAwaitingTerminal {
        request_id: RequestId,
        uri: String,
        surface_generation: u64,
        surface_revision: u64,
    },
    ImportAwaitingReadback {
        expected_draft: ProtocolDraft,
        surface_generation: u64,
        surface_revision: u64,
    },
    SaveAwaitingTerminal {
        request_id: RequestId,
        draft: ProtocolDraft,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProtocolEditorEffect {
    Ignored,
    ImportAwaitingReadback,
    Imported(ProtocolStudioSnapshot),
    Saved,
    Failed(Failure),
}

impl ProtocolEditorOperation {
    pub fn begin_import(
        &mut self,
        request_id: RequestId,
        uri: String,
        surface_generation: u64,
        surface_revision: u64,
    ) -> Result<(), Failure> {
        match self {
            Self::Idle => {
                *self = Self::ImportAwaitingTerminal {
                    request_id,
                    uri,
                    surface_generation,
                    surface_revision,
                };
                Ok(())
            }
            Self::ImportAwaitingTerminal { .. }
            | Self::ImportAwaitingReadback { .. }
            | Self::SaveAwaitingTerminal { .. } => Err(Failure::new(
                ErrorCode::InvalidState,
                "a custom-node operation is already active",
                true,
            )),
        }
    }

    pub fn begin_save(
        &mut self,
        request_id: RequestId,
        draft: ProtocolDraft,
    ) -> Result<(), Failure> {
        match self {
            Self::Idle => {
                *self = Self::SaveAwaitingTerminal { request_id, draft };
                Ok(())
            }
            Self::ImportAwaitingTerminal { .. }
            | Self::ImportAwaitingReadback { .. }
            | Self::SaveAwaitingTerminal { .. } => Err(Failure::new(
                ErrorCode::InvalidState,
                "a custom-node operation is already active",
                true,
            )),
        }
    }

    pub fn finish_import(
        &mut self,
        request_id: RequestId,
        uri: &str,
        result: Result<(), Failure>,
    ) -> ProtocolEditorEffect {
        let previous = mem::take(self);
        match previous {
            Self::ImportAwaitingTerminal {
                request_id: expected_request,
                uri: expected_uri,
                surface_generation,
                surface_revision,
            } if request_id == expected_request && uri == expected_uri => match result {
                Err(failure) => ProtocolEditorEffect::Failed(failure),
                Ok(()) => match draft_from_uri(&expected_uri) {
                    Ok(expected_draft) => {
                        *self = Self::ImportAwaitingReadback {
                            expected_draft,
                            surface_generation,
                            surface_revision,
                        };
                        ProtocolEditorEffect::ImportAwaitingReadback
                    }
                    Err(failure) => ProtocolEditorEffect::Failed(failure),
                },
            },
            other => {
                *self = other;
                ProtocolEditorEffect::Ignored
            }
        }
    }

    pub fn finish_save(
        &mut self,
        request_id: RequestId,
        draft: &ProtocolDraft,
        result: Result<(), Failure>,
    ) -> ProtocolEditorEffect {
        let previous = mem::take(self);
        match previous {
            Self::SaveAwaitingTerminal {
                request_id: expected_request,
                draft: expected_draft,
            } if request_id == expected_request && draft == &expected_draft => match result {
                Ok(()) => ProtocolEditorEffect::Saved,
                Err(failure) => ProtocolEditorEffect::Failed(failure),
            },
            other => {
                *self = other;
                ProtocolEditorEffect::Ignored
            }
        }
    }

    pub fn observe_owner_snapshot(
        &mut self,
        generation: u64,
        revision: u64,
        snapshot: &ProtocolStudioSnapshot,
    ) -> ProtocolEditorEffect {
        let previous = mem::take(self);
        match previous {
            Self::ImportAwaitingReadback {
                expected_draft,
                surface_generation,
                surface_revision,
            } if (generation, revision) > (surface_generation, surface_revision)
                && snapshot.draft.as_ref() == Some(&expected_draft) =>
            {
                ProtocolEditorEffect::Imported(snapshot.clone())
            }
            other => {
                *self = other;
                ProtocolEditorEffect::Ignored
            }
        }
    }

    /// Retire this editor's pending observation when its owning surface closes.
    /// A later terminal receipt is ignored because it no longer matches an
    /// active operation.
    pub fn cancel(&mut self) {
        *self = Self::Idle;
    }

    pub const fn is_idle(&self) -> bool {
        matches!(self, Self::Idle)
    }

    pub const fn is_saving(&self) -> bool {
        matches!(self, Self::SaveAwaitingTerminal { .. })
    }

    pub const fn is_busy(&self) -> bool {
        !self.is_idle()
    }
}

#[cfg(test)]
#[path = "protocol_editor_operation_tests.rs"]
mod tests;
