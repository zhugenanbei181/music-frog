//! Live runtime configuration: query non-routing controller settings and
//! patch TUN/sniffer toggles through the runtime gateway.

use crate::port_conflict_application::application;
use crate::state::AppState;
use crate::types::app::ToastStatus;
use crate::types::message::Message;
use crate::types::runtime::RuntimePatchSnapshot;
use iced::Task;
use infiltrator_application::runtime_configuration_application::RuntimeConfigurationApplication;
use infiltrator_application::runtime_control_projection::{
    ipv6_observation, lan_security_observation, runtime_control,
};
use infiltrator_application::service_mode_application::ServiceModeApplication;
use infiltrator_application::system_toggle_application::SystemToggleApplication;
use infiltrator_contract::error::{Failure, InfiltratorError};
use infiltrator_contract::runtime_control::RuntimeControlStatus;
use infiltrator_contract::service_mode::{ServiceModeSnapshot, ServiceModeState};
use infiltrator_contract::system_toggle::{SystemToggle, SystemToggleState};
use infiltrator_contract::tun::TunStack;
use infiltrator_desktop::tun_service::TunServiceManager;
use infiltrator_ports::host_runtime::TunServiceStatus;
use infiltrator_ports::runtime_configuration::ObservedValue;
use infiltrator_ports::runtime_gateway::RuntimeGateway;
use infiltrator_shared::i18n_interpolator::localize;
use infiltrator_shared::locales::{Lang, Localizer};
use std::sync::Arc;
use tokio::task::spawn_blocking;

impl AppState {
    fn runtime_configuration_scope(
        &self,
    ) -> Result<infiltrator_contract::core_identity::CoreScope, Failure> {
        self.runtime.core_lifecycle.scope().ok_or_else(|| {
            Failure::new(
                infiltrator_contract::error::ErrorCode::NotReady,
                "live configuration requires an observed Core scope",
                false,
            )
        })
    }

    fn configuration_failure(&mut self, failure: Failure) -> Task<Message> {
        let error = InfiltratorError::Config(failure.message);
        self.set_error(&error);
        Task::done(Message::ShowToast(error.to_string(), ToastStatus::Error))
    }

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
            || self.runtime.routing_intent_actions.pending.is_some()
        {
            return None;
        }
        if self.runtime.runtime.is_some() {
            if self.runtime.pending_runtime_patch.is_none() {
                self.runtime.pending_runtime_patch = Some(RuntimePatchSnapshot {
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
        let scope = match self.runtime_configuration_scope() {
            Ok(scope) => scope,
            Err(failure) => {
                let error = InfiltratorError::Config(failure.message);
                self.set_error(&error);
                return Task::done(Message::ShowToast(error.to_string(), ToastStatus::Error));
            }
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
                RuntimeConfigurationApplication::from_gateway(scope, gateway.as_ref())
                    .set_tun_enabled(enabled)
                    .await
                    .map(|_| ())
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
                    let scope = match self.runtime_configuration_scope() {
                        Ok(scope) => scope,
                        Err(failure) => {
                            return Task::done(Message::RuntimeConfigFetched(
                                Err(failure),
                                generation,
                            ));
                        }
                    };
                    let gateway: Arc<dyn RuntimeGateway> = rt;
                    Task::perform(
                        async move {
                            RuntimeConfigurationApplication::from_gateway(scope, gateway.as_ref())
                                .observe_readout()
                                .await
                        },
                        move |result| Message::RuntimeConfigFetched(result, generation),
                    )
                } else {
                    Task::none()
                }
            }
            Message::RuntimeConfigFetched(result, generation) => {
                if generation != self.runtime.runtime_generation {
                    return Task::none();
                }
                match result {
                    Ok(config) => {
                        let observed = runtime_control(Some(&Ok(config.clone())));
                        self.runtime.runtime_control = observed.clone();
                        if let Some(ipv6) = ipv6_observation(&config) {
                            self.runtime.ipv6_routing = ipv6;
                        }
                        let mut lan_committed = self.runtime.lan_sharing_committed.clone();
                        if let ObservedValue::Reported(enabled) = &config.lan_sharing.value.enabled
                        {
                            lan_committed.allow_lan = *enabled;
                        }
                        if let ObservedValue::Reported(mixed_port) =
                            &config.lan_sharing.value.mixed_port
                        {
                            lan_committed.mixed_port = *mixed_port;
                        }
                        if let ObservedValue::Reported(bind_address) =
                            &config.lan_sharing.value.bind_address
                        {
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
                        self.runtime.tun_enabled = observed.tun_enabled;
                        self.runtime.system_toggles = self
                            .runtime
                            .system_toggles
                            .clone()
                            .with_tun_readback(observed.tun_enabled);
                        if let ObservedValue::Reported(nameserver) = &config.dns.value.nameserver {
                            self.editor.dns_nameservers = nameserver.clone();
                        }
                        if let ObservedValue::Reported(fallback) = &config.dns.value.fallback {
                            self.editor.dns_fallback_servers = fallback.clone();
                        }
                        if let ObservedValue::Reported(mode) = &config.dns.value.enhanced_mode {
                            self.editor.dns_enhanced_mode = mode.clone();
                        }
                        if let ObservedValue::Reported(stack) = &config.tun.value.stack {
                            self.editor.tun_stack = stack.clone();
                        }
                        if let ObservedValue::Reported(auto_route) = &config.tun.value.auto_route {
                            self.editor.tun_auto_route = *auto_route;
                        }
                        if let ObservedValue::Reported(strict_route) =
                            &config.tun.value.strict_route
                        {
                            self.editor.tun_strict_route = *strict_route;
                        }
                        if let ObservedValue::Reported(enabled) = &config.sniffer.value.enabled {
                            self.editor.sniffer_enabled = *enabled;
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
                let scope = match self.runtime_configuration_scope() {
                    Ok(scope) => scope,
                    Err(failure) => return self.configuration_failure(failure),
                };
                let generation = rt.generation();
                self.runtime.ipv6_routing.enabled = enabled;
                let gateway: Arc<dyn RuntimeGateway> = rt;
                Task::perform(
                    async move {
                        RuntimeConfigurationApplication::from_gateway(scope, gateway.as_ref())
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
                let scope = match self.runtime_configuration_scope() {
                    Ok(scope) => scope,
                    Err(failure) => return self.configuration_failure(failure),
                };
                let generation = rt.generation();
                self.editor.tun_stack = parsed.as_str().to_owned();
                self.runtime.tun_stack_config.active_stack = parsed.as_str().to_owned();
                let gateway: Arc<dyn RuntimeGateway> = rt.clone();
                Task::perform(
                    async move {
                        RuntimeConfigurationApplication::from_gateway(scope, gateway.as_ref())
                            .set_tun_stack(parsed)
                            .await
                            .map(|_| ())
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
                let scope = match self.runtime_configuration_scope() {
                    Ok(scope) => scope,
                    Err(failure) => return self.configuration_failure(failure),
                };
                let generation = rt.generation();
                self.editor.tun_auto_route = enabled;
                if !enabled {
                    self.editor.tun_strict_route = false;
                }
                let gateway: Arc<dyn RuntimeGateway> = rt.clone();
                Task::perform(
                    async move {
                        RuntimeConfigurationApplication::from_gateway(scope, gateway.as_ref())
                            .set_tun_auto_route(enabled)
                            .await
                            .map(|_| ())
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
                let scope = match self.runtime_configuration_scope() {
                    Ok(scope) => scope,
                    Err(failure) => return self.configuration_failure(failure),
                };
                let generation = rt.generation();
                self.editor.tun_strict_route = enabled;
                if enabled {
                    self.editor.tun_auto_route = true;
                }
                let gateway: Arc<dyn RuntimeGateway> = rt.clone();
                Task::perform(
                    async move {
                        RuntimeConfigurationApplication::from_gateway(scope, gateway.as_ref())
                            .set_tun_strict_route(enabled)
                            .await
                            .map(|_| ())
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
                let scope = match self.runtime_configuration_scope() {
                    Ok(scope) => scope,
                    Err(failure) => return self.configuration_failure(failure),
                };
                let generation = rt.generation();
                self.editor.sniffer_enabled = enabled;
                let gateway: Arc<dyn RuntimeGateway> = rt;
                Task::perform(
                    async move {
                        RuntimeConfigurationApplication::from_gateway(scope, gateway.as_ref())
                            .set_sniffer_enabled(enabled)
                            .await
                            .map(|_| ())
                            .map_err(|failure| InfiltratorError::Config(failure.message))
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
