//! test-intent: behavior
use crate::support::headless_plugins;
use bevy::app::App;
use bevy::camera::NormalizedRenderTarget;
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::Children;
use bevy::input::ButtonState;
use bevy::input::keyboard::{Key, KeyCode, KeyboardInput};
use bevy::math::Vec2;
use bevy::picking::backend::HitData;
use bevy::picking::events::{Pointer, PointerPress};
use bevy::picking::pointer::{Location, PointerButton, PointerId};
use bevy::text::TextColor;
use bevy::ui::widget::Text;
use infiltrator_application::dns_health_projection::project_dns_health;
use infiltrator_application::dns_latency_projection::project_dns_latency;
use infiltrator_application::dns_mapping_projection::project_mappings;
use infiltrator_application::stun_projection::project_stun;
use infiltrator_bevy_ui::app::ShellPlugin;
use infiltrator_bevy_ui::pages::dns::{DnsLine, DnsLineKind, DnsProjection, DnsProjectionUpdated};
use infiltrator_bevy_ui::pages::dns_fakeip::DnsFakeIpSearchField;
use infiltrator_bevy_ui::route::{PagesPlugin, Route, RouteChanged};
use infiltrator_bevy_widgets::localization::UiLocale;
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::text_input::{TextField, TextFieldFocused};
use infiltrator_contract::dns::{FakeIpMappingEntry, FakeIpMappingPool, FakeIpMappingSource};
use infiltrator_contract::dns_cache::DnsCacheFlushReport;
use infiltrator_contract::dns_latency::{
    DnsLatencyReport, DnsProbeOutcome, DnsProbeTransport, DnsServerLatency,
};
use infiltrator_contract::dns_leak::DnsLeakReport;
use infiltrator_contract::dns_self_heal::{
    DnsSelfHealCheck, DnsSelfHealKind, DnsSelfHealSnapshot, DnsSelfHealState,
};
use infiltrator_contract::surface_snapshot::DnsPageSnapshot;

fn setup() -> (App, DnsProjection) {
    let mut app = App::new();
    headless_plugins(&mut app);
    app.add_plugins((ShellPlugin::default(), PagesPlugin::default()));
    app.update();
    app.world_mut().trigger(RouteChanged(Route::Dns));
    app.update();
    let mut projection = DnsProjection::from_snapshot(
        &DnsPageSnapshot::default(),
        &DnsLeakReport::default(),
        None,
        &DnsCacheFlushReport::default(),
    );
    projection.fake_ip_pool = FakeIpMappingPool {
        source: FakeIpMappingSource::LiveConnections,
        range: "198.18.0.1/16".into(),
        total: 2,
        entries: vec![
            FakeIpMappingEntry {
                address: "198.18.0.5".into(),
                domain: "music.example".into(),
            },
            FakeIpMappingEntry {
                address: "198.18.0.7".into(),
                domain: "cdn.example".into(),
            },
        ],
    };
    projection.latency = DnsLatencyReport::measured(
        "example.org",
        vec![DnsServerLatency {
            address: "1.1.1.1".into(),
            is_fallback: false,
            transport: DnsProbeTransport::Udp,
            outcome: DnsProbeOutcome::TimedOut,
        }],
    );
    projection.self_heal = DnsSelfHealSnapshot::new(vec![DnsSelfHealCheck {
        kind: DnsSelfHealKind::ListenPort,
        state: DnsSelfHealState::Healthy,
        detail: "observed port {fix}".into(),
        fix: None,
    }]);
    app.world_mut()
        .trigger(DnsProjectionUpdated(projection.clone()));
    app.update();
    (app, projection)
}
fn line(app: &mut App, kind: DnsLineKind) -> (Entity, String) {
    app.world_mut()
        .query::<(Entity, &Text, &DnsLine)>()
        .iter(app.world())
        .find(|(_, _, line)| line.0 == kind)
        .map(|(entity, text, _)| (entity, text.0.clone()))
        .expect("mounted native DNS observation line")
}
fn search(app: &mut App) -> Entity {
    let children = app
        .world_mut()
        .query::<(&Children, &DnsFakeIpSearchField)>()
        .iter(app.world())
        .next()
        .unwrap()
        .0
        .iter()
        .copied()
        .collect::<Vec<_>>();
    children
        .iter()
        .copied()
        .find(|entity| app.world().get::<TextField>(*entity).is_some())
        .unwrap()
}
fn focus(app: &mut App, entity: Entity) {
    app.world_mut().trigger(PointerPress {
        entity,
        pointer: Pointer::new(
            PointerId::Mouse,
            Location {
                target: NormalizedRenderTarget::None {
                    width: 1180,
                    height: 780,
                },
                position: Vec2::new(20.0, 20.0),
            },
        ),
        button: PointerButton::Primary,
        hit: HitData::new(Entity::PLACEHOLDER, 0.0, None, None),
        count: 1,
    });
    app.update();
}
fn key(app: &mut App, logical_key: Key, value: Option<&str>) {
    app.world_mut().write_message(KeyboardInput {
        key_code: KeyCode::KeyC,
        logical_key,
        state: ButtonState::Pressed,
        text: value.map(Into::into),
        repeat: false,
        window: Entity::PLACEHOLDER,
    });
    app.update();
}
#[test]
fn native_fake_ip_search_and_locale_replay_preserve_the_field_and_actual_observation_states() {
    let (mut app, projection) = setup();
    let field = search(&mut app);
    let listing = line(&mut app, DnsLineKind::FakeIpMapping).0;
    let counter = line(&mut app, DnsLineKind::FakeIpMappingCount).0;
    let health = line(&mut app, DnsLineKind::SelfHealOverall).0;
    let policy = line(&mut app, DnsLineKind::LatencyPolicy).0;
    let stun = line(&mut app, DnsLineKind::StunConclusion).0;
    focus(&mut app, field);
    assert!(app.world().get::<TextFieldFocused>(field).unwrap().0);
    key(&mut app, Key::Character("cdn".into()), Some("cdn"));
    assert_eq!(app.world().get::<TextField>(field).unwrap().0.text(), "cdn");
    assert_eq!(
        app.world().get::<Text>(listing).unwrap().0,
        "198.18.0.7 ↔ cdn.example"
    );
    for code in ["en-US", "zh-CN"] {
        app.insert_resource(UiLocale::new(code));
        app.update();
        let mappings = project_mappings(&projection.fake_ip_pool, "cdn", code);
        assert_eq!(
            line(&mut app, DnsLineKind::FakeIpMappingCount),
            (counter, mappings.count)
        );
        let display = project_dns_health(&projection.self_heal, code);
        assert_eq!(
            line(&mut app, DnsLineKind::SelfHealOverall),
            (health, display.overall)
        );
        assert_eq!(
            app.world().get::<TextColor>(health).unwrap().0,
            app.world().resource::<UiPalette>().ink_dim
        );
        assert_eq!(
            line(&mut app, DnsLineKind::LatencyPolicy),
            (
                policy,
                project_dns_latency(&projection.latency, code).summary
            )
        );
        assert_eq!(
            app.world().get::<TextColor>(policy).unwrap().0,
            app.world().resource::<UiPalette>().danger
        );
        assert_eq!(
            line(&mut app, DnsLineKind::StunConclusion),
            (stun, project_stun(&projection.stun, code).status)
        );
        assert_eq!(search(&mut app), field);
        assert!(app.world().get::<TextFieldFocused>(field).unwrap().0);
        assert_eq!(app.world().get::<TextField>(field).unwrap().0.text(), "cdn");
    }
    key(&mut app, Key::Backspace, None);
    key(&mut app, Key::Backspace, None);
    key(&mut app, Key::Backspace, None);
    assert_eq!(app.world().get::<TextField>(field).unwrap().0.text(), "");
    assert_eq!(
        app.world().get::<Text>(listing).unwrap().0.lines().count(),
        2
    );
    key(&mut app, Key::Character("missing".into()), Some("missing"));
    let code = app.world().resource::<UiLocale>().code().to_string();
    assert_eq!(
        app.world().get::<Text>(listing).unwrap().0,
        project_mappings(&projection.fake_ip_pool, "missing", &code).empty
    );
    let mut changed = projection.clone();
    changed.fake_ip_pool.source = FakeIpMappingSource::Unavailable {
        reason: "read failure {total}".into(),
    };
    app.world_mut()
        .trigger(DnsProjectionUpdated(changed.clone()));
    app.update();
    assert_eq!(
        app.world().get::<Text>(listing).unwrap().0,
        project_mappings(&changed.fake_ip_pool, "missing", &code).empty
    );
    assert_eq!(
        app.world().get::<TextField>(field).unwrap().0.text(),
        "missing"
    );
    app.world_mut().trigger(DnsProjectionUpdated(projection));
    app.update();
    assert_eq!(search(&mut app), field);
    assert_eq!(
        app.world().get::<TextField>(field).unwrap().0.text(),
        "missing"
    );
}
