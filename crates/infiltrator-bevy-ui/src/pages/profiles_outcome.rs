//! The Profiles page's outcome lines.
//!
//! Three facts the page could not show before: the answer to a profile
//! deletion, the answer to a subscription import and the answer to one
//! profile's refresh. They are the owner's own typed answers to the commands
//! this page submitted, so the page renders them instead of pretending the
//! action never happened.

use crate::pages::command_outcome::{CommandOutcome, CommandOutcomes, OutcomeKey};
use crate::pages::profiles::{LastProfilesProjection, ProfilesProjection};
use crate::pages::rules_outcome::RuleProviderFailureText;
use bevy::ecs::component::Component;
use bevy::ecs::query::{With, Without};
use bevy::ecs::resource::Resource;
use bevy::ecs::system::{Query, Res, ResMut};
use bevy::ui::widget::Text;

/// One profile card's refresh outcome line.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ProfileRefreshText(pub usize);

/// The page-level deletion outcome line.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ProfileDeleteOutcomeText;

/// The page-level import outcome line.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ProfileImportOutcomeText;

/// The deletion this page submitted last: the profile id and its name.
///
/// The record is the *request* the page made, not a second truth about the
/// profile: the owner's answer to it lives in the outcome record.
#[derive(Resource, Clone, Debug, Default, PartialEq, Eq)]
pub struct SubmittedProfileDelete(pub Option<(String, String)>);

/// The copy of one command outcome.
fn outcome_copy(outcome: Option<&CommandOutcome>, confirmed: &str) -> String {
    match outcome {
        Some(CommandOutcome::Confirmed) => confirmed.to_owned(),
        Some(CommandOutcome::Refused(failure)) => failure.message.clone(),
        None => String::new(),
    }
}

/// Fold the owner's answer to every profile's refresh into the profile item.
///
/// The owner publishes no per-profile refresh failure yet, so the surface
/// shows the owner's own typed answer to the refresh it submitted. When the
/// contract publishes the per-profile failure, this fold disappears.
pub(crate) fn fold_refresh_failures(
    projection: &mut ProfilesProjection,
    outcomes: &CommandOutcomes,
) {
    for profile in &mut projection.profiles {
        profile.failure = outcomes
            .failure(&OutcomeKey::ProfileRefresh(profile.id.clone()))
            .cloned();
    }
}

/// Restamp the profiles page's outcome lines from the owner's answers.
pub fn sync_profiles_outcomes(
    outcomes: Option<Res<CommandOutcomes>>,
    mut projection: Option<ResMut<LastProfilesProjection>>,
    submitted: Option<Res<SubmittedProfileDelete>>,
    // The three outcome lines and the rule provider's failure line are
    // written by different systems in this schedule, so every query is
    // disjoint from the others.
    mut refreshes: Query<
        (&ProfileRefreshText, &mut Text),
        (
            With<ProfileRefreshText>,
            Without<ProfileDeleteOutcomeText>,
            Without<ProfileImportOutcomeText>,
            Without<RuleProviderFailureText>,
        ),
    >,
    mut deletions: Query<
        &mut Text,
        (
            With<ProfileDeleteOutcomeText>,
            Without<ProfileRefreshText>,
            Without<ProfileImportOutcomeText>,
            Without<RuleProviderFailureText>,
        ),
    >,
    mut imports: Query<
        &mut Text,
        (
            With<ProfileImportOutcomeText>,
            Without<ProfileRefreshText>,
            Without<ProfileDeleteOutcomeText>,
            Without<RuleProviderFailureText>,
        ),
    >,
) {
    let Some(outcomes) = outcomes else {
        return;
    };
    if let Some(projection) = projection.as_mut()
        && let Some(projection) = projection.0.as_mut()
    {
        fold_refresh_failures(projection, &outcomes);
    }

    let profiles = projection
        .as_deref()
        .and_then(|projection| projection.0.clone());
    for (marker, mut text) in &mut refreshes {
        let value = profiles
            .as_ref()
            .and_then(|profiles| profiles.profiles.get(marker.0))
            .and_then(|profile| profile.failure.as_ref())
            .map(|failure| failure.message.clone())
            .unwrap_or_default();
        if text.0 != value {
            text.0 = value.clone();
        }
    }

    let deletion = deletion_copy(outcomes.as_ref(), submitted.as_deref());
    for mut text in &mut deletions {
        if text.0 != deletion {
            text.0 = deletion.clone();
        }
    }

    let import = import_copy(outcomes.as_ref());
    for mut text in &mut imports {
        if text.0 != import {
            text.0 = import.clone();
        }
    }
}

/// The deletion outcome line: the owner's answer to the delete this page
/// submitted last.
fn deletion_copy(outcomes: &CommandOutcomes, submitted: Option<&SubmittedProfileDelete>) -> String {
    let Some((id, name)) = submitted.and_then(|submitted| submitted.0.clone()) else {
        return String::new();
    };
    let key = OutcomeKey::ProfileDelete(id);
    let value = outcome_copy(outcomes.outcome(&key), "deleted");
    if value.is_empty() {
        return String::new();
    }
    format!("{name}: {value}")
}

/// The import outcome line: the owner's answer to the last import this page
/// submitted.
fn import_copy(outcomes: &CommandOutcomes) -> String {
    let mut answer: Option<&CommandOutcome> = None;
    for (key, outcome) in &outcomes.0 {
        if matches!(key, OutcomeKey::ProfileImport(_)) {
            answer = Some(outcome);
        }
    }
    outcome_copy(answer, "imported")
}
