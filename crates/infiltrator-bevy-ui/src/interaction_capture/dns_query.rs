//! Drive a visible native launcher, actual keyboard and menu, then the real query command.
use super::geometry::CaptureGeometry;
use super::host::{CaptureCapability, CaptureCommandSink};
use super::{InteractionCapture, ObservedInteraction, selected};
use crate::command::CommandSinkHandle;
use crate::pages::dns_query::{QueryAction, QueryLine, QueryModalCard, QueryNameField, QueryPanel};
use crate::route::{ActiveRoute, Route};
use crate::surface::LatestSurfaceSnapshot;
use bevy::app::App;
use bevy::ecs::entity::Entity;
use bevy::ecs::query::With;
use bevy::ecs::resource::Resource;
use bevy::ecs::system::{Commands, Query, Res, ResMut, SystemParam};
use bevy::input::ButtonState;
use bevy::input::keyboard::{Key, KeyCode, KeyboardInput};
use bevy::ui::widget::Text;
use bevy::ui_widgets::Activate;
use bevy::window::{PrimaryWindow, Window};
use infiltrator_application::command_application::CommandApplication;
use infiltrator_application::dns_query_actions::QuerySection;
use infiltrator_application::dns_query_application::DnsQueryApplication;
use infiltrator_application::dns_query_fixtures::{IsolatedQueries, QueryFixtureMode};
use infiltrator_bevy_widgets::button::ButtonDisabled;
use infiltrator_bevy_widgets::localization::LocalizedLabel;
use infiltrator_contract::dns_query::DnsRecordType;
use infiltrator_contract::parity::FeatureId;
use std::sync::Arc;

#[derive(Resource)]
struct QueryCapture {
    application: DnsQueryApplication,
    port: Arc<IsolatedQueries>,
    stage: u8,
}
pub fn install(app: &mut App) {
    let port = Arc::new(IsolatedQueries::default());
    port.set_mode(QueryFixtureMode::ShortAnswer);
    let application = DnsQueryApplication::new(Some(port.clone()));
    app.insert_resource(CommandSinkHandle(Arc::new(CaptureCommandSink::new(
        CommandApplication::new().with_dns_query(application.clone()),
        CaptureCapability::DnsQuery,
    ))));
    app.insert_resource(QueryCapture {
        application,
        port,
        stage: 0,
    });
}
#[derive(SystemParam)]
pub struct QueryActivation<'w, 's> {
    feature: Res<'w, InteractionCapture>,
    route: Res<'w, ActiveRoute>,
    capture: ResMut<'w, QueryCapture>,
    panel: Res<'w, QueryPanel>,
    latest: ResMut<'w, LatestSurfaceSnapshot>,
    actions: Query<'w, 's, (Entity, &'static QueryAction, &'static ButtonDisabled)>,
    fields: Query<'w, 's, Entity, With<QueryNameField>>,
    windows: Query<'w, 's, (Entity, &'static Window), With<PrimaryWindow>>,
}
pub fn activate(mut probe: QueryActivation, mut geometry: CaptureGeometry, mut commands: Commands) {
    if !selected(
        &probe.feature,
        &probe.route,
        FeatureId::DnsQueryDetails,
        Route::Dns,
    ) {
        return;
    }
    let wanted = match probe.capture.stage {
        0 => QueryAction::Open,
        1 if probe.panel.model.open => {
            let Ok((window, _)) = probe.windows.single() else {
                return;
            };
            let Some(field) = probe.fields.iter().next() else {
                return;
            };
            if geometry
                .bounds(field, "DNS query native name field")
                .is_none()
            {
                return;
            }
            commands.write_message(KeyboardInput {
                key_code: KeyCode::KeyM,
                logical_key: Key::Character("music.test".into()),
                state: ButtonState::Pressed,
                text: Some("music.test".into()),
                repeat: false,
                window,
            });
            probe.capture.stage = 2;
            return;
        }
        2 if probe.panel.model.name == "music.test" => QueryAction::TypePicker,
        3 if probe.panel.kind_open => QueryAction::Type(DnsRecordType::Txt),
        4 if probe.panel.model.record_type == DnsRecordType::Txt => QueryAction::Run,
        5 => {
            if probe.panel.model.pending.is_none() {
                probe.latest.0.dns_query = probe.capture.application.snapshot();
            }
            return;
        }
        _ => return,
    };
    let Some((entity, _, _)) = probe
        .actions
        .iter()
        .find(|(_, action, disabled)| **action == wanted && !disabled.0)
    else {
        return;
    };
    let Some([x, y, width, height]) = geometry.bounds(entity, "DNS query activation control")
    else {
        return;
    };
    let Ok((_, window)) = probe.windows.single() else {
        return;
    };
    if x < 0.0 || y < 0.0 || x + width > window.width() || y + height > window.height() {
        return;
    }
    commands.trigger(Activate { entity });
    probe.capture.stage += 1;
}
#[derive(SystemParam)]
pub struct QueryObservation<'w, 's> {
    capture: Res<'w, QueryCapture>,
    panel: Res<'w, QueryPanel>,
    cards: Query<'w, 's, Entity, With<QueryModalCard>>,
    fields: Query<'w, 's, Entity, With<QueryNameField>>,
    actions: Query<
        'w,
        's,
        (
            Entity,
            &'static QueryAction,
            &'static ButtonDisabled,
            &'static LocalizedLabel,
        ),
    >,
    lines: Query<'w, 's, (Entity, &'static QueryLine, &'static Text)>,
}
pub fn observe(
    probe: QueryObservation,
    mut geometry: CaptureGeometry,
    mut feature: ResMut<InteractionCapture>,
    mut observation: ResMut<ObservedInteraction>,
) {
    let bounds = (|| {
        let model = &probe.panel.model;
        if probe.capture.stage != 5
            || probe.capture.port.requests().len() != 1
            || !model.open
            || model.pending.is_some()
            || model.failure.is_some()
            || model.requested != model.snapshot.report_id
            || model.requested.is_none()
            || model.snapshot.report.as_ref()?.response.answers.len() != 2
        {
            return None;
        }
        for wanted in [
            QueryAction::Run,
            QueryAction::Cancel,
            QueryAction::Section(QuerySection::Answer),
            QueryAction::Section(QuerySection::Authority),
            QueryAction::Section(QuerySection::Additional),
        ] {
            let (entity, _, _, _) = probe
                .actions
                .iter()
                .find(|(_, action, disabled, _)| **action == wanted && !disabled.0)?;
            geometry.bounds(entity, "DNS query result action")?;
        }
        for (entity, line, text) in &probe.lines {
            if matches!(line, QueryLine::Records) && !text.0.contains("observed-0 {ttl}") {
                return None;
            }
            geometry.bounds(entity, "DNS query observed response copy")?;
        }
        geometry.bounds(probe.fields.iter().next()?, "DNS query observed name field")?;
        geometry.bounds(probe.cards.iter().next()?, "DNS query details modal")
    })();
    observation.publish(&mut feature, bounds);
}
