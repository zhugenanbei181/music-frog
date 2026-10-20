//! The proxies page performs its own fold, search, inspection and node gestures.
use crate::state::AppState;
use crate::types::message::Message;
use iced::Task;
use infiltrator_contract::parity::FeatureId;
use infiltrator_contract::runtime_settings::{RuntimeProbeSettings, RuntimeProbeSettingsState};

pub(super) fn activate(state: &mut AppState, feature: FeatureId) -> Task<Message> {
    match feature {
        FeatureId::ProxiesGroupReorder => {
            let group = state
                .runtime
                .proxy_groups
                .get(1)
                .expect("typed outbound fixture has reorderable groups")
                .id
                .clone();
            assert_eq!(
                state
                    .update(Message::MoveProxyGroupUp(
                        group.expect("typed group identity")
                    ))
                    .units(),
                0
            );
            Task::none()
        }
        FeatureId::ProxiesProbeSettings => {
            state.runtime.runtime_probe_settings_editor.observe(
                &RuntimeProbeSettingsState::Available {
                    settings: RuntimeProbeSettings::default(),
                },
            );
            assert_eq!(state.update(Message::OpenRuntimeProbeSettings).units(), 0);
            assert_eq!(
                state
                    .update(Message::UpdateRuntimeProbeUrlDraft(
                        "https://probe.example.test/204".into()
                    ))
                    .units(),
                0
            );
            assert_eq!(
                state
                    .update(Message::UpdateRuntimeProbeTimeoutDraft("60000".into()))
                    .units(),
                0
            );
            Task::none()
        }
        FeatureId::ProxiesSearchHighlight => {
            // The search only reaches the nodes of an open group, so the
            // capture opens the group and then types the query: both are the
            // user's own gestures on this page.
            open_first_group(state);
            assert_eq!(state.update(Message::FilterProxies("HK".into())).units(), 0);
            Task::none()
        }
        FeatureId::ProxiesGroupExpanded => {
            // The demo fixture folds the first group, so the capture performs
            // the fold gesture that opens it again.
            open_first_group(state);
            Task::none()
        }
        FeatureId::ProxiesNodeDetailDrawer => {
            let outbound_id = state
                .runtime
                .proxy_groups
                .iter()
                .flat_map(|group| &group.proxies)
                .find(|member| member.id.is_some())
                .and_then(|member| member.id.clone())
                .expect("typed outbound fixture has detail rows");
            assert_eq!(
                state
                    .update(Message::InspectOutbound(Some(outbound_id)))
                    .units(),
                0
            );
            Task::none()
        }
        FeatureId::ProxiesCustomNodeModal | FeatureId::ProxiesUriImportPreview => {
            assert_eq!(
                state.update(Message::OpenCustomNodeModal).units(),
                0,
                "opening an inspection cannot schedule effects"
            );
            if feature == FeatureId::ProxiesUriImportPreview {
                assert_eq!(state.update(Message::UpdateCustomNodeUriInput("vless://b831381d-6324-4d53-ad4f-8cda48b30811@node.example.com:443?security=tls#Imported-Node".into())).units(), 0);
                assert_eq!(state.update(Message::ParseAndImportCustomUri).units(), 0);
            }
            Task::none()
        }
        _ => Task::none(),
    }
}

/// Open the first proxy group of the demo fixture.
///
/// The fixture folds that group so the fold business has something to open;
/// every group-bound capture opens it before it measures the group's own
/// control.
fn open_first_group(state: &mut AppState) {
    let name = state
        .runtime
        .filtered_groups
        .first()
        .expect("typed demo collection")
        .0
        .clone();
    let group_id = state
        .runtime
        .proxy_groups
        .iter()
        .find(|candidate| candidate.name == name)
        .and_then(|candidate| candidate.id.clone())
        .expect("typed demo collection ID");
    // The expansion is the shared proxies model's fact, not a private copy.
    if !state.runtime.proxies_model.group_expanded(&group_id) {
        assert_eq!(
            state
                .update(Message::ToggleOutboundGroupExpanded(group_id))
                .units(),
            0
        );
    }
}
