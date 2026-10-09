//! BEVY-032: audio-haptic feedback wired into real page interactions.
//!
//! Each observer translates one real control activation into the typed
//! [`FeedbackTriggerEvent`] the host gate ([`crate::host_capabilities`])
//! consumes. The gate owns capability and mute decisions, so an unsupported or
//! muted host turns every one of these into a no-op without any page knowing.

use crate::pages::connections_confirm::CloseAllConfirmationAction;
use crate::pages::overview_cards::OverviewModeSegmentPill;
use crate::pages::proxies::ProxyNodeButton;
use bevy::app::{App, Plugin};
use bevy::ecs::observer::On;
use bevy::ecs::query::With;
use bevy::ecs::system::{Commands, Query};
use bevy::ui_widgets::Activate;
use infiltrator_bevy_widgets::haptics::{FeedbackTriggerEvent, HapticPattern};

/// Feedback for selecting a proxy node (also the pin / node-detail path).
pub const PROXY_SELECT_FEEDBACK: HapticPattern = HapticPattern::SelectionTick;
/// Feedback for switching the proxy mode segment.
pub const MODE_TOGGLE_FEEDBACK: HapticPattern = HapticPattern::LightTick;
/// Feedback for confirming a destructive action.
pub const DESTRUCTIVE_CONFIRM_FEEDBACK: HapticPattern = HapticPattern::WarningDoublePulse;

fn emit(commands: &mut Commands, pattern: HapticPattern) {
    commands.trigger(FeedbackTriggerEvent {
        pattern,
        play_sound: true,
    });
}

/// A proxy node selection fires the selection tick.
pub fn on_proxy_select_feedback(
    activate: On<Activate>,
    buttons: Query<(), With<ProxyNodeButton>>,
    mut commands: Commands,
) {
    if buttons.contains(activate.entity) {
        emit(&mut commands, PROXY_SELECT_FEEDBACK);
    }
}

/// A proxy-mode segment switch fires the light tick.
pub fn on_mode_toggle_feedback(
    activate: On<Activate>,
    buttons: Query<(), With<OverviewModeSegmentPill>>,
    mut commands: Commands,
) {
    if buttons.contains(activate.entity) {
        emit(&mut commands, MODE_TOGGLE_FEEDBACK);
    }
}

/// Confirming the close-all dialog fires the warning double pulse. The cancel
/// and scrim actions stay silent.
pub fn on_destructive_confirm_feedback(
    activate: On<Activate>,
    actions: Query<&CloseAllConfirmationAction>,
    mut commands: Commands,
) {
    if actions
        .get(activate.entity)
        .is_ok_and(|action| *action == CloseAllConfirmationAction::Confirm)
    {
        emit(&mut commands, DESTRUCTIVE_CONFIRM_FEEDBACK);
    }
}

/// Registers the interaction → feedback observers once at product assembly.
#[derive(Default)]
pub struct FeedbackPlugin;

impl Plugin for FeedbackPlugin {
    fn build(&self, app: &mut App) {
        app.add_observer(on_proxy_select_feedback);
        app.add_observer(on_mode_toggle_feedback);
        app.add_observer(on_destructive_confirm_feedback);
    }
}
