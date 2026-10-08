//! Close-all confirmation is a visible, cancellable modal, never a second-click latch.

use crate::command::{CommandSinkHandle, UiCommand};
use crate::pages::connections::CloseAllConnectionsButton;
use crate::pages::connections_view::ConnectionsCloseAllState;
use crate::route::{ActiveRoute, Route, RouteChanged};
use bevy::ecs::prelude::*;
use bevy::input::ButtonInput;
use bevy::input::keyboard::KeyCode;
use bevy::scene::{Scene, bsn};
use bevy::ui::prelude::*;
use bevy::ui::widget::Text;
use bevy::ui_widgets::{Activate, Button};
use infiltrator_bevy_widgets::localization::UiLocale;
use infiltrator_bevy_widgets::modal::{ModalDialogCard, ModalScrim};
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_shared::locales::{Lang, Localizer};

#[derive(Event, Clone, Copy, Debug)]
pub struct RequestCloseAllConfirmation;

pub(crate) fn request_confirmation(
    _request: On<RequestCloseAllConfirmation>,
    mut state: ResMut<ConnectionsCloseAllState>,
    mut commands: Commands,
) {
    state.requested = true;
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
        Children [
            Node {
                position_type: PositionType::Absolute,
                width: percent(100), height: percent(100),
            }
            BackgroundColor({ palette.scrim })
            ModalScrim
            Button
            CloseAllConfirmationAction::Cancel
            --
            Node {
                width: px(420.0), max_width: percent(90),
                max_height: percent(90),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(px(20.0)), row_gap: px(16.0),
                border_radius: BorderRadius::all(px(palette.card_radius_px)),
                overflow: Overflow::scroll_y(),
            }
            BackgroundColor({ palette.surface })
            CloseAllConfirmationCard
            ModalDialogCard
            Children [
                Text(title) TextRole(Role::Heading)
                --
                Text(detail) TextRole(Role::Body)
                --
                Node {
                    width: percent(100), column_gap: px(12.0),
                    justify_content: JustifyContent::FlexEnd,
                }
                Children [
                    Node {
                        min_height: px(palette.control_height_px),
                        padding: UiRect::axes(px(16.0), px(8.0)),
                        align_items: AlignItems::Center,
                    }
                    BackgroundColor({ palette.surface_elevated })
                    Button CloseAllConfirmationAction::Cancel
                    Children [ Text(cancel) TextRole(Role::Body) ]
                    --
                    Node {
                        min_height: px(palette.control_height_px),
                        padding: UiRect::axes(px(16.0), px(8.0)),
                        align_items: AlignItems::Center,
                    }
                    BackgroundColor({ palette.danger })
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
    actions: Query<&CloseAllConfirmationAction>,
    mut roots: Query<&mut Node, With<CloseAllConfirmationRoot>>,
    mut state: Option<ResMut<ConnectionsCloseAllState>>,
    handle: Option<Res<CommandSinkHandle>>,
) {
    let Some(state) = state.as_deref_mut() else {
        return;
    };
    // A missing modal must never fall through to immediate destruction.
    let Ok(mut root) = roots.single_mut() else {
        return;
    };
    if open_buttons.contains(activate.entity) {
        state.requested = false;
        state.armed = true;
        root.display = Display::Flex;
    } else if state.armed
        && let Ok(action) = actions.get(activate.entity)
    {
        state.armed = false;
        root.display = Display::None;
        if *action == CloseAllConfirmationAction::Confirm
            && let Some(handle) = handle
        {
            handle.submit(UiCommand::CloseAllConnections);
        }
    }
}

pub(crate) fn cancel_on_escape_or_navigation(
    keys: Option<Res<ButtonInput<KeyCode>>>,
    route: Res<ActiveRoute>,
    mut state: ResMut<ConnectionsCloseAllState>,
    mut roots: Query<&mut Node, With<CloseAllConfirmationRoot>>,
) {
    if state.requested
        && route.0 == Some(Route::Connections)
        && let Ok(mut root) = roots.single_mut()
    {
        state.requested = false;
        state.armed = true;
        root.display = Display::Flex;
    }
    if state.armed
        && (route.0 != Some(Route::Connections)
            || keys.is_some_and(|keys| keys.just_pressed(KeyCode::Escape)))
    {
        state.armed = false;
        state.requested = false;
        for mut root in &mut roots {
            root.display = Display::None;
        }
    }
}
