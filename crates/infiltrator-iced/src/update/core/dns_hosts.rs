//! Hosts TEA actions consume the shared editor and real command terminal results.
use crate::state::AppState;
use crate::types::message::Message;
use iced::Task;
use infiltrator_contract::command::CommandIntent;
use lurker_core_contract::error::{ErrorCode, Failure};
impl AppState {
    pub(super) fn update_core_dns_hosts(&mut self, message: Message) -> Task<Message> {
        let command_ingress = self.command_ingress();
        let editor = &mut self.editor.dns_hosts_editor;
        match message {
            Message::OpenDnsHostsEditor => {
                editor.show();
                Task::none()
            }
            Message::CancelDnsHostRowInput => {
                if editor.is_open() {
                    editor.cancel_row();
                }
                Task::none()
            }
            Message::CancelDnsHostsEditor => {
                editor.cancel();
                Task::none()
            }
            Message::UpdateDnsHostsAddress(value) => {
                if editor.is_open() {
                    editor.edit_address(value);
                }
                Task::none()
            }
            Message::UpdateDnsHostsDomain(value) => {
                if editor.is_open() {
                    editor.edit_domain(value);
                }
                Task::none()
            }
            Message::AddDnsHostRow => {
                if editor.is_open() {
                    editor.commit_row();
                }
                Task::none()
            }
            Message::EditDnsHostRow(id) => {
                if editor.is_open() {
                    editor.select(id);
                }
                Task::none()
            }
            Message::RemoveDnsHostRow(id) => {
                if editor.is_open() {
                    editor.remove(id);
                }
                Task::none()
            }
            Message::ImportLegacyDnsHosts => {
                if editor.is_open() {
                    editor.import_legacy();
                }
                Task::none()
            }
            Message::SaveDnsHosts => {
                let pending = match editor.begin() {
                    Ok(pending) => pending,
                    Err(_) => return Task::none(),
                };
                let Some(application) = command_ingress else {
                    editor.finish(
                        pending.token,
                        Err(Failure::new(
                            ErrorCode::NotReady,
                            "Hosts command service is unavailable",
                            true,
                        )),
                    );
                    return Task::none();
                };
                Task::perform(
                    async move {
                        match (application
                            .execute(CommandIntent::ApplyDnsSettings {
                                patch: pending.patch,
                            })
                            .await)
                            .into_unit()
                        {
                            Ok(()) => Ok(()),
                            Err(failure) => Err(failure),
                        }
                    },
                    move |result| Message::DnsHostsCommandFinished {
                        token: pending.token,
                        result,
                    },
                )
            }
            Message::DnsHostsCommandFinished { token, result } => {
                editor.finish(token, result);
                Task::none()
            }
            other => self.update_core_dns_leak(other),
        }
    }
}
