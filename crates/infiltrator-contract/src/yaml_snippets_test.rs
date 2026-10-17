//! DUAL-09-04: shared snippet catalogue tests.
//!
//! Splice mechanics are covered by `mihomo_driver::codec::snippet`; this test
//! only pins the catalogue that the product adapter injects into the driver.

use super::*;

#[test]
fn catalogue_ids_and_locale_keys_are_unique_and_stable() {
    let mut ids: Vec<&str> = YAML_SNIPPETS.iter().map(|snippet| snippet.id).collect();
    ids.sort_unstable();
    let unique = ids.len();
    ids.dedup();
    assert_eq!(ids.len(), unique, "snippet ids must be unique");
    assert!(YAML_SNIPPETS.len() >= 8);
    for snippet in YAML_SNIPPETS {
        assert!(
            snippet.label_key.starts_with("yaml_snippet_"),
            "{} carries an i18n key",
            snippet.id
        );
        assert!(!snippet.body.is_empty());
        assert_eq!(
            yaml_snippet(snippet.id).map(|found| found.id),
            Some(snippet.id)
        );
    }
}

#[test]
fn every_catalogue_entry_keeps_a_real_document_parseable() {
    // The catalogue is the contract, so this test pins the *shape* of every
    // body: a snippets test in the application layer re-checks the same bodies
    // with the identity-scoped codec inspection path.
    for snippet in YAML_SNIPPETS {
        assert!(
            snippet.body.ends_with('\n'),
            "{} ends with a newline",
            snippet.id
        );
        assert!(
            snippet.body.starts_with("  - "),
            "{} is a two-space-indented list item",
            snippet.id
        );
    }
}

#[test]
fn byte_and_character_columns_round_trip_on_multibyte_lines() {
    let line = "  # 中文注释: ok";
    let column = 6;
    let offset = byte_offset_of_column(line, column);
    assert_eq!(&line[..offset], "  # 中文");
    assert_eq!(column_of_byte_offset(line, offset), column);
    assert_eq!(byte_offset_of_column(line, 999), line.len());
    assert_eq!(column_of_byte_offset(line, 999), line.chars().count());
    // An offset inside a character clamps back onto its boundary.
    assert_eq!(column_of_byte_offset("中", 1), 0);
}
