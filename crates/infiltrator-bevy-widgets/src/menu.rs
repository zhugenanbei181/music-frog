//! Menu / context menu: cyclic pure-core navigation + token overlay scene.
//!
//! **Why not the official `bevy_ui_widgets` menu primitives** (`MenuButton` /
//! `MenuPopup` / `MenuItem`): that machinery is pointer- and focus-driven —
//! popups open on menu-button clicks, dismiss on focus loss and navigate
//! through `TabGroup` focus, every lane of which rides the picking / IME
//! queues only a windowed composition registers. The same finding taskmanager
//! recorded for its own menu; ours differs on purpose: navigation is
//! keyboard-first and driven by a typed [`MenuNavEvent`] the host translates
//! from whatever input seam it owns (there is no honest way to filter
//! `FocusedInput` inside this business-agnostic layer), and activation flows
//! back as a typed [`MenuOutcome`] event. The pure core
//! ([`MenuState`]) stays headless-composable and headless-testable — the
//! essential difference from the official primitives.
//!
//! Rendering is [`menu_overlay_scene`]: a token scrim over the viewport, a
//! surface panel, one row per entry (highlight = the hover token's semantic),
//! disabled entries caption-dim (honest unavailability, taskmanager parity),
//! separators as hairline bars. [`sync_menu_visuals`] re-projects scrim,
//! panel and row fills compare-and-set, so a [`MenuNavEvent`]-driven
//! highlight move or a theme switch repaints in place — no remount. Mount /
//! unmount of the overlay (what open / dismiss means) belongs to the host;
//! a confirmed or canceled outcome does not unmount anything here.

use bevy::color::Color;
use bevy::ecs::component::Component;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::message::{Message, MessageReader, MessageWriter};
use bevy::ecs::query::{With, Without};
use bevy::ecs::system::{Query, Res};
use bevy::scene::{Scene, bsn};
use bevy::ui::BorderColor;
use bevy::ui::prelude::{
    AlignItems, BackgroundColor, BorderRadius, FlexDirection, JustifyContent, Node, UiRect, Val,
    percent, px,
};
use bevy::ui::widget::Text;

use crate::palette::UiPalette;
use crate::text::{Role, TextRole};
use crate::theme::space;

/// Panel width (px) — a context menu is a word-scale surface, not a card.
pub const MENU_WIDTH: f32 = 220.0;

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

/// One navigation input the host's seam feeds into a [`MenuState`] via the
/// [`MenuNavEvent`] typed event.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MenuNav {
    Up,
    Down,
    Confirm,
    Cancel,
}

/// Typed navigation *message* (bevy 0.19's buffered-event vocabulary): hosts
/// translate raw keyboard / pointer input into this and the
/// [`advance_menus`] system drives every mounted menu. Wiring the translation
/// is host work (the pill pattern: interaction wiring stays with the
/// caller). Contract: mount one menu at a time — a nav message is global,
/// every mounted menu advances.
#[derive(Message, Clone, Copy, Debug, PartialEq, Eq)]
pub struct MenuNavEvent(pub MenuNav);

/// One activation outcome, as a message: the confirmed entry's index (into
/// the same `Vec<MenuEntry>` the state navigates) or an explicit cancel.
#[derive(Message, Clone, Copy, Debug, PartialEq, Eq)]
pub enum MenuOutcome {
    Confirmed(usize),
    Canceled,
}

/// The cursor state over one menu's entries: the entry list plus the
/// highlighted entry index (always an `Item` entry — separators are
/// navigation-transparent). Zero bevy — the headless test surface.
///
/// Navigation is **cyclic** (Up from the first item wraps to the last): a
/// context menu is a short ring, and wrap-around is the desktop convention.
/// Disabled items hold the cursor (they must be visibly reachable) but
/// never confirm. All operations are total: an empty menu absorbs every
/// input without panic and without an outcome.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MenuState {
    entries: Vec<MenuEntry>,
    highlight: usize,
}

impl MenuState {
    /// A menu over `entries`, highlighting its first item.
    pub fn new(entries: Vec<MenuEntry>) -> Self {
        let mut state = Self {
            entries,
            highlight: 0,
        };
        state.snap_to_item();
        state
    }

    /// The entries, in scene order (indices are the outcome payload).
    pub fn entries(&self) -> &[MenuEntry] {
        &self.entries
    }

    /// The highlighted entry index (an `Item` entry, or `0` when empty).
    pub fn highlight(&self) -> usize {
        self.highlight
    }

    /// Apply one navigation input, answering the outcome if this input
    /// completes the interaction. `Confirm` on a disabled item (or an
    /// empty menu) answers `None` — the menu stays open and nothing is
    /// authorized.
    pub fn advance(&mut self, nav: MenuNav) -> Option<MenuOutcome> {
        match nav {
            MenuNav::Up => {
                self.step(-1);
                None
            }
            MenuNav::Down => {
                self.step(1);
                None
            }
            MenuNav::Confirm => match self.entries.get(self.highlight) {
                Some(MenuEntry::Item { enabled: true, .. }) => {
                    Some(MenuOutcome::Confirmed(self.highlight))
                }
                _ => None,
            },
            MenuNav::Cancel => Some(MenuOutcome::Canceled),
        }
    }

    /// Cycle the highlight one item-slot in `direction`, skipping separators,
    /// wrapping at both ends. A menu without items keeps its cursor.
    fn step(&mut self, direction: isize) {
        let count = self.entries.len();
        if self.item_at(self.highlight).is_none() && count > 0 {
            // Only reachable through a hand-built state; snap before cycling.
            self.snap_to_item();
        }
        for _ in 0..count {
            let next = (self.highlight as isize + direction).rem_euclid(count as isize) as usize;
            self.highlight = next;
            if self.item_at(next).is_some() {
                return;
            }
        }
    }

    fn item_at(&self, index: usize) -> Option<&MenuEntry> {
        match self.entries.get(index)? {
            MenuEntry::Item { .. } => Some(self.entries.get(index)?),
            MenuEntry::Separator => None,
        }
    }

    /// Move the highlight to the first item at or after `from`.
    fn snap_to_item(&mut self) {
        for offset in 0..self.entries.len() {
            let index = (self.highlight + offset) % self.entries.len().max(1);
            if self.item_at(index).is_some() {
                self.highlight = index;
                return;
            }
        }
        self.highlight = 0;
    }
}

/// The menu mounted on the panel: the live pure-core state, advanced by
/// [`advance_menus`] and re-projected by [`sync_menu_visuals`].
#[derive(Component, Clone, Debug, Default)]
pub struct Menu(pub MenuState);

/// Marker on the full-viewport scrim behind the panel.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MenuScrim;

/// Marker on the floating panel.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MenuPanel;

/// Marker on one item row, carrying its entry index; the row fill follows
/// the live highlight.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MenuRowIndex(pub usize);

/// Marker on a separator's bar node; it paints the border token.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MenuSeparator;

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

/// The full overlay: scrim + panel + one row per entry, the highlighted row
/// filled at spawn (the state's own highlight decides — callers never pass a
/// separate index to drift out of sync). Runtime highlight moves flow
/// through [`MenuNavEvent`] → [`advance_menus`] → [`sync_menu_visuals`].
pub fn menu_overlay_scene(entries: Vec<MenuEntry>, palette: &UiPalette) -> impl Scene + use<> {
    let state = MenuState::new(entries);
    let rows: Vec<Box<dyn Scene>> = state
        .entries()
        .iter()
        .enumerate()
        .map(|(index, entry)| menu_row_scene(entry, index, index == state.highlight(), palette))
        .collect();
    let edge = palette.border;
    bsn! {
        Node {
            width: percent(100),
            height: percent(100),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
        }
        BackgroundColor({ palette.scrim() })
        MenuScrim
        Children [
            (
                Node {
                    min_width: px(MENU_WIDTH),
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(space::S4),
                    padding: UiRect::all(Val::Px(space::S4)),
                    border: UiRect::all(Val::Px(palette.hairline_px)),
                    border_radius: BorderRadius::all(Val::Px(palette.card_radius_px)),
                }
                BackgroundColor({ palette.surface })
                BorderColor { top: edge, right: edge, bottom: edge, left: edge }
                MenuPanel
                Menu({ state })
                Children [
                    { rows },
                ]
            ),
        ]
    }
}

fn menu_row_scene(
    entry: &MenuEntry,
    index: usize,
    highlighted: bool,
    palette: &UiPalette,
) -> Box<dyn Scene> {
    match entry {
        MenuEntry::Separator => {
            let bar = palette.border;
            Box::new(bsn! {
                Node {
                    width: percent(100),
                    padding: UiRect::vertical(Val::Px(space::S4)),
                }
                MenuRowIndex({ index })
                Children [
                    (
                        Node {
                            width: percent(100),
                            height: px(palette.hairline_px),
                            flex_shrink: 0.0,
                        }
                        BackgroundColor({ bar })
                        MenuSeparator
                    ),
                ]
            })
        }
        MenuEntry::Item { label, enabled } => {
            let label = label.clone();
            // Dim ink for the honest unavailable state; body ink otherwise.
            // The role marker drives the ink — no literal color here.
            let role = if *enabled { Role::Body } else { Role::Caption };
            let fill = menu_row_fill(highlighted, palette);
            Box::new(bsn! {
                Node {
                    width: percent(100),
                    min_height: px(palette.control_height_px),
                    align_items: AlignItems::Center,
                    padding: UiRect::horizontal(Val::Px(space::S12)),
                    border_radius: BorderRadius::all(Val::Px(palette.control_radius_px)),
                }
                BackgroundColor({ fill })
                MenuRowIndex({ index })
                Children [
                    ( Text(label) TextRole(role) ),
                ]
            })
        }
    }
}

/// Drive every mounted menu from the typed navigation events and emit the
/// activation outcomes. The highlight move itself is pure state; the
/// repaint happens in [`sync_menu_visuals`].
pub fn advance_menus(
    mut navs: MessageReader<MenuNavEvent>,
    mut outcomes: MessageWriter<MenuOutcome>,
    mut menus: Query<&mut Menu>,
) {
    for event in navs.read() {
        for mut menu in &mut menus {
            if let Some(outcome) = menu.0.advance(event.0) {
                outcomes.write(outcome);
            }
        }
    }
}

/// Repaint every mounted menu from its live state and the palette:
/// scrim, panel fill and edge, item-row highlights — compare-and-set, so
/// unchanged frames cost nothing and a theme switch repaints the overlay
/// with no switch-specific hook. The fill queries are provably disjoint by
/// marker (scrim / panel / rows).
#[allow(clippy::type_complexity)]
pub fn sync_menu_visuals(
    palette: Res<UiPalette>,
    mut scrims: Query<&mut BackgroundColor, (With<MenuScrim>, Without<MenuPanel>)>,
    mut panels: Query<&mut BackgroundColor, (With<MenuPanel>, Without<MenuScrim>)>,
    mut borders: Query<&mut BorderColor, With<MenuPanel>>,
    mut rows: Query<
        (&MenuRowIndex, &mut BackgroundColor),
        (
            With<MenuRowIndex>,
            Without<MenuPanel>,
            Without<MenuScrim>,
            Without<MenuSeparator>,
        ),
    >,
    menus: Query<(&Menu, &Children)>,
) {
    let scrim = palette.scrim();
    for mut fill in &mut scrims {
        if fill.0 != scrim {
            fill.0 = scrim;
        }
    }
    for mut fill in &mut panels {
        if fill.0 != palette.surface {
            fill.0 = palette.surface;
        }
    }
    let edge = palette.border;
    for mut border in &mut borders {
        if border.top != edge {
            border.set_all(edge);
        }
    }
    for (menu, children) in &menus {
        for child in children.iter() {
            // Separator wrappers carry a row index but no fill; the get_mut
            // below simply never matches them. Their bars repaint via the
            // border token in the loop after this one.
            let Ok((row, mut fill)) = rows.get_mut(*child) else {
                continue;
            };
            let target = menu_row_fill(row.0 == menu.0.highlight(), &palette);
            if fill.0 != target {
                fill.0 = target;
            }
        }
    }
}
