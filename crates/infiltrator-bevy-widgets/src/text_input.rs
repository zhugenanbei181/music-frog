//! Text field: a controlled single-line input, pure core + scene adapter.
//!
//! **Why not the official `EditableText` primitive** (`bevy_text::
//! EditableText` + `bevy_ui_widgets::EditableTextInputPlugin`): the official
//! editing core itself is headless-composable, but every input path that
//! drives it — `FocusedInput<KeyboardInput>` from the focus dispatcher,
//! `Pointer<Press/Drag>` click-to-place from the picking runtime, and `Ime`
//! window messages — originates in window event queues that only a windowed
//! composition registers. A `MinimalPlugins` headless composition can spawn
//! the component but can never exercise it, the same finding taskmanager
//! recorded for the menu primitives. This module therefore owns a zero-bevy
//! state machine ([`TextFieldState`]: controlled text, caret, minimal
//! selection, IME preedit) that hosts drive from whatever input seam they
//! own, and a token-skinned [`text_field_scene`] adapter. Revisit when the
//! official primitive grows a drivable edit seam.
//!
//! **State → visual projection** (BEVY-010): [`field_visual`] decomposes the
//! controlled state into the four visible runs — before / selected / after —
//! plus the caret slot (caret at the selection's left or right edge). The
//! scene mounts that decomposition as a fixed run structure; two sync
//! systems restamp it in place: [`sync_text_fields`] mirrors text, selection
//! wash and preedit (with its bottom-bar underline — never a glyph
//! underline), and [`sync_field_carets`] blinks the active caret bar on a
//! `Local`-timed cadence via compare-and-set [`Visibility`]. The input seam
//! (keyboard / IME events) stays a windowed milestone; everything here is
//! exercised through state injection.

use bevy::camera::visibility::Visibility;
use bevy::ecs::component::Component;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::query::{With, Without};
use bevy::ecs::system::{Local, Query, Res};
use bevy::scene::{Scene, bsn};
use bevy::time::{Time, Virtual};
use bevy::ui::BorderColor;
use bevy::ui::prelude::{
    AlignItems, BackgroundColor, BorderRadius, FlexDirection, Node, UiRect, Val, percent, px,
};
use bevy::ui::widget::Text;

use crate::palette::UiPalette;
use crate::text::{Role, TextRole};
use crate::theme::space;

/// One editing operation the host's input seam feeds into
/// [`TextFieldState::apply`]. Kept minimal: this covers the caret and
/// selection surface; clipboard and IME composition belong to the windowed
/// binding layer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TextFieldInput {
    /// Insert text at the caret, replacing any selection (an IME commit or
    /// a typed run may carry several chars).
    Insert(String),
    /// Remove the selection, or the char before the caret.
    Backspace,
    /// Remove the selection, or the char after the caret.
    Delete,
    /// Move the caret left one char; `true` extends the selection.
    Left(bool),
    /// Move the caret right one char; `true` extends the selection.
    Right(bool),
    /// Move the caret to the start; `true` extends the selection.
    Home,
    /// Move the caret to the end; `true` extends the selection.
    End,
    /// Select the whole text.
    SelectAll,
}

/// The controlled state of one single-line field: text, caret (a char
/// index — never a byte offset, so multi-byte text cannot split a codepoint)
/// and an optional selection anchor.
///
/// Zero bevy dependencies — this struct is the headless test surface. All
/// operations are total: out-of-range input clamps instead of panicking.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TextFieldState {
    text: String,
    cursor: usize,
    anchor: Option<usize>,
    /// The active IME composition string, rendered as its own underlined run
    /// at the caret. It never rides [`Self::apply`] — the host's IME seam
    /// sets it directly on `Ime::Preedit` and clears it (ideally together
    /// with the commit's [`TextFieldInput::Insert`]) on `Ime::Commit`.
    preedit: String,
}

impl TextFieldState {
    /// A field holding `text` with the caret at the end and no selection.
    pub fn new(text: impl Into<String>) -> Self {
        let text = text.into();
        let cursor = text.chars().count();
        Self {
            text,
            cursor,
            anchor: None,
            preedit: String::new(),
        }
    }

    /// The controlled text.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// The caret as a char index into [`Self::text`].
    pub fn cursor(&self) -> usize {
        self.cursor
    }

    /// The selection as ordered `(start, end)` char indices, or `None`.
    pub fn selection(&self) -> Option<(usize, usize)> {
        let anchor = self.anchor?;
        if anchor == self.cursor {
            return None;
        }
        Some((anchor.min(self.cursor), anchor.max(self.cursor)))
    }

    /// The active IME composition string, if any.
    pub fn preedit(&self) -> &str {
        &self.preedit
    }

    /// Set the IME composition string (the host's `Ime::Preedit` seam),
    /// reporting whether the field changed.
    pub fn set_preedit(&mut self, preedit: impl Into<String>) -> bool {
        let preedit = preedit.into();
        if self.preedit == preedit {
            return false;
        }
        self.preedit = preedit;
        true
    }

    /// Clear the IME composition (the host's `Ime::Commit` / `Ime::Cancel`
    /// seam), reporting whether a composition was active.
    pub fn clear_preedit(&mut self) -> bool {
        self.set_preedit("")
    }

    /// Apply one input, reporting whether the field changed.
    pub fn apply(&mut self, input: TextFieldInput) -> bool {
        match input {
            TextFieldInput::Insert(inserted) => self.insert(&inserted),
            TextFieldInput::Backspace => self.remove_before_cursor(),
            TextFieldInput::Delete => self.remove_at_cursor(),
            TextFieldInput::Left(extend) => self.move_left(extend),
            TextFieldInput::Right(extend) => self.move_right(extend),
            TextFieldInput::Home => self.move_to(0),
            TextFieldInput::End => self.move_to(self.text.chars().count()),
            TextFieldInput::SelectAll => self.select_all(),
        }
    }

    fn selected_range(&self) -> Option<(usize, usize)> {
        self.selection()
    }

    fn insert(&mut self, inserted: &str) -> bool {
        let count = inserted.chars().count();
        let kept = self.remove_selected();
        let head = self.text.chars().take(self.cursor).collect::<String>();
        let tail = self.text.chars().skip(self.cursor).collect::<String>();
        self.text = head + inserted + &tail;
        self.cursor += count;
        self.anchor = None;
        kept || !inserted.is_empty()
    }

    fn remove_before_cursor(&mut self) -> bool {
        if self.remove_selected() {
            return true;
        }
        if self.cursor == 0 {
            return false;
        }
        let head = self.text.chars().take(self.cursor - 1).collect::<String>();
        let tail = self.text.chars().skip(self.cursor).collect::<String>();
        self.text = head + &tail;
        self.cursor -= 1;
        true
    }

    fn remove_at_cursor(&mut self) -> bool {
        if self.remove_selected() {
            return true;
        }
        let len = self.text.chars().count();
        if self.cursor >= len {
            return false;
        }
        let head = self.text.chars().take(self.cursor).collect::<String>();
        let tail = self.text.chars().skip(self.cursor + 1).collect::<String>();
        self.text = head + &tail;
        true
    }

    fn move_left(&mut self, extend: bool) -> bool {
        if !extend {
            self.anchor = None;
        } else if self.anchor.is_none() {
            self.anchor = Some(self.cursor);
        }
        if self.cursor == 0 {
            return false;
        }
        self.cursor -= 1;
        true
    }

    fn move_right(&mut self, extend: bool) -> bool {
        if !extend {
            self.anchor = None;
        } else if self.anchor.is_none() {
            self.anchor = Some(self.cursor);
        }
        if self.cursor >= self.text.chars().count() {
            return false;
        }
        self.cursor += 1;
        true
    }

    fn move_to(&mut self, index: usize) -> bool {
        self.anchor = None;
        if self.cursor == index {
            return false;
        }
        self.cursor = index;
        true
    }

    fn select_all(&mut self) -> bool {
        let len = self.text.chars().count();
        if len == 0 || self.selection() == Some((0, len)) {
            return false;
        }
        self.anchor = Some(0);
        self.cursor = len;
        true
    }

    /// Drop any selection, reporting whether one existed.
    fn remove_selected(&mut self) -> bool {
        let Some((start, end)) = self.selected_range() else {
            return false;
        };
        let head = self.text.chars().take(start).collect::<String>();
        let tail = self.text.chars().skip(end).collect::<String>();
        self.text = head + &tail;
        self.cursor = start;
        self.anchor = None;
        true
    }
}

/// The visible decomposition of one field state: the three text runs (the
/// selected one rendered on the selection wash) and which caret slot blinks
/// — `0` places the caret at the selection's left edge (and is the only slot
/// without a selection), `1` at its right edge. Zero bevy — the headless
/// test surface for the whole state → visual contract.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FieldVisual {
    pub before: String,
    pub selected: String,
    pub after: String,
    pub caret_slot: usize,
}

/// Project a controlled state onto the visible runs. Pure function; the
/// scene adapter mounts it at spawn and the sync systems re-derive it every
/// pass.
pub fn field_visual(state: &TextFieldState) -> FieldVisual {
    let chars: Vec<char> = state.text.chars().collect();
    let len = chars.len();
    let cursor = state.cursor.min(len);
    let (start, end) = state.selection().unwrap_or((cursor, cursor));
    let caret_slot = usize::from(cursor > start);
    let slice = |from: usize, to: usize| chars[from.min(len)..to.min(len)].iter().collect();
    FieldVisual {
        before: slice(0, start),
        selected: slice(start, end),
        after: slice(end, len),
        caret_slot,
    }
}

/// The controlled state mounted on a field's root node. Hosts mutate the
/// state through their own input seam; [`sync_text_fields`] mirrors it onto
/// the run structure.
#[derive(Component, Clone, Debug, Default)]
pub struct TextField(pub TextFieldState);

/// Marker on the run left of the caret: its text mirrors
/// [`FieldVisual::before`].
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TextFieldBefore;

/// Marker on the run right of the selection: its text mirrors
/// [`FieldVisual::after`].
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TextFieldAfter;

/// Marker on the selection wash node; its fill is the palette's
/// token-derived [`UiPalette::selection_fill`].
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TextFieldSelection;

/// Marker on the text inside the selection wash: mirrors
/// [`FieldVisual::selected`].
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TextFieldSelectionText;

/// Marker on the preedit column (composition text + underline bar).
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TextFieldPreedit;

/// Marker on the preedit composition text: mirrors
/// [`TextFieldState::preedit`].
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TextFieldPreeditText;

/// Marker on the preedit underline — a 1px bottom bar (never a glyph
/// underline: embedded faces guarantee no combining coverage, and a bar
/// rethemes with the palette like any token fill).
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TextFieldPreeditUnderline;

/// One blinking caret bar. `0` is the slot at the selection's left edge (the
/// only slot without a selection), `1` the slot at its right edge;
/// [`sync_field_carets`] shows exactly the active one and blinks it.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TextFieldCaret(pub usize);

/// A single-line field: token box chrome (elevated fill, hairline accent
/// edge, control radius) around the controlled body text, decomposed into a
/// fixed run structure — before / caret slot 0 / preedit / selection / caret
/// slot 1 / after. The fixed shape is what lets [`sync_text_fields`] and
/// [`sync_field_carets`] restamp in place for every state the field can
/// reach: empty runs measure zero width and paint nothing, so absent
/// selection and preedit cost nothing visually; a caret change flips
/// visibility between the two bars instead of rebuilding the row. (When an
/// IME composition coexists with a selection — a degenerate pair hosts
/// should collapse before committing — the preedit renders at the slot-0
/// edge; the honest contract is documented, not silently reordered.)
pub fn text_field_scene(initial: String, palette: &UiPalette) -> impl Scene + use<> {
    let edge = palette.border;
    let visual = field_visual(&TextFieldState::new(initial.clone()));
    let before = visual.before;
    let after = visual.after;
    let caret_w = palette.caret_width_px;
    let caret_h = palette.control_square_px;
    bsn! {
        Node {
            width: percent(100),
            min_height: px(palette.control_height_px),
            align_items: AlignItems::Center,
            padding: UiRect::horizontal(Val::Px(space::S12)),
            border: UiRect::all(Val::Px(palette.hairline_px)),
            border_radius: BorderRadius::all(Val::Px(palette.control_radius_px)),
        }
        BackgroundColor({ palette.surface_elevated })
        BorderColor {
            top: edge,
            right: edge,
            bottom: edge,
            left: edge,
        }
        TextField({ TextFieldState::new(initial) })
        Children [
            ( Text(before) TextRole(Role::Body) TextFieldBefore ),
            (
                Node {
                    width: px(caret_w),
                    height: px(caret_h),
                    flex_shrink: 0.0,
                }
                BackgroundColor({ palette.accent })
                TextFieldCaret(0)
            ),
            (
                Node {
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(space::S4),
                }
                TextFieldPreedit
                Children [
                    ( Text({ String::new() }) TextRole(Role::Body) TextFieldPreeditText ),
                    (
                        Node {
                            width: percent(100),
                            height: px(palette.hairline_px),
                            flex_shrink: 0.0,
                        }
                        BackgroundColor({ palette.accent })
                        TextFieldPreeditUnderline
                    ),
                ]
            ),
            (
                Node { padding: UiRect::horizontal(Val::Px(space::S4)) }
                BackgroundColor({ palette.selection_fill() })
                TextFieldSelection
                Children [
                    ( Text({ String::new() }) TextRole(Role::Body) TextFieldSelectionText ),
                ]
            ),
            (
                Node {
                    width: px(caret_w),
                    height: px(caret_h),
                    flex_shrink: 0.0,
                }
                BackgroundColor({ palette.accent })
                TextFieldCaret(1)
            ),
            ( Text(after) TextRole(Role::Body) TextFieldAfter ),
        ]
    }
}

/// Mirror controlled state onto each field's run structure, compare-and-set:
/// run texts, the selection wash, the preedit underline and the caret bars'
/// token ink. Runs every pass so a theme switch re-derives the token fills
/// with no switch-specific hook; unchanged fields cost nothing. The run and
/// fill queries carry cross `Without` filters — every child carries exactly
/// one run marker and one fill marker, which makes the accesses provably
/// disjoint (bevy B0001 discipline).
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub fn sync_text_fields(
    palette: Res<UiPalette>,
    fields: Query<(&TextField, &Children)>,
    wrappers: Query<&Children>,
    mut befores: Query<
        (&TextFieldBefore, &mut Text),
        (
            With<TextFieldBefore>,
            Without<TextFieldAfter>,
            Without<TextFieldSelectionText>,
            Without<TextFieldPreeditText>,
        ),
    >,
    mut afters: Query<
        (&TextFieldAfter, &mut Text),
        (
            With<TextFieldAfter>,
            Without<TextFieldBefore>,
            Without<TextFieldSelectionText>,
            Without<TextFieldPreeditText>,
        ),
    >,
    mut selecteds: Query<
        (&TextFieldSelectionText, &mut Text),
        (
            With<TextFieldSelectionText>,
            Without<TextFieldBefore>,
            Without<TextFieldAfter>,
            Without<TextFieldPreeditText>,
        ),
    >,
    mut preedit_texts: Query<
        (&TextFieldPreeditText, &mut Text),
        (
            With<TextFieldPreeditText>,
            Without<TextFieldBefore>,
            Without<TextFieldAfter>,
            Without<TextFieldSelectionText>,
        ),
    >,
    mut washes: Query<
        &mut BackgroundColor,
        (
            With<TextFieldSelection>,
            Without<TextFieldPreeditUnderline>,
            Without<TextFieldCaret>,
        ),
    >,
    mut underlines: Query<
        &mut BackgroundColor,
        (
            With<TextFieldPreeditUnderline>,
            Without<TextFieldSelection>,
            Without<TextFieldCaret>,
        ),
    >,
    mut carets: Query<
        &mut BackgroundColor,
        (
            With<TextFieldCaret>,
            Without<TextFieldSelection>,
            Without<TextFieldPreeditUnderline>,
        ),
    >,
) {
    let accent = palette.accent;
    let wash = palette.selection_fill();
    for (field, children) in &fields {
        let visual = field_visual(&field.0);
        let preedit = field.0.preedit().to_string();
        for child in children.iter() {
            // Direct text runs (before / after) and fills (caret bars, the
            // selection wash) live on the field's own children.
            if let Ok((_, mut text)) = befores.get_mut(*child)
                && text.0 != visual.before
            {
                text.0 = visual.before.clone();
            }
            if let Ok((_, mut text)) = afters.get_mut(*child)
                && text.0 != visual.after
            {
                text.0 = visual.after.clone();
            }
            if let Ok(mut fill) = washes.get_mut(*child)
                && fill.0 != wash
            {
                fill.0 = wash;
            }
            if let Ok(mut fill) = carets.get_mut(*child)
                && fill.0 != accent
            {
                fill.0 = accent;
            }
            // The preedit column and the selection node host their text (and
            // the underline bar) one level down — restamp that subtree too.
            if let Ok(grandchildren) = wrappers.get(*child) {
                for inner in grandchildren.iter() {
                    if let Ok((_, mut text)) = selecteds.get_mut(*inner)
                        && text.0 != visual.selected
                    {
                        text.0 = visual.selected.clone();
                    }
                    if let Ok((_, mut text)) = preedit_texts.get_mut(*inner)
                        && text.0 != preedit
                    {
                        text.0 = preedit.clone();
                    }
                    if let Ok(mut fill) = underlines.get_mut(*inner)
                        && fill.0 != accent
                    {
                        fill.0 = accent;
                    }
                }
            }
        }
    }
}

/// The blink cadence state: elapsed virtual time within the current
/// half-period and which half the caret is in. `Local` to the blink system
/// (public only because the system signature carries it); the pure period
/// token lives on the palette.
#[derive(Clone, Copy, Debug)]
pub struct CaretClock {
    elapsed: f32,
    shown: bool,
}

impl Default for CaretClock {
    fn default() -> Self {
        Self {
            elapsed: 0.0,
            shown: true,
        }
    }
}

/// Blink the active caret bar: a `Local` clock accumulates virtual time and
/// flips the phase every [`UiPalette::CARET_BLINK_SECS`]; the active slot's
/// bar follows the phase, the inactive slot stays hidden — both writes
/// compare-and-set on [`Visibility`], never a tree rebuild. Virtual time
/// (not real time) so a paused app freezes the caret instead of blinking at
/// nothing.
pub fn sync_field_carets(
    mut clock: Local<CaretClock>,
    time: Res<Time<Virtual>>,
    fields: Query<(&TextField, &Children)>,
    mut carets: Query<(&TextFieldCaret, &mut Visibility)>,
) {
    clock.elapsed += time.delta().as_secs_f32();
    if clock.elapsed >= UiPalette::CARET_BLINK_SECS {
        clock.elapsed %= UiPalette::CARET_BLINK_SECS;
        clock.shown = !clock.shown;
    }
    for (field, children) in &fields {
        let visual = field_visual(&field.0);
        for child in children.iter() {
            if let Ok((caret, mut visibility)) = carets.get_mut(*child) {
                let target = if caret.0 == visual.caret_slot && clock.shown {
                    Visibility::Visible
                } else {
                    Visibility::Hidden
                };
                if *visibility != target {
                    *visibility = target;
                }
            }
        }
    }
}
