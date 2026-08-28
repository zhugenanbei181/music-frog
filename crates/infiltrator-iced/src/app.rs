use crate::autostart;
use crate::locales::{Lang, Localizer, get_system_language};
use crate::state::AppState;
use crate::tray::TrayManager;
use crate::types::{
    AdvancedEditMode, DnsFormDraft, DnsTab, EditorLazyState, FakeIpFormDraft, InfiltratorError,
    Message, RebuildFlowState, Route, RulesJsonTab, RulesTab, RuntimeStatus, TunFormDraft,
};
use iced::Task;
use mihomo_config::ConfigManager;

impl AppState {
    pub fn title(&self) -> String {
        Lang(&self.lang).tr("app_title").to_string()
    }

    pub fn theme(&self) -> iced::Theme {
        self.theme.clone()
    }

    pub fn new() -> (Self, Task<Message>) {
        (
            Self {
                current_route: Route::Overview,
                runtime: None,
                status: RuntimeStatus::Stopped,
                error_msg: None,
                profiles: Vec::new(),
                profiles_filter: String::new(),
                is_loading_profiles: false,
                proxies: std::collections::HashMap::new(),
                is_loading_proxies: false,
                filtered_groups: Vec::new(),
                transition: crate::types::Transition::default(),
                proxy_filter: String::new(),
                proxy_sort_by_delay: false,
                proxy_delay_sort: "delay_asc".to_string(),
                runtime_delay_test_url: "http://www.gstatic.com/generate_204".to_string(),
                runtime_delay_timeout_ms: "5000".to_string(),
                runtime_testing_delay_proxy: String::new(),
                runtime_testing_all_delays: false,
                runtime_selected_group: String::new(),
                runtime_selected_proxy: String::new(),
                runtime_connection_filter: String::new(),
                runtime_connection_sort: "download_desc".to_string(),
                traffic: None,
                traffic_history: std::collections::VecDeque::new(),
                runtime_prev_upload_total: None,
                runtime_prev_download_total: None,
                memory: None,
                public_ip: None,
                connections: None,
                logs: std::collections::VecDeque::new(),
                log_level: "info".to_string(),
                runtime_auto_refresh: true,
                runtime_poll_tick: 0,
                lang: get_system_language(),
                proxy_mode: None,
                tun_enabled: None,
                tun_stack: "gvisor".to_string(),
                tun_auto_route: false,
                tun_strict_route: false,
                sniffer_enabled: false,
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
                rules_providers_expanded: false,
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
                advanced_validation: crate::types::AdvancedValidationState::default(),
                new_rule_type: "DOMAIN".to_string(),
                new_rule_payload: String::new(),
                new_rule_target: "DIRECT".to_string(),
                is_adding_rule: false,
                proxy_providers: Vec::new(),
                rule_providers: Vec::new(),
                is_loading_providers: false,
                tray_manager: Some(TrayManager::new()),
                dns_nameservers: Vec::new(),
                dns_fallback_servers: Vec::new(),
                dns_enhanced_mode: "fake-ip".to_string(),
                is_saving_dns: false,
                is_saving_fake_ip: false,
                is_saving_tun: false,
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
                is_saving_subscription: false,
                is_updating_subscription_now: false,
                webdav_url: String::new(),
                webdav_user: String::new(),
                webdav_pass: String::new(),
                webdav_enabled: false,
                webdav_sync_interval_mins: "60".to_string(),
                webdav_sync_on_startup: false,
                is_syncing: false,
                is_saving_app_settings: false,
                is_admin: is_elevated::is_elevated(),
                system_proxy_enabled: infiltrator_desktop::proxy::read_system_proxy_state()
                    .map(|s| s.enabled)
                    .unwrap_or(false),
                autostart_enabled: autostart::is_autostart_enabled(),
                installed_kernels: Vec::new(),
                latest_core_version: None,
                download_progress: 0.0,
                is_checking_update: false,
                last_task_id: 0,
                toasts: Vec::new(),
                rebuild_flow: RebuildFlowState::Idle,
                theme: iced::Theme::Dark,
                fps: 0,
                last_frame_time: std::time::Instant::now(),
                perf_snapshot: crate::types::PerfSnapshot::default(),
                perf_panel_visible: false,
                perf_nav_started_at: None,
                perf_nav_route: None,
                editor_content: iced::widget::text_editor::Content::new(),
                editor_path: None,
                editor_path_setting: String::new(),
            },
            Task::batch(vec![
                Task::perform(
                    async {
                        let data_dir = mihomo_platform::get_home_dir().unwrap_or_default();
                        let path = infiltrator_core::settings::settings_path(&data_dir)
                            .unwrap_or_else(|_| data_dir.join("settings.toml"));
                        infiltrator_core::settings::load_settings(&path)
                            .await
                            .map_err(|e| InfiltratorError::Config(e.to_string()))
                    },
                    Message::SettingsLoaded,
                ),
                Task::perform(
                    async {
                        let cm = ConfigManager::new().map_err(InfiltratorError::from)?;
                        cm.list_profiles().await.map_err(InfiltratorError::from)
                    },
                    Message::ProfilesLoaded,
                ),
                Task::done(Message::LoadKernels),
            ]),
        )
    }
}
