//! Rule enabled/reorder controls for the Bevy Rules page (DUAL-11-09/10).
//!
//! The mutation itself is the shared `infiltrator_domain::rules::edit`
//! reduction; this module only owns the row controls and forwards a typed
//! identity to the draft owner. Only explicit Save persists the change.

use crate::pages::rules_draft::RulesDraftState;
use crate::pages::rules_statistics::RulesStatisticsState;
use bevy::ecs::component::Component;
use bevy::ecs::observer::On;
use bevy::ecs::system::{Query, Res, ResMut};
use bevy::ui_widgets::Activate;
use infiltrator_contract::rule_document::RuleRowId;
use infiltrator_contract::rule_edit::RuleMoveDirection;

/// Marker on a rule row's enable/disable switch; payload is its stable draft identity.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RuleToggleButton(pub Option<RuleRowId>);

/// Marker on a rule row's "move up" control.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RuleMoveUpButton(pub Option<RuleRowId>);

/// Marker on a rule row's "move down" control.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RuleMoveDownButton(pub Option<RuleRowId>);

/// DUAL-11-09/10: submit the shared toggle/reorder intent for the clicked row.
pub(crate) fn on_rules_row_edit_activated(
    activate: On<Activate>,
    toggles: Query<&RuleToggleButton>,
    move_up: Query<&RuleMoveUpButton>,
    move_down: Query<&RuleMoveDownButton>,
    mut state: ResMut<RulesDraftState>,
    statistics: Option<Res<RulesStatisticsState>>,
) {
    if statistics.is_some_and(|state| state.model.confirmation.is_some()) || state.input_composing {
        return;
    }
    if let Ok(button) = toggles.get(activate.entity) {
        if let Some(id) = button.0 {
            state.repaint |= state.model.toggle(id);
        }
        return;
    }
    if let Ok(button) = move_up.get(activate.entity) {
        if let Some(id) = button.0 {
            state.repaint |= state.model.move_rule(id, RuleMoveDirection::Up);
        }
        return;
    }
    if let Ok(button) = move_down.get(activate.entity)
        && let Some(id) = button.0
    {
        state.repaint |= state.model.move_rule(id, RuleMoveDirection::Down);
    }
}
