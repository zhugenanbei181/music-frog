use serde::Serialize;
use std::io::{self, Write};

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

/// Print an aligned text table. Column widths are measured in characters, so
/// CJK cells keep their separators aligned too.
pub fn print_table(headers: &[&str], rows: &[Vec<String>]) {
    if rows.is_empty() {
        return;
    }

    let column_count = headers.len();
    let mut widths: Vec<usize> = headers.iter().map(|header| width(header)).collect();
    for row in rows {
        for (index, cell) in row.iter().enumerate().take(column_count) {
            widths[index] = widths[index].max(width(cell));
        }
    }

    let stdout = io::stdout();
    let mut out = stdout.lock();
    let header_cells: Vec<String> = headers.iter().map(|header| header.to_string()).collect();
    let _ = writeln!(out);
    write_row(&mut out, &header_cells, &widths);
    write_separator(&mut out, &widths);
    for row in rows {
        write_row(&mut out, row, &widths);
    }
    let _ = writeln!(out);
}

fn write_row(out: &mut dyn Write, cells: &[String], widths: &[usize]) {
    let _ = write!(out, "│");
    for (index, cell) in cells.iter().enumerate().take(widths.len()) {
        let _ = write!(out, " {}", pad(cell, widths[index]));
        let _ = write!(out, " │");
    }
    let _ = writeln!(out);
}

fn write_separator(out: &mut dyn Write, widths: &[usize]) {
    let _ = write!(out, "├");
    for (index, width) in widths.iter().enumerate() {
        let _ = write!(out, "{}", "─".repeat(width + 2));
        if index + 1 < widths.len() {
            let _ = write!(out, "┼");
        }
    }
    let _ = writeln!(out, "┤");
}

fn width(text: &str) -> usize {
    text.chars().count()
}

fn pad(text: &str, target: usize) -> String {
    let current = width(text);
    if target > current {
        format!("{text}{}", " ".repeat(target - current))
    } else {
        text.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::{pad, print_json, print_table, width};

    #[test]
    fn width_counts_characters_not_bytes() {
        assert_eq!(width("abc"), 3);
        assert_eq!(width("测试"), 2);
        assert_eq!(width("a测b"), 3);
    }

    #[test]
    fn pad_pads_right_side_only() {
        assert_eq!(pad("a", 3), "a  ");
        assert_eq!(pad("abc", 3), "abc");
        assert_eq!(pad("测试", 4), "测试  ");
    }

    #[test]
    fn print_table_skips_empty_rows_and_never_panics() {
        print_table(&["name", "value"], &[]);
        print_table(
            &["name", "value"],
            &[vec!["alpha".to_string(), "测试节点".to_string()]],
        );
    }

    #[test]
    fn print_json_pretty_prints() {
        print_json(&serde_json::json!({ "ok": true })).unwrap();
    }
}
