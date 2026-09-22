//! DUAL-05 custom-node protocol studio actions (shared protocol codec surface).
//!
//! The modal and its form are a projection of the shared
//! `ProtocolStudioSnapshot` published by
//! `infiltrator_application::protocol_codec_application`: parsing a share
//! link, editing the draft, exporting a link and committing the node into the
//! active profile all go through that one application. This module owns only
//! the surface state machine around it.

use crate::state::AppState;
use crate::types::app::ToastStatus;
use crate::types::message::Message;
use iced::Task;
use infiltrator_application::protocol_codec_application::ProtocolCodecApplication;
use infiltrator_contract::protocol_fidelity::ProtocolDraft;

impl AppState {
    /// DUAL-05-01/02/11/14: the shared protocol codec handlers.
    pub(super) fn update_protocol_codec(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::OpenCustomNodeModal => {
                // DUAL-05: the modal is a projection of the shared studio, so
                // opening it seeds an explicit draft instead of leaving the
                // form in a "no facts" limbo.
                self.runtime.custom_node_modal_open = true;
                self.runtime.custom_node_uri_input.clear();
                self.runtime.custom_node_studio =
                    ProtocolCodecApplication::publish_draft(ProtocolDraft::new("vless"), None);
                Task::none()
            }
            Message::CloseCustomNodeModal => {
                self.runtime.custom_node_modal_open = false;
                Task::none()
            }
            Message::UpdateCustomNodeUriInput(u) => {
                self.runtime.custom_node_uri_input = u;
                Task::none()
            }
            Message::UpdateCustomNodeDraft(draft) => {
                // One shared re-derivation: report + URI gaps come from the
                // application, never from the view.
                let preview = ProtocolCodecApplication::uri_from_draft(&draft).ok();
                self.runtime.custom_node_studio =
                    ProtocolCodecApplication::publish_draft(*draft, preview);
                Task::none()
            }
            Message::ParseAndImportCustomUri => {
                // DUAL-05-14: decode through the shared codec so both surfaces
                // see the same typed draft, cipher family, REALITY block and
                // multiplexing parameters. Iced no longer copies fields by hand.
                let uri = self.runtime.custom_node_uri_input.trim().to_string();
                match ProtocolCodecApplication::draft_from_uri(&uri) {
                    Ok(draft) => {
                        let preview = ProtocolCodecApplication::uri_from_draft(&draft).ok();
                        self.runtime.custom_node_studio =
                            ProtocolCodecApplication::publish_draft(draft, preview);
                        Task::none()
                    }
                    Err(failure) => {
                        // Keep the current draft and mark it clearly: a failed
                        // parse never half-fills a node. The error is published
                        // for the shared studio projection too.
                        let next = self
                            .runtime
                            .custom_node_studio
                            .clone()
                            .with_error(failure.message.clone());
                        infiltrator_application::protocol_codec_application::publish_studio(
                            next.clone(),
                        );
                        self.runtime.custom_node_studio = next;
                        Task::done(Message::ShowToast(failure.message, ToastStatus::Error))
                    }
                }
            }
            Message::ExportCustomNodeUri => {
                // Re-export from the real draft and report what a share link
                // cannot carry; nothing is fabricated from a placeholder node.
                let Some(draft) = self.runtime.custom_node_studio.draft.clone() else {
                    // No draft means nothing to encode: surface the shared
                    // error instead of fabricating a placeholder node.
                    let failure = "no node draft to export".to_string();
                    let next = self
                        .runtime
                        .custom_node_studio
                        .clone()
                        .with_error(failure.clone());
                    infiltrator_application::protocol_codec_application::publish_studio(
                        next.clone(),
                    );
                    self.runtime.custom_node_studio = next;
                    return Task::done(Message::ShowToast(failure, ToastStatus::Error));
                };
                let preview = ProtocolCodecApplication::uri_from_draft(&draft).ok();
                self.runtime.custom_node_studio =
                    ProtocolCodecApplication::publish_draft(draft, preview);
                Task::none()
            }
            Message::SaveCustomNodeForm => {
                let Some(draft) = self.runtime.custom_node_studio.draft.clone() else {
                    let failure = "no node draft to save".to_string();
                    let next = self
                        .runtime
                        .custom_node_studio
                        .clone()
                        .with_error(failure.clone());
                    infiltrator_application::protocol_codec_application::publish_studio(
                        next.clone(),
                    );
                    self.runtime.custom_node_studio = next;
                    return Task::done(Message::ShowToast(failure, ToastStatus::Error));
                };
                if self.runtime.custom_node_studio.has_blocking_issue() {
                    let issues = self.runtime.custom_node_studio.issue_lines().join("; ");
                    let next = self
                        .runtime
                        .custom_node_studio
                        .clone()
                        .with_error(issues.clone());
                    infiltrator_application::protocol_codec_application::publish_studio(
                        next.clone(),
                    );
                    self.runtime.custom_node_studio = next;
                    return Task::done(Message::ShowToast(issues, ToastStatus::Error));
                }
                let runtime = self.runtime.runtime.clone();
                // DUAL-05-14: the splice keeps every other profile section and
                // every unknown node key; the old parse/export round trip
                // silently dropped rules/dns/proxy-groups.
                crate::update::core::profile_apply::save_task(
                    runtime,
                    move |content| {
                        ProtocolCodecApplication::upsert_draft_into_profile(content, &draft)
                            .map(|commit| commit.profile_yaml)
                            .map_err(|failure| anyhow::anyhow!(failure.message))
                    },
                    Message::CustomNodeSaved,
                )
            }
            Message::CustomNodeSaved(result) => {
                self.runtime.custom_node_modal_open = false;
                match result {
                    Ok(_) => {
                        self.runtime.custom_node_uri_input.clear();
                        Task::batch(vec![
                            Task::done(Message::LoadProxies),
                            Task::done(Message::ShowToast(
                                "Proxy node saved to active profile".to_string(),
                                ToastStatus::Success,
                            )),
                        ])
                    }
                    Err(e) => {
                        self.set_error(&e);
                        Task::done(Message::ShowToast(e.to_string(), ToastStatus::Error))
                    }
                }
            }
            _ => Task::none(),
        }
    }
}
