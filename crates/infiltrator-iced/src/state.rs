//! AppState 域拆分(UI-002):原先 161 个平铺字段收敛为五个语义域结构体。
//!
//! 域边界(与 `TODO.md` UI-002 对应):
//! - [`RuntimeState`] 内核与运行控制;[`ProfileState`] 订阅/档案/同步;
//! - [`ConfigEditorState`] 全部配置编辑器;[`DiagnosticsState`] 运行态诊断与性能;
//! - [`ShellState`] 导航/语言/主题/托盘/Admin/demo 等外壳关注点。
//!
//! 视图层只读这些域做纯渲染投影;update 层按域定位字段。

use crate::state::diagnostics::DiagnosticsState;
use crate::state::editor::ConfigEditorState;
use crate::state::profiles::ProfileState;
use crate::state::runtime::RuntimeState;
use crate::state::shell::ShellState;
use crate::surface::{SurfaceBridge, SurfaceModel};
use crate::types::app::UwpAppItem;
use crate::types::app_routing::AppRoutingState;
use crate::types::runtime::{RuntimeStatus, RuntimeStreamState};
use crate::utils::sanitize_ui_text;
use infiltrator_application::command_palette_projection::filtered_indices;
use infiltrator_application::core_application::CoreApplication;
use infiltrator_application::proxy_mode_application::ProxyModeApplication;
use infiltrator_application::system_toggle_application::SystemToggleApplication;
use infiltrator_contract::command::ProxyMode;
use infiltrator_contract::command_catalogue::{CommandCatalogue, ProfileChoice};
use infiltrator_contract::dns_form::DnsWorkbenchForm;
use infiltrator_contract::logs::LogStreamState;
use infiltrator_contract::mini_hud::{MiniHudReadModel, MiniHudWaveformStrip};
use infiltrator_contract::pac::PacServiceState;
use infiltrator_contract::profile_protection::ProfileWriteProtection;
use infiltrator_contract::runtime_control::RuntimeControlStatus;
use infiltrator_contract::surface_snapshot::{PageStatus, SurfaceSnapshot};
use infiltrator_contract::system_proxy::SystemProxyStatus;
use infiltrator_domain::runtime::{MemoryData, TrafficData};
use infiltrator_shared::fuzzy_search::pinyin_fuzzy_match;
use infiltrator_shared::locales::{Lang, Localizer};
use std::sync::Arc;

pub mod core_control;
pub mod diagnostics;
pub mod editor;
mod editor_observation;
pub mod language;
pub mod profiles;
pub mod proxy_group_order;
pub mod proxy_inspection;
pub mod proxy_preferences;
pub mod proxy_probe;
pub mod proxy_probe_settings;
pub mod proxy_search;
pub mod runtime;
pub mod shell;
mod traffic;

pub struct AppState {
    pub runtime: RuntimeState,
    pub profile: ProfileState,
    pub editor: ConfigEditorState,
    pub diag: DiagnosticsState,
    pub shell: ShellState,
    pub app_routing: AppRoutingState,
    /// Canonical cross-surface snapshot cache. Existing Elm fields are local
    /// render/update projections and must not become a second shared source.
    pub surface: SurfaceModel,
    /// Optional application pump bridge installed by a desktop/mobile
    /// composition root. `None` keeps the existing pull-free test/demo app.
    pub surface_bridge: Option<SurfaceBridge>,
    /// Host-injected neutral command facade; embedded/headless hosts need no desktop runtime.
    pub commands: Option<CoreApplication>,
    /// Host-provided process-exit cleanup callback. The UI knows only this
    /// zero-argument seam; OS signal and proxy/TUN details stay in the host.
    pub exit_cleanup: Option<Arc<dyn Fn() + Send + Sync>>,
}

impl AppState {
    /// Rebuild the command catalogue from the stored profile list. Called when
    /// the palette opens and after the profile list changes, so the palette
    /// always lists the live profiles alongside the shared product commands.
    pub fn rebuild_command_catalogue(&mut self) {
        let profiles: Vec<ProfileChoice> = self
            .profile
            .profiles
            .iter()
            .map(|profile| ProfileChoice::new(profile.name.clone(), profile.name.clone()))
            .collect();
        self.shell.command_catalogue = CommandCatalogue::with_profiles(&profiles);
        self.shell.command_selected_index = self
            .shell
            .command_selected_index
            .min(self.filtered_command_indices().len().saturating_sub(1));
    }

    /// Indices of the shared catalogue kept by the current query. The shared
    /// substring rule plus the Iced pinyin matcher; both surfaces derive the
    /// arrow-key order from the same shared catalogue.
    pub fn filtered_command_indices(&self) -> Vec<usize> {
        let lang = Lang(&self.shell.lang);
        filtered_indices(
            &self.shell.command_catalogue,
            &self.shell.command_query,
            &|key| lang.tr(key).into_owned(),
            &pinyin_fuzzy_match,
        )
    }

    /// The Mini HUD's shared read model, assembled from the live projections:
    /// traffic rates, the controller-reported mode, the selected exit node and
    /// the shared system-toggle snapshot. Nothing here is a constant.
    pub fn mini_hud_read_model(&self) -> MiniHudReadModel {
        let active_node = if !self.runtime.runtime_selected_proxy.is_empty() {
            self.runtime.runtime_selected_proxy.clone()
        } else if let Some(active) = self.profile.profiles.iter().find(|profile| profile.active) {
            active.name.clone()
        } else {
            String::new()
        };
        let mode_zh = self
            .runtime
            .proxy_mode
            .as_deref()
            .and_then(ProxyMode::from_wire)
            .map(|mode| {
                Localizer::tr(&Lang(&self.shell.lang), &format!("mode_{}", mode.to_wire()))
                    .into_owned()
            })
            .unwrap_or_default();
        let (up, down) = self
            .diag
            .traffic
            .as_ref()
            .map(|traffic| (traffic.up, traffic.down))
            .unwrap_or((0, 0));
        MiniHudReadModel {
            visible: self.shell.mini_hud_mode,
            mode_zh,
            exit_node: active_node,
            down_bytes_per_sec: down,
            up_bytes_per_sec: up,
            system_proxy: self.runtime.system_toggles.system_proxy.clone(),
            tun: self.runtime.system_toggles.tun.clone(),
            placement: self.shell.mini_hud_placement,
            waveform: MiniHudWaveformStrip::from_snapshot(&self.runtime.traffic_waveform),
        }
    }

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
        if changed_scope {
            self.runtime.proxies.clear();
            self.runtime.proxy_groups.clear();
            self.runtime.proxy_name_runs.clear();
            self.runtime.filtered_groups.clear();
            self.runtime.is_loading_proxies = false;
            self.runtime.runtime_selected_group.clear();
            self.runtime.runtime_selected_proxy.clear();
            self.runtime.inspecting_proxy = None;
            self.runtime.inspection_read = Default::default();
            self.diag.connections = None;
            self.diag.connection_groups.clear_observation();
            self.diag.connection_rates = Default::default();
            self.diag.connection_rate_book = Default::default();
            self.diag.connection_activity = Default::default();
            self.diag.inspecting_connection_id = None;
            self.diag.traffic = None;
            self.diag.traffic_history.clear();
            self.diag.memory = None;
            self.diag.public_ip = None;
            self.diag.public_ip_provider = None;
            self.diag.public_ip_checked_at = None;
            self.diag.public_ip_error = None;
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

        self.observe_rule_list_page(&snapshot.pages.rules);
        self.reconcile_proxy_inspection();
        self.observe_probe_settings(&snapshot.probe_settings);
        self.observe_language_settings(&snapshot.language_settings);
        self.observe_group_order();
        if let Some(proxies) = snapshot.pages.proxies.data.as_ref() {
            if self.runtime.proxy_search.pending.is_none()
                && self.runtime.proxy_search.failure.is_none()
                && self.runtime.proxy_filter == proxies.search_query
            {
                self.runtime.proxy_search.draft_dirty = false;
            }
            if !self.runtime.proxy_search.draft_dirty {
                self.runtime.proxy_filter = proxies.search_query.clone();
            }
            self.runtime.proxy_groups = proxies.groups.clone();
            self.runtime.proxy_name_runs = proxies.name_runs.clone();
            self.runtime.filtered_groups = proxies
                .groups
                .iter()
                .map(|group| {
                    (
                        group.name.clone(),
                        group.proxies.iter().map(|node| node.name.clone()).collect(),
                    )
                })
                .collect();
            self.runtime.filter_alive_only = proxies.filter_alive.enabled;
            self.runtime.proxy_compact_view = proxies.compact_view;
            self.runtime.proxy_delay_sort = proxies.sort_order.as_wire_key().into();
            self.runtime.favorite_proxies = proxies
                .groups
                .iter()
                .flat_map(|group| &group.proxies)
                .filter(|node| node.favorite)
                .map(|node| node.name.clone())
                .collect();
        }
        self.runtime.proxy_mode = snapshot
            .runtime_control
            .mode
            .or(snapshot.core.proxy_mode)
            .map(|mode| mode.to_wire().to_owned());
        self.runtime.script_block_present = snapshot.runtime_control.script_available == Some(true);
        self.runtime.mode_actions.observe(
            snapshot.generation,
            snapshot.revision,
            ProxyModeApplication::from_surface(&snapshot),
        );
        self.runtime.proxy_mode_state = self.runtime.mode_actions.render_snapshot();
        self.runtime.proxy_mode = self
            .runtime
            .proxy_mode_state
            .current
            .map(|mode| mode.to_wire().to_owned());
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
        self.runtime.vpn = snapshot.vpn.clone();
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
            if !self.editor.is_probing_dns_latency || dns.latency.is_probed() {
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
            self.editor.rule_provider_source_urls = rules_page
                .providers
                .iter()
                .filter_map(|provider| {
                    provider
                        .source_url
                        .as_ref()
                        .map(|url| (provider.name.clone(), url.clone()))
                })
                .collect();
            self.editor.rule_provider_intervals = rules_page
                .providers
                .iter()
                .filter_map(|provider| {
                    provider
                        .refresh_interval_secs
                        .map(|secs| (provider.name.clone(), secs))
                })
                .collect();
            // DUAL-11-05: the local cache fingerprints the shared reader
            // observed; the provider row renders them verbatim.
            self.editor.rule_provider_fingerprints = rules_page
                .providers
                .iter()
                .filter_map(|provider| {
                    provider
                        .cache_fingerprint
                        .as_ref()
                        .map(|observation| (provider.name.clone(), observation.clone()))
                })
                .collect();
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
        self.runtime.system_toggles = SystemToggleApplication::from_surface(&snapshot);
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
            .value
            .zip(snapshot.shell_readout.download_bps.value)
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
        self.diag.memory = snapshot.core.memory_bytes.map(|in_use| MemoryData {
            in_use,
            os_limit: 0,
        });
        true
    }

    /// Attach the host-composed shared surface input before the Iced runtime
    /// starts. The update loop only receives typed `Message` values afterward.
    pub fn attach_surface_bridge(&mut self, bridge: SurfaceBridge) {
        self.surface_bridge = Some(bridge);
    }

    /// DUAL-09-12: the write classification of the profile open in the editor.
    /// This reads the same shared metadata the application guard enforces, so
    /// the banner, the disabled save button and the guard cannot disagree.
    pub fn edited_profile_write_protection(&self) -> ProfileWriteProtection {
        if let Some((_, document)) = &self.editor.document_latest
            && self
                .editor
                .document_session
                .source()
                .is_some_and(|source| source.profile == document.profile)
        {
            return document.write_protection;
        }
        use infiltrator_contract::profile_protection::ProfileWriteProtection;
        let Some(name) = self
            .editor
            .editor_path
            .as_ref()
            .and_then(|path| path.file_stem())
            .and_then(|stem| stem.to_str())
        else {
            return ProfileWriteProtection::Editable;
        };
        self.profile
            .profiles
            .iter()
            .find(|profile| profile.name == name)
            .map(|profile| {
                ProfileWriteProtection::from_subscription_url(
                    profile.subscription_url.as_deref().unwrap_or_default(),
                )
            })
            .unwrap_or(ProfileWriteProtection::Editable)
    }
}
