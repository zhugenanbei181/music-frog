//! Capture opens a reachable native launcher and verifies both confirmation choices; no cache is cleared.
use super::geometry::CaptureGeometry;
use super::{InteractionCapture, ObservedInteraction, selected};
use crate::pages::dns::ClearDnsCacheButton;
use crate::pages::dns_cache::{CacheAction, CacheConfirmation, CacheLine, CacheModalCard};
use crate::route::{ActiveRoute, Route};
use bevy::ecs::entity::Entity;
use bevy::ecs::query::With;
use bevy::ecs::system::{Commands, Query, Res, ResMut};
use bevy::ui_widgets::Activate;
use bevy::window::{PrimaryWindow, Window};
use infiltrator_bevy_widgets::button::ButtonDisabled;
use infiltrator_bevy_widgets::localization::LocalizedLabel;
use infiltrator_contract::parity::FeatureId;
pub fn activate(
    feature: Res<InteractionCapture>,
    route: Res<ActiveRoute>,
    state: Res<CacheConfirmation>,
    launchers: Query<Entity, With<ClearDnsCacheButton>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut geometry: CaptureGeometry,
    mut commands: Commands,
) {
    if !selected(
        &feature,
        &route,
        FeatureId::DnsFakeIpFlushConfirm,
        Route::Dns,
    ) || state.model.open
    {
        return;
    }
    let Some(entity) = launchers.iter().next() else {
        return;
    };
    let Some([x, y, width, height]) = geometry.bounds(entity, "DNS cache launcher") else {
        return;
    };
    let Ok(window) = windows.single() else {
        return;
    };
    if x < 0.0 || y < 0.0 || x + width > window.width() || y + height > window.height() {
        return;
    }
    commands.trigger(Activate { entity });
}
pub fn observe(
    state: Res<CacheConfirmation>,
    cards: Query<Entity, With<CacheModalCard>>,
    choices: Query<(
        Entity,
        &CacheAction,
        &ButtonDisabled,
        Option<&LocalizedLabel>,
    )>,
    lines: Query<Entity, With<CacheLine>>,
    mut geometry: CaptureGeometry,
    mut feature: ResMut<InteractionCapture>,
    mut observation: ResMut<ObservedInteraction>,
) {
    let bounds = (|| {
        if !state.model.open
            || state.model.confirmed
            || state.model.pending.is_some()
            || state.request.is_some()
        {
            return None;
        }
        let confirm = choices
            .iter()
            .find(|(_, action, disabled, _)| matches!(action, CacheAction::Confirm) && !disabled.0)?
            .0;
        let cancel = choices
            .iter()
            .find(|(_, action, disabled, label)| {
                matches!(action, CacheAction::Cancel) && !disabled.0 && label.is_some()
            })?
            .0;
        for line in &lines {
            geometry.bounds(line, "DNS cache confirmation copy")?;
        }
        geometry.bounds(confirm, "DNS cache confirm action")?;
        geometry.bounds(cancel, "DNS cache cancel action")?;
        geometry.bounds(cards.iter().next()?, "DNS cache confirmation card")
    })();
    observation.publish(&mut feature, bounds);
}
