//! test-intent: behavior
use super::*;
use infiltrator_bevy_ui::pages::dns_hosts::{DnsHostsDomainField, DnsHostsEditorState, HostAction};
fn action(app: &mut App, wanted: impl Fn(&HostAction) -> bool) -> Entity {
    app.world_mut()
        .query::<(Entity, &HostAction)>()
        .iter(app.world())
        .find(|(_, action)| wanted(action))
        .expect("native Hosts control")
        .0
}
fn click(app: &mut App, entity: Entity) {
    app.world_mut().commands().trigger(Activate { entity });
    app.update();
}
fn input(app: &mut App, address: bool, value: &str) {
    let wrapper = if address {
        app.world_mut()
            .query::<(Entity, &DnsHostsEditorField)>()
            .iter(app.world())
            .next()
            .unwrap()
            .0
    } else {
        app.world_mut()
            .query::<(Entity, &DnsHostsDomainField)>()
            .iter(app.world())
            .next()
            .unwrap()
            .0
    };
    let fields: Vec<_> = app
        .world()
        .get::<Children>(wrapper)
        .unwrap()
        .iter()
        .copied()
        .collect();
    let field = fields
        .into_iter()
        .find(|entity| app.world().get::<TextField>(*entity).is_some())
        .unwrap();
    app.world_mut()
        .get_mut::<TextField>(field)
        .unwrap()
        .0
        .apply(TextFieldInput::SetText(value.into()));
}
fn open(app: &mut App) {
    let button = action(app, |action| matches!(action, HostAction::Open));
    click(app, button);
}
#[test]
fn test_dns_hosts_editor_submits_shared_patch() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_b_app(Arc::clone(&sink));
    navigate_to(&mut app, Route::Dns);
    open(&mut app);
    let original = app
        .world()
        .resource::<DnsHostsEditorState>()
        .editor
        .applied
        .as_ref()
        .unwrap()
        .entries
        .clone();
    for address in ["1.1.1.1", "8.8.8.8"] {
        input(&mut app, true, address);
        input(&mut app, false, "new.test");
        let button = action(&mut app, |action| matches!(action, HostAction::CommitRow));
        click(&mut app, button);
    }
    let button = action(&mut app, |action| matches!(action, HostAction::Apply));
    click(&mut app, button);
    match sink.submitted().first() {
        Some(UiCommand::ApplyDnsSettings { patch }) => {
            let hosts = patch.hosts.as_ref().unwrap();
            assert_eq!(hosts.len(), original.len() + 2);
            assert_eq!(
                hosts.iter().filter(|row| row.domain == "new.test").count(),
                2
            );
            assert_eq!(patch.expected_hosts.as_ref(), Some(&original));
            assert_eq!(patch.expected_profile.as_deref(), Some("demo"));
            assert!(!patch.clear_hosts);
        }
        other => panic!("expected a profile-fenced Hosts patch: {other:?}"),
    }
    assert_eq!(
        app.world()
            .resource::<DnsHostsEditorState>()
            .editor
            .applied
            .as_ref()
            .unwrap()
            .entries,
        original,
        "queue acceptance cannot publish applied rows"
    );
}
#[test]
fn test_dns_hosts_editor_local_validation_blocks_bad_rows() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_b_app(Arc::clone(&sink));
    navigate_to(&mut app, Route::Dns);
    open(&mut app);
    let original = app
        .world()
        .resource::<DnsHostsEditorState>()
        .editor
        .rows
        .clone();
    input(&mut app, true, "nope");
    input(&mut app, false, "bad domain");
    let button = action(&mut app, |action| matches!(action, HostAction::CommitRow));
    click(&mut app, button);
    assert!(sink.submitted().is_empty());
    assert_eq!(
        app.world().resource::<DnsHostsEditorState>().editor.rows,
        original
    );
    let status = app
        .world_mut()
        .query::<(&Text, &DnsHostsStatusLine)>()
        .iter(app.world())
        .next()
        .unwrap()
        .0
        .0
        .clone();
    assert!(status.contains("域名不合法"), "{status}");
    assert!(status.contains("bad domain"), "{status}");
}
#[test]
fn test_dns_hosts_editor_clears_an_emptied_mapping() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_b_app(Arc::clone(&sink));
    navigate_to(&mut app, Route::Dns);
    open(&mut app);
    let original = app
        .world()
        .resource::<DnsHostsEditorState>()
        .editor
        .applied
        .as_ref()
        .unwrap()
        .entries
        .clone();
    let ids: Vec<_> = app
        .world()
        .resource::<DnsHostsEditorState>()
        .editor
        .rows
        .iter()
        .map(|row| row.id)
        .collect();
    for id in ids {
        let button = action(
            &mut app,
            |action| matches!(action, HostAction::Remove(row) if *row == id),
        );
        click(&mut app, button);
    }
    let button = action(&mut app, |action| matches!(action, HostAction::Apply));
    click(&mut app, button);
    match sink.submitted().first() {
        Some(UiCommand::ApplyDnsSettings { patch }) => {
            assert!(patch.clear_hosts);
            assert!(patch.hosts.is_none());
            assert_eq!(patch.expected_hosts.as_ref(), Some(&original));
        }
        other => panic!("expected explicit clear: {other:?}"),
    }
    assert_eq!(
        app.world()
            .resource::<DnsHostsEditorState>()
            .editor
            .applied
            .as_ref()
            .unwrap()
            .entries,
        original
    );
}
