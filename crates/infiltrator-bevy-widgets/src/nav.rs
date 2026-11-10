//! Navigation item: the **selected-member** pill — chosen = the accent itself
//! with `on_accent` ink, unchosen = the elevated control surface with ordinary
//! ink. [`crate::list`] borrows this vocabulary for its rows, which is the
//! reading it belongs to: Slint's judged surface paints a chosen egress with
//! `LumenTheme.selection` (= the accent) and `accent-foreground` ink
//! (`ui/components/proxy_picker.slint`).
//!
//! **A navigation row is not a chosen member** and must not use these two
//! functions. Every reference and every peer surface states "where the user
//! is" as a quiet wash plus a narrow indicator: FlClash's sidebar draws
//! `selectedFill = onSurface.withOpacity(0.08)` across the full row with a 3dp
//! primary bar (`plugins/prism_ui/lib/sidebar.dart`), Slint publishes
//! `nav-selected-fill: text-primary.with_alpha(8%)` with a 3u×16u indicator,
//! and Iced's `nav_button` reads `accent_soft` (12–18%) with a 4×18 indicator.
//! The shell owns that row and its wash
//! (`infiltrator-bevy-ui/src/shell_nav.rs`), because a solid accent capsule
//! makes the current destination the loudest thing in the column and fills the
//! row's right side, which the desktop reference records as breathing room.
//!
//! Same contract as the other controls: pure function core
//! ([`nav_fill`] / [`nav_label_ink`]), a `bsn!` scene adapter
//! ([`nav_item_scene`]) and a compare-and-set repaint system
//! ([`sync_nav_visuals`], the checkbox idiom) so a palette swap or an
//! active-bit flip re-projects fill and label ink in place — no switch
//! hook, no tree rebuild. Activation wiring belongs to the caller: the
//! item is a plain node, not the official `Button`, because a nav target
//! that routes nowhere must not pretend to be pressable.

use crate::feathers_theme::Token;
use crate::icon::IconTint;
use crate::palette::{UiPalette, rem};
use crate::text::TextRole;
use crate::theme::space;
use bevy::color::Color;
use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::query::With;
use bevy::ecs::schedule::SystemSet;
use bevy::ecs::system::{Commands, Query, Res};
use bevy::feathers::theme::{SurfaceLevel, ThemeBackgroundColor, ThemeTextColor, UiTheme};
use bevy::scene::{Scene, bsn};
use bevy::ui::prelude::{AlignItems, BackgroundColor, BorderRadius, Node, UiRect};
use bevy::ui::widget::Text;
use infiltrator_contract::design_resources;

/// Marker on the item root; [`sync_nav_visuals`] re-projects its fill and
/// its label's ink from [`NavActive`] and the live palette.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[require(NavRowKind)]
pub struct NavItem;

/// Which of the two selection readings a row wears.
///
/// This is a typed fact about the row, not a per-caller colour choice: the
/// widget layer is the only writer of a row's fill and ink, so the reading has
/// to be declared once at mount and read back by [`sync_nav_visuals`]. A
/// surface that painted its own fill for some rows and let this system paint
/// others is how the Bevy rail ended up with two writers fighting over one
/// selected row.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum NavRowKind {
    /// A chosen member of a set: the accent itself.
    #[default]
    SelectedMember,
    /// Where the user is in the shell's navigation: a quiet wash plus a
    /// narrow accent indicator.
    CurrentDestination,
}

/// The page-owned active bit. Spawned state; later flips restamp the
/// visuals through the sync system without any remount.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct NavActive(pub bool);

/// Marker on the item's label; its ink follows the active bit.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct NavLabel;

/// The chosen-member fill: accent while active, the elevated control surface
/// otherwise. Pure function — headless-testable without any app.
pub fn nav_fill_token(active: bool) -> Token {
    if active {
        Token::Accent
    } else {
        Token::SurfaceElevated
    }
}

/// The chosen-member label ink: `on_accent` while active, ordinary ink
/// otherwise. Pure function.
pub fn nav_label_ink_token(active: bool) -> Token {
    if active { Token::OnAccent } else { Token::Ink }
}

/// The **current-destination** fill: the reference's quiet ink wash while the
/// row is current, and `None` when it is not.
///
/// `None` rather than the column's own colour, because the sidebar's ground is
/// a material wash that is itself translucent: an opaque idle row would stop
/// whatever shows through the panel and would repaint under a focus change.
pub fn current_destination_fill(active: bool) -> Option<Token> {
    active.then_some(Token::NavSelectionFill)
}

/// The current-destination label ink, shared by the glyph beside it.
///
/// The current row reads the accent itself: the wash is deliberately quiet, so
/// the ink and the narrow indicator are what say "here" — the reference's
/// `primary` label plus its `primary` indicator bar.
pub fn current_destination_ink(active: bool) -> Token {
    if active { Token::Accent } else { Token::Ink }
}

/// One navigation item: a control-height pill row carrying its label. The
/// `flex_grow` root lets callers park a trailing caption beside it (a
/// disabled "not migrated" tag) or stack items in a column.
pub fn nav_item_scene(label: String, active: bool, palette: &UiPalette) -> impl Scene + use<> {
    bsn! {
            Node {
                flex_grow: 1.0,
                min_height: rem(palette.control_height_px),
                align_items: AlignItems::Center,
                padding: UiRect::horizontal(rem(space::S12)),
                border_radius: BorderRadius::all(rem(palette.control_radius_px)),
            }
            ThemeBackgroundColor({ nav_fill_token(active) })
            NavItem
            NavRowKind::SelectedMember
            NavActive(active)
            Children [
                Text(label) TextRole(design_resources::TextRole::Body) NavLabel
            ]
    }
}

/// The fill, ink and label weight one nav row wears, from its kind and its
/// active bit. One pure answer per row, so no surface restates a half of it.
///
/// `fill` is `None` exactly when the row paints nothing: the current
/// destination's idle state, which must let the sidebar's translucent material
/// ground through.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NavRowVisual {
    pub fill: Option<Token>,
    pub ink: Token,
    pub role: design_resources::TextRole,
}

/// The row's visual answer. Pure function, exhaustively matched: a new
/// [`NavRowKind`] cannot inherit a reading by accident.
pub fn nav_row_visual(kind: NavRowKind, active: bool) -> NavRowVisual {
    match kind {
        NavRowKind::SelectedMember => NavRowVisual {
            fill: Some(nav_fill_token(active)),
            ink: nav_label_ink_token(active),
            role: if active {
                design_resources::TextRole::BodyStrong
            } else {
                design_resources::TextRole::Body
            },
        },
        NavRowKind::CurrentDestination => NavRowVisual {
            fill: current_destination_fill(active),
            ink: current_destination_ink(active),
            role: if active {
                design_resources::TextRole::LabelStrong
            } else {
                design_resources::TextRole::Label
            },
        },
    }
}

/// The schedule set that owns the **paint half** of a nav row.
///
/// The split is the row's whole state machine: a surface sets [`NavActive`]
/// from its own routing fact, and this set is the only writer of the fill,
/// ink and weight that follow from it. The edge is declared instead of being
/// left to plugin registration order, because the two halves live in
/// different crates: without it the paint can run before the bits moved and
/// every selection would show one frame late, which looks like a stuck row.
#[derive(SystemSet, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct NavVisualPaintSet;

/// Repaint every nav item from its [`NavRowKind`], its [`NavActive`] bit and
/// the live theme: root fill, label ink and weight, and the ink of any glyph
/// riding in the row. Compare-and-set, so unchanged frames cost nothing and a
/// theme switch repaints items without any switch-specific hook.
///
/// This is the **only** writer of a nav row's fill and ink. A surface that
/// painted either one itself would be a second writer of one fact — which is
/// how the Bevy rail came to fight over its selected row: the scene mounted a
/// solid accent capsule, this system repainted the same node, and the icon's
/// tint was resolved from a third place.
/// The rows the paint half walks: the row's typed reading, its active bit, its
/// content, and the theme site it currently wears (if any).
type NavRowQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static NavRowKind,
        &'static NavActive,
        &'static Children,
        Option<&'static ThemeBackgroundColor>,
    ),
    With<NavItem>,
>;

pub fn sync_nav_visuals(
    theme: Res<UiTheme>,
    mut commands: Commands,
    items: NavRowQuery<'_, '_>,
    mut labels: Query<(
        Entity,
        &NavLabel,
        Option<&ThemeTextColor>,
        Option<&mut TextRole>,
    )>,
    mut tints: Query<&mut IconTint>,
    mut fills: Query<&mut BackgroundColor, With<NavItem>>,
) {
    for (entity, kind, active, children, fill) in &items {
        let visual = nav_row_visual(*kind, active.0);
        let wanted = visual.fill.map(|token| token.theme_token());
        let stale = fill.is_none() != wanted.is_none()
            || matches!((fill, &wanted), (Some(have), Some(want)) if have.0 != *want);
        if stale {
            if let Some(token) = wanted {
                commands.entity(entity).insert(ThemeBackgroundColor(token));
            } else {
                commands.entity(entity).remove::<ThemeBackgroundColor>();
                // The token site is what the official theme pipeline paints, so
                // dropping the site must also drop the colour it last wrote: an
                // idle row returns to the transparent ground that lets the
                // column's own material show through. Without this, the row the
                // user just left kept wearing the selection wash forever.
                if let Ok(mut painted) = fills.get_mut(entity)
                    && painted.0 != Color::NONE
                {
                    painted.0 = Color::NONE;
                }
            }
        }
        let ink = theme.context_color(&visual.ink.theme_token(), SurfaceLevel::Base);
        let ink_token = visual.ink.theme_token();
        for child in children.iter() {
            if let Ok(mut tint) = tints.get_mut(*child)
                && tint.0 != ink
            {
                tint.0 = ink;
            }
            if let Ok((label_entity, _, label_ink, role)) = labels.get_mut(*child) {
                if label_ink.map(|have| have.0 != ink_token).unwrap_or(true) {
                    commands
                        .entity(label_entity)
                        .insert(ThemeTextColor(ink_token.clone()));
                }
                if let Some(mut role) = role
                    && role.0 != visual.role
                {
                    role.0 = visual.role;
                }
            }
        }
    }
}
