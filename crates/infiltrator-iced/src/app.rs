use crate::admin_server::{ADMIN_DEFAULT_PORT, AdminServerManager, AdminSharedRuntime};
use crate::configs_dir::config_manager;
use crate::host::desktop::{read_system_proxy_state, system_proxy_port};
use crate::mini_hud_window::install_host_handle;
use crate::notify::startup_probe_task;
use crate::settings_store::load_hydrated;
use crate::state::AppState;
use crate::state::diagnostics::DiagnosticsState;
use crate::state::editor::ConfigEditorState;
use crate::state::profiles::ProfileState;
use crate::state::runtime::RuntimeState;
use crate::state::shell::ShellState;
use crate::types::app::{RouteHistory, Transition};
use crate::types::dns::AdvancedValidationState;
use crate::types::doctor::DoctorPanelState;
use crate::types::options::EditorPane;
use crate::types::perf::PerfSnapshot;
use crate::types::runtime::RuntimeStreamState;
use iced::system::theme;
use iced::theme::Mode;
use iced::widget::text_editor::Content;
use iced::window::latest;
use infiltrator_application::connection_rate_application::ConnectionRateApplication;
use infiltrator_application::system_proxy_application::SystemProxyApplication;
use infiltrator_contract::command_catalogue::CommandCatalogue;
use infiltrator_contract::dns::DnsEnhancedMode;
use infiltrator_contract::dns_cache::DnsCacheFlushReport;
use infiltrator_contract::dns_form::DnsWorkbenchForm;
use infiltrator_contract::editor_viewport::EditorViewport;
use infiltrator_contract::ime::ImeCompositionTracker;
use infiltrator_contract::mini_hud::MiniHudPlacement;
use infiltrator_contract::overview_layout::OverviewCardKind;
use infiltrator_contract::privileged_network::PrivilegedNetworkSnapshot;
use infiltrator_contract::proxies::ProxyUiPreferences;
use infiltrator_contract::responsive_viewport::ResponsiveViewportSnapshot;
use infiltrator_contract::shortcuts::ShortcutRegistry;
use infiltrator_contract::snapshot_history::SNAPSHOT_DEFAULT_KEEP;
use infiltrator_contract::system_toggle::SystemToggleSnapshot;
use infiltrator_contract::theme::ThemePreference;
use infiltrator_contract::toast::ToastGate;
use infiltrator_contract::vpn::VpnSessionSnapshot;
use infiltrator_domain::connection_activity::{
    ConnectionActivityTracker, DEFAULT_IDLE_TIMEOUT_SECS,
};
use infiltrator_domain::rules::edit::DEFAULT_RULE_TARGET;
use infiltrator_domain::rules::logical::default_logical_draft;
use infiltrator_domain::rules::view::{RULE_DEFAULT_VIEWPORT_PX, RULE_PUBLISH_LIMIT};
use infiltrator_shared::autostart;
use infiltrator_shared::locales::{Lang, Localizer, get_system_language};
use std::collections::{HashMap, HashSet, VecDeque};
use std::env::var;
use std::sync::atomic::AtomicBool;
use std::sync::mpsc::channel;
use std::time::Instant;
// Only the non-test build spawns a real system tray (ksni/muda must never run
// in unit tests), so the import is test-gated.
#[cfg(not(test))]
use crate::tray;
#[cfg(not(test))]
use crate::tray::spec::TrayStartup;
use crate::types::app::Route;
use crate::types::dns::{AdvancedEditMode, DnsTab, FakeIpFormDraft, TunFormDraft};
use crate::types::editor::EditorLazyState;
use crate::types::message::Message;
use crate::types::runtime::{RebuildFlowState, RuntimeStatus};
use iced::Task;
use infiltrator_application::profile_application::ProfileApplication;
use infiltrator_contract::error::InfiltratorError;
use infiltrator_contract::rules_workspace::{RulesJsonSection, RulesTab};
use std::sync::{Arc, Mutex};

/// `INFILTRATOR_LANG` 会话级语言覆写（非 demo 启动路径；demo 有自己的同名
/// 契约，见 demo.rs）。只接受 `zh-CN` / `en-US`，其它值忽略。
///
/// 语义：**仅会话级** —— 启动时注入内存语言，并在设置加载回灌时剥离
/// settings.toml 的 `language` 字段让 env 值存活；绝不写盘、绝不修改用户
/// 的设置文件，用户仍可在会话内用设置页改语言（保存语义不变）。
fn env_lang_override() -> Option<String> {
    let value = var("INFILTRATOR_LANG").ok()?;
    match value.trim() {
        "zh-CN" => Some("zh-CN".to_string()),
        "en-US" => Some("en-US".to_string()),
        _ => None,
    }
}

impl AppState {
    pub fn title(&self) -> String {
        Lang(&self.shell.lang).tr("app_title").to_string()
    }

    pub fn theme(&self) -> iced::Theme {
        self.shell.theme.clone()
    }

    /// Production defaults with no fixture data — the shared base for the
    /// real constructor ([`Self::new`]) and the demo one ([`Self::demo`]).
    /// Pure in-memory construction apart from two host-state reads (system
    /// proxy state, autostart flag) that the demo constructor overrides.
    pub(crate) fn empty() -> Self {
        // Admin web server plumbing: one shared event bus, one manager for the
        // live server handle, one command channel back into update(). Pure
        // in-memory construction — nothing here does I/O or spawns a server.
        let admin_server_manager = AdminServerManager::new();
        let (admin_command_tx, admin_command_rx) = channel();
        let admin_shared =
            AdminSharedRuntime::new(admin_server_manager.event_bus(), admin_command_tx);
        let system_proxy_port = system_proxy_port();
        let system_proxy_application = SystemProxyApplication::new(system_proxy_port.clone());
        let system_proxy_enabled = read_system_proxy_state()
            .map(|s| s.enabled)
            .unwrap_or(false);

        let state = Self {
            runtime: RuntimeState {
                runtime: None,
                host_composition_failure: None,
                runtime_generation: 0,
                core_session_token: None,
                core_lifecycle: Default::default(),
                mtu: Default::default(),
                ipv6_routing: Default::default(),
                vpn: VpnSessionSnapshot::unsupported(
                    0,
                    "Android VpnService is a mobile-host capability",
                ),
                system_toggles: SystemToggleSnapshot::from_legacy(system_proxy_enabled, None, 0),
                privileged_network: PrivilegedNetworkSnapshot::unsupported(
                    0,
                    "privileged network regression port is not composed for this host",
                ),
                privileged_network_port: None,
                rule_provider_cache_port: None,
                traffic_waveform: Default::default(),
                traffic_scale: Default::default(),
                traffic_topology: Default::default(),
                reconnect_mask: Default::default(),
                active_exit: Default::default(),
                subscription_quota: Default::default(),
                system_proxy: Default::default(),
                system_proxy_recovery: Default::default(),
                system_proxy_port: Some(system_proxy_port),
                system_proxy_application: Some(system_proxy_application),
                system_proxy_last_repair_count: 0,
                lifecycle_token: 0,
                lifecycle_pending: None,
                lifecycle_failure: None,
                status: RuntimeStatus::Stopped,
                proxy_mode: None,
                proxy_mode_state: Default::default(),
                mode_actions: Default::default(),
                mode_read_revision: 0,
                runtime_control: Default::default(),
                script_block_present: false,
                tun_enabled: None,
                tun_service_status: None,
                is_installing_tun_service: false,
                system_proxy_enabled,
                system_proxy_pending: false,
                autostart_enabled: autostart::is_autostart_enabled(crate::AUTOSTART_REG_NAME),
                filter_alive_only: false,
                favorite_proxies: HashSet::new(),
                proxy_compact_view: false,
                inspecting_proxy: None,
                inspection_probe: Default::default(),
                inspection_read: Default::default(),
                is_adding_custom_node: false,
                new_node_type: "ss".to_string(),
                new_node_name: String::new(),
                new_node_server: String::new(),
                new_node_port: "443".to_string(),
                new_node_credential: String::new(),
                new_node_cipher: "aes-256-gcm".to_string(),
                new_node_tls: true,
                installed_kernels: Vec::new(),
                latest_core_version: None,
                core_channel: "stable".to_string(),
                core_versions: Default::default(),
                core_integrity: Default::default(),
                controller_auth: Default::default(),
                service_mode: Default::default(),
                port_conflicts: Default::default(),
                core_resources: Default::default(),
                offline_startup: Default::default(),
                download_progress: 0.0,
                download_stats: None,
                core_download_token: 0,
                core_download_cancel: None,
                is_downloading_core: false,
                is_checking_update: false,
                rebuild_flow: RebuildFlowState::Idle,
                probe_options_editor: Default::default(),
                probe_options_open: false,
                runtime_delay_test_url: "http://www.gstatic.com/generate_204".to_string(),
                runtime_speedtest_url: String::new(),
                runtime_delay_timeout_ms: "5000".to_string(),
                runtime_testing_delay_proxy: String::new(),
                runtime_testing_all_delays: false,
                runtime_selected_group: String::new(),
                runtime_selected_proxy: String::new(),
                runtime_connection_filter: String::new(),
                runtime_connection_sort: "download_desc".to_string(),
                runtime_auto_refresh: true,
                runtime_poll_tick: 0,
                runtime_prev_upload_total: None,
                runtime_prev_download_total: None,
                runtime_prev_snapshot_at: None,
                pending_runtime_patch: None,
                runtime_patch_token: 0,
                proxies: HashMap::new(),
                is_loading_proxies: false,
                proxy_groups: Vec::new(),
                proxy_name_runs: Default::default(),
                proxy_search: Default::default(),
                proxy_preferences: Default::default(),
                filtered_groups: Vec::new(),
                proxy_filter: String::new(),
                proxy_sort_by_delay: false,
                proxy_delay_sort: "delay_asc".to_string(),
                proxy_ui_preferences: ProxyUiPreferences::default(),
                group_order_editor: Default::default(),
                group_order_open: false,
                custom_node_modal_open: false,
                custom_node_uri_input: String::new(),
                custom_node_studio: Default::default(),
                custom_node_inputs: Default::default(),
                custom_node_saving: false,
                network_roaming: Default::default(),
                pac_manager: Default::default(),
                lan_sharing: Default::default(),
                lan_sharing_committed: Default::default(),
                lan_sharing_dirty: false,
                lan_security: Default::default(),
                lan_security_committed: Default::default(),
                lan_security_dirty: false,
                tun_stack_config: Default::default(),
            },
            profile: ProfileState {
                profiles: Vec::new(),
                profiles_filter: String::new(),
                is_loading_profiles: false,
                import_url: String::new(),
                import_name: String::new(),
                import_activate: false,
                is_importing: false,
                local_import_path: String::new(),
                local_import_name: String::new(),
                local_import_activate: false,
                is_importing_local: false,
                subscription_profile_name: String::new(),
                subscription_url: String::new(),
                subscription_auto_update_enabled: false,
                subscription_update_interval_hours: String::new(),
                subscription_cron_expression: String::new(),
                subscription_user_agent: String::new(),
                subscription_insecure_skip_verify: false,
                subscription_auto_reload_core: false,
                is_saving_subscription: false,
                is_updating_subscription_now: false,
                webdav_url: String::new(),
                webdav_user: String::new(),
                webdav_pass: String::new(),
                webdav_enabled: false,
                webdav_sync_interval_mins: "60".to_string(),
                webdav_sync_on_startup: false,
                is_syncing: false,
                sync_progress: None,
                sync_conflicts: Vec::new(),
                is_testing_webdav: false,
                sync_cancel: None,
                sync_from_tick: false,
                is_saving_app_settings: false,
                is_saving_profile: false,
                restart_after_profile_reset: false,
                sync_diff: None,
                is_loading_sync_diff: false,
                is_applying_sync_diff: false,
                aggregator_modal_open: false,
                aggregator_selected_profiles: Vec::new(),
                aggregator_name_input: "Aggregated".to_string(),
                aggregator_report: None,
                aggregator_deduplicate: true,
                aggregator_geo_cluster: true,
                aggregator_generate_groups: true,
                aggregator_remove_emojis: true,
                aggregator_availability_precheck: true,
                aggregator_activate_after_create: false,
                aggregator_renames: String::new(),
                aggregator_custom_name: String::new(),
                aggregator_custom_keywords: String::new(),
                aggregator_custom_groups: Vec::new(),
                aggregator_templates: Vec::new(),
                aggregator_template_name: String::new(),
                is_aggregating: false,
                encrypted_backup: Default::default(),
            },
            editor: ConfigEditorState {
                document_session: Default::default(),
                mixin_session: Default::default(),
                document_latest: None,
                document_load: None,
                mixin_load: None,
                next_editor_read: 0,
                rule_list: Default::default(),
                rules_filter: String::new(),
                is_loading_rules: false,
                rules_loaded_once: false,
                is_saving_rules: false,
                rules_tab: RulesTab::List,
                rules_json_tab: RulesJsonSection::RuleProviders,
                rules_page: 0,
                rules_page_size: 200,
                rules_scroll_offset_px: 0.0,
                rules_viewport_px: RULE_DEFAULT_VIEWPORT_PX,
                rule_trace: Default::default(),
                rules_tracer_input: String::new(),
                rules_tracer_src_ip: String::new(),
                rules_tracer_chain: None,
                rules_tracer_can_reverse_apply: false,
                rules_tracer_suggested_target: None,
                rules_tracer_override_target: String::new(),
                rules_providers_expanded: true,
                rules_render_cache: Vec::new(),
                rules_filtered_indices: Vec::new(),
                rules_heavy_ready: true,
                rule_provider_source_urls: HashMap::new(),
                rule_provider_intervals: HashMap::new(),
                rule_provider_fingerprints: HashMap::new(),
                rule_publish_limit: RULE_PUBLISH_LIMIT,
                rule_publish_omitted: None,
                rule_provider_cache: Default::default(),
                rule_etag_support: Default::default(),
                mrs_acceleration: Default::default(),
                rule_providers_json_content: Content::new(),
                proxy_providers_json_content: Content::new(),
                sniffer_json_content: Content::new(),
                rule_providers_json_cache: "{}".to_string(),
                proxy_providers_json_cache: "{}".to_string(),
                sniffer_json_cache: "{}".to_string(),
                rule_providers_editor_state: EditorLazyState::Unloaded,
                proxy_providers_editor_state: EditorLazyState::Unloaded,
                sniffer_editor_state: EditorLazyState::Unloaded,
                rule_providers_json_dirty: false,
                proxy_providers_json_dirty: false,
                sniffer_json_dirty: false,
                is_saving_rule_providers_json: false,
                is_saving_proxy_providers_json: false,
                is_saving_sniffer_json: false,
                is_updating_geo_databases: false,
                dns_json_content: Content::new(),
                fake_ip_json_content: Content::new(),
                tun_json_content: Content::new(),
                dns_json_cache: "{}".to_string(),
                fake_ip_json_cache: "{}".to_string(),
                tun_json_cache: "{}".to_string(),
                dns_editor_state: EditorLazyState::Unloaded,
                fake_ip_editor_state: EditorLazyState::Unloaded,
                tun_editor_state: EditorLazyState::Unloaded,
                dns_tab: DnsTab::Dns,
                dns_mode: AdvancedEditMode::Form,
                fake_ip_mode: AdvancedEditMode::Form,
                tun_mode: AdvancedEditMode::Form,
                dns_heavy_ready: true,
                advanced_configs_loaded_once: false,
                dns_json_dirty: false,
                fake_ip_json_dirty: false,
                tun_json_dirty: false,
                dns_form: DnsWorkbenchForm {
                    enhanced_mode: DnsEnhancedMode::FakeIp,
                    ..DnsWorkbenchForm::default()
                },
                fake_ip_form: FakeIpFormDraft::default(),
                tun_form: TunFormDraft {
                    stack: "gvisor".to_string(),
                    ..TunFormDraft::default()
                },
                dns_form_dirty: false,
                fake_ip_form_dirty: false,
                tun_form_dirty: false,
                advanced_validation: AdvancedValidationState::default(),
                new_rule_type: "DOMAIN".to_string(),
                rule_form_binding: Default::default(),
                new_rule_payload: String::new(),
                new_rule_target: "DIRECT".to_string(),
                is_adding_rule: false,
                proxy_providers: Vec::new(),
                rule_providers: Vec::new(),
                is_loading_providers: false,
                script_sandbox: Default::default(),
                snapshot_restore: Default::default(),
                script_code_content: Content::new(),
                script_yaml_content: Content::new(),
                snapshot_diff_modal_open: false,
                snapshot_diff_selected_id: None,
                snapshot_diff: None,
                snapshot_diff_mode: Default::default(),
                snapshot_diff_loading: false,
                snapshot_diff_error: None,
                profile_protection_override: false,
                subrule_draft: default_logical_draft(DEFAULT_RULE_TARGET),
                geodata_status: Default::default(),
                rule_hit_audit: Default::default(),
                provider_unpack: Default::default(),
                dns_nameservers: Vec::new(),
                dns_fallback_servers: Vec::new(),
                dns_enhanced_mode: "fake-ip".to_string(),
                dns_fake_ip_pool: Default::default(),
                dns_fake_ip_query: String::new(),
                dns_latency: Default::default(),
                dns_leak: Default::default(),
                dns_stun: Default::default(),
                dns_self_heal: Default::default(),
                is_probing_dns_latency: false,
                dns_hosts_editor: Default::default(),
                is_saving_dns: false,
                is_saving_fake_ip: false,
                is_saving_tun: false,
                tun_stack: "gvisor".to_string(),
                tun_auto_route: false,
                tun_strict_route: false,
                sniffer_enabled: false,
                editor_content: Content::new(),
                profile_viewport: EditorViewport::top(1, 1),
                mixin_viewport: EditorViewport::top(1, 1),
                editor_path: None,
                editor_path_setting: String::new(),
                snapshot_history: None,
                is_backing_up_snapshot: false,
                snapshot_prune_keep: SNAPSHOT_DEFAULT_KEEP,
                is_pruning_snapshots: false,
                apply_transaction: None,
                is_loading_snapshots: false,
                editor_pane: EditorPane::default(),
                mixin_content: Content::new(),
                mixin_loaded_for: None,
                is_saving_mixin: false,
                filter_editor: Default::default(),
                filter_load: None,
                next_filter_load: 0,
                mrs_details: Vec::new(),
                is_scanning_mrs: false,
                syntax_error: None,
                syntax_error_line: None,
                inspecting_rule_provider_diff: None,
                is_loading_rule_provider_diff: false,
            },
            diag: DiagnosticsState {
                traffic: None,
                traffic_history: VecDeque::new(),
                memory: None,
                public_ip: None,
                public_ip_provider: None,
                public_ip_checked_at: None,
                public_ip_error: None,
                connections: None,
                connections_page: 0,
                connections_page_size: 100,
                logs: VecDeque::new(),
                log_level: "info".to_string(),
                fps: 0,
                last_frame_time: Instant::now(),
                topology_flow_phase: 0.0,
                perf_snapshot: PerfSnapshot::default(),
                // ui-fix: the debug perf HUD (FPS badge + snapshot panel, rendered
                // by view_root) starts hidden in production AND demo sessions;
                // Message::TogglePerfPanel flips it back on.
                perf_panel_visible: false,
                perf_nav_started_at: None,
                perf_nav_route: None,
                logs_stream_state: RuntimeStreamState::Idle,
                log_command_failure: None,
                log_search: Default::default(),
                log_export: Default::default(),
                traffic_stream_state: RuntimeStreamState::Idle,
                connections_stream_state: RuntimeStreamState::Idle,
                doctor: DoctorPanelState::default(),
                inspecting_connection_id: None,
                dns_cache_actions: Default::default(),
                dns_query: Default::default(),
                dns_cache_flush: DnsCacheFlushReport::default(),
                is_probing_dns_leak: false,
                dns_leak_action: Default::default(),
                dns_leak_capture: None,
                is_probing_stun: false,
                pcap_state: Default::default(),
                speedtest: Default::default(),
                speedtest_detail_open: false,
                overview_card_order: OverviewCardKind::DEFAULT_ORDER.to_vec(),
                crash_watchdog: Default::default(),
                log_filter: Default::default(),
                connection_groups: Default::default(),
                connection_activity: ConnectionActivityTracker::new(),
                connection_rates: ConnectionRateApplication::new(),
                connection_rate_book: Default::default(),
                connection_pulse_phase: 0.0,
                connection_idle_timeout_secs: DEFAULT_IDLE_TIMEOUT_SECS,
                last_idle_sweep: None,
            },
            shell: ShellState {
                current_route: Route::Overview,
                history: RouteHistory::default(),
                viewport: ResponsiveViewportSnapshot::default(),
                transition: Transition::default(),
                error_msg: None,
                lang: get_system_language(),
                language_choice: Default::default(),
                language_user_selected: false,
                toasts: Vec::new(),
                toast_ids: Vec::new(),
                toast_gate: ToastGate::default(),
                toast_epoch: Instant::now(),
                next_toast_id: 1,
                theme: iced::Theme::Dark,
                tray_controller: None,
                tray_events: None,
                // Admin defaults: embedded server on at port 25210 (API-only
                // since the 0.20 WebUI retirement); the
                // real values are applied from settings in `SettingsLoaded`.
                admin_enabled: true,
                admin_port: ADMIN_DEFAULT_PORT,
                admin_port_input: ADMIN_DEFAULT_PORT.to_string(),
                admin_server: admin_server_manager,
                admin_shared,
                admin_commands: Some(Arc::new(Mutex::new(admin_command_rx))),
                is_admin: {
                    #[cfg(windows)]
                    {
                        is_elevated::is_elevated()
                    }
                    #[cfg(not(windows))]
                    {
                        false
                    }
                },
                notifications_enabled: true,
                close_to_tray: true,
                system_proxy_bypass: String::new(),
                last_task_id: 0,
                tray_refresh_cooldown: None,
                tray_last_rate_text: None,
                // demo-mode: production default is a non-demo session with no
                // capture marker (see demo.rs for the demo boot path).
                demo: false,
                confirmation: None,
                is_factory_resetting: false,
                capture_marker: None,
                capture_region_bounds: None,
                capture_scenario: None,
                capture_marker_written: AtomicBool::new(false),
                capture_frame: Default::default(),
                readout: Default::default(),
                command_palette_open: false,
                command_query: String::new(),
                command_selected_index: 0,
                command_catalogue: CommandCatalogue::new(),
                mini_hud_placement: MiniHudPlacement::default(),
                mini_hud_drag_anchor: None,
                mini_hud_display: None,
                window_id: None,
                mini_hud_mode: false,
                always_on_top: false,
                window_focused: true,
                theme_preference: ThemePreference::System,
                system_prefers_dark: true,
                shortcut_registry: ShortcutRegistry::with_defaults(),
                hotkey_capture: None,
                ime: ImeCompositionTracker::new(),
                uwp_loopback: Default::default(),
            },
            app_routing: Default::default(),
            surface: Default::default(),
            surface_bridge: None,
            commands: None,
            exit_cleanup: None,
        };
        // DUAL-15-04: bind the Iced window handle into the desktop host port
        // so a persisted HUD placement reaches this window once its id is
        // resolved (before that the adapter answers typed unsupported).
        install_host_handle();
        state
    }

    pub fn new() -> (Self, Task<Message>) {
        // `mut` is only consumed by the tray spawn block, which is absent
        // from the test build by design (tests never spawn a tray).
        #[cfg_attr(test, allow(unused_mut))]
        let mut state = Self::empty();

        // Startup: try the system tray; on Unavailable continue window-only
        // with a warning. Never spawn a real tray in unit tests.
        #[cfg(not(test))]
        match tray::spawn(state.current_tray_spec()) {
            TrayStartup::Ready { controller, events } => {
                state.shell.tray_controller = Some(controller);
                state.shell.tray_events = Some(Arc::new(Mutex::new(events)));
            }
            TrayStartup::Unavailable { reason } => {
                eprintln!("system tray unavailable, continuing window-only: {reason}");
            }
        }

        // INFILTRATOR_LANG 会话级覆写：注入初始语言；SettingsLoaded 回灌时
        // 再剥离设置文件里的 language 让该值存活（见 env_lang_override）。
        let lang_override = env_lang_override();
        if let Some(lang) = lang_override.clone() {
            state.shell.lang = lang;
        }
        let system_proxy_application = state.runtime.system_proxy_application.clone();

        (
            state,
            Task::batch(vec![
                Task::perform(
                    async move {
                        match system_proxy_application {
                            Some(application) => application.recover_orphaned().await,
                            None => Default::default(),
                        }
                    },
                    Message::SystemProxyRecoveryFinished,
                ),
                Task::perform(
                    async { load_hydrated().await },
                    // env 覆写生效时清空回灌快照的 language 字段（仅内存
                    // 快照），apply_loaded_settings 因此保留 env 注入值；
                    // 磁盘上的设置文件不动。
                    move |result| {
                        Message::SettingsLoaded(result.map(|mut settings| {
                            if lang_override.is_some() {
                                settings.language.clear();
                            }
                            settings
                        }))
                    },
                ),
                Task::perform(
                    async {
                        let store = config_manager().await?;
                        ProfileApplication::new(store)
                            .list_profiles()
                            .await
                            .map_err(|failure| InfiltratorError::Config(failure.message))
                    },
                    Message::ProfilesLoaded,
                ),
                Task::done(Message::LoadKernels),
                // 单窗口桌面宿主：启动即解析窗口 id，Mini HUD 的移动/置顶
                // 任务需要它（未解析前所有窗口任务都是 no-op）。
                latest().map(Message::WindowIdResolved),
                // 启动即读取 OS 外观：`system` 偏好下冷启动就与系统一致。
                theme().map(|mode| Message::SystemThemeChanged(matches!(mode, Mode::Dark))),
                // desktop-smoke 钩子（仅测试用）：INFILTRATOR_FORCE_NOTIFY=1
                // 时启动即发一条探针通知，见 notify.rs 模块文档。
                startup_probe_task(),
            ]),
        )
    }
}
