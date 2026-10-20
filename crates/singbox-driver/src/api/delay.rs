//! The typed wire semantics of the sing-box Clash API delay endpoint.
//!
//! sing-box answers `GET /proxies/{name}/delay?url={url}&timeout={ms}` with
//! one of its own typed answers. This module owns that classification, so the
//! outbound adapter never reads a status code or a message on its own and
//! never turns an answer the engine did not produce into a measurement.
//!
//! The answers of sing-box 1.13 and 1.14 are:
//!
//! | Status | Body | Meaning |
//! | --- | --- | --- |
//! | `200` | `{"delay": N}` | the engine measured a round trip through the outbound |
//! | `400` | `{"message": "Body invalid"}` | the engine refused the request |
//! | `401` / `403` | `{"message": ...}` | the controller refused the secret |
//! | `404` | `{"message": "Resource not found"}` | the engine does not know the outbound |
//! | `503` | `{"message": "An error occurred in the delay test"}` | the delay test failed |
//! | `504` | `{"message": "Timeout"}` | the engine's own delay test exceeded its deadline |
//!
//! The endpoint parses the timeout as a signed 16-bit millisecond count and
//! measures HTTPS targets only: a plain-HTTP target is replaced by the
//! engine's own URL test target. The driver therefore sends the requested
//! target unchanged and reports the measurement the engine publishes, and it
//! refuses a timeout the endpoint cannot carry before the request is sent.

use lurker_core_contract::error::ErrorCode;
use serde::Deserialize;

/// The largest delay timeout the sing-box delay endpoint accepts, in
/// milliseconds.
///
/// The endpoint parses the timeout as a signed 16-bit millisecond count, so a
/// larger value is refused before the request is sent.
pub const MAX_SINGBOX_DELAY_TIMEOUT_MS: u32 = i16::MAX as u32;

/// The status the engine answers with when its own delay test exceeded the
/// deadline it was given.
const ENGINE_TIMEOUT_STATUS: u16 = 504;

/// The status the engine answers with when the delay test itself failed.
const ENGINE_TEST_FAILED_STATUS: u16 = 503;

/// The status the engine answers with when it does not know the outbound.
const ENGINE_UNKNOWN_OUTBOUND_STATUS: u16 = 404;

/// The status the engine answers with when it refuses the request.
const ENGINE_REFUSED_STATUS: u16 = 400;

/// The status the engine answers with when the controller refuses the secret.
const ENGINE_UNAUTHORIZED_STATUS: u16 = 401;

/// The status the engine answers with when the controller forbids the request.
const ENGINE_FORBIDDEN_STATUS: u16 = 403;

/// The status the engine answers with when its own handler failed.
const ENGINE_INTERNAL_ERROR_STATUS: u16 = 500;

/// The typed answer of the sing-box delay endpoint.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DelayAnswer {
    /// The engine measured a round-trip delay through the outbound.
    Measured {
        /// The measured delay in milliseconds.
        delay_ms: u32,
    },
    /// The engine's own delay test exceeded the deadline it was given.
    TimedOut,
    /// The engine refused the request.
    Refused {
        /// The status the engine answered with.
        status: u16,
        /// The engine's own reason, when it sent one.
        reason: Option<String>,
    },
    /// The engine's answer is not one of the answers its delay endpoint
    /// produces, so the driver claims no measurement, timeout or refusal.
    NotProvided,
    /// The engine answered with a measurement the driver could not read.
    Malformed,
}

/// Classifies the engine's answer to one delay request.
///
/// A `200` answer carries the measurement; every other answer of the engine
/// carries its own typed error body. An answer the engine's delay endpoint
/// does not produce is never read as a measurement, a timeout or a refusal.
pub fn classify(status: u16, body: &[u8]) -> DelayAnswer {
    match status {
        200 => match serde_json::from_slice::<DelayResponse>(body) {
            Ok(response) => DelayAnswer::Measured {
                delay_ms: response.delay,
            },
            Err(_) => DelayAnswer::Malformed,
        },
        ENGINE_TIMEOUT_STATUS => DelayAnswer::TimedOut,
        ENGINE_REFUSED_STATUS
        | ENGINE_UNAUTHORIZED_STATUS
        | ENGINE_FORBIDDEN_STATUS
        | ENGINE_UNKNOWN_OUTBOUND_STATUS
        | ENGINE_INTERNAL_ERROR_STATUS
        | ENGINE_TEST_FAILED_STATUS => {
            let reason = serde_json::from_slice::<EngineError>(body)
                .ok()
                .map(|error| error.message);
            if status == ENGINE_UNKNOWN_OUTBOUND_STATUS && reason.is_none() {
                // The engine's delay route did not answer: the body is not one
                // of the engine's own typed error bodies.
                return DelayAnswer::NotProvided;
            }
            DelayAnswer::Refused { status, reason }
        }
        _ => DelayAnswer::NotProvided,
    }
}

/// The neutral failure code one refused delay answer maps to.
///
/// The mapping follows the engine's own answers: a malformed request is the
/// caller's input, a refused secret is an authentication failure, an unknown
/// outbound is a stale observation and every other refusal is a network
/// failure of the engine's delay test.
pub fn refusal_code(status: u16) -> ErrorCode {
    match status {
        ENGINE_REFUSED_STATUS => ErrorCode::InvalidInput,
        ENGINE_UNAUTHORIZED_STATUS | ENGINE_FORBIDDEN_STATUS => ErrorCode::Authentication,
        ENGINE_UNKNOWN_OUTBOUND_STATUS => ErrorCode::InvalidState,
        _ => ErrorCode::Network,
    }
}

/// The measurement the engine published.
#[derive(Deserialize)]
struct DelayResponse {
    delay: u32,
}

/// The typed error body every refusal of the engine carries.
#[derive(Deserialize)]
struct EngineError {
    message: String,
}

#[cfg(test)]
#[path = "delay_tests.rs"]
mod tests;
