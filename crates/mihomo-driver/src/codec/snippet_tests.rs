//! Behavior checks for byte-faithful editor snippet splicing.
//! test-intent: behavior

use super::splice_at_caret;
use lurker_core_ports::profile_document_codec::ProfileDocumentCaret;

const SS_BODY: &str = "  - name: SS-Node\n    type: ss\n    server: server.example.com\n    port: 8388\n    cipher: aes-256-gcm\n    password: password\n";
const RULE_GEOIP_BODY: &str = "  - GEOIP,CN,DIRECT\n";
const RULE_DOMAIN_BODY: &str = "  - DOMAIN-SUFFIX,google.com,PROXIES\n";

#[test]
fn splice_preserves_the_rest_of_the_document() {
    let insertion = splice_at_caret(
        "rules:\n  - MATCH,DIRECT",
        "rule-geoip",
        RULE_GEOIP_BODY,
        ProfileDocumentCaret {
            line: 2,
            column: 16,
        },
    );
    assert_eq!(
        insertion.content,
        "rules:\n  - MATCH,DIRECT\n  - GEOIP,CN,DIRECT\n"
    );
    assert_eq!(insertion.snippet_id, "rule-geoip");
    // The caret lands on the fresh line after the inserted item.
    assert_eq!(insertion.cursor_line, 4);
    assert_eq!(insertion.cursor_column, 0);
}

#[test]
fn a_caret_inside_a_line_starts_a_fresh_line() {
    let insertion = splice_at_caret(
        "proxies:\n  - name: keep\n",
        "rule-domain",
        RULE_DOMAIN_BODY,
        ProfileDocumentCaret { line: 2, column: 4 },
    );
    assert_eq!(
        insertion.content,
        "proxies:\n  - \n  - DOMAIN-SUFFIX,google.com,PROXIES\nname: keep\n"
    );
}

#[test]
fn an_empty_line_receives_the_snippet_without_a_leading_blank() {
    let insertion = splice_at_caret(
        "proxies:\n\n",
        "ss",
        SS_BODY,
        ProfileDocumentCaret { line: 2, column: 0 },
    );
    assert_eq!(
        insertion.content,
        "proxies:\n  - name: SS-Node\n    type: ss\n    server: server.example.com\n    port: 8388\n    cipher: aes-256-gcm\n    password: password\n\n"
    );
    assert_eq!(insertion.cursor_line, 8);
    assert_eq!(insertion.cursor_column, 0);
}

#[test]
fn an_out_of_range_caret_is_clamped_to_the_document() {
    let insertion = splice_at_caret(
        "proxies:\n",
        "ss",
        SS_BODY,
        ProfileDocumentCaret {
            line: 99,
            column: 99,
        },
    );
    assert_eq!(
        insertion.content,
        "proxies:\n  - name: SS-Node\n    type: ss\n    server: server.example.com\n    port: 8388\n    cipher: aes-256-gcm\n    password: password\n"
    );
}

#[test]
fn a_multibyte_line_splices_at_the_character_column() {
    let insertion = splice_at_caret(
        "proxies:\n  # 手写注释\n",
        "ss",
        SS_BODY,
        ProfileDocumentCaret { line: 2, column: 8 },
    );
    assert_eq!(
        insertion.content,
        "proxies:\n  # 手写注释\n  - name: SS-Node\n    type: ss\n    server: server.example.com\n    port: 8388\n    cipher: aes-256-gcm\n    password: password\n\n"
    );
}
