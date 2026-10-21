//! The Rules page's per-provider refresh outcome (DUAL-11-05).
//!
//! One rule provider can fail to refresh while the others succeed, so the
//! failure belongs to the provider row, not to the page. This module owns
//! the per-provider refresh control, the per-provider failure line and the
//! typed intent they publish.

use crate::command::{CommandSinkHandle, UiCommand};
use crate::pages::command_outcome::{CommandOutcomes, OutcomeKey};
use crate::pages::profiles_outcome::{ProfileDeleteOutcomeText, ProfileImportOutcomeText};
use crate::pages::rules::{LastRulesProjection, RulesProjection};
use bevy::ecs::component::Component;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::observer::On;
use bevy::ecs::query::{QueryFilter, With, Without};
use bevy::ecs::system::{Query, Res};
use bevy::feathers::theme::ThemeBackgroundColor;
use bevy::scene::{Scene, bsn};
use bevy::ui::prelude::{AlignItems, BorderRadius, JustifyContent, Node, UiRect};
use bevy::ui::widget::Text;
use bevy::ui_widgets::{Activate, Button};
use infiltrator_bevy_widgets::button::ButtonDisabled;
use infiltrator_bevy_widgets::feathers_theme::Token;
use infiltrator_bevy_widgets::palette::{UiPalette, rem};
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_bevy_widgets::theme::space;

/// One rule provider row's refresh failure line.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RuleProviderFailureText(pub usize);

/// The per-provider refresh control; the index addresses the provider row.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RuleProviderRefreshButton(pub usize);

/// The per-provider refresh control.
///
/// A provider the owner published no controller reference for cannot be
/// refreshed from this surface, so its control is disabled.
pub fn rule_provider_refresh_scene(
    idx: usize,
    enabled: bool,
    palette: &UiPalette,
) -> Box<dyn Scene> {
    Box::new(bsn! {
            Node {
                min_height: rem(palette.control_height_px),
                padding: UiRect::horizontal(rem(space::S12)),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border_radius: BorderRadius::all(rem(palette.control_radius_px)),
            }
            ThemeBackgroundColor({ Token::SurfaceElevated })
            RuleProviderRefreshButton(idx)
            ButtonDisabled({ !enabled })
            Button
            Children [
                Text({ "↻".to_owned() }) TextRole(Role::Body)
            ]
    })
}

/// Publish the shared per-provider refresh intent for one provider row.
pub(crate) fn on_rule_provider_refresh_activated(
    activate: On<Activate>,
    buttons: Query<&RuleProviderRefreshButton>,
    last: Option<Res<LastRulesProjection>>,
    handle: Option<Res<CommandSinkHandle>>,
) {
    let Some(handle) = handle else {
        return;
    };
    let Ok(button) = buttons.get(activate.entity) else {
        return;
    };
    let Some(reference) = last
        .as_ref()
        .and_then(|last| last.0.as_ref())
        .and_then(|projection| projection.providers.get(button.0))
        .and_then(|provider| provider.controller.clone())
    else {
        return;
    };
    handle.submit(UiCommand::RefreshRuleProvider { reference });
}

/// Fold the owner's answer to every provider's refresh into the provider item.
///
/// The owner publishes no per-provider refresh failure yet, so the surface
/// shows the owner's own typed answer to the refresh it submitted. When the
/// contract publishes the per-provider failure, this fold disappears.
pub(crate) fn fold_provider_failures(projection: &mut RulesProjection, outcomes: &CommandOutcomes) {
    for provider in &mut projection.providers {
        provider.failure = outcomes
            .failure(&OutcomeKey::RuleProviderRefresh(provider.name.clone()))
            .cloned();
    }
}

/// The provider failure line belongs to this system alone; the profiles'
/// outcome lines are written by another system in the same schedule.
#[derive(QueryFilter)]
pub struct RuleProviderFailureFilter {
    provider: With<RuleProviderFailureText>,
    without_profile_delete: Without<ProfileDeleteOutcomeText>,
    without_profile_import: Without<ProfileImportOutcomeText>,
}

/// Restamp the per-provider failure lines from the owner's answer.
///
/// The projection is the page's own read model, and the page that owns it
/// writes it: this system only reads it. Writing it here would mark the
/// projection changed on every frame, and a projection that claims to change
/// every frame rebuilds the rule list on every frame — the mounted row
/// controls would lose their identity while the user is clicking them.
///
/// The failure itself is the owner's answer to the refresh this page
/// submitted, so the line carries that answer directly.
pub fn sync_rule_provider_outcomes(
    outcomes: Option<Res<CommandOutcomes>>,
    projection: Option<Res<LastRulesProjection>>,
    // The profiles' outcome lines are written by another system in this
    // schedule, so the two queries are disjoint.
    mut failures: Query<(&RuleProviderFailureText, &mut Text), RuleProviderFailureFilter>,
) {
    let Some(outcomes) = outcomes else {
        return;
    };
    let Some(projection) = projection
        .as_ref()
        .and_then(|projection| projection.0.as_ref())
    else {
        return;
    };
    for (marker, mut text) in &mut failures {
        let value = projection
            .providers
            .get(marker.0)
            .and_then(|provider| {
                outcomes.failure(&OutcomeKey::RuleProviderRefresh(provider.name.clone()))
            })
            .map(|failure| failure.message.clone())
            .unwrap_or_default();
        if text.0 != value {
            text.0 = value;
        }
    }
}
