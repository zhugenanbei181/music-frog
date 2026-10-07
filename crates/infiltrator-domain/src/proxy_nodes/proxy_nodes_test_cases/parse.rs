//! Behavior cases for parse.
//! test-intent: behavior

use super::*;

#[test]
fn test_parse_rejects_malformed_profiles() {
    // Top-level sequence is not a profile document.
    let err = parse_profile_yaml("- a\n- b\n").expect_err("must reject");
    assert!(err.to_string().contains("mapping"), "{err}");

    // `proxies` that is not a list.
    let err = parse_profile_yaml("proxies: oops\n").expect_err("must reject");
    assert!(err.to_string().contains("proxies"), "{err}");

    // An entry without a string `type` key.
    let err = parse_profile_yaml("proxies:\n  - name: n0\n    server: 1.2.3.4\n    port: 1\n")
        .expect_err("must reject");
    assert!(err.to_string().contains("proxies[0]"), "{err}");

    // A scalar entry.
    let err = parse_profile_yaml("proxies:\n  - just-a-string\n").expect_err("must reject");
    assert!(err.to_string().contains("proxies[0]"), "{err}");
}
