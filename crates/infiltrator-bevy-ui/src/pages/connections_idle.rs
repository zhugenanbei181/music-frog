//! Idle-connection sweep state, controls, and activation for the Bevy
//! Connections page (DUAL-13-11).
//!
//! The idle reduction itself is the shared
//! [`infiltrator_domain::connection_activity::ConnectionActivityTracker`]; this
//! module owns only the Bevy-side resource, the timeout strip, the sweep
//! button and the honest last-sweep status line.

use crate::command::{CommandSinkHandle, UiCommand};
use crate::pages::connections::LastConnectionsProjection;
use crate::pages::connections_view::{ConnectionsViewState, idle_timeout_strip_scene};
use bevy::ecs::component::Component;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::observer::On;
use bevy::ecs::query::With;
use bevy::ecs::resource::Resource;
use bevy::ecs::system::{Query, Res, ResMut, SystemParam};
use bevy::scene::{Scene, bsn};
use bevy::ui::prelude::{AlignItems, FlexWrap, Node, percent};
use bevy::ui::widget::Text;
use bevy::ui_widgets::Activate;
use infiltrator_bevy_widgets::button::{ButtonSize, ButtonVariant, button_with_label_scene};
use infiltrator_bevy_widgets::localization::{LocalizedText, UiLocale};
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::palette::rem;
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_bevy_widgets::theme::space;
use infiltrator_domain::connection_activity::{
    ConnectionActivityTracker, DEFAULT_IDLE_TIMEOUT_SECS,
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

/// Marker on the manual idle-sweep button.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ConnIdleSweepButton;

/// Marker on the honest last-sweep status line.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ConnIdleStatus;

/// DUAL-13-11: shared idle-timeout choices + manual sweep + honest status.
///
/// The timeout choices are the OFFICIAL headless tab strip (the same control
/// family as the view filter and the row order), and the sweep button is a
/// quiet inline action, so the idle row reads as one family instead of three.
pub fn conn_idle_controls_scene(palette: &UiPalette) -> impl Scene + use<> {
    bsn! {
            Node {
                width: percent(100),
                align_items: AlignItems::Center,
                column_gap: rem(space::S8),
                flex_wrap: FlexWrap::Wrap,
                row_gap: rem(space::S8),
            }
            Children [
                LocalizedText::plain("dns_fake_ip_idle_label") TextRole(Role::Caption)
                --
                @{ idle_timeout_strip_scene(DEFAULT_IDLE_TIMEOUT_SECS, palette) }
                --
                @{ sweep_button_scene(palette) }
                --
                LocalizedText::plain("conn_idle_last_sweep_none") ConnIdleStatus TextRole(Role::Caption)
            ]
    }
}

/// The manual sweep: a quiet inline action beside the timeout strip.
fn sweep_button_scene(palette: &UiPalette) -> impl Scene + use<> {
    bsn! {
            @{ button_with_label_scene(
                bsn! { LocalizedText::plain("conn_idle_sweep_btn") TextRole(Role::Caption) },
                ButtonVariant::Ghost,
                ButtonSize::Sm,
                palette,
            ) }
            ConnIdleSweepButton
    }
}

/// DUAL-13-11: run a manual sweep. The sweep restamps the honest last-sweep
/// status and submits one close per idle id.
#[derive(SystemParam)]
pub(crate) struct IdleControls<'w, 's> {
    sweep_buttons: Query<'w, 's, (), With<ConnIdleSweepButton>>,
    last: Option<Res<'w, LastConnectionsProjection>>,
    idle: Option<ResMut<'w, ConnectionsIdleState>>,
    status: Query<'w, 's, (&'static mut Text, &'static mut LocalizedText), With<ConnIdleStatus>>,
    handle: Option<Res<'w, CommandSinkHandle>>,
    locale: Res<'w, UiLocale>,
    view: Res<'w, ConnectionsViewState>,
}
pub(crate) fn on_connections_idle_activated(activate: On<Activate>, controls: IdleControls) {
    let IdleControls {
        sweep_buttons,
        last,
        mut idle,
        mut status,
        handle,
        locale,
        view,
    } = controls;
    if !sweep_buttons.contains(activate.entity) || !view.page.source_current() {
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

/// Wall-clock seconds for activity observations.
pub(crate) fn current_unix_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}
