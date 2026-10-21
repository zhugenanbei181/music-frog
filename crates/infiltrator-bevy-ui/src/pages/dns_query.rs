//! A native modal owns a scoped draft and replays the one shared query state machine.
use crate::command::{CommandSinkHandle, UiCommand};
use crate::command_events::CommandExecutedEvent;
use crate::pages::dns_self_heal::observation_color;
use crate::route::{ActiveRoute, Route, RouteChanged};
use crate::surface::LatestSurfaceSnapshot;
use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::observer::On;
use bevy::ecs::query::{QueryData, With, Without};
use bevy::ecs::resource::Resource;
use bevy::ecs::system::{Commands, Query, Res, ResMut};
use bevy::input::ButtonInput;
use bevy::input::keyboard::KeyCode;
use bevy::input_focus::tab_navigation::TabIndex;
use bevy::input_focus::{AcquireFocus, FocusCause, InputFocus};
use bevy::text::TextColor;
use bevy::ui::widget::Text;
use bevy::ui::{Display, Node};
use bevy::ui_widgets::{Activate, Button};
use bevy::window::PrimaryWindow;
use infiltrator_bevy_widgets::button::{ButtonDisabled, ButtonVariant, ButtonVariantStyle};
use infiltrator_bevy_widgets::localization::{LocalizedText, UiLocale};
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::text_input::{TextField, TextFieldFocused};
use infiltrator_contract::command::RequestId;
use infiltrator_contract::dns_query::{DnsQueryOperationId, DnsRecordType};
use infiltrator_presentation::dns_query_actions::{DnsQueryActions, QuerySection};
use infiltrator_presentation::dns_query_projection::project_query;
use infiltrator_shared::locales::Lang;
use lurker_core_contract::error::{ErrorCode, Failure};

#[derive(Resource, Default)]
pub struct QueryPanel {
    pub model: DnsQueryActions,
    pub request: Option<(RequestId, DnsQueryOperationId)>,
    pub kind_open: bool,
    pub restore_focus: Vec<Entity>,
}
#[derive(Component, Clone, Copy, Default)]
pub struct QueryModalRoot;
#[derive(Component, Clone, Copy, Default)]
pub struct QueryModalCard;
#[derive(Component, Clone, Copy, Default)]
pub struct QueryNameField;
#[derive(Component, Clone, Copy, Default)]
pub struct QueryTypeMenu;
#[derive(Component, Clone, Copy, PartialEq, Eq, Default)]
#[require(Button, ButtonDisabled)]
pub enum QueryAction {
    #[default]
    Open,
    Cancel,
    Settings,
    Run,
    Retry,
    TypePicker,
    Type(DnsRecordType),
    Section(QuerySection),
    Previous,
    Next,
}
#[derive(Component, Clone, Copy, Default, PartialEq, Eq)]
pub enum QueryLine {
    #[default]
    Status,
    Provenance,
    Question,
    Flags,
    Records,
    Counter,
    RecordType,
    /// The inspected section's own name: the record listing says which
    /// section it shows.
    SectionLabel,
}
pub fn activate(
    event: On<Activate>,
    actions: Query<(Entity, &QueryAction)>,
    route: Res<ActiveRoute>,
    mut state: ResMut<QueryPanel>,
    sink: Option<Res<CommandSinkHandle>>,
    mut fields: Query<(
        Entity,
        &TextField,
        &mut TextFieldFocused,
        Option<&QueryNameField>,
    )>,
    mut commands: Commands,
    mut input_focus: ResMut<InputFocus>,
    windows: Query<Entity, With<PrimaryWindow>>,
) {
    if route.0 != Some(Route::Dns) {
        return;
    }
    let Ok((_, action)) = actions.get(event.entity) else {
        return;
    };
    match action {
        QueryAction::Open if !state.model.is_pending() && !state.model.is_open() => {
            state.model.show();
            state.restore_focus.clear();
            for (entity, _, mut focus, own) in &mut fields {
                if focus.0 && own.is_none() {
                    state.restore_focus.push(entity);
                }
                focus.0 = own.is_some();
            }
            // The modal is presented: the official focus hand-off moves the
            // input focus into the dialog, so Bevy's own `ModalDialogBarrier`
            // owns the Escape interception.
            if let Some(control) = dismiss_control(&actions) {
                hand_off_focus(&mut commands, &mut input_focus, &windows, control);
            }
            return;
        }
        QueryAction::Cancel => {
            if state.kind_open {
                state.kind_open = false;
                return;
            }
            if state.model.cancel() {
                for (entity, field, mut focus, own) in &mut fields {
                    if own.is_some() {
                        focus.0 = false;
                    } else if state.restore_focus.contains(&entity) && !field.0.is_disabled() {
                        focus.0 = true;
                    }
                }
                state.restore_focus.clear();
                // The modal is closed: the focus leaves the dialog again.
                release_focus(&mut input_focus, dismiss_control(&actions));
            }
            return;
        }
        _ if !state.model.is_open() => return,
        QueryAction::TypePicker if !state.model.is_pending() => {
            state.kind_open = !state.kind_open;
            return;
        }
        QueryAction::Settings => {
            if state.model.can_guide() {
                state.model.cancel();
                state.kind_open = false;
                for (_, _, mut focused, own) in &mut fields {
                    if own.is_some() {
                        focused.0 = false;
                    }
                }
                state.restore_focus.clear();
                commands.trigger(RouteChanged(Route::Settings));
            }
            return;
        }
        QueryAction::Type(_) if !state.kind_open => return,
        QueryAction::Type(kind) => {
            state.kind_open = false;
            state.model.set_type(*kind);
            return;
        }
        QueryAction::Section(section) => {
            state.model.select(*section);
            return;
        }
        QueryAction::Previous => {
            state.model.previous();
            return;
        }
        QueryAction::Next => {
            state.model.next();
            return;
        }
        QueryAction::Retry if !state.model.can_retry() => return,
        QueryAction::Run | QueryAction::Retry => {}
        QueryAction::Open | QueryAction::TypePicker => return,
    }
    state.kind_open = false;
    let Ok((token, request)) = state.model.begin() else {
        return;
    };
    if let Some(id) = sink.and_then(|sink| {
        sink.submit_tracked(UiCommand::QueryDns {
            operation: token,
            request,
        })
    }) {
        state.request = Some((id, token));
    } else {
        state.model.finish(
            token,
            Err(Failure::new(
                ErrorCode::NotReady,
                "DNS query command service has no terminal response",
                true,
            )),
        );
    }
}
pub fn finish(event: On<CommandExecutedEvent>, mut state: ResMut<QueryPanel>) {
    let Some((id, token)) = state.request else {
        return;
    };
    let UiCommand::QueryDns { operation, .. } = &event.command else {
        return;
    };
    if event.request_id == id
        && *operation == token
        && state.model.finish(token, event.unit_result())
    {
        state.request = None;
    }
}
pub fn observe(
    latest: Res<LatestSurfaceSnapshot>,
    route: Res<ActiveRoute>,
    keys: Option<Res<ButtonInput<KeyCode>>>,
    mut state: ResMut<QueryPanel>,
    mut fields: Query<(
        Entity,
        &mut TextField,
        &mut TextFieldFocused,
        Option<&QueryNameField>,
    )>,
    actions: Query<(Entity, &QueryAction)>,
    mut input_focus: ResMut<InputFocus>,
) {
    state.model.observe(&latest.0.dns_query);
    let composing = fields
        .iter()
        .any(|(_, field, _, own)| own.is_some() && field.0.is_in_ime_transaction());
    // The record-type picker is a sub-menu stage of the query modal, not the
    // modal itself: it has no barrier of its own, so its own Escape arm stays
    // — the modal's Escape belongs to Bevy's official dialog layer.
    if state.kind_open
        && !composing
        && keys
            .as_ref()
            .is_some_and(|keys| keys.just_pressed(KeyCode::Escape))
    {
        state.kind_open = false;
        return;
    }
    // The modal's Escape interception belongs to Bevy's official dialog
    // layer: the barrier reports the close request and the modal's own close
    // path runs from it. This system only owns the page navigation.
    let leaving = route.0 != Some(Route::Dns);
    if leaving && state.model.cancel() {
        for (entity, field, mut focus, own) in &mut fields {
            if own.is_some() {
                focus.0 = false;
            } else if state.restore_focus.contains(&entity) && !field.0.is_disabled() {
                focus.0 = true;
            }
        }
        state.restore_focus.clear();
        // The modal is closed: the focus leaves the dialog again.
        release_focus(&mut input_focus, dismiss_control(&actions));
    }
    for (_, mut field, _, own) in &mut fields {
        if own.is_none() {
            continue;
        }
        let disabled = !state.model.is_open() || state.model.is_pending();
        if field.0.is_disabled() != disabled {
            field.0.set_disabled(disabled);
        }
        if state.model.is_open() && !state.model.is_pending() {
            state.model.set_name(field.0.text().to_string());
        }
    }
}
/// The modal's dismiss control: the one close control of the query modal.
fn dismiss_control(actions: &Query<(Entity, &QueryAction)>) -> Option<Entity> {
    actions
        .iter()
        .find(|(_, action)| matches!(action, QueryAction::Cancel))
        .map(|(entity, _)| entity)
}

/// The official focus hand-off.
///
/// The dialog's dismiss control is the dialog's focusable control — the
/// official `TabIndex` marks it for Bevy's focus scheme — and the official
/// `AcquireFocus` request asks that scheme to move the input focus into the
/// dialog. The official focus resource carries the result, so the page and
/// the official layer read the same fact. While the focus lives inside the
/// barrier, Bevy's own `ModalDialogBarrier` owns the Escape interception.
fn hand_off_focus(
    commands: &mut Commands,
    focus: &mut InputFocus,
    windows: &Query<Entity, With<PrimaryWindow>>,
    control: Entity,
) {
    commands
        .entity(control)
        .entry::<TabIndex>()
        .or_insert(TabIndex(0));
    if let Ok(window) = windows.single() {
        commands.trigger(AcquireFocus {
            focused_entity: control,
            window,
        });
    }
    focus.set(control, FocusCause::Navigated);
}

/// Release the input focus the hand-off moved into the dialog.
fn release_focus(focus: &mut InputFocus, control: Option<Entity>) {
    if let Some(control) = control
        && focus.get() == Some(control)
    {
        focus.clear();
    }
}

#[derive(QueryData)]
#[query_data(mutable)]
pub struct QueryControl {
    action: &'static QueryAction,
    disabled: &'static mut ButtonDisabled,
    variant: &'static mut ButtonVariantStyle,
    node: &'static mut Node,
}
pub fn render(
    state: Res<QueryPanel>,
    locale: Res<UiLocale>,
    palette: Res<UiPalette>,
    mut roots: Query<&mut Node, (With<QueryModalRoot>, Without<QueryTypeMenu>)>,
    mut menus: Query<&mut Node, (With<QueryTypeMenu>, Without<QueryModalRoot>)>,
    mut controls: Query<QueryControl, (Without<QueryModalRoot>, Without<QueryTypeMenu>)>,
    mut lines: Query<(&QueryLine, &mut Text, &mut TextColor)>,
) {
    let view = project_query(&state.model, &Lang(locale.code()));
    let section_label = LocalizedText::plain(state.model.section.key()).render(&locale);
    for mut node in &mut roots {
        let display = if state.model.is_open() {
            Display::Flex
        } else {
            Display::None
        };
        if node.display != display {
            node.display = display;
        }
    }
    for mut menu in &mut menus {
        let display = if state.model.is_open() && state.kind_open {
            Display::Flex
        } else {
            Display::None
        };
        if menu.display != display {
            menu.display = display;
        }
    }
    for mut control in &mut controls {
        let enabled = match control.action {
            QueryAction::Open => !state.model.is_open(),
            QueryAction::Cancel => view.close,
            QueryAction::Settings => view.guide,
            QueryAction::Run => view.run,
            QueryAction::Retry => view.retry,
            QueryAction::TypePicker => view.run,
            QueryAction::Type(_) => view.run && state.kind_open,
            QueryAction::Section(_) => true,
            QueryAction::Previous => view.previous,
            QueryAction::Next => view.next,
        } && (matches!(control.action, QueryAction::Open) || state.model.is_open());
        if control.disabled.0 == enabled {
            control.disabled.0 = !enabled;
        }
        let shown = match control.action {
            QueryAction::Retry => view.retry,
            QueryAction::Settings => view.guide,
            QueryAction::Run => !view.retry,
            _ => true,
        };
        let display = if shown { Display::Flex } else { Display::None };
        if control.node.display != display {
            control.node.display = display;
        }
        let selected = matches!(control.action, QueryAction::Type(kind) if *kind == state.model.record_type)
            || matches!(control.action, QueryAction::Section(section) if *section == state.model.section);
        let variant = if selected || matches!(control.action, QueryAction::Run | QueryAction::Retry)
        {
            ButtonVariant::Primary
        } else {
            ButtonVariant::Default
        };
        if control.variant.0 != variant {
            control.variant.0 = variant;
        }
    }
    for (line, mut text, mut color) in &mut lines {
        let kind = state.model.record_type.wire().to_string();
        let wanted = match line {
            QueryLine::Status => &view.status,
            QueryLine::Provenance => &view.provenance,
            QueryLine::Question => &view.question,
            QueryLine::Flags => &view.flags,
            QueryLine::Records => &view.records,
            QueryLine::Counter => &view.counter,
            QueryLine::RecordType => &kind,
            QueryLine::SectionLabel => &section_label,
        };
        if &text.0 != wanted {
            text.0 = wanted.clone();
        }
        let wanted_color = if matches!(line, QueryLine::Status) {
            observation_color(view.tone, &palette)
        } else {
            palette.ink
        };
        if color.0 != wanted_color {
            color.0 = wanted_color;
        }
    }
}
