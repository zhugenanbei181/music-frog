//! The connections page performs its own grouping, search, drawer and confirmation.
use crate::state::AppState;
use crate::types::app::ConfirmAction;
use crate::types::message::Message;
use iced::Task;
use infiltrator_composition::capture_fixtures::grouping_surface;
use infiltrator_contract::parity::FeatureId;
use infiltrator_contract::surface::{HostKind, SurfaceKind};
use infiltrator_contract::surface_snapshot::SurfaceSnapshot;
use infiltrator_domain::connection_view::ConnectionGroupingMode;
use infiltrator_test_support::connection_grouping_fixtures::grouping_snapshot;
use lurker_core_contract::error::Failure;

pub(super) fn activate(state: &mut AppState, feature: FeatureId) -> Task<Message> {
    if matches!(
        feature,
        FeatureId::ConnectionsGrouping | FeatureId::ConnectionsSearchHighlight
    ) {
        let snapshot = grouping_surface(state.surface.latest().cloned().unwrap_or_else(|| {
            SurfaceSnapshot::unavailable(
                SurfaceKind::IcedDesktop,
                HostKind::Desktop,
                Failure::unsupported("unrelated capture services"),
            )
        }));
        let _ = state.update(Message::SurfaceSnapshotUpdated(Box::new(snapshot)));
        let _ = state.update(Message::ConnectionsReceived(grouping_snapshot()));
        let search = feature == FeatureId::ConnectionsSearchHighlight;
        let _ = state.update(Message::SetConnectionGroupingMode(if search {
            ConnectionGroupingMode::Flat
        } else {
            ConnectionGroupingMode::ByProcess
        }));
        let _ = state.update(Message::UpdateRuntimeConnectionFilter(
            if search { "API-0" } else { "CLIENT-0" }.into(),
        ));
        return Task::none();
    }
    let message = match feature {
        FeatureId::ConnectionsCloseAllConfirm => {
            Message::RequestConfirmation(ConfirmAction::CloseAllConnections)
        }
        FeatureId::ConnectionsDetailsDrawer => Message::InspectConnection(Some(
            state
                .diag
                .connections
                .as_ref()
                .and_then(|s| s.connections.first())
                .expect("connection inspection fixture")
                .id
                .clone(),
        )),
        _ => return Task::none(),
    };
    assert_eq!(
        state.update(message).units(),
        0,
        "opening an inspection cannot schedule effects"
    );
    Task::none()
}
