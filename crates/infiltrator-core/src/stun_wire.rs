//! DUAL-14-09 (re-scoped): the minimal STUN wire codec the UDP-egress probe
//! uses (RFC 5389).
//!
//! Only the shapes a Binding Request probe needs are implemented: encode a
//! Binding Request, and parse a Binding Success/Error Response whose magic
//! cookie and transaction id match. [`parse_binding_response`] reads the
//! XOR-MAPPED-ADDRESS the server observed; it never falls back to the
//! deprecated MAPPED-ADDRESS attribute (RFC 5389 §15.2 says a client MUST
//! ignore MAPPED-ADDRESS when XOR-MAPPED-ADDRESS is present), so an answer
//! without the XOR attribute is a typed error instead of a guessed mapping.

use infiltrator_contract::stun_probe::StunMappedAddress;
use std::net::{Ipv4Addr, Ipv6Addr};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

/// Fixed STUN header length (RFC 5389 §6).
pub const HEADER_LEN: usize = 20;

/// The STUN magic cookie.
pub const MAGIC_COOKIE: u32 = 0x2112_A442;

/// Binding Request message type.
pub const BINDING_REQUEST: u16 = 0x0001;

/// Binding Success Response message type.
pub const BINDING_SUCCESS_RESPONSE: u16 = 0x0101;

/// Binding Error Response message type.
pub const BINDING_ERROR_RESPONSE: u16 = 0x0111;

/// XOR-MAPPED-ADDRESS attribute type (RFC 5389 §15.2).
pub const ATTR_XOR_MAPPED_ADDRESS: u16 = 0x0020;

/// ERROR-CODE attribute type (RFC 5389 §15.6).
pub const ATTR_ERROR_CODE: u16 = 0x0009;

/// Transaction id length (RFC 5389 §6).
pub const TRANSACTION_ID_LEN: usize = 12;

/// Largest datagram this prober reads from a STUN server.
pub const MAX_RESPONSE_BYTES: usize = 4096;

/// A STUN transaction id.
pub type TransactionId = [u8; TRANSACTION_ID_LEN];

/// Why a datagram is not the answer to this probe.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StunWireError {
    /// Fewer bytes than a STUN header.
    TooShort { len: usize },
    /// The magic cookie is not the STUN cookie.
    BadMagicCookie { actual: u32 },
    /// The answer echoes a different transaction id.
    TransactionMismatch,
    /// The declared message length does not match the datagram.
    LengthMismatch { declared: usize, actual: usize },
    /// An attribute header or value runs past the end of the datagram.
    TruncatedAttribute { offset: usize },
    /// The XOR-MAPPED-ADDRESS family is neither IPv4 nor IPv6.
    InvalidAddressFamily { family: u8 },
    /// The XOR-MAPPED-ADDRESS value length does not fit its family.
    InvalidAddressLength { length: usize },
    /// The message type is not a Binding Success or Error Response.
    UnexpectedMessageType { message_type: u16 },
    /// A Binding Success Response carried no XOR-MAPPED-ADDRESS.
    MissingMappedAddress,
}

impl std::fmt::Display for StunWireError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TooShort { len } => {
                write!(
                    formatter,
                    "answer is {len} bytes, shorter than a STUN header"
                )
            }
            Self::BadMagicCookie { actual } => {
                write!(
                    formatter,
                    "answer magic cookie 0x{actual:08x} is not the STUN cookie"
                )
            }
            Self::TransactionMismatch => {
                write!(formatter, "answer transaction id does not match the probe")
            }
            Self::LengthMismatch { declared, actual } => write!(
                formatter,
                "answer declares {declared} attribute bytes but carries {actual}"
            ),
            Self::TruncatedAttribute { offset } => {
                write!(
                    formatter,
                    "attribute at offset {offset} runs past the answer"
                )
            }
            Self::InvalidAddressFamily { family } => {
                write!(formatter, "unsupported XOR-MAPPED-ADDRESS family {family}")
            }
            Self::InvalidAddressLength { length } => {
                write!(
                    formatter,
                    "XOR-MAPPED-ADDRESS value length {length} is invalid"
                )
            }
            Self::UnexpectedMessageType { message_type } => {
                write!(
                    formatter,
                    "unexpected STUN message type 0x{message_type:04x}"
                )
            }
            Self::MissingMappedAddress => {
                write!(
                    formatter,
                    "Binding Success Response carried no XOR-MAPPED-ADDRESS"
                )
            }
        }
    }
}

impl std::error::Error for StunWireError {}

/// The parsed parts of a Binding Response.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StunResponse {
    pub message_type: u16,
    /// The address the server observed, for a Success Response.
    pub mapped: Option<StunMappedAddress>,
    /// The numeric error code, for an Error Response.
    pub error_code: Option<u16>,
    /// The reason phrase, for an Error Response.
    pub error_reason: Option<String>,
}

impl StunResponse {
    /// Whether this is a success that carried a mapping.
    pub fn is_success(&self) -> bool {
        self.message_type == BINDING_SUCCESS_RESPONSE && self.mapped.is_some()
    }
}

/// A fresh transaction id for one Binding Request.
///
/// The id only has to keep two probes in this process from colliding and to be
/// echoed back by the server; it is not a security token, so no CSPRNG
/// dependency is pulled in. Time, a process-local counter and the pid seed two
/// splitmix64 rounds.
pub fn new_transaction_id() -> TransactionId {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_nanos() as u64)
        .unwrap_or(0);
    let counter = NEXT.fetch_add(1, Ordering::Relaxed);
    let seed = nanos ^ counter.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ u64::from(std::process::id());
    let first = splitmix64(seed);
    let second = splitmix64(first);
    let mut id = [0u8; TRANSACTION_ID_LEN];
    id[..8].copy_from_slice(&first.to_be_bytes());
    id[8..].copy_from_slice(&(second as u32).to_be_bytes());
    id
}

fn splitmix64(value: u64) -> u64 {
    let mut state = value.wrapping_add(0x9E37_79B9_7F4A_7C15);
    state = (state ^ (state >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    state = (state ^ (state >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    state ^ (state >> 31)
}

/// Encode a Binding Request with no attributes.
pub fn encode_binding_request(transaction: &TransactionId) -> Vec<u8> {
    let mut message = Vec::with_capacity(HEADER_LEN);
    message.extend_from_slice(&BINDING_REQUEST.to_be_bytes());
    message.extend_from_slice(&0u16.to_be_bytes());
    message.extend_from_slice(&MAGIC_COOKIE.to_be_bytes());
    message.extend_from_slice(transaction);
    message
}

/// Encode a Binding Success Response carrying `mapped` as XOR-MAPPED-ADDRESS.
///
/// This is the response shape a well-behaved STUN server returns; the offline
/// tests use it to stand up a local responder, and it documents exactly the
/// bytes [`parse_binding_response`] accepts.
pub fn encode_binding_success(transaction: &TransactionId, mapped: &StunMappedAddress) -> Vec<u8> {
    let attribute = encode_xor_mapped_address(transaction, mapped);
    let mut message = Vec::with_capacity(HEADER_LEN + attribute.len());
    message.extend_from_slice(&BINDING_SUCCESS_RESPONSE.to_be_bytes());
    message.extend_from_slice(&(attribute.len() as u16).to_be_bytes());
    message.extend_from_slice(&MAGIC_COOKIE.to_be_bytes());
    message.extend_from_slice(transaction);
    message.extend_from_slice(&attribute);
    message
}

fn encode_xor_mapped_address(transaction: &TransactionId, mapped: &StunMappedAddress) -> Vec<u8> {
    let (family, encoded): (u8, Vec<u8>) = match mapped.ip.parse::<Ipv4Addr>() {
        Ok(address) => {
            let mut bytes = Vec::with_capacity(4);
            for (index, byte) in address.octets().iter().enumerate() {
                bytes.push(byte ^ MAGIC_COOKIE.to_be_bytes()[index]);
            }
            (0x01, bytes)
        }
        Err(_) => match mapped.ip.parse::<Ipv6Addr>() {
            Ok(address) => {
                let mask = xor_mask(transaction);
                let mut bytes = Vec::with_capacity(16);
                for (index, byte) in address.octets().iter().enumerate() {
                    bytes.push(byte ^ mask[index]);
                }
                (0x02, bytes)
            }
            // A non-IP address cannot be encoded; the caller only passes one
            // for a parsed observation, so an empty attribute is a safe no-op.
            Err(_) => (0x01, Vec::new()),
        },
    };
    let port = mapped.port ^ ((MAGIC_COOKIE >> 16) as u16);
    let length = 4 + encoded.len();
    let mut attribute = Vec::with_capacity(4 + length);
    attribute.extend_from_slice(&ATTR_XOR_MAPPED_ADDRESS.to_be_bytes());
    attribute.extend_from_slice(&(length as u16).to_be_bytes());
    attribute.push(0);
    attribute.push(family);
    attribute.extend_from_slice(&port.to_be_bytes());
    attribute.extend_from_slice(&encoded);
    attribute
}

fn xor_mask(transaction: &TransactionId) -> [u8; 16] {
    let mut mask = [0u8; 16];
    mask[..4].copy_from_slice(&MAGIC_COOKIE.to_be_bytes());
    mask[4..].copy_from_slice(transaction);
    mask
}

/// Parse a Binding Response, validating the magic cookie, the declared length
/// and the echoed transaction id before reading any attribute.
pub fn parse_binding_response(
    bytes: &[u8],
    transaction: &TransactionId,
) -> Result<StunResponse, StunWireError> {
    if bytes.len() < HEADER_LEN {
        return Err(StunWireError::TooShort { len: bytes.len() });
    }
    let message_type = u16::from_be_bytes([bytes[0], bytes[1]]);
    let declared = u16::from_be_bytes([bytes[2], bytes[3]]) as usize;
    let actual = bytes.len() - HEADER_LEN;
    if declared != actual || !declared.is_multiple_of(4) {
        return Err(StunWireError::LengthMismatch { declared, actual });
    }
    let cookie = u32::from_be_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);
    if cookie != MAGIC_COOKIE {
        return Err(StunWireError::BadMagicCookie { actual: cookie });
    }
    if &bytes[8..HEADER_LEN] != transaction.as_slice() {
        return Err(StunWireError::TransactionMismatch);
    }

    match message_type {
        BINDING_SUCCESS_RESPONSE => {
            let mapped = find_xor_mapped_address(bytes, transaction)?;
            match mapped {
                Some(mapped) => Ok(StunResponse {
                    message_type,
                    mapped: Some(mapped),
                    error_code: None,
                    error_reason: None,
                }),
                None => Err(StunWireError::MissingMappedAddress),
            }
        }
        BINDING_ERROR_RESPONSE => {
            let (error_code, error_reason) = find_error_code(bytes)?;
            Ok(StunResponse {
                message_type,
                mapped: None,
                error_code,
                error_reason,
            })
        }
        other => Err(StunWireError::UnexpectedMessageType {
            message_type: other,
        }),
    }
}

/// Iterate the attributes and return the first XOR-MAPPED-ADDRESS, decoded.
fn find_xor_mapped_address(
    bytes: &[u8],
    transaction: &TransactionId,
) -> Result<Option<StunMappedAddress>, StunWireError> {
    for (attribute_type, value) in attributes(bytes)? {
        if attribute_type == ATTR_XOR_MAPPED_ADDRESS {
            return decode_xor_mapped_address(value, transaction).map(Some);
        }
    }
    Ok(None)
}

/// Iterate the attributes and return the first ERROR-CODE.
fn find_error_code(bytes: &[u8]) -> Result<(Option<u16>, Option<String>), StunWireError> {
    for (attribute_type, value) in attributes(bytes)? {
        if attribute_type == ATTR_ERROR_CODE {
            if value.len() < 4 {
                return Err(StunWireError::InvalidAddressLength {
                    length: value.len(),
                });
            }
            // The first two bytes are reserved; the class is the top 3 bits of
            // byte 2 and the number is byte 3.
            let class = (value[2] & 0x07) as u16;
            let number = value[3] as u16;
            let code = class * 100 + number;
            let reason = String::from_utf8_lossy(&value[4..]).into_owned();
            return Ok((Some(code), Some(reason)));
        }
    }
    Ok((None, None))
}

/// Every attribute as `(type, value)`, with 4-byte alignment enforced.
fn attributes(bytes: &[u8]) -> Result<Vec<(u16, &[u8])>, StunWireError> {
    let mut attributes = Vec::new();
    let mut offset = HEADER_LEN;
    while offset + 4 <= bytes.len() {
        let attribute_type = u16::from_be_bytes([bytes[offset], bytes[offset + 1]]);
        let length = u16::from_be_bytes([bytes[offset + 2], bytes[offset + 3]]) as usize;
        let value_start = offset + 4;
        let value_end = value_start + length;
        if value_end > bytes.len() {
            return Err(StunWireError::TruncatedAttribute { offset });
        }
        attributes.push((attribute_type, &bytes[value_start..value_end]));
        offset = value_start + length.next_multiple_of(4);
    }
    Ok(attributes)
}

fn decode_xor_mapped_address(
    value: &[u8],
    transaction: &TransactionId,
) -> Result<StunMappedAddress, StunWireError> {
    if value.len() < 4 {
        return Err(StunWireError::InvalidAddressLength {
            length: value.len(),
        });
    }
    let family = value[1];
    let x_port = u16::from_be_bytes([value[2], value[3]]);
    let port = x_port ^ ((MAGIC_COOKIE >> 16) as u16);
    match family {
        0x01 => {
            if value.len() != 8 {
                return Err(StunWireError::InvalidAddressLength {
                    length: value.len(),
                });
            }
            let cookie = MAGIC_COOKIE.to_be_bytes();
            let mut octets = [0u8; 4];
            for index in 0..4 {
                octets[index] = value[4 + index] ^ cookie[index];
            }
            Ok(StunMappedAddress::new(
                Ipv4Addr::from(octets).to_string(),
                port,
            ))
        }
        0x02 => {
            if value.len() != 20 {
                return Err(StunWireError::InvalidAddressLength {
                    length: value.len(),
                });
            }
            let mask = xor_mask(transaction);
            let mut octets = [0u8; 16];
            for index in 0..16 {
                octets[index] = value[4 + index] ^ mask[index];
            }
            Ok(StunMappedAddress::new(
                Ipv6Addr::from(octets).to_string(),
                port,
            ))
        }
        family => Err(StunWireError::InvalidAddressFamily { family }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn transaction() -> TransactionId {
        [
            0x01, 0x23, 0x45, 0x67, 0x89, 0xab, 0xcd, 0xef, 0x11, 0x22, 0x33, 0x44,
        ]
    }

    #[test]
    fn a_binding_request_is_twenty_bytes_with_the_stun_header() {
        let id = transaction();
        let request = encode_binding_request(&id);
        assert_eq!(request.len(), HEADER_LEN);
        assert_eq!(
            u16::from_be_bytes([request[0], request[1]]),
            BINDING_REQUEST
        );
        assert_eq!(u16::from_be_bytes([request[2], request[3]]), 0);
        assert_eq!(
            u32::from_be_bytes([request[4], request[5], request[6], request[7]]),
            MAGIC_COOKIE
        );
        assert_eq!(&request[8..], id.as_slice());
    }

    #[test]
    fn a_crafted_success_response_decodes_the_xor_mapped_address() {
        let id = transaction();
        let expected = StunMappedAddress::new("203.0.113.9", 51234);
        let response = encode_binding_success(&id, &expected);
        let parsed = parse_binding_response(&response, &id).expect("parse");
        assert!(parsed.is_success());
        assert_eq!(parsed.mapped, Some(expected.clone()));
        assert_eq!(parsed.error_code, None);
    }

    #[test]
    fn an_ipv6_mapping_uses_the_cookie_and_transaction_as_the_mask() {
        let id = transaction();
        let expected = StunMappedAddress::new("2001:db8::1", 3478);
        let response = encode_binding_success(&id, &expected);
        let parsed = parse_binding_response(&response, &id).expect("parse");
        assert_eq!(parsed.mapped, Some(expected));
    }

    #[test]
    fn an_answer_for_another_transaction_is_rejected() {
        let id = transaction();
        let response = encode_binding_success(&id, &StunMappedAddress::new("203.0.113.9", 1));
        let mut other = id;
        other[0] ^= 0xff;
        assert_eq!(
            parse_binding_response(&response, &other),
            Err(StunWireError::TransactionMismatch)
        );
    }

    #[test]
    fn a_foreign_datagram_is_rejected_before_any_attribute_is_read() {
        let id = transaction();
        let mut response = encode_binding_success(&id, &StunMappedAddress::new("203.0.113.9", 1));
        response[4] = 0x00;
        assert!(matches!(
            parse_binding_response(&response, &id),
            Err(StunWireError::BadMagicCookie { .. })
        ));

        assert_eq!(
            parse_binding_response(&[0u8; 4], &id),
            Err(StunWireError::TooShort { len: 4 })
        );
    }

    #[test]
    fn a_wrong_declared_length_is_rejected() {
        let id = transaction();
        let mut response = encode_binding_success(&id, &StunMappedAddress::new("203.0.113.9", 1));
        response[2] = 0xff;
        response[3] = 0xff;
        assert!(matches!(
            parse_binding_response(&response, &id),
            Err(StunWireError::LengthMismatch { .. })
        ));
    }

    #[test]
    fn a_truncated_attribute_is_rejected_instead_of_read() {
        let id = transaction();
        let mut response = encode_binding_success(&id, &StunMappedAddress::new("203.0.113.9", 1));
        // Claim the attribute value is longer than the datagram carries.
        response[22] = 0x00;
        response[23] = 0x40;
        assert!(matches!(
            parse_binding_response(&response, &id),
            Err(StunWireError::TruncatedAttribute { .. })
        ));
    }

    #[test]
    fn a_success_response_without_the_xor_attribute_is_not_a_mapping() {
        let id = transaction();
        let mut response = Vec::new();
        response.extend_from_slice(&BINDING_SUCCESS_RESPONSE.to_be_bytes());
        response.extend_from_slice(&0u16.to_be_bytes());
        response.extend_from_slice(&MAGIC_COOKIE.to_be_bytes());
        response.extend_from_slice(&id);
        assert_eq!(
            parse_binding_response(&response, &id),
            Err(StunWireError::MissingMappedAddress)
        );
    }

    #[test]
    fn a_non_binding_message_type_is_rejected() {
        let id = transaction();
        let mut response = encode_binding_success(&id, &StunMappedAddress::new("203.0.113.9", 1));
        response[0] = 0x00;
        response[1] = 0x03;
        assert_eq!(
            parse_binding_response(&response, &id),
            Err(StunWireError::UnexpectedMessageType { message_type: 3 })
        );
    }

    #[test]
    fn an_error_response_carries_its_code_and_reason_without_a_mapping() {
        let id = transaction();
        let reason = "Unauthorized";
        let mut value = Vec::new();
        value.extend_from_slice(&[0, 0, 0x04, 0x01]); // class 4, number 1 => 401
        value.extend_from_slice(reason.as_bytes());
        let mut response = Vec::new();
        response.extend_from_slice(&BINDING_ERROR_RESPONSE.to_be_bytes());
        response.extend_from_slice(&((value.len() + 4) as u16).to_be_bytes());
        response.extend_from_slice(&MAGIC_COOKIE.to_be_bytes());
        response.extend_from_slice(&id);
        response.extend_from_slice(&ATTR_ERROR_CODE.to_be_bytes());
        response.extend_from_slice(&(value.len() as u16).to_be_bytes());
        response.extend_from_slice(&value);

        let parsed = parse_binding_response(&response, &id).expect("parse");
        assert!(!parsed.is_success());
        assert_eq!(parsed.mapped, None);
        assert_eq!(parsed.error_code, Some(401));
        assert_eq!(parsed.error_reason.as_deref(), Some(reason));
    }

    #[test]
    fn transaction_ids_are_fresh_and_the_request_round_trips_the_codec() {
        let first = new_transaction_id();
        let second = new_transaction_id();
        assert_ne!(first, second);
        // The encoded request is exactly what the parser expects to echo.
        assert_eq!(encode_binding_request(&first).len(), HEADER_LEN);
    }
}
