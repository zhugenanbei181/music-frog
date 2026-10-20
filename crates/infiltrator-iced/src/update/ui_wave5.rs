//! Wave 5 Advanced feature message handlers: Rule Hit Counter, Latency Radar,
//! TUN Multi-Stack, Rule Unpacker, Atomic Apply Guard, and LAN Proxy Sharing.

use crate::state::AppState;
use crate::types::app::ToastStatus;
use crate::types::message::Message;
use crate::types::message::mtu_probe::MtuProbeCompletion;
use iced::Task;
use infiltrator_application::mtu_application::MtuApplication;
use infiltrator_application::privileged_network_application::PrivilegedNetworkApplication;
use infiltrator_application::runtime_configuration_application::RuntimeConfigurationApplication;
use infiltrator_contract::lan::LanCredentials;
use infiltrator_contract::mtu::{MtuNegotiationSnapshot, MtuProbeState, PhysicalMtuSnapshot};
use infiltrator_contract::privileged_network::{
    PrivilegedNetworkRequest, PrivilegedNetworkSnapshot, PrivilegedNetworkState,
};
use infiltrator_ports::runtime_configuration::LanAuthenticationUpdate;
use infiltrator_shared::i18n_interpolator::localize;
use infiltrator_shared::locales::{Lang, Localizer};
use lurker_core_contract::error::{ErrorCode, Failure, InfiltratorError};

impl AppState {
    fn run_privileged_network_regression(&mut self) -> Task<Message> {
        let copy_locale = self.shell.lang.clone();
        let Some(runtime) = self.runtime.runtime.clone() else {
            let snapshot = PrivilegedNetworkSnapshot::unsupported(
                self.runtime.privileged_network.revision.saturating_add(1),
                Lang(&copy_locale)
                    .tr("privileged_network_unavailable")
                    .as_ref(),
            );
            return Task::done(Message::PrivilegedNetworkRegressionUpdated(Ok(snapshot)));
        };
        let Some(port) = runtime.privileged_network_port() else {
            let snapshot = PrivilegedNetworkSnapshot::unsupported(
                self.runtime.privileged_network.revision.saturating_add(1),
                Lang(&copy_locale)
                    .tr("privileged_network_unavailable")
                    .as_ref(),
            );
            return Task::done(Message::PrivilegedNetworkRegressionUpdated(Ok(snapshot)));
        };
        self.runtime.privileged_network = PrivilegedNetworkSnapshot {
            state: PrivilegedNetworkState::Injecting,
            operation_count: PrivilegedNetworkRequest::standard().operations.len(),
            revision: self.runtime.privileged_network.revision.saturating_add(1),
            ..PrivilegedNetworkSnapshot::default()
        };
        let application = PrivilegedNetworkApplication::new(port);
        Task::perform(
            async move {
                application
                    .run(PrivilegedNetworkRequest::standard())
                    .await
                    .map_err(|failure| InfiltratorError::Privilege(failure.message))
            },
            Message::PrivilegedNetworkRegressionUpdated,
        )
    }

    fn finish_privileged_network_regression(
        &mut self,
        result: Result<PrivilegedNetworkSnapshot, InfiltratorError>,
    ) -> Task<Message> {
        let copy_locale = self.shell.lang.clone();
        match result {
            Ok(snapshot) => {
                let clean = snapshot.is_clean();
                self.runtime.privileged_network = snapshot;
                if clean {
                    Task::done(Message::ShowToast(
                        Lang(&copy_locale)
                            .tr("privileged_network_verified")
                            .into_owned(),
                        ToastStatus::Success,
                    ))
                } else {
                    Task::none()
                }
            }
            Err(error) => {
                let failure = Failure::new(ErrorCode::Internal, error.to_string(), true);
                self.runtime.privileged_network = PrivilegedNetworkSnapshot::failed(
                    self.runtime.privileged_network.revision.saturating_add(1),
                    failure,
                );
                self.set_error(&error);
                Task::done(Message::ShowToast(error.to_string(), ToastStatus::Error))
            }
        }
    }

    fn apply_lan_sharing(&mut self) -> Task<Message> {
        let copy_locale = self.shell.lang.clone();
        if self.shell.demo {
            return Task::none();
        }
        let Some(runtime) = self.runtime.runtime.clone() else {
            return self.runtime_unavailable(Lang(&copy_locale).tr("runtime_action_lan").as_ref());
        };
        let generation = runtime.lifecycle_port().generation();
        let Some(scope) = self.runtime.core_lifecycle.scope() else {
            let error = InfiltratorError::Config(
                "LAN configuration requires an observed Core scope".to_owned(),
            );
            self.set_error(&error);
            return Task::done(Message::ShowToast(error.to_string(), ToastStatus::Error));
        };
        let desired = self.runtime.settings_model.lan_sharing().clone();
        self.runtime
            .settings_model
            .lan_sharing_write_phase_mut()
            .begin();
        let configuration_ports = runtime.runtime_configuration_ports();
        Task::perform(
            async move {
                RuntimeConfigurationApplication::new(scope, configuration_ports)
                    .set_lan_sharing(desired.enabled, desired.mixed_port, &desired.bind_address)
                    .await
                    .map_err(|failure| InfiltratorError::Config(failure.message))
            },
            move |result| Message::LanSharingSet(result, generation),
        )
    }

    fn apply_lan_security(&mut self) -> Task<Message> {
        let copy_locale = self.shell.lang.clone();
        if self.shell.demo {
            return Task::none();
        }
        let Some(runtime) = self.runtime.runtime.clone() else {
            return self
                .runtime_unavailable(Lang(&copy_locale).tr("runtime_action_lan_access").as_ref());
        };
        let generation = runtime.lifecycle_port().generation();
        let Some(scope) = self.runtime.core_lifecycle.scope() else {
            let error = InfiltratorError::Config(
                "LAN configuration requires an observed Core scope".to_owned(),
            );
            self.set_error(&error);
            return Task::done(Message::ShowToast(error.to_string(), ToastStatus::Error));
        };
        let desired = self.runtime.settings_model.lan_security().clone();
        self.runtime
            .settings_model
            .lan_security_write_phase_mut()
            .begin();
        let allowed_ips = desired.allowed_ips.clone();
        let disallowed_ips = desired.disallowed_ips.clone();
        let skip_auth_prefixes = desired.skip_auth_prefixes.clone();
        let credentials = desired.authentication_enabled.then(|| LanCredentials {
            username: desired.auth_username.clone(),
            password: desired.auth_password.clone(),
        });
        let authentication = match credentials {
            Some(credentials) => LanAuthenticationUpdate::Enabled(credentials),
            None if !desired.authentication_enabled => LanAuthenticationUpdate::Disabled,
            None => {
                let error = InfiltratorError::Config(
                    "LAN authentication requires username and password".to_owned(),
                );
                self.set_error(&error);
                return Task::done(Message::ShowToast(error.to_string(), ToastStatus::Error));
            }
        };
        let configuration_ports = runtime.runtime_configuration_ports();
        Task::perform(
            async move {
                RuntimeConfigurationApplication::new(scope, configuration_ports)
                    .set_lan_security(
                        &allowed_ips,
                        &disallowed_ips,
                        &skip_auth_prefixes,
                        authentication,
                    )
                    .await
                    .map_err(|failure| InfiltratorError::Config(failure.message))
            },
            move |result| Message::LanSecuritySet(result, generation),
        )
    }

    pub(super) fn update_ui_wave5(&mut self, message: Message) -> Task<Message> {
        let copy_locale = self.shell.lang.clone();
        match message {
            Message::RunPrivilegedNetworkRegression => self.run_privileged_network_regression(),
            Message::PrivilegedNetworkRegressionUpdated(result) => {
                self.finish_privileged_network_regression(result)
            }
            Message::RuleStatistics(action) => self.update_rule_statistics(action),
            Message::SelectTunStack(stack) => {
                self.runtime.tun_stack_config.active_stack = stack;
                Task::none()
            }
            Message::ProbeOptimalMtu => {
                if self.shell.demo {
                    let optimal_mtu = 1420u32;
                    self.runtime.mtu = MtuNegotiationSnapshot::ready(
                        self.runtime.mtu.revision.saturating_add(1).max(1),
                        PhysicalMtuSnapshot {
                            interface: "demo-link".to_owned(),
                            mtu: 1500,
                        },
                        optimal_mtu,
                        1380,
                    );
                    self.runtime.tun_stack_config.negotiated_mtu = optimal_mtu;
                    self.runtime.tun_stack_config.probe_result_summary =
                        Some(format!("Optimal MTU: {optimal_mtu} bytes"));
                    return Task::done(Message::MtuProbed(optimal_mtu));
                }
                let Some(runtime) = self.runtime.runtime.clone() else {
                    return self.runtime_unavailable(
                        Lang(&copy_locale).tr("runtime_action_mtu_probe").as_ref(),
                    );
                };
                let Some(port) = runtime.mtu_probe_port() else {
                    let error = InfiltratorError::Privilege(
                        Lang(&copy_locale).tr("mtu_probe_unavailable").into_owned(),
                    );
                    self.set_error(&error);
                    return Task::done(Message::ShowToast(error.to_string(), ToastStatus::Error));
                };
                let application = MtuApplication::new(port);
                let configuration_ports = runtime.runtime_configuration_ports();
                let generation = runtime.lifecycle_port().generation();
                let session_token = self.runtime.core_session_token;
                let Some(scope) = self.runtime.core_lifecycle.scope() else {
                    let failure = Failure::new(
                        ErrorCode::NotReady,
                        "TUN MTU apply requires an observed Core scope",
                        false,
                    );
                    let error = InfiltratorError::Config(failure.message);
                    self.set_error(&error);
                    return Task::done(Message::ShowToast(error.to_string(), ToastStatus::Error));
                };
                self.runtime.mtu = application.probing_snapshot();
                self.runtime.tun_stack_config.is_probing_mtu = true;
                Task::perform(
                    async move {
                        application
                            .probe_and_apply(configuration_ports, scope)
                            .await
                    },
                    move |snapshot| {
                        Message::MtuProbeFinished(MtuProbeCompletion {
                            snapshot,
                            generation,
                            session_token,
                        })
                    },
                )
            }
            Message::MtuProbed(mtu) => {
                self.runtime.tun_stack_config.negotiated_mtu = mtu;
                Task::none()
            }
            Message::MtuProbeFinished(completion) => {
                if completion.generation != self.runtime.runtime_generation
                    || completion.session_token != self.runtime.core_session_token
                {
                    return Task::none();
                }
                let snapshot = completion.snapshot;
                self.runtime.mtu = snapshot.clone();
                self.runtime.tun_stack_config.is_probing_mtu = false;
                match &snapshot.state {
                    MtuProbeState::Ready => {
                        if let (Some(physical), Some(tun), Some(mss)) =
                            (snapshot.physical_mtu, snapshot.tun_mtu, snapshot.tcp_mss)
                        {
                            self.runtime.tun_stack_config.negotiated_mtu = tun;
                            self.runtime.tun_stack_config.probe_result_summary = Some(format!(
                                "{}: physical {physical} → TUN {tun}, TCP MSS {mss}",
                                snapshot
                                    .physical_interface
                                    .as_deref()
                                    .unwrap_or("active link")
                            ));
                        }
                        Task::none()
                    }
                    MtuProbeState::Unsupported => Task::done(Message::ShowToast(
                        Lang(&copy_locale).tr("mtu_probe_unsupported").into_owned(),
                        ToastStatus::Warning,
                    )),
                    MtuProbeState::Failed { failure } => Task::done(Message::ShowToast(
                        localize(
                            &copy_locale,
                            "mtu_probe_failed_notice",
                            &[("reason", failure.message.clone())],
                        ),
                        ToastStatus::Error,
                    )),
                    MtuProbeState::Unknown | MtuProbeState::Probing => Task::none(),
                }
            }
            Message::UnpackRuleProviderToCustom(_)
            | Message::PurgeRuleProviderCache
            | Message::RuleProviderUnpacked(_)
            | Message::RuleProviderCachePurged(_) => self.update_rule_provider(message),
            Message::ToggleLanSharing(on) => {
                self.runtime.settings_model.edit_lan_sharing(|draft| {
                    draft.enabled = on;
                    if draft.mixed_port == 0 {
                        draft.mixed_port = 7890;
                    }
                });
                self.apply_lan_sharing()
            }
            Message::UpdateLanSharingPort(p) => {
                self.runtime
                    .settings_model
                    .edit_lan_sharing(|draft| draft.mixed_port = p);
                Task::none()
            }
            Message::UpdateLanBindAddress(address) => {
                self.runtime
                    .settings_model
                    .edit_lan_sharing(|draft| draft.bind_address = address);
                Task::none()
            }
            Message::UpdateLanAclWhitelist(w) => {
                self.runtime.settings_text.lan_allowed_ips = w.clone();
                let parsed = split_cidrs(&w);
                self.runtime
                    .settings_model
                    .edit_lan_security(|draft| draft.allowed_ips = parsed);
                Task::none()
            }
            Message::ApplyLanSharing => self.apply_lan_sharing(),
            Message::LanSharingSet(result, generation) => {
                if generation != self.runtime.runtime_generation {
                    return Task::none();
                }
                // The owner's answer settles the draft: the value it confirms
                // becomes the draft the page shows, and the write phase
                // settles with it.
                let token = self
                    .runtime
                    .settings_model
                    .lan_sharing_write_phase()
                    .token();
                match result {
                    Ok(snapshot) => {
                        self.runtime.settings_model.discard_lan_sharing();
                        self.runtime.settings_model.observe_lan_sharing(&snapshot);
                        if let Some(token) = token {
                            self.runtime
                                .settings_model
                                .lan_sharing_write_phase_mut()
                                .succeed(token);
                        }
                        Task::done(Message::ShowToast(
                            Lang(&copy_locale).tr("lan_listen_verified").into_owned(),
                            ToastStatus::Success,
                        ))
                    }
                    Err(error) => {
                        if let Some(token) = token {
                            self.runtime
                                .settings_model
                                .lan_sharing_write_phase_mut()
                                .fail(
                                    token,
                                    Failure::new(ErrorCode::Internal, error.to_string(), false),
                                );
                        }
                        self.runtime.settings_model.discard_lan_sharing();
                        self.set_error(&error);
                        Task::done(Message::ShowToast(error.to_string(), ToastStatus::Error))
                    }
                }
            }
            Message::UpdateLanAllowedIps(value) => {
                self.runtime.settings_text.lan_allowed_ips = value.clone();
                let parsed = split_cidrs(&value);
                self.runtime
                    .settings_model
                    .edit_lan_security(|draft| draft.allowed_ips = parsed);
                Task::none()
            }
            Message::UpdateLanDisallowedIps(value) => {
                self.runtime.settings_text.lan_disallowed_ips = value.clone();
                let parsed = split_cidrs(&value);
                self.runtime
                    .settings_model
                    .edit_lan_security(|draft| draft.disallowed_ips = parsed);
                Task::none()
            }
            Message::UpdateLanSkipAuthPrefixes(value) => {
                self.runtime.settings_text.lan_skip_auth_prefixes = value.clone();
                let parsed = split_cidrs(&value);
                self.runtime
                    .settings_model
                    .edit_lan_security(|draft| draft.skip_auth_prefixes = parsed);
                Task::none()
            }
            Message::ToggleLanAuthentication(enabled) => {
                self.runtime
                    .settings_model
                    .edit_lan_security(|draft| draft.authentication_enabled = enabled);
                Task::none()
            }
            Message::UpdateLanAuthUsername(value) => {
                self.runtime
                    .settings_model
                    .edit_lan_security(|draft| draft.auth_username = value);
                Task::none()
            }
            Message::UpdateLanAuthPassword(value) => {
                self.runtime
                    .settings_model
                    .edit_lan_security(|draft| draft.auth_password = value);
                Task::none()
            }
            Message::ApplyLanSecurity => self.apply_lan_security(),
            Message::LanSecuritySet(result, generation) => {
                if generation != self.runtime.runtime_generation {
                    return Task::none();
                }
                // The owner's answer settles the draft: the ACL it confirms
                // becomes the draft the page shows, the password it never
                // published is dropped, and the write phase settles with it.
                let token = self
                    .runtime
                    .settings_model
                    .lan_security_write_phase()
                    .token();
                match result {
                    Ok(snapshot) => {
                        self.runtime.settings_model.discard_lan_security();
                        self.runtime.settings_model.observe_lan_security(&snapshot);
                        self.runtime.settings_model.accept_lan_security();
                        if let Some(token) = token {
                            self.runtime
                                .settings_model
                                .lan_security_write_phase_mut()
                                .succeed(token);
                        }
                        self.runtime.settings_text.lan_allowed_ips =
                            snapshot.allowed_ips.join(", ");
                        self.runtime.settings_text.lan_disallowed_ips =
                            snapshot.disallowed_ips.join(", ");
                        self.runtime.settings_text.lan_skip_auth_prefixes =
                            snapshot.skip_auth_prefixes.join(", ");
                        Task::done(Message::ShowToast(
                            Lang(&copy_locale).tr("lan_access_verified").into_owned(),
                            ToastStatus::Success,
                        ))
                    }
                    Err(error) => {
                        if let Some(token) = token {
                            self.runtime
                                .settings_model
                                .lan_security_write_phase_mut()
                                .fail(
                                    token,
                                    Failure::new(ErrorCode::Internal, error.to_string(), false),
                                );
                        }
                        self.runtime.settings_model.discard_lan_security();
                        self.set_error(&error);
                        Task::done(Message::ShowToast(error.to_string(), ToastStatus::Error))
                    }
                }
            }
            _ => Task::none(),
        }
    }
}

fn split_cidrs(value: &str) -> Vec<String> {
    value
        .split([',', ';', '\n'])
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
        .collect()
}
