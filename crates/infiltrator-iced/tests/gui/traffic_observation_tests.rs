//! test-intent: behavior
//! Real telemetry reader receipts feed the native Runtime/Overview adapter; private streams cannot override them.
use crate::state::AppState;
use crate::types::message::Message;
use crate::view::runtime::traffic_legend::legend_indicator;
use iced::advanced::layout::{Layout, Limits};
use iced::advanced::widget::{Id, Operation, Tree};
use iced::{Color, Rectangle, Size};
use infiltrator_application::core_application::CoreApplication;
use infiltrator_application::log_capture_fixtures::LogCaptureProcess;
use infiltrator_application::surface_reader::ApplicationSurfaceReader;
use infiltrator_application::telemetry_observation_fixtures::TelemetryObservationReader;
use infiltrator_composition::tokio_application_runtime;
use infiltrator_contract::command::CommandIntent;
use infiltrator_contract::surface::{HostKind, SurfaceKind};
use infiltrator_domain::runtime::TrafficData;
use infiltrator_ports::surface::SurfaceReader;
use std::sync::{Arc, atomic::Ordering};
use tokio::runtime::Builder;
fn run<T>(future: impl Future<Output = T>) -> T {
    Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(future)
}

#[derive(Default)]
struct CopyProbe(Vec<String>);
impl Operation for CopyProbe {
    fn traverse(&mut self, visit: &mut dyn FnMut(&mut dyn Operation)) {
        visit(self);
    }
    fn text(&mut self, _: Option<&Id>, _: Rectangle, text: &str) {
        // Empty renderer traverses native copy without font geometry; L3 measures actual GPU frames.
        assert!(!text.is_empty());
        self.0.push(text.to_owned());
    }
}
fn native_rate(state: &AppState) -> Vec<String> {
    let rates = state.traffic_readout();
    let mut element =
        legend_indicator::<()>(Color::BLACK, "Upload", &rates.upload, rates.upload_peak);
    let mut tree = Tree::new(element.as_widget());
    let node = element.as_widget_mut().layout(
        &mut tree,
        &(),
        &Limits::new(Size::ZERO, Size::new(1000.0, 200.0)),
    );
    let mut probe = CopyProbe::default();
    element
        .as_widget_mut()
        .operate(&mut tree, Layout::new(&node), &(), &mut probe);
    probe.0
}
#[test]
fn composed_rates_unknown_zero_failed_stale_and_new_session_are_distinct_and_unscoped_streams_cannot_replace_them()
 {
    let process = Arc::new(LogCaptureProcess::default());
    let telemetry = Arc::new(TelemetryObservationReader::default());
    let core = CoreApplication::new_with_overview(
        process.clone(),
        process,
        telemetry.clone(),
        tokio_application_runtime().unwrap(),
    );
    run(core.execute(CommandIntent::StartCore))
        .into_unit()
        .unwrap();
    let reader = ApplicationSurfaceReader::new(
        Arc::new(core.clone()),
        SurfaceKind::IcedDesktop,
        HostKind::Desktop,
    );
    let (mut state, _) = AppState::new();
    state.commands = Some(core.clone());
    state.shell.lang = "en-US".into();
    let first = run(reader.read()).unwrap();
    let _ = state.update(Message::SurfaceSnapshotUpdated(Box::new(first)));
    assert!(state.diag.traffic.is_none());
    assert_eq!(state.traffic_readout().upload, "Not observed");
    assert!(
        native_rate(&state)
            .iter()
            .any(|copy| copy == "Not observed")
    );
    assert!(state.traffic_readout().upload_peak.is_none());
    let zero = run(reader.read()).unwrap();
    let _ = state.update(Message::SurfaceSnapshotUpdated(Box::new(zero)));
    assert_eq!(state.diag.traffic.as_ref().unwrap().up, 0);
    assert_eq!(state.traffic_readout().upload, "0 B/s");
    assert!(native_rate(&state).iter().any(|copy| copy == "0 B/s"));
    assert!(state.traffic_readout().current);
    let _ = state.update(Message::TrafficReceived(TrafficData {
        up: 99999,
        down: 99999,
    }));
    assert_eq!(state.traffic_readout().upload, "0 B/s");
    telemetry.denied.store(true, Ordering::Release);
    let failed = run(reader.read()).unwrap();
    let _ = state.update(Message::SurfaceSnapshotUpdated(Box::new(failed)));
    let failed = state.traffic_readout();
    assert_eq!(failed.upload, "0 B/s (stale)");
    assert!(!failed.current && failed.failure.contains("isolated telemetry read denied"));
    assert!(
        native_rate(&state)
            .iter()
            .any(|copy| copy == "0 B/s (stale)")
    );
    state.shell.lang = "zh-CN".into();
    assert_eq!(state.traffic_readout().upload, "0 B/s（已失效）");
    assert!(
        native_rate(&state)
            .iter()
            .any(|copy| copy == "0 B/s（已失效）")
    );
    telemetry.denied.store(false, Ordering::Release);
    run(core.execute(CommandIntent::RestartCore))
        .into_unit()
        .unwrap();
    let new = run(reader.read()).unwrap();
    let _ = state.update(Message::SurfaceSnapshotUpdated(Box::new(new)));
    assert!(state.diag.traffic.is_none());
    assert!(state.runtime.traffic_waveform.samples.is_empty());
    assert!(!state.traffic_readout().observed);
    run(core.close()).unwrap();
}
