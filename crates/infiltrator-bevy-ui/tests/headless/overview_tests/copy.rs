//! test-intent: behavior
//! Native lifecycle and locale replay preserve nested captions, accessibility and active native input.
use super::*;
use infiltrator_bevy_widgets::localization::UiLocale;
use infiltrator_bevy_widgets::stat_chip::StatChipLabel;

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
fn locale_replays_nested_labels_values_and_accessibility_without_submitting() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = mounted_default();
    app.add_plugins(CommandPumpPlugin::new(sink.clone()));
    app.insert_resource(UiLocale::new("en-US"));
    app.update();
    let ids = overview_entity_ids(app.world_mut());
    let label = caption(&mut app, OverviewChipKind::Memory);
    assert_eq!(label.1, "Memory");
    assert_eq!(line(app.world_mut(), OverviewLineKind::State).1, "Demo Mode");
    assert_eq!(
        line(app.world_mut(), OverviewLineKind::RoutingIntentChip).1,
        "Follow rules"
    );
    assert_eq!(
        line(app.world_mut(), OverviewLineKind::BannerNote).1,
        "Demo data · No live core"
    );
    app.insert_resource(UiLocale::new("zh-CN"));
    app.update();
    assert_eq!(
        caption(&mut app, OverviewChipKind::Memory),
        (label.0, "内存".into())
    );
    assert_eq!(line(app.world_mut(), OverviewLineKind::State).1, "演示模式");
    assert_eq!(
        line(app.world_mut(), OverviewLineKind::RoutingIntentChip).1,
        "遵循分流规则"
    );
    assert_eq!(overview_entity_ids(app.world_mut()), ids);
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
    assert_eq!(line(app.world_mut(), OverviewLineKind::State).1, "Demo Mode");
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

    fn kind(&self) -> SourceKind {
        SourceKind::LiveCore
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
