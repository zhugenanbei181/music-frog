//! Single-click routing rule override intent and execution contract (FLPARITY-028).
//!
//! Provides strongly-typed models for creating custom routing rule overrides
//! directly from active connection / request details (such as forcing a domain
//! or IP to DIRECT, PROXY, REJECT, or a user-selected strategy group),
//! with de-duplication, format validation, and hot-reload guarantees.

use serde::{Deserialize, Serialize};
use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::net::{Ipv4Addr, Ipv6Addr};

/// Standard rule types supported for fast rule creation.
pub const SUPPORTED_OVERRIDE_RULE_TYPES: [&str; 6] = [
    "DOMAIN-SUFFIX",
    "DOMAIN",
    "DOMAIN-KEYWORD",
    "IP-CIDR",
    "IP-CIDR6",
    "GEOIP",
];

/// Common default override routing destinations.
pub const DEFAULT_OVERRIDE_TARGETS: [&str; 3] = ["DIRECT", "PROXY", "REJECT"];

/// Error variants encountered during rule override specification validation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuleOverrideValidationError {
    EmptyRuleType,
    UnsupportedRuleType(String),
    EmptyPayload,
    InvalidPayload(String),
    EmptyTarget,
}

impl Display for RuleOverrideValidationError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyRuleType => write!(f, "规则类型不能为空"),
            Self::UnsupportedRuleType(t) => write!(f, "不支持的规则类型: {t}"),
            Self::EmptyPayload => write!(f, "匹配目标不能为空"),
            Self::InvalidPayload(p) => write!(f, "匹配格式不合法: {p}"),
            Self::EmptyTarget => write!(f, "出站策略不能为空"),
        }
    }
}

impl Error for RuleOverrideValidationError {}

/// Intent to create a new rule override from request/connection observations.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreateRuleOverrideIntent {
    /// Rule type, e.g. "DOMAIN-SUFFIX", "DOMAIN", "IP-CIDR".
    pub rule_type: String,
    /// Pattern payload, e.g. "github.com", "1.1.1.1/32".
    pub payload: String,
    /// Destination target, e.g. "DIRECT", "PROXY", "REJECT", or a proxy group name.
    pub target: String,
    /// Whether to prepend at the beginning of the active profile rules (top priority).
    #[serde(default)]
    pub prepend: bool,
}

impl CreateRuleOverrideIntent {
    /// Construct a new rule override intent.
    pub fn new(
        rule_type: impl Into<String>,
        payload: impl Into<String>,
        target: impl Into<String>,
    ) -> Self {
        Self {
            rule_type: rule_type.into().trim().to_ascii_uppercase(),
            payload: payload.into().trim().to_owned(),
            target: target.into().trim().to_owned(),
            prepend: true,
        }
    }

    /// Construct an override intent from a destination host/IP and desired outbound target.
    pub fn from_host(host_or_ip: &str, target: impl Into<String>) -> Option<Self> {
        let trimmed = host_or_ip.trim();
        if trimmed.is_empty() {
            return None;
        }

        // If host contains a port suffix, strip it
        let clean = match trimmed.rsplit_once(':') {
            Some((host, port))
                if !host.is_empty()
                    && !host.contains(':')
                    && !port.is_empty()
                    && port.chars().all(|c| c.is_ascii_digit()) =>
            {
                host
            }
            _ => trimmed,
        };

        if clean.parse::<Ipv4Addr>().is_ok() {
            Some(Self::new("IP-CIDR", format!("{clean}/32"), target))
        } else if clean.parse::<Ipv6Addr>().is_ok() {
            Some(Self::new("IP-CIDR6", format!("{clean}/128"), target))
        } else {
            Some(Self::new("DOMAIN-SUFFIX", clean, target))
        }
    }

    /// Complete canonical rule string representation (e.g. "DOMAIN-SUFFIX,github.com,DIRECT").
    pub fn rule_line(&self) -> String {
        format!("{},{},{}", self.rule_type, self.payload, self.target)
    }

    /// Strict validation of syntax and supported parameters.
    pub fn validate(&self) -> Result<(), RuleOverrideValidationError> {
        if self.rule_type.is_empty() {
            return Err(RuleOverrideValidationError::EmptyRuleType);
        }
        if !SUPPORTED_OVERRIDE_RULE_TYPES
            .iter()
            .any(|t| t.eq_ignore_ascii_case(&self.rule_type))
        {
            return Err(RuleOverrideValidationError::UnsupportedRuleType(
                self.rule_type.clone(),
            ));
        }
        if self.payload.is_empty() {
            return Err(RuleOverrideValidationError::EmptyPayload);
        }
        if self.payload.contains(',') || self.payload.contains(' ') {
            return Err(RuleOverrideValidationError::InvalidPayload(
                self.payload.clone(),
            ));
        }
        if self.target.is_empty() {
            return Err(RuleOverrideValidationError::EmptyTarget);
        }
        Ok(())
    }
}

/// Execution status of a rule override intent.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CreateRuleOverrideStatus {
    /// Rule was successfully appended/prepended and hot-reloaded.
    Applied,
    /// Identical rule line already exists in the profile.
    AlreadyExists,
    /// Invalid rule syntax or payload rejected before apply.
    InvalidInput,
    /// Kernel reload or persistence failed.
    ApplyFailed,
}

/// Typed result after executing a rule override intent.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreateRuleOverrideResult {
    pub status: CreateRuleOverrideStatus,
    pub rule_line: String,
    pub message: Option<String>,
}

impl CreateRuleOverrideResult {
    pub fn applied(rule_line: impl Into<String>) -> Self {
        Self {
            status: CreateRuleOverrideStatus::Applied,
            rule_line: rule_line.into(),
            message: None,
        }
    }

    pub fn already_exists(rule_line: impl Into<String>) -> Self {
        Self {
            status: CreateRuleOverrideStatus::AlreadyExists,
            rule_line: rule_line.into(),
            message: Some("规则已存在，无需重复添加".to_owned()),
        }
    }

    pub fn failed(
        status: CreateRuleOverrideStatus,
        rule_line: impl Into<String>,
        msg: impl Into<String>,
    ) -> Self {
        Self {
            status,
            rule_line: rule_line.into(),
            message: Some(msg.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_host_domain_and_ip_matching() {
        let intent_domain =
            CreateRuleOverrideIntent::from_host("api.github.com:443", "PROXY").unwrap();
        assert_eq!(intent_domain.rule_type, "DOMAIN-SUFFIX");
        assert_eq!(intent_domain.payload, "api.github.com");
        assert_eq!(intent_domain.target, "PROXY");
        assert_eq!(
            intent_domain.rule_line(),
            "DOMAIN-SUFFIX,api.github.com,PROXY"
        );
        assert!(intent_domain.validate().is_ok());

        let intent_ip = CreateRuleOverrideIntent::from_host("1.1.1.1:53", "DIRECT").unwrap();
        assert_eq!(intent_ip.rule_type, "IP-CIDR");
        assert_eq!(intent_ip.payload, "1.1.1.1/32");
        assert_eq!(intent_ip.target, "DIRECT");
        assert_eq!(intent_ip.rule_line(), "IP-CIDR,1.1.1.1/32,DIRECT");
        assert!(intent_ip.validate().is_ok());
    }

    #[test]
    fn validation_rejects_empty_and_illegal_characters() {
        let bad_type = CreateRuleOverrideIntent::new("UNKNOWN_TYPE", "example.com", "DIRECT");
        assert!(bad_type.validate().is_err());

        let comma_payload = CreateRuleOverrideIntent::new("DOMAIN", "a.com,b.com", "DIRECT");
        assert!(comma_payload.validate().is_err());

        let empty_target = CreateRuleOverrideIntent::new("DOMAIN", "example.com", "");
        assert!(empty_target.validate().is_err());
    }

    #[test]
    fn override_result_variants() {
        let applied = CreateRuleOverrideResult::applied("DOMAIN,a.com,DIRECT");
        assert_eq!(applied.status, CreateRuleOverrideStatus::Applied);

        let dup = CreateRuleOverrideResult::already_exists("DOMAIN,a.com,DIRECT");
        assert_eq!(dup.status, CreateRuleOverrideStatus::AlreadyExists);
        assert!(dup.message.is_some());
    }
}
