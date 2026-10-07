//! Live runtime configuration: querying the running mihomo for its active
//! config and patching mode/TUN/sniffer toggles through the REST API.

use crate::port_conflict_application::application;
use crate::state::AppState;
use crate::types::app::ToastStatus;
use crate::types::message::Message;
use crate::types::runtime::RuntimePatchSnapshot;
use iced::Task;
use infiltrator_application::runtime_control_projection::{
    ipv6_observation, lan_security_observation, runtime_control,
};
use infiltrator_application::runtime_query_application::RuntimeQueryApplication;
use infiltrator_application::service_mode_application::ServiceModeApplication;
use infiltrator_application::system_toggle_application::SystemToggleApplication;
use infiltrator_contract::error::{Failure, InfiltratorError};
use infiltrator_contract::proxy_mode::{ProxyModeSnapshot, ProxyModeStatus};
use infiltrator_contract::runtime_control::RuntimeControlStatus;
use infiltrator_contract::service_mode::{ServiceModeSnapshot, ServiceModeState};
use infiltrator_contract::system_toggle::{SystemToggle, SystemToggleState};
use infiltrator_contract::tun::TunStack;
use infiltrator_desktop::tun_service::TunServiceManager;
use infiltrator_ports::host_runtime::TunServiceStatus;
use infiltrator_ports::runtime_gateway::RuntimeGateway;
use infiltrator_shared::i18n_interpolator::localize;
use infiltrator_shared::locales::{Lang, Localizer};
use std::sync::Arc;
use tokio::task::spawn_blocking;

impl AppState {
    pub(crate) fn runtime_unavailable(&mut self, operation: &str) -> Task<Message> {
        let copy_locale = self.shell.lang.clone();
        let error = InfiltratorError::Internal(localize(
            &copy_locale,
            "runtime_action_requires_core",
            &[("operation", operation.to_owned())],
        ));
        self.set_error(&error);
        Task::done(Message::ShowToast(error.to_string(), ToastStatus::Error))
    }

    fn begin_runtime_patch(&mut self) -> Option<u64> {
        if self.runtime.pending_runtime_patch.is_some()
            || self.runtime.mode_actions.pending.is_some()
        {
            return None;
        }
        if self.runtime.runtime.is_some() {
            if self.runtime.pending_runtime_patch.is_none() {
                self.runtime.pending_runtime_patch = Some(RuntimePatchSnapshot {
                    proxy_mode: self.runtime.proxy_mode.clone(),
                    proxy_mode_state: self.runtime.proxy_mode_state.clone(),
                    ipv6_enabled: self.runtime.ipv6_routing.enabled,
                    tun_enabled: self.runtime.tun_enabled,
                    tun_stack: self.editor.tun_stack.clone(),
                    tun_stack_selector: self.runtime.tun_stack_config.active_stack.clone(),
                    tun_auto_route: self.editor.tun_auto_route,
                    tun_strict_route: self.editor.tun_strict_route,
                    sniffer_enabled: self.editor.sniffer_enabled,
                });
            }
            self.runtime.runtime_patch_token = self.runtime.runtime_patch_token.wrapping_add(1);
        }
        Some(self.runtime.runtime_patch_token)
    }

    fn restore_runtime_patch(&mut self) {
        if let Some(previous) = self.runtime.pending_runtime_patch.take() {
            self.runtime.proxy_mode = previous.proxy_mode;
            self.runtime.proxy_mode_state = previous.proxy_mode_state;
            self.runtime.ipv6_routing.enabled = previous.ipv6_enabled;
            self.runtime.tun_enabled = previous.tun_enabled;
            self.runtime.system_toggles = self
                .runtime
                .system_toggles
                .clone()
                .with_tun_readback(previous.tun_enabled);
            self.editor.tun_stack = previous.tun_stack;
            self.runtime.tun_stack_config.active_stack = previous.tun_stack_selector;
            self.editor.tun_auto_route = previous.tun_auto_route;
            self.editor.tun_strict_route = previous.tun_strict_route;
            self.editor.sniffer_enabled = previous.sniffer_enabled;
            self.refresh_tray();
        }
    }

    fn patch_tun_enabled(&mut self, enabled: bool) -> Task<Message> {
        let copy_locale = self.shell.lang.clone();
        let Some(rt) = self.runtime.runtime.clone() else {
            if enabled {
                let error = InfiltratorError::Privilege(
                    Lang(&copy_locale)
                        .tr("tun_start_requires_core")
                        .into_owned(),
                );
                self.set_error(&error);
                return Task::done(Message::ShowToast(error.to_string(), ToastStatus::Error));
            }
            return Task::none();
        };
        let Some(token) = self.begin_runtime_patch() else {
            return Task::none();
        };
        let generation = rt.generation();
        self.runtime.tun_enabled = Some(enabled);
        self.runtime.system_toggles = self
            .runtime
            .system_toggles
            .clone()
            .with_legacy_tun(Some(enabled))
            .with_pending(SystemToggle::Tun, enabled);
        self.refresh_tray();
        let gateway: Arc<dyn RuntimeGateway> = rt.clone();
        Task::perform(
            async move {
                RuntimeQueryApplication::new(gateway)
                    .set_tun_enabled(enabled)
                    .await
                    .map_err(|failure| InfiltratorError::Config(failure.message))
            },
            move |result| Message::RuntimePatchResult(result, token, generation),
        )
    }

    pub(crate) fn install_or_start_tun_service(
        &mut self,
        status: TunServiceStatus,
    ) -> Task<Message> {
        let copy_locale = self.shell.lang.clone();
        let Some(runtime) = self.runtime.runtime.clone() else {
            return Task::done(Message::ShowToast(
                Lang(&copy_locale)
                    .tr("tun_prepare_requires_core")
                    .into_owned(),
                ToastStatus::Error,
            ));
        };
        self.shell.error_msg = None;
        self.editor.is_saving_tun = false;
        let binary = runtime.core_binary_path();
        self.runtime.tun_service_status = Some(status);
        self.runtime.service_mode = legacy_service_snapshot(status);
        self.runtime.is_installing_tun_service = true;
        if let Some(service_mode) = runtime.service_mode_port() {
            return Task::perform(
                async move {
                    ServiceModeApplication::new(service_mode)
                        .prepare()
                        .await
                        .map_err(|failure| InfiltratorError::Privilege(failure.message))
                },
                Message::ServiceModePrepared,
            );
        }
        Task::perform(
            async move {
                spawn_blocking(move || match status {
                    TunServiceStatus::InstalledStopped => TunServiceManager::start_service(),
                    TunServiceStatus::NotInstalled | TunServiceStatus::MissingPrivilege => {
                        TunServiceManager::install_service(&binary)
                    }
                    TunServiceStatus::InstalledAndRunning | TunServiceStatus::Unsupported => Ok(()),
                })
                .await
                .map_err(|error| InfiltratorError::Privilege(error.to_string()))?
                .map_err(|error| InfiltratorError::Privilege(error.to_string()))
            },
            Message::TunServiceInstalled,
        )
    }

    /// Runtime config fetch and live patch toggles. Unmatched messages fall
    /// through to the next domain in the `update_core` chain.
    pub(super) fn update_core_runtime_config(&mut self, message: Message) -> Task<Message> {
        let copy_locale = self.shell.lang.clone();
        match message {
            Message::FetchRuntimeConfig => {
                if let Some(rt) = self.runtime.runtime.clone() {
                    let generation = rt.generation();
                    let mode_epoch = self.runtime.mode_actions.read_epoch();
                    Task::perform(
                        async move { rt.get_config().await.map_err(Failure::from) },
                        move |result| Message::RuntimeConfigReadFinished {
                            result,
                            generation,
                            mode_epoch,
                        },
                    )
                } else {
                    Task::none()
                }
            }
            Message::RuntimeConfigReadFinished {
                result,
                generation,
                mode_epoch,
            } => {
                if !self.runtime.mode_actions.accepts_read(mode_epoch) {
                    return Task::none();
                }
                self.update_core(Message::RuntimeConfigFetched(result, generation))
            }
            Message::RuntimeConfigFetched(result, generation) => {
                if generation != self.runtime.runtime_generation {
                    return Task::none();
                }
                match result {
                    Ok(config) => {
                        let observed = runtime_control(Some(&Ok(config.clone())));
                        self.runtime.runtime_control = observed.clone();
                        self.runtime.proxy_mode_state = ProxyModeSnapshot {
                            current: observed.mode,
                            script_available: observed.script_available,
                            status: if observed.mode.is_some() {
                                ProxyModeStatus::Ready
                            } else {
                                ProxyModeStatus::Unobserved
                            },
                            failure: None,
                        };
                        self.runtime.mode_read_revision = self
                            .runtime
                            .mode_actions
                            .next_observation_revision()
                            .max(self.surface.revision().saturating_add(1));
                        let revision = self.runtime.mode_read_revision;
                        self.runtime.mode_actions.observe(
                            generation,
                            revision,
                            self.runtime.proxy_mode_state.clone(),
                        );
                        self.runtime.proxy_mode_state = self.runtime.mode_actions.render_snapshot();
                        self.runtime.proxy_mode = self
                            .runtime
                            .proxy_mode_state
                            .current
                            .map(|mode| mode.to_wire().to_owned());
                        if let Some(ipv6) = ipv6_observation(&config) {
                            self.runtime.ipv6_routing = ipv6;
                        }
                        let mut lan_committed = self.runtime.lan_sharing_committed.clone();
                        lan_committed.allow_lan = config.allow_lan;
                        lan_committed.mixed_port = config.mixed_port;
                        if let Some(bind_address) = &config.bind_address {
                            lan_committed.bind_address = bind_address.clone();
                        }
                        self.runtime.lan_sharing_committed = lan_committed.clone();
                        if !self.runtime.lan_sharing_dirty {
                            self.runtime.lan_sharing = lan_committed;
                        }
                        if let Some(security) = lan_security_observation(&config) {
                            let mut committed = self.runtime.lan_security_committed.clone();
                            committed.allowed_ips = security.allowed_ips.join(", ");
                            committed.disallowed_ips = security.disallowed_ips.join(", ");
                            committed.skip_auth_prefixes = security.skip_auth_prefixes.join(", ");
                            committed.authentication_enabled = security.authentication_enabled;
                            committed.authentication_user_count =
                                security.authentication_user_count;
                            committed.auth_username =
                                security.authentication_username.unwrap_or_default();
                            committed.auth_password.clear();
                            self.runtime.lan_security_committed = committed.clone();
                            if !self.runtime.lan_security_dirty {
                                self.runtime.lan_security = committed;
                                self.runtime.lan_sharing.acl_whitelist_cidrs =
                                    self.runtime.lan_security.allowed_ips.clone();
                            }
                        }
                        self.runtime.script_block_present = observed.script_available == Some(true);
                        self.runtime.tun_enabled = observed.tun_enabled;
                        self.runtime.system_toggles = self
                            .runtime
                            .system_toggles
                            .clone()
                            .with_tun_readback(observed.tun_enabled);
                        if let Some(dns) = config.dns {
                            self.editor.dns_nameservers = dns.nameserver;
                            self.editor.dns_fallback_servers = dns.fallback;
                            self.editor.dns_enhanced_mode = dns.enhanced_mode;
                        }
                        if let Some(tun) = config.tun {
                            if let Some(stack) = tun.stack {
                                self.editor.tun_stack = stack;
                            }
                            if let Some(auto_route) = tun.auto_route {
                                self.editor.tun_auto_route = auto_route;
                            }
                            if let Some(strict_route) = tun.strict_route {
                                self.editor.tun_strict_route = strict_route;
                            }
                        }
                        if let Some(sniffer) = config.sniffer {
                            self.editor.sniffer_enabled = sniffer.enable;
                        }
                        self.refresh_tray();
                    }
                    Err(failure) => {
                        self.runtime.runtime_control.status = RuntimeControlStatus::Failed {
                            failure: failure.clone(),
                        };
                        self.runtime.system_toggles.tun = SystemToggleState::Failed {
                            failure: failure.clone(),
                        };
                        self.runtime.proxy_mode_state.status = ProxyModeStatus::Failed;
                        self.runtime.proxy_mode_state.failure = Some(failure.clone());
                        self.runtime.mode_read_revision = self
                            .runtime
                            .mode_actions
                            .next_observation_revision()
                            .max(self.surface.revision().saturating_add(1));
                        self.runtime.mode_actions.observe(
                            generation,
                            self.runtime.mode_read_revision,
                            self.runtime.proxy_mode_state.clone(),
                        );
                        self.runtime.proxy_mode_state = self.runtime.mode_actions.render_snapshot();
                        self.shell.error_msg = Some(failure.message);
                    }
                }
                Task::none()
            }
            Message::SetIpv6Routing(enabled) => {
                let Some(rt) = self.runtime.runtime.clone() else {
                    return self.runtime_unavailable(
                        Lang(&copy_locale).tr("runtime_action_ipv6").as_ref(),
                    );
                };
                let Some(token) = self.begin_runtime_patch() else {
                    return Task::none();
                };
                let generation = rt.generation();
                self.runtime.ipv6_routing.enabled = enabled;
                let gateway: Arc<dyn RuntimeGateway> = rt;
                Task::perform(
                    async move {
                        RuntimeQueryApplication::new(gateway)
                            .set_ipv6_routing(enabled)
                            .await
                            .map(|_| ())
                            .map_err(|failure| InfiltratorError::Config(failure.message))
                    },
                    move |result| Message::RuntimePatchResult(result, token, generation),
                )
            }
            Message::SetTunEnabled(enabled) => {
                if self.runtime.runtime.is_none() {
                    return self
                        .runtime_unavailable(Lang(&copy_locale).tr("runtime_action_tun").as_ref());
                }
                let toggle_snapshot = self
                    .runtime
                    .system_toggles
                    .clone()
                    .with_legacy_tun(self.runtime.tun_enabled);
                if let Err(failure) =
                    SystemToggleApplication::intent(&toggle_snapshot, SystemToggle::Tun, enabled)
                {
                    let error = InfiltratorError::Privilege(failure.message);
                    self.set_error(&error);
                    return Task::done(Message::ShowToast(error.to_string(), ToastStatus::Error));
                }
                self.runtime.system_toggles = toggle_snapshot;
                if enabled && let Some(runtime) = self.runtime.runtime.clone() {
                    let status = runtime.tun_service_status();
                    self.runtime.tun_service_status = Some(status);
                    match status {
                        TunServiceStatus::InstalledAndRunning => {}
                        TunServiceStatus::InstalledStopped
                        | TunServiceStatus::NotInstalled
                        | TunServiceStatus::MissingPrivilege => {
                            return self.install_or_start_tun_service(status);
                        }
                        TunServiceStatus::Unsupported => {
                            let error = InfiltratorError::Privilege(
                                Lang(&copy_locale)
                                    .tr("tun_service_unavailable")
                                    .into_owned(),
                            );
                            self.set_error(&error);
                            return Task::done(Message::ShowToast(
                                error.to_string(),
                                ToastStatus::Error,
                            ));
                        }
                    }
                }
                self.patch_tun_enabled(enabled)
            }
            Message::InstallTunService => {
                let Some(runtime) = self.runtime.runtime.clone() else {
                    let error = InfiltratorError::Privilege(
                        Lang(&copy_locale)
                            .tr("tun_prepare_requires_core")
                            .into_owned(),
                    );
                    self.set_error(&error);
                    return Task::done(Message::ShowToast(error.to_string(), ToastStatus::Error));
                };
                let status = runtime.tun_service_status();
                self.runtime.tun_service_status = Some(status);
                match status {
                    TunServiceStatus::InstalledAndRunning => Task::done(Message::ShowToast(
                        Lang(&copy_locale).tr("tun_service_ready").into_owned(),
                        ToastStatus::Success,
                    )),
                    TunServiceStatus::InstalledStopped
                    | TunServiceStatus::NotInstalled
                    | TunServiceStatus::MissingPrivilege => {
                        self.install_or_start_tun_service(status)
                    }
                    TunServiceStatus::Unsupported => {
                        let error = InfiltratorError::Privilege(
                            Lang(&copy_locale)
                                .tr("tun_service_unavailable")
                                .into_owned(),
                        );
                        self.set_error(&error);
                        Task::done(Message::ShowToast(error.to_string(), ToastStatus::Error))
                    }
                }
            }
            Message::RefreshTunServiceStatus => {
                let Some(runtime) = self.runtime.runtime.clone() else {
                    self.runtime.tun_service_status = None;
                    return Task::none();
                };
                let status_runtime = runtime.clone();
                Task::perform(
                    spawn_blocking(move || status_runtime.tun_service_status()),
                    |result| match result {
                        Ok(status) => Message::TunServiceStatusLoaded(Ok(status)),
                        Err(error) => Message::TunServiceStatusLoaded(Err(
                            InfiltratorError::Privilege(error.to_string()),
                        )),
                    },
                )
            }
            Message::TunServiceStatusLoaded(result) => {
                match result {
                    Ok(status) => self.runtime.tun_service_status = Some(status),
                    Err(error) => self.set_error(&error),
                }
                Task::none()
            }
            Message::TunServiceInstalled(result) => {
                self.runtime.is_installing_tun_service = false;
                match result {
                    Ok(()) => {
                        self.runtime.tun_service_status = self
                            .runtime
                            .runtime
                            .as_ref()
                            .map(|runtime| runtime.tun_service_status());
                        self.patch_tun_enabled(true)
                    }
                    Err(error) => {
                        self.set_error(&error);
                        Task::done(Message::ShowToast(error.to_string(), ToastStatus::Error))
                    }
                }
            }
            Message::ServiceModePrepared(result) => {
                self.runtime.is_installing_tun_service = false;
                match result {
                    Ok(snapshot) => {
                        self.runtime.service_mode = snapshot;
                        self.runtime.tun_service_status = Some(tun_status(snapshot.state));
                        self.patch_tun_enabled(true)
                    }
                    Err(error) => {
                        self.set_error(&error);
                        Task::done(Message::ShowToast(error.to_string(), ToastStatus::Error))
                    }
                }
            }
            Message::RepairPortConflicts => {
                let application = match application() {
                    Ok(application) => application,
                    Err(error) => {
                        self.set_error(&error);
                        return Task::done(Message::ShowToast(
                            error.to_string(),
                            ToastStatus::Error,
                        ));
                    }
                };
                Task::perform(
                    async move {
                        application
                            .repair()
                            .await
                            .map_err(|failure| InfiltratorError::Privilege(failure.message))
                    },
                    Message::PortConflictsRepaired,
                )
            }
            Message::PortConflictsRepaired(result) => match result {
                Ok(snapshot) => {
                    let had_conflicts = self.runtime.port_conflicts.has_conflicts();
                    self.runtime.port_conflicts = snapshot;
                    let message = if had_conflicts && !self.runtime.port_conflicts.has_conflicts() {
                        Lang(&copy_locale).tr("port_conflict_repaired").into_owned()
                    } else {
                        Lang(&copy_locale).tr("port_check_completed").into_owned()
                    };
                    Task::done(Message::ShowToast(message.to_owned(), ToastStatus::Success))
                }
                Err(error) => {
                    self.set_error(&error);
                    Task::done(Message::ShowToast(error.to_string(), ToastStatus::Error))
                }
            },
            Message::SetTunStack(stack) => {
                let parsed = match TunStack::parse(&stack) {
                    Some(stack) if stack.is_live_supported() => stack,
                    Some(stack) => {
                        let error = InfiltratorError::Config(format!(
                            "TUN stack {} is reference-only",
                            stack.as_str()
                        ));
                        self.set_error(&error);
                        return Task::done(Message::ShowToast(
                            error.to_string(),
                            ToastStatus::Error,
                        ));
                    }
                    None => {
                        let error = InfiltratorError::Config(
                            "unsupported tun stack: expected gvisor, system, or mixed".to_owned(),
                        );
                        self.set_error(&error);
                        return Task::done(Message::ShowToast(
                            error.to_string(),
                            ToastStatus::Error,
                        ));
                    }
                };
                let Some(rt) = self.runtime.runtime.clone() else {
                    return self.runtime_unavailable(
                        Lang(&copy_locale).tr("runtime_action_tun_stack").as_ref(),
                    );
                };
                let Some(token) = self.begin_runtime_patch() else {
                    return Task::none();
                };
                let generation = rt.generation();
                self.editor.tun_stack = parsed.as_str().to_owned();
                self.runtime.tun_stack_config.active_stack = parsed.as_str().to_owned();
                let gateway: Arc<dyn RuntimeGateway> = rt.clone();
                Task::perform(
                    async move {
                        RuntimeQueryApplication::new(gateway)
                            .set_tun_stack(parsed)
                            .await
                            .map_err(|failure| InfiltratorError::Config(failure.message))
                    },
                    move |result| Message::RuntimePatchResult(result, token, generation),
                )
            }
            Message::SetTunAutoRoute(enabled) => {
                let Some(rt) = self.runtime.runtime.clone() else {
                    return self.runtime_unavailable(
                        Lang(&copy_locale)
                            .tr("runtime_action_tun_auto_route")
                            .as_ref(),
                    );
                };
                let Some(token) = self.begin_runtime_patch() else {
                    return Task::none();
                };
                let generation = rt.generation();
                self.editor.tun_auto_route = enabled;
                if !enabled {
                    self.editor.tun_strict_route = false;
                }
                let gateway: Arc<dyn RuntimeGateway> = rt.clone();
                Task::perform(
                    async move {
                        RuntimeQueryApplication::new(gateway)
                            .set_tun_auto_route(enabled)
                            .await
                            .map_err(|failure| InfiltratorError::Config(failure.message))
                    },
                    move |result| Message::RuntimePatchResult(result, token, generation),
                )
            }
            Message::SetTunStrictRoute(enabled) => {
                let Some(rt) = self.runtime.runtime.clone() else {
                    return self.runtime_unavailable(
                        Lang(&copy_locale)
                            .tr("runtime_action_tun_strict_route")
                            .as_ref(),
                    );
                };
                let Some(token) = self.begin_runtime_patch() else {
                    return Task::none();
                };
                let generation = rt.generation();
                self.editor.tun_strict_route = enabled;
                if enabled {
                    self.editor.tun_auto_route = true;
                }
                let gateway: Arc<dyn RuntimeGateway> = rt.clone();
                Task::perform(
                    async move {
                        RuntimeQueryApplication::new(gateway)
                            .set_tun_strict_route(enabled)
                            .await
                            .map_err(|failure| InfiltratorError::Config(failure.message))
                    },
                    move |result| Message::RuntimePatchResult(result, token, generation),
                )
            }
            Message::SetSnifferEnabled(enabled) => {
                let Some(rt) = self.runtime.runtime.clone() else {
                    return self.runtime_unavailable(
                        Lang(&copy_locale).tr("runtime_action_sniffer").as_ref(),
                    );
                };
                let Some(token) = self.begin_runtime_patch() else {
                    return Task::none();
                };
                let generation = rt.generation();
                self.editor.sniffer_enabled = enabled;
                Task::perform(
                    async move {
                        rt.patch_config(serde_json::json!({ "sniffer": { "enable": enabled } }))
                            .await
                            .map_err(|error| InfiltratorError::Internal(error.to_string()))
                    },
                    move |result| Message::RuntimePatchResult(result, token, generation),
                )
            }
            Message::RuntimePatchResult(result, token, generation) => {
                if token != self.runtime.runtime_patch_token
                    || generation != self.runtime.runtime_generation
                {
                    return Task::none();
                }
                match result {
                    Ok(_) => {
                        self.runtime.pending_runtime_patch = None;
                        Task::done(Message::FetchRuntimeConfig)
                    }
                    Err(error) => {
                        self.restore_runtime_patch();
                        self.set_error(&error);
                        Task::none()
                    }
                }
            }
            Message::OperationResult(result) => match result {
                Ok(_) => Task::done(Message::FetchRuntimeConfig),
                Err(e) => {
                    self.set_error(&e);
                    Task::none()
                }
            },
            other => self.update_core_rules(other),
        }
    }
}

fn tun_status(state: ServiceModeState) -> TunServiceStatus {
    match state {
        ServiceModeState::Ready => TunServiceStatus::InstalledAndRunning,
        ServiceModeState::InstalledStopped => TunServiceStatus::InstalledStopped,
        ServiceModeState::NotInstalled => TunServiceStatus::NotInstalled,
        ServiceModeState::MissingPrivilege => TunServiceStatus::MissingPrivilege,
        ServiceModeState::Unsupported | ServiceModeState::Unavailable => {
            TunServiceStatus::Unsupported
        }
    }
}

fn legacy_service_snapshot(status: TunServiceStatus) -> ServiceModeSnapshot {
    ServiceModeSnapshot {
        platform: Default::default(),
        state: match status {
            TunServiceStatus::InstalledAndRunning => ServiceModeState::Ready,
            TunServiceStatus::InstalledStopped => ServiceModeState::InstalledStopped,
            TunServiceStatus::NotInstalled => ServiceModeState::NotInstalled,
            TunServiceStatus::MissingPrivilege => ServiceModeState::MissingPrivilege,
            TunServiceStatus::Unsupported => ServiceModeState::Unsupported,
        },
    }
}
