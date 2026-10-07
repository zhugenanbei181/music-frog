//! Idle-connection sweep state, controls, and activation for the Bevy
//! Connections page (DUAL-13-11).
//!
//! The idle reduction itself is the shared
//! [`infiltrator_domain::connection_activity::ConnectionActivityTracker`]; this
//! module owns only the Bevy-side resource, the timeout pills and sweep
//! button, and the honest last-sweep status line.

use crate::command::{CommandSinkHandle, UiCommand};
use crate::pages::connections::LastConnectionsProjection;
use crate::pages::connections_view::ConnectionsViewState;
use bevy::ecs::component::Component;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::observer::On;
use bevy::ecs::query::{QueryFilter, With, Without};
use bevy::ecs::resource::Resource;
use bevy::ecs::system::{Query, Res, ResMut, SystemParam};
use bevy::scene::{Scene, bsn};
use bevy::text::TextColor;
use bevy::ui::prelude::{
    AlignItems, BackgroundColor, BorderRadius, JustifyContent, Node, UiRect, Val, percent, px,
};
use bevy::ui::widget::Text;
use bevy::ui_widgets::{Activate, Button};
use infiltrator_bevy_widgets::localization::{LocalizedText, UiLocale};
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_bevy_widgets::theme::space;
use infiltrator_domain::connection_activity::{
    ConnectionActivityTracker, DEFAULT_IDLE_TIMEOUT_SECS, IDLE_TIMEOUT_CHOICES,
    idle_timeout_minutes_label,
};
use std::time::{SystemTime, UNIX_EPOCH};

/// DUAL-13-11: byte-change tracker, configured timeout, and honest last-sweep
/// result for the connections idle sweeper.
#[derive(Resource, Clone, Debug)]
pub struct ConnectionsIdleState {
    pub tracker: ConnectionActivityTracker,
    pub timeout_secs: u64,
    pub last_sweep_idle: Option<usize>,
}

impl Default for ConnectionsIdleState {
    fn default() -> Self {
        Self {
            tracker: ConnectionActivityTracker::new(),
            timeout_secs: DEFAULT_IDLE_TIMEOUT_SECS,
            last_sweep_idle: None,
        }
    }
}

/// Marker on an idle-timeout choice pill; payload is the timeout in seconds.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ConnIdleTimeoutPill(pub u64);

/// Marker on the manual idle-sweep button.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ConnIdleSweepButton;

/// Marker on the honest last-sweep status line.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ConnIdleStatus;

/// DUAL-13-11: shared idle-timeout choices + manual sweep + honest status.
pub fn conn_idle_controls_scene(palette: &UiPalette) -> impl Scene + use<> {
    let pills: Vec<Box<dyn Scene>> = IDLE_TIMEOUT_CHOICES
        .iter()
        .map(|secs| {
            Box::new(conn_idle_timeout_pill(
                *secs,
                *secs == DEFAULT_IDLE_TIMEOUT_SECS,
                palette,
            )) as Box<dyn Scene>
        })
        .collect();

    bsn! {
            Node {
                width: percent(100),
                align_items: AlignItems::Center,
                column_gap: Val::Px(space::S8),
            }
            Children [
                LocalizedText::plain("dns_fake_ip_idle_label") TextRole(Role::Caption)
                --
                Node {
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(space::S4),
                }
                Children [
                    { pills }
                ]
                --
                Node {
                    min_height: px(palette.control_height_px),
                    padding: UiRect::horizontal(Val::Px(space::S12)),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    border_radius: BorderRadius::all(Val::Px(palette.control_radius_px)),
                }
                BackgroundColor({ palette.surface_elevated })
                Button
                ConnIdleSweepButton
                Children [
                    LocalizedText::plain("conn_idle_sweep_btn") TextRole(Role::Caption)
                ]
                --
                LocalizedText::plain("conn_idle_last_sweep_none") ConnIdleStatus TextRole(Role::Caption)
            ]
    }
}

fn conn_idle_timeout_pill(secs: u64, active: bool, palette: &UiPalette) -> impl Scene + use<> {
    let (bg, text_color) = if active {
        (palette.accent_container, palette.accent)
    } else {
        (palette.surface, palette.ink_dim)
    };
    let label = idle_timeout_minutes_label(secs);

    bsn! {
            Node {
                padding: UiRect::axes(Val::Px(space::S8), Val::Px(space::S4)),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                align_items: AlignItems::Center,
            }
            BackgroundColor({ bg })
            ConnIdleTimeoutPill(secs)
            Button
            Children [
                Text(label) TextRole(Role::Caption) TextColor({ text_color })
            ]
    }
}

/// DUAL-13-11: switch the idle timeout or run a manual sweep. The sweep
/// restamps the honest last-sweep status and submits one close per idle id.
#[derive(SystemParam)]
pub(crate) struct IdleControls<'w, 's> {
    timeout_pills: Query<'w, 's, &'static ConnIdleTimeoutPill>,
    sweep_buttons: Query<'w, 's, (), With<ConnIdleSweepButton>>,
    last: Option<Res<'w, LastConnectionsProjection>>,
    idle: Option<ResMut<'w, ConnectionsIdleState>>,
    palette: Res<'w, UiPalette>,
    pill_fills: Query<
        'w,
        's,
        (&'static mut BackgroundColor, &'static ConnIdleTimeoutPill),
        Without<ConnIdleStatus>,
    >,
    status: Query<'w, 's, (&'static mut Text, &'static mut LocalizedText), With<ConnIdleStatus>>,
    handle: Option<Res<'w, CommandSinkHandle>>,
    locale: Res<'w, UiLocale>,
    view: Res<'w, ConnectionsViewState>,
}
pub(crate) fn on_connections_idle_activated(activate: On<Activate>, controls: IdleControls) {
    let IdleControls {
        timeout_pills,
        sweep_buttons,
        last,
        mut idle,
        palette,
        mut pill_fills,
        mut status,
        handle,
        locale,
        view,
    } = controls;
    if let Ok(pill) = timeout_pills.get(activate.entity) {
        if let Some(state) = idle.as_deref_mut() {
            state.timeout_secs = pill.0;
        }
        restamp_idle_pills(&palette, &mut pill_fills, pill.0);
        return;
    }
    if !sweep_buttons.contains(activate.entity) || !view.groups.source_current() {
        return;
    }
    let Some(state) = idle.as_deref_mut() else {
        return;
    };
    let now = current_unix_secs();
    if let Some(projection) = last.as_ref().and_then(|last| last.0.as_ref()) {
        state.tracker.observe(&projection.connections, now);
    }
    let idle_ids = state.tracker.idle_ids(now, state.timeout_secs);
    state.last_sweep_idle = Some(idle_ids.len());
    let copy = LocalizedText::new(
        "conn_idle_last_sweep",
        vec![("count", idle_ids.len().to_string())],
    );
    for (mut text, mut label) in &mut status {
        *label = copy.clone();
        text.0 = label.render(&locale);
    }
    if let Some(handle) = handle {
        for id in idle_ids {
            handle.submit(UiCommand::CloseConnection { id });
        }
    }
}

/// Restamp every idle-timeout pill fill for the active timeout.
pub(crate) fn restamp_idle_pills<F: QueryFilter>(
    palette: &UiPalette,
    pills: &mut Query<(&mut BackgroundColor, &ConnIdleTimeoutPill), F>,
    active: u64,
) {
    for (mut fill, pill) in pills.iter_mut() {
        fill.0 = if pill.0 == active {
            palette.accent_container
        } else {
            palette.surface
        };
    }
}

/// Wall-clock seconds for activity observations.
pub(crate) fn current_unix_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}
