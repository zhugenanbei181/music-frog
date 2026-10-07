//! Persistent native sandbox inspector; each field has its own typed draft identity.
use crate::localized_widgets::{localized_button_scene, localized_field_scene};
use crate::pages::rules_tabs::RulesTabState;
use crate::pages::rules_tracer::RulesTraceState;
use crate::route::{ActiveRoute, Route};
use bevy::ecs::component::Component;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::observer::On;
use bevy::ecs::system::{Query, Res, ResMut};
use bevy::scene::{Scene, bsn};
use bevy::ui::{Display, FlexDirection, Node, Overflow, UiRect, percent, px};
use bevy::ui_widgets::{Activate, ScrollArea};
use infiltrator_application::rule_condition_projection::field_key;
use infiltrator_bevy_widgets::button::{ButtonDisabled, ButtonVariant};
use infiltrator_bevy_widgets::localization::LocalizedText;
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_bevy_widgets::text_input::native::NativeTextField;
use infiltrator_bevy_widgets::text_input::state::TextFieldInput;
use infiltrator_bevy_widgets::text_input::{TextField, TextFieldFocused};
use infiltrator_contract::rule_condition::TrafficField;
use infiltrator_contract::rules_workspace::RulesTab;

#[derive(Component, Clone, Copy, Default)]
pub struct TracerSandboxRoot;
#[derive(Component, Clone, Copy, Default)]
pub struct TracerSandboxToggle;
#[derive(Component, Clone, Copy)]
pub struct TracerSandboxField {
    pub field: TrafficField,
    pub initialized: bool,
}
impl Default for TracerSandboxField {
    fn default() -> Self {
        Self {
            field: TrafficField::DestinationIp,
            initialized: false,
        }
    }
}

pub fn scene(palette: &UiPalette) -> impl Scene + use<> {
    let fields: Vec<Box<dyn Scene>> = TrafficField::SANDBOX.into_iter().enumerate().map(|(index, field)| {
        Box::new(bsn! {
            Node { width: percent(100), flex_direction: FlexDirection::Column }
            Children [
                LocalizedText::plain(field_key(field)) TextRole(Role::Caption)
                --
                @{ (localized_field_scene(String::new(), LocalizedText::plain("rule_trace_sandbox_unknown"), palette),
                    bsn! { TracerSandboxField { field, initialized: false } NativeTextField({ index as i32+3 }) }) }
            ]
        }) as Box<dyn Scene>
    }).collect();
    bsn! {
        Node { width: percent(100), flex_direction: FlexDirection::Column }
        Children [
            @{ (localized_button_scene(LocalizedText::plain("rule_trace_sandbox_toggle"), ButtonVariant::Default, palette), bsn! { TracerSandboxToggle }) }
            --
            Node { width: percent(100), height: px(180.0), min_height: px(0.0), display: Display::None, flex_direction: FlexDirection::Column, row_gap: px(8.0), overflow: Overflow::scroll_y(), padding: UiRect::all(px(4.0)) }
            TracerSandboxRoot ScrollArea
            Children [
                LocalizedText::plain("rule_trace_sandbox_notice") TextRole(Role::Caption)
                --
                { fields }
            ]
        ]
    }
}
pub fn activate(
    event: On<Activate>,
    buttons: Query<&TracerSandboxToggle>,
    route: Res<ActiveRoute>,
    tab: Option<Res<RulesTabState>>,
    mut state: ResMut<RulesTraceState>,
    mut fields: Query<(&TracerSandboxField, &mut TextFieldFocused)>,
) {
    if buttons.get(event.entity).is_err()
        || route.0 != Some(Route::Rules)
        || tab.as_ref().is_none_or(|tab| tab.tab != RulesTab::Tracer)
        || state.model.busy()
        || state.model.confirmation.is_some()
    {
        return;
    }
    state.model.advanced_open = !state.model.advanced_open;
    if !state.model.advanced_open {
        for (_, mut focused) in &mut fields {
            focused.0 = false;
        }
    }
}
pub fn sync(
    route: Res<ActiveRoute>,
    tab: Option<Res<RulesTabState>>,
    mut state: ResMut<RulesTraceState>,
    mut roots: Query<(&TracerSandboxRoot, &mut Node)>,
    mut fields: Query<(
        &mut TracerSandboxField,
        &mut TextField,
        &mut TextFieldFocused,
    )>,
    mut toggles: Query<(&TracerSandboxToggle, &mut ButtonDisabled)>,
) {
    let active = route.0 == Some(Route::Rules)
        && tab.as_ref().is_some_and(|tab| tab.tab == RulesTab::Tracer);
    let open = active && state.model.advanced_open;
    let disabled = !open || state.model.busy() || state.model.confirmation.is_some();
    for (_, mut node) in &mut roots {
        node.display = if open { Display::Flex } else { Display::None };
    }
    for (mut field, mut input, mut focused) in &mut fields {
        if !field.initialized {
            let value = state
                .model
                .sandbox
                .get(&field.field)
                .cloned()
                .unwrap_or_default();
            input.0.apply(TextFieldInput::SetText(value));
            field.initialized = true;
        }
        if open && !disabled {
            state
                .model
                .set_sandbox(field.field, input.0.text().to_owned());
        }
        if input.0.is_disabled() != disabled {
            input.0.set_disabled(disabled);
        }
        if !open {
            focused.0 = false;
        }
    }
    for (_, mut disabled) in &mut toggles {
        disabled.0 = !active || state.model.busy() || state.model.confirmation.is_some();
    }
}
