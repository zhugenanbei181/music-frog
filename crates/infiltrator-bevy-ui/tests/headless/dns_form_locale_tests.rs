//! test-intent: behavior
//! Native locale replay preserves editable DNS bytes and refuses missing composition.
use crate::native_input::{click_entity, press, replace_text};
use crate::support::headless_plugins;
use bevy::app::App;
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::Children;
use bevy::ui::InteractionDisabled;
use bevy::ui::widget::Text;
use bevy::ui_widgets::ButtonPlugin;
use bevy::window::{Ime, PrimaryWindow, Window};
use infiltrator_bevy_ui::app::ShellPlugin;
use infiltrator_bevy_ui::pages::dns_edit::{
    DnsEditApplyButton, DnsEditField, DnsEditStatusLine, DnsFormState,
};
use infiltrator_bevy_ui::projection::DemoOverviewSource;
use infiltrator_bevy_ui::route::{PagesPlugin, Route, RouteChanged};
use infiltrator_bevy_widgets::button::ButtonDisabled;
use infiltrator_bevy_widgets::localization::{LocalizedText, UiLocale};
use infiltrator_bevy_widgets::text_input::{TextField, TextFieldFocused};
use infiltrator_contract::dns_form::DnsFormField;

fn field(app: &mut App, kind: DnsFormField) -> Entity {
    let children = app
        .world_mut()
        .query::<(&DnsEditField, &Children)>()
        .iter(app.world())
        .find(|(field, _)| field.0 == kind)
        .unwrap()
        .1;
    children
        .iter()
        .copied()
        .find(|entity| app.world().get::<TextField>(*entity).is_some())
        .unwrap()
}
fn status(app: &mut App) -> String {
    app.world_mut()
        .query::<(&DnsEditStatusLine, &Text)>()
        .single(app.world())
        .unwrap()
        .1
        .0
        .clone()
}
#[test]
fn actual_dns_form_locale_keeps_native_input_identity_and_missing_host_cannot_queue_a_patch() {
    let mut app = App::new();
    headless_plugins(&mut app);
    app.add_plugins((
        ButtonPlugin,
        ShellPlugin::default(),
        PagesPlugin::new(DemoOverviewSource::running()),
    ));
    let window = app
        .world_mut()
        .spawn((Window::default(), PrimaryWindow))
        .id();
    app.update();
    app.world_mut().trigger(RouteChanged(Route::Dns));
    app.update();
    let entity = field(&mut app, DnsFormField::Nameserver);
    press(&mut app, entity);
    let invalid = "ftp://{field}/中文🙂";
    replace_text(&mut app, invalid);
    assert_eq!(
        app.world().get::<TextField>(entity).unwrap().0.text(),
        invalid
    );
    let apply = app
        .world_mut()
        .query::<(Entity, &DnsEditApplyButton)>()
        .single(app.world())
        .unwrap()
        .0;
    click_entity(&mut app, apply);
    let observed = status(&mut app);
    assert!(
        observed.contains(invalid),
        "{observed}; {:?}",
        app.world().resource::<DnsFormState>()
    );
    assert!(!app.world().resource::<DnsFormState>().submitted);
    press(&mut app, entity);
    let before = app.world().get::<TextField>(entity).unwrap().0.clone();
    app.insert_resource(UiLocale::new("en-US"));
    app.update();
    assert_eq!(field(&mut app, DnsFormField::Nameserver), entity);
    assert_eq!(app.world().get::<TextField>(entity).unwrap().0, before);
    assert!(app.world().get::<TextFieldFocused>(entity).unwrap().0);
    assert!(status(&mut app).starts_with("Local validation failed:"));
    assert!(status(&mut app).contains(invalid));
    let label = app
        .world_mut()
        .query::<(&LocalizedText, &Text)>()
        .iter(app.world())
        .find(|(copy, _)| copy.key == "dns_field_bootstrap")
        .unwrap()
        .1
        .0
        .clone();
    assert_eq!(label, "default_nameserver (bootstrap, pure IP)");
    replace_text(&mut app, "https://dns.example.test/query");
    app.world_mut().write_message(Ime::Preedit {
        window,
        value: "zhong".into(),
        cursor: Some((0, 5)),
    });
    app.update();
    assert!(app.world().get::<ButtonDisabled>(apply).unwrap().0);
    assert!(app.world().get::<InteractionDisabled>(apply).is_some());
    click_entity(&mut app, apply);
    assert!(!app.world().resource::<DnsFormState>().host_missing);
    assert!(!app.world().resource::<DnsFormState>().submitted);
    app.world_mut().write_message(Ime::Disabled { window });
    app.update();
    assert_eq!(
        app.world().get::<TextField>(entity).unwrap().0.text(),
        "https://dns.example.test/query"
    );
    assert!(!app.world().get::<ButtonDisabled>(apply).unwrap().0);

    click_entity(&mut app, apply);
    let state = app.world().resource::<DnsFormState>();
    assert!(state.host_missing);
    assert!(!state.submitted);
    assert!(state.issues.is_empty());
    assert_eq!(
        status(&mut app),
        "Command service is not composed; DNS patch was not submitted"
    );
}
