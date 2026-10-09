//! The routing seam: bounded subtree replacement under the shell's
//! [`ContentSlot`] (charter law — page remount = replace a bounded
//! subtree, docs/bevy-ui/BEVY_UI_FRONTEND.md).
//!
//! Architecture:
//! - [`Route`] / [`RouteChanged`]: typed navigation vocabulary across all 11 pages.
//! - [`RouteHistory`]: complete navigation stack supporting back/forward/push/replace.
//! - [`sync_route`]: global observer on [`RouteChanged`]; despawns the mounted
//!   page subtree below [`ContentSlot`] and mounts the new page scene with
//!   `spawn_scene` and `ChildOf(slot)`.
//! - Idempotency & Re-entrancy: same-route triggers short-circuit; multiple
//!   transitions in one frame queue despawn before spawn to cleanly converge on
//!   one active page tree without leaked entities.

use crate::app::{ContentSlot, SidebarFoot, SidebarToggleProjection};
use crate::capture::page_from_env;
use crate::command_execution::drain_command_results;
use crate::history::TrafficHistory;
use crate::ime::ShellImeSet;
use crate::localization::read_language_preference;
use crate::pages::app_routing::{AppRoutingProjectionUpdated, app_routing_page};
use crate::pages::business_panel::{BusinessPanelState, activate_panel, cancel_panel};
use crate::pages::connections::{
    ConnectionsProjectionUpdated, LastConnectionsProjection, connections_page,
};
use crate::pages::connections_confirm::{cancel_on_escape_or_navigation, request_confirmation};
use crate::pages::connections_drawer::dismiss_connection_inspection;
use crate::pages::connections_drawer::{
    ConnectionsDrawerState, ConnectionsRuleDraft, reconcile_connection_inspection,
    sync_connections_drawer,
};
use crate::pages::connections_idle::ConnectionsIdleState;
use crate::pages::connections_pulse::{ConnectionsPulseState, animate_connection_pulses};
use crate::pages::connections_view::{ConnectionsCloseAllState, ConnectionsViewState};
use crate::pages::connections_virtual::{ConnectionsVirtualState, sync_connections_virtual_window};
use crate::pages::dns::{DnsProjectionUpdated, LastDnsProjection, dns_page};
use crate::pages::dns_cache::{self, CacheConfirmation};
use crate::pages::dns_cache_scene::spawn;
use crate::pages::dns_edit::sync_dns_edit_dirty;
use crate::pages::dns_fakeip::sync_dns_fake_ip_filter;
use crate::pages::dns_hosts::DnsHostsEditorState;
use crate::pages::dns_leak::sync_dns_leak_line;
use crate::pages::dns_leak_actions::{self, LeakActions};
use crate::pages::dns_leak_rows;
use crate::pages::dns_query::{self, QueryPanel};
use crate::pages::dns_query_scene;
use crate::pages::dns_self_heal::sync_dns_observation_lines;
use crate::pages::doctor::{DoctorProjectionUpdated, LastDoctorProjection, doctor_page};
use crate::pages::doctor_actions::{DoctorActions, on_action, on_result, sync_controls};
use crate::pages::doctor_rows::{reconcile_rows, replay_copy, replay_header, sort_rows};
use crate::pages::logs::{LogsProjectionUpdated, logs_page};
use crate::pages::logs_virtual::{LogsVirtualState, sync_logs_virtual_window};
use crate::pages::overview::{
    LastOverviewProjection, OverviewProjectionUpdated, banner_note, overview_page,
    replay_projection_after_theme, sync_overview_metrics_columns,
};
use crate::pages::overview_lifecycle::CoreControlPlugin;
use crate::pages::overview_restamp::reskin_overview_tokens;
use crate::pages::overview_speedtest::dismiss_speedtest_detail;
use crate::pages::overview_speedtest::{
    sync_overview_speedtest_button, sync_overview_speedtest_detail,
};
use crate::pages::overview_topology::{
    TopologyDrilldownFilter, sync_overview_responsive, sync_topology_hover_highlight,
};
use crate::pages::plugin::PageBindingsPlugin;
use crate::pages::profiles::{ProfilesProjectionUpdated, profiles_page};
use crate::pages::profiles_aggregator::replay_aggregation_copy;
use crate::pages::proxies::scene::proxies_page;
use crate::pages::proxies::{ProxiesProjectionUpdated, sync_proxies_node_columns};
use crate::pages::proxies_form::{CustomNodeForm, finish_custom_node, sync_protocol_form};
use crate::pages::proxies_highlight::sync_name_highlights;
use crate::pages::proxies_preferences::{
    sync_proxy_favorites, sync_proxy_preferences, sync_proxy_vertical_spacing,
};
use crate::pages::proxies_reconcile::{
    reconcile_proxy_cards, reconcile_proxy_nodes, sort_proxy_children,
};
use crate::pages::proxies_search::{
    ProxySearchState, finish_search, on_clear_search, on_retry_search, sync_search_input,
    sync_search_status,
};
use crate::pages::proxies_virtual::sync_proxies_virtual_window;
use crate::pages::proxy_group_order::{
    GroupOrderState, activate_order, finish_order, reconcile_order_rows, sort_order_rows,
    sync_order_editor, sync_order_surface,
};
use crate::pages::proxy_inspection::ProxyInspectionPlugin;
use crate::pages::proxy_probe_settings::{
    ProbeSettingsState, activate, finish, sync_editor, sync_fields, sync_surface,
};
use crate::pages::rules::{RulesProjectionUpdated, rules_page};
use crate::pages::rules_builder_input;
use crate::pages::rules_draft;
use crate::pages::rules_json::{
    refresh_rules_json_body, restamp_rules_json, rules_json_keyboard_input,
};
use crate::pages::rules_row_copy;
use crate::pages::rules_tabs::sync_rules_tabs;
use crate::pages::rules_tracer::{finish_simulation, observe_simulation, sync_basic_drafts};
use crate::pages::rules_tracer_confirm;
use crate::pages::rules_tracer_sandbox;
use crate::pages::rules_view::{sync_rules_view, sync_rules_window};
use crate::pages::settings::settings_copy::replay_locale;
use crate::pages::settings::{LastSettingsProjection, SettingsProjectionUpdated, settings_page};
use crate::pages::settings_language::{
    LanguageChoiceResource, activate_language, finish_language, sync_language_controls,
    sync_language_model,
};
use crate::pages::snapshot_restore::SnapshotRestorePlugin;
use crate::pages::sync::{SyncProjectionUpdated, sync_page};
use crate::pages::{dns_hosts_actions, dns_hosts_rows, dns_hosts_scene, dns_hosts_sync};
use crate::projection::OverviewSource;
use crate::route::mount::sync_route;
use crate::surface::{
    DemoSurfaceSource, LatestCoreLifecycle, LatestSurfaceSnapshot, LegacyOverviewSurfaceSource,
    SurfaceOverviewAdapter, SurfaceSnapshotUpdated, SurfaceSource, SurfaceStatusBanner,
    UnavailableSurfaceSource, app_routing_projection, connections_projection,
    core_lifecycle_projection, dns_projection, doctor_projection, logs_projection,
    overview_projection, profiles_projection, proxies_projection, rules_projection,
    settings_projection, status_banner_scene, sync_projection,
};
use bevy::app::{App, Plugin, Startup, Update};
use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::event::Event;
use bevy::ecs::hierarchy::ChildOf;
use bevy::ecs::lifecycle::Add;
use bevy::ecs::observer::On;
use bevy::ecs::query::With;
use bevy::ecs::resource::Resource;
use bevy::ecs::schedule::IntoScheduleConfigs;
use bevy::ecs::system::{Commands, Query, Res, ResMut};
use bevy::input::keyboard::KeyboardInput;
use bevy::scene::{CommandsSceneExt, Scene};
use bevy::ui::widget::Text;
use infiltrator_application::system_toggle_application::SystemToggleApplication;
use infiltrator_bevy_widgets::button::{sync_control_labels, sync_control_visuals};
use infiltrator_bevy_widgets::icon::IconId;
use infiltrator_bevy_widgets::localization::{LocalizedText, UiLocale};
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_contract::command_catalogue::ShellPage;
use infiltrator_contract::surface_snapshot::{PageId, PageStatus, SurfaceSnapshot};
use std::collections::HashSet;
use std::sync::Arc;

pub mod mount;

/// The app's pages. New pages append a variant and an arm in
/// [`page_scene`] — never a second mount path.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Route {
    /// 核心概览 (Home / Overview: run state, proxy mode, traffic, connections).
    #[default]
    Overview,
    /// 代理策略 (Proxies & Groups: selector, latency, test).
    Proxies,
    /// 配置订阅 (Profiles & Subscriptions: list, import, auto-update).
    Profiles,
    /// 分流规则 (Rules: ruleset, MRS, rule tracer).
    Rules,
    /// 连接审计 (Connections: active connections, bandwidth, disconnect).
    Connections,
    /// 运行日志 (Logs: level filter, ring buffer, regex search).
    Logs,
    /// 域名解析 (DNS: server table, fake-ip, dot/doh).
    Dns,
    /// 自愈诊断 (Doctor: system diagnostics, tun health, port scan).
    Doctor,
    /// 应用分流 (App Routing: split tunneling, per-app proxy).
    AppRouting,
    /// 数据同步 (Sync: WebDAV, 3-way merge, roaming).
    Sync,
    /// 系统设置 (Settings: autostart, system proxy, tun stack, theme).
    Settings,
}

impl Route {
    /// Every route in stable enumeration order.
    pub const ALL: [Route; 11] = [
        Route::Overview,
        Route::Proxies,
        Route::Profiles,
        Route::Rules,
        Route::Connections,
        Route::Logs,
        Route::Dns,
        Route::Doctor,
        Route::AppRouting,
        Route::Sync,
        Route::Settings,
    ];

    /// Canonical copy identity shared with the other peer.
    pub const fn label_key(self) -> &'static str {
        match self {
            Self::Overview => "nav_overview",
            Self::Proxies => "nav_proxies",
            Self::Profiles => "nav_profiles",
            Self::Rules => "nav_rules",
            Self::Connections => "nav_connections",
            Self::Logs => "nav_logs",
            Self::Dns => "nav_dns",
            Self::Doctor => "nav_doctor",
            Self::AppRouting => "nav_app_routing",
            Self::Sync => "nav_sync",
            Self::Settings => "nav_settings",
        }
    }

    /// Initial copy for consumers without a mounted locale resource.
    pub fn label(self) -> String {
        LocalizedText::plain(self.label_key()).render(&UiLocale::default())
    }

    /// Semantic icon id for each route.
    pub const fn icon(&self) -> IconId {
        match self {
            Self::Overview => IconId::Activity,
            Self::Proxies => IconId::Globe,
            Self::Profiles => IconId::FileText,
            Self::Rules => IconId::Network,
            Self::Connections => IconId::Zap,
            Self::Logs => IconId::FileText,
            Self::Dns => IconId::Globe,
            Self::Doctor => IconId::Activity,
            Self::AppRouting => IconId::Network,
            Self::Sync => IconId::Settings,
            Self::Settings => IconId::Settings,
        }
    }

    /// Map a shared command-catalogue page onto this surface's route. The Bevy
    /// route set covers every shared page one-to-one (the Iced surface folds
    /// `Logs` into its runtime page and keeps a local editor page).
    pub const fn from_shell_page(page: ShellPage) -> Self {
        match page {
            ShellPage::Overview => Self::Overview,
            ShellPage::Proxies => Self::Proxies,
            ShellPage::Profiles => Self::Profiles,
            ShellPage::Rules => Self::Rules,
            ShellPage::Connections => Self::Connections,
            ShellPage::Logs => Self::Logs,
            ShellPage::Dns => Self::Dns,
            ShellPage::Doctor => Self::Doctor,
            ShellPage::AppRouting => Self::AppRouting,
            ShellPage::Sync => Self::Sync,
            ShellPage::Settings => Self::Settings,
        }
    }
}

/// A navigation request. Observed by [`sync_route`] (installed by
/// [`PagesPlugin`]); trigger with `commands.trigger(RouteChanged(…))`.
#[derive(Event, Clone, Copy, Debug, PartialEq, Eq)]
pub struct RouteChanged(pub Route);

/// Event requesting navigation back in the route history stack.
#[derive(Event, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct NavigateBack;

/// Event requesting navigation forward in the route history stack.
#[derive(Event, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct NavigateForward;

/// Navigation history stack tracking back/forward transitions.
#[derive(Resource, Clone, Debug, PartialEq, Eq)]
pub struct RouteHistory {
    back_stack: Vec<Route>,
    forward_stack: Vec<Route>,
    max_depth: usize,
}

impl Default for RouteHistory {
    fn default() -> Self {
        Self {
            back_stack: vec![Route::default()],
            forward_stack: Vec::new(),
            max_depth: 50,
        }
    }
}

impl RouteHistory {
    /// Create history with a specific max depth.
    pub fn new(max_depth: usize) -> Self {
        Self {
            back_stack: vec![Route::default()],
            forward_stack: Vec::new(),
            max_depth,
        }
    }

    /// Current active route at top of stack.
    pub fn current(&self) -> Route {
        self.back_stack.last().copied().unwrap_or_default()
    }

    /// Push a new route onto the stack, clearing forward history.
    pub fn push(&mut self, route: Route) {
        if self.current() == route {
            return;
        }
        self.back_stack.push(route);
        if self.back_stack.len() > self.max_depth {
            self.back_stack.remove(0);
        }
        self.forward_stack.clear();
    }

    /// Replace current top of stack with a new route.
    pub fn replace(&mut self, route: Route) {
        if let Some(top) = self.back_stack.last_mut() {
            *top = route;
        } else {
            self.back_stack.push(route);
        }
        self.forward_stack.clear();
    }

    /// Can the user navigate back?
    pub fn can_go_back(&self) -> bool {
        self.back_stack.len() > 1
    }

    /// Can the user navigate forward?
    pub fn can_go_forward(&self) -> bool {
        !self.forward_stack.is_empty()
    }

    /// Navigate back: pops current route to forward stack, returns previous route.
    pub fn go_back(&mut self) -> Option<Route> {
        if !self.can_go_back() {
            return None;
        }
        let current = self.back_stack.pop()?;
        self.forward_stack.push(current);
        Some(self.current())
    }

    /// Navigate forward: pops from forward stack to back stack, returns new route.
    pub fn go_forward(&mut self) -> Option<Route> {
        let next = self.forward_stack.pop()?;
        self.back_stack.push(next);
        Some(next)
    }
}

/// Marker on a mounted page's root entity, carrying its route. Tests and
/// nav chrome assert on it; the exactly-one-page invariant lives here.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PageRoot(pub Route);

/// Mirror of the currently mounted route (`None` before the first
/// mount). Identical triggers short-circuit against this so a shown page
/// keeps its entity ids.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ActiveRoute(pub Option<Route>);

/// The injected projection source behind the Overview page. [`Arc`]
/// because [`PagesPlugin`] must hand its configured source through the
/// `&self` plugin-build seam.
#[derive(Resource, Clone)]
pub struct OverviewSourceHandle(pub Arc<dyn OverviewSource>);

/// The complete shared surface source. The old Overview handle remains a
/// compatibility seam for Overview-specific tests and UI chrome; pages use
/// this source for all 11 projections.
#[derive(Resource, Clone)]
pub struct SurfaceSourceHandle(pub Arc<dyn SurfaceSource>);

/// Installs routing and the first page. Reads the shell ([`ContentSlot`],
/// [`UiPalette`]) — add it after [`ShellPlugin`](crate::app::ShellPlugin).
/// The default source is the demo fixture; inject any [`OverviewSource`]
/// with [`PagesPlugin::new`].
pub struct PagesPlugin {
    overview_source: Arc<dyn OverviewSource>,
    surface_source: Arc<dyn SurfaceSource>,
}

impl PagesPlugin {
    /// Inject an Overview source. Demo sources receive the complete demo
    /// snapshot; live Overview-only sources receive typed unavailable state
    /// for pages that have not been composed.
    pub fn new(source: impl OverviewSource + 'static) -> Self {
        let overview_source: Arc<dyn OverviewSource> = Arc::new(source);
        let surface_source: Arc<dyn SurfaceSource> = Arc::new(
            LegacyOverviewSurfaceSource::from_arc(Arc::clone(&overview_source)),
        );
        Self {
            overview_source,
            surface_source,
        }
    }

    /// Inject the complete application-owned surface source. This is the
    /// production entry for a host that has composed all page readers.
    pub fn new_surface(source: impl SurfaceSource + 'static) -> Self {
        Self::new_surface_arc(Arc::new(source))
    }

    pub fn new_surface_arc(source: Arc<dyn SurfaceSource>) -> Self {
        let overview_source: Arc<dyn OverviewSource> =
            Arc::new(SurfaceOverviewAdapter(Arc::clone(&source)));
        Self {
            overview_source,
            surface_source: source,
        }
    }

    /// Explicit deterministic demo composition for screenshot/test hosts.
    pub fn demo() -> Self {
        Self::new_surface(DemoSurfaceSource::running())
    }
}

impl Default for PagesPlugin {
    fn default() -> Self {
        Self::new_surface(UnavailableSurfaceSource::default())
    }
}

impl Plugin for PagesPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(SnapshotRestorePlugin);
        app.add_plugins(PageBindingsPlugin);
        app.add_plugins(CoreControlPlugin);
        app.add_plugins(ProxyInspectionPlugin);
        app.init_resource::<ActiveRoute>();
        app.init_resource::<RouteHistory>();
        app.init_resource::<LastOverviewProjection>();
        app.init_resource::<TopologyDrilldownFilter>();
        app.init_resource::<LastDnsProjection>();
        app.init_resource::<LeakActions>();
        app.add_systems(
            Update,
            (
                dns_leak_rows::reconcile,
                dns_leak_rows::sort,
                dns_leak_rows::replay,
            )
                .chain(),
        );
        app.add_observer(dns_leak_actions::activate);
        app.add_observer(dns_leak_actions::finish);
        app.add_systems(
            Update,
            dns_leak_actions::sync_controls.after(drain_command_results),
        );
        app.init_resource::<LastDoctorProjection>();
        app.init_resource::<DoctorActions>();
        app.add_observer(on_action);
        app.add_observer(on_result);
        app.add_systems(
            Update,
            (
                reconcile_rows,
                sort_rows,
                replay_copy,
                replay_header,
                sync_controls,
            )
                .chain()
                .after(drain_command_results),
        );
        app.init_resource::<QueryPanel>();
        app.add_systems(Startup, dns_query_scene::spawn);
        app.add_observer(dns_query::activate);
        app.add_observer(dns_query::finish);
        app.add_systems(
            Update,
            rules_builder_input::sync_selection.after(rules_builder_input::sync),
        );
        app.add_systems(
            Update,
            rules_builder_input::sync_status.after(rules_builder_input::sync),
        );
        app.add_systems(
            Update,
            rules_draft::sync_form_discard.after(rules_draft::observe),
        );
        app.add_systems(
            Update,
            rules_builder_input::sync
                .after(rules_draft::observe)
                .after(ShellImeSet::Composition)
                .before(rules_draft::render),
        );
        app.add_systems(
            Update,
            rules_row_copy::sync
                .after(sync_rules_window)
                .after(rules_draft::observe),
        );
        app.add_observer(rules_draft::activate);
        app.add_observer(rules_draft::finish);
        app.add_systems(
            Update,
            rules_draft::observe
                .before(sync_rules_view)
                .before(observe_simulation)
                .before(sync_rules_window),
        );
        app.add_systems(
            Update,
            rules_draft::render
                .after(rules_draft::observe)
                .after(sync_rules_window),
        );
        app.add_observer(finish_simulation);
        app.add_observer(rules_tracer_sandbox::activate);
        app.add_systems(
            Update,
            (
                rules_tracer_sandbox::sync,
                sync_basic_drafts,
                observe_simulation,
                rules_tracer_confirm::render,
            )
                .chain(),
        );
        app.add_systems(Startup, rules_tracer_confirm::spawn);
        app.add_observer(rules_tracer_confirm::activate);
        app.add_observer(rules_tracer_confirm::finish);
        app.add_observer(rules_tracer_confirm::dismiss_scrim);
        app.add_systems(
            Update,
            (dns_query::observe, dns_query::render)
                .chain()
                .after(drain_command_results),
        );
        app.init_resource::<CacheConfirmation>();
        app.add_systems(Startup, spawn);
        app.add_observer(dns_cache::request);
        app.add_observer(dns_cache::activate);
        app.add_observer(dns_cache::finish);
        app.add_systems(
            Update,
            (dns_cache::observe, dns_cache::render)
                .chain()
                .after(drain_command_results),
        );
        app.init_resource::<DnsHostsEditorState>();
        app.add_systems(Startup, dns_hosts_scene::spawn_modal);
        app.add_observer(dns_hosts_actions::activate);
        app.add_observer(dns_hosts_actions::finish);
        app.add_systems(
            Update,
            (
                dns_hosts_sync::observe,
                dns_hosts_sync::inputs,
                dns_hosts_rows::reconcile,
                dns_hosts_rows::sort,
                dns_hosts_rows::replay,
                dns_hosts_sync::controls,
                dns_hosts_sync::copy,
            )
                .chain()
                .after(drain_command_results),
        );
        app.init_resource::<LastConnectionsProjection>();
        app.init_resource::<ConnectionsViewState>();
        app.init_resource::<ConnectionsCloseAllState>();
        app.init_resource::<ConnectionsVirtualState>();
        app.init_resource::<LogsVirtualState>();
        app.init_resource::<BusinessPanelState>();
        app.init_resource::<CustomNodeForm>();
        app.add_observer(activate_panel);
        app.add_observer(finish_custom_node);
        app.add_systems(
            Update,
            (
                (cancel_panel, sync_protocol_form)
                    .chain()
                    .after(drain_command_results),
                dismiss_speedtest_detail,
                dismiss_connection_inspection,
            ),
        );
        app.add_observer(request_confirmation);
        app.add_systems(Update, cancel_on_escape_or_navigation);
        app.init_resource::<ConnectionsIdleState>();
        app.init_resource::<ConnectionsDrawerState>();
        app.init_resource::<ConnectionsRuleDraft>();
        // DUAL-13-10: the shared breathing phase of the high-throughput pulse.
        app.init_resource::<ConnectionsPulseState>();
        // The trend chart's sample ring: written by the live pump's drain
        // (when one is mounted), read by the page's refresh observer and
        // mount scene. The demo fixture ignores it (its trend is the
        // synthetic series — see `history`).
        app.init_resource::<TrafficHistory>();
        app.insert_resource(OverviewSourceHandle(Arc::clone(&self.overview_source)));
        app.insert_resource(SurfaceSourceHandle(Arc::clone(&self.surface_source)));
        // DUAL-11-14: the rules JSON editor routes raw keyboard messages; a
        // headless composition has no InputPlugin, so the queue is registered
        // here (a windowed composition reuses the same queue).
        app.add_message::<KeyboardInput>();
        let initial_snapshot = self.surface_source.surface_snapshot();
        app.insert_resource(LatestCoreLifecycle(core_lifecycle_projection(
            &initial_snapshot,
        )));
        app.insert_resource(LatestSurfaceSnapshot(initial_snapshot.clone()));
        app.insert_resource(SidebarToggleProjection(
            SystemToggleApplication::from_surface(&initial_snapshot),
        ));
        app.add_observer(on_content_slot_added);
        app.add_observer(sync_route);
        app.add_observer(apply_surface_snapshot);
        app.init_resource::<GroupOrderState>();
        app.add_observer(activate_order);
        app.add_observer(finish_order);
        app.add_systems(
            Update,
            (
                sync_order_editor,
                reconcile_order_rows,
                sort_order_rows,
                sync_order_surface,
            )
                .chain(),
        );
        app.init_resource::<LanguageChoiceResource>();
        app.init_resource::<LastSettingsProjection>();
        app.add_systems(Update, replay_locale.after(read_language_preference));
        app.add_systems(
            Update,
            replay_aggregation_copy.after(read_language_preference),
        );
        app.add_observer(activate_language);
        app.add_observer(finish_language);
        app.add_systems(
            Update,
            (
                sync_language_model
                    .before(read_language_preference)
                    .after(drain_command_results),
                sync_language_controls,
            )
                .chain(),
        );
        app.init_resource::<ProbeSettingsState>();
        app.add_observer(activate);
        app.add_observer(finish);
        app.add_systems(Update, (sync_editor, sync_fields, sync_surface).chain());
        app.init_resource::<ProxySearchState>();
        app.add_observer(finish_search);
        app.add_observer(on_clear_search);
        app.add_observer(on_retry_search);
        app.add_systems(Update, (sync_search_input, sync_search_status).chain());
        app.add_observer(reconcile_surface_status);
        app.add_observer(on_page_root_added);
        app.add_observer(on_navigate_back);
        app.add_observer(on_navigate_forward);
        // The Overview page's per-frame token reskin (banner / dot / mode
        // chip / stop button compare-and-set from the live palette).
        app.add_systems(Update, reskin_overview_tokens);
        // State-ink recovery after a theme switch (the widget layer's
        // apply_theme restamps role ink over state ink; the replay re-fires
        // the page's last projection once the switch dispatch is done).
        app.add_observer(replay_projection_after_theme);
        // The sidebar foot follows the injected source (demo caption vs
        // 实时内核 version).
        app.add_systems(
            Update,
            (
                sync_sidebar_foot,
                sync_overview_responsive,
                sync_topology_hover_highlight,
                sync_overview_metrics_columns,
                sync_overview_speedtest_button,
                sync_overview_speedtest_detail,
                (
                    sync_proxies_node_columns,
                    sync_proxy_preferences,
                    sync_proxy_favorites,
                    sync_proxy_vertical_spacing,
                ),
                (
                    reconcile_proxy_cards,
                    reconcile_proxy_nodes,
                    sort_proxy_children,
                    sync_proxies_virtual_window,
                    sync_name_highlights,
                )
                    .chain(),
                // DUAL-11-08: the filter reduction runs first, then the window
                // rebuild mounts exactly the rows the window covers.
                (sync_rules_view, sync_rules_window).chain(),
                // DUAL-11-14: the partition bar restyles from the shared tab
                // state, the JSON body rebuilds from its buffer generation and
                // the editor keys are routed only while it owns focus.
                sync_rules_tabs,
                refresh_rules_json_body,
                restamp_rules_json,
                rules_json_keyboard_input,
                sync_connections_drawer,
                sync_connections_virtual_window,
                sync_logs_virtual_window,
                reconcile_connection_inspection,
                animate_connection_pulses,
                sync_dns_edit_dirty
                    .after(ShellImeSet::Composition)
                    .before(sync_control_visuals)
                    .before(sync_control_labels),
                (
                    sync_dns_fake_ip_filter,
                    sync_dns_observation_lines,
                    sync_dns_leak_line,
                ),
            ),
        );
    }
}

/// Handle navigate back trigger.
fn on_navigate_back(
    _trigger: On<NavigateBack>,
    mut history: ResMut<RouteHistory>,
    mut commands: Commands,
) {
    if let Some(prev) = history.go_back() {
        commands.trigger(RouteChanged(prev));
    }
}

/// Handle navigate forward trigger.
fn on_navigate_forward(
    _trigger: On<NavigateForward>,
    mut history: ResMut<RouteHistory>,
    mut commands: Commands,
) {
    if let Some(next) = history.go_forward() {
        commands.trigger(RouteChanged(next));
    }
}

/// The sidebar foot follows the injected source: `0.30 demo` for the demo
/// fixture, `实时内核 · <version>` for the live pump. Compare-and-set per
/// frame — the live version lands with the pump's first successful sample
/// and the caption picks it up with no event of its own.
fn sync_sidebar_foot(
    handle: Res<OverviewSourceHandle>,
    locale: Res<UiLocale>,
    mut foot: Query<&mut Text, With<SidebarFoot>>,
) {
    let want = banner_note(&handle.0.current(), locale.code());
    for mut text in &mut foot {
        if text.0 != want {
            text.0 = want.clone();
        }
    }
}

/// Mount the default route the moment the shell's content slot lands —
/// the routing bootstrap with zero schedule-ordering assumptions.
fn on_content_slot_added(_ready: On<Add<ContentSlot>>, mut commands: Commands) {
    let initial = page_from_env().unwrap_or_default();
    commands.trigger(RouteChanged(initial));
}

/// Render a typed status banner for a page whose shared source is loading,
/// empty, unavailable, or failed. Ready pages stay visually unchanged.
fn on_page_root_added(
    trigger: On<Add<PageRoot>>,
    roots: Query<&PageRoot>,
    latest: Res<LatestSurfaceSnapshot>,
    palette: Res<UiPalette>,
    mut commands: Commands,
) {
    let entity = trigger.event().entity;
    let Ok(root) = roots.get(entity) else {
        return;
    };
    let page = page_id_for_route(root.0);
    let status = latest.0.page_status(page).clone();
    if matches!(status, PageStatus::Ready) {
        return;
    }
    commands
        .spawn_scene(status_banner_scene(page, &status, &palette))
        .insert(ChildOf(entity));
}

#[derive(Event, Clone, Debug, PartialEq)]
struct SurfaceStatusChanged(pub SurfaceSnapshot);

/// Reconcile the status banner after a live snapshot replaces the initial
/// placeholder. A page keeps exactly one banner for each non-ready status;
/// Ready removes it, and a changed failure/loading message replaces it.
fn reconcile_surface_status(
    update: On<SurfaceStatusChanged>,
    roots: Query<(Entity, &PageRoot)>,
    banners: Query<(Entity, &SurfaceStatusBanner)>,
    palette: Res<UiPalette>,
    mut commands: Commands,
) {
    let snapshot = &update.0;
    let mut retained_pages = HashSet::new();
    for (entity, banner) in &banners {
        let wanted = snapshot.page_status(banner.page);
        if matches!(wanted, PageStatus::Ready) || wanted != &banner.status {
            commands.entity(entity).despawn_children();
            commands.entity(entity).despawn();
        } else {
            retained_pages.insert(banner.page);
        }
    }

    for (root_entity, root) in &roots {
        let page = page_id_for_route(root.0);
        let status = snapshot.page_status(page);
        if !matches!(status, PageStatus::Ready) && !retained_pages.contains(&page) {
            commands
                .spawn_scene(status_banner_scene(page, status, &palette))
                .insert(ChildOf(root_entity));
        }
    }
}

pub(crate) const fn page_id_for_route(route: Route) -> PageId {
    use infiltrator_contract::surface_snapshot::PageId;
    match route {
        Route::Overview => PageId::Overview,
        Route::Proxies => PageId::Proxies,
        Route::Profiles => PageId::Profiles,
        Route::Rules => PageId::Rules,
        Route::Connections => PageId::Connections,
        Route::Logs => PageId::Logs,
        Route::Dns => PageId::Dns,
        Route::Doctor => PageId::Doctor,
        Route::AppRouting => PageId::AppRouting,
        Route::Sync => PageId::Sync,
        Route::Settings => PageId::Settings,
    }
}

/// The route → scene table. The only place that knows which page backs
/// which route.
fn page_scene(
    route: Route,
    snapshot: &SurfaceSnapshot,
    history: &TrafficHistory,
    palette: &UiPalette,
) -> Box<dyn Scene> {
    let projection = overview_projection(snapshot);
    match route {
        Route::Overview => Box::new(overview_page(&projection, history, palette)),
        Route::Proxies => Box::new(proxies_page(&proxies_projection(snapshot), palette)),
        Route::Profiles => Box::new(profiles_page(&profiles_projection(snapshot), palette)),
        Route::Rules => Box::new(rules_page(&rules_projection(snapshot), palette)),
        Route::Connections => {
            Box::new(connections_page(&connections_projection(snapshot), palette))
        }
        Route::Logs => Box::new(logs_page(&logs_projection(snapshot), palette)),
        Route::Dns => Box::new(dns_page(&dns_projection(snapshot), palette)),
        Route::Doctor => Box::new(doctor_page(&doctor_projection(snapshot), palette)),
        Route::AppRouting => Box::new(app_routing_page(&app_routing_projection(snapshot), palette)),
        Route::Sync => Box::new(sync_page(&sync_projection(snapshot), palette)),
        Route::Settings => Box::new(settings_page(&settings_projection(snapshot), palette)),
    }
}

/// Fan one shared snapshot into the page-local render projections. This is
/// the only place where the Bevy page event vocabulary is derived from the
/// cross-surface contract.
fn trigger_page_projection_events(snapshot: &SurfaceSnapshot, commands: &mut Commands) {
    commands.trigger(OverviewProjectionUpdated(overview_projection(snapshot)));
    if matches!(
        snapshot.pages.proxies.status,
        PageStatus::Ready | PageStatus::Empty
    ) && snapshot.pages.proxies.data.is_some()
    {
        commands.trigger(ProxiesProjectionUpdated(proxies_projection(snapshot)));
    }
    commands.trigger(ProfilesProjectionUpdated(profiles_projection(snapshot)));
    commands.trigger(RulesProjectionUpdated(rules_projection(snapshot)));
    commands.trigger(ConnectionsProjectionUpdated(connections_projection(
        snapshot,
    )));
    commands.trigger(LogsProjectionUpdated(logs_projection(snapshot)));
    commands.trigger(DnsProjectionUpdated(dns_projection(snapshot)));
    commands.trigger(DoctorProjectionUpdated(doctor_projection(snapshot)));
    commands.trigger(AppRoutingProjectionUpdated(app_routing_projection(
        snapshot,
    )));
    commands.trigger(SyncProjectionUpdated(sync_projection(snapshot)));
    commands.trigger(SettingsProjectionUpdated(settings_projection(snapshot)));
}

fn apply_surface_snapshot(
    update: On<SurfaceSnapshotUpdated>,
    latest: Res<LatestSurfaceSnapshot>,
    mut commands: Commands,
) {
    let snapshot = update.0.clone();
    if !snapshot.is_newer_than(&latest.0) {
        return;
    }
    replay_surface_snapshot(snapshot, &mut commands);
}

/// Replay one composed source coherently. Capture-host initialization uses the
/// same projection wiring when it replaces its initial explicit demo source.
pub(crate) fn replay_surface_snapshot(snapshot: SurfaceSnapshot, commands: &mut Commands) {
    commands.insert_resource(LatestSurfaceSnapshot(snapshot.clone()));
    commands.insert_resource(LatestCoreLifecycle(core_lifecycle_projection(&snapshot)));
    commands.insert_resource(SidebarToggleProjection(
        SystemToggleApplication::from_surface(&snapshot),
    ));
    trigger_page_projection_events(&snapshot, commands);
    commands.trigger(SurfaceStatusChanged(snapshot));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn route_history_stack_operations() {
        let mut history = RouteHistory::new(10);
        assert_eq!(history.current(), Route::Overview);
        assert!(!history.can_go_back());
        assert!(!history.can_go_forward());

        history.push(Route::Proxies);
        assert_eq!(history.current(), Route::Proxies);
        assert!(history.can_go_back());

        history.push(Route::Logs);
        assert_eq!(history.current(), Route::Logs);

        // Duplicate push is ignored
        history.push(Route::Logs);
        assert_eq!(history.back_stack.len(), 3);

        // Go back
        assert_eq!(history.go_back(), Some(Route::Proxies));
        assert_eq!(history.current(), Route::Proxies);
        assert!(history.can_go_forward());

        // Go forward
        assert_eq!(history.go_forward(), Some(Route::Logs));
        assert_eq!(history.current(), Route::Logs);
        assert!(!history.can_go_forward());

        // Push invalidates forward stack
        assert_eq!(history.go_back(), Some(Route::Proxies));
        history.push(Route::Dns);
        assert_eq!(history.current(), Route::Dns);
        assert!(!history.can_go_forward());
    }
}
