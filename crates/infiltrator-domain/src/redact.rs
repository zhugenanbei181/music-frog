//! Structural log redaction of credential-bearing fragments (CORE-001).
//!
//! Log lines written around profiles, subscriptions and the controller
//! routinely carry credentials: subscription URLs embed access tokens,
//! profile YAML embeds node passwords and the controller `secret`, and
//! HTTP traces show `Authorization` headers. [`redact_line`] strips those
//! fragments with plain string scans — no regex, no new dependencies — so
//! any caller can sanitize a line before handing it to a logger.

/// Replacement marker substituted for every redacted fragment.
const MASK: &str = "***";

/// Secrets shorter than this are skipped by [`redact_line`]: masking 1-3
/// character strings would garble unrelated log text for no protection.
const MIN_SECRET_LEN: usize = 4;

/// Keys whose value is always treated as a credential.
const SENSITIVE_KEYS: [&str; 4] = ["secret", "password", "token", "authorization"];

/// Query parameter names whose value is always treated as a credential.
const SENSITIVE_QUERY_KEYS: [&str; 2] = ["token", "key"];

/// Structural redaction of credential-bearing fragments in log lines.
///
/// Applied in order:
/// 1. entries of `secrets` that are empty or shorter than 4 characters are
///    skipped; longer entries are replaced by `***` wherever they appear
///    (longest first, so a secret that prefixes another cannot leak partly);
/// 2. `secret` / `password` / `token` / `authorization` key/value pairs
///    (case-insensitive, `:` or `=` separated, optionally quoted) keep the
///    key and have their value replaced by `***`;
/// 3. the password part of `scheme://user:password@host` userinfo URLs is
///    replaced by `***`;
/// 4. `token` / `key` values inside URL queries (`?token=…&key=…`) are
///    replaced by `***`.
///
/// Everything else is preserved byte-for-byte, and the transform is
/// idempotent: running it on its own output yields the same string because
/// already-masked values are rewritten to the same `***`.
///
/// Limitation: only a `Bearer` scheme word is recognized in front of a
/// credential. Other multi-word header values (e.g. `Basic <base64>`) have
/// just their first word replaced; pass such credentials explicitly via
/// `secrets` when they can appear in logs.
pub fn redact_line(line: &str, secrets: &[String]) -> String {
    // Explicit secrets go first: they must also cover fragments the
    // structural rules cannot see (bare tokens in prose). Longest first so
    // that replacing a prefix cannot leave a partial secret behind.
    let mut masked = line.to_string();
    let mut explicit: Vec<&str> = secrets.iter().map(String::as_str).collect();
    explicit.sort_by_key(|secret| std::cmp::Reverse(secret.len()));
    for secret in explicit {
        masked = mask_secret(&masked, secret);
    }

    let masked = mask_key_values(&masked);
    let masked = mask_url_userinfo(&masked);
    mask_query_tokens(&masked)
}

/// Convenience: mask a single secret occurrence everywhere it appears.
///
/// Same length guard as [`redact_line`]: empty and sub-4-character secrets
/// are returned unchanged instead of scribbling `***` across the line.
pub fn mask_secret(line: &str, secret: &str) -> String {
    if is_maskable(secret) {
        replace_all(line, secret)
    } else {
        line.to_string()
    }
}

/// Whether `line[at..]` starts with `name`, comparing ASCII case
/// insensitively. Byte-based so multi-byte UTF-8 cannot split a `char`.
fn starts_with_ignore_case(line: &str, at: usize, name: &str) -> bool {
    let bytes = line.as_bytes();
    bytes.len() >= at + name.len()
        && bytes[at..at + name.len()]
            .iter()
            .zip(name.as_bytes())
            .all(|(a, b)| a.eq_ignore_ascii_case(b))
}

/// A secret qualifies for masking only when it is long enough that the
/// replacement cannot be worse than the disease.
fn is_maskable(secret: &str) -> bool {
    !secret.is_empty() && secret.len() >= MIN_SECRET_LEN
}

/// Replace every occurrence of `needle` in `line` with [`MASK`].
fn replace_all(line: &str, needle: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut rest = line;
    while let Some(pos) = rest.find(needle) {
        out.push_str(&rest[..pos]);
        out.push_str(MASK);
        rest = &rest[pos + needle.len()..];
    }
    out.push_str(rest);
    out
}

/// Apply rule 2: mask values of sensitive `key separator value` pairs.
fn mask_key_values(line: &str) -> String {
    let bytes = line.as_bytes();
    let mut out = String::with_capacity(line.len());
    let mut i = 0;
    while i < bytes.len() {
        if let Some((end, replacement)) = match_key_value_at(line, i) {
            out.push_str(&replacement);
            i = end;
        } else {
            let ch = line[i..].chars().next().expect("non-empty remainder");
            out.push(ch);
            i += ch.len_utf8();
        }
    }
    out
}

/// Try to match `<key> <separator> <value>` starting exactly at `start`.
///
/// Returns the index just past the value plus the replacement text for
/// `start..index`. The key must be one of [`SENSITIVE_KEYS`] (case
/// insensitive), must not continue a longer word on either side, and the
/// value is either quoted (runs to the closing quote) or bare (runs to the
/// next delimiter). A `:` separator after an unquoted key additionally
/// requires following whitespace so URLs (`token://…`, userinfo) never
/// match as key/value pairs.
fn match_key_value_at(line: &str, start: usize) -> Option<(usize, String)> {
    let bytes = line.as_bytes();

    // Left word boundary: `mysecret: v` must not match as `secret: v`.
    if let Some(prev) = start.checked_sub(1).and_then(|p| bytes.get(p))
        && (prev.is_ascii_alphanumeric() || *prev == b'_' || *prev == b'-')
    {
        return None;
    }

    let mut idx = start;
    let key_quote = match bytes.get(idx) {
        Some(q @ (b'"' | b'\'')) => {
            let q = *q;
            idx += 1;
            Some(q)
        }
        _ => None,
    };

    let key = SENSITIVE_KEYS
        .into_iter()
        .find(|key| starts_with_ignore_case(line, idx, key))?;
    idx += key.len();

    if let Some(q) = key_quote {
        if bytes.get(idx) != Some(&q) {
            return None;
        }
        idx += 1;
    }

    // Right word boundary: only quotes, whitespace or a separator may
    // follow, so `secrets` / `token2` / `secret-key` never match.
    match bytes.get(idx) {
        Some(b'"' | b'\'' | b' ' | b'\t' | b':' | b'=') | None => {}
        _ => return None,
    }

    while matches!(bytes.get(idx), Some(b' ' | b'\t')) {
        idx += 1;
    }
    let sep = match bytes.get(idx) {
        Some(s @ (b':' | b'=')) => *s,
        _ => return None,
    };
    idx += 1;
    if sep == b':' && key_quote.is_none() {
        // Without a quoted key a bare `:` needs whitespace after it
        // (YAML/header style); this keeps `token://…` and the userinfo in
        // `token:pass@host` out of this rule.
        match bytes.get(idx) {
            Some(b' ' | b'\t') => {}
            _ => return None,
        }
    }
    while matches!(bytes.get(idx), Some(b' ' | b'\t')) {
        idx += 1;
    }

    let quote = match bytes.get(idx) {
        Some(q @ (b'"' | b'\'')) => {
            let q = *q;
            idx += 1;
            Some(q)
        }
        _ => None,
    };
    let value_start = idx;
    let value_end = match quote {
        Some(q) => bytes[value_start..]
            .iter()
            .position(|b| *b == q)
            .map(|p| value_start + p)?,
        None => {
            let end = unquoted_value_end(line, value_start);
            if is_bearer_word(&line[value_start..end]) {
                // The token lives beyond the whitespace that ended the bare
                // value, so resume scanning after the scheme word.
                let after_scheme = value_start + 6;
                let token_start = after_scheme
                    + (line[after_scheme..].len()
                        - line[after_scheme..].trim_start_matches([' ', '\t']).len());
                if token_start > after_scheme && token_start < line.len() {
                    unquoted_value_end(line, token_start)
                } else {
                    end
                }
            } else {
                end
            }
        }
    };
    if value_end == value_start {
        return None;
    }

    let value = &line[value_start..value_end];
    let end = if quote.is_some() {
        value_end + 1
    } else {
        value_end
    };
    let mut replacement = String::with_capacity(end - start);
    replacement.push_str(&line[start..value_start]);
    replacement.push_str(&mask_value(value));
    if let Some(q) = quote {
        replacement.push(q as char);
    }
    Some((end, replacement))
}

/// Redact one key/value value, keeping a leading `Bearer` scheme word so
/// `Authorization: Bearer xyz` becomes `Authorization: Bearer ***`.
fn mask_value(value: &str) -> String {
    match bearer_tail(value) {
        // `Bearer` with no token after it: nothing to hide, keep as is.
        Some("") => value.to_string(),
        Some(tail) => format!("{}{MASK}", &value[..value.len() - tail.len()]),
        None => MASK.to_string(),
    }
}

/// Whether `word` is exactly the `Bearer` scheme word (case-insensitive).
fn is_bearer_word(word: &str) -> bool {
    word.len() == 6 && word.as_bytes().eq_ignore_ascii_case(b"bearer")
}

/// The token tail when `value` starts with the `Bearer` scheme word
/// (case-insensitive) followed by whitespace; `None` otherwise. An empty
/// tail means the value is a bare scheme word with nothing to mask.
fn bearer_tail(value: &str) -> Option<&str> {
    let bytes = value.as_bytes();
    if bytes.len() < 6 || !bytes[..6].eq_ignore_ascii_case(b"bearer") {
        return None;
    }
    let after = &value[6..];
    let tail = after.trim_start_matches([' ', '\t']);
    (tail.len() != after.len()).then_some(tail)
}

/// Index where a bare value ends: the first delimiter or end of line.
fn unquoted_value_end(line: &str, start: usize) -> usize {
    line[start..]
        .char_indices()
        .find(|(_, ch)| is_value_delimiter(*ch))
        .map(|(p, _)| start + p)
        .unwrap_or(line.len())
}

fn is_value_delimiter(ch: char) -> bool {
    matches!(
        ch,
        ' ' | '\t'
            | '\n'
            | '\r'
            | '&'
            | ','
            | ';'
            | ')'
            | ']'
            | '}'
            | '"'
            | '\''
            | '<'
            | '>'
            | '`'
            | '#'
    )
}

/// Apply rule 3: mask the password inside `scheme://user:password@host`.
///
/// The scheme must end at `://`, and only the authority component (up to
/// the first `/`, `?` or `#`) is inspected, so query strings containing `@`
/// are never misread as userinfo.
fn mask_url_userinfo(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut rest = line;
    while let Some(pos) = rest.find("://") {
        let before = &rest[..pos];
        let after = &rest[pos + 3..];
        let auth_end = after.find(['/', '?', '#']).unwrap_or(after.len());
        let authority = &after[..auth_end];

        let mut masked = false;
        if ends_with_scheme(before)
            && let Some(at) = authority.find('@')
            && let Some(colon) = authority[..at].rfind(':')
            && !authority[colon + 1..at].is_empty()
        {
            out.push_str(before);
            out.push_str("://");
            out.push_str(&authority[..colon + 1]);
            out.push_str(MASK);
            out.push_str(&authority[at..]);
            rest = &after[auth_end..];
            masked = true;
        }
        if !masked {
            out.push_str(before);
            out.push_str("://");
            rest = after;
        }
    }
    out.push_str(rest);
    out
}

/// Whether `before` ends in a plausible URL scheme run (`https`, `socks5`).
fn ends_with_scheme(before: &str) -> bool {
    let mut chars = before.chars().rev();
    match chars.next() {
        Some(c) if c.is_ascii_alphanumeric() => {}
        _ => return false,
    }
    for c in chars {
        if !(c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.')) {
            break;
        }
    }
    true
}

/// Apply rule 4: mask `token` / `key` values inside URL queries.
///
/// Only parameters introduced by `?` or `&` count, so a bare `key=` in
/// prose is left alone.
fn mask_query_tokens(line: &str) -> String {
    let bytes = line.as_bytes();
    let mut out = String::with_capacity(line.len());
    let mut i = 0;
    while i < bytes.len() {
        if (bytes[i] == b'?' || bytes[i] == b'&')
            && let Some((value_start, value_end)) = match_query_param(line, i + 1)
            && value_end > value_start
        {
            out.push_str(&line[i..value_start]);
            out.push_str(MASK);
            i = value_end;
        } else {
            let ch = line[i..].chars().next().expect("non-empty remainder");
            out.push(ch);
            i += ch.len_utf8();
        }
    }
    out
}

/// Match `token=` / `key=` (case-insensitive) at `at` and return the span
/// of the value, which runs to the next query delimiter or end of line.
fn match_query_param(line: &str, at: usize) -> Option<(usize, usize)> {
    let bytes = line.as_bytes();
    for name in SENSITIVE_QUERY_KEYS {
        if starts_with_ignore_case(line, at, name) {
            let after_name = at + name.len();
            if bytes.get(after_name) == Some(&b'=') {
                let value_start = after_name + 1;
                let value_end = line[value_start..]
                    .char_indices()
                    .find(|(_, ch)| {
                        matches!(ch, '&' | ';' | '#' | ' ' | '\t' | '\n' | '\r' | '"' | '\'')
                    })
                    .map(|(p, _)| value_start + p)
                    .unwrap_or(line.len());
                return Some((value_start, value_end));
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn secrets(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| (*s).to_string()).collect()
    }

    // --- rule a: explicit secrets ---

    #[test]
    fn explicit_controller_secret_is_masked() {
        let line = "controller handshake using secret hunter2 at 127.0.0.1:9090";
        let out = redact_line(line, &secrets(&["hunter2"]));
        assert_eq!(
            out,
            "controller handshake using secret *** at 127.0.0.1:9090"
        );
    }

    #[test]
    fn multiple_secrets_on_one_line_are_all_masked() {
        let line = "aaaa1111 then bbbb2222 then aaaa1111";
        let out = redact_line(line, &secrets(&["aaaa1111", "bbbb2222"]));
        assert_eq!(out, "*** then *** then ***");
    }

    #[test]
    fn shorter_secret_that_prefixes_longer_one_cannot_leak_partial() {
        // Longest-first replacement: `aaaa` must not turn `aaaa1111` into
        // the still-leaky `***1111`.
        let out = redact_line("token aaaa1111", &secrets(&["aaaa", "aaaa1111"]));
        assert_eq!(out, "token ***");
    }

    #[test]
    fn empty_and_short_secrets_are_skipped() {
        let line = "keep abc and ab intact";
        let out = redact_line(line, &secrets(&["", "abc", "ab"]));
        assert_eq!(out, line);
    }

    // --- rule b: key/value pairs ---

    #[test]
    fn yaml_secret_pair_is_masked() {
        let out = redact_line("secret: abc123", &[]);
        assert_eq!(out, "secret: ***");
    }

    #[test]
    fn json_password_pair_is_masked() {
        let out = redact_line(r#"{"password": "p@ss", "user": "me"}"#, &[]);
        assert_eq!(out, r#"{"password": "***", "user": "me"}"#);
    }

    #[test]
    fn quoted_yaml_value_keeps_its_quotes() {
        let out = redact_line("password: 'p@ss'", &[]);
        assert_eq!(out, "password: '***'");
    }

    #[test]
    fn bearer_authorization_header_keeps_scheme_word() {
        let out = redact_line("Authorization: Bearer abc123", &[]);
        assert_eq!(out, "Authorization: Bearer ***");
        let lower = redact_line("authorization: Bearer abc123", &[]);
        assert_eq!(lower, "authorization: Bearer ***");
    }

    #[test]
    fn key_matching_is_case_insensitive_and_supports_equals() {
        assert_eq!(redact_line("PASSWORD=hunter2", &[]), "PASSWORD=***");
        assert_eq!(redact_line("Token=abc123", &[]), "Token=***");
    }

    #[test]
    fn non_sensitive_words_containing_key_names_are_untouched() {
        let line = "token bucket refilled; mysecret: 1, secrets=3 tokens=5";
        assert_eq!(redact_line(line, &[]), line);
    }

    #[test]
    fn scheme_colon_never_matches_as_key_value() {
        assert_eq!(redact_line("token://abc", &[]), "token://abc");
        // But the userinfo password in the same shape is still masked.
        let out = redact_line("https://token:abc123@host/x", &[]);
        assert_eq!(out, "https://token:***@host/x");
    }

    // --- rule c: URL userinfo ---

    #[test]
    fn url_userinfo_password_is_masked() {
        let out = redact_line("dialing socks5://admin:pass123@10.0.0.1:1080", &[]);
        assert_eq!(out, "dialing socks5://admin:***@10.0.0.1:1080");
    }

    #[test]
    fn userinfo_with_empty_username_is_masked() {
        let out = redact_line("https://:pass123@host/x", &[]);
        assert_eq!(out, "https://:***@host/x");
    }

    #[test]
    fn userinfo_without_password_is_untouched() {
        let line = "GET https://user@host/x and https://plain.example.com/y?q=1";
        assert_eq!(redact_line(line, &[]), line);
    }

    // --- rule d: URL query tokens ---

    #[test]
    fn query_token_and_key_values_are_masked() {
        let out = redact_line(
            "GET https://sub.example.com/d?token=tok1234&key=ky99&x=1",
            &[],
        );
        assert_eq!(out, "GET https://sub.example.com/d?token=***&key=***&x=1");
    }

    #[test]
    fn query_param_names_are_case_insensitive() {
        let out = redact_line("https://e.com/s?Token=abc123", &[]);
        assert_eq!(out, "https://e.com/s?Token=***");
        let out = redact_line("https://e.com/s?x=1&KEY=ky99", &[]);
        assert_eq!(out, "https://e.com/s?x=1&KEY=***");
    }

    // --- rule e: preservation and idempotency ---

    #[test]
    fn plain_lines_pass_through_unchanged() {
        let line = "core started, 12 proxies, rule provider reload done";
        assert_eq!(redact_line(line, &secrets(&["zzzz9999"])), line);
    }

    #[test]
    fn already_masked_output_is_stable() {
        let first = redact_line("secret: abc123, u=https://a:p@h/x?token=t", &[]);
        assert_eq!(first, "secret: ***, u=https://a:***@h/x?token=***");
        assert_eq!(redact_line(&first, &[]), first);
    }

    #[test]
    fn composite_subscription_line_is_fully_masked_and_idempotent() {
        let line = "update from url=https://sub.example.com/d?token=tok1234 \
                    with Authorization: Bearer abc123 (secret=s3cret)";
        let expected = "update from url=https://sub.example.com/d?token=*** \
                        with Authorization: Bearer *** (secret=***)";
        let out = redact_line(line, &secrets(&["s3cret", "tok1234", "abc123"]));
        assert_eq!(out, expected);
        assert_eq!(redact_line(&out, &secrets(&["s3cret"])), out);
    }

    // --- mask_secret ---

    #[test]
    fn mask_secret_replaces_every_occurrence() {
        let out = mask_secret("a=xxxx1111 b=xxxx1111 c=ok", "xxxx1111");
        assert_eq!(out, "a=*** b=*** c=ok");
    }

    #[test]
    fn mask_secret_ignores_short_secrets() {
        let line = "ab abc";
        assert_eq!(mask_secret(line, "ab"), line);
        assert_eq!(mask_secret(line, ""), line);
    }
}
