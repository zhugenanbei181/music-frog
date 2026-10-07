//! Single native keyboard/IME owner with route-persistent wizard drafts.
use crate::pages::rules_builder::{
    RuleBuilderSelection, RuleFormStatus, RuleTypeChip, RulesBuilderState, restamp_type_chips,
};
use crate::pages::rules_draft::RulesDraftState;
use crate::pages::rules_tabs::RulesTabState;
use crate::route::{ActiveRoute, Route};
use bevy::ecs::component::Component;
use bevy::ecs::query::With;
use bevy::ecs::system::{Query, Res, ResMut};
use bevy::ui::BackgroundColor;
use bevy::ui::widget::Text;
use infiltrator_application::rule_form_binding::default_form_target;
use infiltrator_bevy_widgets::localization::{LocalizedText, UiLocale};
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::text_input::{TextField, TextFieldFocused};
use infiltrator_contract::rules_workspace::RulesTab;

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub enum RuleBuilderFieldKind {
    #[default]
    Payload,
    Target,
}
pub fn sync_selection(
    builder: Option<Res<RulesBuilderState>>,
    locale: Res<UiLocale>,
    palette: Res<UiPalette>,
    mut chips: Query<(&mut BackgroundColor, &RuleTypeChip)>,
    mut labels: Query<(&mut Text, &mut LocalizedText), With<RuleBuilderSelection>>,
) {
    let Some(builder) = builder else {
        return;
    };
    restamp_type_chips(&palette, &mut chips, &builder.rule_type);
    for (mut text, mut copy) in &mut labels {
        copy.params = vec![("type", builder.rule_type.clone())];
        let value = copy.render(&locale);
        if text.0 != value {
            text.0 = value;
        }
    }
}
pub fn sync_status(
    editor: Res<RulesDraftState>,
    builder: Res<RulesBuilderState>,
    locale: Res<UiLocale>,
    mut labels: Query<&mut Text, With<RuleFormStatus>>,
) {
    let status = builder.binding.status(&editor.model, locale.code());
    for mut text in &mut labels {
        text.0.clone_from(&status);
    }
}
#[derive(Component, Clone, Copy, Default)]
pub struct RuleBuilderField {
    pub kind: RuleBuilderFieldKind,
    pub initialized: bool,
}

pub fn sync(
    route: Res<ActiveRoute>,
    tab: Option<Res<RulesTabState>>,
    mut editor: ResMut<RulesDraftState>,
    mut builder: Option<ResMut<RulesBuilderState>>,
    mut fields: Query<(&mut TextField, &mut TextFieldFocused, &mut RuleBuilderField)>,
) {
    let Some(builder) = builder.as_deref_mut() else {
        return;
    };
    let active = route.0 == Some(Route::Rules) && tab.is_some_and(|tab| tab.tab == RulesTab::List);
    if builder.binding.observe(&editor.model) {
        builder.target = default_form_target(&editor.model);
        builder.initialized = false;
    }
    let editable = active && builder.binding.current(&editor.model);
    let restore = builder.initialized;
    let mut seen = 0;
    let mut composing = false;
    for (mut field, mut focus, mut marker) in &mut fields {
        field.0.set_disabled(!editable);
        if !marker.initialized || !restore {
            focus.0 = false;
            let text = match marker.kind {
                RuleBuilderFieldKind::Payload => &builder.payload,
                RuleBuilderFieldKind::Target => &builder.target,
            };
            field.0.restore_text(text.clone());
            marker.initialized = true;
        }
        if !editable {
            focus.0 = false;
        }
        if editable {
            let changed = match marker.kind {
                RuleBuilderFieldKind::Payload => builder.payload != field.0.text(),
                RuleBuilderFieldKind::Target => builder.target != field.0.text(),
            };
            if changed && restore {
                builder.binding.edit(&editor.model);
            }
            match marker.kind {
                RuleBuilderFieldKind::Payload => builder.payload = field.0.text().to_owned(),
                RuleBuilderFieldKind::Target => builder.target = field.0.text().to_owned(),
            }
        }
        composing |= editable && field.0.is_in_ime_transaction();
        seen += 1;
    }
    builder.initialized |= seen == 2;
    editor.input_composing = composing;
}
