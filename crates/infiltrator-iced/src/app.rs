use crate::state::AppState;
use infiltrator_shared::autostart;
use infiltrator_shared::locales::{Lang, Localizer, get_system_language};
// Only the non-test build spawns a real system tray (ksni/muda must never run
// in unit tests), so the import is test-gated.
#[cfg(not(test))]
use crate::tray;
#[cfg(not(test))]
use crate::tray::spec::TrayStartup;
use crate::types::app::Route;
use crate::types::dns::{AdvancedEditMode, DnsFormDraft, DnsTab, FakeIpFormDraft, TunFormDraft};
use crate::types::editor::EditorLazyState;
use crate::types::message::Message;
use crate::types::rules::{RulesJsonTab, RulesTab};
use crate::types::runtime::{RebuildFlowState, RuntimeStatus};
use iced::Task;
use infiltrator_core::error::InfiltratorError;
use std::sync::{Arc, Mutex};

/// `INFILTRATOR_LANG` 会话级语言覆写（非 demo 启动路径；demo 有自己的同名
/// 契约，见 demo.rs）。只接受 `zh-CN` / `en-US`，其它值忽略。
///
/// 语义：**仅会话级** —— 启动时注入内存语言，并在设置加载回灌时剥离
/// settings.toml 的 `language` 字段让 env 值存活；绝不写盘、绝不修改用户
/// 的设置文件，用户仍可在会话内用设置页改语言（保存语义不变）。
fn env_lang_override() -> Option<String> {
    let value = std::env::var("INFILTRATOR_LANG").ok()?;
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
        let admin_server_manager = crate::admin_server::AdminServerManager::new();
        let (admin_command_tx, admin_command_rx) = std::sync::mpsc::channel();
        let admin_shared = crate::admin_server::AdminSharedRuntime::new(
            admin_server_manager.event_bus(),
            admin_command_tx,
        );

        Self {
            runtime: crate::state::RuntimeState {
                runtime: None,
                runtime_generation: 0,
                lifecycle_token: 0,
                status: RuntimeStatus::Stopped,
                proxy_mode: None,
                script_block_present: false,
                tun_enabled: None,
                tun_service_status: None,
                is_installing_tun_service: false,
                system_proxy_enabled: infiltrator_desktop::proxy::read_system_proxy_state()
                    .map(|s| s.enabled)
                    .unwrap_or(false),
                system_proxy_pending: false,
                autostart_enabled: autostart::is_autostart_enabled(crate::AUTOSTART_REG_NAME),
                filter_alive_only: false,
                favorite_proxies: std::collections::HashSet::new(),
                proxy_compact_view: false,
                inspecting_proxy: None,
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
                download_progress: 0.0,
                download_stats: None,
                core_download_token: 0,
                core_download_cancel: None,
                is_downloading_core: false,
                is_checking_update: false,
                rebuild_flow: RebuildFlowState::Idle,
                runtime_delay_test_url: "http://www.gstatic.com/generate_204".to_string(),
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
                proxies: std::collections::HashMap::new(),
                is_loading_proxies: false,
                filtered_groups: Vec::new(),
                proxy_filter: String::new(),
                proxy_sort_by_delay: false,
                proxy_delay_sort: "delay_asc".to_string(),
                // ui-wave2-p: proxies page expand/collapse state starts pristine
                // (None = view expands the first group by default).
                proxy_groups_expanded: None,
            },
            profile: crate::state::ProfileState {
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
                subscription_user_agent: String::new(),
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
            },
            editor: crate::state::ConfigEditorState {
                rules: Vec::new(),
                rules_filter: String::new(),
                is_loading_rules: false,
                rules_loaded_once: false,
                is_saving_rules: false,
                rules_dirty: false,
                rules_tab: RulesTab::RulesList,
                rules_json_tab: RulesJsonTab::RuleProviders,
                rules_page: 0,
                rules_page_size: 200,
                rules_tracer_input: String::new(),
                rules_tracer_result: None,
                rules_providers_expanded: true,
                rules_render_cache: Vec::new(),
                rules_filtered_indices: Vec::new(),
                rules_heavy_ready: true,
                rule_providers_json_content: iced::widget::text_editor::Content::new(),
                proxy_providers_json_content: iced::widget::text_editor::Content::new(),
                sniffer_json_content: iced::widget::text_editor::Content::new(),
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
                dns_json_content: iced::widget::text_editor::Content::new(),
                fake_ip_json_content: iced::widget::text_editor::Content::new(),
                tun_json_content: iced::widget::text_editor::Content::new(),
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
                dns_form: DnsFormDraft {
                    enhanced_mode: "fake-ip".to_string(),
                    ..DnsFormDraft::default()
                },
                fake_ip_form: FakeIpFormDraft::default(),
                tun_form: TunFormDraft {
                    stack: "gvisor".to_string(),
                    ..TunFormDraft::default()
                },
                dns_form_dirty: false,
                fake_ip_form_dirty: false,
                tun_form_dirty: false,
                advanced_validation: crate::types::dns::AdvancedValidationState::default(),
                new_rule_type: "DOMAIN".to_string(),
                new_rule_payload: String::new(),
                new_rule_target: "DIRECT".to_string(),
                is_adding_rule: false,
                proxy_providers: Vec::new(),
                rule_providers: Vec::new(),
                is_loading_providers: false,
                dns_nameservers: Vec::new(),
                dns_fallback_servers: Vec::new(),
                dns_enhanced_mode: "fake-ip".to_string(),
                is_saving_dns: false,
                is_saving_fake_ip: false,
                is_saving_tun: false,
                tun_stack: "gvisor".to_string(),
                tun_auto_route: false,
                tun_strict_route: false,
                sniffer_enabled: false,
                editor_content: iced::widget::text_editor::Content::new(),
                editor_path: None,
                editor_path_setting: String::new(),
                profile_snapshots: Vec::new(),
                is_loading_snapshots: false,
                is_restoring_snapshot: false,
                editor_pane: crate::types::options::EditorPane::default(),
                mixin_content: iced::widget::text_editor::Content::new(),
                mixin_loaded_for: None,
                is_saving_mixin: false,
                filter_draft: crate::types::options::FilterDraft::default(),
                filter_loaded_for: None,
                is_saving_filter: false,
                mrs_details: Vec::new(),
                is_scanning_mrs: false,
                syntax_error: None,
                syntax_error_line: None,
                inspecting_rule_provider_diff: None,
                is_loading_rule_provider_diff: false,
            },
            diag: crate::state::DiagnosticsState {
                traffic: None,
                traffic_history: std::collections::VecDeque::new(),
                memory: None,
                public_ip: None,
                public_ip_provider: None,
                public_ip_checked_at: None,
                public_ip_error: None,
                connections: None,
                connections_page: 0,
                connections_page_size: 100,
                logs: std::collections::VecDeque::new(),
                log_level: "info".to_string(),
                fps: 0,
                last_frame_time: std::time::Instant::now(),
                perf_snapshot: crate::types::perf::PerfSnapshot::default(),
                // ui-fix: the debug perf HUD (FPS badge + snapshot panel, rendered
                // by view_root) starts hidden in production AND demo sessions;
                // Message::TogglePerfPanel flips it back on.
                perf_panel_visible: false,
                perf_nav_started_at: None,
                perf_nav_route: None,
                logs_stream_state: crate::types::runtime::RuntimeStreamState::Idle,
                traffic_stream_state: crate::types::runtime::RuntimeStreamState::Idle,
                connections_stream_state: crate::types::runtime::RuntimeStreamState::Idle,
                doctor: crate::types::doctor::DoctorPanelState::default(),
            },
            shell: crate::state::ShellState {
                current_route: Route::Overview,
                transition: crate::types::app::Transition::default(),
                error_msg: None,
                lang: get_system_language(),
                toasts: Vec::new(),
                theme: iced::Theme::Dark,
                tray_controller: None,
                tray_events: None,
                // Admin defaults: embedded server on at port 25210 (API-only
                // since the 0.20 WebUI retirement); the
                // real values are applied from settings in `SettingsLoaded`.
                admin_enabled: true,
                admin_port: crate::admin_server::ADMIN_DEFAULT_PORT,
                admin_port_input: crate::admin_server::ADMIN_DEFAULT_PORT.to_string(),
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
                // demo-mode: production default is a non-demo session with no
                // capture marker (see demo.rs for the demo boot path).
                demo: false,
                confirmation: None,
                is_factory_resetting: false,
                capture_marker: None,
                capture_marker_written: std::sync::atomic::AtomicBool::new(false),
            },
        }
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

        (
            state,
            Task::batch(vec![
                Task::perform(
                    async {
                        let data_dir = mihomo_platform::paths::get_home_dir().unwrap_or_default();
                        let path = infiltrator_core::settings::settings_path(&data_dir)
                            .unwrap_or_else(|_| data_dir.join("settings.toml"));
                        // hydrated：顺带从 keyring 取回 WebDAV 密码填充内存
                        // 镜像（磁盘序列化跳过该字段，不会因此落盘）。
                        infiltrator_core::settings::load_settings_hydrated(&path)
                            .await
                            .map_err(|e| InfiltratorError::Config(e.to_string()))
                    },
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
                        let cm = crate::configs_dir::config_manager().await?;
                        cm.list_profiles().await.map_err(InfiltratorError::from)
                    },
                    Message::ProfilesLoaded,
                ),
                Task::done(Message::LoadKernels),
                // desktop-smoke 钩子（仅测试用）：INFILTRATOR_FORCE_NOTIFY=1
                // 时启动即发一条探针通知，见 notify.rs 模块文档。
                crate::notify::startup_probe_task(),
            ]),
        )
    }
}
