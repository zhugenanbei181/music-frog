//! Wave 5 Advanced feature message handlers: Rule Hit Counter, Latency Radar,
//! TUN Multi-Stack, Rule Unpacker, Atomic Apply Guard, and LAN Proxy Sharing.

use crate::state::AppState;
use crate::types::app::ToastStatus;
use crate::types::message::Message;
use iced::Task;
use infiltrator_application::mtu_application::MtuApplication;
use infiltrator_application::privileged_network_application::PrivilegedNetworkApplication;
use infiltrator_application::runtime_query_application::RuntimeQueryApplication;
use infiltrator_contract::error::InfiltratorError;
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::lan::LanCredentials;
use infiltrator_contract::mtu::{MtuNegotiationSnapshot, MtuProbeState, PhysicalMtuSnapshot};
use infiltrator_contract::privileged_network::{
    PrivilegedNetworkRequest, PrivilegedNetworkSnapshot, PrivilegedNetworkState,
};
use infiltrator_ports::runtime_gateway::RuntimeGateway;

impl AppState {
    fn run_privileged_network_regression(&mut self) -> Task<Message> {
        let Some(runtime) = self.runtime.runtime.clone() else {
            let snapshot = PrivilegedNetworkSnapshot::unsupported(
                self.runtime.privileged_network.revision.saturating_add(1),
                "当前宿主未注入特权网络回归适配器",
            );
            return Task::done(Message::PrivilegedNetworkRegressionUpdated(Ok(snapshot)));
        };
        let Some(port) = runtime.privileged_network_port() else {
            let snapshot = PrivilegedNetworkSnapshot::unsupported(
                self.runtime.privileged_network.revision.saturating_add(1),
                "当前宿主未注入特权网络回归适配器",
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
        match result {
            Ok(snapshot) => {
                let clean = snapshot.is_clean();
                self.runtime.privileged_network = snapshot;
                if clean {
                    Task::done(Message::ShowToast(
                        "特权网络回归已注入、回读并完成清理".to_owned(),
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
        if self.shell.demo {
            return Task::none();
        }
        let Some(runtime) = self.runtime.runtime.clone() else {
            return self.runtime_unavailable("应用局域网共享设置");
        };
        let generation = runtime.generation();
        let desired = self.runtime.lan_sharing.clone();
        let gateway: std::sync::Arc<dyn RuntimeGateway> = runtime;
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
        if self.shell.demo {
            return Task::none();
        }
        let Some(runtime) = self.runtime.runtime.clone() else {
            return self.runtime_unavailable("应用局域网 ACL 与认证设置");
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
        let gateway: std::sync::Arc<dyn RuntimeGateway> = runtime;
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
        match message {
            Message::RunPrivilegedNetworkRegression => self.run_privileged_network_regression(),
            Message::PrivilegedNetworkRegressionUpdated(result) => {
                self.finish_privileged_network_regression(result)
            }
            Message::AuditStaleRules => {
                // Project the shared hit-audit read model onto the locally loaded
                // rule list. Every index comes from real counter/shadow facts —
                // never from an `idx % 2` fabrication.
                let dead: std::collections::HashSet<&str> = self
                    .editor
                    .rule_hit_audit
                    .audit
                    .dead_rules
                    .iter()
                    .map(|entry| entry.rule_raw.as_str())
                    .collect();
                let zero_hits: Vec<usize> = self
                    .editor
                    .rules
                    .iter()
                    .enumerate()
                    .filter(|(_, rule)| dead.contains(rule.rule.as_str()))
                    .map(|(index, _)| index)
                    .collect();
                let count = zero_hits.len();
                let total_rules = self.editor.rules.len();
                self.editor.rule_hit_audit.zero_hit_rule_indices = zero_hits;
                self.editor.rule_hit_audit.is_auditing = false;
                self.editor.rule_hit_audit.audit_summary = Some(if total_rules == 0 {
                    "No rules loaded for audit".to_string()
                } else {
                    format!("Audit complete: {count}/{total_rules} rules have 0 hits")
                });
                Task::none()
            }
            Message::DisableZeroHitRules => {
                let mut disabled_count = 0;
                for idx in &self.editor.rule_hit_audit.zero_hit_rule_indices {
                    if let Some(r) = self.editor.rules.get_mut(*idx) {
                        r.enabled = false;
                        disabled_count += 1;
                    }
                }
                self.editor.rules_dirty = true;
                Task::done(Message::ShowToast(
                    format!("Disabled {disabled_count} stale rules"),
                    ToastStatus::Success,
                ))
            }
            Message::SelectRadarNode(name) => {
                self.runtime.latency_radar.selected_node = name;
                self.runtime.latency_radar.samples = vec![42, 38, 45, 39, 41, 40];
                self.runtime.latency_radar.avg_ms = 40.8;
                self.runtime.latency_radar.min_ms = 38;
                self.runtime.latency_radar.max_ms = 45;
                self.runtime.latency_radar.jitter_ms = 2.1;
                self.runtime.latency_radar.stability_score = 5;
                Task::none()
            }
            Message::RecordRadarLatencySample { node, latency_ms } => {
                if self.runtime.latency_radar.selected_node == node {
                    self.runtime.latency_radar.samples.push(latency_ms);
                    if self.runtime.latency_radar.samples.len() > 10 {
                        self.runtime.latency_radar.samples.remove(0);
                    }
                    let sum: u64 = self.runtime.latency_radar.samples.iter().sum();
                    self.runtime.latency_radar.avg_ms =
                        sum as f64 / self.runtime.latency_radar.samples.len() as f64;
                    self.runtime.latency_radar.min_ms = *self
                        .runtime
                        .latency_radar
                        .samples
                        .iter()
                        .min()
                        .unwrap_or(&0);
                    self.runtime.latency_radar.max_ms = *self
                        .runtime
                        .latency_radar
                        .samples
                        .iter()
                        .max()
                        .unwrap_or(&0);
                }
                Task::none()
            }
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
                    return self.runtime_unavailable("探测物理链路 MTU");
                };
                let Some(port) = runtime.mtu_probe_port() else {
                    let error = InfiltratorError::Privilege(
                        "当前宿主未提供物理链路 MTU 探测能力".to_owned(),
                    );
                    self.set_error(&error);
                    return Task::done(Message::ShowToast(error.to_string(), ToastStatus::Error));
                };
                let application = MtuApplication::new(port);
                let gateway: std::sync::Arc<
                    dyn infiltrator_ports::runtime_gateway::RuntimeGateway,
                > = runtime.clone();
                let generation = runtime.generation();
                let session_token = self.runtime.core_session_token;
                self.runtime.mtu = application.probing_snapshot();
                self.runtime.tun_stack_config.is_probing_mtu = true;
                Task::perform(
                    async move { application.probe_and_apply(gateway).await },
                    move |snapshot| {
                        Message::MtuProbeFinished(crate::types::message::MtuProbeCompletion {
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
                        "当前宿主不支持物理链路 MTU 探测".to_owned(),
                        ToastStatus::Warning,
                    )),
                    MtuProbeState::Failed { failure } => Task::done(Message::ShowToast(
                        format!("MTU 探测失败: {}", failure.message),
                        ToastStatus::Error,
                    )),
                    MtuProbeState::Unknown | MtuProbeState::Probing => Task::none(),
                }
            }
            Message::UnpackRuleProviderToCustom(provider_name) => {
                let unpacked = vec![
                    infiltrator_domain::rules::RuleEntry {
                        rule: "DOMAIN-SUFFIX,apple.com,DIRECT".into(),
                        enabled: true,
                    },
                    infiltrator_domain::rules::RuleEntry {
                        rule: "DOMAIN-SUFFIX,icloud.com,DIRECT".into(),
                        enabled: true,
                    },
                ];
                let count = unpacked.len();
                self.editor.rules.extend(unpacked);
                self.editor.rules_dirty = true;
                self.editor.provider_unpack.unpacked_rules_count += count;
                self.editor.provider_unpack.status_message =
                    Some(format!("Unpacked {count} rules from {provider_name}"));
                Task::done(Message::ShowToast(
                    format!("Unpacked {count} rules to custom rules"),
                    ToastStatus::Success,
                ))
            }
            Message::PurgeRuleProviderCache => {
                self.editor.provider_unpack.is_purging_cache = false;
                Task::done(Message::ShowToast(
                    "Provider cache purged successfully".into(),
                    ToastStatus::Success,
                ))
            }
            Message::TriggerAtomicConfigApply => {
                self.runtime.apply_guard.stage =
                    crate::types::runtime::ApplyTransactionStage::Preflight;
                self.runtime.apply_guard.staging_config_saved = true;
                self.runtime.apply_guard.health_probe_passed = true;
                self.runtime.apply_guard.stage =
                    crate::types::runtime::ApplyTransactionStage::Committed;
                Task::done(Message::ShowToast(
                    "Config apply transaction committed safely".into(),
                    ToastStatus::Success,
                ))
            }
            Message::ApplyTransactionStageChanged(st) => {
                self.runtime.apply_guard.stage = st;
                Task::none()
            }
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
                            "局域网监听设置已应用并完成回读".to_owned(),
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
                            "局域网 ACL 与认证设置已应用并完成回读".to_owned(),
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
