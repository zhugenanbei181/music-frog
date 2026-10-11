use infiltrator_contract::command::CoreLogLevel;

pub(crate) fn encode_mihomo_log_level(level: CoreLogLevel) -> &'static str {
    match level {
        CoreLogLevel::Debug => "debug",
        CoreLogLevel::Info => "info",
        CoreLogLevel::Warn => "warn",
        CoreLogLevel::Error => "error",
    }
}

pub(crate) fn decode_mihomo_log_level(raw: &str) -> Option<CoreLogLevel> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "debug" => Some(CoreLogLevel::Debug),
        "info" => Some(CoreLogLevel::Info),
        "warn" | "warning" => Some(CoreLogLevel::Warn),
        "error" | "err" => Some(CoreLogLevel::Error),
        _ => None,
    }
}
