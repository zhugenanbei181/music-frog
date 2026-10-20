//! Neutral text-catalog port and single-pass placeholder interpolation.
//!
//! The presentation kernel owns localization **keys** and the interpolation
//! rules; every product keeps its own catalog data and implements
//! [`TextCatalog`] on top of its real localizer. Nothing here selects a
//! language, reads a `.po` file, or embeds user-facing copy.

use std::borrow::Cow;

/// Renders canonical copy for one stable localization key.
///
/// Implementations resolve the key against their own catalog. The signature
/// matches the existing product localizer (`fn tr(&self, key: &str) ->
/// Cow<'static, str>`) so a product adapter is a single delegating `impl`.
pub trait TextCatalog {
    /// Resolves one stable localization key against the product catalog.
    fn text(&self, key: &str) -> Cow<'static, str>;
}

/// Resolve `{placeholder}` occurrences in `template` once.
///
/// Inserted values stay opaque: a placeholder that appears inside a value is
/// never expanded a second time, and unknown or unterminated placeholders are
/// preserved verbatim.
pub fn interpolate(template: &str, pairs: &[(&str, &str)]) -> String {
    interpolate_with(template, |key| {
        pairs
            .iter()
            .find(|(name, _)| *name == key)
            .map(|(_, value)| *value)
    })
}

/// Resolve placeholders through an arbitrary lookup; used for owned values.
///
/// This is the single interpolation algorithm in the workspace: the product
/// localizer delegates here instead of keeping a second copy.
pub fn interpolate_with<'a>(
    template: &str,
    mut lookup: impl FnMut(&str) -> Option<&'a str>,
) -> String {
    let mut output = String::with_capacity(template.len());
    let mut remaining = template;
    while let Some(start) = remaining.find('{') {
        output.push_str(&remaining[..start]);
        let tail = &remaining[start + 1..];
        let Some(end) = tail.find('}') else {
            output.push_str(&remaining[start..]);
            return output;
        };
        if let Some(value) = lookup(&tail[..end]) {
            output.push_str(value);
        } else {
            output.push_str(&remaining[start..start + end + 2]);
        }
        remaining = &tail[end + 1..];
    }
    output.push_str(remaining);
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interpolated_values_stay_opaque_and_order_does_not_matter() {
        let pairs = [("user", "User {count} / 日本"), ("count", "3")];
        assert_eq!(
            interpolate("{user} · {count}", &pairs),
            "User {count} / 日本 · 3"
        );
        assert_eq!(
            interpolate("{user} · {count}", &[pairs[1], pairs[0]]),
            "User {count} / 日本 · 3"
        );
    }

    #[test]
    fn unknown_or_unterminated_placeholders_are_preserved() {
        assert_eq!(
            interpolate("{known} {missing} {unfinished", &[("known", "value")]),
            "value {missing} {unfinished"
        );
    }
}
