//! Native latency-parameter modal with shared draft validation and durable result correlation.
use crate::command::{CommandSinkHandle, UiCommand};
use crate::command_events::CommandExecutedEvent;
use crate::route::{ActiveRoute, Route};
use crate::surface::LatestSurfaceSnapshot;
use bevy::a11y::AccessibilityNode;
use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::observer::On;
use bevy::ecs::query::{Has, Or, With, Without};
use bevy::ecs::resource::Resource;
use bevy::ecs::system::{Commands, Query, Res, ResMut};
use bevy::feathers::theme::ThemeBackgroundColor;
use bevy::input_focus::tab_navigation::TabIndex;
use bevy::scene::{Scene, bsn, on};
use bevy::ui::GlobalZIndex;
use bevy::ui::prelude::{
    BorderRadius,
    AlignItems, Display, FlexDirection, JustifyContent, Node, Overflow, PositionType, UiRect,
    percent, px,
};
use bevy::ui::widget::Text;
use bevy::ui_widgets::{
    Activate, Button, ModalDialog, ModalDialogBarrier, RequestClose, ScrollArea,
};
use infiltrator_bevy_widgets::button::ButtonDisabled;
use infiltrator_bevy_widgets::feathers_theme::Token;
use infiltrator_bevy_widgets::localization::{LocalizedLabel, LocalizedText, UiLocale};
use infiltrator_bevy_widgets::modal::{ModalDialogCard, ModalScrim};
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::palette::rem;
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_bevy_widgets::text_input::state::TextFieldInput;
use infiltrator_bevy_widgets::text_input::{
    TextField, TextFieldFocused, text_field_with_placeholder_scene,
};
use infiltrator_bevy_widgets::theme::space;
use infiltrator_contract::command::RequestId;
use infiltrator_presentation::runtime_probe_settings_editor::{
    RuntimeProbeSettingsEditor, RuntimeProbeSettingsWriteOutcome,
};
use lurker_core_contract::error::{ErrorCode, Failure};

/// One-off layout value (not a ladder rung): the widest the probe settings
/// modal card may grow on a wide viewport.
const PROBE_SETTINGS_MODAL_MAX_WIDTH_PX: f32 = 520.0;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ProbeSettingsDialogState {
    #[default]
    Closed,
    Editing,
}

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
    pub dialog: ProbeSettingsDialogState,
    pub editor: RuntimeProbeSettingsEditor,
    pub request: ProbeSettingsRequestState,
    field_projection: ProbeSettingsFieldProjection,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ProbeSettingsRequestState {
    #[default]
    Idle,
    Submitted {
        request_id: RequestId,
        token: u64,
    },
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum ProbeSettingsFieldProjection {
    #[default]
    Current,
    NeedsProjection,
}

impl ProbeSettingsFieldProjection {
    fn request(&mut self) {
        *self = Self::NeedsProjection;
    }
}

pub fn launcher(palette: &UiPalette) -> impl Scene + use<> {
    bsn! {
        Node { min_height: rem(palette.control_height_px), padding: UiRect::axes(rem(space::S12), rem(space::S8)), align_items: AlignItems::Center, column_gap: rem(space::S8) }
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
        // The barrier is the modal layer's root: it owns the click-outside
        // close and the Escape interception, and the card is its child, so the
        // official behaviour layer reaches the focused control.
        ThemeBackgroundColor({ Token::Scrim }) ModalScrim ModalDialogBarrier
        // The close request travels as the official `RequestClose` and the
        // page's own close path runs from it.
        on(|close: On<RequestClose>, closes: Query<Entity, (With<CancelProbeSettings>, Without<ModalScrim>)>, mut commands: Commands| {
            for entity in &closes {
                // A close request the dismiss control itself raised is that
                // control's business; every other close request (the barrier's
                // click-outside, the official Escape interception) runs the
                // page's own close path.
                if entity != close.original_event_target() {
                    commands.trigger(Activate { entity });
                }
            }
        })
        Children [
            Node { width: percent(90), max_width: rem(PROBE_SETTINGS_MODAL_MAX_WIDTH_PX), max_height: percent(90), padding: UiRect::all(rem(space::S16)), flex_direction: FlexDirection::Column, row_gap: rem(space::S12), border_radius: BorderRadius::all(rem(palette.modal_radius_px)) }
            ThemeBackgroundColor({ Token::Surface }) ModalDialogCard ProbeSettingsCard ModalDialog
            AccessibilityNode(semantic) LocalizedLabel::plain("proxy_probe_settings_title")
            Children [
                LocalizedText::plain("proxy_probe_settings_title") TextRole(Role::Heading)
                --
                Node { width: percent(100), min_height: px(0.0), flex_shrink: 1.0, flex_direction: FlexDirection::Column, row_gap: rem(space::S12), overflow: Overflow::scroll_y() }
                ScrollArea ProbeSettingsScrollArea
                Children [
                LocalizedText::plain("proxy_probe_settings_description") TextRole(Role::Caption)
                --
                LocalizedText::plain("runtime_probe_url_label") TextRole(Role::Body)
                --
                Node { width: percent(100) } ProbeUrlField
                Children [ @{ text_field_with_placeholder_scene(String::new(), "https://example.test/204".into(), palette) } ]
                --
                LocalizedText::plain("runtime_probe_timeout_label") TextRole(Role::Body)
                --
                Node { width: percent(100) } ProbeTimeoutField
                Children [ @{ text_field_with_placeholder_scene(String::new(), "5000".into(), palette) } ]
                --
                Text(String::new()) ProbeSettingsStatus TextRole(Role::Caption)
                ]
                --
                Node { width: percent(100), justify_content: JustifyContent::SpaceBetween, flex_shrink: 0.0 }
                Children [
                    Node { min_height: rem(palette.control_height_px), padding: UiRect::axes(rem(space::S12), rem(space::S8)) }
                    Button CancelProbeSettings ButtonDisabled(false)
                    TabIndex(0)
                    // The dismiss control asks the official layer to close; the
                    // page never closes the probe editor on its own.
                    on(|activate: On<Activate>, mut commands: Commands| {
                        commands.trigger(RequestClose { source: activate.entity });
                    })
                    Children [ LocalizedText::plain("modal_cancel") TextRole(Role::Body) ]
                    --
                    Node { min_height: rem(palette.control_height_px), padding: UiRect::axes(rem(space::S12), rem(space::S8)) }
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
        state.dialog = ProbeSettingsDialogState::Editing;
        state.field_projection.request();
    } else if cancel.contains(event.entity)
        && state.dialog == ProbeSettingsDialogState::Editing
        && !state.editor.is_pending()
    {
        state.editor.cancel();
        state.dialog = ProbeSettingsDialogState::Closed;
        state.field_projection.request();
    } else if apply.contains(event.entity)
        && state.dialog == ProbeSettingsDialogState::Editing
        && state.editor.can_apply()
    {
        let pending = match state.editor.begin() {
            Ok(pending) => pending,
            Err(_) => return,
        };
        if let Some(request) = sink.and_then(|sink| {
            sink.submit_tracked(UiCommand::SetRuntimeProbeSettings {
                settings: pending.settings,
            })
        }) {
            state.request = ProbeSettingsRequestState::Submitted {
                request_id: request,
                token: pending.token,
            };
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
    let ProbeSettingsRequestState::Submitted { request_id, token } = state.request else {
        return;
    };
    let UiCommand::SetRuntimeProbeSettings { settings } = &event.command else {
        return;
    };
    let Some(pending) = state.editor.pending() else {
        return;
    };
    if request_id != event.request_id || pending.token != token || pending.settings != *settings {
        return;
    }
    match state.editor.finish(token, event.unit_result()) {
        RuntimeProbeSettingsWriteOutcome::Saved => {
            state.request = ProbeSettingsRequestState::Idle;
            state.field_projection.request();
            state.dialog = ProbeSettingsDialogState::Closed;
        }
        RuntimeProbeSettingsWriteOutcome::Failed => {
            state.request = ProbeSettingsRequestState::Idle;
            state.field_projection.request();
        }
        RuntimeProbeSettingsWriteOutcome::Stale => {}
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
    let mut synchronized_fields = 0;
    let mut deferred_projection = false;
    for (children, url) in &wrappers {
        for child in children {
            let Ok((mut field, mut focus)) = fields.get_mut(*child) else {
                continue;
            };
            synchronized_fields += 1;
            let value = state.editor.draft().map_or_else(String::new, |draft| {
                if url {
                    draft.url.clone()
                } else {
                    draft.timeout_ms.clone()
                }
            });
            let disabled = state.dialog != ProbeSettingsDialogState::Editing
                || state.editor.is_pending()
                || state.editor.applied().is_none();
            if field.0.is_disabled() && !disabled {
                field.0.set_disabled(false);
            }
            if state.field_projection == ProbeSettingsFieldProjection::NeedsProjection {
                if field.0.text() != value {
                    field.0.apply(TextFieldInput::SetText(value.clone()));
                    deferred_projection |= field.0.text() != value;
                }
            } else if state.dialog == ProbeSettingsDialogState::Editing
                && !state.editor.is_pending()
                && state.editor.applied().is_some()
                && field.0.text() != value
            {
                let value = field.0.text().to_owned();
                if url {
                    state.editor.edit_url(value);
                } else {
                    state.editor.edit_timeout(value);
                }
            }
            deferred_projection |= state.field_projection
                == ProbeSettingsFieldProjection::NeedsProjection
                && field.0.text() != value;
            if field.0.is_disabled() != disabled {
                field.0.set_disabled(disabled);
            }
            if disabled && focus.0 {
                focus.0 = false;
            }
        }
    }
    // Route scenes can mount after the Core snapshot arrives. Keep the
    // projection dirty until both controlled fields exist, so the first
    // frame that mounts them receives the observed values.
    if synchronized_fields >= 2 && !deferred_projection {
        state.field_projection = ProbeSettingsFieldProjection::Current;
    }
}

pub fn sync_editor(
    route: Res<ActiveRoute>,
    latest: Res<LatestSurfaceSnapshot>,
    mut state: ResMut<ProbeSettingsState>,
) {
    let previous = state.editor.draft().cloned();
    state.editor.observe(&latest.0.probe_settings);
    if state.editor.draft().cloned() != previous {
        state.field_projection.request();
    }
    if route.0 != Some(Route::Proxies) && state.dialog == ProbeSettingsDialogState::Editing {
        if !state.editor.is_pending() {
            state.editor.cancel();
        }
        state.dialog = ProbeSettingsDialogState::Closed;
        state.field_projection.request();
    }
    // Escape is owned by the official dialog layer: the barrier raises the
    // official close request and the page closes through its own close path.
}

pub fn sync_surface(
    locale: Res<UiLocale>,
    state: Res<ProbeSettingsState>,
    mut roots: Query<&mut Node, With<ProbeSettingsRoot>>,
    mut buttons: Query<(&mut ButtonDisabled, Has<ApplyProbeSettings>), ProbeActionButtons>,
    mut text_rows: Query<(&mut Text, Has<ProbeAppliedSummary>), ProbeCopyRows>,
) {
    for mut root in &mut roots {
        let display = match state.dialog {
            ProbeSettingsDialogState::Closed => Display::None,
            ProbeSettingsDialogState::Editing => Display::Flex,
        };
        if root.display != display {
            root.display = display;
        }
    }
    for (mut button, apply) in &mut buttons {
        let disabled = if apply {
            !state.editor.can_apply()
        } else {
            state.editor.is_pending()
        };
        if button.0 != disabled {
            button.0 = disabled;
        }
    }
    let validation = state.editor.validation();
    let message = state
        .editor
        .failure()
        .or(validation.as_ref())
        .map(|failure| failure.message.clone())
        .unwrap_or_else(|| {
            if state.editor.is_pending() {
                LocalizedText::plain("proxy_probe_settings_pending").render(&locale)
            } else {
                String::new()
            }
        });
    let applied = state
        .editor
        .applied()
        .map(|settings| format!("{} · {} ms", settings.url(), settings.timeout_ms()))
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
