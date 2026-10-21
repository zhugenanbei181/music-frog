//! The Bevy surface's record of the owner's typed answer to the commands it
//! submitted.
//!
//! The Bevy command bus is fire-and-forget: a submitted command leaves the
//! surface and only the owner's terminal answer ([`CommandExecutedEvent`])
//! comes back. A page that owns an operation — a profile deletion, a
//! subscription refresh, a port-conflict repair — needs that answer to show
//! the outcome; without it the page can only pretend the action never
//! happened.
//!
//! The record is **not** a second truth about the owner's state: it holds the
//! owner's own typed answer, keyed by the business the page renders. The
//! owner's published facts (the profile list, the port-conflict snapshot)
//! stay the truth about the *state*; this record is the truth about *what
//! this surface's last action was answered with*.

use crate::command::UiCommand;
use crate::command_events::CommandExecutedEvent;
use bevy::app::{App, Plugin};
use bevy::ecs::observer::On;
use bevy::ecs::resource::Resource;
use bevy::ecs::system::ResMut;
use lurker_core_contract::error::Failure;
use std::collections::BTreeMap;

/// The business a submitted command belongs to.
///
/// The key is typed, so a page addresses its own operation and never another
/// page's.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum OutcomeKey {
    /// The refresh of one subscription profile.
    ProfileRefresh(String),
    /// The refresh of every subscription profile at once.
    ProfileRefreshAll,
    /// The deletion of one subscription profile.
    ProfileDelete(String),
    /// The import of one subscription profile.
    ProfileImport(String),
    /// The refresh of every rule provider at once.
    RuleProviderRefreshAll,
    /// The refresh of one rule provider.
    RuleProviderRefresh(String),
    /// The unpack of one rule provider.
    RuleProviderUnpack(String),
    /// The safe port-conflict repair.
    PortConflictRepair,
}

/// The typed outcome of one submitted command.
#[derive(Clone, Debug, PartialEq)]
pub enum CommandOutcome {
    /// The owner confirmed the command.
    Confirmed,
    /// The owner refused the command with a typed failure.
    Refused(Failure),
}

/// The owner's answers to the commands this surface submitted.
#[derive(Resource, Clone, Debug, Default)]
pub struct CommandOutcomes(pub BTreeMap<OutcomeKey, CommandOutcome>);

impl CommandOutcomes {
    /// The outcome of one business, when the owner answered it.
    pub fn outcome(&self, key: &OutcomeKey) -> Option<&CommandOutcome> {
        self.0.get(key)
    }

    /// The failure the owner answered one business with, when it refused.
    pub fn failure(&self, key: &OutcomeKey) -> Option<&Failure> {
        match self.0.get(key) {
            Some(CommandOutcome::Refused(failure)) => Some(failure),
            Some(CommandOutcome::Confirmed) | None => None,
        }
    }

    /// Whether the owner confirmed one business.
    pub fn confirmed(&self, key: &OutcomeKey) -> bool {
        matches!(self.0.get(key), Some(CommandOutcome::Confirmed))
    }
}

/// The business one submitted command belongs to.
///
/// A command this surface does not render an outcome for has no key: the
/// owner's answer to it is not this surface's business.
pub fn outcome_key(command: &UiCommand) -> Option<OutcomeKey> {
    match command {
        UiCommand::UpdateProfile { id } => Some(OutcomeKey::ProfileRefresh(id.clone())),
        UiCommand::UpdateAllSubscriptions => Some(OutcomeKey::ProfileRefreshAll),
        UiCommand::DeleteProfile { id } => Some(OutcomeKey::ProfileDelete(id.clone())),
        UiCommand::ImportSubscription { profile_id, .. } => {
            Some(OutcomeKey::ProfileImport(profile_id.clone()))
        }
        UiCommand::RefreshRuleProviders => Some(OutcomeKey::RuleProviderRefreshAll),
        UiCommand::RefreshRuleProvider { reference } => Some(OutcomeKey::RuleProviderRefresh(
            reference.id.name().to_owned(),
        )),
        UiCommand::UnpackRuleProvider(name) => Some(OutcomeKey::RuleProviderUnpack(name.clone())),
        UiCommand::RepairPortConflicts => Some(OutcomeKey::PortConflictRepair),
        _ => None,
    }
}

/// Record the owner's answer to one submitted command.
pub fn record_outcome(event: On<CommandExecutedEvent>, mut outcomes: ResMut<CommandOutcomes>) {
    let Some(key) = outcome_key(&event.command) else {
        return;
    };
    let outcome = match &event.result {
        Ok(_) => CommandOutcome::Confirmed,
        Err(failure) => CommandOutcome::Refused(failure.clone()),
    };
    outcomes.0.insert(key, outcome);
}

/// Install the outcome record.
#[derive(Default)]
pub struct CommandOutcomePlugin;

impl Plugin for CommandOutcomePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CommandOutcomes>();
        app.add_observer(record_outcome);
    }
}
