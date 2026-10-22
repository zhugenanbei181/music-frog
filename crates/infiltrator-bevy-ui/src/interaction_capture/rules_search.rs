//! Type the keyword into the rules list's own search field.
//!
//! The rules list owns a live keyword field: the shared filter reduction
//! decides which projection rows survive it. The capture types into that very
//! field, so the mounted list really narrows and the pixels really change.
use super::geometry::CaptureGeometry;
use super::scroll::request_scroll;
use super::{CaptureObservationSet, InteractionCapture, ObservedInteraction, selected};
use crate::pages::rules::RulesPageRoot;
use crate::pages::rules_view::{RuleRow, RuleSearchField, RulesViewState};
use crate::route::{ActiveRoute, Route};
use bevy::app::{App, PostUpdate, Update};
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::query::With;
use bevy::ecs::resource::Resource;
use bevy::ecs::schedule::IntoScheduleConfigs;
use bevy::ecs::system::{Commands, Query, Res, ResMut, SystemParam};
use infiltrator_bevy_widgets::text_input::TextField;
use infiltrator_bevy_widgets::text_input::state::TextFieldInput;
use infiltrator_contract::parity::FeatureId;

/// The keyword the capture types into the rules list's own search field.
const KEYWORD: &str = "github";

/// The projection index of the one rule the keyword selects.
const MATCHED_RULE: usize = 1;

/// The capture's own progress: the keyword is typed exactly once.
#[derive(Resource, Default)]
pub struct RulesSearchCapture {
    typed: bool,
    field: Option<Entity>,
}

pub fn install(app: &mut App) {
    app.init_resource::<RulesSearchCapture>();
    app.add_systems(Update, activate);
    app.add_systems(PostUpdate, observe.in_set(CaptureObservationSet));
}

#[derive(SystemParam)]
pub struct SearchControls<'w, 's> {
    route: Res<'w, ActiveRoute>,
    capture: ResMut<'w, RulesSearchCapture>,
    view: Res<'w, RulesViewState>,
    fields: Query<'w, 's, (Entity, &'static Children), With<RuleSearchField>>,
    texts: Query<'w, 's, &'static mut TextField>,
    rows: Query<'w, 's, (Entity, &'static RuleRow)>,
    pages: Query<'w, 's, Entity, With<RulesPageRoot>>,
}

/// Type the keyword into the mounted search field.
pub fn activate(mut controls: SearchControls, feature: Res<'_, InteractionCapture>) {
    if !selected(
        &feature,
        &controls.route,
        FeatureId::RulesSearchHighlight,
        Route::Rules,
    ) || controls.capture.typed
    {
        return;
    }
    let Some((_, children)) = controls.fields.iter().next() else {
        return;
    };
    let Some(entity) = children
        .iter()
        .find(|entity| controls.texts.contains(**entity))
    else {
        return;
    };
    let Ok(mut field) = controls.texts.get_mut(*entity) else {
        return;
    };
    field.0.apply(TextFieldInput::SetText(KEYWORD.into()));
    controls.capture.typed = true;
    controls.capture.field = Some(*entity);
}

/// Observe the shared filter reduction's result and the row it kept.
pub fn observe(
    mut controls: SearchControls,
    mut geometry: CaptureGeometry,
    mut feature: ResMut<InteractionCapture>,
    mut observation: ResMut<ObservedInteraction>,
    mut commands: Commands,
) {
    if !controls.capture.typed || controls.view.filtered_indices.as_slice() != &[MATCHED_RULE][..] {
        observation.publish(&mut feature, None);
        return;
    }
    let Some(field) = controls.capture.field else {
        observation.publish(&mut feature, None);
        return;
    };
    let Some((row, _)) = controls.rows.iter().find(|(_, row)| row.0 == MATCHED_RULE) else {
        observation.publish(&mut feature, None);
        return;
    };
    // The compact viewport starts the keyword field below the fold, so the
    // capture scrolls the page's own viewport until the field and the row it
    // kept are really on screen. The row is the deeper of the two, so
    // revealing it reveals the field as well.
    if !geometry.unclipped(field) || !geometry.unclipped(row) {
        reveal(&controls, &mut geometry, &mut commands, field, row);
        observation.publish(&mut feature, None);
        return;
    }
    let bounds = (|| {
        geometry.bounds(field, "rules keyword field")?;
        geometry.bounds(row, "rules matched row")
    })();
    observation.publish(&mut feature, bounds);
}

/// Scroll the rules page until both controls are really rendered.
fn reveal(
    controls: &SearchControls<'_, '_>,
    geometry: &mut CaptureGeometry<'_, '_>,
    commands: &mut Commands,
    field: Entity,
    row: Entity,
) {
    let Ok(page) = controls.pages.single() else {
        return;
    };
    let Some(viewport) = geometry.rect(page) else {
        return;
    };
    // One scroll per frame: the row is the deeper control, so it decides
    // where the page stops; the field follows it into the viewport.
    let target = if geometry.unclipped(row) { field } else { row };
    let Some(bounds) = geometry.rect(target) else {
        return;
    };
    request_scroll(commands, page, bounds, viewport);
}
