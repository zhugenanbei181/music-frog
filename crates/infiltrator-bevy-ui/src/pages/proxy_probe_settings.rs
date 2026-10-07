//! Native latency-parameter modal with shared draft validation and durable result correlation.
use crate::command::{CommandSinkHandle, UiCommand};
use crate::command_events::CommandExecutedEvent;
use crate::route::{ActiveRoute, Route};
use crate::surface::LatestSurfaceSnapshot;
use bevy::a11y::AccessibilityNode;
use bevy::ecs::component::Component;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::observer::On;
use bevy::ecs::query::{Has, Or, With};
use bevy::ecs::resource::Resource;
use bevy::ecs::system::{Query, Res, ResMut};
use bevy::input::ButtonInput;
use bevy::input::keyboard::KeyCode;
use bevy::scene::{Scene, bsn};
use bevy::ui::GlobalZIndex;
use bevy::ui::prelude::{
    AlignItems, BackgroundColor, Display, FlexDirection, JustifyContent, Node, Overflow,
    PositionType, UiRect, percent, px,
};
use bevy::ui::widget::Text;
use bevy::ui_widgets::{Activate, Button, ScrollArea};
use infiltrator_application::proxy_probe_editor::ProxyProbeEditor;
use infiltrator_bevy_widgets::button::ButtonDisabled;
use infiltrator_bevy_widgets::localization::{LocalizedLabel, LocalizedText, UiLocale};
use infiltrator_bevy_widgets::modal::{ModalDialogCard, ModalScrim};
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_bevy_widgets::text_input::state::TextFieldInput;
use infiltrator_bevy_widgets::text_input::{
    TextField, TextFieldFocused, text_field_with_placeholder_scene,
};
use infiltrator_contract::command::RequestId;
use infiltrator_contract::error::{ErrorCode, Failure};

#[derive(Component, Clone, Copy, Default)]
pub struct OpenProbeSettings;
#[derive(Component, Clone, Copy, Default)]
pub struct CancelProbeSettings;
#[derive(Component, Clone, Copy, Default)]
pub struct ApplyProbeSettings;
#[derive(Component, Clone, Copy, Default)]
pub struct ProbeSettingsRoot;
#[derive(Component, Clone, Copy, Default)]
pub struct ProbeSettingsCard;
#[derive(Component, Clone, Copy, Default)]
pub struct ProbeSettingsStatus;
#[derive(Component, Clone, Copy, Default)]
pub struct ProbeAppliedSummary;
#[derive(Component, Clone, Copy, Default)]
pub struct ProbeSettingsScrollArea;
#[derive(Component, Clone, Copy, Default)]
pub struct ProbeUrlField;
#[derive(Component, Clone, Copy, Default)]
pub struct ProbeTimeoutField;

#[derive(Resource, Default)]
pub struct ProbeSettingsState {
    pub open: bool,
    pub editor: ProxyProbeEditor,
    pub request: Option<(RequestId, u64)>,
    sync_fields: bool,
}

pub fn launcher(palette: &UiPalette) -> impl Scene + use<> {
    bsn! {
        Node { min_height: px(palette.control_height_px), padding: UiRect::axes(px(12.0), px(8.0)), align_items: AlignItems::Center, column_gap: px(8.0) }
        Button OpenProbeSettings LocalizedLabel::plain("proxy_probe_settings_title")
        Children [
            LocalizedText::plain("proxy_probe_settings_title") TextRole(Role::Caption)
            --
            Text(String::new()) ProbeAppliedSummary TextRole(Role::Caption)
        ]
    }
}
pub fn modal(palette: &UiPalette) -> impl Scene + use<> {
    let semantic = accesskit::Node::new(accesskit::Role::Dialog);
    bsn! {
        Node { position_type: PositionType::Absolute, width: percent(100), height: percent(100), align_items: AlignItems::Center, justify_content: JustifyContent::Center, display: Display::None }
        ProbeSettingsRoot GlobalZIndex(110)
        Children [
            Node { position_type: PositionType::Absolute, width: percent(100), height: percent(100) }
            BackgroundColor({ palette.scrim }) ModalScrim Button CancelProbeSettings ButtonDisabled(false)
            --
            Node { width: percent(90), max_width: px(520.0), max_height: percent(90), padding: UiRect::all(px(16.0)), flex_direction: FlexDirection::Column, row_gap: px(12.0) }
            BackgroundColor({ palette.surface }) ModalDialogCard ProbeSettingsCard
            AccessibilityNode(semantic) LocalizedLabel::plain("proxy_probe_settings_title")
            Children [
                LocalizedText::plain("proxy_probe_settings_title") TextRole(Role::Heading)
                --
                Node { width: percent(100), min_height: px(0.0), flex_shrink: 1.0, flex_direction: FlexDirection::Column, row_gap: px(12.0), overflow: Overflow::scroll_y() }
                ScrollArea ProbeSettingsScrollArea
                Children [
                LocalizedText::plain("proxy_probe_settings_description") TextRole(Role::Caption)
                --
                LocalizedText::plain("proxies_delay_test_url_label") TextRole(Role::Body)
                --
                Node { width: percent(100) } ProbeUrlField
                Children [ @{ text_field_with_placeholder_scene(String::new(), "https://example.test/204".into(), palette) } ]
                --
                LocalizedText::plain("proxies_delay_timeout_label") TextRole(Role::Body)
                --
                Node { width: percent(100) } ProbeTimeoutField
                Children [ @{ text_field_with_placeholder_scene(String::new(), "5000".into(), palette) } ]
                --
                Text(String::new()) ProbeSettingsStatus TextRole(Role::Caption)
                ]
                --
                Node { width: percent(100), justify_content: JustifyContent::SpaceBetween, flex_shrink: 0.0 }
                Children [
                    Node { min_height: px(palette.control_height_px), padding: UiRect::axes(px(12.0), px(8.0)) }
                    Button CancelProbeSettings ButtonDisabled(false)
                    Children [ LocalizedText::plain("modal_cancel") TextRole(Role::Body) ]
                    --
                    Node { min_height: px(palette.control_height_px), padding: UiRect::axes(px(12.0), px(8.0)) }
                    Button ApplyProbeSettings ButtonDisabled(true)
                    Children [ LocalizedText::plain("proxy_probe_settings_apply") TextRole(Role::BodyStrong) ]
                ]
            ]
        ]
    }
}

pub fn activate(
    event: On<Activate>,
    open: Query<(), With<OpenProbeSettings>>,
    cancel: Query<(), With<CancelProbeSettings>>,
    apply: Query<(), With<ApplyProbeSettings>>,
    latest: Res<LatestSurfaceSnapshot>,
    mut state: ResMut<ProbeSettingsState>,
    sink: Option<Res<CommandSinkHandle>>,
) {
    if open.contains(event.entity) {
        state.editor.observe(&latest.0.probe_settings);
        state.open = true;
        state.sync_fields = true;
    } else if cancel.contains(event.entity) && state.open && state.editor.pending.is_none() {
        state.editor.cancel();
        state.open = false;
        state.sync_fields = true;
    } else if apply.contains(event.entity) && state.open && state.editor.can_apply() {
        let pending = match state.editor.begin() {
            Ok(pending) => pending,
            Err(_) => return,
        };
        if let Some(request) = sink.and_then(|sink| {
            sink.submit_tracked(UiCommand::SetProxyProbeOptions {
                options: pending.options,
            })
        }) {
            state.request = Some((request, pending.token));
        } else {
            state.editor.finish(
                pending.token,
                Err(Failure::new(
                    ErrorCode::NotReady,
                    "probe settings have no terminal command service",
                    true,
                )),
            );
        }
    }
}

pub fn finish(event: On<CommandExecutedEvent>, mut state: ResMut<ProbeSettingsState>) {
    let Some((request, token)) = state.request else {
        return;
    };
    let UiCommand::SetProxyProbeOptions { options } = &event.command else {
        return;
    };
    if request != event.request_id
        || !state
            .editor
            .pending
            .as_ref()
            .is_some_and(|pending| pending.token == token && pending.options == *options)
    {
        return;
    }
    let success = event.unit_result().is_ok();
    if state.editor.finish(token, event.unit_result()) {
        state.request = None;
        state.sync_fields = true;
        if success {
            state.open = false;
        }
    }
}

type ProbeFieldWrappers = Or<(With<ProbeUrlField>, With<ProbeTimeoutField>)>;
type ProbeCopyRows = Or<(With<ProbeSettingsStatus>, With<ProbeAppliedSummary>)>;
type ProbeActionButtons = Or<(With<ApplyProbeSettings>, With<CancelProbeSettings>)>;

pub fn sync_fields(
    mut state: ResMut<ProbeSettingsState>,
    wrappers: Query<(&Children, Has<ProbeUrlField>), ProbeFieldWrappers>,
    mut fields: Query<(&mut TextField, &mut TextFieldFocused)>,
) {
    for (children, url) in &wrappers {
        for child in children {
            let Ok((mut field, mut focus)) = fields.get_mut(*child) else {
                continue;
            };
            let value = if url {
                &state.editor.draft.test_url
            } else {
                &state.editor.draft.timeout_ms
            };
            if state.sync_fields {
                if field.0.text() != value {
                    field.0.apply(TextFieldInput::SetText(value.clone()));
                }
            } else if state.open && state.editor.pending.is_none() && field.0.text() != value {
                let value = field.0.text().to_owned();
                if url {
                    state.editor.edit_url(value);
                } else {
                    state.editor.edit_timeout(value);
                }
            }
            let disabled = !state.open || state.editor.pending.is_some();
            if field.0.is_disabled() != disabled {
                field.0.set_disabled(disabled);
            }
            if disabled && focus.0 {
                focus.0 = false;
            }
        }
    }
    state.sync_fields = false;
}

pub fn sync_editor(
    keys: Option<Res<ButtonInput<KeyCode>>>,
    route: Res<ActiveRoute>,
    latest: Res<LatestSurfaceSnapshot>,
    mut state: ResMut<ProbeSettingsState>,
) {
    let previous = state.editor.draft.clone();
    state.editor.observe(&latest.0.probe_settings);
    state.sync_fields |= state.editor.draft != previous;
    if route.0 != Some(Route::Proxies) && state.open {
        if state.editor.pending.is_none() {
            state.editor.cancel();
        }
        state.open = false;
        state.sync_fields = true;
    }
    if keys.is_some_and(|keys| keys.just_pressed(KeyCode::Escape))
        && state.open
        && state.editor.pending.is_none()
    {
        state.editor.cancel();
        state.open = false;
        state.sync_fields = true;
    }
}

pub fn sync_surface(
    locale: Res<UiLocale>,
    state: Res<ProbeSettingsState>,
    mut roots: Query<&mut Node, With<ProbeSettingsRoot>>,
    mut buttons: Query<(&mut ButtonDisabled, Has<ApplyProbeSettings>), ProbeActionButtons>,
    mut text_rows: Query<(&mut Text, Has<ProbeAppliedSummary>), ProbeCopyRows>,
) {
    for mut root in &mut roots {
        let display = if state.open {
            Display::Flex
        } else {
            Display::None
        };
        if root.display != display {
            root.display = display;
        }
    }
    for (mut button, apply) in &mut buttons {
        let disabled = if apply {
            !state.editor.can_apply()
        } else {
            state.editor.pending.is_some()
        };
        if button.0 != disabled {
            button.0 = disabled;
        }
    }
    let message = state
        .editor
        .failure
        .as_ref()
        .or(state.editor.validation.as_ref())
        .map(|failure| failure.message.clone())
        .unwrap_or_else(|| {
            if state.editor.pending.is_some() {
                LocalizedText::plain("proxy_probe_settings_pending").render(&locale)
            } else {
                String::new()
            }
        });
    let applied = state
        .editor
        .applied
        .as_ref()
        .map(|options| format!("{} · {} ms", options.test_url, options.timeout_ms))
        .unwrap_or_else(|| {
            LocalizedText::plain("proxy_probe_settings_unavailable").render(&locale)
        });
    for (mut text, summary) in &mut text_rows {
        let value = if summary { &applied } else { &message };
        if text.0 != *value {
            text.0 = value.clone();
        }
    }
}
