//! Capture opens the real editor, previews migration, then observes an actual denied write.
use crate::state::AppState;
use crate::types::message::Message;
use iced::Task;
use infiltrator_application::command_application::CommandApplication;
use infiltrator_application::configuration_application::ConfigurationApplication;
use infiltrator_application::core_application::CoreApplication;
use infiltrator_application::dns_hosts_fixtures::{HostsCaptureStore, ORIGINAL_HOSTS};
use infiltrator_application::language_choice_fixtures::LanguageCaptureProcess;
use infiltrator_composition::tokio_application_runtime;
use infiltrator_contract::error::Failure;
use infiltrator_contract::surface::{HostKind, SurfaceKind};
use infiltrator_contract::surface_snapshot::{PageData, SurfaceOrigin, SurfaceSnapshot};
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};

pub(super) fn activate(state: &mut AppState) -> Task<Message> {
    let store = Arc::new(HostsCaptureStore::default());
    store.deny_save.store(true, Ordering::SeqCst);
    let configuration = ConfigurationApplication::new(store.clone());
    let read = configuration.clone();
    let runtime = tokio_application_runtime().expect("isolated capture runtime");
    let observed = Arc::new(Mutex::new(None));
    let result = observed.clone();
    runtime.block_on(Box::pin(async move {
        *result.lock().expect("capture observation") = Some(read.load_hosts_profile().await);
    }));
    let hosts = observed
        .lock()
        .expect("capture observation")
        .take()
        .expect("read completed");
    let core = CoreApplication::new(
        Arc::new(LanguageCaptureProcess),
        Arc::new(LanguageCaptureProcess),
        runtime,
    );
    core.install_command_handler(Arc::new(
        CommandApplication::new().with_configuration(configuration),
    ));
    state.commands = Some(core);
    let mut snapshot = state.surface.latest().cloned().unwrap_or_else(|| {
        SurfaceSnapshot::unavailable(
            SurfaceKind::IcedDesktop,
            HostKind::Desktop,
            Failure::unsupported("unrelated capture services"),
        )
    });
    snapshot.origin = SurfaceOrigin::Demo;
    snapshot.revision += 1;
    snapshot.dns_hosts = PageData::ready(hosts.expect("actual initial Hosts read"));
    state.apply_shared_surface_snapshot(snapshot);
    assert_eq!(state.update(Message::OpenDnsHostsEditor).units(), 0);
    assert_eq!(state.update(Message::ImportLegacyDnsHosts).units(), 0);
    state.update(Message::SaveDnsHosts).map(move |message| {
        assert_eq!(store.writes.load(Ordering::SeqCst), 1);
        assert_eq!(store.content(), ORIGINAL_HOSTS);
        message
    })
}
