//! The Iced glyph set resolves from the shared icon authority.
//!
//! `IconSemantic` is the one owner of "which pictogram means this destination";
//! `Icon::canonical_name` is this surface's name for a glyph. The resolver ties
//! the two together by name, so no page keeps a semantic-to-glyph table and a
//! semantic added to the contract cannot silently render as another glyph.

use super::*;
use infiltrator_contract::design_resources::IconSemantic;

#[test]
fn every_glyph_has_one_canonical_name() {
    let mut names: Vec<&str> = Icon::ALL.iter().map(|icon| icon.canonical_name()).collect();
    assert_eq!(names.len(), Icon::ALL.len());
    names.sort_unstable();
    names.dedup();
    assert_eq!(
        names.len(),
        Icon::ALL.len(),
        "two glyphs claim one catalog name: {names:?}"
    );
}

#[test]
fn every_semantic_resolves_onto_its_own_glyph() {
    for semantic in IconSemantic::ALL {
        assert_eq!(
            icon_for_semantic(semantic).canonical_name(),
            semantic.canonical_name(),
            "{} resolved onto another glyph",
            semantic.canonical_name()
        );
    }
}
