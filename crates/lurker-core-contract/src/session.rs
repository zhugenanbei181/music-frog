//! Opaque identity for one managed mihomo core session.
//!
//! A generation is useful for ordering, but it is not sufficient to fence a
//! delayed event from a previous application instance. `SessionToken` gives
//! every started session an independent identity while remaining a small,
//! serializable value at every inbound surface.

use serde::{Deserialize, Serialize};

/// Identity of one core process/application session.
///
/// The value is opaque: only the crate that minted a token may compare it
/// against a token it owns. [`SessionToken::ZERO`] is the explicit "no
/// session" marker and never identifies a live session.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SessionToken(u128);

impl SessionToken {
    /// The reserved "no session" token.
    ///
    /// A request carrying it is never fenced against a live session, so
    /// callers that require a session fence must reject it explicitly.
    pub const ZERO: Self = Self(0);

    /// Mints a session token from a caller-supplied value.
    ///
    /// The value is stored verbatim; callers are responsible for choosing a
    /// value that is unique among the sessions of one owner.
    pub const fn new(value: u128) -> Self {
        Self(value)
    }

    /// Returns the raw token value.
    pub const fn value(self) -> u128 {
        self.0
    }

    /// Whether the token identifies a live session.
    ///
    /// Returns `false` only for [`SessionToken::ZERO`].
    pub const fn is_valid(self) -> bool {
        self.0 != 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_is_not_a_session() {
        assert!(!SessionToken::ZERO.is_valid());
        assert!(SessionToken::new(42).is_valid());
    }

    #[test]
    fn token_round_trips_through_serde() {
        let token = SessionToken::new(0x1234);
        let encoded = serde_json::to_string(&token).expect("serialize token");
        let decoded: SessionToken = serde_json::from_str(&encoded).expect("deserialize token");
        assert_eq!(decoded, token);
    }
}
