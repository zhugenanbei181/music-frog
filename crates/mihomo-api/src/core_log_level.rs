use infiltrator_contract::command::CoreLogLevel;

pub(crate) fn encode_mihomo_log_level(level: CoreLogLevel) -> &'static str {
    match level {
        CoreLogLevel::Silent => "silent",
        CoreLogLevel::Debug => "debug",
        CoreLogLevel::Info => "info",
        CoreLogLevel::Warn => "warn",
        CoreLogLevel::Error => "error",
    }
}

pub(crate) fn decode_mihomo_log_level(raw: &str) -> Option<CoreLogLevel> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "silent" => Some(CoreLogLevel::Silent),
        "debug" => Some(CoreLogLevel::Debug),
        "info" => Some(CoreLogLevel::Info),
        "warn" | "warning" => Some(CoreLogLevel::Warn),
        "error" | "err" => Some(CoreLogLevel::Error),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{decode_mihomo_log_level, encode_mihomo_log_level};
    use infiltrator_contract::command::CoreLogLevel;

    #[test]
    fn the_silent_rung_round_trips_over_the_wire() {
        // The reference product and the admin API already accept `silent`;
        // the typed Core setting must not degrade it to `Unknown`.
        assert_eq!(encode_mihomo_log_level(CoreLogLevel::Silent), "silent");
        assert_eq!(
            decode_mihomo_log_level("SILENT"),
            Some(CoreLogLevel::Silent)
        );
        assert_eq!(
            decode_mihomo_log_level(" silent "),
            Some(CoreLogLevel::Silent)
        );
    }

    #[test]
    fn every_typed_level_has_one_wire_spelling_and_one_readback() {
        for level in CoreLogLevel::ALL {
            let wire = encode_mihomo_log_level(level);
            assert_eq!(decode_mihomo_log_level(wire), Some(level), "{wire}");
        }
    }
}
