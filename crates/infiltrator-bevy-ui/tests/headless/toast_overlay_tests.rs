//! Headless tests for the shell toast overlay (DUAL-15-12): the shared dedup
//! policy, redaction before the queue, and the mounted widget stack.

use bevy::MinimalPlugins;
use bevy::app::App;
use bevy::asset::AssetPlugin;
use bevy::scene::ScenePlugin;
use bevy::ui::widget::Text;
use infiltrator_bevy_ui::app::ShellPlugin;
use infiltrator_bevy_ui::toast::{ShellToast, ToastPolicyGate};
use infiltrator_bevy_widgets::toast::{ToastContainer, ToastKind, ToastQueue};
use infiltrator_contract::toast::ToastPolicy;

fn mounted_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins((AssetPlugin::default(), ScenePlugin));
    app.add_plugins(ShellPlugin::new(
        infiltrator_contract::theme::ThemePreference::Fixed(
            infiltrator_contract::theme::ThemeSkin::Dark,
        ),
    ));
    app.update();
    app
}

fn toast_texts(app: &mut App) -> Vec<String> {
    let world = app.world_mut();
    let mut query = world.query::<&Text>();
    query
        .iter(world)
        .map(|text| text.0.clone())
        .collect::<Vec<_>>()
}

#[test]
fn identical_toasts_are_deduplicated_and_distinct_ones_stack() {
    let mut app = mounted_app();
    {
        let world = app.world_mut();
        world.write_message(ShellToast::danger("controller unreachable"));
        world.write_message(ShellToast::danger("controller unreachable"));
    }
    app.update();
    assert_eq!(app.world().resource::<ToastQueue>().items().len(), 1);

    {
        let world = app.world_mut();
        world.write_message(ShellToast::info("profile saved"));
    }
    app.update();
    assert_eq!(
        app.world().resource::<ToastQueue>().items().len(),
        2,
        "a distinct toast still stacks"
    );
    assert_eq!(
        app.world().resource::<ToastPolicyGate>().gate().policy(),
        ToastPolicy::default()
    );
}

#[test]
fn a_sensitive_toast_is_redacted_before_it_renders() {
    let mut app = mounted_app();
    {
        let world = app.world_mut();
        world.write_message(ShellToast::danger(
            "update failed: https://sub.example.com/d?token=tok1234",
        ));
    }
    app.update();
    app.update();

    let stored = app
        .world()
        .resource::<ToastQueue>()
        .items()
        .first()
        .expect("toast queued")
        .content
        .clone();
    assert!(
        stored.contains("token=***"),
        "stored text redacted: {stored}"
    );
    assert!(!stored.contains("tok1234"));

    let rendered = toast_texts(&mut app).join(" | ");
    assert!(
        rendered.contains("token=***"),
        "rendered overlay carries the redacted text: {rendered}"
    );
    assert!(!rendered.contains("tok1234"), "raw token never renders");
    assert_eq!(app.world().resource::<ToastQueue>().items().len(), 1);
}

#[test]
fn the_overlay_mounts_a_single_stack_root() {
    let mut app = mounted_app();
    assert_eq!(
        app.world_mut()
            .query::<&ToastContainer>()
            .iter(app.world())
            .count(),
        0,
        "an empty queue mounts no overlay"
    );

    {
        let world = app.world_mut();
        world.write_message(ShellToast {
            kind: ToastKind::Warning,
            text: "shortcut conflict: Ctrl+Alt+T".to_string(),
            duration_secs: 5.0,
        });
    }
    app.update();
    app.update();
    assert_eq!(
        app.world_mut()
            .query::<&ToastContainer>()
            .iter(app.world())
            .count(),
        1,
        "exactly one overlay root while a toast is live"
    );
}

#[test]
fn capacity_evicts_the_oldest_toast() {
    let mut app = mounted_app();
    {
        let world = app.world_mut();
        for index in 0..5 {
            world.write_message(ShellToast::info(format!("message {index}")));
        }
    }
    app.update();
    let queue = app.world().resource::<ToastQueue>();
    assert_eq!(queue.items().len(), ToastPolicy::default().max_visible);
    assert_eq!(
        queue.items().last().map(|toast| toast.content.clone()),
        Some("message 4".to_string()),
        "the newest toast survives"
    );
    assert_eq!(
        queue.items().first().map(|toast| toast.content.clone()),
        Some("message 2".to_string()),
        "the two oldest were evicted"
    );
}
