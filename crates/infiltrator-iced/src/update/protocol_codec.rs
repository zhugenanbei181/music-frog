//! DUAL-05 custom-node protocol studio actions (shared protocol codec surface).
//!
//! The modal and its form are a projection of the shared
//! `ProtocolStudioSnapshot` published by
//! `infiltrator_application::protocol_codec_application`: parsing a share
//! link, editing the draft, exporting a link and committing the node into the
//! active profile all go through that one application. This module owns only
//! the surface state machine around it.

use crate::configs_dir::config_manager;
use crate::configuration::profiles_for_mihomo;
use crate::state::AppState;
use crate::state::runtime::CustomNodeSaveState;
use crate::types::app::ToastStatus;
use crate::types::message::Message;
use iced::Task;
use infiltrator_application::protocol_codec_application::{
    ProtocolCodecApplication, publish_studio,
};
use infiltrator_application::protocol_form::ProtocolInputs;
use infiltrator_contract::command::CommandIntent;
use infiltrator_contract::dialer_chain::DialerChainReport;
use infiltrator_contract::error::InfiltratorError;
use infiltrator_contract::profile_protection::ProfileWriteAuthorization;
use infiltrator_contract::protocol_fidelity::ProtocolDraft;
use infiltrator_contract::protocol_trust::CaTrustReport;
use infiltrator_ports::certificate_authority::CertificateAuthorityPort;
use std::sync::Arc;

/// DUAL-05-09/10: load the active profile and run the shared dialer analyzer.
async fn scan_dialer_chains() -> Result<DialerChainReport, InfiltratorError> {
    let store = config_manager().await?;
    let application = profiles_for_mihomo(store);
    let (_profile, content) = application
        .current_content()
        .await
        .map_err(|failure| InfiltratorError::Config(failure.message))?;
    ProtocolCodecApplication::publish_dialer_report(&content)
        .map_err(|failure| InfiltratorError::Config(failure.message))
}

impl AppState {
    /// DUAL-05-13: this host's CA reader, if it composed one.
    fn certificate_authority_port(&self) -> Option<Arc<dyn CertificateAuthorityPort>> {
        self.runtime
            .runtime
            .as_ref()
            .and_then(|runtime| runtime.certificate_authority_port())
    }

    /// DUAL-05-13: resolve the draft's CA request and refresh the studio.
    fn refresh_custom_node_ca_trust(&mut self) -> CaTrustReport {
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
        if !self.runtime.custom_node_modal_open
            && !matches!(
                message,
                Message::OpenCustomNodeModal
                    | Message::CustomNodeSaved(_)
                    | Message::CustomNodeDialerScanned(_)
            )
        {
            return Task::none();
        }
        match message {
            Message::OpenCustomNodeModal => {
                if self.runtime.custom_node_save_state == CustomNodeSaveState::Saving {
                    return Task::none();
                }
                // DUAL-05: the modal is a projection of the shared studio, so
                // opening it seeds an explicit draft instead of leaving the
                // form in a "no facts" limbo.
                self.runtime.custom_node_modal_open = true;
                self.runtime.custom_node_uri_input.clear();
                self.runtime.custom_node_inputs = ProtocolInputs::default();
                self.runtime.custom_node_studio =
                    ProtocolCodecApplication::publish_draft(ProtocolDraft::new("vless"), None);
                self.refresh_custom_node_ca_trust();
                // DUAL-05-09/10: the dialer graph is a profile fact, so the
                // modal scans the active profile through the shared analyzer.
                Task::none()
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
            Message::UpdateCustomNodeField(id, raw) => {
                let Some(draft) = self.runtime.custom_node_studio.draft.clone() else {
                    return Task::none();
                };
                match self.runtime.custom_node_inputs.edit(&draft, id, raw) {
                    Ok(next) => {
                        self.update_protocol_codec(Message::UpdateCustomNodeDraft(Box::new(next)))
                    }
                    Err(error) => {
                        self.runtime.custom_node_studio.last_error = Some(error.message);
                        Task::none()
                    }
                }
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
                        self.runtime.custom_node_inputs = ProtocolInputs::default();
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
                        publish_studio(next.clone());
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
                    publish_studio(next.clone());
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
                if self.runtime.custom_node_save_state == CustomNodeSaveState::Saving
                    || !self.runtime.custom_node_inputs.errors.is_empty()
                {
                    return Task::none();
                }
                let Some(draft) = self.runtime.custom_node_studio.draft.clone() else {
                    let failure = "no node draft to save".to_string();
                    let next = self
                        .runtime
                        .custom_node_studio
                        .clone()
                        .with_error(failure.clone());
                    publish_studio(next.clone());
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
                    publish_studio(next.clone());
                    self.runtime.custom_node_studio = next;
                    return Task::done(Message::ShowToast(issues, ToastStatus::Error));
                }
                self.runtime.custom_node_save_state = CustomNodeSaveState::Saving;
                if let Some(application) = self.command_ingress() {
                    return Task::perform(
                        async move {
                            match (application
                                .execute(CommandIntent::SaveCustomNodeDraft {
                                    draft: Box::new(draft),
                                })
                                .await)
                                .into_unit()
                            {
                                Ok(()) => Ok(()),
                                Err(failure) => Err(InfiltratorError::Config(failure.message)),
                            }
                        },
                        Message::CustomNodeSaved,
                    );
                }
                let runtime = self.runtime.runtime.clone();
                Task::perform(
                    async move {
                        let runtime = runtime.ok_or_else(|| {
                            InfiltratorError::Config(
                                "custom protocol editing requires an engine profile transaction"
                                    .to_owned(),
                            )
                        })?;
                        let store = config_manager().await?;
                        let profiles = profiles_for_mihomo(store);
                        let commit = ProtocolCodecApplication::save_draft_to_active_profile(
                            &profiles,
                            runtime,
                            &draft,
                            ProfileWriteAuthorization::RequireUnlock,
                        )
                        .await
                        .map_err(|failure| InfiltratorError::Config(failure.message))?;
                        ProtocolCodecApplication::publish_commit(&commit);
                        Ok(())
                    },
                    Message::CustomNodeSaved,
                )
            }
            Message::CustomNodeSaved(result) => {
                if self.runtime.custom_node_save_state != CustomNodeSaveState::Saving {
                    return Task::none();
                }
                self.runtime.custom_node_save_state = CustomNodeSaveState::Idle;
                match result {
                    Ok(_) => {
                        self.runtime.custom_node_modal_open = false;
                        self.runtime.custom_node_uri_input.clear();
                        // DUAL-05-13/09: the save published the written
                        // document's dialer verdict; re-derive the CA state so
                        // the panel never shows a stale resolution.
                        self.refresh_custom_node_ca_trust();
                        Task::batch(vec![
                            Task::done(Message::RefreshOutboundInventory),
                            Task::done(Message::ShowToast(
                                "Proxy node saved to active profile".to_string(),
                                ToastStatus::Success,
                            )),
                        ])
                    }
                    Err(e) => {
                        self.runtime.custom_node_studio.last_error = Some(e.to_string());
                        self.set_error(&e);
                        Task::done(Message::ShowToast(e.to_string(), ToastStatus::Error))
                    }
                }
            }
            _ => Task::none(),
        }
    }
}
