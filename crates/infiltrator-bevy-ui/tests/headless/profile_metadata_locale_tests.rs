//! test-intent: behavior
//! Metadata locale replay must preserve honest partial traffic and native editing.
use crate::native_input::{press, replace_text};
use crate::support::headless_plugins;
use bevy::app::App;
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::Children;
use bevy::ui::widget::Text;
use infiltrator_bevy_ui::app::ShellPlugin;
use infiltrator_bevy_ui::pages::profiles::{
    ProfileTrafficText, ProfilesProjection, ProfilesProjectionUpdated,
};
use infiltrator_bevy_ui::pages::profiles_subscription_policy::{
    SubscriptionPolicyStatus, SubscriptionPolicyUrlField,
};
use infiltrator_bevy_ui::projection::DemoOverviewSource;
use infiltrator_bevy_ui::route::{PagesPlugin, Route, RouteChanged};
use infiltrator_bevy_widgets::localization::UiLocale;
use infiltrator_bevy_widgets::text_input::{TextField, TextFieldFocused};

#[test]
fn native_profile_locale_preserves_partial_quota_and_url_draft_while_showing_paused_plan() {
    let mut app = App::new();
    headless_plugins(&mut app);
    app.add_plugins((
        ShellPlugin::default(),
        PagesPlugin::new(DemoOverviewSource::running()),
    ));
    app.update();
    app.world_mut().trigger(RouteChanged(Route::Profiles));
    app.update();
    let mut facts = ProfilesProjection::demo();
    facts.profiles[0].upload_bytes = None;
    facts.profiles[0].download_bytes = Some(0);
    facts.profiles[0].total_bytes = Some(0);
    facts.profiles[0].auto_update_enabled = false;
    facts.profiles[0].cron_expression = Some("0 0 * * *".into());
    app.world_mut().trigger(ProfilesProjectionUpdated(facts));
    app.update();
    let caption = app
        .world_mut()
        .query::<(Entity, &ProfileTrafficText)>()
        .iter(app.world())
        .find(|(_, marker)| marker.0 == 0)
        .unwrap()
        .0;
    assert_eq!(
        app.world().get::<Text>(caption).unwrap().0,
        "↑ — ↓ 0 B · 已用: — / 总计: 0 B"
    );
    let parent = app
        .world_mut()
        .query::<(Entity, &SubscriptionPolicyUrlField)>()
        .single(app.world())
        .unwrap()
        .0;
    let input = app
        .world()
        .get::<Children>(parent)
        .unwrap()
        .iter()
        .copied()
        .find(|child| app.world().get::<TextField>(*child).is_some())
        .unwrap();
    press(&mut app, input);
    let draft = "https://draft.example/{profile}/中文🙂";
    replace_text(&mut app, draft);
    assert_eq!(app.world().get::<TextField>(input).unwrap().0.text(), draft);
    let before = app.world().get::<TextField>(input).unwrap().0.clone();
    app.world_mut()
        .resource_mut::<UiLocale>()
        .apply_preference("en-US");
    app.update();
    let field = &app.world().get::<TextField>(input).unwrap().0;
    assert_eq!(field.text(), before.text());
    assert_eq!(field.cursor(), before.cursor());
    assert_eq!(field.selection(), before.selection());
    assert!(app.world().get::<TextFieldFocused>(input).unwrap().0);
    assert_eq!(
        app.world().get::<Text>(caption).unwrap().0,
        "↑ — ↓ 0 B · Used: — / Total: 0 B"
    );
    let label = app
        .world_mut()
        .query::<(&SubscriptionPolicyStatus, &Text)>()
        .single(app.world())
        .unwrap()
        .1
        .0
        .clone();
    assert!(label.contains("paused"), "{label}");
    assert!(label.contains("0 0 * * *"));
    assert!(
        !app.world()
            .get::<Text>(caption)
            .unwrap()
            .0
            .contains("unlimited")
    );
}
