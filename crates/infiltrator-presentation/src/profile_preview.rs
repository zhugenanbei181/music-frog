//! Safe read-only preview of profile documents for all UI surfaces.
//!
//! The source document remains owner-authored and unchanged. This projection
//! masks credential-bearing YAML values before a UI displays a preview.

use lurker_core_contract::redact::redact_line;
use serde_yaml_ng::Value;

/// Maximum byte size of a document allowed to undergo recursive YAML AST
/// construction for preview redaction. Documents exceeding this threshold
/// skip AST allocation and use single-pass conservative line scrubbing,
/// ensuring 50k-scale documents never block the UI thread.
pub const MAX_AST_REDACTION_BYTES: usize = 65_536;

/// Return a display-only copy of a profile document with common Mihomo and
/// sing-box credential fields replaced by `***`.
///
/// Valid YAML is traversed structurally, including nested maps, sequences,
/// and block scalars. If the input is not valid YAML, a conservative line
/// scrubber masks recognizable credential assignments instead of returning
/// the original bytes unredacted.
pub fn redact_profile_preview(source: &str) -> String {
    redact_profile_preview_bounded(source, usize::MAX)
}

/// Bounded variant that caps the rendered preview to `max_lines`
/// for responsive performance on large (4k..50k line) documents.
pub fn redact_profile_preview_bounded(source: &str, max_lines: usize) -> String {
    let (truncated_source, hidden_lines) = if max_lines < usize::MAX {
        let mut count = 0;
        let mut cutoff = None;
        for (idx, line) in source.split_inclusive('\n').enumerate() {
            if idx >= max_lines {
                cutoff = Some(count);
                break;
            }
            count += line.len();
        }
        if let Some(byte_cutoff) = cutoff {
            let total = source.lines().count();
            (&source[..byte_cutoff], total.saturating_sub(max_lines))
        } else {
            (source, 0)
        }
    } else {
        (source, 0)
    };

    let mut result = if truncated_source.len() <= MAX_AST_REDACTION_BYTES {
        match serde_yaml_ng::from_str::<Value>(truncated_source) {
            Ok(mut document) => {
                redact_value(&mut document, false);
                serde_yaml_ng::to_string(&document)
                    .unwrap_or_else(|_| redact_lines_conservatively(truncated_source))
            }
            Err(_) => redact_lines_conservatively(truncated_source),
        }
    } else {
        redact_lines_conservatively(truncated_source)
    };

    if hidden_lines > 0 {
        result.push_str(&format!(
            "\n# ... [{} lines hidden for preview performance]\n",
            hidden_lines
        ));
    }
    result
}

fn redact_value(value: &mut Value, sensitive: bool) {
    if sensitive {
        *value = Value::String("***".to_owned());
        return;
    }
    match value {
        Value::String(text) => *text = redact_line(text, &[]),
        Value::Sequence(items) => {
            for item in items {
                redact_value(item, false);
            }
        }
        Value::Mapping(entries) => {
            for (key, value) in entries {
                let secret = key.as_str().is_some_and(is_sensitive_key);
                redact_value(value, secret);
            }
        }
        Value::Tagged(tagged) => redact_value(&mut tagged.value, false),
        Value::Null | Value::Bool(_) | Value::Number(_) => {}
    }
}

fn is_sensitive_key(key: &str) -> bool {
    let normalized = key
        .trim()
        .trim_matches(|character| character == '\'' || character == '"')
        .to_ascii_lowercase()
        .replace('_', "-");
    matches!(
        normalized.as_str(),
        "password"
            | "passwd"
            | "secret"
            | "token"
            | "uuid"
            | "auth"
            | "authorization"
            | "username"
            | "user-name"
            | "key"
            | "client-key"
            | "private-key"
            | "privatekey"
            | "psk"
    ) || normalized.ends_with("-password")
        || normalized.ends_with("-secret")
        || normalized.ends_with("-token")
        || normalized.ends_with("-private-key")
}

fn redact_lines_conservatively(source: &str) -> String {
    let mut output = String::new();
    let mut block_indent: Option<usize> = None;
    for raw_line in source.split_inclusive('\n') {
        let line = raw_line.trim_end_matches(|character| character == '\r' || character == '\n');
        let indent = line.len()
            - line
                .trim_start_matches(|character| character == ' ' || character == '\t')
                .len();
        if let Some(required_indent) = block_indent {
            if line.trim().is_empty() || indent > required_indent {
                output.push_str(&line[..indent]);
                output.push_str("***");
                output.push_str(&raw_line[line.len()..]);
                continue;
            }
            block_indent = None;
        }

        let scrubbed = redact_line(line, &[]);
        let trimmed =
            scrubbed.trim_start_matches(|character| character == ' ' || character == '\t');
        let line_indent = scrubbed.len() - trimmed.len();
        if let Some((key, value)) = trimmed.split_once(':')
            && is_sensitive_key(key)
        {
            let value_indent = value.len()
                - value
                    .trim_start_matches(|character| character == ' ' || character == '\t')
                    .len();
            let value = &value[value_indent..];
            if matches!(value.trim(), "|" | ">" | "|-" | ">-" | "|+" | ">+") {
                block_indent = Some(line_indent);
            }
            output.push_str(&scrubbed[..line_indent]);
            output.push_str(key);
            output.push_str(": ***");
            output.push_str(&raw_line[line.len()..]);
        } else {
            output.push_str(&scrubbed);
            output.push_str(&raw_line[line.len()..]);
        }
    }
    output
}

#[cfg(test)]
mod tests {
    use super::{redact_profile_preview, redact_profile_preview_bounded};

    #[test]
    fn yaml_preview_masks_nested_credentials_and_keeps_non_secret_profile_data() {
        let source = "proxies:\n  - name: Example\n    uuid: 12345678-1234-1234-1234-123456789abc\n    password: node-password\n    server: edge.example.test\n    reality-opts:\n      private-key: a-private-key\n";
        let preview = redact_profile_preview(source);
        for secret in [
            "12345678-1234-1234-1234-123456789abc",
            "node-password",
            "a-private-key",
        ] {
            assert!(!preview.contains(secret), "preview leaked {secret}");
        }
        assert!(preview.contains("Example"));
        assert!(preview.contains("edge.example.test"));
    }

    #[test]
    fn malformed_yaml_fallback_masks_credentials_and_block_values() {
        let source = "uuid: 12345678\nprivate-key: |\n  PRIVATE KEY MATERIAL\n  SECOND SECRET LINE\nserver: edge.example.test\n";
        let preview = redact_profile_preview(source);
        assert!(!preview.contains("12345678"));
        assert!(!preview.contains("PRIVATE KEY MATERIAL"));
        assert!(!preview.contains("SECOND SECRET LINE"));
        assert!(preview.contains("server: edge.example.test"));
    }

    #[test]
    fn bounded_preview_truncates_large_documents_and_redacts_efficiently() {
        let mut source = String::from("proxies:\n");
        for index in 0..5_000 {
            source.push_str(&format!(
                "  - name: node-{index}\n    password: secret-{index}\n"
            ));
        }
        let preview = redact_profile_preview_bounded(&source, 50);
        assert!(preview.contains("node-0"));
        assert!(!preview.contains("secret-0"));
        assert!(preview.contains("lines hidden for preview performance"));
        assert!(!preview.contains("node-4999"));
    }
}
