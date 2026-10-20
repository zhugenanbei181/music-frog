//! test-intent: behavior

use super::{DelayAnswer, MAX_SINGBOX_DELAY_TIMEOUT_MS, classify, refusal_code};
use lurker_core_contract::error::ErrorCode;

#[test]
fn a_measurement_is_classified_as_the_engines_own_delay() {
    assert_eq!(
        classify(200, br#"{"delay": 42}"#),
        DelayAnswer::Measured { delay_ms: 42 }
    );
    assert_eq!(
        classify(200, br#"{"delay":0}"#),
        DelayAnswer::Measured { delay_ms: 0 }
    );
}

#[test]
fn the_engines_own_timeout_answer_is_classified_as_a_timeout() {
    assert_eq!(
        classify(504, br#"{"message":"Timeout"}"#),
        DelayAnswer::TimedOut
    );
}

#[test]
fn every_refusal_of_the_engine_keeps_its_status_and_reason() {
    assert_eq!(
        classify(400, br#"{"message":"Body invalid"}"#),
        DelayAnswer::Refused {
            status: 400,
            reason: Some("Body invalid".to_owned())
        }
    );
    assert_eq!(
        classify(401, br#"{"message":"Unauthorized"}"#),
        DelayAnswer::Refused {
            status: 401,
            reason: Some("Unauthorized".to_owned())
        }
    );
    assert_eq!(
        classify(404, br#"{"message":"Resource not found"}"#),
        DelayAnswer::Refused {
            status: 404,
            reason: Some("Resource not found".to_owned())
        }
    );
    assert_eq!(
        classify(503, br#"{"message":"An error occurred in the delay test"}"#),
        DelayAnswer::Refused {
            status: 503,
            reason: Some("An error occurred in the delay test".to_owned())
        }
    );
    assert_eq!(
        classify(500, br#"{"message":"internal error"}"#),
        DelayAnswer::Refused {
            status: 500,
            reason: Some("internal error".to_owned())
        }
    );
}

#[test]
fn a_delay_route_that_does_not_answer_is_never_read_as_a_refusal() {
    assert_eq!(
        classify(404, b"404 page not found"),
        DelayAnswer::NotProvided
    );
    assert_eq!(classify(405, b""), DelayAnswer::NotProvided);
}

#[test]
fn a_body_that_is_not_a_measurement_is_never_read_as_one() {
    assert_eq!(
        classify(200, br#"{"delay":"fast"}"#),
        DelayAnswer::Malformed
    );
    assert_eq!(classify(200, b"not json"), DelayAnswer::Malformed);
    assert_eq!(
        classify(200, br#"{"message":"An error occurred in the delay test"}"#),
        DelayAnswer::Malformed
    );
}

#[test]
fn the_refusal_codes_follow_the_engines_own_answers() {
    assert_eq!(refusal_code(400), ErrorCode::InvalidInput);
    assert_eq!(refusal_code(401), ErrorCode::Authentication);
    assert_eq!(refusal_code(403), ErrorCode::Authentication);
    assert_eq!(refusal_code(404), ErrorCode::InvalidState);
    assert_eq!(refusal_code(503), ErrorCode::Network);
    assert_eq!(refusal_code(500), ErrorCode::Network);
}

#[test]
fn the_delay_timeout_bound_is_the_endpoints_signed_sixteen_bit_limit() {
    assert_eq!(MAX_SINGBOX_DELAY_TIMEOUT_MS, 32_767);
}
