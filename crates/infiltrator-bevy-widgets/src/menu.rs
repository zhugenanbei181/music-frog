//! Menu: our product skin over Bevy 0.20's official headless menu widgets.
//!
//! **Official behaviour layer**: `bevy_ui_widgets`' `MenuButton` / `MenuPopup`
//! / `MenuItem` own the behaviour. The popup opens from its trigger, the
//! arrow keys walk the items, `Enter` / `Space` / a click activate one,
//! `Escape` closes the popup and hands the focus back to the trigger, and the
//! popup closes as soon as the focus leaves it. The official `MenuEvent` /
//! `MenuAction` pair is the control channel and the official `MenuFocusState`
//! (`Opening` / `Open` / `Closed`) is the popup's own focus bookkeeping.
//!
//! **Product layer**: this module owns the entry description ([`MenuEntry`]),
//! the token skin (the panel chrome, the row fills, the dim disabled ink) and
//! the host-facing typed outcome ([`MenuOutcome`]). The outcome is derived
//! from the official events, never from a navigation state machine of our
//! own: there is no local arrow-key handling, no local selection cursor and
//! no local focus state here — the keyboard navigation, the activation and
//! the dismissal are the official ones.
//!
//! The keyboard reaches the focused entity through Bevy's
//! `InputDispatchPlugin` (the windowed host carries it through
//! `DefaultPlugins`); the pointer focus — the way the official layer detects
//! a click outside the popup — comes from `PointerFocusPlugin`; and the
//! inherited visibility must be resolved (the official focus bookkeeping only
//! reaches the entries of a visible popup), which the windowed host gets
//! from `DefaultPlugins` too.

use crate::button::{ButtonVariant, button_scene};
use crate::palette::{UiPalette, rem};
use crate::text::{Role, TextRole};
use crate::theme::space;
use bevy::app::{App, Plugin, Update};
use bevy::camera::visibility::Visibility;
use bevy::color::Color;
use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::event::EntityEvent;
use bevy::ecs::hierarchy::{ChildOf, Children};
use bevy::ecs::message::{Message, MessageWriter};
use bevy::ecs::observer::On;
use bevy::ecs::query::{Has, QueryFilter, With, Without};
use bevy::ecs::system::{Commands, Query, Res, ResMut};
use bevy::feathers::theme::{SurfaceLevel, ThemeContext};
use bevy::input_focus::pointer_focus::PointerFocusPlugin;
use bevy::input_focus::tab_navigation::{NavAction, TabIndex};
use bevy::input_focus::{FocusCause, InputFocus, InputFocusPlugin};
use bevy::picking::hover::PickingInteraction;
use bevy::scene::{Scene, bsn, on};
use bevy::ui::prelude::{
    AlignItems, BackgroundColor, BorderRadius, Display, FlexDirection, Node, PositionType, UiRect,
    Val, percent, px,
};
use bevy::ui::widget::Text;
use bevy::ui::{BorderColor, InteractionDisabled};
use bevy::ui_widgets::popover::{
    Popover, PopoverAlign, PopoverPlacement, PopoverPlugin, PopoverSide,
};
use bevy::ui_widgets::{
    MenuAction, MenuButton, MenuEvent, MenuFocusState, MenuItem, MenuLayout, MenuPlugin, MenuPopup,
};

/// Panel width (px) — a context menu is a word-scale surface, not a card.
pub const MENU_WIDTH: f32 = 220.0;

/// The gap between the trigger and the popup (logical px): the official
/// `Popover` placement takes its gap in logical pixels, not in the rem
/// channel.
const MENU_POPOVER_GAP_PX: f32 = space::S4;

/// How close to the window edge the popup may come (logical px), again an
/// official `Popover` quantity.
const MENU_POPOVER_MARGIN_PX: f32 = space::S8;

/// One menu entry. A disabled item renders caption-dim and never confirms
/// (honest unavailable state — never hidden, never faked as activatable).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MenuEntry {
    Item { label: String, enabled: bool },
    Separator,
}

impl MenuEntry {
    /// An enabled, activatable item.
    pub fn item(label: impl Into<String>) -> Self {
        Self::Item {
            label: label.into(),
            enabled: true,
        }
    }

    /// A disabled item: visible, dimmed, never confirms.
    pub fn disabled(label: impl Into<String>) -> Self {
        Self::Item {
            label: label.into(),
            enabled: false,
        }
    }
}

/// The host-facing menu outcome, derived from the official menu events.
///
/// `Confirmed` carries the entry index in the menu's entry list; `Canceled`
/// reports that the official layer closed the menu without activating an
/// entry.
#[derive(Message, Clone, Copy, Debug, PartialEq, Eq)]
pub enum MenuOutcome {
    /// The official layer activated an entry: the entry index in the menu's
    /// entry list.
    Confirmed(usize),
    /// The official layer dismissed the menu without activating an entry.
    Canceled,
}

/// Marker on the menu owner: the node that holds the trigger and the popup
/// and observes the official `MenuEvent` channel.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MenuOwner;

/// Marker on the trigger button (the official `MenuButton` node).
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MenuTrigger;

/// Marker on the popup panel (the official `MenuPopup` node).
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MenuPanel;

/// Marker on one item row, carrying its entry index in the menu's entry
/// list; the outcome names the activated entry by this index.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MenuRowIndex(pub usize);

/// Marker on a separator's bar node; it paints the border token.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MenuSeparator;

/// The availability of one menu entry. A disabled entry stays visible and
/// reachable — the official layer never activates it, so the unavailable
/// state is honest instead of faked.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MenuItemDisabled(pub bool);

/// The row fill: the highlighted row paints the hover token (the hover
/// color, used deliberately as the keyboard-highlight semantic so mouse and
/// keyboard agree on one vocabulary); every other row paints the panel
/// surface, i.e. nothing visible. Pure function.
pub fn menu_row_fill(highlighted: bool, palette: &UiPalette) -> Color {
    if highlighted {
        palette.hover_bg
    } else {
        palette.surface
    }
}

/// The official menu behaviour layer.
///
/// Installs Bevy's own `MenuPlugin` — the popup focus bookkeeping, the arrow
/// navigation, the activation and the dismissal — plus the popup placement
/// (`PopoverPlugin`, the viewport-flipping placement) and the pointer focus
/// the official layer uses to notice a click outside the popup.
///
/// The official focus bookkeeping only reaches the entries of a *visible*
/// popup, so the host must run Bevy's visibility propagation (the windowed
/// host carries it through `DefaultPlugins`).
pub struct MenuWidgetsPlugin;

impl Plugin for MenuWidgetsPlugin {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<InputFocusPlugin>() {
            app.add_plugins(InputFocusPlugin);
        }
        if !app.is_plugin_added::<PointerFocusPlugin>() {
            app.add_plugins(PointerFocusPlugin);
        }
        if !app.is_plugin_added::<PopoverPlugin>() {
            app.add_plugins(PopoverPlugin);
        }
        if !app.is_plugin_added::<MenuPlugin>() {
            app.add_plugins(MenuPlugin);
        }
        app.add_systems(Update, sync_menu_item_disabled);
    }
}

/// The whole menu: the owner, the trigger and the popup.
///
/// The owner is the node the official `MenuEvent` bubbles to; it observes
/// the official channel and answers with the official vocabulary. The trigger
/// carries the official `MenuButton`, the popup the official `MenuPopup` and
/// every row the official `MenuItem` — so the keyboard navigation, the
/// activation and the dismissal are the official ones.
///
/// The popup is mounted closed (the official `MenuFocusState::Closed`, hidden
/// in the layout): opening it is the official `MenuAction::Open` handshake,
/// which the official layer answers by taking the focus to the first (or
/// last) entry.
pub fn menu_scene(
    label: String,
    entries: Vec<MenuEntry>,
    palette: &UiPalette,
) -> impl Scene + use<> {
    let rows: Vec<Box<dyn Scene>> = entries
        .iter()
        .enumerate()
        .map(|(index, entry)| menu_row_scene(entry, index, palette))
        .collect();
    let edge = palette.border;
    let placements = vec![
        PopoverPlacement {
            side: PopoverSide::Bottom,
            align: PopoverAlign::Start,
            gap: MENU_POPOVER_GAP_PX,
        },
        PopoverPlacement {
            side: PopoverSide::Top,
            align: PopoverAlign::Start,
            gap: MENU_POPOVER_GAP_PX,
        },
    ];
    bsn! {
            Node {
                position_type: PositionType::Relative,
                width: Val::Auto,
            }
            MenuOwner
            on(on_menu_event)
            Children [
                @{ button_scene(label, ButtonVariant::Default, palette) }
                MenuTrigger
                MenuButton
                TabIndex(0)
                --
                Node {
                    position_type: PositionType::Absolute,
                    min_width: rem(MENU_WIDTH),
                    flex_direction: FlexDirection::Column,
                    row_gap: rem(space::S4),
                    padding: UiRect::all(rem(space::S4)),
                    border: UiRect::all(Val::Px(palette.hairline_px)),
                    border_radius: BorderRadius::all(rem(palette.card_radius_px)),
                    display: Display::None,
                }
                BackgroundColor({ palette.surface })
                BorderColor { top: edge, right: edge, bottom: edge, left: edge }
                MenuPanel
                MenuPopup { layout: MenuLayout::Column }
                // The popup declares itself visible: the official focus
                // bookkeeping only reaches the entries of a visible popup,
                // and the closed state is carried by the layout (`display`),
                // not by the visibility system.
                Visibility::Visible
                Popover { positions: { placements }, window_margin: { MENU_POPOVER_MARGIN_PX } }
                ThemeContext(SurfaceLevel::Floating)
                Children [
                    { rows }
                ]
            ]
    }
}

/// One row of the popup: an entry is either a separator bar or an official
/// menu item.
fn menu_row_scene(entry: &MenuEntry, index: usize, palette: &UiPalette) -> Box<dyn Scene> {
    match entry {
        MenuEntry::Separator => {
            let bar = palette.border;
            Box::new(bsn! {
                            Node {
                                width: percent(100),
                                padding: UiRect::vertical(rem(space::S4)),
                            }
                            Children [
                                Node {
                                    width: percent(100),
                                    height: px(palette.hairline_px),
                                    flex_shrink: 0.0,
                                }
                                BackgroundColor({ bar })
                                MenuSeparator
                            ]
            })
        }
        MenuEntry::Item { label, enabled } => {
            let label = label.clone();
            // Dim ink for the honest unavailable state; body ink otherwise.
            // The role marker drives the ink — no literal color here.
            let role = if *enabled { Role::Body } else { Role::Caption };
            let fill = menu_row_fill(false, palette);
            Box::new(bsn! {
                            Node {
                                width: percent(100),
                                min_height: rem(palette.control_height_px),
                                align_items: AlignItems::Center,
                                padding: UiRect::horizontal(rem(space::S12)),
                                border_radius: BorderRadius::all(rem(palette.control_radius_px)),
                            }
                            BackgroundColor({ fill })
                            MenuItem
                            MenuRowIndex({ index })
                            MenuItemDisabled({ !*enabled })
                            TabIndex(0)
                            Children [
                                Text(label) TextRole(role)
                            ]
            })
        }
    }
}

/// The owner observer: the official `MenuEvent` channel, answered with the
/// official vocabulary.
///
/// * `Open` / `Toggle` open the popup — the official `MenuFocusState` takes
///   the focus to the first (or last) entry and the official focus keeps the
///   popup open;
/// * `CloseAll` closes it, and the close reports the outcome: an entry
///   activation closes the popup too, and the official layer asks for that
///   close from the entry itself;
/// * `FocusRoot` gives the focus back to the trigger.
fn on_menu_event(
    event: On<MenuEvent>,
    panels: Query<(Entity, &ChildOf), With<MenuPanel>>,
    mut popups: Query<(&mut MenuFocusState, &mut Node), With<MenuPanel>>,
    triggers: Query<(Entity, &ChildOf), With<MenuTrigger>>,
    rows: Query<&MenuRowIndex, With<MenuItem>>,
    mut focus: ResMut<InputFocus>,
    mut outcomes: MessageWriter<MenuOutcome>,
) {
    // The event's current target is the owner: the official `MenuEvent`
    // bubbles from the trigger and the entries up to it.
    let owner = event.event_target();
    let Some(popup) = panels
        .iter()
        .find(|(_, parent)| parent.parent() == owner)
        .map(|(popup, _)| popup)
    else {
        return;
    };
    // The official focus bookkeeping is the single truth about the popup's
    // own state.
    let is_open = popups.get_mut(popup).is_ok_and(|(state, _)| {
        matches!(&*state, MenuFocusState::Opening(_) | MenuFocusState::Open)
    });
    match event.action {
        MenuAction::Open(nav) => open_popup(&mut popups, popup, nav),
        MenuAction::Toggle => {
            if is_open {
                close_popup(&mut popups, popup);
                outcomes.write(MenuOutcome::Canceled);
            } else {
                open_popup(&mut popups, popup, NavAction::First);
            }
        }
        MenuAction::CloseAll => {
            // The official layer closes the popup: an entry activation asks
            // for the close from the entry itself, every other close (the
            // focus left the popup, Escape) asks from the popup.
            let source = event.original_event_target();
            close_popup(&mut popups, popup);
            if let Ok(index) = rows.get(source) {
                outcomes.write(MenuOutcome::Confirmed(index.0));
            } else {
                outcomes.write(MenuOutcome::Canceled);
            }
        }
        MenuAction::FocusRoot => {
            if let Some(trigger) = triggers
                .iter()
                .find(|(_, parent)| parent.parent() == owner)
                .map(|(trigger, _)| trigger)
            {
                focus.set(trigger, FocusCause::Navigated);
            }
        }
    }
}

/// Open the popup: the official layer takes the focus to the first (or last)
/// entry and keeps the popup open as long as the focus stays inside it.
fn open_popup(
    popups: &mut Query<(&mut MenuFocusState, &mut Node), With<MenuPanel>>,
    popup: Entity,
    nav: NavAction,
) {
    if let Ok((mut state, mut node)) = popups.get_mut(popup) {
        *state = MenuFocusState::Opening(nav);
        node.display = Display::Flex;
    }
}

/// Close the popup: the official state goes back to `Closed`, so the official
/// layer stops watching the focus for it.
fn close_popup(
    popups: &mut Query<(&mut MenuFocusState, &mut Node), With<MenuPanel>>,
    popup: Entity,
) {
    if let Ok((mut state, mut node)) = popups.get_mut(popup) {
        *state = MenuFocusState::Closed;
        node.display = Display::None;
    }
}

/// Keep the official `InteractionDisabled` in step with the entry's own
/// availability fact: the official layer refuses to activate a disabled
/// entry, so an unavailable entry can never confirm.
pub fn sync_menu_item_disabled(
    mut commands: Commands,
    items: Query<(Entity, &MenuItemDisabled, Has<InteractionDisabled>), With<MenuItem>>,
) {
    for (entity, disabled, sdk_disabled) in &items {
        if disabled.0 == sdk_disabled {
            continue;
        }
        if disabled.0 {
            commands.entity(entity).insert(InteractionDisabled);
        } else {
            commands.entity(entity).remove::<InteractionDisabled>();
        }
    }
}

/// The popup panel: it paints the surface and the hairline edge.
#[derive(QueryFilter)]
pub struct MenuPanelPaintFilter {
    with_menu_panel: With<MenuPanel>,
    without_menu_item: Without<MenuItem>,
    without_menu_separator: Without<MenuSeparator>,
}

/// A separator bar: it paints the border token.
#[derive(QueryFilter)]
pub struct MenuSeparatorPaintFilter {
    with_menu_separator: With<MenuSeparator>,
    without_menu_panel: Without<MenuPanel>,
    without_menu_item: Without<MenuItem>,
}

/// An item row: it paints the highlight or the panel surface.
#[derive(QueryFilter)]
pub struct MenuItemPaintFilter {
    with_menu_item: With<MenuItem>,
    without_menu_panel: Without<MenuPanel>,
    without_menu_separator: Without<MenuSeparator>,
}

/// Repaint the menu from the live palette and the official focus: the panel
/// chrome and the row fills. The focused row paints the hover token — the
/// official focus *is* the menu's highlight, so the mouse and the keyboard
/// agree on one vocabulary. Compare-and-set, so unchanged frames cost
/// nothing and a theme switch repaints in place.
pub fn sync_menu_visuals(
    palette: Res<UiPalette>,
    focus: Res<InputFocus>,
    mut panels: Query<(&mut BackgroundColor, &mut BorderColor), MenuPanelPaintFilter>,
    mut separators: Query<&mut BackgroundColor, MenuSeparatorPaintFilter>,
    mut rows: Query<
        (Entity, Option<&PickingInteraction>, &mut BackgroundColor),
        MenuItemPaintFilter,
    >,
) {
    let edge = palette.border;
    for (mut fill, mut border) in &mut panels {
        if fill.0 != palette.surface {
            fill.0 = palette.surface;
        }
        if border.top != edge {
            border.set_all(edge);
        }
    }
    for mut fill in &mut separators {
        if fill.0 != edge {
            fill.0 = edge;
        }
    }
    let focused = focus.get();
    for (entity, interaction, mut fill) in &mut rows {
        let hovered = matches!(interaction, Some(PickingInteraction::Hovered));
        let highlighted = focused == Some(entity) || hovered;
        let target = menu_row_fill(highlighted, &palette);
        if fill.0 != target {
            fill.0 = target;
        }
    }
}
