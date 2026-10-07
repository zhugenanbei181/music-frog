//! test-intent: behavior
//! Native lifecycle and locale replay preserve nested captions, accessibility and active native input.
use super::*;
use crate::native_input::{click_entity, type_text};
use bevy::window::{Ime, PrimaryWindow, Window};
use infiltrator_bevy_widgets::localization::UiLocale;
use infiltrator_bevy_widgets::stat_chip::StatChipLabel;
use infiltrator_bevy_widgets::text_input::TextFieldFocused;

fn caption(app: &mut App, kind: OverviewChipKind) -> (Entity, String) {
    let world = app.world_mut();
    let root = world
        .query::<(Entity, &OverviewChip)>()
        .iter(world)
        .find(|(_, chip)| chip.0 == kind)
        .unwrap()
        .0;
    let mut pending = vec![root];
    while let Some(entity) = pending.pop() {
        if world.get::<StatChipLabel>(entity).is_some() {
            return (entity, world.get::<Text>(entity).unwrap().0.clone());
        }
        if let Some(children) = world.get::<Children>(entity) {
            pending.extend(children.iter());
        }
    }
    panic!("native stat caption absent");
}

#[test]
fn locale_replays_nested_labels_values_and_a11y_without_replacing_active_url_input_or_ime() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = mounted_default();
    app.add_plugins(CommandPumpPlugin::new(sink.clone()));
    let window = app
        .world_mut()
        .spawn((Window::default(), PrimaryWindow))
        .id();
    app.insert_resource(UiLocale::new("en-US"));
    app.update();
    let ids = overview_entity_ids(app.world_mut());
    let label = caption(&mut app, OverviewChipKind::Memory);
    assert_eq!(label.1, "Memory");
    assert_eq!(line(app.world_mut(), OverviewLineKind::State).1, "Running");
    assert_eq!(line(app.world_mut(), OverviewLineKind::ModeChip).1, "Rule");
    assert_eq!(
        line(app.world_mut(), OverviewLineKind::BannerNote).1,
        "Demo data · No live core"
    );
    let field = native_url_field(app.world_mut());
    click_entity(&mut app, field);
    let url = "https://probe.example.test/204";
    type_text(&mut app, url);
    assert_eq!(app.world().get::<TextField>(field).unwrap().0.text(), url);
    app.world_mut().write_message(Ime::Preedit {
        window,
        value: "ni hao".into(),
        cursor: Some((0, 2)),
    });
    app.update();
    assert_eq!(
        app.world().get::<TextField>(field).unwrap().0.preedit(),
        "ni hao"
    );
    app.insert_resource(UiLocale::new("zh-CN"));
    app.update();
    assert_eq!(
        caption(&mut app, OverviewChipKind::Memory),
        (label.0, "内存".into())
    );
    assert_eq!(line(app.world_mut(), OverviewLineKind::State).1, "运行中");
    assert_eq!(line(app.world_mut(), OverviewLineKind::ModeChip).1, "规则");
    assert_eq!(overview_entity_ids(app.world_mut()), ids);
    assert!(app.world().get::<TextFieldFocused>(field).unwrap().0);
    assert_eq!(app.world().get::<TextField>(field).unwrap().0.text(), url);
    assert_eq!(
        app.world().get::<TextField>(field).unwrap().0.preedit(),
        "ni hao"
    );
    let memory = app
        .world_mut()
        .query::<(&OverviewChip, &AccessibilityNode)>()
        .iter(app.world())
        .find(|(chip, _)| chip.0 == OverviewChipKind::Memory)
        .unwrap()
        .1
        .label()
        .unwrap()
        .to_owned();
    assert_eq!(memory, "内存 96.00 MB");
    app.insert_resource(UiLocale::new("en-US"));
    app.update();
    assert_eq!(caption(&mut app, OverviewChipKind::Memory), label);
    assert_eq!(line(app.world_mut(), OverviewLineKind::State).1, "Running");
    assert_eq!(
        app.world().get::<TextField>(field).unwrap().0.preedit(),
        "ni hao"
    );
    assert!(
        sink.submitted().is_empty(),
        "locale and native editing never submit a measurement"
    );
}

struct LifecycleSource(CoreLifecycle);
impl OverviewSource for LifecycleSource {
    fn current(&self) -> OverviewProjection {
        let mut projection = DemoOverviewSource::running().current();
        projection.lifecycle = self.0.clone();
        projection.state = match self.0 {
            CoreLifecycle::Running | CoreLifecycle::Ready => OverviewState::Running,
            CoreLifecycle::Stopped => OverviewState::Stopped,
            _ => OverviewState::Unavailable,
        };
        projection.origin = OverviewOrigin::LiveCore;
        projection.failure = None;
        projection.core_version = None;
        projection
    }
}
#[test]
fn pending_lifecycles_are_visible_without_fabricated_errors_and_their_ink_survives_theme_replay() {
    for (lifecycle, expected) in [
        (CoreLifecycle::Starting, "Starting..."),
        (CoreLifecycle::Stopping, "Stopping…"),
    ] {
        let mut app = mounted_app_with(LifecycleSource(lifecycle.clone()));
        app.insert_resource(UiLocale::new("en-US"));
        app.update();
        let ids = overview_entity_ids(app.world_mut());
        assert_eq!(line(app.world_mut(), OverviewLineKind::State).1, expected);
        assert_eq!(line(app.world_mut(), OverviewLineKind::Failure).1, "");
        assert_eq!(
            line(app.world_mut(), OverviewLineKind::BannerNote).1,
            "Live core · Version not observed"
        );
        assert_eq!(card(app.world_mut()).2, lifecycle);
        app.world_mut().trigger(ThemeSwitch(ThemeSkin::Light));
        app.update();
        let palette = UiPalette::new(&Theme::light());
        assert_eq!(
            line(app.world_mut(), OverviewLineKind::State).2.0,
            palette.warning
        );
        assert_eq!(card(app.world_mut()).1, palette.accent_container);
        let dot = app
            .world_mut()
            .query::<(&StatusDot, &BackgroundColor)>()
            .single(app.world())
            .unwrap()
            .1
            .0;
        assert_eq!(
            dot, palette.warning,
            "a pending core never shows a green activity indicator"
        );
        assert_eq!(line(app.world_mut(), OverviewLineKind::Failure).1, "");
        assert_eq!(overview_entity_ids(app.world_mut()), ids);
    }
}
