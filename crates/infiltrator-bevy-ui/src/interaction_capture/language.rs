//! Capture uses native choices, the real settings writer and its correlated terminal event.
use super::capability::capture_command_application;
use super::geometry::CaptureGeometry;
use super::host::{CaptureCapability, CaptureCommandSink};
use super::{InteractionCapture, ObservedInteraction, selected};
use crate::command::CommandSinkHandle;
use crate::pages::settings_language::{
    LanguageChoiceButton, LanguageChoiceCard, LanguageChoiceResource,
};
use crate::route::{ActiveRoute, Route};
use crate::surface::LatestSurfaceSnapshot;
use bevy::app::App;
use bevy::app::Update;
use bevy::ecs::prelude::*;
use bevy::ecs::schedule::IntoScheduleConfigs;
use bevy::ecs::system::SystemParam;
use bevy::ui_widgets::Activate;
use infiltrator_application::settings_application::SettingsApplication;
use infiltrator_bevy_widgets::button::ButtonDisabled;
use infiltrator_bevy_widgets::localization::{LocalizedLabel, UiLocale};
use infiltrator_contract::language::LanguagePreference;
use infiltrator_contract::parity::FeatureId;
use infiltrator_presentation::language_choice::{
    LanguageChoicePhase, LanguageReadPhase, project_language,
};
use infiltrator_test_support::language_choice_fixtures::LanguageCaptureStore;
use std::sync::Arc;

#[derive(Resource)]
struct CaptureLanguageStore {
    store: Arc<LanguageCaptureStore>,
    initialized: bool,
    published: Option<String>,
}
pub fn install(app: &mut App) {
    let store = Arc::new(LanguageCaptureStore::default());
    app.insert_resource(UiLocale::new("zh-CN"));
    app.insert_resource(CommandSinkHandle(Arc::new(CaptureCommandSink::new(
        capture_command_application().with_settings(SettingsApplication::new(store.clone())),
        CaptureCapability::Language,
    ))));
    app.insert_resource(CaptureLanguageStore {
        store,
        initialized: false,
        published: None,
    });
    app.add_systems(Update, initialize.before(activate));
    app.add_systems(Update, publish_saved.after(activate));
}
fn initialize(mut store: ResMut<CaptureLanguageStore>, mut latest: ResMut<LatestSurfaceSnapshot>) {
    if store.initialized {
        return;
    }
    let saved = store.store.saved();
    latest.0.language_settings = project_language(Some(&Ok(saved.clone())));
    store.published = Some(saved.language);
    store.initialized = true;
}

/// Publish the persisted language the shared settings owner really stored.
///
/// The language choice owner only leaves its `AwaitingObservation` phase when
/// the surface reads the persisted preference back. The capture therefore
/// republishes the stored settings once the real save landed; without it the
/// choice never settles and the capture can never observe the change.
fn publish_saved(
    mut store: ResMut<CaptureLanguageStore>,
    mut latest: ResMut<LatestSurfaceSnapshot>,
) {
    let saved = store.store.saved();
    if store.published.as_deref() == Some(saved.language.as_str()) {
        return;
    }
    latest.0.language_settings = project_language(Some(&Ok(saved.clone())));
    latest.0.revision += 1;
    store.published = Some(saved.language);
}
#[derive(SystemParam)]
pub struct LanguageControls<'w, 's> {
    feature: Res<'w, InteractionCapture>,
    route: Res<'w, ActiveRoute>,
    choice: Res<'w, LanguageChoiceResource>,
    buttons: Query<
        'w,
        's,
        (
            Entity,
            &'static LanguageChoiceButton,
            &'static ButtonDisabled,
        ),
    >,
}
pub fn activate(controls: LanguageControls, mut commands: Commands) {
    let preference = match &controls.choice.model.read {
        LanguageReadPhase::Unobserved => None,
        LanguageReadPhase::Observed { preference } => *preference,
        LanguageReadPhase::Failed { last_known, .. } => *last_known,
    };
    let operation_pending = match &controls.choice.model.phase {
        LanguageChoicePhase::Pending(_) | LanguageChoicePhase::AwaitingObservation { .. } => true,
        LanguageChoicePhase::Ready
        | LanguageChoicePhase::Rejected(_)
        | LanguageChoicePhase::Failed { .. } => false,
    };
    if !selected(
        &controls.feature,
        &controls.route,
        FeatureId::SettingsLanguageChoice,
        Route::Settings,
    ) || preference == Some(LanguagePreference::English)
        || operation_pending
    {
        return;
    }
    if let Some((entity, _, _)) = controls
        .buttons
        .iter()
        .find(|(_, choice, disabled)| choice.0 == LanguagePreference::English && !disabled.0)
    {
        commands.trigger(Activate { entity });
    }
}
#[derive(SystemParam)]
pub struct LanguageObservation<'w, 's> {
    choice: Res<'w, LanguageChoiceResource>,
    locale: Res<'w, UiLocale>,
    store: Res<'w, CaptureLanguageStore>,
    buttons: Query<
        'w,
        's,
        (
            Entity,
            &'static LanguageChoiceButton,
            &'static LocalizedLabel,
        ),
    >,
    cards: Query<'w, 's, Entity, With<LanguageChoiceCard>>,
}
impl LanguageObservation<'_, '_> {
    fn bounds(&self, geometry: &mut CaptureGeometry) -> Option<[f32; 4]> {
        let state = &self.choice.model;
        let preference = match &state.read {
            LanguageReadPhase::Unobserved => None,
            LanguageReadPhase::Observed { preference } => *preference,
            LanguageReadPhase::Failed { last_known, .. } => *last_known,
        };
        let settled = match &state.phase {
            LanguageChoicePhase::Ready => true,
            LanguageChoicePhase::Pending(_)
            | LanguageChoicePhase::Rejected(_)
            | LanguageChoicePhase::Failed { .. }
            | LanguageChoicePhase::AwaitingObservation { .. } => false,
        };
        if preference != Some(LanguagePreference::English)
            || !settled
            || self.locale.code() != "en-US"
            || self.store.store.saved().language != "en-US"
            || self.buttons.iter().count() != LanguagePreference::ALL.len()
        {
            return None;
        }
        let card = geometry.bounds(self.cards.iter().next()?, "language settings")?;
        for (entity, choice, label) in self.buttons.iter() {
            if label.0.key != choice.0.label_key() {
                return None;
            }
            let field = geometry.bounds(entity, "language choice")?;
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
    language: LanguageObservation,
    mut geometry: CaptureGeometry,
    mut state: ResMut<InteractionCapture>,
    mut observation: ResMut<ObservedInteraction>,
) {
    observation.publish(&mut state, language.bounds(&mut geometry));
}
