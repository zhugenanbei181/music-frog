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
use infiltrator_application::profile_application::ProfileApplication;
use infiltrator_application::protocol_codec_application::ProtocolCodecApplication;
use infiltrator_contract::error::InfiltratorError;
use infiltrator_contract::protocol_fidelity::ProtocolDraft;

/// DUAL-05-09/10: load the active profile and run the shared dialer analyzer.
async fn scan_dialer_chains()
-> Result<infiltrator_contract::dialer_chain::DialerChainReport, InfiltratorError> {
    let store = crate::configs_dir::config_manager().await?;
    let application = ProfileApplication::new(store);
    let (_profile, content) = application
        .current_content()
        .await
        .map_err(|failure| InfiltratorError::Config(failure.message))?;
    ProtocolCodecApplication::publish_dialer_report(&content)
        .map_err(|failure| InfiltratorError::Config(failure.message))
}

impl AppState {
    /// DUAL-05-13: this host's CA reader, if it composed one.
    fn certificate_authority_port(
        &self,
    ) -> Option<
        std::sync::Arc<dyn infiltrator_ports::certificate_authority::CertificateAuthorityPort>,
    > {
        self.runtime
            .runtime
            .as_ref()
            .and_then(|runtime| runtime.certificate_authority_port())
    }

    /// DUAL-05-13: resolve the draft's CA request and refresh the studio.
    fn refresh_custom_node_ca_trust(
        &mut self,
    ) -> infiltrator_contract::protocol_trust::CaTrustReport {
        let draft = self.runtime.custom_node_studio.draft.clone();
        let params = draft
            .as_ref()
            .map(|draft| draft.params.tls_trust.clone())
            .unwrap_or_default();
        let port = self.certificate_authority_port();
        let report = ProtocolCodecApplication::publish_ca_trust(&params, port.as_deref());
        self.runtime.custom_node_studio.ca_trust = report.clone();
        report
    }

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
                self.refresh_custom_node_ca_trust();
                // DUAL-05-09/10: the dialer graph is a profile fact, so the
                // modal scans the active profile through the shared analyzer.
                Task::perform(scan_dialer_chains(), Message::CustomNodeDialerScanned)
            }
            Message::ScanCustomNodeDialer => {
                Task::perform(scan_dialer_chains(), Message::CustomNodeDialerScanned)
            }
            Message::CustomNodeDialerScanned(result) => match result {
                Ok(report) => {
                    self.runtime.custom_node_studio.dialer = report;
                    Task::none()
                }
                Err(error) => Task::done(Message::ShowToast(error.to_string(), ToastStatus::Error)),
            },
            Message::VerifyCustomNodeCertificateAuthority => {
                // DUAL-05-13: the outcome is a typed state (loaded / unsupported
                // / failed) rendered by the modal, not a toast; an unsupported
                // host is an expected answer.
                self.refresh_custom_node_ca_trust();
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
                // DUAL-05-13: the trust request changed with the draft, so the
                // host resolution is re-derived instead of going stale.
                self.refresh_custom_node_ca_trust();
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
                        // DUAL-05-13: a freshly imported draft carries its own
                        // trust request; resolve it against this host at once.
                        self.refresh_custom_node_ca_trust();
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
                // DUAL-05-13: keep the published CA resolution in step with the
                // draft the modal is showing.
                self.refresh_custom_node_ca_trust();
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
                        // DUAL-05-13/09: the save published the written
                        // document's dialer verdict; re-derive the CA state so
                        // the panel never shows a stale resolution.
                        self.refresh_custom_node_ca_trust();
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
