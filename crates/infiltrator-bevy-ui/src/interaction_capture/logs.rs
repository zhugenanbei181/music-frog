//! Actual application facts, native regex input and visible highlight observation.
use super::geometry::CaptureGeometry;
use super::scroll::request_scroll;
use super::{InteractionCapture, ObservedInteraction, selected};
use crate::command::CommandSinkHandle;
use crate::command_execution::ApplicationCommandSink;
use crate::pages::logs::{LogMessageText, LogsPageRoot, PauseLogsButton};
use crate::pages::logs_rows::LogRowIdentity;
use crate::pages::logs_search::LogsViewState;
use crate::route::{ActiveRoute, Route};
use crate::surface::{LatestSurfaceSnapshot, SurfaceSnapshotUpdated};
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
use bevy::picking::events::{Pointer, PointerClick, PointerPress};
use bevy::picking::pointer::{Location, PointerButton, PointerId};
use bevy::text::TextColor;
use bevy::ui::widget::Text;
use bevy::window::{PrimaryWindow, Window};
use infiltrator_application::command_application::CommandApplication;
use infiltrator_application::core_application::CoreApplication;
use infiltrator_application::log_capture_fixtures::{
    LOG_QUERY, LogCaptureProcess, append_follow_records, populate_logs,
};
use infiltrator_application::log_export_application::LogExportApplication;
use infiltrator_application::log_export_capture::ReadOnlyLogExportCapture;
use infiltrator_application::surface_reader::ApplicationSurfaceReader;
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::text_input::TextField;
use infiltrator_bevy_widgets::text_input::native::NativeTextField;
use infiltrator_composition::tokio_application_runtime;
use infiltrator_contract::parity::FeatureId;
use infiltrator_contract::surface::{HostKind, SurfaceKind};
use infiltrator_contract::surface_snapshot::SurfaceOrigin;
use infiltrator_ports::application_runtime::ApplicationRuntime;
use infiltrator_ports::surface::SurfaceReader;
use std::sync::{Arc, Mutex};
use std::time::Duration;

#[derive(Resource)]
pub(super) struct LogsCapture {
    core: Arc<CoreApplication>,
    reader: Arc<ApplicationSurfaceReader>,
    runtime: Arc<dyn ApplicationRuntime>,
    pub(super) stage: u8,
}
pub fn install(app: &mut App) {
    let runtime = tokio_application_runtime().expect("isolated logs executor");
    let process = Arc::new(LogCaptureProcess::default());
    let core = Arc::new(CoreApplication::new(
        process.clone(),
        process,
        runtime.clone(),
    ));
    core.install_command_handler(Arc::new(
        CommandApplication::new()
            .with_logs(core.log_application())
            .with_log_export(LogExportApplication::new(
                core.log_application(),
                Some(Arc::new(ReadOnlyLogExportCapture)),
                vec![],
            )),
    ));
    let reader = Arc::new(ApplicationSurfaceReader::new(
        core.clone(),
        SurfaceKind::BevyDesktop,
        HostKind::Desktop,
    ));
    app.insert_resource(CommandSinkHandle(Arc::new(ApplicationCommandSink::new(
        core.clone(),
    ))));
    app.insert_resource(LogsCapture {
        core,
        reader,
        runtime,
        stage: 0,
    });
}
#[derive(SystemParam)]
pub struct LogsControls<'w, 's> {
    capture: ResMut<'w, LogsCapture>,
    route: Res<'w, ActiveRoute>,
    view: Res<'w, LogsViewState>,
    latest: Res<'w, LatestSurfaceSnapshot>,
    fields: Query<'w, 's, (Entity, &'static NativeTextField, &'static TextField)>,
    rows: Query<'w, 's, (Entity, &'static LogRowIdentity)>,
    pause: Query<'w, 's, Entity, With<PauseLogsButton>>,
    messages: Query<
        'w,
        's,
        (
            Entity,
            &'static LogMessageText,
            &'static Text,
            &'static TextColor,
        ),
    >,
    palette: Res<'w, UiPalette>,
    scroll: Query<'w, 's, Entity, With<LogsPageRoot>>,
    windows: Query<'w, 's, (Entity, &'static Window), With<PrimaryWindow>>,
}
pub fn activate(
    mut controls: LogsControls,
    feature: Res<InteractionCapture>,
    geometry: CaptureGeometry,
    mut commands: Commands,
) {
    if !selected(&feature, &controls.route, feature.feature, Route::Logs) {
        return;
    }
    if feature.feature == FeatureId::LogsRedactedExport && controls.capture.stage != 0 {
        return;
    }
    if feature.feature == FeatureId::LogsScrollLock && controls.capture.stage != 0 {
        activate_follow(&mut controls, &geometry, &mut commands);
        return;
    }
    match controls.capture.stage {
        0 => {
            let core = controls.capture.core.clone();
            let reader = controls.capture.reader.clone();
            let result = Arc::new(Mutex::new(None));
            let output = result.clone();
            controls.capture.runtime.block_on(Box::pin(async move {
                populate_logs(&core)
                    .await
                    .expect("actual isolated logs lifecycle");
                *output.lock().unwrap() =
                    Some(reader.read().await.expect("actual isolated logs reader"));
            }));
            let mut snapshot = result
                .lock()
                .unwrap()
                .take()
                .expect("actual isolated log facts");
            snapshot.origin = SurfaceOrigin::Demo;
            snapshot.revision = controls.latest.0.revision + 1;
            commands.trigger(SurfaceSnapshotUpdated(snapshot));
            controls.capture.stage = 1;
        }
        1 => {
            let Some((entity, _, _)) = controls.fields.iter().find(|(_, marker, _)| marker.0 == 11)
            else {
                return;
            };
            let (Ok(scroll), Ok((_, window))) =
                (controls.scroll.single(), controls.windows.single())
            else {
                return;
            };
            let (Some(bounds), Some(viewport)) = (geometry.rect(entity), geometry.rect(scroll))
            else {
                return;
            };
            if !request_scroll(&mut commands, scroll, bounds, viewport) {
                return;
            }
            commands.trigger(PointerPress {
                entity,
                pointer: Pointer::new(
                    PointerId::Mouse,
                    Location {
                        target: NormalizedRenderTarget::None {
                            width: window.width() as u32,
                            height: window.height() as u32,
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
            controls.capture.stage = 2;
        }
        2 => {
            let Ok((window, _)) = controls.windows.single() else {
                return;
            };
            commands.write_message(KeyboardInput {
                key_code: KeyCode::KeyL,
                logical_key: Key::Character(LOG_QUERY.into()),
                text: Some(LOG_QUERY.into()),
                state: ButtonState::Pressed,
                repeat: false,
                window,
            });
            controls.capture.stage = 3;
        }
        _ => {
            if let Some((row, _)) = controls.rows.iter().find(|(_, identity)| identity.0 == 1)
                && let Ok(scroll) = controls.scroll.single()
                && let (Some(bounds), Some(viewport)) = (geometry.rect(row), geometry.rect(scroll))
            {
                request_scroll(&mut commands, scroll, bounds, viewport);
            }
        }
    }
}
pub fn observe(
    controls: LogsControls,
    mut geometry: CaptureGeometry,
    mut feature: ResMut<InteractionCapture>,
    mut observed: ResMut<ObservedInteraction>,
) {
    if !selected(&feature, &controls.route, feature.feature, Route::Logs)
        || controls.capture.stage != 3
    {
        return;
    }
    if feature.feature == FeatureId::LogsScrollLock {
        let bounds = (|| {
            if controls.view.0.follow.should_follow()
                || controls.view.0.source_count() != 35
                || !controls.view.0.source_current()
            {
                return None;
            }
            geometry.bounds(controls.pause.single().ok()?, "logs-follow-control")?;
            let (row, _) = controls.rows.iter().find(|(_, identity)| identity.0 == 1)?;
            geometry.bounds(row, "logs-locked-records")
        })();
        observed.publish(&mut feature, bounds);
        return;
    }
    let state = &controls.view.0;
    let (Some((field, _, text)), Some((row, _)), Some((message, _, copy, color))) = (
        controls.fields.iter().find(|(_, marker, _)| marker.0 == 11),
        controls.rows.iter().find(|(_, identity)| identity.0 == 1),
        controls
            .messages
            .iter()
            .find(|(_, marker, _, _)| marker.0 == 0),
    ) else {
        return;
    };
    if text.0.text() != LOG_QUERY
        || !state.source_current()
        || state.matched_count() != 2
        || state.source_count() != 3
        || copy.0 != "API.EXAMPLE"
        || color.0 != controls.palette.accent
    {
        return;
    }
    let bounds = (|| {
        geometry.bounds(field, "logs-regex-input")?;
        geometry.bounds(message, "logs-highlight-text")?;
        geometry.bounds(row, "logs-highlight-result")
    })();
    observed.publish(&mut feature, bounds);
}

fn activate_follow(
    controls: &mut LogsControls,
    geometry: &CaptureGeometry,
    commands: &mut Commands,
) {
    match controls.capture.stage {
        1 => {
            if controls.view.0.source_count() != 3 {
                return;
            }
            let (Ok(entity), Ok((_, window))) =
                (controls.pause.single(), controls.windows.single())
            else {
                return;
            };
            let Some(bounds) = geometry.rect(entity) else {
                return;
            };
            let pointer = || {
                Pointer::new(
                    PointerId::Mouse,
                    Location {
                        target: NormalizedRenderTarget::None {
                            width: window.width() as u32,
                            height: window.height() as u32,
                        },
                        position: Vec2::new(
                            bounds[0] + bounds[2] / 2.0,
                            bounds[1] + bounds[3] / 2.0,
                        ),
                    },
                )
            };
            commands.trigger(PointerPress {
                entity,
                pointer: pointer(),
                button: PointerButton::Primary,
                hit: HitData::new(Entity::PLACEHOLDER, 0.0, None, None),
                count: 1,
            });
            commands.trigger(PointerClick {
                duration: Duration::from_millis(40),
                entity,
                pointer: pointer(),
                button: PointerButton::Primary,
                hit: HitData::new(Entity::PLACEHOLDER, 0.0, None, None),
                count: 1,
            });
            controls.capture.stage = 2;
        }
        2 => {
            if controls.view.0.follow.should_follow() {
                return;
            }
            append_follow_records(&controls.capture.core)
                .expect("actual continued logs while locked");
            let reader = controls.capture.reader.clone();
            let result = Arc::new(Mutex::new(None));
            let output = result.clone();
            controls.capture.runtime.block_on(Box::pin(async move {
                *output.lock().unwrap() =
                    Some(reader.read().await.expect("continued actual log reader"));
            }));
            let mut snapshot = result
                .lock()
                .unwrap()
                .take()
                .expect("continued log snapshot");
            snapshot.origin = SurfaceOrigin::Demo;
            snapshot.revision = controls.latest.0.revision + 1;
            commands.trigger(SurfaceSnapshotUpdated(snapshot));
            controls.capture.stage = 3;
        }
        _ => {
            if let Some((row, _)) = controls.rows.iter().find(|(_, identity)| identity.0 == 1)
                && let Ok(scroll) = controls.scroll.single()
                && let (Some(bounds), Some(viewport)) = (geometry.rect(row), geometry.rect(scroll))
            {
                request_scroll(commands, scroll, bounds, viewport);
            }
        }
    }
}
