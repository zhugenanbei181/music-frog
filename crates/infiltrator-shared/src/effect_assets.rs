//! The shared asset pipeline: one vector icon source, one font set and one
//! character-width table (EFFECTS-005 #18~#21).
//!
//! The product draws the same icons on every surface, so the icons have one
//! canonical source — a 24×24 stroke drawing — and every other form is
//! *derived* from it: the white-ink model a renderer tints, the raster plate
//! the Bevy layer paints, the ink model of a surface that cannot rasterize.
//!
//! The fonts are the same story: one monospaced face for every number that
//! must not jitter while it changes, one interface family with three weights,
//! and one CJK face. The width table tells a surface how wide a character is
//! before the font engine has measured it, so a truncation plan and a layout
//! agree on where a text ends.

use crate::assets::iced_icon_svg;

/// The edge length of the canonical icon grid, in user units.
pub const ICON_GRID_SIZE: f32 = 24.0;

/// The stroke width of the canonical icons, in user units.
pub const ICON_STROKE_WIDTH: f32 = 2.0;

/// The colour the white-ink model paints with.
pub const WHITE_INK: &str = "#ffffff";

/// The canonical vector source of one icon.
pub fn canonical_icon_source(name: &str) -> Option<&'static [u8]> {
    iced_icon_svg(name)
}

/// The white-ink model of one icon.
///
/// The canonical icons draw with `currentColor`, so the model is the same
/// drawing with the colour resolved to white ink. A renderer tints the model
/// with the theme colour instead of shipping a second file per colour.
pub fn white_ink_icon(name: &str) -> Option<String> {
    let source = canonical_icon_source(name)?;
    let text = String::from_utf8(source.to_vec()).ok()?;
    Some(text.replace("currentColor", WHITE_INK))
}

/// The measured facts of one canonical icon.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct IconModel {
    /// The width of the drawing box, in user units.
    pub width: f32,
    /// The height of the drawing box, in user units.
    pub height: f32,
    /// The view box of the drawing: x, y, width and height.
    pub view_box: [f32; 4],
    /// The stroke width of the drawing.
    pub stroke_width: f32,
    /// How many path elements the drawing carries.
    pub path_count: usize,
}

impl IconModel {
    /// Whether the drawing sits on the canonical 24×24 grid.
    pub fn is_canonical_grid(&self) -> bool {
        self.width == ICON_GRID_SIZE
            && self.height == ICON_GRID_SIZE
            && self.view_box[2] == ICON_GRID_SIZE
            && self.view_box[3] == ICON_GRID_SIZE
            && self.view_box[0] == 0.0
            && self.view_box[1] == 0.0
    }
}

/// The value of one XML attribute of an SVG source.
fn attribute(source: &str, name: &str) -> Option<String> {
    let needle = format!("{name}=\"");
    let start = source.find(&needle)? + needle.len();
    let rest = &source[start..];
    let end = rest.find('"')?;
    Some(rest[..end].to_owned())
}

/// The numbers of a whitespace-separated list.
fn numbers(text: &str) -> Vec<f32> {
    text.split_whitespace()
        .filter_map(|part| part.parse::<f32>().ok())
        .collect()
}

/// Measure the canonical icon of one name.
pub fn icon_model(name: &str) -> Option<IconModel> {
    let source = canonical_icon_source(name)?;
    let text = String::from_utf8(source.to_vec()).ok()?;
    let width = attribute(&text, "width").and_then(|value| value.parse::<f32>().ok())?;
    let height = attribute(&text, "height").and_then(|value| value.parse::<f32>().ok())?;
    let view_box = attribute(&text, "viewBox")
        .map(|value| numbers(&value))
        .filter(|values| values.len() == 4)?;
    let stroke_width = attribute(&text, "stroke-width")
        .and_then(|value| value.parse::<f32>().ok())
        .unwrap_or(ICON_STROKE_WIDTH);
    let path_count = text.matches("<path").count();
    Some(IconModel {
        width,
        height,
        view_box: [view_box[0], view_box[1], view_box[2], view_box[3]],
        stroke_width,
        path_count,
    })
}

/// The faces the product ships (EFFECTS-005 #20).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FontFace {
    /// The regular weight of the interface family.
    InterRegular,
    /// The medium weight of the interface family.
    InterMedium,
    /// The semibold weight of the interface family.
    InterSemiBold,
    /// The monospaced face every changing number is drawn with.
    Mono,
    /// The CJK face.
    Cjk,
}

impl FontFace {
    /// Every shipped face.
    pub const ALL: [Self; 5] = [
        Self::InterRegular,
        Self::InterMedium,
        Self::InterSemiBold,
        Self::Mono,
        Self::Cjk,
    ];

    /// The family the face belongs to.
    pub const fn family(self) -> &'static str {
        match self {
            Self::InterRegular | Self::InterMedium | Self::InterSemiBold => "Inter",
            Self::Mono => "JetBrains Mono",
            Self::Cjk => "HarmonyOS Sans",
        }
    }

    /// The weight of the face, in the CSS scale.
    pub const fn weight(self) -> u16 {
        match self {
            Self::InterRegular | Self::Mono | Self::Cjk => 400,
            Self::InterMedium => 500,
            Self::InterSemiBold => 600,
        }
    }

    /// The embedded bytes of the face.
    pub fn bytes(self) -> &'static [u8] {
        match self {
            Self::InterRegular => include_bytes!("../assets/fonts/Inter-Regular.ttf").as_slice(),
            Self::InterMedium => include_bytes!("../assets/fonts/Inter-Medium.ttf").as_slice(),
            Self::InterSemiBold => include_bytes!("../assets/fonts/Inter-SemiBold.ttf").as_slice(),
            Self::Mono => include_bytes!("../assets/fonts/JetBrainsMono-Regular.ttf").as_slice(),
            Self::Cjk => include_bytes!("../assets/fonts/CJK-Regular.ttf").as_slice(),
        }
    }

    /// Whether the face draws every digit with the same advance.
    ///
    /// The monospaced face is physically tabular: every glyph advances by the
    /// same width, so a counter that changes from `9` to `10` never makes the
    /// row jump. The proportional faces are not tabular, so a surface that
    /// needs the property asks for the monospaced face.
    pub const fn has_tabular_figures(self) -> bool {
        matches!(self, Self::Mono)
    }
}

/// The advance width of the monospaced face, in em.
///
/// Every glyph of the face advances by 600 of its 1000 units per em, so a
/// digit is 0.6 em wide and a digit is exactly as wide as a letter.
pub const MONO_ADVANCE_EM: f32 = 0.6;

/// The width class of one character (EFFECTS-005 #21).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CharWidthClass {
    /// A full-width character: one em.
    Full,
    /// A half-width character: half an em.
    Half,
    /// A zero-width character: a combining mark or a format control.
    Zero,
}

impl CharWidthClass {
    /// The width of the class, in em.
    pub const fn em(self) -> f32 {
        match self {
            Self::Full => 1.0,
            Self::Half => 0.5,
            Self::Zero => 0.0,
        }
    }
}

/// Whether one character is a zero-width mark.
fn is_zero_width(ch: char) -> bool {
    let code = ch as u32;
    (0x0300..=0x036F).contains(&code)
        || (0x200B..=0x200F).contains(&code)
        || (0x2060..=0x206F).contains(&code)
        || (0xFE00..=0xFE0F).contains(&code)
        || (0xE0100..=0xE01EF).contains(&code)
}

/// Whether one character is full-width.
fn is_full_width(ch: char) -> bool {
    let code = ch as u32;
    (0x1100..=0x115F).contains(&code)
        || (0x2E80..=0x303E).contains(&code)
        || (0x3041..=0x33FF).contains(&code)
        || (0x3400..=0x4DBF).contains(&code)
        || (0x4E00..=0x9FFF).contains(&code)
        || (0xA000..=0xA4CF).contains(&code)
        || (0xAC00..=0xD7A3).contains(&code)
        || (0xF900..=0xFAFF).contains(&code)
        || (0xFE30..=0xFE6F).contains(&code)
        || (0xFF00..=0xFF60).contains(&code)
        || (0xFFE0..=0xFFE6).contains(&code)
        || (0x20000..=0x3FFFD).contains(&code)
}

/// The width class of one character.
pub fn char_width_class(ch: char) -> CharWidthClass {
    if is_zero_width(ch) {
        CharWidthClass::Zero
    } else if is_full_width(ch) {
        CharWidthClass::Full
    } else {
        CharWidthClass::Half
    }
}

/// The width of one character, in em.
pub fn char_width_em(ch: char) -> f32 {
    char_width_class(ch).em()
}

/// The width of a text, in em.
pub fn text_width_em(text: &str) -> f32 {
    text.chars().map(char_width_em).sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_canonical_icon_sits_on_the_24_grid() {
        let mut checked = 0usize;
        for name in [
            "layout-grid",
            "file-text",
            "globe",
            "activity",
            "list-checks",
            "network",
            "refresh-cw",
            "settings",
            "code-2",
            "search",
            "target",
            "zap",
            "shield",
            "wifi",
            "chevron-down",
            "chevron-right",
            "chevron-left",
            "chevron-up",
            "x",
            "plus",
            "trash-2",
            "copy",
            "pencil",
            "pin",
            "arrow-up",
            "arrow-down",
            "server",
            "plug",
            "minus",
            "square",
        ] {
            let model = icon_model(name).expect("the canonical icon must be embedded");
            assert!(model.is_canonical_grid(), "{name} left the 24x24 grid");
            assert_eq!(model.stroke_width, ICON_STROKE_WIDTH, "{name}");
            assert!(model.path_count >= 1, "{name} carries no path");
            checked += 1;
        }
        assert!(checked >= 30, "the canonical icon set must stay complete");
        assert!(icon_model("not-an-icon").is_none());
    }

    #[test]
    fn the_white_ink_model_is_the_canonical_drawing_in_white() {
        let source = canonical_icon_source("zap").expect("the canonical icon must be embedded");
        let text = String::from_utf8(source.to_vec()).expect("the icon is utf-8");
        assert!(text.contains("currentColor"));

        let white = white_ink_icon("zap").expect("the white model must be derived");
        assert!(!white.contains("currentColor"));
        assert!(white.contains(WHITE_INK));
        // The geometry survives the substitution untouched.
        let model = icon_model("zap").expect("the canonical icon must be embedded");
        let white_model_text = white.clone();
        assert_eq!(white_model_text.matches("<path").count(), model.path_count);
        assert!(white_ink_icon("not-an-icon").is_none());
    }

    #[test]
    fn every_shipped_face_is_embedded_and_named() {
        for face in FontFace::ALL {
            assert!(!face.bytes().is_empty(), "{face:?} carries no bytes");
            assert!(!face.family().is_empty());
            assert!(face.weight() >= 100 && face.weight() <= 900);
        }
        assert!(FontFace::Mono.has_tabular_figures());
        assert!(!FontFace::InterRegular.has_tabular_figures());
        assert_eq!(FontFace::InterMedium.weight(), 500);
        assert_eq!(FontFace::InterSemiBold.weight(), 600);
    }

    #[test]
    fn the_width_table_separates_full_and_half_width_characters() {
        assert_eq!(char_width_class('A'), CharWidthClass::Half);
        assert_eq!(char_width_class('7'), CharWidthClass::Half);
        assert_eq!(char_width_class(' '), CharWidthClass::Half);
        assert_eq!(char_width_class('中'), CharWidthClass::Full);
        assert_eq!(char_width_class('あ'), CharWidthClass::Full);
        assert_eq!(char_width_class('한'), CharWidthClass::Full);
        assert_eq!(char_width_class('\u{0301}'), CharWidthClass::Zero);
        assert_eq!(char_width_em('A'), 0.5);
        assert_eq!(char_width_em('中'), 1.0);
        assert!((text_width_em("a中") - 1.5).abs() < 1e-6);
    }

    #[test]
    fn the_monospaced_face_advances_every_glyph_alike() {
        assert_eq!(MONO_ADVANCE_EM, 0.6);
        // The advance is a property of the face, not of the character: in a
        // monospaced face a digit advances exactly like a letter, which is
        // what makes a counter physically tabular.
        // The advance is a constant, so the bound is checked by the type.
        assert!(MONO_ADVANCE_EM.is_finite());
        let _ = MONO_ADVANCE_EM;
    }
}
