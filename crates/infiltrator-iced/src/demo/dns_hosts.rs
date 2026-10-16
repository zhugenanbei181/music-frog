//! Capture opens the real editor, previews migration, then observes an actual denied write.
use infiltrator_composition::mihomo_configuration::for_mihomo_store;
use crate::demo::owner::attach_capture_owner_with_snapshot;
use crate::state::AppState;
use crate::types::message::Message;
use iced::Task;
use infiltrator_application::command_application::CommandApplication;
use infiltrator_application::core_application::CoreApplication;
use infiltrator_composition::tokio_application_runtime;
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::surface::{HostKind, SurfaceKind};
use infiltrator_contract::surface_snapshot::{PageData, SurfaceSnapshot};
use infiltrator_test_support::dns_hosts_fixtures::{HostsCaptureStore, ORIGINAL_HOSTS};
use infiltrator_test_support::language_choice_fixtures::LanguageCaptureProcess;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};

pub(super) fn activate(state: &mut AppState) -> Task<Message> {
    let store = Arc::new(HostsCaptureStore::default());
    store.deny_save.store(true, Ordering::SeqCst);
    let configuration =
        for_mihomo_store(store.clone());
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
    let mut snapshot = SurfaceSnapshot::unavailable(
        SurfaceKind::IcedDesktop,
        HostKind::Desktop,
        Failure::new(ErrorCode::NotReady, "DNS hosts capture is preparing", true),
    );
    snapshot.dns_hosts = PageData::ready(hosts.expect("actual initial Hosts read"));
    attach_capture_owner_with_snapshot(state, core, snapshot);
    assert_eq!(state.update(Message::OpenDnsHostsEditor).units(), 0);
    assert_eq!(state.update(Message::ImportLegacyDnsHosts).units(), 0);
    state.update(Message::SaveDnsHosts).map(move |message| {
        assert_eq!(store.writes.load(Ordering::SeqCst), 1);
        assert_eq!(store.content(), ORIGINAL_HOSTS);
        message
    })
}
