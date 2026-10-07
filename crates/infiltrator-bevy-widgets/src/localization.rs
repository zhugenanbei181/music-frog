//! Native text localization updates mounted entities and never interprets user data as copy keys.
use crate::text_input::TextField;
use crate::text_input::render::sync_text_fields;
use bevy::a11y::AccessibilityNode;
use bevy::app::{App, Plugin, Update};
use bevy::ecs::change_detection::{DetectChanges, Ref};
use bevy::ecs::component::Component;
use bevy::ecs::lifecycle::Insert;
use bevy::ecs::observer::On;
use bevy::ecs::resource::Resource;
use bevy::ecs::schedule::{IntoScheduleConfigs, SystemSet};
use bevy::ecs::system::{Query, Res};
use bevy::ui::widget::Text;
use infiltrator_shared::i18n_interpolator::interpolate;
use infiltrator_shared::locales::{Lang, Localizer, resolve_language_code};
use std::env;

#[derive(Resource, Clone, Debug, PartialEq, Eq)]
pub struct UiLocale {
    code: String,
    preference: Option<String>,
}
impl UiLocale {
    pub fn new(code: &str) -> Self {
        Self {
            code: resolve_language_code(code),
            preference: None,
        }
    }
    pub fn apply_preference(&mut self, preference: &str) {
        self.code = resolve_language_code(preference);
        self.preference = Some(preference.into());
    }
    pub fn observe_preference(&mut self, preference: &str) {
        if env::var("INFILTRATOR_LANG").is_ok() && self.preference.is_none() {
            return;
        }
        if self.preference.as_deref() != Some(preference) {
            self.apply_preference(preference);
        }
    }
    pub fn text(&self, key: &str) -> String {
        Lang(self.code()).tr(key).into_owned()
    }
    pub fn code(&self) -> &str {
        &self.code
    }
}
impl Default for UiLocale {
    fn default() -> Self {
        Self::new(&env::var("INFILTRATOR_LANG").unwrap_or_else(|_| "zh-CN".into()))
    }
}

#[derive(Component, Clone, Debug, Default, PartialEq, Eq)]
#[require(Text)]
pub struct LocalizedText {
    pub key: &'static str,
    pub params: Vec<(&'static str, String)>,
}
impl LocalizedText {
    pub fn plain(key: &'static str) -> Self {
        Self {
            key,
            params: Vec::new(),
        }
    }
    pub fn new(key: &'static str, params: Vec<(&'static str, String)>) -> Self {
        Self { key, params }
    }
    pub fn render(&self, locale: &UiLocale) -> String {
        let params: Vec<_> = self
            .params
            .iter()
            .map(|(key, value)| (*key, value.as_str()))
            .collect();
        interpolate(Lang(locale.code()).tr(self.key).as_ref(), &params)
    }
}

fn initialize_copy(
    insert: On<Insert<LocalizedText>>,
    locale: Res<UiLocale>,
    mut texts: Query<(&LocalizedText, &mut Text)>,
) {
    if let Ok((copy, mut text)) = texts.get_mut(insert.entity) {
        text.0 = copy.render(&locale);
    }
}

#[derive(Component, Clone, Debug, Default, PartialEq, Eq)]
pub struct LocalizedLabel(pub LocalizedText);
impl LocalizedLabel {
    pub fn plain(key: &'static str) -> Self {
        Self(LocalizedText::plain(key))
    }
}

pub struct WidgetLocalizationPlugin;

#[derive(SystemSet, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct LocaleCopySet;
/// Native placeholder copy, independent of the user's editing state.
#[derive(Component, Clone, Debug, Default, PartialEq, Eq)]
#[require(TextField)]
pub struct LocalizedPlaceholder(pub LocalizedText);
impl LocalizedPlaceholder {
    pub fn plain(key: &'static str) -> Self {
        Self(LocalizedText::plain(key))
    }
}
impl Plugin for WidgetLocalizationPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<UiLocale>();
        app.add_observer(initialize_copy);
        app.add_systems(
            Update,
            synchronize_copy
                .in_set(LocaleCopySet)
                .before(sync_text_fields),
        );
    }
}

fn synchronize_copy(
    locale: Res<UiLocale>,
    mut texts: Query<(Ref<LocalizedText>, &mut Text)>,
    mut labels: Query<(Ref<LocalizedLabel>, &mut AccessibilityNode)>,
    mut placeholders: Query<(Ref<LocalizedPlaceholder>, &mut TextField)>,
) {
    for (copy, mut text) in &mut texts {
        if locale.is_changed() || copy.is_changed() {
            let value = copy.render(&locale);
            if text.0 != value {
                text.0 = value;
            }
        }
    }
    for (copy, mut node) in &mut labels {
        if locale.is_changed() || copy.is_changed() {
            node.set_label(copy.0.render(&locale));
        }
    }
    for (copy, mut field) in &mut placeholders {
        if locale.is_changed() || copy.is_changed() {
            let value = copy.0.render(&locale);
            if field.0.placeholder() != value {
                field.0.set_placeholder(value);
            }
        }
    }
}
