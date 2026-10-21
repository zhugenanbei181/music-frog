//! The Settings page's port-conflict repair outcome (DUAL-14-13).
//!
//! The port-conflict row renders the shared conflict observation; the repair
//! *outcome* is a different fact — it is the owner's answer to the repair
//! this page asked for. Without it a successful repair and a route-only
//! baseline look identical, so the page could never show that the business
//! ran at all.

use crate::pages::command_outcome::{CommandOutcome, CommandOutcomes, OutcomeKey};
use bevy::ecs::component::Component;
use bevy::ecs::query::With;
use bevy::ecs::system::{Query, Res};
use bevy::ui::widget::Text;

/// The port-conflict repair outcome line.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PortConflictOutcomeText;

/// The copy of one repair outcome.
fn outcome_copy(outcome: Option<&CommandOutcome>) -> String {
    match outcome {
        Some(CommandOutcome::Confirmed) => "port conflicts repaired".to_owned(),
        Some(CommandOutcome::Refused(failure)) => failure.message.clone(),
        None => String::new(),
    }
}

/// Restamp the repair outcome line from the owner's answer.
pub fn sync_port_conflict_outcome(
    outcomes: Option<Res<CommandOutcomes>>,
    mut texts: Query<&mut Text, With<PortConflictOutcomeText>>,
) {
    let key = OutcomeKey::PortConflictRepair;
    let value = outcome_copy(
        outcomes
            .as_deref()
            .and_then(|outcomes| outcomes.outcome(&key)),
    );
    for mut text in &mut texts {
        if text.0 != value {
            text.0 = value.clone();
        }
    }
}
