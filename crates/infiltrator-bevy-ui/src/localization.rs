//! Product language preference adapter; all renderer copy is owned by the widget layer.
use crate::pages::settings_language::LanguageChoiceResource;
use crate::surface::LatestSurfaceSnapshot;
use bevy::app::{App, Plugin, Update};
use bevy::ecs::schedule::IntoScheduleConfigs;
use bevy::ecs::system::{Res, ResMut};
use infiltrator_bevy_widgets::localization::{LocaleCopySet, UiLocale, WidgetLocalizationPlugin};

pub struct LocalizationPlugin;
impl Plugin for LocalizationPlugin {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<WidgetLocalizationPlugin>() {
            app.add_plugins(WidgetLocalizationPlugin);
        }
        app.add_systems(Update, read_language_preference.before(LocaleCopySet));
    }
}

pub(crate) fn read_language_preference(
    latest: Option<Res<LatestSurfaceSnapshot>>,
    state: Option<Res<LanguageChoiceResource>>,
    mut locale: ResMut<UiLocale>,
) {
    let preference = if let Some(state) = state {
        state.model.applied
    } else {
        latest
            .as_ref()
            .and_then(|latest| latest.0.language_settings.preference)
    };
    if let Some(preference) = preference {
        locale.observe_preference(preference.as_setting());
    }
}
