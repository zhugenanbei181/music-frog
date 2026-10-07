//! Restamp parameter and parse facts without changing row identity or input state.
use crate::pages::rules::LastRulesProjection;
use crate::pages::rules_projection::RulePayloadText;
use bevy::ecs::system::{Query, Res};
use bevy::ui::widget::Text;
use infiltrator_application::rule_row_projection::row_detail;
use infiltrator_bevy_widgets::localization::UiLocale;
pub fn sync(
    last: Option<Res<LastRulesProjection>>,
    locale: Res<UiLocale>,
    mut texts: Query<(&mut Text, &RulePayloadText)>,
) {
    let Some(projection) = last.as_ref().and_then(|last| last.0.as_ref()) else {
        return;
    };
    for (mut text, marker) in &mut texts {
        let Some(rule) = projection.rules.get(marker.0) else {
            continue;
        };
        let detail = row_detail(
            rule.source_ip,
            rule.no_resolve,
            rule.failure.as_ref(),
            locale.code(),
        );
        let value = if detail.is_empty() {
            rule.payload.clone()
        } else {
            format!("{} · {detail}", rule.payload)
        };
        if text.0 != value {
            text.0 = value;
        }
    }
}
