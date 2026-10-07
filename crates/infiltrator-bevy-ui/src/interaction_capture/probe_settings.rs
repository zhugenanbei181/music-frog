//! Native field edits and modal geometry use explicit ECS access sets.
use super::geometry::CaptureGeometry;
use super::{InteractionCapture, ObservedInteraction, selected};
use crate::pages::proxy_probe_settings::{
    ApplyProbeSettings, OpenProbeSettings, ProbeSettingsCard, ProbeSettingsState,
    ProbeTimeoutField, ProbeUrlField,
};
use crate::route::{ActiveRoute, Route};
use crate::surface::LatestSurfaceSnapshot;
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::query::With;
use bevy::ecs::system::{Commands, Query, Res, ResMut, SystemParam};
use bevy::ui_widgets::Activate;
use infiltrator_bevy_widgets::button::ButtonDisabled;
use infiltrator_bevy_widgets::text_input::TextField;
use infiltrator_bevy_widgets::text_input::state::TextFieldInput;
use infiltrator_contract::parity::FeatureId;
#[derive(SystemParam)]
pub struct ProbeFields<'w, 's> {
    editor: Res<'w, ProbeSettingsState>,
    latest: ResMut<'w, LatestSurfaceSnapshot>,
    route: Res<'w, ActiveRoute>,
    open: Query<'w, 's, Entity, With<OpenProbeSettings>>,
    urls: Query<'w, 's, &'static Children, With<ProbeUrlField>>,
    timeouts: Query<'w, 's, &'static Children, With<ProbeTimeoutField>>,
    inputs: Query<'w, 's, &'static mut TextField>,
}
pub fn activate(mut fields: ProbeFields, feature: Res<InteractionCapture>, mut commands: Commands) {
    if !selected(
        &feature,
        &fields.route,
        FeatureId::ProxiesProbeSettings,
        Route::Proxies,
    ) {
        return;
    }
    if !fields.editor.open {
        if let Some(entity) = fields.open.iter().next() {
            fields.latest.0.probe_settings.can_persist = true;
            commands.trigger(Activate { entity });
        }
        return;
    }
    if fields.editor.editor.draft.timeout_ms == "32767" {
        return;
    }
    for (roots, value) in [
        (fields.urls.iter().next(), "https://probe.example.test/204"),
        (fields.timeouts.iter().next(), "32767"),
    ] {
        if let Some(children) = roots
            && let Some(entity) = children
                .iter()
                .find(|entity| fields.inputs.contains(**entity))
            && let Ok(mut input) = fields.inputs.get_mut(*entity)
            && !input.0.is_disabled()
        {
            input.0.apply(TextFieldInput::SetText(value.into()));
        }
    }
}
#[derive(SystemParam)]
pub struct ProbeModal<'w, 's> {
    editor: Res<'w, ProbeSettingsState>,
    cards: Query<'w, 's, Entity, With<ProbeSettingsCard>>,
    urls: Query<'w, 's, Entity, With<ProbeUrlField>>,
    timeouts: Query<'w, 's, Entity, With<ProbeTimeoutField>>,
    apply: Query<'w, 's, (Entity, &'static ButtonDisabled), With<ApplyProbeSettings>>,
}
impl ProbeModal<'_, '_> {
    fn bounds(&self, geometry: &mut CaptureGeometry) -> Option<[f32; 4]> {
        let state = &self.editor;
        if !state.open
            || !state.editor.can_apply()
            || state.editor.draft.test_url != "https://probe.example.test/204"
            || state.editor.draft.timeout_ms != "32767"
        {
            return None;
        }
        let (apply, disabled) = self.apply.iter().next()?;
        if disabled.0 {
            return None;
        }
        let card = geometry.bounds(self.cards.iter().next()?, "probe parameters")?;
        for (entity, label) in [
            (self.urls.iter().next()?, "probe URL"),
            (self.timeouts.iter().next()?, "probe timeout"),
            (apply, "apply parameters"),
        ] {
            let field = geometry.bounds(entity, label)?;
            if field[0] < card[0]
                || field[1] < card[1]
                || field[0] + field[2] > card[0] + card[2]
                || field[1] + field[3] > card[1] + card[3]
            {
                return None;
            }
        }
        Some(card)
    }
}
pub fn observe(
    modal: ProbeModal,
    mut geometry: CaptureGeometry,
    mut state: ResMut<InteractionCapture>,
    mut observation: ResMut<ObservedInteraction>,
) {
    observation.publish(&mut state, modal.bounds(&mut geometry));
}
