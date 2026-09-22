//! Rule enabled/reorder controls for the Bevy Rules page (DUAL-11-09/10).
//!
//! The mutation itself is the shared `infiltrator_domain::rules::edit`
//! reduction; this module only owns the row controls and forwards a typed
//! intent through the command bus so the application persists the change.

use bevy::ecs::component::Component;
use bevy::ecs::observer::On;
use bevy::ecs::system::{Query, Res};
use bevy::ui_widgets::Activate;

use crate::command::{CommandSinkHandle, UiCommand};

/// Marker on a rule row's enable/disable switch; payload is the row index.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RuleToggleButton(pub usize);

/// Marker on a rule row's "move up" control.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RuleMoveUpButton(pub usize);

/// Marker on a rule row's "move down" control.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RuleMoveDownButton(pub usize);

/// DUAL-11-09/10: submit the shared toggle/reorder intent for the clicked row.
pub(crate) fn on_rules_row_edit_activated(
    activate: On<Activate>,
    toggles: Query<&RuleToggleButton>,
    move_up: Query<&RuleMoveUpButton>,
    move_down: Query<&RuleMoveDownButton>,
    handle: Option<Res<CommandSinkHandle>>,
) {
    let Some(handle) = handle else {
        return;
    };
    if let Ok(button) = toggles.get(activate.entity) {
        handle.submit(UiCommand::ToggleRuleEnabled(button.0));
        return;
    }
    if let Ok(button) = move_up.get(activate.entity) {
        handle.submit(UiCommand::MoveRuleUp(button.0));
        return;
    }
    if let Ok(button) = move_down.get(activate.entity) {
        handle.submit(UiCommand::MoveRuleDown(button.0));
    }
}
