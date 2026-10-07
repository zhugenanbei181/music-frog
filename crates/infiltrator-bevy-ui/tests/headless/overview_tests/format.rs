//! Behavior cases for format.
//! test-intent: behavior

use super::*;

/// The rate formatter: the iced reference ladder (B / KB / MB / GB labels
/// over 1024-based divisors, two decimals from KB up — see
/// `crates/infiltrator-iced/src/utils.rs:3-17`), `/s`-suffixed, with an
/// an explicit unknown for non-finite or negative input.
#[test]
fn format_rate_spans_the_unit_ladder() {
    assert_eq!(format_rate(0.0), "0 B/s");
    assert_eq!(format_rate(-3.0), "Not observed");
    assert_eq!(format_rate(f64::NAN), "Not observed");
    assert_eq!(format_rate(512.4), "512 B/s");
    assert_eq!(format_rate(1023.9), "1023 B/s");
    assert_eq!(format_rate(1024.0), "1.00 KB/s");
    assert_eq!(format_rate(250_000.0), "244.14 KB/s");
    assert_eq!(format_rate(999.0 * 1024.0), "999.00 KB/s");
    assert_eq!(format_rate(1024.0 * 1024.0), "1.00 MB/s");
    assert_eq!(format_rate(3.95 * 1024.0 * 1024.0), "3.95 MB/s");
    assert_eq!(format_rate(2.5 * 1024.0 * 1024.0), "2.50 MB/s");
    assert_eq!(format_rate(1.3 * 1024.0 * 1024.0 * 1024.0), "1.30 GB/s");
    assert_eq!(
        format_rate(3.0 * 1024.0 * 1024.0 * 1024.0),
        "3.00 GB/s",
        "GB is the ladder's top tier, exactly as the reference formatter"
    );
}

/// The memory formatter: the shared reference ladder with two decimals,
/// and an honest em-dash for an absent reading.
#[test]
fn format_memory_spans_the_unit_ladder() {
    assert_eq!(format_memory(None), "—");
    assert_eq!(format_memory(Some(0)), "0 B");
    assert_eq!(format_memory(Some(1023)), "1023 B");
    assert_eq!(format_memory(Some(1024)), "1.00 KB");
    assert_eq!(format_memory(Some(96 * 1024 * 1024)), "96.00 MB");
    assert_eq!(format_memory(Some(3 * 1024 * 1024 * 1024)), "3.00 GB");
}
