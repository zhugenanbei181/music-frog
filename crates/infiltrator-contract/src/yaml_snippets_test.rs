//! DUAL-09-04: shared snippet catalogue and byte-faithful splice tests.

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
        assert!(!snippet.label_zh.is_empty());
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
    // with the shared YAML preflight.
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
fn insert_at_caret_splices_without_touching_the_rest_of_the_document() {
    let content = "rules:\n  - MATCH,DIRECT";
    let insertion = insert_at_caret(
        content,
        "rule-geoip",
        SnippetCaret {
            line: 2,
            column: 16,
        },
    )
    .expect("catalogue id");
    assert_eq!(
        insertion.content,
        "rules:\n  - MATCH,DIRECT\n  - GEOIP,CN,DIRECT\n"
    );
    assert_eq!(insertion.snippet_id, "rule-geoip");
    assert!(insertion.is_clean());
    // The caret lands on the fresh line after the inserted item.
    assert_eq!(insertion.cursor_line, 4);
    assert_eq!(insertion.cursor_column, 0);
}

#[test]
fn a_caret_inside_a_line_starts_a_fresh_line() {
    let content = "proxies:\n  - name: keep\n";
    let insertion = insert_at_caret(content, "rule-domain", SnippetCaret { line: 2, column: 4 })
        .expect("catalogue id");
    assert_eq!(
        insertion.content,
        "proxies:\n  - \n  - DOMAIN-SUFFIX,google.com,PROXIES\nname: keep\n"
    );
}

#[test]
fn an_empty_line_receives_the_snippet_without_a_leading_blank() {
    let content = "proxies:\n\n";
    let insertion =
        insert_at_caret(content, "ss", SnippetCaret { line: 2, column: 0 }).expect("catalogue id");
    assert_eq!(
        insertion.content,
        "proxies:\n  - name: SS-Node\n    type: ss\n    server: server.example.com\n    port: 8388\n    cipher: aes-256-gcm\n    password: password\n\n"
    );
    assert_eq!(insertion.cursor_line, 8);
    assert_eq!(insertion.cursor_column, 0);
}

#[test]
fn an_out_of_range_caret_is_clamped_to_the_document() {
    let insertion = insert_at_caret(
        "proxies:\n",
        "ss",
        SnippetCaret {
            line: 99,
            column: 99,
        },
    )
    .expect("catalogue id");
    assert_eq!(
        insertion.content,
        "proxies:\n  - name: SS-Node\n    type: ss\n    server: server.example.com\n    port: 8388\n    cipher: aes-256-gcm\n    password: password\n"
    );
}

#[test]
fn an_unknown_id_is_an_error_never_a_silent_no_op() {
    let error = insert_at_caret("proxies:\n", "nope", SnippetCaret { line: 1, column: 0 })
        .expect_err("unknown ids are rejected");
    assert_eq!(error, SnippetInsertError::UnknownSnippet("nope".to_owned()));
    assert!(error.to_string().contains("nope"));
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

#[test]
fn a_multibyte_line_splices_at_the_character_column() {
    let insertion = insert_at_caret(
        "proxies:\n  # 手写注释\n",
        "ss",
        SnippetCaret { line: 2, column: 8 },
    )
    .expect("catalogue id");
    assert_eq!(
        insertion.content,
        "proxies:\n  # 手写注释\n  - name: SS-Node\n    type: ss\n    server: server.example.com\n    port: 8388\n    cipher: aes-256-gcm\n    password: password\n\n"
    );
}
