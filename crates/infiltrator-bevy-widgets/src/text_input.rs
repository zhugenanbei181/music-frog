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
//!
//! **CJK IME & Text Interaction Engine**:
//! - [`ImeCursorArea`] and [`compute_ime_cursor_area`]: absolute screen coordinate
//!   calculation for candidate window popup placement and soft keyboard avoidance;
//! - [`PreeditStateMachine`]: Pinyin / CJK syllable and clause segmentation
//!   state machine with navigation and conversion states;
//! - [`TextFieldState::safe_backspace`] / [`TextFieldState::safe_delete`]:
//!   Unicode extended grapheme cluster safe deletion (never breaks emojis, flags,
//!   skin tones, or combining marks);
//! - [`ImeTransaction`]: transaction snapshots with rollback on cancellation and
//!   atomic commit.

use bevy::camera::visibility::Visibility;
use bevy::ecs::component::Component;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::query::{With, Without};
use bevy::ecs::system::{Commands, Local, Query, Res};
use bevy::math::Vec2;
use bevy::scene::{Scene, bsn};
use bevy::time::{Time, Virtual};
use bevy::transform::components::GlobalTransform;
use bevy::ui::BorderColor;
use bevy::ui::prelude::{
    AlignItems, BackgroundColor, BorderRadius, ComputedNode, FlexDirection, Node, UiRect, Val,
    percent, px,
};
use bevy::ui::widget::Text;
use unicode_segmentation::UnicodeSegmentation;

use crate::palette::UiPalette;
use crate::text::{Role, TextRole};
use crate::theme::space;

mod ime;

/// Classification of a preedit segment in CJK IME composition.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum PreeditClauseState {
    /// Raw uncommitted input (e.g. latin pinyin syllables).
    #[default]
    Raw,
    /// Currently focused / active candidate clause being selected.
    Selected,
    /// Converted clause that is not currently selected.
    Converted,
}

/// A segmented clause within an IME preedit composition.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PreeditClause {
    /// Text content of this clause.
    pub text: String,
    /// State / classification of this clause.
    pub state: PreeditClauseState,
    /// Character range `(start, end)` within the full preedit string.
    pub range: (usize, usize),
}

impl PreeditClause {
    /// Construct a new preedit clause.
    pub fn new(text: impl Into<String>, state: PreeditClauseState, range: (usize, usize)) -> Self {
        Self {
            text: text.into(),
            state,
            range,
        }
    }

    /// Whether this clause is the active / selected one.
    pub fn is_selected(&self) -> bool {
        self.state == PreeditClauseState::Selected
    }
}

/// Status of the IME Preedit composition state machine.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum PreeditStatus {
    /// No active composition.
    #[default]
    Idle,
    /// Active composition in progress.
    Composing,
    /// Composition just committed.
    Committed,
}

/// State machine for IME preedit composition and CJK syllable/clause segmentation.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PreeditStateMachine {
    raw_text: String,
    clauses: Vec<PreeditClause>,
    cursor: usize,
    active_clause: Option<usize>,
    status: PreeditStatus,
}

/// Snapshot of text field state prior to an IME composition session,
/// enabling transaction rollback on cancellation (e.g. Escape / Cancel).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImeTransaction {
    /// Controlled text when the transaction began.
    pub original_text: String,
    /// Caret character position when the transaction began.
    pub original_cursor: usize,
    /// Selection anchor when the transaction began.
    pub original_anchor: Option<usize>,
}

/// Absolute screen rectangle for the IME cursor / composition area.
/// Used to position the OS IME candidate window (e.g. on Wayland/Windows/macOS)
/// and to calculate soft keyboard avoidance on mobile.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq)]
pub struct ImeCursorArea {
    /// Top-left position in window/screen pixel coordinates.
    pub position: Vec2,
    /// Dimensions (width, height) of the cursor / composition area in pixels.
    pub size: Vec2,
}

impl ImeCursorArea {
    /// Construct a new cursor area from position and size.
    pub fn new(position: Vec2, size: Vec2) -> Self {
        Self { position, size }
    }

    /// Construct a new cursor area from individual scalar bounds.
    pub fn from_rect(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            position: Vec2::new(x, y),
            size: Vec2::new(width, height),
        }
    }

    /// Top-left X coordinate in screen pixels.
    pub fn x(&self) -> f32 {
        self.position.x
    }

    /// Top-left Y coordinate in screen pixels.
    pub fn y(&self) -> f32 {
        self.position.y
    }

    /// Width of the cursor area in pixels.
    pub fn width(&self) -> f32 {
        self.size.x
    }

    /// Height of the cursor area in pixels.
    pub fn height(&self) -> f32 {
        self.size.y
    }

    /// Top-left point.
    pub fn min(&self) -> Vec2 {
        self.position
    }

    /// Bottom-right point.
    pub fn max(&self) -> Vec2 {
        self.position + self.size
    }

    /// Top edge Y.
    pub fn top(&self) -> f32 {
        self.position.y
    }

    /// Bottom edge Y.
    pub fn bottom(&self) -> f32 {
        self.position.y + self.size.y
    }

    /// Left edge X.
    pub fn left(&self) -> f32 {
        self.position.x
    }

    /// Right edge X.
    pub fn right(&self) -> f32 {
        self.position.x + self.size.x
    }
}

/// Parameters for calculating the absolute IME cursor area.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ImeCursorAreaParams {
    /// Absolute top-left position of the text field bounding box on screen.
    pub field_origin: Vec2,
    /// Total bounding size of the text field.
    pub field_size: Vec2,
    /// Left / top padding of the text container inside the field.
    pub padding: Vec2,
    /// Horizontal offset in pixels from the start of the text to the caret.
    pub caret_offset_x: f32,
    /// Caret width in pixels (typically 2px).
    pub caret_width: f32,
    /// Caret height in pixels (typically line height / control square size).
    pub caret_height: f32,
    /// Width of any active preedit string in pixels (0 if no preedit).
    pub preedit_width: f32,
}

/// Pure function: compute the absolute screen coordinates for the IME cursor area.
pub fn compute_ime_cursor_area(params: ImeCursorAreaParams) -> ImeCursorArea {
    let x = params.field_origin.x + params.padding.x + params.caret_offset_x;
    let y = params.field_origin.y
        + params.padding.y
        + ((params.field_size.y - params.padding.y * 2.0 - params.caret_height).max(0.0) * 0.5);
    let width = if params.preedit_width > 0.0 {
        params.preedit_width.max(params.caret_width)
    } else {
        params.caret_width
    };
    ImeCursorArea {
        position: Vec2::new(x, y),
        size: Vec2::new(width, params.caret_height),
    }
}

/// Estimate text advance width in pixels based on character classes (CJK/wide vs ASCII).
pub fn estimate_text_width(text: &str, font_size: f32) -> f32 {
    let mut width = 0.0;
    for c in text.chars() {
        if is_cjk_or_wide(c) {
            width += font_size;
        } else {
            width += font_size * 0.55;
        }
    }
    width
}

fn is_cjk_or_wide(c: char) -> bool {
    matches!(c as u32,
        0x1100..=0x115F | // Hangul Jamo
        0x2E80..=0xA4CF | // CJK Radicals, Kangxi, Ideographic, Hiragana, Katakana, Bopomofo, CJK Unified Ideographs, Yi
        0xAC00..=0xD7A3 | // Hangul Syllables
        0xF900..=0xFAFF | // CJK Compatibility Ideographs
        0xFE30..=0xFE4F | // CJK Compatibility Forms
        0xFF00..=0xFF60 | // Fullwidth Forms
        0xFFE0..=0xFFE6 | // Fullwidth Signs
        0x1F300..=0x1F9FF // Emojis & Pictographs
    )
}

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
    /// at the caret.
    preedit: String,
    /// The IME preedit state machine managing CJK syllable segmentation.
    preedit_machine: PreeditStateMachine,
    /// Snapshot for active IME transaction rollback.
    ime_transaction: Option<ImeTransaction>,
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
            preedit_machine: PreeditStateMachine::new(),
            ime_transaction: None,
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

    /// Access the preedit state machine.
    pub fn preedit_machine(&self) -> &PreeditStateMachine {
        &self.preedit_machine
    }

    /// Access the mutable preedit state machine.
    pub fn preedit_machine_mut(&mut self) -> &mut PreeditStateMachine {
        &mut self.preedit_machine
    }

    /// Set the IME composition string (the host's `Ime::Preedit` seam),
    /// reporting whether the field changed.
    pub fn set_preedit(&mut self, preedit: impl Into<String>) -> bool {
        let preedit = preedit.into();
        if self.preedit == preedit {
            return false;
        }
        self.preedit = preedit.clone();
        self.preedit_machine.update(preedit, None);
        true
    }

    /// Clear the IME composition (the host's `Ime::Commit` / `Ime::Cancel`
    /// seam), reporting whether a composition was active.
    pub fn clear_preedit(&mut self) -> bool {
        self.preedit_machine.cancel();
        self.set_preedit("")
    }

    /// Begin an IME transaction snapshot if none is currently active.
    pub fn begin_ime_transaction(&mut self) {
        if self.ime_transaction.is_none() {
            self.ime_transaction = Some(ImeTransaction {
                original_text: self.text.clone(),
                original_cursor: self.cursor,
                original_anchor: self.anchor,
            });
        }
    }

    /// Roll back the text field state to the snapshot taken when the IME transaction began.
    /// Clears preedit and returns true if a transaction was active and rolled back.
    pub fn rollback_ime_transaction(&mut self) -> bool {
        self.preedit_machine.cancel();
        self.preedit.clear();
        if let Some(tx) = self.ime_transaction.take() {
            self.text = tx.original_text;
            self.cursor = tx.original_cursor;
            self.anchor = tx.original_anchor;
            true
        } else {
            false
        }
    }

    /// Commit the IME transaction: consumes the snapshot and inserts `committed_text`
    /// at the transaction's insertion point, clearing preedit.
    pub fn commit_ime_transaction(&mut self, committed_text: &str) -> bool {
        self.preedit_machine.commit();
        self.preedit.clear();
        if let Some(tx) = self.ime_transaction.take() {
            self.text = tx.original_text;
            self.cursor = tx.original_cursor;
            self.anchor = tx.original_anchor;
            self.insert(committed_text);
            true
        } else {
            self.insert(committed_text)
        }
    }

    /// Whether an IME transaction is currently active.
    pub fn is_in_ime_transaction(&self) -> bool {
        self.ime_transaction.is_some()
    }

    /// Apply one input, reporting whether the field changed.
    pub fn apply(&mut self, input: TextFieldInput) -> bool {
        match input {
            TextFieldInput::Insert(inserted) => self.insert(&inserted),
            TextFieldInput::Backspace => self.safe_backspace(),
            TextFieldInput::Delete => self.safe_delete(),
            TextFieldInput::Left(extend) => self.move_left(extend),
            TextFieldInput::Right(extend) => self.move_right(extend),
            TextFieldInput::Home => self.move_to(0),
            TextFieldInput::End => self.move_to(self.text.chars().count()),
            TextFieldInput::SelectAll => self.select_all(),
        }
    }

    /// Safely remove the Unicode extended grapheme cluster before the cursor,
    /// or the selected range if one exists.
    pub fn safe_backspace(&mut self) -> bool {
        if self.remove_selected() {
            return true;
        }
        if self.cursor == 0 || self.text.is_empty() {
            return false;
        }

        let head: String = self.text.chars().take(self.cursor).collect();
        let tail: String = self.text.chars().skip(self.cursor).collect();

        let mut grapheme_indices: Vec<(usize, &str)> = head.grapheme_indices(true).collect();
        if let Some((byte_offset, last_grapheme)) = grapheme_indices.pop() {
            let grapheme_chars = last_grapheme.chars().count();
            let new_head = &head[..byte_offset];
            self.text = format!("{new_head}{tail}");
            self.cursor = self.cursor.saturating_sub(grapheme_chars);
            self.anchor = None;
            true
        } else {
            false
        }
    }

    /// Safely remove the Unicode extended grapheme cluster at/after the cursor,
    /// or the selected range if one exists.
    pub fn safe_delete(&mut self) -> bool {
        if self.remove_selected() {
            return true;
        }
        let total_chars = self.text.chars().count();
        if self.cursor >= total_chars {
            return false;
        }

        let head: String = self.text.chars().take(self.cursor).collect();
        let tail: String = self.text.chars().skip(self.cursor).collect();

        let mut graphemes = tail.graphemes(true);
        if let Some(first_grapheme) = graphemes.next() {
            let grapheme_bytes = first_grapheme.len();
            let new_tail = &tail[grapheme_bytes..];
            self.text = format!("{head}{new_tail}");
            self.anchor = None;
            true
        } else {
            false
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

/// System to compute and update [`ImeCursorArea`] for all text fields with computed layout geometry.
#[allow(clippy::type_complexity)]
pub fn sync_ime_cursor_areas(
    mut commands: Commands,
    palette: Res<UiPalette>,
    fields: Query<(
        bevy::ecs::entity::Entity,
        &TextField,
        Option<&GlobalTransform>,
        Option<&ComputedNode>,
        Option<&ImeCursorArea>,
    )>,
) {
    for (entity, field, transform, node, existing_area) in &fields {
        let size = node
            .map(|n| n.size())
            .unwrap_or_else(|| Vec2::new(200.0, palette.control_height_px));
        let origin = transform
            .map(|t| {
                let trans = t.translation();
                Vec2::new(trans.x, trans.y) - size * 0.5
            })
            .unwrap_or(Vec2::ZERO);
        let visual = field_visual(&field.0);

        let font_size = palette.body_font_px;
        let caret_offset_x = estimate_text_width(&visual.before, font_size);
        let preedit_width = estimate_text_width(field.0.preedit(), font_size);

        let params = ImeCursorAreaParams {
            field_origin: origin,
            field_size: size,
            padding: Vec2::new(space::S12, 0.0),
            caret_offset_x,
            caret_width: palette.caret_width_px,
            caret_height: palette.control_square_px,
            preedit_width,
        };

        let calculated = compute_ime_cursor_area(params);
        if let Some(existing) = existing_area {
            if *existing != calculated {
                commands.entity(entity).insert(calculated);
            }
        } else {
            commands.entity(entity).insert(calculated);
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
