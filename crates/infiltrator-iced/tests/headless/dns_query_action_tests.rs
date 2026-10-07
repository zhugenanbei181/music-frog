//! test-intent: behavior
use crate::command_harness::recording_application;
use crate::test_support::demo_env;
use futures_util::StreamExt;
use iced::Task;
use iced::advanced::widget::Tree;
use iced_runtime::{Action, task::into_stream};
use infiltrator_application::command_application::CommandApplication;
use infiltrator_application::dns_query_actions::QuerySection;
use infiltrator_application::dns_query_application::DnsQueryApplication;
use infiltrator_application::dns_query_fixtures::{IsolatedQueries, QueryFixtureMode};
use infiltrator_application::dns_query_projection::project_query;
use infiltrator_application::surface_reader::ApplicationSurfaceReader;
use infiltrator_contract::dns_query::DnsRecordType;
use infiltrator_contract::error::ErrorCode;
use infiltrator_contract::surface::{HostKind, SurfaceKind};
use infiltrator_iced::state::AppState;
use infiltrator_iced::types::app::Route;
use infiltrator_iced::types::dns_query::QueryAction;
use infiltrator_iced::types::message::Message;
use infiltrator_ports::surface::SurfaceReader;
use std::sync::Arc;
use tokio::runtime::Builder;

fn terminal(task: Task<Message>) -> Message {
    Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            let mut stream = into_stream(task).expect("actual DNS query command task");
            let Some(Action::Output(message)) = stream.next().await else {
                panic!("DNS query terminal result");
            };
            assert!(stream.next().await.is_none());
            message
        })
}
fn setup(port: Arc<IsolatedQueries>) -> (AppState, ApplicationSurfaceReader) {
    let owner = DnsQueryApplication::new(Some(port));
    let (application, _) = recording_application();
    application.install_command_handler(Arc::new(
        CommandApplication::new().with_dns_query(owner.clone()),
    ));
    let reader = ApplicationSurfaceReader::new(
        Arc::new(application.clone()),
        SurfaceKind::IcedDesktop,
        HostKind::Desktop,
    )
    .with_dns_query(owner);
    let (mut state, _) = AppState::demo(&demo_env(Route::Dns));
    state.commands = Some(application);
    state.shell.demo = false;
    publish(&mut state, &reader);
    (state, reader)
}
fn publish(state: &mut AppState, reader: &ApplicationSurfaceReader) {
    let mut snapshot = Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(reader.read())
        .unwrap();
    snapshot.revision = state.surface.revision() + 1;
    assert!(snapshot.pages.dns.data.is_none());
    assert!(state.apply_shared_surface_snapshot(snapshot));
}
fn send(state: &mut AppState, action: QueryAction) -> Task<Message> {
    state.update(Message::DnsQuery(action))
}
#[test]
fn actual_query_modal_cancel_tabs_pages_stale_terminals_permission_retry_and_guidance_keep_owner_facts()
 {
    let port = Arc::new(IsolatedQueries::default());
    let (mut state, reader) = setup(port.clone());
    let _ = send(&mut state, QueryAction::Open);
    let mut tree = Tree::new(state.view().as_widget());
    let _ = send(&mut state, QueryAction::Name("music.test".into()));
    let _ = send(&mut state, QueryAction::RecordType(DnsRecordType::Txt));
    let _ = send(&mut state, QueryAction::Cancel);
    assert!(!state.diag.dns_query.open);
    assert!(port.requests().is_empty());
    assert_eq!(send(&mut state, QueryAction::Run).units(), 0);
    let _ = send(&mut state, QueryAction::Open);
    let _ = state.update(Message::Navigate(Route::Settings));
    assert!(!state.diag.dns_query.open);
    assert!(port.requests().is_empty());
    let _ = state.update(Message::Navigate(Route::Dns));
    let _ = send(&mut state, QueryAction::Open);
    let task = send(&mut state, QueryAction::Run);
    let token = state.diag.dns_query.pending.unwrap();
    assert_eq!(send(&mut state, QueryAction::Run).units(), 0);
    let _ = send(&mut state, QueryAction::Name("cannot-change.test".into()));
    assert_eq!(state.diag.dns_query.name, "music.test");
    let _ = send(
        &mut state,
        QueryAction::Finished {
            token: token + 1,
            result: Ok(()),
        },
    );
    assert_eq!(state.diag.dns_query.pending, Some(token));
    let _ = state.update(terminal(task));
    publish(&mut state, &reader);
    assert_eq!(port.requests().len(), 1);
    assert_eq!(port.requests()[0].record_type, DnsRecordType::Txt);
    assert!(
        project_query(&state.diag.dns_query, "en-US")
            .records
            .contains("observed-0 {ttl}")
    );
    let _ = send(&mut state, QueryAction::Next);
    let _ = send(&mut state, QueryAction::Next);
    assert!(
        project_query(&state.diag.dns_query, "en-US")
            .records
            .contains("observed-17 {ttl}")
    );
    let _ = send(&mut state, QueryAction::Section(QuerySection::Authority));
    assert!(
        project_query(&state.diag.dns_query, "en-US")
            .records
            .contains("ns.test.")
    );
    let _ = send(&mut state, QueryAction::Section(QuerySection::Additional));
    assert!(
        project_query(&state.diag.dns_query, "en-US")
            .records
            .contains("TTL 0s")
    );
    let previous = state.diag.dns_query.snapshot.report.clone();
    port.set_mode(QueryFixtureMode::Permission);
    let task = send(&mut state, QueryAction::Run);
    let _ = state.update(terminal(task));
    publish(&mut state, &reader);
    assert_eq!(
        state.diag.dns_query.failure.as_ref().unwrap().code,
        ErrorCode::Permission
    );
    assert_eq!(state.diag.dns_query.snapshot.report, previous);
    assert!(project_query(&state.diag.dns_query, "en-US").guide);
    for code in ["en-US", "zh-CN"] {
        state.shell.lang = code.into();
        tree.diff(state.view().as_widget());
        assert_eq!(state.diag.dns_query.name, "music.test");
        assert!(
            project_query(&state.diag.dns_query, code)
                .status
                .ends_with("Allow controller DNS query access {reason}")
        );
    }
    port.set_mode(QueryFixtureMode::Answer);
    let task = send(&mut state, QueryAction::Retry);
    let _ = state.update(terminal(task));
    publish(&mut state, &reader);
    assert!(state.diag.dns_query.failure.is_none());
    assert_eq!(port.requests().len(), 3);
    assert!(
        project_query(&state.diag.dns_query, "en-US")
            .records
            .contains("observed-0 {ttl}")
    );
    port.set_mode(QueryFixtureMode::Authentication);
    let task = send(&mut state, QueryAction::Run);
    let _ = state.update(terminal(task));
    publish(&mut state, &reader);
    assert!(state.diag.dns_query.can_guide());
    let _ = state.update(Message::Navigate(Route::Settings));
    assert!(!state.diag.dns_query.open);
    assert_eq!(port.requests().len(), 4);
}
#[test]
fn invalid_query_and_unsupported_host_cannot_invent_records_while_negative_answers_keep_authority()
{
    let port = Arc::new(IsolatedQueries::default());
    let (mut state, reader) = setup(port.clone());
    let _ = send(&mut state, QueryAction::Open);
    let _ = send(&mut state, QueryAction::Name("bad..name".into()));
    assert_eq!(send(&mut state, QueryAction::Run).units(), 0);
    assert_eq!(
        state.diag.dns_query.failure.as_ref().unwrap().code,
        ErrorCode::InvalidInput
    );
    assert!(port.requests().is_empty());
    let _ = send(&mut state, QueryAction::Name("music.test".into()));
    port.set_mode(QueryFixtureMode::Unsupported);
    let task = send(&mut state, QueryAction::Run);
    let _ = state.update(terminal(task));
    publish(&mut state, &reader);
    assert_eq!(
        state.diag.dns_query.failure.as_ref().unwrap().code,
        ErrorCode::Unsupported
    );
    assert!(state.diag.dns_query.snapshot.report.is_none());
    assert!(!state.diag.dns_query.can_retry());
    port.set_mode(QueryFixtureMode::NegativeAnswer);
    let task = send(&mut state, QueryAction::Run);
    let _ = state.update(terminal(task));
    publish(&mut state, &reader);
    assert_eq!(
        state
            .diag
            .dns_query
            .snapshot
            .report
            .as_ref()
            .unwrap()
            .response
            .status,
        3
    );
    assert!(state.diag.dns_query.rows().is_empty());
    let _ = send(&mut state, QueryAction::Section(QuerySection::Authority));
    assert_eq!(state.diag.dns_query.rows()[0].data, "ns.test.");
}
