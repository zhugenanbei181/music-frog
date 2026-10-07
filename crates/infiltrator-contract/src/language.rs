//! Supported persisted language choices, independent of UI and system locale detection.
use crate::error::{ErrorCode, Failure};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum LanguagePreference {
    #[serde(rename = "system")]
    System,
    #[default]
    #[serde(rename = "zh-CN")]
    SimplifiedChinese,
    #[serde(rename = "en-US")]
    English,
}
impl LanguagePreference {
    pub const ALL: [Self; 3] = [Self::System, Self::SimplifiedChinese, Self::English];
    pub const fn as_setting(self) -> &'static str {
        match self {
            Self::System => "system",
            Self::SimplifiedChinese => "zh-CN",
            Self::English => "en-US",
        }
    }
    pub const fn label_key(self) -> &'static str {
        match self {
            Self::System => "language_system",
            Self::SimplifiedChinese => "language_simplified_chinese",
            Self::English => "language_english",
        }
    }
    pub fn parse(value: &str) -> Result<Self, Failure> {
        match value.trim().to_ascii_lowercase().replace('_', "-").as_str() {
            "system" => Ok(Self::System),
            "zh" | "zh-cn" => Ok(Self::SimplifiedChinese),
            "en" | "en-us" => Ok(Self::English),
            _ => Err(Failure::new(
                ErrorCode::InvalidInput,
                "unsupported language preference",
                false,
            )),
        }
    }
}
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LanguageSettingsSnapshot {
    pub preference: Option<LanguagePreference>,
    pub can_persist: bool,
    pub failure: Option<Failure>,
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn aliases_canonicalize_but_unknown_preferences_never_turn_into_chinese() {
        for (raw, expected) in [
            ("en", LanguagePreference::English),
            ("EN_us", LanguagePreference::English),
            (" zh-CN ", LanguagePreference::SimplifiedChinese),
            ("SYSTEM", LanguagePreference::System),
        ] {
            assert_eq!(LanguagePreference::parse(raw).unwrap(), expected);
            let wire = serde_json::to_string(&expected).unwrap();
            assert_eq!(
                serde_json::from_str::<LanguagePreference>(&wire).unwrap(),
                expected
            );
        }
        for invalid in ["", "de-DE", "englsh", "auto", "zh-TW"] {
            assert_eq!(
                LanguagePreference::parse(invalid).unwrap_err().code,
                ErrorCode::InvalidInput
            );
        }
    }
}
