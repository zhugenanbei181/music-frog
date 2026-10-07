//! Native filter activation uses actual typed commands and narrowly scoped ECS parameters.
use super::geometry::CaptureGeometry;
use super::scroll::request_scroll;
use super::{InteractionCapture, ObservedInteraction, selected};
use crate::command::CommandSinkHandle;
use crate::command_execution::ApplicationCommandSink;
use crate::pages::profiles::ProfilesPageRoot;
use crate::pages::profiles_editor_filter::{
    EditorFilterDiscard, EditorFilterText, EditorFilterTransaction,
};
use crate::pages::profiles_editor_panes::{
    EditorFilterSaveButton, EditorFilterStatusText, ProfileEditorOptionsState, ProfileEditorPane,
    ProfileEditorPaneButton,
};
use crate::route::{ActiveRoute, Route, replay_surface_snapshot};
use crate::surface::LatestSurfaceSnapshot;
use bevy::app::App;
use bevy::camera::NormalizedRenderTarget;
use bevy::ecs::entity::Entity;
use bevy::ecs::query::With;
use bevy::ecs::resource::Resource;
use bevy::ecs::system::{Commands, Query, Res, ResMut, SystemParam};
use bevy::input::ButtonState;
use bevy::input::keyboard::{Key, KeyCode, KeyboardInput};
use bevy::math::Vec2;
use bevy::picking::backend::HitData;
use bevy::picking::events::{Pointer, PointerPress};
use bevy::picking::pointer::{Location, PointerButton, PointerId};
use bevy::ui::widget::Text;
use bevy::ui_widgets::Activate;
use bevy::window::{PrimaryWindow, Window};
use infiltrator_application::command_application::CommandApplication;
use infiltrator_application::core_application::CoreApplication;
use infiltrator_application::filter_capture_store::{
    FILTER_POLICY, FILTER_PROFILE, FilterCaptureStore,
};
use infiltrator_application::language_choice_fixtures::LanguageCaptureProcess;
use infiltrator_application::profile_application::ProfileApplication;
use infiltrator_application::surface_reader::ApplicationSurfaceReader;
use infiltrator_bevy_widgets::button::ButtonDisabled;
use infiltrator_bevy_widgets::text_input::TextField;
use infiltrator_composition::tokio_application_runtime;
use infiltrator_contract::command::CommandIntent;
use infiltrator_contract::error::ErrorCode;
use infiltrator_contract::parity::FeatureId;
use infiltrator_contract::subscription_filter_form::FilterField;
use infiltrator_contract::surface::{HostKind, SurfaceKind};
use infiltrator_contract::surface_snapshot::SurfaceOrigin;
use infiltrator_ports::surface::SurfaceReader;
use std::sync::{Arc, Mutex, atomic::Ordering};

#[derive(Resource)]
struct FilterCapture {
    core: Arc<CoreApplication>,
    reader: Arc<ApplicationSurfaceReader>,
    store: Arc<FilterCaptureStore>,
    stage: u8,
    reported: Option<(u8, ProfileEditorPane, bool)>,
}
pub fn install(app: &mut App) {
    let store = Arc::new(FilterCaptureStore::default());
    store.deny_write.store(true, Ordering::SeqCst);
    let core = Arc::new(CoreApplication::new(
        Arc::new(LanguageCaptureProcess),
        Arc::new(LanguageCaptureProcess),
        tokio_application_runtime().expect("isolated filter runtime"),
    ));
    core.install_command_handler(Arc::new(
        CommandApplication::new().with_profile(ProfileApplication::new(store.clone())),
    ));
    let reader = Arc::new(
        ApplicationSurfaceReader::new(core.clone(), SurfaceKind::BevyDesktop, HostKind::Desktop)
            .with_profiles(ProfileApplication::new(store.clone())),
    );
    app.insert_resource(CommandSinkHandle(Arc::new(ApplicationCommandSink::new(
        core.clone(),
    ))));
    app.insert_resource(FilterCapture {
        core,
        reader,
        store,
        stage: 0,
        reported: None,
    });
}
#[derive(SystemParam)]
pub struct FilterActivation<'w, 's> {
    feature: Res<'w, InteractionCapture>,
    route: Res<'w, ActiveRoute>,
    capture: ResMut<'w, FilterCapture>,
    latest: ResMut<'w, LatestSurfaceSnapshot>,
    options: ResMut<'w, ProfileEditorOptionsState>,
    tabs: Query<'w, 's, (Entity, &'static ProfileEditorPaneButton)>,
    fields: Query<'w, 's, (Entity, &'static EditorFilterText, &'static TextField)>,
    save: Query<'w, 's, Entity, With<EditorFilterSaveButton>>,
    cards: Query<'w, 's, Entity, With<EditorFilterTransaction>>,
    pages: Query<'w, 's, Entity, With<ProfilesPageRoot>>,
    windows: Query<'w, 's, (Entity, &'static Window), With<PrimaryWindow>>,
}
fn key(
    commands: &mut Commands,
    window: Entity,
    logical_key: Key,
    key_code: KeyCode,
    text: Option<&str>,
    state: ButtonState,
) {
    commands.write_message(KeyboardInput {
        logical_key,
        key_code,
        text: text.map(Into::into),
        state,
        repeat: false,
        window,
    });
}
pub fn activate(
    mut probe: FilterActivation,
    mut geometry: CaptureGeometry,
    mut commands: Commands,
) {
    if !selected(
        &probe.feature,
        &probe.route,
        FeatureId::ProfilesFilterEditor,
        Route::Profiles,
    ) {
        return;
    }
    let progress = (
        probe.capture.stage,
        probe.options.pane,
        probe.options.filter.can_edit(),
    );
    if probe.capture.reported != Some(progress) {
        eprintln!(
            "filter capture progress: stage={} pane={:?} editable={} tabs={} fields={} pages={} source={:?}",
            progress.0,
            progress.1,
            progress.2,
            probe.tabs.iter().count(),
            probe.fields.iter().count(),
            probe.pages.iter().count(),
            probe.options.filter.source_profile()
        );
        probe.capture.reported = Some(progress);
    }
    if probe.capture.stage == 0 {
        let core = probe.capture.core.clone();
        let reader = probe.capture.reader.clone();
        let result = Arc::new(Mutex::new(None));
        let reply = result.clone();
        tokio_application_runtime()
            .expect("filter runtime")
            .block_on(Box::pin(async move {
                core.execute(CommandIntent::LoadProfileDocument {
                    profile: Some(FILTER_PROFILE.into()),
                })
                .await
                .into_output()
                .expect("actual fixture document")
                .into_profile_document()
                .expect("typed document");
                let options = core
                    .execute(CommandIntent::LoadProfileOptions {
                        profile: Some(FILTER_PROFILE.into()),
                    })
                    .await
                    .into_output()
                    .expect("actual fixture options")
                    .into_profile_options()
                    .expect("typed options");
                let snapshot = reader.read().await;
                *reply.lock().expect("filter read") = Some((snapshot, options));
            }));
        let (snapshot, options) = result
            .lock()
            .expect("filter read")
            .take()
            .expect("completed filter read");
        let mut snapshot = snapshot.expect("actual page");
        snapshot.origin = SurfaceOrigin::Demo;
        snapshot.revision = probe.latest.0.revision + 1;
        probe
            .options
            .adopt_snapshot(&options.source, &options.mixin_yaml, &options.filter);
        probe.latest.0 = snapshot;
        replay_surface_snapshot(probe.latest.0.clone(), &mut commands);
        probe.capture.stage = 1;
        return;
    }
    let target = match probe.capture.stage {
        1 => probe
            .tabs
            .iter()
            .find(|(_, tab)| tab.pane == ProfileEditorPane::Filter)
            .map(|(entity, _)| entity),
        2 if probe.options.filter.can_edit() => probe
            .fields
            .iter()
            .find(|(_, kind, _)| kind.0 == FilterField::Include)
            .map(|(entity, _, _)| entity),
        3 if probe.options.filter.draft.include == "HK" => probe
            .fields
            .iter()
            .find(|(_, kind, _)| kind.0 == FilterField::Advanced)
            .map(|(entity, _, _)| entity),
        5 if probe.options.filter.draft.advanced_policy.as_deref() == Some(FILTER_POLICY) => {
            probe.save.single().ok()
        }
        _ => None,
    };
    if let Some(entity) = target {
        let Ok(page) = probe.pages.single() else {
            return;
        };
        if let (Some(bounds), Some(viewport)) = (geometry.rect(entity), geometry.rect(page)) {
            if !request_scroll(&mut commands, entity, bounds, viewport) {
                return;
            }
        } else {
            return;
        }
        if geometry.bounds(entity, "filter-native-control").is_none() {
            return;
        }
        let Ok((window, window_data)) = probe.windows.single() else {
            return;
        };
        if matches!(probe.capture.stage, 2 | 3) {
            let bounds = geometry.rect(entity).expect("measured native field");
            commands.trigger(PointerPress {
                entity,
                pointer: Pointer::new(
                    PointerId::Mouse,
                    Location {
                        target: NormalizedRenderTarget::None {
                            width: window_data.width() as u32,
                            height: window_data.height() as u32,
                        },
                        position: Vec2::new(
                            bounds[0] + bounds[2] / 2.0,
                            bounds[1] + bounds[3] / 2.0,
                        ),
                    },
                ),
                button: PointerButton::Primary,
                hit: HitData::new(Entity::PLACEHOLDER, 0.0, None, None),
                count: 1,
            });
        } else {
            commands.trigger(Activate { entity });
        }

        if probe.capture.stage == 2 {
            key(
                &mut commands,
                window,
                Key::Character("HK".into()),
                KeyCode::KeyH,
                Some("HK"),
                ButtonState::Pressed,
            );
        }
        if probe.capture.stage == 3 {
            key(
                &mut commands,
                window,
                Key::Control,
                KeyCode::ControlLeft,
                None,
                ButtonState::Pressed,
            );
            key(
                &mut commands,
                window,
                Key::Character("a".into()),
                KeyCode::KeyA,
                Some("a"),
                ButtonState::Pressed,
            );
        }
        probe.capture.stage += 1;
        return;
    }
    if probe.capture.stage == 4 {
        let Ok((window, _)) = probe.windows.single() else {
            return;
        };
        key(
            &mut commands,
            window,
            Key::Control,
            KeyCode::ControlLeft,
            None,
            ButtonState::Released,
        );
        key(
            &mut commands,
            window,
            Key::Character(FILTER_POLICY.into()),
            KeyCode::KeyM,
            Some(FILTER_POLICY),
            ButtonState::Pressed,
        );
        probe.capture.stage = 5;
        return;
    }
    if probe.capture.stage == 6
        && let (Ok(card), Ok(page)) = (probe.cards.single(), probe.pages.single())
        && let (Some(bounds), Some(viewport)) = (geometry.rect(card), geometry.rect(page))
    {
        request_scroll(&mut commands, card, bounds, viewport);
    }
}
#[derive(SystemParam)]
pub struct FilterObservation<'w, 's> {
    capture: Res<'w, FilterCapture>,
    options: Res<'w, ProfileEditorOptionsState>,
    cards: Query<'w, 's, Entity, With<EditorFilterTransaction>>,
    status: Query<'w, 's, (Entity, &'static Text), With<EditorFilterStatusText>>,
    fields: Query<'w, 's, (Entity, &'static EditorFilterText, &'static TextField)>,
    save: Query<'w, 's, (Entity, &'static ButtonDisabled), With<EditorFilterSaveButton>>,
    discard: Query<'w, 's, (Entity, &'static ButtonDisabled), With<EditorFilterDiscard>>,
}
pub fn observe(
    probe: FilterObservation,
    mut geometry: CaptureGeometry,
    mut feature: ResMut<InteractionCapture>,
    mut observed: ResMut<ObservedInteraction>,
) {
    if feature.feature != FeatureId::ProfilesFilterEditor || probe.capture.stage != 6 {
        return;
    }
    let editor = &probe.options.filter;
    if editor.pending.is_some()
        || editor.source_profile() != Some(FILTER_PROFILE)
        || editor.draft.include != "HK"
        || editor.draft.advanced_policy.as_deref() != Some(FILTER_POLICY)
        || !editor
            .failure
            .as_ref()
            .is_some_and(|failure| failure.code == ErrorCode::Permission)
        || probe.capture.store.writes.load(Ordering::SeqCst) != 0
    {
        return;
    }
    let (Ok(card), Ok((status, text)), Ok((save, save_disabled)), Ok((discard, discard_disabled))) = (
        probe.cards.single(),
        probe.status.single(),
        probe.save.single(),
        probe.discard.single(),
    ) else {
        return;
    };
    if save_disabled.0 || discard_disabled.0 || !text.0.contains("Allow profile write access") {
        return;
    }
    let Some((advanced, _, field)) = probe
        .fields
        .iter()
        .find(|(_, kind, _)| kind.0 == FilterField::Advanced)
    else {
        return;
    };
    if field.0.text() != FILTER_POLICY {
        return;
    }
    let Some(bounds) = geometry.bounds(card, "filter-transaction") else {
        return;
    };
    for entity in [advanced, status, save, discard] {
        if geometry.bounds(entity, "filter-required-surface").is_none() {
            return;
        }
    }
    observed.publish(&mut feature, Some(bounds));
}
