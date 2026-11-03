//! Shared contract for Profile QR sharing, link export, and phone-to-TV import (FLPARITY-002).

use serde::{Deserialize, Serialize};

/// Format in which a profile can be shared across devices.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProfileShareFormat {
    QrCode,
    ShareLink,
    PairingToken,
}

/// Ephemeral pairing token for phone-to-TV profile transfer.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PairingToken {
    pub token: String,
    pub created_at_epoch_sec: u64,
    pub ttl_seconds: u64,
}

impl PairingToken {
    pub const DEFAULT_TTL_SECONDS: u64 = 300;

    pub fn new(token: impl Into<String>, created_at_epoch_sec: u64) -> Self {
        Self {
            token: token.into(),
            created_at_epoch_sec,
            ttl_seconds: Self::DEFAULT_TTL_SECONDS,
        }
    }

    pub fn is_expired(&self, now_epoch_sec: u64) -> bool {
        now_epoch_sec.saturating_sub(self.created_at_epoch_sec) > self.ttl_seconds
    }

    pub fn remaining_seconds(&self, now_epoch_sec: u64) -> u64 {
        let elapsed = now_epoch_sec.saturating_sub(self.created_at_epoch_sec);
        self.ttl_seconds.saturating_sub(elapsed)
    }
}

/// Desensitizes a raw profile subscription URL by redacting secret parameters.
pub fn sanitize_profile_url(raw_url: &str) -> String {
    if let Some((base, query)) = raw_url.split_once('?') {
        let sanitized_pairs: Vec<String> = query
            .split('&')
            .map(|pair| {
                if let Some((k, _v)) = pair.split_once('=') {
                    let lower_key = k.to_ascii_lowercase();
                    if matches!(
                        lower_key.as_str(),
                        "token" | "password" | "secret" | "key" | "auth" | "pass"
                    ) {
                        format!("{k}=REDACTED")
                    } else {
                        pair.to_owned()
                    }
                } else {
                    pair.to_owned()
                }
            })
            .collect();
        format!("{base}?{}", sanitized_pairs.join("&"))
    } else {
        raw_url.to_owned()
    }
}

/// Percent-encodes a string according to RFC 3986.
pub fn percent_encode(input: &str) -> String {
    let mut encoded = String::with_capacity(input.len());
    for b in input.bytes() {
        match b {
            b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(b as char);
            }
            _ => {
                use std::fmt::Write;
                let _ = write!(encoded, "%{:02X}", b);
            }
        }
    }
    encoded
}

/// Desensitized share payload suitable for QR display and remote pairing.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProfileSharePayload {
    pub profile_id: String,
    pub display_name: String,
    pub sanitized_url: String,
    pub share_format: ProfileShareFormat,
    pub pairing_token: Option<PairingToken>,
}

impl ProfileSharePayload {
    pub fn of(
        profile_id: impl Into<String>,
        display_name: impl Into<String>,
        raw_url: &str,
        format: ProfileShareFormat,
        pairing_token: Option<PairingToken>,
    ) -> Self {
        Self {
            profile_id: profile_id.into(),
            display_name: display_name.into(),
            sanitized_url: sanitize_profile_url(raw_url),
            share_format: format,
            pairing_token,
        }
    }

    pub fn to_clash_import_uri(&self) -> String {
        format!(
            "clash://install-config?url={}",
            percent_encode(&self.sanitized_url)
        )
    }
}

/// Explicit multi-stage state machine for TV-side import transfer.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TvImportStage {
    AwaitingCode {
        code: String,
        remaining_seconds: u64,
    },
    Connecting {
        code: String,
    },
    Received {
        payload: ProfileSharePayload,
    },
    Expired,
    Failed {
        reason: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn url_sanitizer_redacts_sensitive_query_tokens() {
        let raw = "https://example.com/sub?token=supersecret123&flag=hk&auth=abc";
        let sanitized = sanitize_profile_url(raw);
        assert!(!sanitized.contains("supersecret123"));
        assert!(!sanitized.contains("=abc"));
        assert!(sanitized.contains("token=REDACTED"));
        assert!(sanitized.contains("auth=REDACTED"));
        assert!(sanitized.contains("flag=hk"));
    }

    #[test]
    fn url_sanitizer_preserves_clean_urls() {
        let clean = "https://example.com/sub/profile.yaml";
        assert_eq!(sanitize_profile_url(clean), clean);
    }

    #[test]
    fn pairing_token_expiration_checks_ttl() {
        let token = PairingToken::new("123456", 1000);
        assert!(!token.is_expired(1100));
        assert_eq!(token.remaining_seconds(1100), 200);
        assert!(token.is_expired(1301));
        assert_eq!(token.remaining_seconds(1301), 0);
    }

    #[test]
    fn clash_import_uri_generation() {
        let payload = ProfileSharePayload::of(
            "p1",
            "MyProfile",
            "https://example.com/sub?name=test",
            ProfileShareFormat::QrCode,
            None,
        );
        let uri = payload.to_clash_import_uri();
        assert!(uri.starts_with("clash://install-config?url=https%3A%2F%2Fexample.com"));
    }

    #[test]
    fn tv_import_stage_serde_round_trip() {
        let stage = TvImportStage::AwaitingCode {
            code: "987654".to_owned(),
            remaining_seconds: 280,
        };
        let json = serde_json::to_string(&stage).expect("serialize stage");
        let decoded: TvImportStage = serde_json::from_str(&json).expect("deserialize stage");
        assert_eq!(stage, decoded);
    }
}
