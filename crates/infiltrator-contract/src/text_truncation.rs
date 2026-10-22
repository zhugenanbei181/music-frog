//! The long-text truncation contract (EFFECTS-006 #22).
//!
//! A long text is never cut by throwing characters away: the product shows
//! the longest prefix that still fits, marks the cut with an ellipsis and
//! fades the tail, so a truncated label reads as "there is more" instead of
//! as a different, shorter word.
//!
//! The contract owns the *rule*; the measurement of a character belongs to
//! the layer that owns the font metrics, so this module asks a
//! [`TextMeasure`] instead of guessing a width.

/// The mark that shows a text was cut.
pub const ELLIPSIS: &str = "…";

/// The width of the fade at the cut, in logical pixels.
pub const FADE_WIDTH_PX: f32 = 24.0;

/// How the characters of a text are measured.
pub trait TextMeasure {
    /// The width of one character, in logical pixels.
    fn char_width(&self, ch: char) -> f32;
}

/// A measure that gives every character the same width.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct UniformCharWidth {
    /// The width of one character, in logical pixels.
    pub width_px: f32,
}

impl UniformCharWidth {
    /// A uniform measure of the given width.
    pub const fn new(width_px: f32) -> Self {
        Self { width_px }
    }
}

impl TextMeasure for UniformCharWidth {
    fn char_width(&self, _ch: char) -> f32 {
        self.width_px
    }
}

/// How one long text fits into its box.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TruncationPlan {
    /// The number of characters that stay visible.
    pub visible_chars: usize,
    /// The number of characters the whole text has.
    pub full_chars: usize,
    /// Whether an ellipsis marks the cut.
    pub has_ellipsis: bool,
    /// Whether the tail of the visible text fades out.
    pub has_fade: bool,
    /// The width of the visible prefix, in logical pixels.
    pub visible_width_px: f32,
    /// The width of the whole text, in logical pixels.
    pub full_width_px: f32,
}

impl TruncationPlan {
    /// Whether the text was cut at all.
    pub const fn is_truncated(&self) -> bool {
        self.visible_chars < self.full_chars
    }
}

/// The width of a text, in logical pixels.
pub fn measure_text_width<M>(text: &str, measure: &M) -> f32
where
    M: TextMeasure + ?Sized,
{
    text.chars().map(|ch| measure.char_width(ch)).sum()
}

/// Plan how a long text fits into a box.
///
/// The plan keeps the longest prefix whose width plus the ellipsis still fits
/// inside `available_px`. A text that already fits is returned whole, without
/// an ellipsis and without a fade.
pub fn plan_truncation<M>(text: &str, available_px: f32, measure: &M) -> TruncationPlan
where
    M: TextMeasure + ?Sized,
{
    let full_width_px = measure_text_width(text, measure);
    let full_chars = text.chars().count();
    if !available_px.is_finite() || available_px <= 0.0 {
        return TruncationPlan {
            visible_chars: 0,
            full_chars,
            has_ellipsis: false,
            has_fade: false,
            visible_width_px: 0.0,
            full_width_px,
        };
    }
    if full_width_px <= available_px {
        return TruncationPlan {
            visible_chars: full_chars,
            full_chars,
            has_ellipsis: false,
            has_fade: false,
            visible_width_px: full_width_px,
            full_width_px,
        };
    }

    let ellipsis_width = measure_text_width(ELLIPSIS, measure);
    let budget = (available_px - ellipsis_width).max(0.0);
    let mut used = 0.0;
    let mut visible = 0usize;
    for (index, ch) in text.chars().enumerate() {
        let width = measure.char_width(ch);
        if used + width > budget {
            break;
        }
        used += width;
        visible = index + 1;
    }
    let has_ellipsis = ellipsis_width <= available_px;
    TruncationPlan {
        visible_chars: visible,
        full_chars,
        has_ellipsis,
        has_fade: visible > 0,
        visible_width_px: used,
        full_width_px,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn uniform() -> UniformCharWidth {
        UniformCharWidth::new(10.0)
    }

    #[test]
    fn a_text_that_fits_is_returned_whole() {
        let plan = plan_truncation("abcde", 200.0, &uniform());
        assert_eq!(plan.visible_chars, 5);
        assert_eq!(plan.full_chars, 5);
        assert!(!plan.has_ellipsis);
        assert!(!plan.has_fade);
        assert!(!plan.is_truncated());
        assert_eq!(plan.full_width_px, 50.0);
        assert_eq!(plan.visible_width_px, 50.0);
    }

    #[test]
    fn a_long_text_keeps_the_longest_prefix_that_fits() {
        // Ten characters of 10px fit into 110px, so nothing is cut.
        let plan = plan_truncation("abcdefghij", 110.0, &uniform());
        assert_eq!(plan.visible_chars, 10);
        assert!(!plan.has_ellipsis);
        assert!(!plan.is_truncated());

        // Into 55px only four characters fit beside the 10px ellipsis.
        let plan = plan_truncation("abcdefghij", 55.0, &uniform());
        assert_eq!(plan.visible_chars, 4);
        assert!(plan.has_ellipsis);
        assert!(plan.has_fade);
        assert!(plan.is_truncated());
        assert_eq!(plan.visible_width_px, 40.0);
    }

    #[test]
    fn a_box_too_small_for_the_ellipsis_keeps_nothing() {
        let plan = plan_truncation("abcdef", 5.0, &uniform());
        assert_eq!(plan.visible_chars, 0);
        assert_eq!(plan.full_chars, 6);
        assert!(!plan.has_ellipsis);
        assert!(!plan.has_fade);
        assert!(plan.is_truncated());
    }

    #[test]
    fn a_box_without_width_keeps_nothing() {
        for available in [0.0, -10.0, f32::NAN] {
            let plan = plan_truncation("abc", available, &uniform());
            assert_eq!(plan.visible_chars, 0);
            assert!(!plan.has_ellipsis);
        }
    }

    #[test]
    fn a_wide_character_costs_more_than_a_narrow_one() {
        struct Mixed;
        impl TextMeasure for Mixed {
            fn char_width(&self, ch: char) -> f32 {
                if ch == '中' { 20.0 } else { 10.0 }
            }
        }
        // "a中b中c" is 70px wide; into 60px only "a中b" fits beside the
        // 10px ellipsis.
        let plan = plan_truncation("a中b中c", 60.0, &Mixed);
        assert_eq!(plan.visible_chars, 3);
        assert!(plan.has_ellipsis);
        assert_eq!(plan.full_width_px, 70.0);
        assert_eq!(plan.visible_width_px, 40.0);
    }
}
