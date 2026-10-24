//! `charset` 的行为用例：协商优先级与两套线框字形的渲染差异。

use super::*;

fn environment(term: Option<&str>, unicode_override: Option<&str>) -> TerminalEnvironment {
    TerminalEnvironment {
        term: term.map(str::to_string),
        unicode_override: unicode_override.map(str::to_string),
        ..TerminalEnvironment::default()
    }
}

#[test]
fn modern_terminals_negotiate_unicode() {
    let capabilities = negotiate_charset(&environment(Some("xterm-256color"), None));
    assert_eq!(capabilities, CharsetProfile::Unicode);
    assert_eq!(capabilities.label(), "unicode");
}

#[test]
fn limited_terms_fall_back_to_ascii() {
    for term in ["dumb", "linux", "cons25-iso", "vt100"] {
        let profile = negotiate_charset(&environment(Some(term), None));
        assert_eq!(profile, CharsetProfile::AsciiSimple, "TERM={term}");
    }
    assert_eq!(CharsetProfile::AsciiSimple.label(), "ascii-simple");
}

#[test]
fn the_explicit_override_wins_both_ways() {
    assert_eq!(
        negotiate_charset(&environment(Some("xterm-256color"), Some("simple"))),
        CharsetProfile::AsciiSimple
    );
    assert_eq!(
        negotiate_charset(&environment(Some("dumb"), Some("full"))),
        CharsetProfile::Unicode
    );
    // 无法识别的覆盖取值不生效，按终端信号回落。
    assert_eq!(
        negotiate_charset(&environment(Some("xterm-256color"), Some("wat"))),
        CharsetProfile::Unicode
    );
}

#[test]
fn the_two_frame_sets_keep_the_same_structure() {
    let unicode = CharsetProfile::Unicode.frame();
    let ascii = CharsetProfile::AsciiSimple.frame();
    // Unicode 档案用圆角与制表符，ASCII 档案全部落在可打印 ASCII 里。
    assert_eq!(unicode.top_left, "╭");
    assert_eq!(unicode.cross, "┼");
    assert_eq!(ascii.top_left, "+");
    assert_eq!(ascii.horizontal, "-");
    for glyph in [
        ascii.top_left,
        ascii.top_right,
        ascii.bottom_left,
        ascii.bottom_right,
        ascii.horizontal,
        ascii.vertical,
        ascii.top_tee,
        ascii.bottom_tee,
        ascii.cross,
    ] {
        assert!(
            glyph.is_ascii(),
            "ASCII 档案里混入了非 ASCII 字形 {glyph:?}"
        );
    }
}
