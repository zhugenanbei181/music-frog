//! `char_fx` 的行为用例：盲文波形的点阵位序、半块回退的转义构造、
//! 转场动效的结束即停。

use super::*;
use crate::terminal_capability::ColorDepth;

#[test]
fn the_braille_waveform_maps_levels_to_dot_bits() {
    // 0.5 的单列采样点高 2 → 低两位点（0x2803）。
    let one_column = braille_waveform(&[0.5]);
    assert_eq!(one_column, "\u{2803}");
    // 满幅两列点满 4 行 → 0x28FF。
    assert_eq!(braille_waveform(&[1.0, 1.0]), "\u{28FF}");
    // 静默输入输出空串。
    assert_eq!(braille_waveform(&[]), "");
    assert_eq!(braille_waveform(&[0.0, 0.0]), "\u{2800}");
}

#[test]
fn the_halfblock_fallback_pairs_rows_foreground_and_background() {
    let white = vec![[255u8, 255, 255]];
    let black = vec![[0u8, 0, 0]];
    let rows = halfblock_rows(&[white.clone(), black.clone()], ColorDepth::TrueColor);
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0], "\x1b[38;2;255;255;255;48;2;0;0;0m▀\x1b[0m");

    // 单行图片：下行按黑色补齐。
    let single = halfblock_rows(&[white.clone()], ColorDepth::TrueColor);
    assert_eq!(single[0], "\x1b[38;2;255;255;255;48;2;0;0;0m▀\x1b[0m");

    // 16 色档：前景白（亮段 97）、背景黑（40）。
    let sixteen = halfblock_rows(&[white.clone(), black.clone()], ColorDepth::Ansi16);
    assert_eq!(sixteen[0], "\x1b[97;40m▀\x1b[0m");

    // 256 色档：调色板索引转义。
    let palette = halfblock_rows(&[white, black], ColorDepth::Ansi256);
    assert!(palette[0].starts_with("\x1b[38;5;"));
}

#[test]
fn the_progress_animation_stops_the_instant_it_ends() {
    let animation = ProgressAnimation::for_progress();
    let first = animation.frame_at(0).expect("第一帧立即可用");
    assert_eq!(first, "⠋");
    // 帧序推进。
    assert_eq!(animation.frame_at(80), Some("⠙"));
    // 结束即停：超过总时长后没有帧，tick 必须立即退出。
    assert_eq!(animation.frame_at(animation.total_ms()), None);
    assert_eq!(animation.frame_at(animation.total_ms() + 1), None);
    // 总时长 = 帧数 × 帧间隔。
    assert_eq!(animation.total_ms(), 10 * 80);
}
