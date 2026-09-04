//! The routing seam: bounded subtree replacement under the shell's
//! [`ContentSlot`] (charter law — page remount = replace a bounded
//! subtree, docs/BEVY_UI_FRONTEND.md).
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

use std::sync::Arc;

use bevy::app::{App, Plugin, Update};
use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::event::Event;
use bevy::ecs::hierarchy::ChildOf;
use bevy::ecs::lifecycle::Add;
use bevy::ecs::observer::On;
use bevy::ecs::query::With;
use bevy::ecs::resource::Resource;
use bevy::ecs::system::{Commands, Query, Res, ResMut};
use bevy::scene::{CommandsSceneExt, Scene};
use bevy::ui::widget::Text;
use infiltrator_bevy_widgets::palette::UiPalette;

use crate::app::{ContentSlot, SidebarFoot};
use crate::history::TrafficHistory;
use crate::pages::app_routing::{
    AppRoutingProjection, AppRoutingProjectionUpdated, app_routing_page,
};
use crate::pages::connections::{
    ConnectionsProjection, ConnectionsProjectionUpdated, connections_page,
};
use crate::pages::dns::{DnsProjection, DnsProjectionUpdated, dns_page};
use crate::pages::doctor::{DoctorProjection, DoctorProjectionUpdated, doctor_page};
use crate::pages::logs::{LogsProjection, LogsProjectionUpdated, logs_page};
use crate::pages::overview::{
    LastOverviewProjection, OverviewProjectionUpdated, banner_note, overview_page,
    replay_projection_after_theme, reskin_overview_tokens, sync_overview_responsive,
};
use crate::pages::profiles::{ProfilesProjection, ProfilesProjectionUpdated, profiles_page};
use crate::pages::proxies::{ProxiesProjection, ProxiesProjectionUpdated, proxies_page};
use crate::pages::rules::{RulesProjection, RulesProjectionUpdated, rules_page};
use crate::pages::settings::{SettingsProjection, SettingsProjectionUpdated, settings_page};
use crate::pages::sync::{SyncProjection, SyncProjectionUpdated, sync_page};
use crate::projection::{OverviewProjection, OverviewSource, SourceKind};

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

    /// The user-facing label for each route.
    pub const fn label(&self) -> &'static str {
        match self {
            Self::Overview => "核心概览",
            Self::Proxies => "代理策略",
            Self::Profiles => "配置订阅",
            Self::Rules => "分流规则",
            Self::Connections => "连接审计",
            Self::Logs => "运行日志",
            Self::Dns => "域名解析",
            Self::Doctor => "自愈诊断",
            Self::AppRouting => "应用分流",
            Self::Sync => "数据同步",
            Self::Settings => "系统设置",
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

/// Installs routing and the first page. Reads the shell ([`ContentSlot`],
/// [`UiPalette`]) — add it after [`ShellPlugin`](crate::app::ShellPlugin).
/// The default source is the demo fixture; inject any [`OverviewSource`]
/// with [`PagesPlugin::new`].
pub struct PagesPlugin {
    source: Arc<dyn OverviewSource>,
}

impl PagesPlugin {
    /// Inject a projection source (tests use this for stub sources and
    /// non-default fixture states; the live core pump plugs in here in
    /// the next slice).
    pub fn new(source: impl OverviewSource + 'static) -> Self {
        Self {
            source: Arc::new(source),
        }
    }
}

impl Default for PagesPlugin {
    fn default() -> Self {
        Self {
            source: Arc::new(crate::projection::DemoOverviewSource::running()),
        }
    }
}

impl Plugin for PagesPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ActiveRoute>();
        app.init_resource::<RouteHistory>();
        app.init_resource::<LastOverviewProjection>();
        // The trend chart's sample ring: written by the live pump's drain
        // (when one is mounted), read by the page's refresh observer and
        // mount scene. The demo fixture ignores it (its trend is the
        // synthetic series — see `history`).
        app.init_resource::<TrafficHistory>();
        app.insert_resource(OverviewSourceHandle(Arc::clone(&self.source)));
        app.add_observer(on_content_slot_added);
        app.add_observer(sync_route);
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
        app.add_systems(Update, (sync_sidebar_foot, sync_overview_responsive));
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

/// The sidebar foot caption for a source kind: the demo fixture keeps the
/// milestone caption; a live core names the version it actually reports
/// (an unread version stays honest as 版本读取中). Pure function.
pub(crate) fn foot_caption(kind: SourceKind, projection: &OverviewProjection) -> String {
    match kind {
        SourceKind::Demo => "0.30 demo".to_owned(),
        SourceKind::LiveCore => banner_note(projection),
    }
}

/// The sidebar foot follows the injected source: `0.30 demo` for the demo
/// fixture, `实时内核 · <version>` for the live pump. Compare-and-set per
/// frame — the live version lands with the pump's first successful sample
/// and the caption picks it up with no event of its own.
fn sync_sidebar_foot(
    handle: Res<OverviewSourceHandle>,
    mut foot: Query<&mut Text, With<SidebarFoot>>,
) {
    let want = foot_caption(handle.0.kind(), &handle.0.current());
    for mut text in &mut foot {
        if text.0 != want {
            text.0 = want.clone();
        }
    }
}

/// Mount the default route the moment the shell's content slot lands —
/// the routing bootstrap with zero schedule-ordering assumptions.
fn on_content_slot_added(_ready: On<Add, ContentSlot>, mut commands: Commands) {
    let initial = crate::capture::page_from_env().unwrap_or_default();
    commands.trigger(RouteChanged(initial));
}

/// The route → scene table. The only place that knows which page backs
/// which route.
fn page_scene(
    route: Route,
    projection: &OverviewProjection,
    history: &TrafficHistory,
    palette: &UiPalette,
) -> Box<dyn Scene> {
    match route {
        Route::Overview => Box::new(overview_page(projection, history, palette)),
        Route::Proxies => Box::new(proxies_page(&ProxiesProjection::demo(), palette)),
        Route::Profiles => Box::new(profiles_page(&ProfilesProjection::demo(), palette)),
        Route::Rules => Box::new(rules_page(&RulesProjection::demo(), palette)),
        Route::Connections => Box::new(connections_page(&ConnectionsProjection::demo(), palette)),
        Route::Logs => Box::new(logs_page(&LogsProjection::demo(), palette)),
        Route::Dns => Box::new(dns_page(&DnsProjection::demo(), palette)),
        Route::Doctor => Box::new(doctor_page(&DoctorProjection::demo(), palette)),
        Route::AppRouting => Box::new(app_routing_page(&AppRoutingProjection::demo(), palette)),
        Route::Sync => Box::new(sync_page(&SyncProjection::demo(), palette)),
        Route::Settings => Box::new(settings_page(&SettingsProjection::demo(), palette)),
    }
}

/// The router: bounded replacement of the page subtree below the
/// [`ContentSlot`]. Same-route re-triggers after a settled mount are
/// no-ops; any accepted trigger queues `despawn_children` on the slot
/// *before* the new `spawn_scene`, so re-entrancy converges on one page.
#[allow(clippy::too_many_arguments)]
fn sync_route(
    trigger: On<RouteChanged>,
    slots: Query<Entity, With<ContentSlot>>,
    active: Option<Res<ActiveRoute>>,
    mut history: Option<ResMut<RouteHistory>>,
    palette: Res<UiPalette>,
    source: Res<OverviewSourceHandle>,
    history_ring: Res<TrafficHistory>,
    mut commands: Commands,
) {
    let route = trigger.0;
    if active.is_some_and(|mounted| mounted.0 == Some(route)) {
        return;
    }
    let Ok(slot) = slots.single() else {
        return;
    };
    if let Some(ref mut h) = history {
        h.push(route);
    }
    let projection = source.0.current();
    let scene = page_scene(route, &projection, &history_ring, &palette);
    commands.entity(slot).despawn_children();
    commands.spawn_scene(scene).insert(ChildOf(slot));
    // First paint: queued after the spawn command, so the page's child
    // lines exist when the freshly bound observer dispatches (the bind
    // hook itself fires at the root insert — before the children do).
    match route {
        Route::Overview => {
            commands.trigger(OverviewProjectionUpdated(projection));
        }
        Route::Proxies => {
            commands.trigger(ProxiesProjectionUpdated(ProxiesProjection::demo()));
        }
        Route::Profiles => {
            commands.trigger(ProfilesProjectionUpdated(ProfilesProjection::demo()));
        }
        Route::Rules => {
            commands.trigger(RulesProjectionUpdated(RulesProjection::demo()));
        }
        Route::Connections => {
            commands.trigger(ConnectionsProjectionUpdated(ConnectionsProjection::demo()));
        }
        Route::Logs => {
            commands.trigger(LogsProjectionUpdated(LogsProjection::demo()));
        }
        Route::Dns => {
            commands.trigger(DnsProjectionUpdated(DnsProjection::demo()));
        }
        Route::Doctor => {
            commands.trigger(DoctorProjectionUpdated(DoctorProjection::demo()));
        }
        Route::AppRouting => {
            commands.trigger(AppRoutingProjectionUpdated(AppRoutingProjection::demo()));
        }
        Route::Sync => {
            commands.trigger(SyncProjectionUpdated(SyncProjection::demo()));
        }
        Route::Settings => {
            commands.trigger(SettingsProjectionUpdated(SettingsProjection::demo()));
        }
    }
    commands.insert_resource(ActiveRoute(Some(route)));
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
