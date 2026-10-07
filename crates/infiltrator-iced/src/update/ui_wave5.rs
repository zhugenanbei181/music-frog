//! Wave 5 Advanced feature message handlers: Rule Hit Counter, Latency Radar,
//! TUN Multi-Stack, Rule Unpacker, Atomic Apply Guard, and LAN Proxy Sharing.

use crate::state::AppState;
use crate::types::app::ToastStatus;
use crate::types::message::{Message, MtuProbeCompletion};
use iced::Task;
use infiltrator_application::mtu_application::MtuApplication;
use infiltrator_application::privileged_network_application::PrivilegedNetworkApplication;
use infiltrator_application::runtime_query_application::RuntimeQueryApplication;
use infiltrator_contract::error::{ErrorCode, Failure, InfiltratorError};
use infiltrator_contract::lan::LanCredentials;
use infiltrator_contract::mtu::{MtuNegotiationSnapshot, MtuProbeState, PhysicalMtuSnapshot};
use infiltrator_contract::privileged_network::{
    PrivilegedNetworkRequest, PrivilegedNetworkSnapshot, PrivilegedNetworkState,
};
use infiltrator_ports::runtime_gateway::RuntimeGateway;
use infiltrator_shared::i18n_interpolator::localize;
use infiltrator_shared::locales::{Lang, Localizer};
use std::sync::Arc;

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
        let generation = runtime.generation();
        let desired = self.runtime.lan_sharing.clone();
        let gateway: Arc<dyn RuntimeGateway> = runtime;
        Task::perform(
            async move {
                RuntimeQueryApplication::new(gateway)
                    .set_lan_sharing(desired.allow_lan, desired.mixed_port, &desired.bind_address)
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
        let generation = runtime.generation();
        let desired = self.runtime.lan_security.clone();
        let allowed_ips = split_cidrs(&desired.allowed_ips);
        let disallowed_ips = split_cidrs(&desired.disallowed_ips);
        let skip_auth_prefixes = split_cidrs(&desired.skip_auth_prefixes);
        let credentials = desired.authentication_enabled.then(|| LanCredentials {
            username: desired.auth_username.clone(),
            password: desired.auth_password.clone(),
        });
        let gateway: Arc<dyn RuntimeGateway> = runtime;
        Task::perform(
            async move {
                RuntimeQueryApplication::new(gateway)
                    .set_lan_security(
                        &allowed_ips,
                        &disallowed_ips,
                        &skip_auth_prefixes,
                        desired.authentication_enabled,
                        credentials.as_ref(),
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
                let gateway: Arc<dyn RuntimeGateway> = runtime.clone();
                let generation = runtime.generation();
                let session_token = self.runtime.core_session_token;
                self.runtime.mtu = application.probing_snapshot();
                self.runtime.tun_stack_config.is_probing_mtu = true;
                Task::perform(
                    async move { application.probe_and_apply(gateway).await },
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
                self.runtime.lan_sharing.allow_lan = on;
                if self.runtime.lan_sharing.mixed_port == 0 {
                    self.runtime.lan_sharing.mixed_port = 7890;
                }
                self.runtime.lan_sharing_dirty = true;
                self.apply_lan_sharing()
            }
            Message::UpdateLanSharingPort(p) => {
                self.runtime.lan_sharing.mixed_port = p;
                self.runtime.lan_sharing_dirty = true;
                Task::none()
            }
            Message::UpdateLanBindAddress(address) => {
                self.runtime.lan_sharing.bind_address = address;
                self.runtime.lan_sharing_dirty = true;
                Task::none()
            }
            Message::UpdateLanAclWhitelist(w) => {
                self.runtime.lan_sharing.acl_whitelist_cidrs = w.clone();
                self.runtime.lan_sharing_dirty = true;
                self.runtime.lan_security.allowed_ips = w;
                self.runtime.lan_security_dirty = true;
                Task::none()
            }
            Message::ApplyLanSharing => self.apply_lan_sharing(),
            Message::LanSharingSet(result, generation) => {
                if generation != self.runtime.runtime_generation {
                    return Task::none();
                }
                match result {
                    Ok(snapshot) => {
                        self.runtime.lan_sharing.allow_lan = snapshot.enabled;
                        self.runtime.lan_sharing.mixed_port = snapshot.mixed_port;
                        self.runtime.lan_sharing.bind_address = snapshot.bind_address;
                        self.runtime.lan_sharing_committed = self.runtime.lan_sharing.clone();
                        self.runtime.lan_sharing_dirty = false;
                        Task::done(Message::ShowToast(
                            Lang(&copy_locale).tr("lan_listen_verified").into_owned(),
                            ToastStatus::Success,
                        ))
                    }
                    Err(error) => {
                        self.runtime.lan_sharing = self.runtime.lan_sharing_committed.clone();
                        self.runtime.lan_sharing_dirty = false;
                        self.set_error(&error);
                        Task::done(Message::ShowToast(error.to_string(), ToastStatus::Error))
                    }
                }
            }
            Message::UpdateLanAllowedIps(value) => {
                self.runtime.lan_security.allowed_ips = value;
                self.runtime.lan_security_dirty = true;
                Task::none()
            }
            Message::UpdateLanDisallowedIps(value) => {
                self.runtime.lan_security.disallowed_ips = value;
                self.runtime.lan_security_dirty = true;
                Task::none()
            }
            Message::UpdateLanSkipAuthPrefixes(value) => {
                self.runtime.lan_security.skip_auth_prefixes = value;
                self.runtime.lan_security_dirty = true;
                Task::none()
            }
            Message::ToggleLanAuthentication(enabled) => {
                self.runtime.lan_security.authentication_enabled = enabled;
                self.runtime.lan_security_dirty = true;
                Task::none()
            }
            Message::UpdateLanAuthUsername(value) => {
                self.runtime.lan_security.auth_username = value;
                self.runtime.lan_security_dirty = true;
                Task::none()
            }
            Message::UpdateLanAuthPassword(value) => {
                self.runtime.lan_security.auth_password = value;
                self.runtime.lan_security_dirty = true;
                Task::none()
            }
            Message::ApplyLanSecurity => self.apply_lan_security(),
            Message::LanSecuritySet(result, generation) => {
                if generation != self.runtime.runtime_generation {
                    return Task::none();
                }
                match result {
                    Ok(snapshot) => {
                        self.runtime.lan_security.allowed_ips = snapshot.allowed_ips.join(", ");
                        self.runtime.lan_security.disallowed_ips =
                            snapshot.disallowed_ips.join(", ");
                        self.runtime.lan_security.skip_auth_prefixes =
                            snapshot.skip_auth_prefixes.join(", ");
                        self.runtime.lan_security.authentication_enabled =
                            snapshot.authentication_enabled;
                        self.runtime.lan_security.authentication_user_count =
                            snapshot.authentication_user_count;
                        if let Some(username) = snapshot.authentication_username {
                            self.runtime.lan_security.auth_username = username;
                        }
                        self.runtime.lan_security.auth_password.clear();
                        self.runtime.lan_security_committed = self.runtime.lan_security.clone();
                        self.runtime.lan_security_dirty = false;
                        self.runtime.lan_sharing.acl_whitelist_cidrs =
                            self.runtime.lan_security.allowed_ips.clone();
                        Task::done(Message::ShowToast(
                            Lang(&copy_locale).tr("lan_access_verified").into_owned(),
                            ToastStatus::Success,
                        ))
                    }
                    Err(error) => {
                        self.runtime.lan_security = self.runtime.lan_security_committed.clone();
                        self.runtime.lan_security_dirty = false;
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
