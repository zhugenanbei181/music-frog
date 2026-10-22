//! The rules page really refreshes a rule provider and really fails.
//!
//! The capture publishes a provider inventory that carries a live controller
//! reference, presses the provider's own refresh control and lets the owner's
//! typed refusal reach the surface's error banner. No failure is invented on
//! the surface: the banner carries the owner's own answer.
use crate::demo::capabilities::{
    DemoMihomoCaptureCore, MIHOMO_CAPTURE_ENGINE, install_demo_core_capabilities,
};
use crate::demo::owner::attach_capture_owner;
use crate::state::AppState;
use crate::types::message::Message;
use iced::Task;
use infiltrator_application::command_application::CommandApplication;
use infiltrator_application::core_application::CoreApplication;
use infiltrator_application::rule_provider_runtime_application::RuleProviderRuntimeApplication;
use infiltrator_composition::mihomo_configuration::{
    for_mihomo_store, mihomo_rule_trace_fixture, profiles_for_mihomo,
};
use infiltrator_composition::surface_reader::ProductSurfaceReader;
use infiltrator_composition::tokio_application_runtime;
use infiltrator_contract::core_capabilities::{CoreOperation, ProviderOperation};
use infiltrator_contract::parity::FeatureId;
use infiltrator_contract::surface::{HostKind, SurfaceKind};
use infiltrator_ports::surface::SurfaceReader;
use infiltrator_test_support::rule_provider_fixtures::RuleProviderCapturePort;
use lurker_core_contract::page::SurfaceOrigin;
use std::sync::Arc;

/// The reason the capture's provider source is offline.
///
/// The owner refuses the refresh with this reason, so the surface's banner
/// carries the owner's own words.
pub(super) const CAPTURE_REFRESH_FAILURE: &str = "the capture rule provider source is offline";

pub(super) fn activate(state: &mut AppState) -> Task<Message> {
    let store = mihomo_rule_trace_fixture();
    let port = Arc::new(RuleProviderCapturePort::default());
    port.fail_refresh(CAPTURE_REFRESH_FAILURE);
    // The capture core speaks the Mihomo engine: the provider inventory and
    // the rule workspace belong to the Mihomo owner, and a refresh for
    // another engine is refused by the owner itself.
    let core = CoreApplication::new(
        Arc::new(DemoMihomoCaptureCore),
        Arc::new(DemoMihomoCaptureCore),
        tokio_application_runtime().expect("isolated rule-provider capture runtime"),
    );
    // The refresh is a write on the provider owner, so the capture publishes
    // the capability matrix that admits exactly that operation. The inventory
    // read is admitted as well: the surface renders the owner's provider
    // list, and the controller reference that list carries is what the
    // refresh control submits.
    let capabilities = install_demo_core_capabilities(
        &core,
        MIHOMO_CAPTURE_ENGINE,
        [
            CoreOperation::Provider(ProviderOperation::ListRule),
            CoreOperation::Provider(ProviderOperation::UpdateRule),
        ],
    );
    let runtime = RuleProviderRuntimeApplication::new(port.clone(), capabilities.clone());
    core.install_command_handler(Arc::new(
        CommandApplication::new()
            .with_core_capabilities(capabilities)
            .with_rule_provider_runtime(runtime.clone())
            .with_profile(profiles_for_mihomo(store.clone()))
            .with_configuration(for_mihomo_store(store.clone())),
    ));
    let reader = ProductSurfaceReader::new(
        Arc::new(core.clone()),
        SurfaceKind::IcedDesktop,
        HostKind::Desktop,
    )
    .with_rule_provider_runtime(runtime)
    .with_profiles(profiles_for_mihomo(store.clone()))
    .with_configuration(for_mihomo_store(store));
    attach_capture_owner(state, core);
    let revision = state.surface.revision() + 1;
    Task::perform(
        async move {
            let mut snapshot = reader
                .read()
                .await
                .expect("actual isolated rule-provider inventory");
            snapshot.origin = SurfaceOrigin::Demo;
            snapshot.revision = revision;
            snapshot
        },
        |snapshot| Message::SurfaceSnapshotUpdated(Box::new(snapshot)),
    )
}

/// The follow-up of the refresh gesture: the owner published the provider
/// inventory, so the surface now has the controller reference its own refresh
/// control submits.
pub(super) fn update(state: &mut AppState, message: Message) -> Task<Message> {
    let observed = matches!(&message, Message::SurfaceSnapshotUpdated(_));
    let task = state.update(message);
    if observed
        && state.shell.capture_scenario == Some(FeatureId::RulesProviderRefreshError)
        && let Some(reference) = state
            .editor
            .rule_providers
            .iter()
            .find_map(|provider| provider.controller.clone())
    {
        task.chain(Task::done(Message::UpdateRuleProvider(reference)))
    } else {
        task
    }
}
