//! Application routing copy replays canonical policy and raw OS application identities.
use crate::pages::app_routing::{
    AppNameText, AppProcessText, AppRoutingLine, AppRuleText, LastAppRoutingProjection,
    app_rule_color,
};
use bevy::ecs::query::{Or, QueryData, QueryFilter, With};
use bevy::ecs::system::{Query, Res};
use bevy::text::TextColor;
use bevy::ui::widget::Text;
use infiltrator_application::routing_projection::{process_copy, routing_summary, rule_key};
use infiltrator_bevy_widgets::localization::UiLocale;
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_shared::locales::{Lang, Localizer};

#[derive(QueryData)]
#[query_data(mutable)]
pub struct RoutingText {
    text: &'static mut Text,
    summary: Option<&'static AppRoutingLine>,
    name: Option<&'static AppNameText>,
    process: Option<&'static AppProcessText>,
    rule: Option<&'static AppRuleText>,
    color: Option<&'static mut TextColor>,
}
#[derive(QueryFilter)]
pub struct AppIdentityFilter {
    app: Or<(With<AppNameText>, With<AppProcessText>)>,
}
#[derive(QueryFilter)]
pub struct RoutingCopyFilter {
    routing: Or<(With<AppRoutingLine>, With<AppRuleText>, AppIdentityFilter)>,
}
pub fn sync(
    last: Res<LastAppRoutingProjection>,
    locale: Res<UiLocale>,
    palette: Res<UiPalette>,
    mut lines: Query<RoutingText, RoutingCopyFilter>,
) {
    let Some(projection) = &last.0 else {
        return;
    };
    for mut line in &mut lines {
        let value = if line.summary.is_some() {
            Some(routing_summary(
                projection.mode,
                projection.apps.len(),
                locale.code(),
            ))
        } else if let Some(marker) = line.name {
            projection.apps.get(marker.0).map(|app| app.name.clone())
        } else if let Some(marker) = line.process {
            projection
                .apps
                .get(marker.0)
                .map(|app| process_copy(&app.process_name, locale.code()))
        } else if let Some(marker) = line.rule {
            projection.apps.get(marker.0).map(|app| {
                if let Some(color) = line.color.as_deref_mut() {
                    color.0 = app_rule_color(app.rule, &palette);
                }
                Lang(locale.code()).tr(rule_key(app.rule)).into_owned()
            })
        } else {
            None
        };
        if let Some(value) = value
            && line.text.0 != value
        {
            line.text.0 = value;
        }
    }
}
