//! Behavior cases for payload.
//! test-intent: behavior

use super::*;

#[test]
fn payload_pair_codec_round_trips_and_rejects_malformed() {
    // The group/node pair survives names containing colons and spaces.
    let encoded = encode_pair_payload("GLOBAL", "HK node:01");
    assert_eq!(encoded, "GLOBAL\u{1}HK node:01");
    assert_eq!(
        decode_pair_payload(&encoded),
        Some(("GLOBAL", "HK node:01"))
    );
    assert_eq!(decode_pair_payload("no-separator"), None);
}
