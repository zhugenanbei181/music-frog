//! `terminal_guard` 的行为用例：显示宽度的 CJK/零宽事实、分区居中的
//! 降级策略、配置覆盖的解析与容错。

use super::*;

#[test]
fn display_width_counts_fullwidth_as_two_columns() {
    // 半角 1 列、全角（CJK 与全角标点）2 列、零宽（组合符）0 列。
    assert_eq!(display_width("abc"), 3);
    assert_eq!(display_width("连接数"), 6);
    assert_eq!(display_width("a，b"), 4);
    assert_eq!(display_width("e\u{301}cole"), 5);
}

#[test]
fn centering_degrades_to_left_when_the_row_is_tight() {
    let (centered, left_pad) = center_or_left("标题", 12);
    assert_eq!(left_pad, 4);
    assert_eq!(centered, "    标题");
    // 宽度不足时不居中、不截断，原样左对齐。
    let (kept, pad) = center_or_left("很长很长很长很长", 10);
    assert_eq!(pad, 0);
    assert_eq!(kept, "很长很长很长很长");
}

#[test]
fn overrides_parse_all_three_keys() {
    let overrides =
        TerminalOverrides::parse("unicode = \"simple\"\nanimation = false\ncolor = \"ansi256\"\n");
    assert_eq!(overrides.unicode.as_deref(), Some("simple"));
    assert_eq!(overrides.animation, Some(false));
    assert_eq!(overrides.color.as_deref(), Some("ansi256"));
}

#[test]
fn overrides_ignore_unknown_keys_and_broken_files() {
    let partial = TerminalOverrides::parse("unicode = \"full\"\nmystery = 7\n");
    assert_eq!(partial.unicode.as_deref(), Some("full"));
    assert_eq!(partial.animation, None);

    // 非法类型按缺失处理；整份坏文件按空覆盖处理——配置是覆盖，不是依赖。
    let wrong_type = TerminalOverrides::parse("animation = \"yes\"\n");
    assert_eq!(wrong_type.animation, None);
    let broken = TerminalOverrides::parse("not [ valid toml");
    assert_eq!(broken, TerminalOverrides::default());
}

#[test]
fn the_restore_sequence_resets_attributes_cursor_and_wrap() {
    // 属性复位 + 光标显示 + 自动折行，三条各就各位。
    assert_eq!(RESTORE_SEQUENCE, "\x1b[0m\x1b[?25h\x1b[?7h");
}
