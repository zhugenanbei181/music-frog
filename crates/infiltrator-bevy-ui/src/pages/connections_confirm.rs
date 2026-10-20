//! Close-all confirmation is a visible, cancellable modal, never a second-click latch.

use crate::command::{CommandSinkHandle, UiCommand};
use crate::pages::connections::CloseAllConnectionsButton;
use crate::pages::connections_view::ConnectionsCloseAllState;
use crate::route::{ActiveRoute, Route, RouteChanged};
use bevy::ecs::prelude::*;
use bevy::feathers::theme::ThemeBackgroundColor;
use bevy::input_focus::tab_navigation::TabIndex;
use bevy::input_focus::{AcquireFocus, FocusCause, InputFocus};
use bevy::scene::{Scene, bsn, on};
use bevy::ui::prelude::*;
use bevy::ui::widget::Text;
use bevy::ui_widgets::{
    Activate, Button, ModalDialog, ModalDialogBarrier, RequestClose, ScrollArea,
};
use bevy::window::PrimaryWindow;
use infiltrator_bevy_widgets::feathers_theme::Token;
use infiltrator_bevy_widgets::localization::UiLocale;
use infiltrator_bevy_widgets::modal::{ModalDialogCard, ModalScrim};
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::palette::rem;
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_bevy_widgets::theme::space;
use infiltrator_shared::locales::{Lang, Localizer};

/// The confirmation dialog's fixed width in baseline pixels.
const CONFIRM_DIALOG_WIDTH_PX: f32 = 420.0;

#[derive(Event, Clone, Copy, Debug)]
pub struct RequestCloseAllConfirmation;

pub(crate) fn request_confirmation(
    _request: On<RequestCloseAllConfirmation>,
    mut state: ResMut<ConnectionsCloseAllState>,
    mut commands: Commands,
) {
    state.request();
    commands.trigger(RouteChanged(Route::Connections));
}

#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CloseAllConfirmationRoot;

#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CloseAllConfirmationCard;

#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CloseAllConfirmationAction {
    #[default]
    Cancel,
    Confirm,
}

pub fn confirmation_scene(palette: &UiPalette) -> impl Scene + use<> {
    let language = UiLocale::default().code().to_string();
    let lang = Lang(&language);
    let title = lang.tr("modal_confirm_disconnect_all_title").into_owned();
    let detail = lang.tr("modal_confirm_disconnect_all_desc").into_owned();
    let cancel = lang.tr("modal_cancel").into_owned();
    let confirm = lang.tr("modal_confirm_disconnect_all_btn").into_owned();
    bsn! {
        Node {
            position_type: PositionType::Absolute,
            left: px(0.0), top: px(0.0),
            width: percent(100), height: percent(100),
            display: Display::None,
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
        }
        GlobalZIndex(100)
        CloseAllConfirmationRoot
        // The confirmation is a modal dialog on Bevy's official behaviour
        // layer: the barrier owns the click-outside close and the Escape
        // interception, the card owns the focus containment, and every close
        // request travels as the official `RequestClose`.
        //
        // The barrier is the confirmation root — the only common ancestor of
        // the scrim and the card — because the official Escape interception
        // reaches the barrier through the focused entity and the official
        // `RequestClose` reaches it through the closing control, both of
        // which live under the card.
        ModalDialogBarrier
        on(|close: On<RequestClose>, cancels: Query<(Entity, &CloseAllConfirmationAction), Without<ModalScrim>>, mut commands: Commands| {
            // A close request that names the confirmation's own cancel control
            // is that control's business (it already asked to close); every
            // other close request — the barrier's click-outside and the Escape
            // interception — runs the confirmation's own close path.
            for (entity, action) in &cancels {
                if matches!(action, CloseAllConfirmationAction::Cancel) && entity != close.original_event_target() {
                    commands.trigger(Activate { entity });
                }
            }
        })
        Children [
            Node {
                position_type: PositionType::Absolute,
                width: percent(100), height: percent(100),
            }
            ThemeBackgroundColor({ Token::Scrim })
            ModalScrim
            CloseAllConfirmationAction::Cancel
            --
            Node {
                width: rem(CONFIRM_DIALOG_WIDTH_PX), max_width: percent(90),
                max_height: percent(90),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(rem(space::S20)), row_gap: rem(space::S16),
                // The contract's radius ladder gives a centered dialog the
                // modal rung; only a panel-style card keeps the card rung.
                border_radius: BorderRadius::all(rem(palette.modal_radius_px)),
                overflow: Overflow::scroll_y(),
            }
            ThemeBackgroundColor({ Token::Surface })
            CloseAllConfirmationCard
            ModalDialogCard
            ModalDialog
            ScrollArea
            Children [
                Text(title) TextRole(Role::Heading)
                --
                Text(detail) TextRole(Role::Body)
                --
                Node {
                    width: percent(100), column_gap: rem(space::S12),
                    justify_content: JustifyContent::FlexEnd,
                }
                Children [
                    Node {
                        min_height: rem(palette.control_height_px),
                        padding: UiRect::axes(rem(space::S16), rem(space::S8)),
                        align_items: AlignItems::Center,
                    }
                    ThemeBackgroundColor({ Token::SurfaceElevated })
                    Button CloseAllConfirmationAction::Cancel
                    // The dismiss control is the dialog's focusable control:
                    // the official `TabIndex` marks it for Bevy's focus
                    // scheme, so the focus hand-off can move the input focus
                    // into the confirmation.
                    TabIndex(0)
                    // The cancel control asks the official layer to close; the
                    // barrier observer runs the confirmation's own close path.
                    on(|activate: On<Activate>, mut commands: Commands| {
                        commands.trigger(RequestClose { source: activate.entity });
                    })
                    Children [ Text(cancel) TextRole(Role::Body) ]
                    --
                    Node {
                        min_height: rem(palette.control_height_px),
                        padding: UiRect::axes(rem(space::S16), rem(space::S8)),
                        align_items: AlignItems::Center,
                    }
                    ThemeBackgroundColor({ Token::Danger })
                    Button CloseAllConfirmationAction::Confirm
                    Children [ Text(confirm) TextRole(Role::BodyStrong) ]
                ]
            ]
        ]
    }
}

pub(crate) fn on_confirmation_activated(
    activate: On<Activate>,
    open_buttons: Query<(), With<CloseAllConnectionsButton>>,
    actions: Query<(Entity, &CloseAllConfirmationAction), Without<ModalScrim>>,
    mut roots: Query<&mut Node, With<CloseAllConfirmationRoot>>,
    mut state: Option<ResMut<ConnectionsCloseAllState>>,
    handle: Option<Res<CommandSinkHandle>>,
    mut commands: Commands,
    mut focus: ResMut<InputFocus>,
    windows: Query<Entity, With<PrimaryWindow>>,
) {
    let Some(state) = state.as_deref_mut() else {
        return;
    };
    // A missing modal must never fall through to immediate destruction.
    let Ok(mut root) = roots.single_mut() else {
        return;
    };
    if open_buttons.contains(activate.entity) {
        state.arm();
        root.display = Display::Flex;
        // The confirmation is presented: the official focus hand-off moves
        // the input focus into the dialog, so Bevy's own
        // `ModalDialogBarrier` owns the Escape interception.
        if let Some(control) = dismiss_control(&actions) {
            hand_off_focus(&mut commands, &mut focus, &windows, control);
        }
    } else if state.is_armed()
        && let Ok((_, action)) = actions.get(activate.entity)
    {
        state.disarm();
        root.display = Display::None;
        // The confirmation is closed: the focus leaves the dialog again.
        release_focus(&mut focus, dismiss_control(&actions));
        if *action == CloseAllConfirmationAction::Confirm
            && let Some(handle) = handle
        {
            handle.submit(UiCommand::CloseAllConnections);
        }
    }
}

/// The confirmation's dismiss control: the one cancel control that is not the
/// scrim.
fn dismiss_control(
    actions: &Query<(Entity, &CloseAllConfirmationAction), Without<ModalScrim>>,
) -> Option<Entity> {
    actions
        .iter()
        .find(|(_, action)| matches!(action, CloseAllConfirmationAction::Cancel))
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

/// The confirmation closes when the user leaves the Connections page.
///
/// The Escape itself belongs to Bevy's official dialog layer now: the
/// barrier reports the close request and the confirmation's own close path
/// runs from it, so this system only owns the navigation half.
pub(crate) fn cancel_on_escape_or_navigation(
    route: Res<ActiveRoute>,
    mut state: ResMut<ConnectionsCloseAllState>,
    mut roots: Query<&mut Node, With<CloseAllConfirmationRoot>>,
    actions: Query<(Entity, &CloseAllConfirmationAction), Without<ModalScrim>>,
    mut commands: Commands,
    mut focus: ResMut<InputFocus>,
    windows: Query<Entity, With<PrimaryWindow>>,
) {
    if state.is_requested()
        && route.0 == Some(Route::Connections)
        && let Ok(mut root) = roots.single_mut()
    {
        state.arm();
        root.display = Display::Flex;
        // The confirmation is presented: the official focus hand-off moves
        // the input focus into the dialog, so Bevy's own
        // `ModalDialogBarrier` owns the Escape interception.
        if let Some(control) = dismiss_control(&actions) {
            hand_off_focus(&mut commands, &mut focus, &windows, control);
        }
    }
    if state.is_armed() && route.0 != Some(Route::Connections) {
        state.disarm();
        for mut root in &mut roots {
            root.display = Display::None;
        }
        // The confirmation is closed: the focus leaves the dialog again.
        release_focus(&mut focus, dismiss_control(&actions));
    }
}
