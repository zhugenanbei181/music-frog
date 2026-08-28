use crate::autostart;
use crate::state::AppState;
use crate::types::{
    AdvancedConfigsBundle, AdvancedEditMode, DnsFormDraft, DnsTab, EditorLazyState,
    FakeIpFormDraft, InfiltratorError, Message, RebuildFlowState, RuleBadgeKind, RuleRenderItem,
    RulesJsonTab, RulesLoadBundle, RulesTab, RuntimeConfig, RuntimeStatus, ToastStatus,
    TunFormDraft,
};
use iced::{Task, stream};
use infiltrator_core::rules::RuleEntry;
use infiltrator_core::settings::{AppSettings, RuntimePanelConfig};
use infiltrator_desktop::MihomoRuntime;
use mihomo_version::{VersionManager, channel::Channel};
use std::sync::Arc;

const DEFAULT_RUNTIME_DELAY_TEST_URL: &str = "http://www.gstatic.com/generate_204";
const DEFAULT_RUNTIME_DELAY_TIMEOUT_MS: u32 = 5000;
const MIN_RUNTIME_DELAY_TIMEOUT_MS: u32 = 100;
const MAX_RUNTIME_DELAY_TIMEOUT_MS: u32 = 60_000;
const DEFAULT_RUNTIME_CONNECTION_SORT: &str = "download_desc";

impl AppState {
    fn split_rule_parts(rule: &str) -> (String, String, String) {
        let mut parts = rule.splitn(3, ',');
        let rule_type = parts.next().unwrap_or("").trim().to_string();
        let payload = parts.next().unwrap_or("").trim().to_string();
        let target = parts.next().unwrap_or("").trim().to_string();
        (rule_type, payload, target)
    }

    fn rule_badge_kind(rule_type: &str) -> RuleBadgeKind {
        match rule_type {
            "DOMAIN" | "DOMAIN-SUFFIX" | "DOMAIN-KEYWORD" => RuleBadgeKind::Domain,
            "IP-CIDR" | "IP-CIDR6" | "GEOIP" => RuleBadgeKind::Ip,
            _ => RuleBadgeKind::Other,
        }
    }

    fn rebuild_rules_render_cache(&mut self) {
        let start = std::time::Instant::now();
        self.rules_render_cache = self
            .rules
            .iter()
            .enumerate()
            .map(|(index, entry)| {
                let (rule_type, payload, target) = Self::split_rule_parts(&entry.rule);
                RuleRenderItem {
                    source_index: index,
                    search_key: entry.rule.to_lowercase(),
                    badge: Self::rule_badge_kind(&rule_type),
                    rule_type,
                    payload,
                    target,
                }
            })
            .collect();
        self.perf_snapshot.rules_cache_build_ms = start.elapsed().as_millis();
    }

    fn apply_rules_filter(&mut self) {
        let filter = self.rules_filter.trim().to_ascii_lowercase();
        self.rules_filtered_indices = if filter.is_empty() {
            (0..self.rules_render_cache.len()).collect()
        } else {
            self.rules_render_cache
                .iter()
                .enumerate()
                .filter_map(|(cache_index, item)| {
                    if item.search_key.contains(&filter) {
                        Some(cache_index)
                    } else {
                        None
                    }
                })
                .collect()
        };
        if self.rules_page_size == 0 {
            self.rules_page_size = 200;
        }
        let total_pages = if self.rules_filtered_indices.is_empty() {
            1
        } else {
            (self.rules_filtered_indices.len() - 1) / self.rules_page_size + 1
        };
        if self.rules_page >= total_pages {
            self.rules_page = total_pages.saturating_sub(1);
        }
        let start = self.rules_page.saturating_mul(self.rules_page_size);
        self.perf_snapshot.rules_visible_rows = self
            .rules_filtered_indices
            .len()
            .saturating_sub(start)
            .min(self.rules_page_size);
    }

    fn reset_rules_lazy_state(&mut self) {
        self.rule_providers_editor_state = EditorLazyState::Unloaded;
        self.proxy_providers_editor_state = EditorLazyState::Unloaded;
        self.sniffer_editor_state = EditorLazyState::Unloaded;
    }

    fn reset_dns_lazy_state(&mut self) {
        self.dns_editor_state = EditorLazyState::Unloaded;
        self.fake_ip_editor_state = EditorLazyState::Unloaded;
        self.tun_editor_state = EditorLazyState::Unloaded;
    }

    fn ensure_rule_providers_editor_loaded(&mut self) {
        if self.rule_providers_editor_state == EditorLazyState::Loaded
            && self.rule_providers_json_content.text() == self.rule_providers_json_cache
        {
            return;
        }
        let start = std::time::Instant::now();
        self.rule_providers_json_content =
            iced::widget::text_editor::Content::with_text(&self.rule_providers_json_cache);
        self.rule_providers_editor_state = EditorLazyState::Loaded;
        self.perf_snapshot.rules_with_text_apply_ms = start.elapsed().as_millis();
    }

    fn ensure_proxy_providers_editor_loaded(&mut self) {
        if self.proxy_providers_editor_state == EditorLazyState::Loaded
            && self.proxy_providers_json_content.text() == self.proxy_providers_json_cache
        {
            return;
        }
        let start = std::time::Instant::now();
        self.proxy_providers_json_content =
            iced::widget::text_editor::Content::with_text(&self.proxy_providers_json_cache);
        self.proxy_providers_editor_state = EditorLazyState::Loaded;
        self.perf_snapshot.rules_with_text_apply_ms = start.elapsed().as_millis();
    }

    fn ensure_sniffer_editor_loaded(&mut self) {
        if self.sniffer_editor_state == EditorLazyState::Loaded
            && self.sniffer_json_content.text() == self.sniffer_json_cache
        {
            return;
        }
        let start = std::time::Instant::now();
        self.sniffer_json_content =
            iced::widget::text_editor::Content::with_text(&self.sniffer_json_cache);
        self.sniffer_editor_state = EditorLazyState::Loaded;
        self.perf_snapshot.rules_with_text_apply_ms = start.elapsed().as_millis();
    }

    fn ensure_dns_editor_loaded(&mut self) {
        if self.dns_editor_state == EditorLazyState::Loaded
            && self.dns_json_content.text() == self.dns_json_cache
        {
            return;
        }
        let start = std::time::Instant::now();
        self.dns_json_content = iced::widget::text_editor::Content::with_text(&self.dns_json_cache);
        self.dns_editor_state = EditorLazyState::Loaded;
        self.perf_snapshot.dns_with_text_apply_ms = start.elapsed().as_millis();
    }

    fn ensure_fake_ip_editor_loaded(&mut self) {
        if self.fake_ip_editor_state == EditorLazyState::Loaded
            && self.fake_ip_json_content.text() == self.fake_ip_json_cache
        {
            return;
        }
        let start = std::time::Instant::now();
        self.fake_ip_json_content =
            iced::widget::text_editor::Content::with_text(&self.fake_ip_json_cache);
        self.fake_ip_editor_state = EditorLazyState::Loaded;
        self.perf_snapshot.dns_with_text_apply_ms = start.elapsed().as_millis();
    }

    fn ensure_tun_editor_loaded(&mut self) {
        if self.tun_editor_state == EditorLazyState::Loaded
            && self.tun_json_content.text() == self.tun_json_cache
        {
            return;
        }
        let start = std::time::Instant::now();
        self.tun_json_content = iced::widget::text_editor::Content::with_text(&self.tun_json_cache);
        self.tun_editor_state = EditorLazyState::Loaded;
        self.perf_snapshot.dns_with_text_apply_ms = start.elapsed().as_millis();
    }

    fn split_list_field(raw: &str) -> Vec<String> {
        raw.split(|ch| ch == ',' || ch == '\n' || ch == '\r')
            .map(str::trim)
            .filter(|item| !item.is_empty())
            .map(str::to_string)
            .collect()
    }

    fn join_list_field(values: &Option<Vec<String>>) -> String {
        values
            .as_ref()
            .map(|items| items.join(", "))
            .unwrap_or_default()
    }

    fn map_advanced_error_message(error: &InfiltratorError) -> String {
        let message = error.to_string();
        if message.contains("unsupported tun stack") {
            return "TUN stack must be 'system' or 'gvisor'".to_string();
        }
        if message.contains("unsupported enhanced-mode") {
            return "DNS enhanced mode must be 'fake-ip' or 'redir-host'".to_string();
        }
        if message.contains("mtu must be greater than 0") {
            return "TUN MTU must be greater than 0".to_string();
        }
        if message.contains("contains empty entry") {
            return "List fields cannot contain empty items".to_string();
        }
        message
    }

    fn apply_dns_form_from_config(&mut self, config: &infiltrator_core::dns::DnsConfig) {
        self.dns_form = DnsFormDraft {
            enable: config.enable.unwrap_or(false),
            nameserver: Self::join_list_field(&config.nameserver),
            fallback: Self::join_list_field(&config.fallback),
            enhanced_mode: config
                .enhanced_mode
                .clone()
                .unwrap_or_else(|| "fake-ip".to_string()),
            fake_ip_range: config.fake_ip_range.clone().unwrap_or_default(),
            fake_ip_filter: Self::join_list_field(&config.fake_ip_filter),
            ipv6: config.ipv6.unwrap_or(false),
            cache: config.cache.unwrap_or(false),
            use_hosts: config.use_hosts.unwrap_or(false),
            use_system_hosts: config.use_system_hosts.unwrap_or(false),
            respect_rules: config.respect_rules.unwrap_or(false),
            proxy_server_nameserver: Self::join_list_field(&config.proxy_server_nameserver),
            direct_nameserver: Self::join_list_field(&config.direct_nameserver),
        };
    }

    fn apply_fake_ip_form_from_config(&mut self, config: &infiltrator_core::fake_ip::FakeIpConfig) {
        self.fake_ip_form = FakeIpFormDraft {
            fake_ip_range: config.fake_ip_range.clone().unwrap_or_default(),
            fake_ip_filter: Self::join_list_field(&config.fake_ip_filter),
            store_fake_ip: config.store_fake_ip.unwrap_or(false),
        };
    }

    fn apply_tun_form_from_config(&mut self, config: &infiltrator_core::tun::TunConfig) {
        self.tun_form = TunFormDraft {
            enable: config.enable.unwrap_or(false),
            stack: config.stack.clone().unwrap_or_else(|| "gvisor".to_string()),
            mtu: config
                .mtu
                .map(|value| value.to_string())
                .unwrap_or_default(),
            dns_hijack: Self::join_list_field(&config.dns_hijack),
            auto_route: config.auto_route.unwrap_or(false),
            auto_detect_interface: config.auto_detect_interface.unwrap_or(false),
            strict_route: config.strict_route.unwrap_or(false),
        };
    }

    fn dns_patch_from_form(
        &self,
    ) -> Result<infiltrator_core::dns::DnsConfigPatch, InfiltratorError> {
        let enhanced_mode = self.dns_form.enhanced_mode.trim().to_ascii_lowercase();
        if !enhanced_mode.is_empty() && enhanced_mode != "fake-ip" && enhanced_mode != "redir-host"
        {
            return Err(InfiltratorError::Config(
                "enhanced_mode must be fake-ip or redir-host".to_string(),
            ));
        }

        let fake_ip_range = self.dns_form.fake_ip_range.trim();
        Ok(infiltrator_core::dns::DnsConfigPatch {
            enable: Some(self.dns_form.enable),
            nameserver: Some(Self::split_list_field(&self.dns_form.nameserver)),
            fallback: Some(Self::split_list_field(&self.dns_form.fallback)),
            enhanced_mode: if enhanced_mode.is_empty() {
                None
            } else {
                Some(enhanced_mode)
            },
            fake_ip_range: if fake_ip_range.is_empty() {
                None
            } else {
                Some(fake_ip_range.to_string())
            },
            fake_ip_filter: Some(Self::split_list_field(&self.dns_form.fake_ip_filter)),
            ipv6: Some(self.dns_form.ipv6),
            cache: Some(self.dns_form.cache),
            use_hosts: Some(self.dns_form.use_hosts),
            use_system_hosts: Some(self.dns_form.use_system_hosts),
            respect_rules: Some(self.dns_form.respect_rules),
            proxy_server_nameserver: Some(Self::split_list_field(
                &self.dns_form.proxy_server_nameserver,
            )),
            direct_nameserver: Some(Self::split_list_field(&self.dns_form.direct_nameserver)),
            ..infiltrator_core::dns::DnsConfigPatch::default()
        })
    }

    fn fake_ip_patch_from_form(
        &self,
    ) -> Result<infiltrator_core::fake_ip::FakeIpConfigPatch, InfiltratorError> {
        let fake_ip_range = self.fake_ip_form.fake_ip_range.trim();
        Ok(infiltrator_core::fake_ip::FakeIpConfigPatch {
            fake_ip_range: if fake_ip_range.is_empty() {
                None
            } else {
                Some(fake_ip_range.to_string())
            },
            fake_ip_filter: Some(Self::split_list_field(&self.fake_ip_form.fake_ip_filter)),
            store_fake_ip: Some(self.fake_ip_form.store_fake_ip),
        })
    }

    fn tun_patch_from_form(
        &self,
    ) -> Result<infiltrator_core::tun::TunConfigPatch, InfiltratorError> {
        let stack = self.tun_form.stack.trim().to_ascii_lowercase();
        if !stack.is_empty() && stack != "system" && stack != "gvisor" {
            return Err(InfiltratorError::Config(
                "stack must be system or gvisor".to_string(),
            ));
        }

        let mtu_text = self.tun_form.mtu.trim();
        let mtu = if mtu_text.is_empty() {
            None
        } else {
            Some(mtu_text.parse::<u32>().map_err(|_| {
                InfiltratorError::Config("mtu must be a positive integer".to_string())
            })?)
        };
        if matches!(mtu, Some(0)) {
            return Err(InfiltratorError::Config(
                "mtu must be greater than 0".to_string(),
            ));
        }

        Ok(infiltrator_core::tun::TunConfigPatch {
            enable: Some(self.tun_form.enable),
            stack: if stack.is_empty() { None } else { Some(stack) },
            mtu,
            dns_hijack: Some(Self::split_list_field(&self.tun_form.dns_hijack)),
            auto_route: Some(self.tun_form.auto_route),
            auto_detect_interface: Some(self.tun_form.auto_detect_interface),
            strict_route: Some(self.tun_form.strict_route),
        })
    }

    fn sync_dns_json_from_form(&mut self) -> Result<(), InfiltratorError> {
        let patch = self.dns_patch_from_form()?;
        self.dns_json_cache = serde_json::to_string_pretty(&patch)
            .map_err(|e| InfiltratorError::Config(e.to_string()))?;
        if self.dns_editor_state == EditorLazyState::Loaded && !self.dns_json_dirty {
            self.ensure_dns_editor_loaded();
        }
        Ok(())
    }

    fn sync_fake_ip_json_from_form(&mut self) -> Result<(), InfiltratorError> {
        let patch = self.fake_ip_patch_from_form()?;
        self.fake_ip_json_cache = serde_json::to_string_pretty(&patch)
            .map_err(|e| InfiltratorError::Config(e.to_string()))?;
        if self.fake_ip_editor_state == EditorLazyState::Loaded && !self.fake_ip_json_dirty {
            self.ensure_fake_ip_editor_loaded();
        }
        Ok(())
    }

    fn sync_tun_json_from_form(&mut self) -> Result<(), InfiltratorError> {
        let patch = self.tun_patch_from_form()?;
        self.tun_json_cache = serde_json::to_string_pretty(&patch)
            .map_err(|e| InfiltratorError::Config(e.to_string()))?;
        if self.tun_editor_state == EditorLazyState::Loaded && !self.tun_json_dirty {
            self.ensure_tun_editor_loaded();
        }
        Ok(())
    }

    fn mark_dns_form_dirty_and_sync(&mut self) {
        self.dns_form_dirty = true;
        match self.sync_dns_json_from_form() {
            Ok(_) => self.advanced_validation.dns = None,
            Err(e) => self.advanced_validation.dns = Some(Self::map_advanced_error_message(&e)),
        }
    }

    fn mark_fake_ip_form_dirty_and_sync(&mut self) {
        self.fake_ip_form_dirty = true;
        match self.sync_fake_ip_json_from_form() {
            Ok(_) => self.advanced_validation.fake_ip = None,
            Err(e) => self.advanced_validation.fake_ip = Some(Self::map_advanced_error_message(&e)),
        }
    }

    fn mark_tun_form_dirty_and_sync(&mut self) {
        self.tun_form_dirty = true;
        match self.sync_tun_json_from_form() {
            Ok(_) => self.advanced_validation.tun = None,
            Err(e) => self.advanced_validation.tun = Some(Self::map_advanced_error_message(&e)),
        }
    }

    fn active_advanced_mode(&self, tab: DnsTab) -> AdvancedEditMode {
        match tab {
            DnsTab::Dns => self.dns_mode,
            DnsTab::FakeIp => self.fake_ip_mode,
            DnsTab::Tun => self.tun_mode,
        }
    }

    fn normalize_delay_sort_key(value: &str) -> &'static str {
        match value.trim().to_ascii_lowercase().as_str() {
            "delay_asc" => "delay_asc",
            "delay_desc" => "delay_desc",
            "name_asc" => "name_asc",
            "name_desc" => "name_desc",
            _ => "delay_asc",
        }
    }

    fn normalize_connection_sort_key(value: &str) -> &'static str {
        match value.trim().to_ascii_lowercase().as_str() {
            "download_desc" => "download_desc",
            "upload_desc" => "upload_desc",
            "latest_desc" => "latest_desc",
            "host_asc" => "host_asc",
            _ => DEFAULT_RUNTIME_CONNECTION_SORT,
        }
    }

    fn delay_sortable_value(&self, name: &str) -> Option<u32> {
        self.proxies
            .get(name)
            .and_then(|proxy| proxy.history().last().map(|item| item.delay))
            .filter(|delay| *delay > 0)
    }

    fn compare_delay_members(&self, left: &str, right: &str) -> std::cmp::Ordering {
        let left_delay = self.delay_sortable_value(left);
        let right_delay = self.delay_sortable_value(right);

        let compare_delay = |desc: bool| match (left_delay, right_delay) {
            (None, None) => left.cmp(right),
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (Some(_), None) => std::cmp::Ordering::Less,
            (Some(lv), Some(rv)) => {
                let base = if desc { rv.cmp(&lv) } else { lv.cmp(&rv) };
                if base == std::cmp::Ordering::Equal {
                    left.cmp(right)
                } else {
                    base
                }
            }
        };

        match self.proxy_delay_sort.as_str() {
            "name_asc" => left.cmp(right),
            "name_desc" => right.cmp(left),
            "delay_desc" => compare_delay(true),
            _ => compare_delay(false),
        }
    }

    fn normalized_delay_test_url(&self) -> String {
        let trimmed = self.runtime_delay_test_url.trim();
        if trimmed.is_empty() {
            DEFAULT_RUNTIME_DELAY_TEST_URL.to_string()
        } else {
            trimmed.to_string()
        }
    }

    fn normalized_delay_timeout_ms(&self) -> u32 {
        self.runtime_delay_timeout_ms
            .trim()
            .parse::<u32>()
            .ok()
            .filter(|value| *value >= MIN_RUNTIME_DELAY_TIMEOUT_MS)
            .unwrap_or(DEFAULT_RUNTIME_DELAY_TIMEOUT_MS)
            .min(MAX_RUNTIME_DELAY_TIMEOUT_MS)
    }

    fn persist_runtime_panel_settings_task(&self) -> Task<Message> {
        let runtime_panel = RuntimePanelConfig {
            auto_refresh: self.runtime_auto_refresh,
            delay_sort: Self::normalize_delay_sort_key(&self.proxy_delay_sort).to_string(),
            delay_test_url: self.normalized_delay_test_url(),
            delay_timeout_ms: self.normalized_delay_timeout_ms(),
            connection_filter: self.runtime_connection_filter.clone(),
            connection_sort: Self::normalize_connection_sort_key(&self.runtime_connection_sort)
                .to_string(),
        };

        Task::perform(
            async move {
                let base_dir = mihomo_platform::get_home_dir().map_err(InfiltratorError::from)?;
                let settings_path = infiltrator_core::settings::settings_path(&base_dir)
                    .map_err(|e| InfiltratorError::Config(e.to_string()))?;
                let mut settings = infiltrator_core::settings::load_settings(&settings_path)
                    .await
                    .unwrap_or_else(|_| AppSettings::default());
                settings.runtime_panel = runtime_panel;
                infiltrator_core::settings::save_settings(&settings_path, &settings)
                    .await
                    .map_err(|e| InfiltratorError::Config(e.to_string()))?;
                Ok(())
            },
            Message::RuntimePanelSettingsSaved,
        )
    }

    fn schedule_runtime_refresh(&mut self, follow_auto_refresh: bool) -> Task<Message> {
        if follow_auto_refresh && !self.runtime_auto_refresh {
            return Task::none();
        }
        if !matches!(self.status, RuntimeStatus::Running) {
            return Task::none();
        }
        let Some(rt) = self.runtime.clone() else {
            return Task::none();
        };

        self.runtime_poll_tick = self.runtime_poll_tick.saturating_add(1);
        let poll_tick = self.runtime_poll_tick;

        let rt_for_connections = rt.clone();
        let rt_for_memory = rt.clone();
        let mut tasks = vec![
            Task::perform(
                async move {
                    rt_for_connections
                        .client()
                        .get_connections()
                        .await
                        .map_err(InfiltratorError::from)
                },
                |result| match result {
                    Ok(snapshot) => Message::ConnectionsReceived(mihomo_api::ConnectionSnapshot {
                        download_total: snapshot.download_total,
                        upload_total: snapshot.upload_total,
                        connections: snapshot.connections,
                    }),
                    Err(_) => Message::Noop,
                },
            ),
            Task::perform(
                async move {
                    rt_for_memory
                        .client()
                        .get_memory()
                        .await
                        .map_err(InfiltratorError::from)
                },
                |result| match result {
                    Ok(memory) => Message::MemoryReceived(memory),
                    Err(_) => Message::Noop,
                },
            ),
        ];

        if poll_tick % 2 == 0 {
            tasks.push(Task::done(Message::LoadProxies));
        }
        if poll_tick % 6 == 0 {
            tasks.push(Task::done(Message::FetchRuntimeConfig));
        }
        if poll_tick % 15 == 0 {
            tasks.push(Task::done(Message::FetchIpInfo));
        }

        Task::batch(tasks)
    }

    pub fn cancel_all_tasks(&mut self) {
        self.last_task_id += 1;
    }

    pub fn recompute_filtered_groups(&mut self) {
        let mut groups: Vec<_> = self.proxies.iter().filter(|(_, p)| p.is_group()).collect();

        // Sort groups: GLOBAL first, then by type
        groups.sort_by(|(na, pa), (nb, pb)| {
            if *na == "GLOBAL" {
                return std::cmp::Ordering::Less;
            }
            if *nb == "GLOBAL" {
                return std::cmp::Ordering::Greater;
            }
            pa.proxy_type().cmp(pb.proxy_type())
        });

        let mut result = Vec::new();
        for (group_name, group_info) in groups {
            let mut members: Vec<String> =
                group_info.all().map(|all| all.to_vec()).unwrap_or_default();

            // 1. Filter
            if !self.proxy_filter.is_empty() {
                let filter = self.proxy_filter.to_lowercase();
                members.retain(|m| m.to_lowercase().contains(&filter));
            }

            if members.is_empty() && !self.proxy_filter.is_empty() {
                continue;
            }

            members.sort_by(|left, right| self.compare_delay_members(left, right));

            result.push((group_name.clone(), members));
        }
        self.filtered_groups = result;
    }

    fn sync_runtime_proxy_selection(&mut self) {
        let mut groups: Vec<String> = self
            .proxies
            .iter()
            .filter_map(|(name, proxy)| {
                if proxy.is_group() {
                    Some(name.clone())
                } else {
                    None
                }
            })
            .collect();

        if groups.is_empty() {
            self.runtime_selected_group.clear();
            self.runtime_selected_proxy.clear();
            return;
        }

        groups.sort();
        if let Some(index) = groups.iter().position(|name| name == "GLOBAL") {
            let global = groups.remove(index);
            groups.insert(0, global);
        }

        if !groups
            .iter()
            .any(|name| name == &self.runtime_selected_group)
        {
            self.runtime_selected_group = groups[0].clone();
        }

        let members: Vec<String> = self
            .proxies
            .get(&self.runtime_selected_group)
            .and_then(|proxy| proxy.all())
            .map(|all| all.to_vec())
            .unwrap_or_default();
        if members.is_empty() {
            self.runtime_selected_proxy.clear();
            return;
        }

        if !members
            .iter()
            .any(|name| name == &self.runtime_selected_proxy)
        {
            let current = self
                .proxies
                .get(&self.runtime_selected_group)
                .and_then(|proxy| proxy.now())
                .map(|name| name.to_string());
            self.runtime_selected_proxy = current
                .filter(|name| members.iter().any(|member| member == name))
                .unwrap_or_else(|| members[0].clone());
        }
    }

    fn active_rebuild_label(&self) -> String {
        match &self.rebuild_flow {
            RebuildFlowState::Saving { label }
            | RebuildFlowState::Rebuilding { label }
            | RebuildFlowState::Done { label }
            | RebuildFlowState::Failed { label, .. } => label.clone(),
            RebuildFlowState::Idle => "Configuration".to_string(),
        }
    }

    fn begin_save_phase(&mut self, label: &str) {
        self.rebuild_flow = RebuildFlowState::Saving {
            label: label.to_string(),
        };
    }

    fn finish_without_rebuild(&mut self, label: String) -> Task<Message> {
        self.rebuild_flow = RebuildFlowState::Done {
            label: label.clone(),
        };
        Task::batch(vec![
            Task::done(Message::ShowToast(
                format!("{label} saved"),
                ToastStatus::Success,
            )),
            Task::perform(
                async {
                    tokio::time::sleep(tokio::time::Duration::from_secs(4)).await;
                },
                |_| Message::ClearRebuildFlow,
            ),
        ])
    }

    fn trigger_runtime_rebuild(&mut self) -> Task<Message> {
        let label = self.active_rebuild_label();
        let Some(runtime) = self.runtime.take() else {
            return self.finish_without_rebuild(label);
        };

        self.rebuild_flow = RebuildFlowState::Rebuilding {
            label: label.clone(),
        };
        self.status = RuntimeStatus::Starting;

        Task::perform(
            async move {
                let _ = runtime.shutdown().await;
                let vm = VersionManager::new().map_err(InfiltratorError::from)?;
                let data_dir = mihomo_platform::get_home_dir().map_err(InfiltratorError::from)?;
                let candidates = vec![];
                let rebuilt = MihomoRuntime::bootstrap(&vm, true, &candidates, &data_dir)
                    .await
                    .map_err(|e: anyhow::Error| InfiltratorError::Mihomo(e.to_string()))?;
                Ok(Arc::new(rebuilt))
            },
            Message::RuntimeRebuildFinished,
        )
    }

    pub fn update_core(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::FetchIpInfo => {
                self.cancel_all_tasks();
                let task_id = self.last_task_id;
                Task::perform(
                    async move {
                        let client = reqwest::Client::builder()
                            .timeout(std::time::Duration::from_secs(5))
                            .build()
                            .map_err(|e| InfiltratorError::Internal(e.to_string()))?;
                        let resp = client
                            .get("https://api.ipify.org")
                            .send()
                            .await
                            .map_err(|e| InfiltratorError::Internal(e.to_string()))?
                            .text()
                            .await
                            .map_err(|e| InfiltratorError::Internal(e.to_string()))?;
                        Ok(resp)
                    },
                    move |res| Message::IpInfoReceived(res, task_id),
                )
            }
            Message::StartProxy => {
                self.cancel_all_tasks();
                self.status = RuntimeStatus::Starting;
                self.error_msg = None;
                self.runtime_poll_tick = 0;
                self.runtime_prev_upload_total = None;
                self.runtime_prev_download_total = None;
                Task::perform(
                    async {
                        let vm = VersionManager::new().map_err(|e: mihomo_api::MihomoError| {
                            InfiltratorError::Mihomo(e.to_string())
                        })?;
                        let data_dir = mihomo_platform::get_home_dir().map_err(
                            |e: mihomo_api::MihomoError| InfiltratorError::Mihomo(e.to_string()),
                        )?;
                        let candidates = vec![];
                        let r = MihomoRuntime::bootstrap(&vm, true, &candidates, &data_dir)
                            .await
                            .map_err(|e: anyhow::Error| InfiltratorError::Mihomo(e.to_string()))?;
                        Ok(Arc::new(r))
                    },
                    Message::ProxyStarted,
                )
            }
            Message::StopProxy => {
                self.cancel_all_tasks();
                let rt = self.runtime.take();
                self.status = RuntimeStatus::Stopped;
                Task::perform(
                    async move {
                        if let Some(r) = rt {
                            let _ = r.shutdown().await;
                        }
                    },
                    |_| Message::ProxyStopped,
                )
            }
            Message::ProxyStarted(result) => match result {
                Ok(runtime) => {
                    self.status = RuntimeStatus::Running;
                    self.runtime = Some(runtime);
                    Task::batch(vec![
                        Task::done(Message::FetchRuntimeConfig),
                        Task::done(Message::LoadProxies),
                        Task::done(Message::FetchIpInfo),
                        Task::done(Message::RefreshRuntimeNow),
                    ])
                }
                Err(e) => {
                    self.status = RuntimeStatus::Error(e.clone());
                    self.error_msg = Some(e.to_string());
                    Task::none()
                }
            },
            Message::ProxyStopped => {
                self.traffic = None;
                self.traffic_history.clear();
                self.connections = None;
                self.memory = None;
                self.public_ip = None;
                self.runtime_selected_group.clear();
                self.runtime_selected_proxy.clear();
                self.runtime_prev_upload_total = None;
                self.runtime_prev_download_total = None;
                self.runtime_poll_tick = 0;
                self.logs.clear();
                self.proxy_mode = None;
                self.tun_enabled = None;
                self.status = RuntimeStatus::Stopped;
                Task::none()
            }
            Message::SettingsLoaded(result) => {
                match result {
                    Ok(settings) => {
                        if !settings.language.trim().is_empty() {
                            self.lang = settings.language;
                        }
                        let theme = settings.theme.trim().to_ascii_lowercase();
                        if theme == "light" {
                            self.theme = iced::Theme::Light;
                        } else if theme == "dark" {
                            self.theme = iced::Theme::Dark;
                        }
                        self.editor_path =
                            settings.editor_path.clone().map(std::path::PathBuf::from);
                        self.editor_path_setting = settings.editor_path.unwrap_or_default();
                        self.webdav_enabled = settings.webdav.enabled;
                        self.webdav_url = settings.webdav.url;
                        self.webdav_user = settings.webdav.username;
                        self.webdav_pass = settings.webdav.password;
                        self.webdav_sync_interval_mins =
                            settings.webdav.sync_interval_mins.to_string();
                        self.webdav_sync_on_startup = settings.webdav.sync_on_startup;
                        self.runtime_auto_refresh = settings.runtime_panel.auto_refresh;
                        self.proxy_delay_sort =
                            Self::normalize_delay_sort_key(&settings.runtime_panel.delay_sort)
                                .to_string();
                        self.proxy_sort_by_delay = self.proxy_delay_sort.starts_with("delay_");
                        self.runtime_delay_test_url =
                            if settings.runtime_panel.delay_test_url.trim().is_empty() {
                                DEFAULT_RUNTIME_DELAY_TEST_URL.to_string()
                            } else {
                                settings.runtime_panel.delay_test_url
                            };
                        let timeout = settings
                            .runtime_panel
                            .delay_timeout_ms
                            .max(MIN_RUNTIME_DELAY_TIMEOUT_MS)
                            .min(MAX_RUNTIME_DELAY_TIMEOUT_MS);
                        self.runtime_delay_timeout_ms = timeout.to_string();
                        self.runtime_connection_filter = settings.runtime_panel.connection_filter;
                        self.runtime_connection_sort = Self::normalize_connection_sort_key(
                            &settings.runtime_panel.connection_sort,
                        )
                        .to_string();
                        if self.webdav_enabled
                            && self.webdav_sync_on_startup
                            && !self.webdav_url.trim().is_empty()
                            && !self.webdav_user.trim().is_empty()
                        {
                            return Task::done(Message::SyncDownload);
                        }
                    }
                    Err(e) => {
                        self.error_msg = Some(e.to_string());
                    }
                }
                Task::none()
            }
            Message::LoadProxies => {
                if let Some(rt) = self.runtime.clone() {
                    self.is_loading_proxies = true;
                    Task::perform(
                        async move {
                            rt.client()
                                .get_proxies()
                                .await
                                .map_err(InfiltratorError::from)
                        },
                        Message::ProxiesLoaded,
                    )
                } else {
                    Task::none()
                }
            }
            Message::ProxiesLoaded(result) => {
                self.is_loading_proxies = false;
                match result {
                    Ok(proxies) => {
                        if let Some(tm) = &self.tray_manager {
                            tm.update_groups(&proxies);
                        }
                        self.proxies = proxies;
                        self.recompute_filtered_groups();
                        self.sync_runtime_proxy_selection();
                    }
                    Err(e) => self.error_msg = Some(e.to_string()),
                }
                Task::none()
            }
            Message::SelectProxy(group, name) => {
                if let Some(rt) = self.runtime.clone() {
                    Task::perform(
                        async move {
                            rt.client()
                                .switch_proxy(&group, &name)
                                .await
                                .map_err(InfiltratorError::from)
                        },
                        |_| Message::LoadProxies,
                    )
                } else {
                    Task::none()
                }
            }
            Message::FilterProxies(filter) => {
                self.proxy_filter = filter;
                Task::done(Message::UpdateFilteredGroups)
            }
            Message::ToggleProxySort => {
                self.proxy_sort_by_delay = !self.proxy_sort_by_delay;
                self.proxy_delay_sort = if self.proxy_sort_by_delay {
                    "delay_asc".to_string()
                } else {
                    "name_asc".to_string()
                };
                Task::done(Message::UpdateFilteredGroups)
            }
            Message::UpdateProxyDelaySort(sort_key) => {
                let normalized = Self::normalize_delay_sort_key(&sort_key).to_string();
                self.proxy_delay_sort = normalized.clone();
                self.proxy_sort_by_delay = normalized.starts_with("delay_");
                Task::batch(vec![
                    Task::done(Message::UpdateFilteredGroups),
                    self.persist_runtime_panel_settings_task(),
                ])
            }
            Message::UpdateDelayTestUrl(url) => {
                self.runtime_delay_test_url = url;
                self.persist_runtime_panel_settings_task()
            }
            Message::UpdateDelayTimeoutMs(timeout) => {
                self.runtime_delay_timeout_ms = timeout;
                self.persist_runtime_panel_settings_task()
            }
            Message::UpdateRuntimeSelectedGroup(group) => {
                self.runtime_selected_group = group;
                self.sync_runtime_proxy_selection();
                Task::none()
            }
            Message::UpdateRuntimeSelectedProxy(proxy) => {
                self.runtime_selected_proxy = proxy;
                Task::none()
            }
            Message::ApplyRuntimeSelectedProxy => {
                let group = self.runtime_selected_group.trim().to_string();
                let proxy = self.runtime_selected_proxy.trim().to_string();
                if group.is_empty() || proxy.is_empty() {
                    return Task::none();
                }
                Task::done(Message::SelectProxy(group, proxy))
            }
            Message::UpdateRuntimeConnectionFilter(filter) => {
                self.runtime_connection_filter = filter;
                self.persist_runtime_panel_settings_task()
            }
            Message::UpdateRuntimeConnectionSort(sort_key) => {
                self.runtime_connection_sort =
                    Self::normalize_connection_sort_key(&sort_key).to_string();
                self.persist_runtime_panel_settings_task()
            }
            Message::RefreshRuntimeNow => self.schedule_runtime_refresh(false),
            Message::RuntimePanelSettingsSaved(_) => Task::none(),
            Message::TickRuntimeRefresh => self.schedule_runtime_refresh(true),
            Message::UpdateRuntimeAutoRefresh(enabled) => {
                self.runtime_auto_refresh = enabled;
                if !enabled {
                    self.runtime_poll_tick = 0;
                    return self.persist_runtime_panel_settings_task();
                }
                Task::batch(vec![
                    Task::done(Message::RefreshRuntimeNow),
                    self.persist_runtime_panel_settings_task(),
                ])
            }
            Message::UpdateFilteredGroups => {
                self.recompute_filtered_groups();
                Task::none()
            }
            Message::TrafficReceived(data) => {
                self.traffic = Some(data.clone());
                self.traffic_history.push_back((data.up, data.down));
                if self.traffic_history.len() > 60 {
                    self.traffic_history.pop_front();
                }
                Task::none()
            }
            Message::MemoryReceived(data) => {
                self.memory = Some(data);
                Task::none()
            }
            Message::IpInfoReceived(result, task_id) => {
                if task_id == self.last_task_id {
                    match result {
                        Ok(ip) => self.public_ip = Some(ip),
                        Err(e) => self.error_msg = Some(e.to_string()),
                    }
                }
                Task::none()
            }
            Message::ConnectionsReceived(data) => {
                let upload_total = data.upload_total;
                let download_total = data.download_total;

                if let (Some(prev_up), Some(prev_down)) = (
                    self.runtime_prev_upload_total,
                    self.runtime_prev_download_total,
                ) {
                    let up_rate = upload_total.saturating_sub(prev_up) / 2;
                    let down_rate = download_total.saturating_sub(prev_down) / 2;
                    self.traffic = Some(mihomo_api::TrafficData {
                        up: up_rate,
                        down: down_rate,
                    });
                    self.traffic_history.push_back((up_rate, down_rate));
                    if self.traffic_history.len() > 60 {
                        self.traffic_history.pop_front();
                    }
                }

                self.runtime_prev_upload_total = Some(upload_total);
                self.runtime_prev_download_total = Some(download_total);
                self.connections = Some(data);
                Task::none()
            }
            Message::AllProxyDelaysTested(result) => {
                self.runtime_testing_all_delays = false;
                match result {
                    Ok((success, failed)) => Task::batch(vec![
                        Task::done(Message::LoadProxies),
                        Task::done(Message::ShowToast(
                            format!(
                                "Delay test complete: {} success, {} failed",
                                success, failed
                            ),
                            ToastStatus::Success,
                        )),
                    ]),
                    Err(e) => {
                        self.error_msg = Some(e.to_string());
                        Task::done(Message::ShowToast(e.to_string(), ToastStatus::Error))
                    }
                }
            }
            Message::LogReceived(log) => {
                self.logs.push_back(log);
                if self.logs.len() > 500 {
                    self.logs.pop_front();
                }
                iced::widget::operation::snap_to(
                    iced::widget::Id::new("log_scroller"),
                    iced::widget::scrollable::RelativeOffset::END,
                )
            }
            Message::ClearRuntimeLogs => {
                self.logs.clear();
                Task::none()
            }
            Message::SetLogLevel(level) => {
                self.log_level = level.clone();
                if let Some(rt) = self.runtime.clone() {
                    Task::perform(
                        async move {
                            rt.client()
                                .patch_config(serde_json::json!({ "log-level": level }))
                                .await
                                .map_err(InfiltratorError::from)
                        },
                        Message::OperationResult,
                    )
                } else {
                    Task::none()
                }
            }
            Message::CloseConnection(id) => {
                if let Some(rt) = self.runtime.clone() {
                    Task::perform(
                        async move {
                            rt.client()
                                .close_connection(&id)
                                .await
                                .map_err(InfiltratorError::from)
                        },
                        Message::OperationResult,
                    )
                } else {
                    Task::none()
                }
            }
            Message::CloseAllConnections => {
                if let Some(rt) = self.runtime.clone() {
                    Task::perform(
                        async move {
                            rt.client()
                                .close_all_connections()
                                .await
                                .map_err(InfiltratorError::from)
                        },
                        Message::OperationResult,
                    )
                } else {
                    Task::none()
                }
            }
            Message::FetchRuntimeConfig => {
                if let Some(rt) = self.runtime.clone() {
                    Task::perform(
                        async move {
                            let config = rt
                                .client()
                                .get_config()
                                .await
                                .map_err(InfiltratorError::from)?;
                            let mode = config.mode;
                            let (tun_en, tun_st, tun_ar, tun_sr) = config
                                .tun
                                .map(|t| (t.enable, t.stack, t.auto_route, t.strict_route))
                                .unwrap_or((false, String::new(), false, false));
                            let (dns, fallback, enhanced) = config
                                .dns
                                .map(|d| {
                                    (
                                        d.nameserver,
                                        d.fallback.unwrap_or_default(),
                                        d.enhanced_mode,
                                    )
                                })
                                .unwrap_or((vec![], vec![], String::new()));
                            let sniff = config.sniffer.map(|s| s.enable).unwrap_or(false);
                            Ok(RuntimeConfig {
                                mode,
                                tun_enabled: tun_en,
                                dns_nameservers: dns,
                                dns_fallback: fallback,
                                dns_enhanced_mode: enhanced,
                                tun_stack: tun_st,
                                tun_auto_route: tun_ar,
                                tun_strict_route: tun_sr,
                                sniffer_enabled: sniff,
                            })
                        },
                        Message::RuntimeConfigFetched,
                    )
                } else {
                    Task::none()
                }
            }
            Message::RuntimeConfigFetched(result) => {
                if let Ok(config) = result {
                    self.proxy_mode = Some(config.mode);
                    self.tun_enabled = Some(config.tun_enabled);
                    self.dns_nameservers = config.dns_nameservers;
                    self.dns_fallback_servers = config.dns_fallback;
                    self.dns_enhanced_mode = config.dns_enhanced_mode;
                    self.tun_stack = config.tun_stack;
                    self.tun_auto_route = config.tun_auto_route;
                    self.tun_strict_route = config.tun_strict_route;
                    self.sniffer_enabled = config.sniffer_enabled;
                    if let Some(tm) = &self.tray_manager {
                        tm.update_status(self.system_proxy_enabled, config.tun_enabled);
                    }
                }
                Task::none()
            }
            Message::SetProxyMode(mode) => {
                self.proxy_mode = Some(mode.clone());
                if let Some(rt) = self.runtime.clone() {
                    Task::perform(
                        async move {
                            rt.client()
                                .patch_config(serde_json::json!({ "mode": mode }))
                                .await
                                .map_err(InfiltratorError::from)
                        },
                        Message::ModeSetResult,
                    )
                } else {
                    Task::none()
                }
            }
            Message::ModeSetResult(result) => match result {
                Ok(_) => Task::done(Message::FetchRuntimeConfig),
                Err(e) => {
                    self.error_msg = Some(e.to_string());
                    Task::none()
                }
            },
            Message::SetTunEnabled(enabled) => {
                self.tun_enabled = Some(enabled);
                if let Some(rt) = self.runtime.clone() {
                    Task::perform(
                        async move {
                            rt.client()
                                .patch_config(serde_json::json!({ "tun": { "enable": enabled } }))
                                .await
                                .map_err(InfiltratorError::from)
                        },
                        Message::OperationResult,
                    )
                } else {
                    Task::none()
                }
            }
            Message::SetTunStack(stack) => {
                self.tun_stack = stack.clone();
                if let Some(rt) = self.runtime.clone() {
                    Task::perform(
                        async move {
                            rt.client()
                                .patch_config(serde_json::json!({ "tun": { "stack": stack } }))
                                .await
                                .map_err(InfiltratorError::from)
                        },
                        Message::OperationResult,
                    )
                } else {
                    Task::none()
                }
            }
            Message::SetTunAutoRoute(enabled) => {
                self.tun_auto_route = enabled;
                if let Some(rt) = self.runtime.clone() {
                    Task::perform(
                        async move {
                            rt.client()
                                .patch_config(
                                    serde_json::json!({ "tun": { "auto-route": enabled } }),
                                )
                                .await
                                .map_err(InfiltratorError::from)
                        },
                        Message::OperationResult,
                    )
                } else {
                    Task::none()
                }
            }
            Message::SetTunStrictRoute(enabled) => {
                self.tun_strict_route = enabled;
                if let Some(rt) = self.runtime.clone() {
                    Task::perform(
                        async move {
                            rt.client()
                                .patch_config(
                                    serde_json::json!({ "tun": { "strict-route": enabled } }),
                                )
                                .await
                                .map_err(InfiltratorError::from)
                        },
                        Message::OperationResult,
                    )
                } else {
                    Task::none()
                }
            }
            Message::SetSnifferEnabled(enabled) => {
                self.sniffer_enabled = enabled;
                if let Some(rt) = self.runtime.clone() {
                    Task::perform(
                        async move {
                            rt.client()
                                .patch_config(
                                    serde_json::json!({ "sniffer": { "enable": enabled } }),
                                )
                                .await
                                .map_err(InfiltratorError::from)
                        },
                        Message::OperationResult,
                    )
                } else {
                    Task::none()
                }
            }
            Message::OperationResult(result) => match result {
                Ok(_) => Task::done(Message::FetchRuntimeConfig),
                Err(e) => {
                    self.error_msg = Some(e.to_string());
                    Task::none()
                }
            },
            Message::FilterRules(filter) => {
                self.rules_filter = filter;
                self.rules_page = 0;
                self.apply_rules_filter();
                Task::none()
            }
            Message::UpdateNewRuleType(t) => {
                self.new_rule_type = t;
                Task::none()
            }
            Message::UpdateNewRulePayload(p) => {
                self.new_rule_payload = p;
                Task::none()
            }
            Message::UpdateNewRuleTarget(t) => {
                self.new_rule_target = t;
                Task::none()
            }
            Message::AddCustomRule => {
                let payload = self.new_rule_payload.trim().to_string();
                if payload.is_empty() {
                    return Task::done(Message::ShowToast(
                        "Payload cannot be empty".to_string(),
                        ToastStatus::Error,
                    ));
                }

                let entry = RuleEntry {
                    rule: format!(
                        "{},{},{}",
                        self.new_rule_type.clone(),
                        payload,
                        self.new_rule_target.clone()
                    ),
                    enabled: true,
                };
                self.is_adding_rule = true;
                Task::perform(
                    async move {
                        let mut rules = infiltrator_core::rules::load_rules()
                            .await
                            .map_err(|e| InfiltratorError::Config(e.to_string()))?;
                        rules.insert(0, entry);
                        infiltrator_core::rules::save_rules(rules)
                            .await
                            .map_err(|e| InfiltratorError::Config(e.to_string()))?;
                        Ok(())
                    },
                    Message::RuleAdded,
                )
            }
            Message::RuleAdded(result) => {
                self.is_adding_rule = false;
                match result {
                    Ok(_) => {
                        self.new_rule_payload.clear();
                        Task::batch(vec![
                            Task::done(Message::LoadRules),
                            Task::done(Message::ShowToast(
                                "Rule added".to_string(),
                                ToastStatus::Success,
                            )),
                        ])
                    }
                    Err(e) => {
                        self.error_msg = Some(e.to_string());
                        Task::done(Message::ShowToast(e.to_string(), ToastStatus::Error))
                    }
                }
            }
            Message::SetRulesTab(tab) => {
                self.rules_tab = tab;
                self.rules_page = 0;
                match tab {
                    RulesTab::JsonEditors => Task::done(match self.rules_json_tab {
                        RulesJsonTab::RuleProviders => Message::EnsureRuleProvidersEditorLoaded,
                        RulesJsonTab::ProxyProviders => Message::EnsureProxyProvidersEditorLoaded,
                        RulesJsonTab::Sniffer => Message::EnsureSnifferEditorLoaded,
                    }),
                    _ => Task::none(),
                }
            }
            Message::SetRulesJsonTab(tab) => {
                self.rules_json_tab = tab;
                Task::done(match tab {
                    RulesJsonTab::RuleProviders => Message::EnsureRuleProvidersEditorLoaded,
                    RulesJsonTab::ProxyProviders => Message::EnsureProxyProvidersEditorLoaded,
                    RulesJsonTab::Sniffer => Message::EnsureSnifferEditorLoaded,
                })
            }
            Message::ToggleRulesProvidersExpanded => {
                self.rules_providers_expanded = !self.rules_providers_expanded;
                Task::none()
            }
            Message::RulesPrevPage => {
                self.rules_page = self.rules_page.saturating_sub(1);
                Task::none()
            }
            Message::RulesNextPage => {
                let total_pages = if self.rules_filtered_indices.is_empty() {
                    1
                } else {
                    (self.rules_filtered_indices.len() - 1) / self.rules_page_size + 1
                };
                if self.rules_page + 1 < total_pages {
                    self.rules_page += 1;
                }
                Task::none()
            }
            Message::RulesSetPage(page) => {
                self.rules_page = page;
                self.apply_rules_filter();
                Task::none()
            }
            Message::EnsureRuleProvidersEditorLoaded => {
                self.ensure_rule_providers_editor_loaded();
                Task::none()
            }
            Message::EnsureProxyProvidersEditorLoaded => {
                self.ensure_proxy_providers_editor_loaded();
                Task::none()
            }
            Message::EnsureSnifferEditorLoaded => {
                self.ensure_sniffer_editor_loaded();
                Task::none()
            }
            Message::ActivateRulesHeavyView => {
                self.rules_heavy_ready = true;
                if self.rules_tab == RulesTab::JsonEditors {
                    Task::done(match self.rules_json_tab {
                        RulesJsonTab::RuleProviders => Message::EnsureRuleProvidersEditorLoaded,
                        RulesJsonTab::ProxyProviders => Message::EnsureProxyProvidersEditorLoaded,
                        RulesJsonTab::Sniffer => Message::EnsureSnifferEditorLoaded,
                    })
                } else {
                    Task::none()
                }
            }
            Message::LoadRules => {
                self.is_loading_rules = true;
                if !self.rules_loaded_once {
                    self.reset_rules_lazy_state();
                }
                let mut tasks = vec![Task::perform(
                    async {
                        let manager = mihomo_config::ConfigManager::new()
                            .map_err(|e: mihomo_api::MihomoError| InfiltratorError::from(e))?;
                        let profile = manager
                            .get_current()
                            .await
                            .map_err(|e: mihomo_api::MihomoError| InfiltratorError::from(e))?;
                        let content = manager
                            .load(&profile)
                            .await
                            .map_err(|e: mihomo_api::MihomoError| InfiltratorError::from(e))?;
                        let doc: serde_yml::Value = serde_yml::from_str(&content)
                            .map_err(|e| InfiltratorError::Config(e.to_string()))?;

                        let rules = infiltrator_core::rules::extract_rules_from_doc(&doc)
                            .map_err(|e| InfiltratorError::Config(e.to_string()))?;
                        let rule_providers =
                            infiltrator_core::rules::extract_rule_providers_from_doc(&doc)
                                .map_err(|e| InfiltratorError::Config(e.to_string()))?;
                        let proxy_providers =
                            infiltrator_core::proxy_providers::extract_proxy_providers_from_doc(
                                &doc,
                            )
                            .map_err(|e| InfiltratorError::Config(e.to_string()))?;
                        let sniffer =
                            infiltrator_core::sniffer::extract_sniffer_config_from_doc(&doc)
                                .map_err(|e| InfiltratorError::Config(e.to_string()))?;

                        let rule_providers_json = serde_json::to_string_pretty(&rule_providers)
                            .map_err(|e| InfiltratorError::Config(e.to_string()))?;
                        let proxy_providers_json =
                            serde_json::to_string_pretty(&proxy_providers)
                                .map_err(|e| InfiltratorError::Config(e.to_string()))?;
                        let sniffer_json = serde_json::to_string_pretty(&sniffer)
                            .map_err(|e| InfiltratorError::Config(e.to_string()))?;

                        Ok(RulesLoadBundle {
                            rules,
                            rule_providers_json,
                            proxy_providers_json,
                            sniffer_json,
                        })
                    },
                    Message::RulesBundleLoaded,
                )];
                if let Some(rt) = self.runtime.clone() {
                    self.is_loading_providers = true;
                    tasks.push(Task::perform(
                        async move {
                            let proxies = rt
                                .client()
                                .get_proxy_providers()
                                .await
                                .map_err(InfiltratorError::from)?;
                            let rules = rt
                                .client()
                                .get_rule_providers()
                                .await
                                .map_err(InfiltratorError::from)?;
                            Ok((
                                proxies.into_values().collect(),
                                rules.into_values().collect(),
                            ))
                        },
                        Message::ProvidersLoaded,
                    ));
                } else {
                    self.is_loading_providers = false;
                }
                Task::batch(tasks)
            }
            Message::RulesBundleLoaded(result) => {
                self.is_loading_rules = false;
                match result {
                    Ok(bundle) => {
                        self.rules_loaded_once = true;
                        self.rules = bundle.rules;
                        self.rules_dirty = false;
                        self.rebuild_rules_render_cache();
                        self.apply_rules_filter();

                        self.rule_providers_json_cache = bundle.rule_providers_json;
                        if !self.rule_providers_json_dirty
                            && self.rule_providers_editor_state == EditorLazyState::Loaded
                            && self.rule_providers_json_content.text()
                                != self.rule_providers_json_cache
                        {
                            self.ensure_rule_providers_editor_loaded();
                            self.rule_providers_json_dirty = false;
                        }

                        self.proxy_providers_json_cache = bundle.proxy_providers_json;
                        if !self.proxy_providers_json_dirty
                            && self.proxy_providers_editor_state == EditorLazyState::Loaded
                            && self.proxy_providers_json_content.text()
                                != self.proxy_providers_json_cache
                        {
                            self.ensure_proxy_providers_editor_loaded();
                            self.proxy_providers_json_dirty = false;
                        }

                        self.sniffer_json_cache = bundle.sniffer_json;
                        if !self.sniffer_json_dirty
                            && self.sniffer_editor_state == EditorLazyState::Loaded
                            && self.sniffer_json_content.text() != self.sniffer_json_cache
                        {
                            self.ensure_sniffer_editor_loaded();
                            self.sniffer_json_dirty = false;
                        }
                    }
                    Err(e) => {
                        self.rules_loaded_once = false;
                        self.error_msg = Some(e.to_string());
                    }
                }
                Task::none()
            }
            Message::RulesLoaded(result) => {
                self.is_loading_rules = false;
                match result {
                    Ok(rules) => {
                        self.rules_loaded_once = true;
                        self.rules = rules;
                        self.rules_dirty = false;
                        self.rebuild_rules_render_cache();
                        self.apply_rules_filter();
                    }
                    Err(e) => {
                        self.rules_loaded_once = false;
                        self.error_msg = Some(e.to_string());
                    }
                }
                Task::none()
            }
            Message::RuleProvidersJsonLoaded(result) => {
                match result {
                    Ok(json) => {
                        self.rule_providers_json_cache = json;
                        if self.rule_providers_editor_state == EditorLazyState::Loaded {
                            self.ensure_rule_providers_editor_loaded();
                        }
                        self.rule_providers_json_dirty = false;
                    }
                    Err(e) => self.error_msg = Some(e.to_string()),
                }
                Task::none()
            }
            Message::ProxyProvidersJsonLoaded(result) => {
                match result {
                    Ok(json) => {
                        self.proxy_providers_json_cache = json;
                        if self.proxy_providers_editor_state == EditorLazyState::Loaded {
                            self.ensure_proxy_providers_editor_loaded();
                        }
                        self.proxy_providers_json_dirty = false;
                    }
                    Err(e) => self.error_msg = Some(e.to_string()),
                }
                Task::none()
            }
            Message::SnifferJsonLoaded(result) => {
                match result {
                    Ok(json) => {
                        self.sniffer_json_cache = json;
                        if self.sniffer_editor_state == EditorLazyState::Loaded {
                            self.ensure_sniffer_editor_loaded();
                        }
                        self.sniffer_json_dirty = false;
                    }
                    Err(e) => self.error_msg = Some(e.to_string()),
                }
                Task::none()
            }
            Message::ToggleRuleEnabled(index) => {
                if let Some(entry) = self.rules.get_mut(index) {
                    entry.enabled = !entry.enabled;
                    self.rules_dirty = true;
                    self.rebuild_rules_render_cache();
                    self.apply_rules_filter();
                }
                Task::none()
            }
            Message::MoveRuleUp(index) => {
                if index > 0 && index < self.rules.len() {
                    self.rules.swap(index, index - 1);
                    self.rules_dirty = true;
                    self.rebuild_rules_render_cache();
                    self.apply_rules_filter();
                }
                Task::none()
            }
            Message::MoveRuleDown(index) => {
                if index + 1 < self.rules.len() {
                    self.rules.swap(index, index + 1);
                    self.rules_dirty = true;
                    self.rebuild_rules_render_cache();
                    self.apply_rules_filter();
                }
                Task::none()
            }
            Message::SaveRules => {
                let rules = self.rules.clone();
                self.is_saving_rules = true;
                self.begin_save_phase("Rules");
                Task::perform(
                    async move {
                        infiltrator_core::rules::save_rules(rules)
                            .await
                            .map_err(|e| InfiltratorError::Config(e.to_string()))?;
                        Ok(())
                    },
                    Message::RulesSaved,
                )
            }
            Message::RulesSaved(result) => {
                self.is_saving_rules = false;
                match result {
                    Ok(_) => {
                        self.rules_dirty = false;
                        Task::batch(vec![
                            Task::done(Message::LoadRules),
                            self.trigger_runtime_rebuild(),
                        ])
                    }
                    Err(e) => {
                        self.rebuild_flow = RebuildFlowState::Failed {
                            label: "Rules".to_string(),
                            error: e.to_string(),
                        };
                        self.error_msg = Some(e.to_string());
                        Task::batch(vec![
                            Task::done(Message::ShowToast(e.to_string(), ToastStatus::Error)),
                            Task::perform(
                                async {
                                    tokio::time::sleep(tokio::time::Duration::from_secs(4)).await;
                                },
                                |_| Message::ClearRebuildFlow,
                            ),
                        ])
                    }
                }
            }
            Message::RuleProvidersEditorAction(action) => {
                self.ensure_rule_providers_editor_loaded();
                self.rule_providers_json_content.perform(action);
                self.rule_providers_json_dirty = true;
                Task::none()
            }
            Message::SaveRuleProvidersJson => {
                self.ensure_rule_providers_editor_loaded();
                let text = self.rule_providers_json_content.text();
                self.is_saving_rule_providers_json = true;
                self.begin_save_phase("Rule Providers");
                Task::perform(
                    async move {
                        let providers = serde_json::from_str::<
                            infiltrator_core::rules::RuleProviders,
                        >(&text)
                        .map_err(|e| {
                            InfiltratorError::Config(format!("Invalid rule providers JSON: {}", e))
                        })?;
                        infiltrator_core::rules::save_rule_providers(providers)
                            .await
                            .map_err(|e| InfiltratorError::Config(e.to_string()))?;
                        Ok(())
                    },
                    Message::RuleProvidersJsonSaved,
                )
            }
            Message::RuleProvidersJsonSaved(result) => {
                self.is_saving_rule_providers_json = false;
                match result {
                    Ok(_) => {
                        self.rule_providers_json_dirty = false;
                        Task::batch(vec![
                            Task::done(Message::LoadRules),
                            self.trigger_runtime_rebuild(),
                        ])
                    }
                    Err(e) => {
                        self.rebuild_flow = RebuildFlowState::Failed {
                            label: "Rule Providers".to_string(),
                            error: e.to_string(),
                        };
                        self.error_msg = Some(e.to_string());
                        Task::batch(vec![
                            Task::done(Message::ShowToast(e.to_string(), ToastStatus::Error)),
                            Task::perform(
                                async {
                                    tokio::time::sleep(tokio::time::Duration::from_secs(4)).await;
                                },
                                |_| Message::ClearRebuildFlow,
                            ),
                        ])
                    }
                }
            }
            Message::ProxyProvidersEditorAction(action) => {
                self.ensure_proxy_providers_editor_loaded();
                self.proxy_providers_json_content.perform(action);
                self.proxy_providers_json_dirty = true;
                Task::none()
            }
            Message::SaveProxyProvidersJson => {
                self.ensure_proxy_providers_editor_loaded();
                let text = self.proxy_providers_json_content.text();
                self.is_saving_proxy_providers_json = true;
                self.begin_save_phase("Proxy Providers");
                Task::perform(
                    async move {
                        let providers = serde_json::from_str::<
                            infiltrator_core::proxy_providers::ProxyProviders,
                        >(&text)
                        .map_err(|e| {
                            InfiltratorError::Config(format!("Invalid proxy providers JSON: {}", e))
                        })?;
                        infiltrator_core::proxy_providers::save_proxy_providers(providers)
                            .await
                            .map_err(|e| InfiltratorError::Config(e.to_string()))?;
                        Ok(())
                    },
                    Message::ProxyProvidersJsonSaved,
                )
            }
            Message::ProxyProvidersJsonSaved(result) => {
                self.is_saving_proxy_providers_json = false;
                match result {
                    Ok(_) => {
                        self.proxy_providers_json_dirty = false;
                        Task::batch(vec![
                            Task::done(Message::LoadRules),
                            self.trigger_runtime_rebuild(),
                        ])
                    }
                    Err(e) => {
                        self.rebuild_flow = RebuildFlowState::Failed {
                            label: "Proxy Providers".to_string(),
                            error: e.to_string(),
                        };
                        self.error_msg = Some(e.to_string());
                        Task::batch(vec![
                            Task::done(Message::ShowToast(e.to_string(), ToastStatus::Error)),
                            Task::perform(
                                async {
                                    tokio::time::sleep(tokio::time::Duration::from_secs(4)).await;
                                },
                                |_| Message::ClearRebuildFlow,
                            ),
                        ])
                    }
                }
            }
            Message::SnifferEditorAction(action) => {
                self.ensure_sniffer_editor_loaded();
                self.sniffer_json_content.perform(action);
                self.sniffer_json_dirty = true;
                Task::none()
            }
            Message::SaveSnifferJson => {
                self.ensure_sniffer_editor_loaded();
                let text = self.sniffer_json_content.text();
                self.is_saving_sniffer_json = true;
                self.begin_save_phase("Sniffer");
                Task::perform(
                    async move {
                        let config =
                            serde_json::from_str::<serde_json::Value>(&text).map_err(|e| {
                                InfiltratorError::Config(format!("Invalid sniffer JSON: {}", e))
                            })?;
                        infiltrator_core::sniffer::save_sniffer_config(config)
                            .await
                            .map_err(|e| InfiltratorError::Config(e.to_string()))?;
                        Ok(())
                    },
                    Message::SnifferJsonSaved,
                )
            }
            Message::SnifferJsonSaved(result) => {
                self.is_saving_sniffer_json = false;
                match result {
                    Ok(_) => {
                        self.sniffer_json_dirty = false;
                        Task::batch(vec![
                            Task::done(Message::LoadRules),
                            self.trigger_runtime_rebuild(),
                        ])
                    }
                    Err(e) => {
                        self.rebuild_flow = RebuildFlowState::Failed {
                            label: "Sniffer".to_string(),
                            error: e.to_string(),
                        };
                        self.error_msg = Some(e.to_string());
                        Task::batch(vec![
                            Task::done(Message::ShowToast(e.to_string(), ToastStatus::Error)),
                            Task::perform(
                                async {
                                    tokio::time::sleep(tokio::time::Duration::from_secs(4)).await;
                                },
                                |_| Message::ClearRebuildFlow,
                            ),
                        ])
                    }
                }
            }
            Message::ProvidersLoaded(result) => {
                self.is_loading_providers = false;
                match result {
                    Ok((proxies, rules)) => {
                        self.proxy_providers = proxies;
                        self.rule_providers = rules;
                    }
                    Err(e) => self.error_msg = Some(e.to_string()),
                }
                Task::none()
            }
            Message::UpdateProxyProvider(name) => {
                if let Some(rt) = self.runtime.clone() {
                    Task::perform(
                        async move {
                            rt.client()
                                .update_proxy_provider(&name)
                                .await
                                .map_err(InfiltratorError::from)
                        },
                        Message::OperationResult,
                    )
                } else {
                    Task::none()
                }
            }
            Message::UpdateRuleProvider(name) => {
                if let Some(rt) = self.runtime.clone() {
                    Task::perform(
                        async move {
                            rt.client()
                                .update_rule_provider(&name)
                                .await
                                .map_err(InfiltratorError::from)
                        },
                        Message::OperationResult,
                    )
                } else {
                    Task::none()
                }
            }
            Message::SetDnsTab(tab) => {
                self.dns_tab = tab;
                if self.active_advanced_mode(tab) == AdvancedEditMode::Json {
                    Task::done(match tab {
                        DnsTab::Dns => Message::EnsureDnsEditorLoaded,
                        DnsTab::FakeIp => Message::EnsureFakeIpEditorLoaded,
                        DnsTab::Tun => Message::EnsureTunEditorLoaded,
                    })
                } else {
                    Task::none()
                }
            }
            Message::SetAdvancedMode(tab, mode) => {
                match tab {
                    DnsTab::Dns => self.dns_mode = mode,
                    DnsTab::FakeIp => self.fake_ip_mode = mode,
                    DnsTab::Tun => self.tun_mode = mode,
                }
                if mode == AdvancedEditMode::Json {
                    Task::done(match tab {
                        DnsTab::Dns => Message::EnsureDnsEditorLoaded,
                        DnsTab::FakeIp => Message::EnsureFakeIpEditorLoaded,
                        DnsTab::Tun => Message::EnsureTunEditorLoaded,
                    })
                } else {
                    Task::none()
                }
            }
            Message::RefreshDnsOnly => Task::perform(
                async {
                    let config = infiltrator_core::dns::load_dns_config()
                        .await
                        .map_err(|e| InfiltratorError::Config(e.to_string()))?;
                    serde_json::to_string_pretty(&config)
                        .map_err(|e| InfiltratorError::Config(e.to_string()))
                },
                Message::DnsConfigJsonLoaded,
            ),
            Message::RefreshFakeIpOnly => Task::perform(
                async {
                    let config = infiltrator_core::fake_ip::load_fake_ip_config()
                        .await
                        .map_err(|e| InfiltratorError::Config(e.to_string()))?;
                    serde_json::to_string_pretty(&config)
                        .map_err(|e| InfiltratorError::Config(e.to_string()))
                },
                Message::FakeIpConfigJsonLoaded,
            ),
            Message::RefreshTunOnly => Task::perform(
                async {
                    let config = infiltrator_core::tun::load_tun_config()
                        .await
                        .map_err(|e| InfiltratorError::Config(e.to_string()))?;
                    serde_json::to_string_pretty(&config)
                        .map_err(|e| InfiltratorError::Config(e.to_string()))
                },
                Message::TunConfigJsonLoaded,
            ),
            Message::EnsureDnsEditorLoaded => {
                self.ensure_dns_editor_loaded();
                Task::none()
            }
            Message::EnsureFakeIpEditorLoaded => {
                self.ensure_fake_ip_editor_loaded();
                Task::none()
            }
            Message::EnsureTunEditorLoaded => {
                self.ensure_tun_editor_loaded();
                Task::none()
            }
            Message::ActivateDnsHeavyView => {
                self.dns_heavy_ready = true;
                if self.active_advanced_mode(self.dns_tab) == AdvancedEditMode::Json {
                    Task::done(match self.dns_tab {
                        DnsTab::Dns => Message::EnsureDnsEditorLoaded,
                        DnsTab::FakeIp => Message::EnsureFakeIpEditorLoaded,
                        DnsTab::Tun => Message::EnsureTunEditorLoaded,
                    })
                } else {
                    Task::none()
                }
            }
            Message::LoadAdvancedConfigs => {
                if !self.advanced_configs_loaded_once {
                    self.reset_dns_lazy_state();
                }
                Task::perform(
                    async {
                        let manager = mihomo_config::ConfigManager::new()
                            .map_err(|e: mihomo_api::MihomoError| InfiltratorError::from(e))?;
                        let profile = manager
                            .get_current()
                            .await
                            .map_err(|e: mihomo_api::MihomoError| InfiltratorError::from(e))?;
                        let content = manager
                            .load(&profile)
                            .await
                            .map_err(|e: mihomo_api::MihomoError| InfiltratorError::from(e))?;
                        let doc: serde_yml::Value = serde_yml::from_str(&content)
                            .map_err(|e| InfiltratorError::Config(e.to_string()))?;

                        let dns = infiltrator_core::dns::extract_dns_config_from_doc(&doc)
                            .map_err(|e| InfiltratorError::Config(e.to_string()))?;
                        let fake_ip =
                            infiltrator_core::fake_ip::extract_fake_ip_config_from_doc(&doc)
                                .map_err(|e| InfiltratorError::Config(e.to_string()))?;
                        let tun = infiltrator_core::tun::extract_tun_config_from_doc(&doc)
                            .map_err(|e| InfiltratorError::Config(e.to_string()))?;

                        Ok(AdvancedConfigsBundle {
                            dns_json: serde_json::to_string_pretty(&dns)
                                .map_err(|e| InfiltratorError::Config(e.to_string()))?,
                            fake_ip_json: serde_json::to_string_pretty(&fake_ip)
                                .map_err(|e| InfiltratorError::Config(e.to_string()))?,
                            tun_json: serde_json::to_string_pretty(&tun)
                                .map_err(|e| InfiltratorError::Config(e.to_string()))?,
                            dns,
                            fake_ip,
                            tun,
                        })
                    },
                    Message::AdvancedConfigsBundleLoaded,
                )
            }
            Message::AdvancedConfigsBundleLoaded(result) => {
                match result {
                    Ok(bundle) => {
                        self.advanced_configs_loaded_once = true;
                        if !self.dns_json_dirty && !self.dns_form_dirty {
                            self.dns_json_cache = bundle.dns_json;
                            self.apply_dns_form_from_config(&bundle.dns);
                            if self.dns_editor_state == EditorLazyState::Loaded {
                                self.ensure_dns_editor_loaded();
                            }
                            self.dns_json_dirty = false;
                            self.dns_form_dirty = false;
                            self.advanced_validation.dns = None;
                        }
                        if !self.fake_ip_json_dirty && !self.fake_ip_form_dirty {
                            self.fake_ip_json_cache = bundle.fake_ip_json;
                            self.apply_fake_ip_form_from_config(&bundle.fake_ip);
                            if self.fake_ip_editor_state == EditorLazyState::Loaded {
                                self.ensure_fake_ip_editor_loaded();
                            }
                            self.fake_ip_json_dirty = false;
                            self.fake_ip_form_dirty = false;
                            self.advanced_validation.fake_ip = None;
                        }
                        if !self.tun_json_dirty && !self.tun_form_dirty {
                            self.tun_json_cache = bundle.tun_json;
                            self.apply_tun_form_from_config(&bundle.tun);
                            if self.tun_editor_state == EditorLazyState::Loaded {
                                self.ensure_tun_editor_loaded();
                            }
                            self.tun_json_dirty = false;
                            self.tun_form_dirty = false;
                            self.advanced_validation.tun = None;
                        }
                    }
                    Err(e) => {
                        self.advanced_configs_loaded_once = false;
                        self.error_msg = Some(e.to_string());
                    }
                }
                Task::none()
            }
            Message::DnsConfigJsonLoaded(result) => {
                match result {
                    Ok(json) => {
                        match serde_json::from_str::<infiltrator_core::dns::DnsConfig>(&json) {
                            Ok(config) => {
                                self.advanced_configs_loaded_once = true;
                                self.dns_json_cache = json;
                                self.apply_dns_form_from_config(&config);
                                if self.dns_editor_state == EditorLazyState::Loaded {
                                    self.ensure_dns_editor_loaded();
                                }
                                self.dns_json_dirty = false;
                                self.dns_form_dirty = false;
                                self.advanced_validation.dns = None;
                            }
                            Err(e) => {
                                self.error_msg = Some(e.to_string());
                            }
                        }
                    }
                    Err(e) => self.error_msg = Some(e.to_string()),
                }
                Task::none()
            }
            Message::FakeIpConfigJsonLoaded(result) => {
                match result {
                    Ok(json) => {
                        match serde_json::from_str::<infiltrator_core::fake_ip::FakeIpConfig>(&json)
                        {
                            Ok(config) => {
                                self.advanced_configs_loaded_once = true;
                                self.fake_ip_json_cache = json;
                                self.apply_fake_ip_form_from_config(&config);
                                if self.fake_ip_editor_state == EditorLazyState::Loaded {
                                    self.ensure_fake_ip_editor_loaded();
                                }
                                self.fake_ip_json_dirty = false;
                                self.fake_ip_form_dirty = false;
                                self.advanced_validation.fake_ip = None;
                            }
                            Err(e) => {
                                self.error_msg = Some(e.to_string());
                            }
                        }
                    }
                    Err(e) => self.error_msg = Some(e.to_string()),
                }
                Task::none()
            }
            Message::TunConfigJsonLoaded(result) => {
                match result {
                    Ok(json) => {
                        match serde_json::from_str::<infiltrator_core::tun::TunConfig>(&json) {
                            Ok(config) => {
                                self.advanced_configs_loaded_once = true;
                                self.tun_json_cache = json;
                                self.apply_tun_form_from_config(&config);
                                if self.tun_editor_state == EditorLazyState::Loaded {
                                    self.ensure_tun_editor_loaded();
                                }
                                self.tun_json_dirty = false;
                                self.tun_form_dirty = false;
                                self.advanced_validation.tun = None;
                            }
                            Err(e) => {
                                self.error_msg = Some(e.to_string());
                            }
                        }
                    }
                    Err(e) => self.error_msg = Some(e.to_string()),
                }
                Task::none()
            }
            Message::UpdateDnsFormEnable(value) => {
                self.dns_form.enable = value;
                self.mark_dns_form_dirty_and_sync();
                Task::none()
            }
            Message::UpdateDnsFormNameserver(value) => {
                self.dns_form.nameserver = value;
                self.mark_dns_form_dirty_and_sync();
                Task::none()
            }
            Message::UpdateDnsFormFallback(value) => {
                self.dns_form.fallback = value;
                self.mark_dns_form_dirty_and_sync();
                Task::none()
            }
            Message::UpdateDnsFormEnhancedMode(value) => {
                self.dns_form.enhanced_mode = value;
                self.mark_dns_form_dirty_and_sync();
                Task::none()
            }
            Message::UpdateDnsFormFakeIpRange(value) => {
                self.dns_form.fake_ip_range = value;
                self.mark_dns_form_dirty_and_sync();
                Task::none()
            }
            Message::UpdateDnsFormFakeIpFilter(value) => {
                self.dns_form.fake_ip_filter = value;
                self.mark_dns_form_dirty_and_sync();
                Task::none()
            }
            Message::UpdateDnsFormIpv6(value) => {
                self.dns_form.ipv6 = value;
                self.mark_dns_form_dirty_and_sync();
                Task::none()
            }
            Message::UpdateDnsFormCache(value) => {
                self.dns_form.cache = value;
                self.mark_dns_form_dirty_and_sync();
                Task::none()
            }
            Message::UpdateDnsFormUseHosts(value) => {
                self.dns_form.use_hosts = value;
                self.mark_dns_form_dirty_and_sync();
                Task::none()
            }
            Message::UpdateDnsFormUseSystemHosts(value) => {
                self.dns_form.use_system_hosts = value;
                self.mark_dns_form_dirty_and_sync();
                Task::none()
            }
            Message::UpdateDnsFormRespectRules(value) => {
                self.dns_form.respect_rules = value;
                self.mark_dns_form_dirty_and_sync();
                Task::none()
            }
            Message::UpdateDnsFormProxyServerNameserver(value) => {
                self.dns_form.proxy_server_nameserver = value;
                self.mark_dns_form_dirty_and_sync();
                Task::none()
            }
            Message::UpdateDnsFormDirectNameserver(value) => {
                self.dns_form.direct_nameserver = value;
                self.mark_dns_form_dirty_and_sync();
                Task::none()
            }
            Message::UpdateFakeIpFormRange(value) => {
                self.fake_ip_form.fake_ip_range = value;
                self.mark_fake_ip_form_dirty_and_sync();
                Task::none()
            }
            Message::UpdateFakeIpFormFilter(value) => {
                self.fake_ip_form.fake_ip_filter = value;
                self.mark_fake_ip_form_dirty_and_sync();
                Task::none()
            }
            Message::UpdateFakeIpFormStore(value) => {
                self.fake_ip_form.store_fake_ip = value;
                self.mark_fake_ip_form_dirty_and_sync();
                Task::none()
            }
            Message::UpdateTunFormEnable(value) => {
                self.tun_form.enable = value;
                self.mark_tun_form_dirty_and_sync();
                Task::none()
            }
            Message::UpdateTunFormStack(value) => {
                self.tun_form.stack = value;
                self.mark_tun_form_dirty_and_sync();
                Task::none()
            }
            Message::UpdateTunFormMtu(value) => {
                self.tun_form.mtu = value;
                self.mark_tun_form_dirty_and_sync();
                Task::none()
            }
            Message::UpdateTunFormDnsHijack(value) => {
                self.tun_form.dns_hijack = value;
                self.mark_tun_form_dirty_and_sync();
                Task::none()
            }
            Message::UpdateTunFormAutoRoute(value) => {
                self.tun_form.auto_route = value;
                self.mark_tun_form_dirty_and_sync();
                Task::none()
            }
            Message::UpdateTunFormAutoDetectInterface(value) => {
                self.tun_form.auto_detect_interface = value;
                self.mark_tun_form_dirty_and_sync();
                Task::none()
            }
            Message::UpdateTunFormStrictRoute(value) => {
                self.tun_form.strict_route = value;
                self.mark_tun_form_dirty_and_sync();
                Task::none()
            }
            Message::DnsConfigEditorAction(action) => {
                self.ensure_dns_editor_loaded();
                self.dns_json_content.perform(action);
                self.dns_json_cache = self.dns_json_content.text();
                self.dns_json_dirty = true;
                self.advanced_validation.dns = None;
                Task::none()
            }
            Message::FakeIpConfigEditorAction(action) => {
                self.ensure_fake_ip_editor_loaded();
                self.fake_ip_json_content.perform(action);
                self.fake_ip_json_cache = self.fake_ip_json_content.text();
                self.fake_ip_json_dirty = true;
                self.advanced_validation.fake_ip = None;
                Task::none()
            }
            Message::TunConfigEditorAction(action) => {
                self.ensure_tun_editor_loaded();
                self.tun_json_content.perform(action);
                self.tun_json_cache = self.tun_json_content.text();
                self.tun_json_dirty = true;
                self.advanced_validation.tun = None;
                Task::none()
            }
            Message::UpdateDnsServer(index, server) => {
                if let Some(target) = self.dns_nameservers.get_mut(index) {
                    *target = server;
                }
                Task::none()
            }
            Message::UpdateDnsEnhancedMode(mode) => {
                self.dns_enhanced_mode = mode;
                Task::none()
            }
            Message::AddDnsServer => {
                self.dns_nameservers.push(String::new());
                Task::none()
            }
            Message::AddDnsServerTemplate(server) => {
                self.dns_nameservers.push(server);
                Task::none()
            }
            Message::RemoveDnsServer(index) => {
                if self.dns_nameservers.len() > index {
                    self.dns_nameservers.remove(index);
                }
                Task::none()
            }
            Message::UpdateFallbackDnsServer(index, value) => {
                if let Some(server) = self.dns_fallback_servers.get_mut(index) {
                    *server = value;
                }
                Task::none()
            }
            Message::AddFallbackDnsServer => {
                self.dns_fallback_servers.push(String::new());
                Task::none()
            }
            Message::RemoveFallbackDnsServer(index) => {
                if self.dns_fallback_servers.len() > index {
                    self.dns_fallback_servers.remove(index);
                }
                Task::none()
            }
            Message::SaveDns => {
                self.is_saving_dns = true;
                self.begin_save_phase("DNS");
                let patch = if self.dns_mode == AdvancedEditMode::Form {
                    self.dns_patch_from_form()
                } else {
                    self.ensure_dns_editor_loaded();
                    let text = self.dns_json_content.text();
                    self.dns_json_cache = text.clone();
                    serde_json::from_str::<infiltrator_core::dns::DnsConfigPatch>(&text)
                        .map_err(|e| InfiltratorError::Config(format!("Invalid DNS JSON: {}", e)))
                };
                let patch = match patch {
                    Ok(value) => value,
                    Err(error) => {
                        self.is_saving_dns = false;
                        let mapped = Self::map_advanced_error_message(&error);
                        self.advanced_validation.dns = Some(mapped.clone());
                        self.rebuild_flow = RebuildFlowState::Failed {
                            label: "DNS".to_string(),
                            error: mapped.clone(),
                        };
                        self.error_msg = Some(mapped.clone());
                        return Task::batch(vec![
                            Task::done(Message::ShowToast(mapped, ToastStatus::Error)),
                            Task::perform(
                                async {
                                    tokio::time::sleep(tokio::time::Duration::from_secs(4)).await;
                                },
                                |_| Message::ClearRebuildFlow,
                            ),
                        ]);
                    }
                };
                Task::perform(
                    async move {
                        infiltrator_core::dns::save_dns_config(patch)
                            .await
                            .map_err(|e| InfiltratorError::Config(e.to_string()))?;
                        Ok(())
                    },
                    Message::DnsSaved,
                )
            }
            Message::DnsSaved(result) => {
                self.is_saving_dns = false;
                match result {
                    Ok(_) => {
                        self.dns_form_dirty = false;
                        self.dns_json_dirty = false;
                        self.advanced_validation.dns = None;
                        Task::batch(vec![
                            Task::done(Message::RefreshDnsOnly),
                            self.trigger_runtime_rebuild(),
                        ])
                    }
                    Err(e) => {
                        let mapped = Self::map_advanced_error_message(&e);
                        self.advanced_validation.dns = Some(mapped.clone());
                        self.rebuild_flow = RebuildFlowState::Failed {
                            label: "DNS".to_string(),
                            error: mapped.clone(),
                        };
                        self.error_msg = Some(mapped.clone());
                        Task::batch(vec![
                            Task::done(Message::ShowToast(mapped, ToastStatus::Error)),
                            Task::perform(
                                async {
                                    tokio::time::sleep(tokio::time::Duration::from_secs(4)).await;
                                },
                                |_| Message::ClearRebuildFlow,
                            ),
                        ])
                    }
                }
            }
            Message::SaveFakeIpConfig => {
                self.is_saving_fake_ip = true;
                self.begin_save_phase("Fake-IP");
                let patch = if self.fake_ip_mode == AdvancedEditMode::Form {
                    self.fake_ip_patch_from_form()
                } else {
                    self.ensure_fake_ip_editor_loaded();
                    let text = self.fake_ip_json_content.text();
                    self.fake_ip_json_cache = text.clone();
                    serde_json::from_str::<infiltrator_core::fake_ip::FakeIpConfigPatch>(&text)
                        .map_err(|e| {
                            InfiltratorError::Config(format!("Invalid Fake-IP JSON: {}", e))
                        })
                };
                let patch = match patch {
                    Ok(value) => value,
                    Err(error) => {
                        self.is_saving_fake_ip = false;
                        let mapped = Self::map_advanced_error_message(&error);
                        self.advanced_validation.fake_ip = Some(mapped.clone());
                        self.rebuild_flow = RebuildFlowState::Failed {
                            label: "Fake-IP".to_string(),
                            error: mapped.clone(),
                        };
                        self.error_msg = Some(mapped.clone());
                        return Task::batch(vec![
                            Task::done(Message::ShowToast(mapped, ToastStatus::Error)),
                            Task::perform(
                                async {
                                    tokio::time::sleep(tokio::time::Duration::from_secs(4)).await;
                                },
                                |_| Message::ClearRebuildFlow,
                            ),
                        ]);
                    }
                };
                Task::perform(
                    async move {
                        infiltrator_core::fake_ip::save_fake_ip_config(patch)
                            .await
                            .map_err(|e| InfiltratorError::Config(e.to_string()))?;
                        Ok(())
                    },
                    Message::FakeIpConfigSaved,
                )
            }
            Message::FakeIpConfigSaved(result) => {
                self.is_saving_fake_ip = false;
                match result {
                    Ok(_) => {
                        self.fake_ip_form_dirty = false;
                        self.fake_ip_json_dirty = false;
                        self.advanced_validation.fake_ip = None;
                        Task::batch(vec![
                            Task::done(Message::RefreshFakeIpOnly),
                            self.trigger_runtime_rebuild(),
                        ])
                    }
                    Err(e) => {
                        let mapped = Self::map_advanced_error_message(&e);
                        self.advanced_validation.fake_ip = Some(mapped.clone());
                        self.rebuild_flow = RebuildFlowState::Failed {
                            label: "Fake-IP".to_string(),
                            error: mapped.clone(),
                        };
                        self.error_msg = Some(mapped.clone());
                        Task::batch(vec![
                            Task::done(Message::ShowToast(mapped, ToastStatus::Error)),
                            Task::perform(
                                async {
                                    tokio::time::sleep(tokio::time::Duration::from_secs(4)).await;
                                },
                                |_| Message::ClearRebuildFlow,
                            ),
                        ])
                    }
                }
            }
            Message::SaveTunConfig => {
                self.is_saving_tun = true;
                self.begin_save_phase("TUN");
                let patch = if self.tun_mode == AdvancedEditMode::Form {
                    self.tun_patch_from_form()
                } else {
                    self.ensure_tun_editor_loaded();
                    let text = self.tun_json_content.text();
                    self.tun_json_cache = text.clone();
                    serde_json::from_str::<infiltrator_core::tun::TunConfigPatch>(&text)
                        .map_err(|e| InfiltratorError::Config(format!("Invalid TUN JSON: {}", e)))
                };
                let patch = match patch {
                    Ok(value) => value,
                    Err(error) => {
                        self.is_saving_tun = false;
                        let mapped = Self::map_advanced_error_message(&error);
                        self.advanced_validation.tun = Some(mapped.clone());
                        self.rebuild_flow = RebuildFlowState::Failed {
                            label: "TUN".to_string(),
                            error: mapped.clone(),
                        };
                        self.error_msg = Some(mapped.clone());
                        return Task::batch(vec![
                            Task::done(Message::ShowToast(mapped, ToastStatus::Error)),
                            Task::perform(
                                async {
                                    tokio::time::sleep(tokio::time::Duration::from_secs(4)).await;
                                },
                                |_| Message::ClearRebuildFlow,
                            ),
                        ]);
                    }
                };
                Task::perform(
                    async move {
                        infiltrator_core::tun::save_tun_config(patch)
                            .await
                            .map_err(|e| InfiltratorError::Config(e.to_string()))?;
                        Ok(())
                    },
                    Message::TunConfigSaved,
                )
            }
            Message::TunConfigSaved(result) => {
                self.is_saving_tun = false;
                match result {
                    Ok(_) => {
                        self.tun_form_dirty = false;
                        self.tun_json_dirty = false;
                        self.advanced_validation.tun = None;
                        Task::batch(vec![
                            Task::done(Message::RefreshTunOnly),
                            self.trigger_runtime_rebuild(),
                        ])
                    }
                    Err(e) => {
                        let mapped = Self::map_advanced_error_message(&e);
                        self.advanced_validation.tun = Some(mapped.clone());
                        self.rebuild_flow = RebuildFlowState::Failed {
                            label: "TUN".to_string(),
                            error: mapped.clone(),
                        };
                        self.error_msg = Some(mapped.clone());
                        Task::batch(vec![
                            Task::done(Message::ShowToast(mapped, ToastStatus::Error)),
                            Task::perform(
                                async {
                                    tokio::time::sleep(tokio::time::Duration::from_secs(4)).await;
                                },
                                |_| Message::ClearRebuildFlow,
                            ),
                        ])
                    }
                }
            }
            Message::FlushFakeIpCache => {
                if let Some(rt) = self.runtime.clone() {
                    Task::perform(
                        async move {
                            rt.client()
                                .flush_fakeip_cache()
                                .await
                                .map_err(InfiltratorError::from)
                        },
                        Message::OperationResult,
                    )
                } else {
                    Task::none()
                }
            }
            Message::TestProxyDelay(name) => {
                if let Some(rt) = self.runtime.clone() {
                    let n = name.clone();
                    let test_url = self.normalized_delay_test_url();
                    let timeout_ms = self.normalized_delay_timeout_ms();
                    self.runtime_testing_delay_proxy = name.clone();
                    Task::perform(
                        async move {
                            rt.client()
                                .test_delay(&n, &test_url, timeout_ms)
                                .await
                                .map(|d| d as u64)
                                .map_err(InfiltratorError::from)
                        },
                        move |res| Message::ProxyTested(name, res),
                    )
                } else {
                    Task::none()
                }
            }
            Message::ProxyTested(name, result) => {
                self.runtime_testing_delay_proxy.clear();
                match result {
                    Ok(delay) => Task::batch(vec![
                        Task::done(Message::LoadProxies),
                        Task::done(Message::ShowToast(
                            format!("{}: {}ms", name, delay),
                            ToastStatus::Success,
                        )),
                    ]),
                    Err(e) => Task::done(Message::ShowToast(
                        format!("{}: {}", name, e),
                        ToastStatus::Error,
                    )),
                }
            }
            Message::TestGroupDelay(name) => {
                if let Some(rt) = self.runtime.clone() {
                    let proxies = self.proxies.clone();
                    let test_url = self.normalized_delay_test_url();
                    let timeout_ms = self.normalized_delay_timeout_ms();
                    self.runtime_testing_all_delays = true;
                    Task::perform(
                        async move {
                            let members = proxies
                                .get(&name)
                                .and_then(|p| p.all())
                                .map(|all| all.to_vec())
                                .unwrap_or_default();
                            let mut success = 0usize;
                            let mut failed = 0usize;
                            for m in members {
                                match rt.client().test_delay(&m, &test_url, timeout_ms).await {
                                    Ok(_) => success += 1,
                                    Err(_) => failed += 1,
                                }
                            }
                            Ok((success, failed))
                        },
                        Message::AllProxyDelaysTested,
                    )
                } else {
                    Task::none()
                }
            }
            Message::TestAllProxyDelays => {
                if let Some(rt) = self.runtime.clone() {
                    if self.runtime_testing_all_delays
                        || !self.runtime_testing_delay_proxy.is_empty()
                    {
                        return Task::none();
                    }
                    let test_url = self.normalized_delay_test_url();
                    let timeout_ms = self.normalized_delay_timeout_ms();
                    let candidates: Vec<String> = self
                        .proxies
                        .iter()
                        .filter_map(|(name, info)| {
                            if info.is_group() {
                                None
                            } else {
                                Some(name.clone())
                            }
                        })
                        .collect();
                    self.runtime_testing_all_delays = true;
                    Task::perform(
                        async move {
                            let mut success = 0usize;
                            let mut failed = 0usize;
                            for proxy in candidates {
                                match rt.client().test_delay(&proxy, &test_url, timeout_ms).await {
                                    Ok(_) => success += 1,
                                    Err(_) => failed += 1,
                                }
                            }
                            Ok((success, failed))
                        },
                        Message::AllProxyDelaysTested,
                    )
                } else {
                    Task::none()
                }
            }
            Message::SetAutostart(enabled) => {
                self.autostart_enabled = enabled;
                Task::perform(
                    async move {
                        autostart::set_autostart_enabled(enabled)
                            .map_err(|e: anyhow::Error| InfiltratorError::Internal(e.to_string()))
                    },
                    Message::AutostartSet,
                )
            }
            Message::AutostartSet(result) => {
                if let Err(e) = result {
                    self.autostart_enabled = !self.autostart_enabled;
                    self.error_msg = Some(e.to_string());
                }
                Task::none()
            }
            Message::RuntimeRebuildFinished(result) => {
                let label = self.active_rebuild_label();
                match result {
                    Ok(runtime) => {
                        self.runtime = Some(runtime);
                        self.status = RuntimeStatus::Running;
                        self.runtime_poll_tick = 0;
                        self.runtime_prev_upload_total = None;
                        self.runtime_prev_download_total = None;
                        self.rebuild_flow = RebuildFlowState::Done {
                            label: label.clone(),
                        };
                        Task::batch(vec![
                            Task::done(Message::FetchRuntimeConfig),
                            Task::done(Message::LoadProxies),
                            Task::done(Message::RefreshRuntimeNow),
                            Task::done(Message::ShowToast(
                                format!("{label} saved and rebuilt"),
                                ToastStatus::Success,
                            )),
                            Task::perform(
                                async {
                                    tokio::time::sleep(tokio::time::Duration::from_secs(4)).await;
                                },
                                |_| Message::ClearRebuildFlow,
                            ),
                        ])
                    }
                    Err(e) => {
                        self.status = RuntimeStatus::Error(e.clone());
                        self.error_msg = Some(e.to_string());
                        self.rebuild_flow = RebuildFlowState::Failed {
                            label,
                            error: e.to_string(),
                        };
                        Task::batch(vec![
                            Task::done(Message::ShowToast(
                                format!("Rebuild failed: {}", e),
                                ToastStatus::Error,
                            )),
                            Task::perform(
                                async {
                                    tokio::time::sleep(tokio::time::Duration::from_secs(4)).await;
                                },
                                |_| Message::ClearRebuildFlow,
                            ),
                        ])
                    }
                }
            }
            Message::ClearRebuildFlow => {
                self.rebuild_flow = RebuildFlowState::Idle;
                Task::none()
            }
            Message::CheckCoreUpdate => {
                self.is_checking_update = true;
                Task::perform(
                    async {
                        let vm = VersionManager::new().map_err(InfiltratorError::from)?;
                        vm.install_channel(Channel::Stable)
                            .await
                            .map_err(InfiltratorError::from)
                    },
                    Message::CoreUpdateInfo,
                )
            }
            Message::CoreUpdateInfo(result) => {
                self.is_checking_update = false;
                match result {
                    Ok(version) => {
                        self.latest_core_version = Some(version);
                        Task::none()
                    }
                    Err(e) => {
                        self.error_msg = Some(e.to_string());
                        Task::none()
                    }
                }
            }
            Message::DownloadCore(version) => {
                let stream = stream::channel(
                    100,
                    move |mut output: iced::futures::channel::mpsc::Sender<Message>| async move {
                        let vm = match VersionManager::new() {
                            Ok(v) => v,
                            Err(e) => {
                                let _ = output.try_send(Message::CoreDownloadFinished(Err(
                                    InfiltratorError::from(e),
                                )));
                                return;
                            }
                        };

                        match vm.install(&version).await {
                            Ok(_) => {
                                let _ = output.try_send(Message::CoreDownloadFinished(Ok(version)));
                            }
                            Err(e) => {
                                let _ = output.try_send(Message::CoreDownloadFinished(Err(
                                    InfiltratorError::from(e),
                                )));
                            }
                        }
                    },
                );
                Task::run(stream, |m| m)
            }
            Message::CoreDownloadProgress(progress) => {
                self.download_progress = progress;
                Task::none()
            }
            Message::CoreDownloadFinished(result) => {
                self.download_progress = 0.0;
                match result {
                    Ok(_) => Task::done(Message::LoadKernels),
                    Err(e) => {
                        self.error_msg = Some(e.to_string());
                        Task::done(Message::ShowToast(e.to_string(), ToastStatus::Error))
                    }
                }
            }
            Message::LoadKernels => Task::perform(
                async {
                    let vm = VersionManager::new().map_err(InfiltratorError::from)?;
                    vm.list_installed().await.map_err(InfiltratorError::from)
                },
                Message::KernelsLoaded,
            ),
            Message::KernelsLoaded(result) => {
                match result {
                    Ok(versions) => self.installed_kernels = versions,
                    Err(e) => self.error_msg = Some(e.to_string()),
                }
                Task::none()
            }
            Message::SetDefaultKernel(version) => Task::perform(
                async move {
                    let vm = VersionManager::new().map_err(InfiltratorError::from)?;
                    vm.set_default(&version)
                        .await
                        .map_err(InfiltratorError::from)
                },
                |_| Message::LoadKernels,
            ),
            Message::DeleteKernel(version) => Task::perform(
                async move {
                    let vm = VersionManager::new().map_err(InfiltratorError::from)?;
                    vm.uninstall(&version).await.map_err(InfiltratorError::from)
                },
                |_| Message::LoadKernels,
            ),
            Message::FactoryReset => Task::none(),
            _ => Task::none(),
        }
    }
}
