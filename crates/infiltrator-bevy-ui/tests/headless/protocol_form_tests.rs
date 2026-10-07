//! test-intent: behavior
use crate::command_harness::{recording_application, rejecting_application};
use crate::support::headless_plugins;
use bevy::app::App;
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::Children;
use bevy::ui::widget::Text;
use bevy::ui::{Display, Node};
use bevy::ui_widgets::{Activate, Button};
use infiltrator_application::command_application::CommandApplication;
use infiltrator_application::protocol_codec_application::clear_studio;
use infiltrator_bevy_ui::app::ShellPlugin;
use infiltrator_bevy_ui::command::{
    CommandPumpPlugin, CommandSinkHandle, DemoCommandSink, UiCommand,
};
use infiltrator_bevy_ui::command_events::CommandExecutedEvent;
use infiltrator_bevy_ui::command_execution::ApplicationCommandSink;
use infiltrator_bevy_ui::pages::business_panel::{
    BusinessPanelRoot, BusinessPanelState, CloseBusinessPanel, OpenBusinessPanel, PanelKind,
};
use infiltrator_bevy_ui::pages::proxies::{ProxiesProjection, ProxiesProjectionUpdated};
use infiltrator_bevy_ui::pages::proxies_custom::SaveCustomNodeButton;
use infiltrator_bevy_ui::pages::proxies_custom::{CustomNodeUriField, ImportUriButton};
use infiltrator_bevy_ui::pages::proxies_form::{
    CustomNodeForm, CustomNodeFormError, ProtocolFieldNode,
};
use infiltrator_bevy_ui::route::{PagesPlugin, Route, RouteChanged};
use infiltrator_bevy_widgets::responsive::TouchHitbox;
use infiltrator_bevy_widgets::text_input::TextField;
use infiltrator_bevy_widgets::text_input::state::TextFieldInput;
use infiltrator_contract::command::{CommandIntent, RequestId};
use infiltrator_contract::command_output::CommandOutput;
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::protocol_form::ProtocolField;
use std::sync::Arc;
use std::time::Duration;
use tokio::runtime::Builder;
use tokio::task::yield_now;
use tokio::time::timeout;

fn setup() -> (App, Arc<DemoCommandSink>) {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = App::new();
    headless_plugins(&mut app);
    app.add_plugins((
        ShellPlugin::default(),
        PagesPlugin::demo(),
        CommandPumpPlugin::new(sink.clone()),
    ));
    app.update();
    app.world_mut()
        .commands()
        .trigger(RouteChanged(Route::Proxies));
    app.update();
    let opener = app
        .world_mut()
        .query::<(Entity, &OpenBusinessPanel)>()
        .iter(app.world())
        .find(|(_, open)| open.0 == PanelKind::CustomNode)
        .map(|(e, _)| e)
        .unwrap();
    app.world_mut()
        .commands()
        .trigger(Activate { entity: opener });
    app.update();
    (app, sink)
}
fn field(app: &mut App, id: ProtocolField) -> Entity {
    let wrapper = app
        .world_mut()
        .query::<(Entity, &ProtocolFieldNode)>()
        .iter(app.world())
        .find(|(_, f)| f.0 == id)
        .map(|(e, _)| e)
        .unwrap();
    let mut stack = vec![wrapper];
    while let Some(entity) = stack.pop() {
        if app.world().get::<TextField>(entity).is_some() {
            return entity;
        }
        if let Some(children) = app.world().get::<Children>(entity) {
            stack.extend(children.iter());
        }
    }
    panic!("native field missing: {id:?}");
}
fn edit(app: &mut App, id: ProtocolField, value: &str) {
    let entity = field(app, id);
    app.world_mut()
        .get_mut::<TextField>(entity)
        .unwrap()
        .0
        .apply(TextFieldInput::SetText(value.into()));
    app.update();
}
fn save(app: &mut App) {
    let entity = app
        .world_mut()
        .query::<(Entity, &SaveCustomNodeButton)>()
        .single(app.world())
        .unwrap()
        .0;
    app.world_mut().commands().trigger(Activate { entity });
    app.update();
}
fn complete(app: &mut App, command: UiCommand, success: bool) {
    let request_id = app
        .world()
        .resource::<CustomNodeForm>()
        .saving
        .as_ref()
        .unwrap()
        .request_id;
    app.world_mut().commands().trigger(CommandExecutedEvent {
        command,
        request_id,
        result: if success {
            Ok(CommandOutput::Unit)
        } else {
            Err(Failure::new(
                ErrorCode::InvalidInput,
                "profile is read-only",
                false,
            ))
        },
    });
    app.update();
}

#[test]
fn full_protocol_form_has_native_fields_and_preserves_invalid_input() {
    let (mut app, sink) = setup();
    assert_eq!(
        app.world().resource::<BusinessPanelState>().0,
        Some(PanelKind::CustomNode)
    );
    edit(&mut app, ProtocolField::Name, "local-vless");
    edit(&mut app, ProtocolField::Server, "node.example.com");
    edit(&mut app, ProtocolField::Port, "70000");
    edit(
        &mut app,
        ProtocolField::Secret,
        "b831381d-6324-4d53-ad4f-8cda48b30811",
    );
    save(&mut app);
    assert!(sink.submitted().is_empty());
    let port_field = field(&mut app, ProtocolField::Port);
    assert_eq!(
        app.world().get::<TextField>(port_field).unwrap().0.text(),
        "70000"
    );
    assert!(
        app.world()
            .resource::<CustomNodeForm>()
            .inputs
            .errors
            .contains_key(&ProtocolField::Port)
    );
    edit(&mut app, ProtocolField::Port, "443");
    edit(&mut app, ProtocolField::Network, "ws");
    edit(&mut app, ProtocolField::WsPath, "/proxy");
    save(&mut app);
    let command = sink.submitted().last().cloned().unwrap();
    assert!(
        matches!(&command, UiCommand::SaveCustomNodeDraft { draft } if draft.name == "local-vless" && draft.port == 443 && draft.params.transport.ws.path == "/proxy")
    );
    save(&mut app);
    assert_eq!(
        sink.submitted().len(),
        1,
        "in-flight save cannot be replayed"
    );
    complete(&mut app, command.clone(), false);
    assert_eq!(
        app.world().resource::<BusinessPanelState>().0,
        Some(PanelKind::CustomNode)
    );
    assert_eq!(
        app.world()
            .resource::<CustomNodeForm>()
            .studio
            .draft
            .as_ref()
            .unwrap()
            .server,
        "node.example.com"
    );
    let error = app
        .world_mut()
        .query::<(&Text, &CustomNodeFormError)>()
        .single(app.world())
        .unwrap()
        .0;
    assert_eq!(error.0, "profile is read-only");
    save(&mut app);
    complete(&mut app, command, true);
    assert_eq!(app.world().resource::<BusinessPanelState>().0, None);
    let root = app
        .world_mut()
        .query::<(&BusinessPanelRoot, &Node)>()
        .iter(app.world())
        .find(|(root, _)| root.0 == PanelKind::CustomNode)
        .unwrap()
        .1;
    assert_eq!(root.display, Display::None);
}

#[test]
fn protocol_form_cancel_blocks_residual_actions_and_switches_family_fields() {
    let (mut app, sink) = setup();
    edit(&mut app, ProtocolField::Type, "wireguard");
    let wg = app
        .world_mut()
        .query::<(&ProtocolFieldNode, &Node)>()
        .iter(app.world())
        .find(|(id, _)| id.0 == ProtocolField::WgPrivate)
        .unwrap()
        .1;
    assert_eq!(wg.display, Display::Flex);
    let tuic = app
        .world_mut()
        .query::<(&ProtocolFieldNode, &Node)>()
        .iter(app.world())
        .find(|(id, _)| id.0 == ProtocolField::TuicTimeout)
        .unwrap()
        .1;
    assert_eq!(tuic.display, Display::None);
    edit(
        &mut app,
        ProtocolField::WgPrivate,
        "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=",
    );
    let cancel = app
        .world_mut()
        .query::<(Entity, &CloseBusinessPanel)>()
        .iter(app.world())
        .find(|(_, close)| close.0 == PanelKind::CustomNode)
        .map(|(e, _)| e)
        .unwrap();
    app.world_mut()
        .commands()
        .trigger(Activate { entity: cancel });
    app.update();
    save(&mut app);
    assert!(sink.submitted().is_empty());
    assert_eq!(app.world().resource::<BusinessPanelState>().0, None);
}

#[test]
fn production_command_sink_delivers_correlated_terminal_save_result() {
    let (mut app, _) = setup();
    let (application, handler) = recording_application();
    // Install the production adapter, retaining the isolated native scene.
    app.insert_resource(CommandSinkHandle(Arc::new(ApplicationCommandSink::new(
        Arc::new(application),
    ))));
    edit(&mut app, ProtocolField::Name, "saved-node");
    edit(&mut app, ProtocolField::Server, "node.example.com");
    edit(&mut app, ProtocolField::Port, "443");
    edit(
        &mut app,
        ProtocolField::Secret,
        "b831381d-6324-4d53-ad4f-8cda48b30811",
    );
    save(&mut app);
    let runtime = Builder::new_current_thread().enable_all().build().unwrap();
    runtime.block_on(async {
        timeout(Duration::from_secs(5), async {
            loop {
                app.update();
                if app.world().resource::<CustomNodeForm>().saving.is_none() {
                    break;
                }
                yield_now().await;
            }
        })
        .await
        .expect("terminal command feedback");
    });
    assert_eq!(app.world().resource::<BusinessPanelState>().0, None);
    let commands = handler.0.lock().unwrap();
    assert_eq!(commands.len(), 1);
    assert!(
        matches!(&commands[0], CommandIntent::SaveCustomNodeDraft { draft } if draft.name == "saved-node")
    );
}

#[test]
fn production_command_failure_keeps_native_form_and_draft_open() {
    let (mut app, _) = setup();
    let (application, handler) = rejecting_application();
    app.insert_resource(CommandSinkHandle(Arc::new(ApplicationCommandSink::new(
        Arc::new(application),
    ))));
    edit(&mut app, ProtocolField::Name, "failed-node");
    edit(&mut app, ProtocolField::Server, "node.example.com");
    edit(&mut app, ProtocolField::Port, "443");
    edit(
        &mut app,
        ProtocolField::Secret,
        "b831381d-6324-4d53-ad4f-8cda48b30811",
    );
    save(&mut app);
    Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            timeout(Duration::from_secs(5), async {
                loop {
                    app.update();
                    if app.world().resource::<CustomNodeForm>().saving.is_none() {
                        break;
                    }
                    yield_now().await;
                }
            })
            .await
            .expect("terminal rejection feedback");
        });
    assert_eq!(
        app.world().resource::<BusinessPanelState>().0,
        Some(PanelKind::CustomNode)
    );
    let form = app.world().resource::<CustomNodeForm>();
    assert_eq!(form.studio.draft.as_ref().unwrap().name, "failed-node");
    assert_eq!(
        form.studio.last_error.as_deref(),
        Some("profile is read-only")
    );
    assert_eq!(handler.0.lock().unwrap().len(), 1);
}

#[test]
fn uri_import_uses_real_codec_handler_and_error_cancel_keep_the_imported_draft() {
    clear_studio();
    let (mut app, _) = setup();
    let (application, _) = recording_application();
    application.install_command_handler(Arc::new(CommandApplication::default()));
    app.insert_resource(CommandSinkHandle(Arc::new(ApplicationCommandSink::new(
        Arc::new(application),
    ))));
    let wrapper = app
        .world_mut()
        .query::<(Entity, &CustomNodeUriField)>()
        .single(app.world())
        .unwrap()
        .0;
    let input = app
        .world()
        .get::<Children>(wrapper)
        .unwrap()
        .iter()
        .next()
        .unwrap()
        .to_owned();
    let button = app
        .world_mut()
        .query::<(Entity, &ImportUriButton)>()
        .single(app.world())
        .unwrap()
        .0;
    let runtime = Builder::new_current_thread().enable_all().build().unwrap();
    for uri in [
        "vless://b831381d-6324-4d53-ad4f-8cda48b30811@node.example.com:443?security=tls#Imported-Node",
        "broken://",
    ] {
        app.world_mut()
            .get_mut::<TextField>(input)
            .unwrap()
            .0
            .apply(TextFieldInput::SetText(uri.into()));
        app.world_mut()
            .commands()
            .trigger(Activate { entity: button });
        app.update();
        runtime.block_on(async {
            timeout(Duration::from_secs(5), async {
                loop {
                    app.update();
                    if app.world().resource::<CustomNodeForm>().importing.is_none()
                        && !app.world().resource::<CustomNodeForm>().reset_fields
                    {
                        break;
                    }
                    yield_now().await;
                }
            })
            .await
            .expect("real import terminal result");
        });
        assert_eq!(
            app.world()
                .resource::<CustomNodeForm>()
                .studio
                .draft
                .as_ref()
                .unwrap()
                .name,
            "Imported-Node"
        );
    }
    let form = app.world().resource::<CustomNodeForm>();
    assert!(form.studio.last_error.is_some());
    assert!(
        form.studio
            .uri_preview
            .as_ref()
            .unwrap()
            .starts_with("vless://")
    );
    let cancel = app
        .world_mut()
        .query::<(Entity, &CloseBusinessPanel)>()
        .iter(app.world())
        .find(|(_, close)| close.0 == PanelKind::CustomNode)
        .map(|(e, _)| e)
        .unwrap();
    app.world_mut()
        .commands()
        .trigger(Activate { entity: cancel });
    app.update();
    save(&mut app);
    assert_eq!(app.world().resource::<BusinessPanelState>().0, None);
    assert!(app.world().resource::<CustomNodeForm>().saving.is_none());
}

#[test]
fn unrelated_projection_refresh_cannot_erase_the_open_native_draft() {
    let (mut app, _) = setup();
    edit(&mut app, ProtocolField::Name, "editing-node");
    app.world_mut()
        .commands()
        .trigger(ProxiesProjectionUpdated(ProxiesProjection {
            name_runs: Default::default(),
            search_query: String::new(),
            groups: Vec::new(),
            testing: false,
            filter_alive: false,
            compact_view: false,
            active_exit: "—".into(),
            custom_node: Default::default(),
        }));
    app.update();
    assert_eq!(
        app.world()
            .resource::<CustomNodeForm>()
            .studio
            .draft
            .as_ref()
            .unwrap()
            .name,
        "editing-node"
    );
    let native = field(&mut app, ProtocolField::Name);
    assert_eq!(
        app.world().get::<TextField>(native).unwrap().0.text(),
        "editing-node"
    );
}

#[test]
fn import_reply_from_cancelled_session_cannot_complete_the_same_uri_in_a_new_session() {
    let (mut app, _) = setup();
    let uri = "vless://same@example.com:443#same";
    let wrapper = app
        .world_mut()
        .query::<(Entity, &CustomNodeUriField)>()
        .single(app.world())
        .unwrap()
        .0;
    let input = app
        .world()
        .get::<Children>(wrapper)
        .unwrap()
        .iter()
        .next()
        .unwrap()
        .to_owned();
    let button = app
        .world_mut()
        .query::<(Entity, &ImportUriButton)>()
        .single(app.world())
        .unwrap()
        .0;
    app.world_mut()
        .get_mut::<TextField>(input)
        .unwrap()
        .0
        .apply(TextFieldInput::SetText(uri.into()));
    app.world_mut()
        .commands()
        .trigger(Activate { entity: button });
    app.update();
    let old_id = app
        .world()
        .resource::<CustomNodeForm>()
        .importing
        .as_ref()
        .unwrap()
        .request_id;
    let cancel = app
        .world_mut()
        .query::<(Entity, &CloseBusinessPanel)>()
        .iter(app.world())
        .find(|(_, close)| close.0 == PanelKind::CustomNode)
        .map(|(e, _)| e)
        .unwrap();
    app.world_mut()
        .commands()
        .trigger(Activate { entity: cancel });
    app.update();
    let opener = app
        .world_mut()
        .query::<(Entity, &OpenBusinessPanel)>()
        .iter(app.world())
        .find(|(_, open)| open.0 == PanelKind::CustomNode)
        .map(|(e, _)| e)
        .unwrap();
    app.world_mut()
        .commands()
        .trigger(Activate { entity: opener });
    app.update();
    assert_eq!(app.world().get::<TextField>(input).unwrap().0.text(), "");
    app.world_mut()
        .get_mut::<TextField>(input)
        .unwrap()
        .0
        .apply(TextFieldInput::SetText(uri.into()));
    app.world_mut()
        .commands()
        .trigger(Activate { entity: button });
    app.update();
    let current_id = app
        .world()
        .resource::<CustomNodeForm>()
        .importing
        .as_ref()
        .unwrap()
        .request_id;
    assert_ne!(old_id, current_id);
    app.world_mut().commands().trigger(CommandExecutedEvent {
        request_id: old_id,
        command: UiCommand::ImportCustomNodeUri { uri: uri.into() },
        result: Err(Failure::new(ErrorCode::NotReady, "stale failure", true)),
    });
    app.update();
    let form = app.world().resource::<CustomNodeForm>();
    assert_eq!(form.importing.as_ref().unwrap().request_id, current_id);
    assert_eq!(form.studio.last_error, None);
    assert_eq!(form.studio.draft.as_ref().unwrap().name, "");
    // An unrelated request also cannot finish a current save.
    assert_ne!(current_id, RequestId(0));
}

#[test]
fn native_buttons_receive_required_hitboxes_before_the_first_frame() {
    let (mut app, _) = setup();
    let entity = app.world_mut().spawn((Button, Node::default())).id();
    assert!(app.world().get::<TouchHitbox>(entity).is_some());
    // Retiring a newly-created control leaves no delayed attachment command behind.
    app.world_mut().entity_mut(entity).despawn();
    app.update();
}
