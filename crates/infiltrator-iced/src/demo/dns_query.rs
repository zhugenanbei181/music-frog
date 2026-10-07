//! Capture submits a real shared query and reads the same owner's published response.
use crate::state::AppState;
use crate::types::dns_query::QueryAction;
use crate::types::message::Message;
use iced::Task;
use infiltrator_application::command_application::CommandApplication;
use infiltrator_application::core_application::CoreApplication;
use infiltrator_application::dns_query_application::DnsQueryApplication;
use infiltrator_application::dns_query_fixtures::{IsolatedQueries, QueryFixtureMode};
use infiltrator_application::language_choice_fixtures::LanguageCaptureProcess;
use infiltrator_application::surface_reader::ApplicationSurfaceReader;
use infiltrator_composition::tokio_application_runtime;
use infiltrator_contract::dns_query::DnsRecordType;
use infiltrator_contract::surface::{HostKind, SurfaceKind};
use infiltrator_contract::surface_snapshot::SurfaceOrigin;
use infiltrator_ports::surface::SurfaceReader;
use std::sync::Arc;

pub(super) fn activate(state: &mut AppState) -> Task<Message> {
    let port = Arc::new(IsolatedQueries::default());
    port.set_mode(QueryFixtureMode::ShortAnswer);
    let owner = DnsQueryApplication::new(Some(port.clone()));
    let core = CoreApplication::new(
        Arc::new(LanguageCaptureProcess),
        Arc::new(LanguageCaptureProcess),
        tokio_application_runtime().expect("isolated query capture runtime"),
    );
    core.install_command_handler(Arc::new(
        CommandApplication::new().with_dns_query(owner.clone()),
    ));
    let reader = Arc::new(
        ApplicationSurfaceReader::new(
            Arc::new(core.clone()),
            SurfaceKind::IcedDesktop,
            HostKind::Desktop,
        )
        .with_dns_query(owner),
    );
    state.commands = Some(core);
    let _ = state.update(Message::DnsQuery(QueryAction::Open));
    let _ = state.update(Message::DnsQuery(QueryAction::Name("music.test".into())));
    let _ = state.update(Message::DnsQuery(QueryAction::RecordType(
        DnsRecordType::Txt,
    )));
    let revision = state.surface.revision() + 1;
    state
        .update(Message::DnsQuery(QueryAction::Run))
        .then(move |message| {
            let reader = reader.clone();
            let port = port.clone();
            Task::done(message).chain(Task::perform(
                async move {
                    let mut snapshot = reader.read().await.expect("actual query capture report");
                    assert_eq!(port.requests().len(), 1);
                    snapshot.origin = SurfaceOrigin::Demo;
                    snapshot.revision = revision;
                    snapshot
                },
                |snapshot| Message::SurfaceSnapshotUpdated(Box::new(snapshot)),
            ))
        })
}
