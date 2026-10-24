use crate::char_fx::ProgressAnimation;
use crate::charset::{CharsetProfile, FrameGlyphs};
use crate::terminal_guard::{center_or_left, display_width};
use serde::Serialize;
use std::env;
use std::io;
use std::io::Write;
use std::sync::OnceLock;

/// Print an informational line to stdout.
pub fn print_info(message: &str) {
    println!("ℹ {message}");
}

/// Print a success line to stdout.
pub fn print_success(message: &str) {
    println!("✓ {message}");
}

/// Print an error line to stderr.
pub fn print_error(message: &str) {
    eprintln!("✗ {message}");
}

/// Render the report of a `--json` capable command as pretty JSON.
pub fn print_json<T: Serialize>(value: &T) -> anyhow::Result<()> {
    let rendered = serde_json::to_string_pretty(value)?;
    println!("{rendered}");
    Ok(())
}

/// 进程级输出档案：CLI 入口按协商结果配置一次，之后所有表格渲染共用。
static TABLE_CHARSET: OnceLock<CharsetProfile> = OnceLock::new();

/// 配置表格输出字符集（EFFECTS-043 #174~#175）。只应调用一次；重复调用
/// 保持首次结果——协商快照是启动期事实，不随运行期漂移。
pub(crate) fn configure_table_charset(profile: CharsetProfile) {
    let _ = TABLE_CHARSET.set(profile);
}

/// Print a centered section header (EFFECTS-045 #182): the title centers when
/// the row is wide enough, falls back to left alignment when tight, and is
/// never truncated. The width comes from `COLUMNS` when the terminal exports
/// it, 80 otherwise.
pub fn print_section(title: &str) {
    let available = env::var("COLUMNS")
        .ok()
        .and_then(|value| value.trim().parse::<usize>().ok())
        .unwrap_or(80);
    let (line, _) = center_or_left(title, available);
    println!("{line}");
}

/// Build one in-place progress line (EFFECTS-044 #176~#177): the caller prints
/// the returned string with a carriage return before it and re-polls; once the
/// animation reports no frame the caller stops ticking and clears the line.
pub(crate) fn progress_frame_line(
    label: &str,
    elapsed_ms: u32,
) -> Option<String> {
    let frame = ProgressAnimation::for_progress()
        .frame_at(elapsed_ms)?;
    Some(format!("{frame} {label}"))
}

/// 当前表格字符集；未配置时按 Unicode（现代终端默认）。
fn table_charset() -> CharsetProfile {
    TABLE_CHARSET.get().copied().unwrap_or_default()
}

/// Print an aligned text table inside a frame (EFFECTS-043 #174): modern
/// terminals get rounded Unicode corners, limited terminals fall back to the
/// pure-ASCII frame negotiated at startup. Column widths are measured in
/// display columns (fullwidth 2 / halfwidth 1 / zero-width 0), so CJK cells
/// keep their separators aligned too (EFFECTS-045 #183).
pub fn print_table(headers: &[&str], rows: &[Vec<String>]) {
    if rows.is_empty() {
        return;
    }

    let glyphs = table_charset().frame();
    let column_count = headers.len();
    let mut widths: Vec<usize> = headers.iter().map(|header| display_width(header)).collect();
    for row in rows {
        for (index, cell) in row.iter().enumerate().take(column_count) {
            widths[index] = widths[index].max(display_width(cell));
        }
    }

    let stdout = io::stdout();
    let mut out = stdout.lock();
    let header_cells: Vec<String> = headers.iter().map(|header| header.to_string()).collect();
    let _ = writeln!(out);
    write_frame_edge(&mut out, &widths, &glyphs, glyphs.top_left, glyphs.top_tee, glyphs.top_right);
    write_row(&mut out, &header_cells, &widths, &glyphs);
    write_frame_edge(
        &mut out,
        &widths,
        &glyphs,
        glyphs.vertical,
        glyphs.cross,
        glyphs.vertical,
    );
    for row in rows {
        write_row(&mut out, row, &widths, &glyphs);
    }
    write_frame_edge(
        &mut out,
        &widths,
        &glyphs,
        glyphs.bottom_left,
        glyphs.bottom_tee,
        glyphs.bottom_right,
    );
    let _ = writeln!(out);
}

/// 一条水平框线：`左端 ─…┬ … 右端`。
fn write_frame_edge(
    out: &mut dyn Write,
    widths: &[usize],
    glyphs: &FrameGlyphs,
    left: &str,
    middle: &str,
    right: &str,
) {
    let _ = write!(out, "{left}");
    for (index, width) in widths.iter().enumerate() {
        let _ = write!(out, "{}", glyphs.horizontal.repeat(width + 2));
        let joiner = if index + 1 < widths.len() { middle } else { right };
        let _ = write!(out, "{joiner}");
    }
    let _ = writeln!(out);
}

fn write_row(out: &mut dyn Write, cells: &[String], widths: &[usize], glyphs: &FrameGlyphs) {
    let _ = write!(out, "{}", glyphs.vertical);
    for (index, cell) in cells.iter().enumerate().take(widths.len()) {
        let _ = write!(out, " {}", pad(cell, widths[index]));
        let _ = write!(out, " {}", glyphs.vertical);
    }
    let _ = writeln!(out);
}

fn pad(text: &str, target: usize) -> String {
    let current = display_width(text);
    if target > current {
        format!("{text}{}", " ".repeat(target - current))
    } else {
        text.to_string()
    }
}
