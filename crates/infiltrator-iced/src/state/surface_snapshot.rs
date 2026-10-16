use super::proxy_probe;
use super::*;
use infiltrator_contract::resources::CoreResourceSnapshot;

impl AppState {
    /// Apply the shared page read model as a monotonic render cache. The
    /// existing Elm fields remain toolkit-local projections; stale host
    /// events cannot overwrite a newer shared revision.
    pub fn apply_shared_surface_snapshot(&mut self, snapshot: SurfaceSnapshot) -> bool {
        let previous_scope = self
            .surface
            .latest()
            .map(|previous| (previous.core.generation, previous.core.session_token))
            .unwrap_or((
                self.runtime.core_lifecycle.generation,
                self.runtime.core_lifecycle.session_token,
            ));
        let changed_scope = self.commands.is_some()
            && (previous_scope != (snapshot.core.generation, snapshot.core.session_token)
                || self
                    .surface
                    .latest()
                    .is_some_and(|previous| previous.origin != snapshot.origin));
        if !self.surface.apply(snapshot.clone()) {
            return false;
        }
        self.rebind_product_command_scope(&snapshot);
        let product_outbound = self.is_product_command_ingress();
        let demo_outbound = self.shell.demo && snapshot.origin == SurfaceOrigin::Demo;
        if changed_scope {
            self.editor.rule_providers.clear();
            self.runtime.proxy_groups.clear();
            self.runtime.proxy_name_runs.clear();
            self.runtime.filtered_groups.clear();
            self.runtime.runtime_selected_group.clear();
            self.runtime.runtime_selected_proxy.clear();
            self.runtime.runtime_outbound_selection = None;
            self.runtime.outbound_details = PageData::unavailable(Failure::new(
                ErrorCode::NotReady,
                "outbound details have not been observed for the current scope",
                true,
            ));
            self.runtime.outbound_ui_preferences = PageData::unavailable(Failure::unsupported(
                "outbound UI preferences have not been observed for the current scope",
            ));
            self.runtime.outbound_inspecting = None;
            self.runtime.outbound_probe_result = None;
            self.runtime.outbound_probe_pending = None;
            self.runtime.outbound_probe_failure = None;
            self.runtime.outbound_probe_blocked = None;
            self.runtime.outbound_probe_batch_token =
                self.runtime.outbound_probe_batch_token.wrapping_add(1);
            self.runtime.outbound_probe_batch = proxy_probe::OutboundProbeBatchState::Idle;
            self.diag.connections = None;
            self.diag.connection_groups.clear_observation();
            self.diag.connection_rates = Default::default();
            self.diag.connection_rate_book = Default::default();
            self.diag.connection_activity = Default::default();
            self.diag.inspecting_connection_id = None;
            self.diag.traffic = None;
            self.diag.traffic_history.clear();
            self.runtime.core_resources = CoreResourceSnapshot::NotObserved;
            self.diag.public_ip = None;
            self.diag.public_ip_provider = None;
            self.diag.public_ip_checked_at = None;
            self.diag.public_ip_error = None;
        }
        if product_outbound || demo_outbound {
            self.apply_outbound_inventory_observation(snapshot.outbound_inventory.clone());
            self.apply_outbound_details_observation(snapshot.outbound_details.clone());
            self.apply_outbound_ui_preferences_observation(
                snapshot.outbound_ui_preferences.clone(),
            );
            self.refresh_outbound_probe_block();
        }
        self.observe_editor_surface(&snapshot.profile_editor);
        self.shell.readout = snapshot.shell_readout.clone();
        self.diag.log_search.observe(
            snapshot.core.generation,
            snapshot.core.session_token,
            &snapshot.pages.logs,
        );
        if self.commands.is_some() {
            self.diag.log_filter.level_filter = snapshot
                .pages
                .logs
                .data
                .as_ref()
                .and_then(|logs| logs.active_level.clone())
                .unwrap_or_default();
            self.diag.logs = snapshot
                .pages
                .logs
                .data
                .as_ref()
                .map(|logs| {
                    logs.entries
                        .iter()
                        .map(|entry| entry.raw.clone().unwrap_or_else(|| entry.message.clone()))
                        .collect()
                })
                .unwrap_or_default();
            self.diag.logs_stream_state =
                match snapshot.pages.logs.data.as_ref().map(|logs| &logs.stream) {
                    Some(LogStreamState::Live) => RuntimeStreamState::Connected,
                    Some(LogStreamState::Connecting) => RuntimeStreamState::Connecting,
                    Some(LogStreamState::Reconnecting(_)) => RuntimeStreamState::Reconnecting,
                    Some(LogStreamState::Failed(failure)) => {
                        RuntimeStreamState::Failed(failure.message.clone())
                    }
                    _ => match &snapshot.pages.logs.status {
                        PageStatus::Failed { failure } | PageStatus::Unavailable { failure } => {
                            RuntimeStreamState::Failed(failure.message.clone())
                        }
                        PageStatus::Loading => RuntimeStreamState::Connecting,
                        _ => RuntimeStreamState::Idle,
                    },
                };
        }

        self.editor.rule_providers = snapshot
            .pages
            .rules
            .data
            .as_ref()
            .map(|page| page.providers.clone())
            .unwrap_or_default();
        self.observe_rule_list_page(&snapshot.pages.rules);
        self.observe_probe_settings(&snapshot.probe_settings);
        self.observe_language_settings(&snapshot.language_settings);
        self.observe_group_order();
        self.runtime.routing_intent_actions.observe(
            snapshot.generation,
            snapshot.revision,
            routing_intent_projection::from_surface(&snapshot),
        );
        self.runtime.routing_intent_state = self.runtime.routing_intent_actions.render_snapshot();
        self.runtime.routing_intent = self.runtime.routing_intent_state.current.clone();
        self.runtime.runtime_control = snapshot.runtime_control.clone();
        if self.commands.is_none()
            || (snapshot.core.generation, snapshot.core.revision)
                >= (
                    self.runtime.core_lifecycle.generation,
                    self.runtime.core_lifecycle.revision,
                )
        {
            self.runtime.status = RuntimeStatus::from_core_snapshot(&snapshot.core);
            self.runtime.core_lifecycle = snapshot.core.lifecycle_snapshot();
        }
        self.runtime.mtu = snapshot.mtu.clone();
        if let Some(settings) = snapshot.pages.settings.data.as_ref()
            && let Some(ipv6) = settings.ipv6_routing
        {
            self.runtime.ipv6_routing = ipv6;
        }
        self.runtime.system_proxy = snapshot.system_proxy.clone();
        self.runtime.system_proxy_recovery = snapshot.system_proxy_recovery.clone();
        self.runtime.network_roaming = snapshot.network_roaming.clone();
        self.runtime.vpn_observation = snapshot.vpn.clone();
        self.runtime.privileged_network = snapshot.privileged_network.clone();
        self.runtime.traffic_waveform = snapshot.traffic_waveform.clone();
        self.runtime.traffic_scale = snapshot.traffic_scale.clone();
        self.runtime.traffic_topology = snapshot.traffic_topology.clone();
        self.runtime.reconnect_mask = snapshot.reconnect_mask.clone();
        self.runtime.active_exit = snapshot.active_exit.clone();
        self.runtime.subscription_quota = snapshot.subscription_quota.clone();
        self.diag.speedtest = snapshot.speedtest.clone();
        self.apply_dns_leak_snapshot(&snapshot.dns_leak);
        self.diag.dns_cache_actions.observe(&snapshot.dns_cache);
        self.diag.dns_query.observe(&snapshot.dns_query);
        self.diag.dns_cache_flush = self.diag.dns_cache_actions.snapshot.report.clone();
        self.editor.dns_hosts_editor.observe(&snapshot.dns_hosts);
        if let Some(dns) = snapshot.pages.dns.data.as_ref() {
            // Canonical cross-surface DNS workbench input: re-seed the form
            // from the shared read model only while the user has no unsaved
            // edits, so a poll never clobbers typing.
            if !self.editor.dns_form_dirty && !self.editor.dns_json_dirty {
                self.editor.dns_form = DnsWorkbenchForm::from_snapshot(dns);
            }
            // DUAL-14-06: the observed Fake-IP bindings are a shared fact.
            self.editor.dns_fake_ip_pool = dns.fake_ip_pool.clone();
            // DUAL-14-10: the last real probe is a shared fact; a probe in
            // flight keeps the local optimistic view until the reader
            // publishes the same report.
            if !self.editor.dns_latency_probe_phase.is_probing() || dns.latency.is_probed() {
                self.editor.dns_latency = dns.latency.clone();
            }
            // DUAL-14-13: the self-heal observation is a shared fact.
            self.editor.dns_self_heal = dns.self_heal.clone();
            // DUAL-14-08: the cross-source leak report is a shared fact.
            // DUAL-14-09 (re-scoped): the STUN UDP-egress report is a shared
            // fact.
            self.apply_stun_snapshot(dns);
            // The read model carries the honest last flush report for the
            // shared host application; a local report from this session is
            // never downgraded back to `NotRequested`.
        }
        let previous_trace_report = self.editor.rule_trace.snapshot.report_id;
        if self.editor.rule_trace.observe(snapshot.rule_trace.clone())
            && let Some(report) = &self.editor.rule_trace.snapshot.report
        {
            self.editor.rules_tracer_chain = report.decision_chain.clone();
        }
        self.editor.rules_tracer_can_reverse_apply = false;
        self.editor.rule_hit_audit.observe(&snapshot.pages.rules);
        if let Some(rules_page) = snapshot.pages.rules.data.as_ref() {
            // DUAL-12-08: consume the shared reverse-apply facts; the chooser
            // is gated and seeded from the surface read model, not guessed.
            self.editor.rules_tracer_can_reverse_apply = rules_page.tracer.can_reverse_apply;
            self.editor.rules_tracer_suggested_target =
                rules_page.tracer.suggested_override_target.clone();
            if previous_trace_report != self.editor.rule_trace.snapshot.report_id {
                self.editor.rules_tracer_override_target = rules_page
                    .tracer
                    .suggested_override_target
                    .clone()
                    .unwrap_or_default();
            }
            // DUAL-11-03 / 11-04: the MRS acceleration read model and the
            // declared provider source URLs are shared facts, never UI-local.
            self.editor.mrs_acceleration = rules_page.mrs_acceleration.clone();
            // DUAL-11-08: the publish cap and the omitted count are shared
            // facts; the editor keeps its full profile list and says so.
            self.editor.rule_publish_limit = rules_page.rule_publish_limit;
            self.editor.rule_publish_omitted = rules_page
                .is_truncated()
                .then(|| rules_page.omitted_rule_count());
            // DUAL-11-07: the observed kernel provider-cache fact is shared
            // with Bevy; the card renders the same directory/count/size.
            self.editor.rule_provider_cache = rules_page.provider_cache.clone();
            // DUAL-11-05: the kernel's real `etag-support` capability declared
            // by the active profile, projected from the same shared read model.
            self.editor.rule_etag_support = rules_page.etag_support;
        }
        self.diag.overview_card_order = snapshot.overview_layout.order.clone();
        self.runtime.system_toggles = system_toggle_projection::from_surface(&snapshot);
        if matches!(
            &snapshot.system_proxy.status,
            SystemProxyStatus::Enabled | SystemProxyStatus::Disabled
        ) {
            self.runtime.system_proxy_enabled = snapshot.system_proxy.is_enabled();
        }
        self.runtime.runtime_generation = snapshot.core.generation;
        self.runtime.core_session_token = snapshot.core.session_token;
        self.runtime.core_versions = snapshot.versions.clone();
        self.runtime.core_integrity = snapshot.versions.verification.clone();
        if let Some(settings) = snapshot.pages.settings.data.as_ref()
            && snapshot.runtime_control.status == RuntimeControlStatus::Ready
        {
            if let Some(auto_route) = settings.tun_auto_route {
                self.editor.tun_auto_route = auto_route;
            }
            if let Some(strict_route) = settings.tun_strict_route {
                self.editor.tun_strict_route = strict_route;
            }
            let mut committed = self.runtime.lan_sharing_committed.clone();
            if let Some(value) = settings.allow_lan {
                committed.allow_lan = value;
            }
            if let Some(value) = settings.mixed_port {
                committed.mixed_port = value;
            }
            if let Some(value) = &settings.lan_bind_address {
                committed.bind_address = value.clone();
            }
            self.runtime.lan_sharing_committed = committed.clone();
            if !self.runtime.lan_sharing_dirty {
                self.runtime.lan_sharing = committed;
            }

            if let Some(security) = &settings.lan_security {
                let mut security_committed = self.runtime.lan_security_committed.clone();
                security_committed.allowed_ips = security.allowed_ips.join(", ");
                security_committed.disallowed_ips = security.disallowed_ips.join(", ");
                security_committed.skip_auth_prefixes = security.skip_auth_prefixes.join(", ");
                security_committed.authentication_enabled = security.authentication_enabled;
                security_committed.authentication_user_count = security.authentication_user_count;
                if let Some(username) = security.authentication_username.as_ref() {
                    security_committed.auth_username = username.clone();
                }
                security_committed.auth_password.clear();
                self.runtime.lan_security_committed = security_committed.clone();
                if !self.runtime.lan_security_dirty {
                    self.runtime.lan_security = security_committed;
                    self.runtime.lan_sharing.acl_whitelist_cidrs = security.allowed_ips.join(", ");
                }
            }
        }
        if !self.shell.uwp_loopback.is_scanning
            && let Some(app_routing) = snapshot.pages.app_routing.data.as_ref()
        {
            let uwp = &app_routing.uwp_loopback;
            self.shell.uwp_loopback.availability = uwp.availability.clone();
            self.shell.uwp_loopback.revision = uwp.revision;
            self.shell.uwp_loopback.apps = uwp
                .packages
                .iter()
                .map(|package| UwpAppItem {
                    sid: package.sid.clone(),
                    display_name: package.display_name.clone(),
                    is_exempt: package.loopback_exempt,
                })
                .collect();
            self.shell.uwp_loopback.is_scanning = false;
        }
        if let Some(settings) = snapshot.pages.settings.data.as_ref() {
            let pac = &mut self.runtime.pac_manager;
            pac.snapshot = settings.pac.clone();
            if !pac.dirty {
                pac.bypass_subnets = settings.pac.bypass_domains.join(", ");
                match &settings.pac.state {
                    PacServiceState::Running { url } => {
                        pac.is_pac_mode_active = true;
                        pac.pac_url = url.clone();
                    }
                    PacServiceState::Disabled | PacServiceState::Unavailable { .. } => {
                        pac.is_pac_mode_active = false;
                        pac.pac_url.clear();
                    }
                }
            }
        }
        self.runtime.controller_auth = snapshot.controller_auth;
        self.runtime.service_mode = snapshot.service_mode;
        self.runtime.port_conflicts = snapshot.port_conflicts.clone();
        self.runtime.core_resources = snapshot.resources.clone();
        self.runtime.offline_startup = snapshot.offline_startup.clone();
        self.diag.crash_watchdog.shared = snapshot.core.watchdog.clone();
        self.diag.crash_watchdog.last_crash_summary = snapshot
            .core
            .watchdog
            .last_error
            .as_ref()
            .map(|failure| sanitize_ui_text(&failure.message));
        self.diag.traffic = snapshot
            .shell_readout
            .upload_bps
            .value()
            .copied()
            .zip(snapshot.shell_readout.download_bps.value().copied())
            .map(|(up, down)| TrafficData {
                up: up as u64,
                down: down as u64,
            });
        if self.commands.is_some() {
            self.diag.traffic_history = snapshot
                .traffic_waveform
                .samples
                .iter()
                .map(|sample| (sample.upload_bps as u64, sample.download_bps as u64))
                .collect();
        }
        true
    }

    pub(crate) fn enable_product_command_ingress(&mut self) {
        self.command_scope = ObservedCoreScope::Unavailable;
    }
}
