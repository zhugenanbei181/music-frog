//! Shared formatting for an elapsed duration a surface renders.
//!
//! A duration is a measured fact, so the owner measures it and the surface
//! only formats it. The clock face lives here so that every surface that
//! shows a running session reads one rule instead of writing its own
//! padding, its own hour rollover and its own separator.

/// The number of milliseconds in one second.
const MILLIS_PER_SECOND: u64 = 1_000;
/// The number of seconds in one minute.
const SECONDS_PER_MINUTE: u64 = 60;
/// The number of seconds in one hour.
const SECONDS_PER_HOUR: u64 = 60 * SECONDS_PER_MINUTE;

/// Format an elapsed duration as `HH:MM:SS`.
///
/// The hour field is never truncated, so a session that runs past a day
/// reads `27:04:49` instead of wrapping back to zero. The width of the
/// rendered value only grows when the hour count itself grows, which keeps
/// a ticking value from shifting the row around it.
pub fn format_elapsed_ms(millis: u64) -> String {
    let total_seconds = millis / MILLIS_PER_SECOND;
    let hours = total_seconds / SECONDS_PER_HOUR;
    let minutes = (total_seconds % SECONDS_PER_HOUR) / SECONDS_PER_MINUTE;
    let seconds = total_seconds % SECONDS_PER_MINUTE;
    format!("{hours:02}:{minutes:02}:{seconds:02}")
}

#[cfg(test)]
mod tests {
    use super::format_elapsed_ms;

    #[test]
    fn a_measured_session_reads_as_a_clock_face() {
        assert_eq!(format_elapsed_ms(0), "00:00:00");
        assert_eq!(format_elapsed_ms(1_000), "00:00:01");
        assert_eq!(format_elapsed_ms(61_000), "00:01:01");
        assert_eq!(format_elapsed_ms(3_600_000), "01:00:00");
    }

    #[test]
    fn the_reference_session_length_keeps_its_shape() {
        // The reference dashboard reads `03:04:49` for its running session.
        assert_eq!(format_elapsed_ms(11_089_000), "03:04:49");
    }

    #[test]
    fn sub_second_remainders_are_not_invented() {
        assert_eq!(format_elapsed_ms(999), "00:00:00");
    }

    #[test]
    fn the_hour_field_never_wraps() {
        assert_eq!(
            format_elapsed_ms(27 * 3_600_000 + 4 * 60_000 + 49_000),
            "27:04:49"
        );
    }
}
