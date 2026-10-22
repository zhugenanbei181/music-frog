//! Run the script sandbox and review a script export through the page's own
//! controls.
//!
//! The script workbench is a pure consumer of the shared script owner: the
//! run control submits the shared `RunScriptSandbox` command and the export
//! control submits the shared `PrepareScriptExport` command. The capture
//! activates the workbench's own preset and action controls, so the surface
//! really submits those typed commands, and the owner's real answer reaches
//! the workbench's own read model.
use super::capability::capture_command_application;
use super::geometry::CaptureGeometry;
use super::host::{CaptureCapability, CaptureCommandSink};
use super::scroll::{outermost_scroll_area, request_scroll};
use super::{InteractionCapture, ObservedInteraction, selected};
use crate::command::CommandSinkHandle;
use crate::pages::profiles::ProfilesProjectionUpdated;
use crate::pages::profiles_script_scene::{ScriptResultBody, ScriptResultRow, ScriptReviewLine};
use crate::pages::profiles_script_workbench::{ScriptControl, ScriptWorkbenchState};
use crate::route::{ActiveRoute, Route};
use crate::surface::{LatestSurfaceSnapshot, profiles_projection};
use bevy::app::App;
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::ChildOf;
use bevy::ecs::query::With;
use bevy::ecs::resource::Resource;
use bevy::ecs::system::{Commands, Query, Res, ResMut, SystemParam};
use bevy::ui_widgets::{Activate, ScrollArea};
use infiltrator_application::script_application::ScriptApplication;
use infiltrator_application::script_export_application::ScriptExportApplication;
use infiltrator_contract::parity::FeatureId;
use infiltrator_contract::script_export::ScriptExportKind;
use std::sync::Arc;

/// The catalogue preset the capture selects before it runs or exports.
const PRESET: &str = "auto-country-groups";

/// The capture's own progress through the workbench interaction.
#[derive(Resource, Default)]
pub struct ScriptCapture {
    submitted: bool,
    published: bool,
}

pub fn install(app: &mut App) {
    app.insert_resource(CommandSinkHandle(Arc::new(CaptureCommandSink::new(
        capture_command_application().with_scripts(
            ScriptApplication::new(),
            ScriptExportApplication::without_host_port(),
        ),
        CaptureCapability::Script,
    ))));
    app.init_resource::<ScriptCapture>();
}

/// The business this capture drives: the export review or the sandbox run.
fn export_business(feature: FeatureId) -> Option<bool> {
    match feature {
        FeatureId::ProfilesScriptSandbox => Some(false),
        FeatureId::ProfilesScriptExportReview => Some(true),
        _ => None,
    }
}

/// The workbench's own controls and its shared read model.
#[derive(SystemParam)]
pub struct ScriptSurface<'w, 's> {
    route: Res<'w, ActiveRoute>,
    capture: ResMut<'w, ScriptCapture>,
    feature: Res<'w, InteractionCapture>,
    state: Res<'w, ScriptWorkbenchState>,
    latest: ResMut<'w, LatestSurfaceSnapshot>,
    controls: Query<'w, 's, (Entity, &'static ScriptControl)>,
}

/// Drive the workbench's own preset and action controls.
pub fn activate(mut surface: ScriptSurface, mut commands: Commands) {
    let Some(export) = export_business(surface.feature.feature) else {
        return;
    };
    if !selected(
        &surface.feature,
        &surface.route,
        surface.feature.feature,
        Route::Profiles,
    ) {
        return;
    }
    if !surface.capture.submitted {
        let Some((preset, _)) = surface
            .controls
            .iter()
            .find(|(_, control)| matches!(**control, ScriptControl::Preset(id) if id == PRESET))
        else {
            return;
        };
        let Some((action, _)) = surface
            .controls
            .iter()
            .find(|(_, control)| match **control {
                ScriptControl::Run => !export,
                ScriptControl::Export(kind) => {
                    export && kind == ScriptExportKind::DirectiveDslScript
                }
                _ => false,
            })
        else {
            return;
        };
        surface.capture.submitted = true;
        // The preset comes first: the workbench applies the queued controls in
        // order, so the action runs against the preset's real script.
        commands.trigger(Activate { entity: preset });
        commands.trigger(Activate { entity: action });
        return;
    }
    if surface.capture.published {
        return;
    }
    // The owner publishes the export review when the prepare command really
    // answered; the panel is already visible while the command is still
    // running, and a capture that published then would carry no artifact.
    let ready = if export {
        surface.state.model.export().is_some()
    } else {
        surface.state.model.snapshot().is_some()
    };
    if !ready {
        return;
    }
    surface.capture.published = true;
    publish_owner_state(&mut surface, export);
    let projection = profiles_projection(&surface.latest.0);
    commands.trigger(ProfilesProjectionUpdated(projection));
}

/// Publish the owner's readback into the shared snapshot.
///
/// The capture's snapshot belongs to another core scope, so the whole-snapshot
/// event is fenced out; the page's own projection event carries the owner's
/// script facts instead.
fn publish_owner_state(surface: &mut ScriptSurface, export: bool) {
    if export {
        let Some(snapshot) = surface.state.model.export().cloned() else {
            return;
        };
        surface.latest.0.script_export = Some(snapshot);
        return;
    }
    let Some(snapshot) = surface.state.model.snapshot().cloned() else {
        return;
    };
    surface.latest.0.script_sandbox = Some(snapshot);
}

/// The workbench's own console and its export review lines.
#[derive(SystemParam)]
pub struct ScriptView<'w, 's> {
    route: Res<'w, ActiveRoute>,
    capture: Res<'w, ScriptCapture>,
    state: Res<'w, ScriptWorkbenchState>,
    bodies: Query<'w, 's, Entity, With<ScriptResultBody>>,
    rows: Query<'w, 's, (Entity, &'static ScriptResultRow)>,
    reviews: Query<'w, 's, (Entity, &'static ScriptReviewLine)>,
    parents: Query<'w, 's, &'static ChildOf>,
    areas: Query<'w, 's, Entity, With<ScrollArea>>,
}

/// Observe the workbench's own console rows and its export review card.
pub fn observe(
    surface: ScriptView,
    mut feature: ResMut<InteractionCapture>,
    mut geometry: CaptureGeometry,
    mut observation: ResMut<ObservedInteraction>,
    mut commands: Commands,
) {
    let bounds = (|| {
        let export = export_business(feature.feature)?;
        if !selected(&feature, &surface.route, feature.feature, Route::Profiles)
            || !surface.capture.submitted
        {
            return None;
        }
        if export {
            if !surface.state.model.is_export_visible() {
                return None;
            }
            let (entity, _) = surface
                .reviews
                .iter()
                .find(|(_, line)| matches!(**line, ScriptReviewLine::Content))?;
            return geometry.bounds(entity, "script export review");
        }
        if surface.state.model.snapshot().is_none() || surface.state.rows.is_empty() {
            return None;
        }
        let body = surface.bodies.iter().next()?;
        let row = surface.rows.iter().next()?.0;
        // The workbench sits far below the fold of the profiles page, so the
        // capture scrolls the page's own viewport until the console is really
        // rendered, and measures it on the next frame.
        if !geometry.unclipped(body) {
            reveal(&surface, &mut geometry, &mut commands, body);
            return None;
        }
        let body_bounds = geometry.bounds(body, "script console")?;
        let row_bounds = geometry.bounds(row, "script console row")?;
        if row_bounds[0] < body_bounds[0] || row_bounds[1] < body_bounds[1] {
            return None;
        }
        Some(body_bounds)
    })();
    observation.publish(&mut feature, bounds);
}

/// Scroll the profiles page until the console is really rendered.
fn reveal(
    surface: &ScriptView<'_, '_>,
    geometry: &mut CaptureGeometry<'_, '_>,
    commands: &mut Commands,
    target: Entity,
) {
    let Some(page) = outermost_scroll_area(target, &surface.parents, &surface.areas) else {
        return;
    };
    let Some(viewport) = geometry.rect(page) else {
        return;
    };
    let Some(bounds) = geometry.rect(target) else {
        return;
    };
    request_scroll(commands, page, bounds, viewport);
}
