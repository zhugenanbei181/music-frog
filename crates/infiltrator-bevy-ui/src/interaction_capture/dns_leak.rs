//! Native probe activation and report geometry are backed by an isolated echo port.
use super::geometry::CaptureGeometry;
use super::host::{CaptureCapability, CaptureCommandSink};
use super::scroll::request_scroll;
use super::{InteractionCapture, ObservedInteraction, selected};
use crate::command::CommandSinkHandle;
use crate::pages::dns::{DnsPageRoot, DnsProjectionUpdated, TestDnsLeakButton};
use crate::pages::dns_leak::LeakCard;
use crate::pages::dns_leak_actions::LeakActions;
use crate::pages::dns_leak_rows::{LeakRowIdentity, LeakRowLabel};
use crate::route::{ActiveRoute, Route};
use crate::surface::{LatestSurfaceSnapshot, dns_projection};
use bevy::app::App;
use bevy::app::Update;
use bevy::ecs::entity::Entity;
use bevy::ecs::query::With;
use bevy::ecs::resource::Resource;
use bevy::ecs::schedule::IntoScheduleConfigs;
use bevy::ecs::system::{Commands, Query, Res, ResMut, SystemParam};
use bevy::ui_widgets::Activate;
use infiltrator_application::command_application::CommandApplication;
use infiltrator_application::dns_leak_application::{DnsLeakApplication, default_echo_sources};
use infiltrator_application::dns_leak_fixtures::IsolatedEcho;
use infiltrator_bevy_widgets::button::ButtonDisabled;
use infiltrator_contract::dns_leak::DnsLeakOperation;
use infiltrator_contract::parity::FeatureId;
use std::sync::Arc;
#[derive(Resource)]
struct LeakCapture {
    application: DnsLeakApplication,
    echo: Arc<IsolatedEcho>,
    started: bool,
    initialized: bool,
}
pub fn install(app: &mut App) {
    let echo = Arc::new(IsolatedEcho::default());
    let application = DnsLeakApplication::new(Some(echo.clone()), default_echo_sources());
    app.insert_resource(CommandSinkHandle(Arc::new(CaptureCommandSink::new(
        CommandApplication::new().with_dns_leak(application.clone()),
        CaptureCapability::DnsLeak,
    ))));
    app.insert_resource(LeakCapture {
        application,
        echo,
        started: false,
        initialized: false,
    });
    app.add_systems(Update, initialize.before(activate));
}
fn initialize(mut capture: ResMut<LeakCapture>, mut latest: ResMut<LatestSurfaceSnapshot>) {
    if capture.initialized {
        return;
    }
    latest.0.dns_leak = capture.application.last_report();
    capture.initialized = true;
}
#[derive(SystemParam)]
pub struct ProbeActivation<'w, 's> {
    feature: Res<'w, InteractionCapture>,
    route: Res<'w, ActiveRoute>,
    capture: ResMut<'w, LeakCapture>,
    actions: Res<'w, LeakActions>,
    latest: ResMut<'w, LatestSurfaceSnapshot>,
    buttons: Query<'w, 's, (Entity, &'static ButtonDisabled), With<TestDnsLeakButton>>,
    cards: Query<'w, 's, Entity, With<LeakCard>>,
    scroll: Query<'w, 's, Entity, With<DnsPageRoot>>,
}
pub fn activate(mut probe: ProbeActivation, geometry: CaptureGeometry, mut commands: Commands) {
    if !selected(
        &probe.feature,
        &probe.route,
        FeatureId::DnsLeakAlert,
        Route::Dns,
    ) {
        return;
    }
    if !probe.capture.started {
        let Some((entity, _)) = probe.buttons.iter().find(|(_, disabled)| !disabled.0) else {
            return;
        };
        probe.capture.started = true;
        commands.trigger(Activate { entity });
        return;
    }
    if probe.actions.state.pending.is_some() {
        return;
    }
    let report = probe.capture.application.last_report();
    if probe.latest.0.dns_leak != report {
        probe.latest.0.dns_leak = report;
        commands.trigger(DnsProjectionUpdated(dns_projection(&probe.latest.0)));
    }
    if let (Some(card), Some(scroll)) = (probe.cards.iter().next(), probe.scroll.iter().next())
        && let (Some(card), Some(viewport)) = (geometry.rect(card), geometry.rect(scroll))
    {
        request_scroll(&mut commands, scroll, card, viewport);
    }
}
#[derive(SystemParam)]
pub struct ProbeObservation<'w, 's> {
    capture: Res<'w, LeakCapture>,
    actions: Res<'w, LeakActions>,
    latest: Res<'w, LatestSurfaceSnapshot>,
    rows: Query<'w, 's, Entity, With<LeakRowIdentity>>,
    labels: Query<'w, 's, Entity, With<LeakRowLabel>>,
    buttons: Query<'w, 's, Entity, With<TestDnsLeakButton>>,
    cards: Query<'w, 's, Entity, With<LeakCard>>,
}
impl ProbeObservation<'_, '_> {
    fn bounds(&self, geometry: &mut CaptureGeometry) -> Option<[f32; 4]> {
        if !self.capture.started
            || self.capture.echo.requests().len() != 1
            || self.actions.state.pending.is_some()
            || self.actions.state.failure.is_some()
        {
            return None;
        }
        let report = &self.latest.0.dns_leak;
        if report.operation != DnsLeakOperation::Completed
            || !report.conclusion().is_divergent()
            || report.observations.len() != 2
            || self.rows.iter().count() != 2
        {
            return None;
        }
        for row in self.rows.iter() {
            geometry.bounds(row, "DNS echo observation")?;
        }
        for label in self.labels.iter() {
            geometry.bounds(label, "DNS echo outcome")?;
        }
        geometry.bounds(self.buttons.iter().next()?, "DNS echo probe control")?;
        geometry.bounds(self.cards.iter().next()?, "DNS echo report")
    }
}
pub fn observe(
    probe: ProbeObservation,
    mut geometry: CaptureGeometry,
    mut state: ResMut<InteractionCapture>,
    mut observation: ResMut<ObservedInteraction>,
) {
    observation.publish(&mut state, probe.bounds(&mut geometry));
}
