//! Context menu: a right-click menu at the cursor, on the official menu
//! primitives.
//!
//! The official `MenuPopup` / `MenuItem` own the behaviour — the popup holds
//! the focus, the arrow keys walk the entries, `Enter` / `Space` / a click
//! activate one, `Escape` closes it — and the official `Popover` owns the
//! placement: it flips the popup back inside the window on its own, so this
//! module carries no viewport arithmetic of its own.
//!
//! The product keeps the entry description ([`ContextMenuItem`]) and the
//! typed stage ([`ContextMenuStage`]): the stage is the single truth about
//! whether the menu is presented, and the outcome
//! ([`ContextMenuOutcome`]) is derived from the official menu events.

use crate::icon::IconId;
use crate::icon_tile::icon_tile_scene;
use crate::menu::MenuItemDisabled;
use crate::palette::{UiPalette, rem};
use crate::text::{Role, TextRole};
use crate::theme::{metrics, radius, space};
use bevy::camera::visibility::Visibility;
use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::message::{Message, MessageWriter};
use bevy::ecs::observer::On;
use bevy::ecs::query::With;
use bevy::ecs::resource::Resource;
use bevy::ecs::system::{Query, ResMut};
use bevy::feathers::theme::{SurfaceLevel, ThemeContext};
use bevy::input_focus::tab_navigation::{NavAction, TabIndex};
use bevy::math::Vec2;
use bevy::scene::{Scene, bsn, on};
use bevy::ui::BorderColor;
use bevy::ui::FixedNode;
use bevy::ui::prelude::{
    AlignItems, BackgroundColor, BorderRadius, FlexDirection, JustifyContent, Node, PositionType,
    UiRect, Val, percent, px,
};
use bevy::ui::widget::Text;
use bevy::ui_widgets::popover::{Popover, PopoverAlign, PopoverPlacement, PopoverSide};
use bevy::ui_widgets::{MenuAction, MenuEvent, MenuFocusState, MenuItem, MenuLayout, MenuPopup};

/// One-off layout value (not a ladder rung): the width of the floating
/// context-menu panel that lists the actions.
const CONTEXT_MENU_PANEL_WIDTH_PX: f32 = 180.0;

/// How close to the window edge the official `Popover` placement may bring
/// the menu (logical px).
const CONTEXT_MENU_WINDOW_MARGIN_PX: f32 = space::S8;

/// The context menu's stage: the single truth about whether the menu is
/// presented. The old design carried a bare `is_open` flag next to the
/// entries; the stage is the typed shape of that fact.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ContextMenuStage {
    /// The menu is not presented.
    #[default]
    Closed,
    /// The menu is presented at the cursor.
    Open,
}

/// An individual action entry inside a context menu.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContextMenuItem {
    pub id: String,
    pub label: String,
    pub icon: Option<IconId>,
    pub is_destructive: bool,
    pub is_disabled: bool,
}

impl ContextMenuItem {
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            icon: None,
            is_destructive: false,
            is_disabled: false,
        }
    }

    pub fn with_icon(mut self, icon: IconId) -> Self {
        self.icon = Some(icon);
        self
    }

    pub fn destructive(mut self) -> Self {
        self.is_destructive = true;
        self
    }

    pub fn disabled(mut self) -> Self {
        self.is_disabled = true;
        self
    }
}

/// The context menu's business state: the typed stage and the entries.
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct ContextMenuState {
    pub stage: ContextMenuStage,
    pub items: Vec<ContextMenuItem>,
}

impl ContextMenuState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Present the menu with these entries. The cursor position belongs to
    /// the scene ([`context_menu_scene`]): the official `Popover` placement
    /// owns the geometry from there.
    pub fn open(&mut self, items: Vec<ContextMenuItem>) {
        self.stage = ContextMenuStage::Open;
        self.items = items;
    }

    /// Dismiss the menu.
    pub fn close(&mut self) {
        self.stage = ContextMenuStage::Closed;
        self.items.clear();
    }

    /// Whether the menu is presented.
    pub fn is_open(&self) -> bool {
        matches!(self.stage, ContextMenuStage::Open)
    }
}

/// The context menu's outcome, derived from the official menu events.
#[derive(Message, Clone, Debug, PartialEq, Eq)]
pub enum ContextMenuOutcome {
    /// The official layer activated an entry: the entry id.
    Selected(String),
    /// The official layer dismissed the menu without activating an entry.
    Dismissed,
}

/// Marker component on the context menu root overlay.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ContextMenuRoot;

/// Marker component on the context menu card (the official `MenuPopup` node).
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ContextMenuCard;

/// The entry id carried by one context menu row.
#[derive(Component, Clone, Debug, Default, PartialEq, Eq)]
pub struct ContextMenuEntryId(pub String);

/// Construct a declarative context menu overlay on the official menu
/// primitives.
///
/// The root is the context-menu layer's root: it declares the floating
/// surface level and mounts `FixedNode`, so the cursor-anchored placement of
/// the card stays correct wherever the host mounts the scene. The card is the
/// official `MenuPopup` and every entry is an official `MenuItem`; the
/// official `Popover` placement flips the card back inside the window, so
/// this module owns no viewport arithmetic.
///
/// The popup mounts with the official `MenuFocusState::Opening`, so the
/// official layer hands the focus to the first entry and keeps the menu open
/// while the focus stays inside it.
pub fn context_menu_scene(
    cursor: Vec2,
    state: &ContextMenuState,
    palette: &UiPalette,
) -> Box<dyn Scene> {
    let edge = palette.border;
    let entries: Vec<Box<dyn Scene>> = state
        .items
        .iter()
        .map(|item| context_menu_row_scene(item, palette))
        .collect();
    let placements = vec![
        PopoverPlacement {
            side: PopoverSide::Bottom,
            align: PopoverAlign::Start,
            gap: 0.0,
        },
        PopoverPlacement {
            side: PopoverSide::Top,
            align: PopoverAlign::Start,
            gap: 0.0,
        },
        PopoverPlacement {
            side: PopoverSide::Bottom,
            align: PopoverAlign::End,
            gap: 0.0,
        },
        PopoverPlacement {
            side: PopoverSide::Top,
            align: PopoverAlign::End,
            gap: 0.0,
        },
    ];
    Box::new(bsn! {
            Node {
                position_type: PositionType::Absolute,
                width: percent(100),
                height: percent(100),
            }
            ContextMenuRoot
            ThemeContext(SurfaceLevel::Floating)
            FixedNode
            on(on_context_menu_event)
            Children [
                Node {
                    position_type: PositionType::Absolute,
                    left: px(cursor.x),
                    top: px(cursor.y),
                    width: px(0.0),
                    height: px(0.0),
                }
                Children [
                    Node {
                        position_type: PositionType::Absolute,
                        width: rem(CONTEXT_MENU_PANEL_WIDTH_PX),
                        flex_direction: FlexDirection::Column,
                        padding: UiRect::all(rem(space::S4)),
                        row_gap: rem(space::S2),
                        border: UiRect::all(Val::Px(metrics::HAIRLINE)),
                        border_radius: BorderRadius::all(rem(palette.card_radius_px)),
                    }
                    BackgroundColor({ palette.surface })
                    BorderColor { top: edge, right: edge, bottom: edge, left: edge }
                    ContextMenuCard
                    MenuPopup { layout: MenuLayout::Column }
                    MenuFocusState::Opening(NavAction::First)
                    // The popup declares itself visible: the official focus
                    // bookkeeping only reaches the entries of a visible popup.
                    Visibility::Visible
                    Popover { positions: { placements }, window_margin: { CONTEXT_MENU_WINDOW_MARGIN_PX } }
                    Children [
                        { entries }
                    ]
                ]
            ]
    })
}

/// One entry row: an official menu item with the product's icon and label.
fn context_menu_row_scene(item: &ContextMenuItem, palette: &UiPalette) -> Box<dyn Scene> {
    let label = item.label.clone();
    let id = item.id.clone();
    // The destructive action reads as emphasised copy; the role vocabulary
    // carries no danger ink of its own.
    let role = if item.is_destructive {
        Role::BodyStrong
    } else {
        Role::Body
    };
    let icon_node: Option<Box<dyn Scene>> = item
        .icon
        .map(|icon| Box::new(icon_tile_scene(icon, 16.0, palette)) as Box<dyn Scene>);
    Box::new(bsn! {
                            Node {
                                width: percent(100),
                                height: rem(space::S32),
                                align_items: AlignItems::Center,
                                justify_content: JustifyContent::FlexStart,
                                padding: UiRect::horizontal(rem(space::S8)),
                                column_gap: rem(space::S8),
                                border_radius: BorderRadius::all(rem(radius::XS)),
                            }
                            MenuItem
                            ContextMenuEntryId({ id })
                            MenuItemDisabled({ item.is_disabled })
                            TabIndex(0)
                            Children [
                                @{ icon_node }
                                --
                                Text({ label }) TextRole({ role })
                            ]
    })
}

/// The context menu observer: the official `MenuEvent` channel, answered
/// with the product's typed outcome.
///
/// The official layer closes the popup when the focus leaves it or when an
/// entry is activated; the close's source tells the two apart (an entry
/// asks for the close from itself).
fn on_context_menu_event(
    event: On<MenuEvent>,
    entries: Query<&ContextMenuEntryId, With<MenuItem>>,
    state: Option<ResMut<ContextMenuState>>,
    mut outcomes: MessageWriter<ContextMenuOutcome>,
) {
    if !matches!(event.action, MenuAction::CloseAll) {
        return;
    }
    let source: Entity = event.original_event_target();
    let outcome = match entries.get(source) {
        Ok(id) => ContextMenuOutcome::Selected(id.0.clone()),
        Err(_) => ContextMenuOutcome::Dismissed,
    };
    if let Some(mut state) = state {
        state.close();
    }
    outcomes.write(outcome);
}
