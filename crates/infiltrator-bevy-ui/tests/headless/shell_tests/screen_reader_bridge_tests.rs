//! BEVY-031: the shell routes semantic actions through the screen-reader bridge
//! seam. With no host bridge the routing is a no-op that reports the typed
//! `Unsupported` outcome; an installed bridge receives the action and the shell
//! reports `Dispatched`.

use bevy::MinimalPlugins;
use bevy::app::App;
use bevy::asset::AssetPlugin;
use bevy::ecs::message::Messages;
use bevy::scene::ScenePlugin;
use infiltrator_bevy_ui::a11y::{SemanticActionOutcome, SemanticActionRequest};
use infiltrator_bevy_ui::app::ShellPlugin;
use infiltrator_bevy_widgets::text_input::native::{
    ScreenReaderAction, ScreenReaderBridge, ScreenReaderCapability, ScreenReaderGate,
    ScreenReaderOutcome, SemanticNodeId,
};
use std::sync::{Arc, Mutex};

#[derive(Clone)]
struct RecordingBridge {
    capability: ScreenReaderCapability,
    actions: Arc<Mutex<Vec<ScreenReaderAction>>>,
}

impl ScreenReaderBridge for RecordingBridge {
    fn capability(&self) -> ScreenReaderCapability {
        self.capability
    }

    fn dispatch(&self, action: ScreenReaderAction) -> ScreenReaderOutcome {
        self.actions.lock().expect("recorder lock").push(action);
        ScreenReaderOutcome::Dispatched
    }
}

fn mounted_shell() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins((AssetPlugin::default(), ScenePlugin));
    app.add_plugins(ShellPlugin::default());
    app.update();
    app
}

fn outcomes(app: &mut App) -> Vec<SemanticActionOutcome> {
    app.world_mut()
        .resource_mut::<Messages<SemanticActionOutcome>>()
        .drain()
        .collect()
}

fn send(app: &mut App, action: ScreenReaderAction) {
    app.world_mut().write_message(SemanticActionRequest(action));
    app.update();
}

#[test]
fn with_no_host_bridge_routing_is_a_typed_unsupported_no_op() {
    let mut app = mounted_shell();
    assert!(!app.world().resource::<ScreenReaderGate>().is_supported());

    send(&mut app, ScreenReaderAction::Focus(SemanticNodeId::new(1)));
    assert_eq!(
        outcomes(&mut app),
        vec![SemanticActionOutcome(ScreenReaderOutcome::Unsupported)]
    );
}

#[test]
fn an_installed_bridge_receives_the_action_and_reports_dispatched() {
    let mut app = mounted_shell();
    let recorder = Arc::new(Mutex::new(Vec::new()));
    app.insert_resource(ScreenReaderGate::install(RecordingBridge {
        capability: ScreenReaderCapability::TalkBack,
        actions: Arc::clone(&recorder),
    }));
    assert_eq!(
        app.world().resource::<ScreenReaderGate>().capability(),
        ScreenReaderCapability::TalkBack
    );

    let node = SemanticNodeId::new(7);
    send(&mut app, ScreenReaderAction::Activate(node));
    assert_eq!(
        outcomes(&mut app),
        vec![SemanticActionOutcome(ScreenReaderOutcome::Dispatched)]
    );
    assert_eq!(
        *recorder.lock().expect("recorder lock"),
        vec![ScreenReaderAction::Activate(node)]
    );
}

#[test]
fn set_value_requests_carry_their_payload_through_the_gate() {
    let mut app = mounted_shell();
    let recorder = Arc::new(Mutex::new(Vec::new()));
    app.insert_resource(ScreenReaderGate::install(RecordingBridge {
        capability: ScreenReaderCapability::VoiceOver,
        actions: Arc::clone(&recorder),
    }));

    let node = SemanticNodeId::new(3);
    send(
        &mut app,
        ScreenReaderAction::SetValue {
            node,
            value: "1.2 MB/s".to_string(),
        },
    );
    assert_eq!(
        *recorder.lock().expect("recorder lock"),
        vec![ScreenReaderAction::SetValue {
            node,
            value: "1.2 MB/s".to_string(),
        }]
    );
}
