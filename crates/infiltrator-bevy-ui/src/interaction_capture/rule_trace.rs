//! Drive the actual tracer Tab, native input and shared command owner before observing pixels.
use super::geometry::CaptureGeometry;
use super::host::{CaptureCapability, CaptureCommandSink};
use super::scroll::request_scroll;
use super::{InteractionCapture, ObservedInteraction, selected};
use crate::command::CommandSinkHandle;
use crate::pages::rules::RulesPageRoot;
use crate::pages::rules_tabs::{RulesTabChip, RulesTabState};
use crate::pages::rules_tracer::{
    ApplyTracerRuleOverrideButton, RulesTraceState, SimulateRuleTraceButton, TracerDecisionTree,
    TracerNativeField, TracerText,
};
use crate::pages::rules_tracer_confirm::{
    TraceConfirmationAction, TraceConfirmationCard, TraceConfirmationCopy, TraceConfirmationRoot,
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
use bevy::ui_widgets::{Activate, Button};
use bevy::window::{PrimaryWindow, Window};
use infiltrator_application::command_application::CommandApplication;
use infiltrator_application::configuration_application::ConfigurationApplication;
use infiltrator_application::core_application::CoreApplication;
use infiltrator_application::language_choice_fixtures::LanguageCaptureProcess;
use infiltrator_application::profile_application::ProfileApplication;
use infiltrator_application::rule_trace_fixtures::RuleTraceStore;
use infiltrator_application::rule_tracer_application::RuleTracerApplication;
use infiltrator_application::surface_reader::ApplicationSurfaceReader;
use infiltrator_bevy_widgets::button::ButtonDisabled;
use infiltrator_bevy_widgets::text_input::TextField;
use infiltrator_bevy_widgets::text_input::native::NativeTextField;
use infiltrator_composition::tokio_application_runtime;
use infiltrator_contract::parity::FeatureId;
use infiltrator_contract::rules_workspace::RulesTab;
use infiltrator_contract::surface::{HostKind, SurfaceKind};
use infiltrator_contract::surface_snapshot::SurfaceOrigin;
use infiltrator_ports::surface::SurfaceReader;
use std::mem::discriminant;
use std::sync::{Arc, Mutex, atomic::Ordering};

#[derive(Resource)]
struct TraceCapture {
    owner: RuleTracerApplication,
    store: Arc<RuleTraceStore>,
    reader: Arc<ApplicationSurfaceReader>,
    stage: u8,
}
pub fn install(app: &mut App) {
    let store = Arc::new(RuleTraceStore::default());
    let owner = RuleTracerApplication::new();
    owner.set_override_port(store.clone());
    let handler = CommandApplication::new().with_rule_tracer(owner.clone());
    let core = Arc::new(CoreApplication::new(
        Arc::new(LanguageCaptureProcess),
        Arc::new(LanguageCaptureProcess),
        tokio_application_runtime().expect("isolated capture runtime"),
    ));
    let reader = Arc::new(
        ApplicationSurfaceReader::new(core, SurfaceKind::BevyDesktop, HostKind::Desktop)
            .with_rule_tracer(owner.clone())
            .with_profiles(ProfileApplication::new(store.clone()))
            .with_configuration(ConfigurationApplication::new(store.clone())),
    );
    app.insert_resource(CommandSinkHandle(Arc::new(CaptureCommandSink::new(
        handler,
        CaptureCapability::RuleTrace,
    ))));
    app.insert_resource(TraceCapture {
        owner,
        store,
        reader,
        stage: 0,
    });
}
#[derive(SystemParam)]
pub struct TraceActivation<'w, 's> {
    feature: Res<'w, InteractionCapture>,
    route: Res<'w, ActiveRoute>,
    capture: ResMut<'w, TraceCapture>,
    state: Res<'w, RulesTraceState>,
    tab: Res<'w, RulesTabState>,
    latest: ResMut<'w, LatestSurfaceSnapshot>,
    tabs: Query<'w, 's, (Entity, &'static RulesTabChip)>,
    fields: Query<
        'w,
        's,
        (Entity, &'static NativeTextField, &'static TextField),
        With<TracerNativeField>,
    >,
    buttons: Query<'w, 's, (Entity, &'static ButtonDisabled), With<SimulateRuleTraceButton>>,
    windows: Query<'w, 's, (Entity, &'static Window), With<PrimaryWindow>>,
    cards: Query<'w, 's, Entity, With<TracerDecisionTree>>,
    pages: Query<'w, 's, Entity, With<RulesPageRoot>>,
    apply: Query<'w, 's, (Entity, &'static ButtonDisabled), With<ApplyTracerRuleOverrideButton>>,
}
pub fn activate(mut probe: TraceActivation, mut geometry: CaptureGeometry, mut commands: Commands) {
    if !selected(
        &probe.feature,
        &probe.route,
        FeatureId::RulesTracerDrawer,
        Route::Rules,
    ) && !selected(
        &probe.feature,
        &probe.route,
        FeatureId::RulesOverrideEditor,
        Route::Rules,
    ) {
        return;
    }
    let Ok((window_id, window)) = probe.windows.single() else {
        return;
    };
    if probe.capture.stage == 0 {
        let Some((entity, _)) = probe
            .tabs
            .iter()
            .find(|(_, tab)| tab.0 == RulesTab::Tracer.index())
        else {
            return;
        };
        if geometry.bounds(entity, "tracer Tab launcher").is_some() {
            commands.trigger(Activate { entity });
            probe.capture.stage = 1;
        }
        return;
    }
    if probe.tab.tab != RulesTab::Tracer {
        return;
    }
    if let Some((order, value)) = match probe.capture.stage {
        1 => Some((0, "google.com")),
        3 => Some((1, "192.0.2.1")),
        _ => None,
    } {
        let Some((entity, _, field)) = probe.fields.iter().find(|(_, native, _)| native.0 == order)
        else {
            return;
        };
        if let Ok(page) = probe.pages.single()
            && let (Some(bounds), Some(viewport)) = (geometry.rect(entity), geometry.rect(page))
            && !request_scroll(&mut commands, page, bounds, viewport)
        {
            return;
        }
        let Some([x, y, width, height]) = geometry.bounds(entity, "tracer native input") else {
            return;
        };
        commands.trigger(PointerPress {
            entity,
            pointer: Pointer::new(
                PointerId::Mouse,
                Location {
                    target: NormalizedRenderTarget::None {
                        width: window.width() as u32,
                        height: window.height() as u32,
                    },
                    position: Vec2::new(x + width / 2.0, y + height / 2.0),
                },
            ),
            button: PointerButton::Primary,
            hit: HitData::new(Entity::PLACEHOLDER, 0.0, None, None),
            count: 1,
        });
        commands.write_message(KeyboardInput {
            key_code: KeyCode::End,
            logical_key: Key::End,
            text: None,
            state: ButtonState::Pressed,
            repeat: false,
            window: window_id,
        });
        for _ in field.0.text().chars() {
            commands.write_message(KeyboardInput {
                key_code: KeyCode::Backspace,
                logical_key: Key::Backspace,
                text: None,
                state: ButtonState::Pressed,
                repeat: false,
                window: window_id,
            });
        }
        commands.write_message(KeyboardInput {
            key_code: KeyCode::KeyM,
            logical_key: Key::Character(value.into()),
            text: Some(value.into()),
            state: ButtonState::Pressed,
            repeat: false,
            window: window_id,
        });
        probe.capture.stage += 1;
        return;
    }
    match probe.capture.stage {
        2 if probe.state.model.query == "google.com" => probe.capture.stage = 3,
        4 if probe.state.model.source_ip == "192.0.2.1" => {
            let Some((entity, _)) = probe.buttons.iter().find(|(_, disabled)| !disabled.0) else {
                return;
            };
            if let Ok(page) = probe.pages.single()
                && let (Some(bounds), Some(viewport)) = (geometry.rect(entity), geometry.rect(page))
                && !request_scroll(&mut commands, page, bounds, viewport)
            {
                return;
            }
            if geometry
                .bounds(entity, "tracer simulation control")
                .is_some()
            {
                commands.trigger(Activate { entity });
                probe.capture.stage = 5;
            }
        }
        5 if !probe.state.model.busy() => {
            let execution = probe.capture.owner.execution();
            if execution.report.is_none() {
                return;
            }
            let reader = probe.capture.reader.clone();
            let observed = Arc::new(Mutex::new(None));
            let result = observed.clone();
            tokio_application_runtime()
                .expect("isolated capture runtime")
                .block_on(Box::pin(async move {
                    *result.lock().expect("capture read observation") = Some(reader.read().await);
                }));
            let mut snapshot = observed
                .lock()
                .expect("capture read observation")
                .take()
                .expect("completed rule page read")
                .expect("actual shared rule page");
            snapshot.origin = SurfaceOrigin::Demo;
            snapshot.revision = probe.latest.0.revision + 1;
            probe.latest.0 = snapshot;
            replay_surface_snapshot(probe.latest.0.clone(), &mut commands);
            probe.capture.stage = 6;
        }
        6 => {
            if probe.feature.feature == FeatureId::RulesOverrideEditor {
                let Some((entity, _)) = probe.apply.iter().find(|(_, disabled)| !disabled.0) else {
                    return;
                };
                if let Ok(page) = probe.pages.single()
                    && let (Some(bounds), Some(viewport)) =
                        (geometry.rect(entity), geometry.rect(page))
                    && !request_scroll(&mut commands, page, bounds, viewport)
                {
                    return;
                }
                if geometry
                    .bounds(entity, "rule override native launcher")
                    .is_some()
                {
                    commands.trigger(Activate { entity });
                    probe.capture.stage = 7;
                }
                return;
            }
            if let (Ok(card), Ok(page)) = (probe.cards.single(), probe.pages.single())
                && let (Some(card_bounds), Some(page_bounds)) =
                    (geometry.rect(card), geometry.rect(page))
            {
                request_scroll(&mut commands, page, card_bounds, page_bounds);
            }
        }
        _ => {}
    }
}
#[derive(SystemParam)]
pub struct TraceObservation<'w, 's> {
    capture: Res<'w, TraceCapture>,
    state: Res<'w, RulesTraceState>,
    cards: Query<'w, 's, Entity, With<TracerDecisionTree>>,
    lines: Query<'w, 's, (Entity, &'static Text, &'static TracerText)>,
    page: Query<'w, 's, Entity, With<RulesPageRoot>>,
    confirmations: Query<'w, 's, Entity, With<TraceConfirmationCard>>,
    roots: Query<'w, 's, Entity, With<TraceConfirmationRoot>>,
    copy: Query<'w, 's, (Entity, &'static Text), With<TraceConfirmationCopy>>,
    controls: Query<
        'w,
        's,
        (
            Entity,
            &'static TraceConfirmationAction,
            &'static ButtonDisabled,
        ),
        With<Button>,
    >,
}
pub fn observe(
    probe: TraceObservation,
    mut geometry: CaptureGeometry,
    mut feature: ResMut<InteractionCapture>,
    mut observed: ResMut<ObservedInteraction>,
) {
    let bounds = (|| {
        if !matches!(probe.capture.stage, 6 | 7)
            || probe.capture.store.rule_loads.load(Ordering::SeqCst) != 1
            || probe.state.model.busy()
            || probe.state.model.current_failure().is_some()
        {
            return None;
        }
        let report = probe.state.model.snapshot.report.as_ref()?;
        if report.decision_chain.as_ref()?.matched_rule_raw != "DOMAIN-SUFFIX,google.com,PROXY"
            || !probe.state.model.draft_matches(report)
        {
            return None;
        }
        if feature.feature == FeatureId::RulesOverrideEditor {
            if probe.capture.stage != 7
                || probe.capture.store.writes.load(Ordering::SeqCst) != 0
                || !geometry.visible(probe.roots.single().ok()?)
            {
                return None;
            }
            let confirmation = probe.state.model.confirmation.as_ref()?;
            if confirmation.expected_rule != "DOMAIN-SUFFIX,google.com,PROXY"
                || confirmation.new_target != "DIRECT"
            {
                return None;
            }
            let (copy, text) = probe.copy.single().ok()?;
            if !text.0.contains(&confirmation.expected_source.profile)
                || !text.0.contains(&confirmation.expected_rule)
                || !text.0.contains(&confirmation.new_target)
            {
                return None;
            }
            geometry.bounds(copy, "rule override source and target")?;
            for action in [
                TraceConfirmationAction::Cancel,
                TraceConfirmationAction::Apply,
            ] {
                let (entity, _, _) = probe.controls.iter().find(|(_, candidate, disabled)| {
                    discriminant(*candidate) == discriminant(&action) && !disabled.0
                })?;
                geometry.bounds(entity, "rule override native confirmation control")?;
            }
            return geometry.bounds(
                probe.confirmations.single().ok()?,
                "rule override confirmation panel",
            );
        }
        for (entity, text, slot) in &probe.lines {
            if slot.0 <= 5 && !text.0.is_empty() {
                geometry.bounds(entity, "tracer semantic stage")?;
            }
        }
        let _page = probe.page.single().ok()?;
        geometry.bounds(probe.cards.single().ok()?, "tracer decision tree")
    })();
    observed.publish(&mut feature, bounds);
}
