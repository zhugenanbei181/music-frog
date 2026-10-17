//! Locale and theme replay preserves mounted quota entities and observed values.
use crate::pages::overview::LastOverviewProjection;
use crate::pages::overview_cards::quota_status_color;
use crate::pages::overview_restamp::{
    SubscriptionQuotaText, SubscriptionQuotaTextKind, subscription_quota_text_value,
};
use bevy::ecs::system::{Query, Res};
use bevy::text::TextColor;
use bevy::ui::widget::Text;
use infiltrator_bevy_widgets::localization::UiLocale;
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_presentation::subscription_quota_projection::project_quota;
use infiltrator_shared::locales::Lang;
pub fn replay(
    last: Res<LastOverviewProjection>,
    locale: Res<UiLocale>,
    palette: Res<UiPalette>,
    mut labels: Query<(&SubscriptionQuotaText, &mut Text, &mut TextColor)>,
) {
    let Some(projection) = &last.0 else {
        return;
    };
    let quota = project_quota(&projection.subscription_quota, &Lang(locale.code()));
    for (role, mut text, mut color) in &mut labels {
        let value = subscription_quota_text_value(&quota, role.0);
        if text.0 != value {
            text.0 = value;
        }
        if matches!(role.0, SubscriptionQuotaTextKind::Status) {
            color.0 = quota_status_color(quota.grade, &palette);
        }
    }
}
