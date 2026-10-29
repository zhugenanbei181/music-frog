//! Settings-owned modal scenes and their official close/focus transitions.

use super::{CoreRollbackAvailability, CoreRollbackButton, LastSettingsProjection};
use crate::command::{CommandSinkHandle, UiCommand};
use crate::localized_widgets::localized_button_scene;
use bevy::a11y::AccessibilityNode;
use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::event::EntityEvent;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::observer::On;
use bevy::ecs::query::With;
use bevy::ecs::system::{Commands, Query, Res, ResMut};
use bevy::feathers::theme::{SurfaceLevel, ThemeBackgroundColor, ThemeContext};
use bevy::input_focus::AcquireFocus;
use bevy::input_focus::FocusCause;
use bevy::input_focus::InputFocus;
use bevy::input_focus::tab_navigation::TabIndex;
use bevy::scene::{CommandsSceneExt, Scene, bsn, on};
use bevy::ui::FixedNode;
use bevy::ui::prelude::{
    AlignItems, BorderRadius, Display, FlexDirection, GlobalZIndex, JustifyContent, Node, Overflow,
    PositionType, UiRect, Val, percent, px,
};
use bevy::ui::widget::Text;
use bevy::ui_widgets::{Activate, ModalDialog, ModalDialogBarrier, RequestClose, ScrollArea};
use bevy::window::PrimaryWindow;
use infiltrator_bevy_widgets::button::ButtonVariant;
use infiltrator_bevy_widgets::feathers_theme::Token;
use infiltrator_bevy_widgets::localization::{LocalizedLabel, LocalizedText};
use infiltrator_bevy_widgets::modal::{
    ModalDialogCard, ModalKind, ModalScrim, ModalStage, ModalState,
};
use infiltrator_bevy_widgets::palette::{UiPalette, rem};
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_bevy_widgets::theme::space;
use infiltrator_shared::assets::harmonyos_font_license_text;

/// Marker for the control that opens the full font licence.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FontLicenseButton;

/// Marker on the font-licence dialog's own root.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FontLicenseDialog;

/// Marker for the font-licence dialog's dismiss control.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FontLicenseCloseButton;

/// Root marker for the core-rollback confirmation dialog.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CoreRollbackDialog;

/// Marker for the confirmation dialog's destructive action.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CoreRollbackConfirmButton;

/// Marker for the confirmation dialog's cancel action.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CoreRollbackCancelButton;

/// The current/target version line inside the confirmation dialog.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CoreRollbackDialogMessage;

/// One-off layout value (not a ladder rung): the widest the font-licence
/// dialog card may grow on a wide viewport.
const FONT_LICENSE_MODAL_MAX_WIDTH_PX: f32 = 640.0;

/// One-off layout value (not a ladder rung): the tallest the font-licence
/// dialog card may grow before its own scroll area takes over.
const FONT_LICENSE_MODAL_MAX_HEIGHT_PX: f32 = 480.0;

/// One-off layout value for the rollback confirmation dialog width.
const CORE_ROLLBACK_MODAL_MAX_WIDTH_PX: f32 = 460.0;

/// The font-licence dialog.
///
/// The redistribution agreement is hundreds of lines of all-caps legal
/// English: it must never be laid out down the settings page. It lives in
/// its own dialog, on the official behaviour layer, with its own scroll area.
pub(super) fn font_license_dialog_scene(palette: &UiPalette) -> impl Scene + use<> {
    let semantic = accesskit::Node::new(accesskit::Role::Dialog);
    bsn! {
            Node {
                position_type: PositionType::Absolute,
                width: percent(100), height: percent(100),
                display: Display::None,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
            }
            FontLicenseDialog GlobalZIndex(118)
            // The dialog sits on Bevy's official behaviour layer: the barrier
            // owns the click-outside close and the Escape interception, and
            // every close request travels as the official `RequestClose`.
            ModalDialogBarrier
            on(|close: On<RequestClose>, closes: Query<(Entity, &FontLicenseCloseButton)>, mut commands: Commands| {
                // A close request that names the dialog's own dismiss control
                // is that control's business; every other close request — the
                // barrier's click-outside and the Escape interception — runs
                // the dialog's own close path.
                for (entity, _) in &closes {
                    if entity != close.original_event_target() {
                        commands.trigger(Activate { entity });
                    }
                }
            })
            Children [
                Node { position_type: PositionType::Absolute, width: percent(100), height: percent(100) }
                ModalScrim ThemeBackgroundColor({ Token::Scrim })
                --
                Node {
                    width: percent(90), max_width: rem(FONT_LICENSE_MODAL_MAX_WIDTH_PX),
                    max_height: rem(FONT_LICENSE_MODAL_MAX_HEIGHT_PX),
                    padding: UiRect::all(rem(space::S16)),
                    flex_direction: FlexDirection::Column, row_gap: rem(space::S12),
                    border_radius: BorderRadius::all(rem(palette.modal_radius_px)),
                }
                ModalDialogCard ModalDialog
                AccessibilityNode(semantic) LocalizedLabel::plain("settings_font_attribution_title")
                ThemeBackgroundColor({ Token::Surface })
                Children [
                    LocalizedText::plain("settings_font_attribution_title") TextRole(Role::Heading)
                    --
                    Node {
                        width: percent(100), flex_grow: 1.0, min_height: px(0.0),
                        overflow: Overflow::scroll_y(),
                        padding: UiRect::all(rem(space::S12)),
                        border: UiRect::all(Val::Px(palette.hairline_px)),
                        border_radius: BorderRadius::all(rem(palette.control_radius_px)),
                    }
                    ScrollArea
                    Children [
                        Text({ harmonyos_font_license_text().to_owned() }) TextRole(Role::Mono)
                    ]
                    --
                    Node { width: percent(100), justify_content: JustifyContent::FlexEnd }
                    Children [
                        @{ (localized_button_scene(LocalizedText::plain("关闭"), ButtonVariant::Default, palette), bsn! {
                            FontLicenseCloseButton
                            // The dismiss control asks the official layer to
                            // close; the page never closes the licence on its own.
                            on(|activate: On<Activate>, mut commands: Commands| {
                                commands.trigger(RequestClose { source: activate.entity });
                            })
                        }) }
                    ]
                ]
            ]
    }
}

/// The confirmation dialog is mounted once at page assembly and starts hidden.
/// Its `ModalState` is the only authority for whether it is presented; the
/// official dialog layer owns barrier clicks, Escape, focus containment and
/// close requests.
pub(super) fn core_rollback_dialog_scene(palette: &UiPalette) -> impl Scene + use<> {
    let state = ModalState {
        stage: ModalStage::Hidden,
        title: "settings_core_rollback_title".to_owned(),
        kind: ModalKind::Danger,
    };
    let semantic = accesskit::Node::new(accesskit::Role::Dialog);

    bsn! {
            Node {
                position_type: PositionType::Absolute,
                width: percent(100),
                height: percent(100),
                display: Display::None,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
            }
            CoreRollbackDialog
            ModalScrim
            ModalDialogBarrier
            state
            ThemeContext(SurfaceLevel::Floating)
            FixedNode
            Children [
                Node {
                    width: percent(90),
                    max_width: rem(CORE_ROLLBACK_MODAL_MAX_WIDTH_PX),
                    padding: UiRect::all(rem(space::S16)),
                    flex_direction: FlexDirection::Column,
                    row_gap: rem(space::S12),
                    border: UiRect::all(Val::Px(palette.hairline_px)),
                    border_radius: BorderRadius::all(rem(palette.modal_radius_px)),
                }
                ModalDialogCard
                ModalDialog
                AccessibilityNode(semantic)
                LocalizedLabel::plain("settings_core_rollback_title")
                ThemeBackgroundColor({ Token::Surface })
                Children [
                    LocalizedText::plain("settings_core_rollback_title") TextRole(Role::Heading)
                    --
                    Text(String::new()) CoreRollbackDialogMessage TextRole(Role::Body)
                    --
                    Node {
                        width: percent(100),
                        min_height: px(0.0),
                        justify_content: JustifyContent::FlexEnd,
                        column_gap: rem(space::S8),
                    }
                    Children [
                        @{ (localized_button_scene(LocalizedText::plain("modal_cancel"), ButtonVariant::Default, palette), bsn! {
                            CoreRollbackCancelButton
                            TabIndex(0)
                            on(|activate: On<Activate>, mut commands: Commands| {
                                commands.trigger(RequestClose { source: activate.entity });
                            })
                        }) }
                        --
                        @{ (localized_button_scene(LocalizedText::plain("core_rollback_action"), ButtonVariant::Danger, palette), bsn! {
                            CoreRollbackConfirmButton
                            TabIndex(0)
                            on(|activate: On<Activate>, mut commands: Commands| {
                                commands.trigger(RequestClose { source: activate.entity });
                            })
                        }) }
                    ]
                ]
            ]
    }
}

/// Mount the font-licence dialog once: it is a page-root overlay, so it
/// never lives inside the settings page's own scroll area.
pub(super) fn spawn_font_license_dialog(mut commands: Commands, palette: Res<UiPalette>) {
    commands.spawn_scene(font_license_dialog_scene(&palette));
}

/// Mount the rollback confirmation once; it starts hidden and is opened only
/// by the owner-backed rollback control.
pub(super) fn spawn_core_rollback_dialog(mut commands: Commands, palette: Res<UiPalette>) {
    commands.spawn_scene(core_rollback_dialog_scene(&palette));
}

/// The disclosure control presents the licence dialog.
pub(super) fn on_font_license_open(
    activate: On<Activate>,
    buttons: Query<(), With<FontLicenseButton>>,
    closes: Query<Entity, With<FontLicenseCloseButton>>,
    mut dialogs: Query<&mut Node, With<FontLicenseDialog>>,
    mut focus: ResMut<InputFocus>,
    windows: Query<Entity, With<PrimaryWindow>>,
    mut commands: Commands,
) {
    if !buttons.contains(activate.entity) {
        return;
    }
    for mut node in &mut dialogs {
        if node.display != Display::Flex {
            node.display = Display::Flex;
        }
    }
    // The official focus hand-off: the dialog's dismiss control is the
    // dialog's focusable entity and the official `AcquireFocus` moves the
    // input focus into the dialog, so Bevy's own `ModalDialogBarrier` owns
    // the Escape interception and the focus containment.
    let Ok(control) = closes.single() else {
        return;
    };
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

/// The dismiss control hides the licence dialog again.
pub(super) fn on_font_license_close(
    activate: On<Activate>,
    buttons: Query<(), With<FontLicenseCloseButton>>,
    disclosures: Query<Entity, With<FontLicenseButton>>,
    mut dialogs: Query<&mut Node, With<FontLicenseDialog>>,
    mut focus: ResMut<InputFocus>,
) {
    if !buttons.contains(activate.entity) {
        return;
    }
    for mut node in &mut dialogs {
        if node.display != Display::None {
            node.display = Display::None;
        }
    }
    // The focus leaves the dialog again: it goes back to the disclosure
    // control when the settings page still carries it.
    match disclosures.single() {
        Ok(disclosure) => focus.set(disclosure, FocusCause::Navigated),
        Err(_) => focus.clear(),
    }
}

/// The first rollback activation only presents the confirmation. It never
/// submits the destructive command by itself.
pub(super) fn on_core_rollback_open(
    activate: On<Activate>,
    buttons: Query<(), With<CoreRollbackButton>>,
    availability: Query<&CoreRollbackAvailability, With<CoreRollbackButton>>,
    last: Option<Res<LastSettingsProjection>>,
    mut dialogs: Query<(&mut ModalState, &mut Node), With<CoreRollbackDialog>>,
    mut messages: Query<&mut Text, With<CoreRollbackDialogMessage>>,
    confirms: Query<Entity, With<CoreRollbackConfirmButton>>,
    mut focus: ResMut<InputFocus>,
    windows: Query<Entity, With<PrimaryWindow>>,
    mut commands: Commands,
) {
    if !buttons.contains(activate.entity)
        || !availability
            .get(activate.entity)
            .is_ok_and(|availability| availability.0)
    {
        return;
    }
    let Some(projection) = last.as_ref().and_then(|last| last.0.as_ref()) else {
        return;
    };
    let Some(target) = projection.core_versions.rollback.target.as_deref() else {
        return;
    };
    let current = projection
        .core_versions
        .rollback
        .current
        .as_deref()
        .unwrap_or("?");
    let message = format!("{current} -> {target}");

    for (mut state, mut node) in &mut dialogs {
        state.open();
        node.display = Display::Flex;
    }
    for mut message_text in &mut messages {
        message_text.0 = message.clone();
    }

    let Ok(confirm) = confirms.single() else {
        return;
    };
    commands
        .entity(confirm)
        .entry::<TabIndex>()
        .or_insert(TabIndex(0));
    if let Ok(window) = windows.single() {
        commands.trigger(AcquireFocus {
            focused_entity: confirm,
            window,
        });
    }
    focus.set(confirm, FocusCause::Navigated);
}

/// Official close requests resolve the confirmation. Only a request sourced
/// by the confirm control submits `RollbackCore`; cancel, barrier click and
/// Escape only dismiss the dialog.
pub(super) fn on_core_rollback_request_close(
    close: On<RequestClose>,
    dialogs: Query<(), With<CoreRollbackDialog>>,
    confirms: Query<(), With<CoreRollbackConfirmButton>>,
    mut states: Query<&mut ModalState, With<CoreRollbackDialog>>,
    handle: Option<Res<CommandSinkHandle>>,
    mut focus: ResMut<InputFocus>,
) {
    if !dialogs.contains(close.event_target()) {
        return;
    }
    let Ok(mut state) = states.get_mut(close.event_target()) else {
        return;
    };
    if !state.is_open() {
        return;
    }
    if confirms.contains(close.original_event_target()) {
        let Some(handle) = handle else {
            return;
        };
        handle.submit(UiCommand::RollbackCore);
        state.hide();
    } else {
        state.dismiss();
    }
    focus.clear();
}
