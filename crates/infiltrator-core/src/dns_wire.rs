//! DUAL-14-10: the minimal DNS wire codec the per-nameserver probe uses.
//!
//! Only the query/response shapes a latency probe needs are implemented: a
//! single-question query whose answer must carry the same id and echo the
//! question. Everything else is rejected, so a stray datagram can never be
//! timed as if it were the answer to this probe.

/// Fixed DNS header length (RFC 1035 §4.1.1).
pub const HEADER_LEN: usize = 12;

/// `A` record type.
pub const TYPE_A: u16 = 1;

/// `IN` class.
pub const CLASS_IN: u16 = 1;

/// Largest datagram this prober reads from a UDP nameserver.
pub const MAX_RESPONSE_BYTES: usize = 4096;

/// The `QR` bit: a response rather than a query.
const FLAG_RESPONSE: u16 = 0x8000;

/// The `RD` bit: recursion desired.
const FLAG_RECURSION_DESIRED: u16 = 0x0100;

/// Largest legal label.
const MAX_LABEL_LEN: usize = 63;

/// One probe question.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DnsQuestion {
    pub id: u16,
    pub qname: String,
    pub qtype: u16,
}

impl DnsQuestion {
    /// The single question every latency probe asks.
    pub fn a_record(id: u16, qname: &str) -> Self {
        Self {
            id,
            qname: qname.trim().trim_end_matches('.').to_owned(),
            qtype: TYPE_A,
        }
    }
}

/// Why a datagram is not the answer to this probe.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DnsWireError {
    /// Fewer bytes than a DNS header.
    TooShort { len: usize },
    /// The answer carries a different transaction id.
    IdMismatch { expected: u16, actual: u16 },
    /// The `QR` bit is clear: this is a query, not an answer.
    NotAResponse,
    /// The answer does not carry the question this probe asked.
    QuestionMismatch,
    /// A qname label is empty, over-long, or the name is over-long.
    InvalidQname { qname: String },
}

impl std::fmt::Display for DnsWireError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TooShort { len } => {
                write!(formatter, "answer is {len} bytes, shorter than a header")
            }
            Self::IdMismatch { expected, actual } => {
                write!(
                    formatter,
                    "answer id {actual} does not match probe id {expected}"
                )
            }
            Self::NotAResponse => write!(formatter, "datagram is not a DNS response"),
            Self::QuestionMismatch => write!(formatter, "answer does not echo the probe question"),
            Self::InvalidQname { qname } => write!(formatter, "invalid probe name {qname}"),
        }
    }
}

impl std::error::Error for DnsWireError {}

/// Encode a single-question query with recursion desired.
pub fn encode_query(question: &DnsQuestion) -> Result<Vec<u8>, DnsWireError> {
    let mut message = Vec::with_capacity(HEADER_LEN + question.qname.len() + 6);
    message.extend_from_slice(&question.id.to_be_bytes());
    message.extend_from_slice(&FLAG_RECURSION_DESIRED.to_be_bytes());
    message.extend_from_slice(&1u16.to_be_bytes());
    message.extend_from_slice(&0u16.to_be_bytes());
    message.extend_from_slice(&0u16.to_be_bytes());
    message.extend_from_slice(&0u16.to_be_bytes());
    message.extend_from_slice(&encode_qname(&question.qname)?);
    message.extend_from_slice(&question.qtype.to_be_bytes());
    message.extend_from_slice(&CLASS_IN.to_be_bytes());
    Ok(message)
}

/// Validate a datagram against the probe that produced it.
///
/// The check is deliberately strict: the id, the `QR` bit and the echoed
/// question must all match before a round trip is reported.
pub fn validate_response(bytes: &[u8], question: &DnsQuestion) -> Result<(), DnsWireError> {
    if bytes.len() < HEADER_LEN {
        return Err(DnsWireError::TooShort { len: bytes.len() });
    }
    let actual = u16::from_be_bytes([bytes[0], bytes[1]]);
    if actual != question.id {
        return Err(DnsWireError::IdMismatch {
            expected: question.id,
            actual,
        });
    }
    let flags = u16::from_be_bytes([bytes[2], bytes[3]]);
    if flags & FLAG_RESPONSE == 0 {
        return Err(DnsWireError::NotAResponse);
    }
    let qdcount = u16::from_be_bytes([bytes[4], bytes[5]]);
    if qdcount == 0 {
        return Err(DnsWireError::QuestionMismatch);
    }
    let expected = encode_qname(&question.qname)?;
    let end = HEADER_LEN + expected.len() + 4;
    if bytes.len() < end || &bytes[HEADER_LEN..HEADER_LEN + expected.len()] != expected.as_slice() {
        return Err(DnsWireError::QuestionMismatch);
    }
    let qtype = u16::from_be_bytes([
        bytes[HEADER_LEN + expected.len()],
        bytes[HEADER_LEN + expected.len() + 1],
    ]);
    let qclass = u16::from_be_bytes([bytes[end - 2], bytes[end - 1]]);
    if qtype != question.qtype || qclass != CLASS_IN {
        return Err(DnsWireError::QuestionMismatch);
    }
    Ok(())
}

/// Encode the qname as length-prefixed labels ending with the root label.
pub fn encode_qname(qname: &str) -> Result<Vec<u8>, DnsWireError> {
    let name = qname.trim().trim_end_matches('.');
    if name.is_empty() {
        return Err(DnsWireError::InvalidQname {
            qname: qname.to_owned(),
        });
    }
    let mut encoded = Vec::with_capacity(name.len() + 2);
    let mut total = 1usize;
    for label in name.split('.') {
        if label.is_empty() || label.len() > MAX_LABEL_LEN {
            return Err(DnsWireError::InvalidQname {
                qname: qname.to_owned(),
            });
        }
        total += label.len() + 1;
        if total > 255 {
            return Err(DnsWireError::InvalidQname {
                qname: qname.to_owned(),
            });
        }
        encoded.push(label.len() as u8);
        encoded.extend_from_slice(label.as_bytes());
    }
    encoded.push(0);
    Ok(encoded)
}

/// Encode the answer a real nameserver would return for `question`, so the
/// loopback tests exercise the exact validation the probe performs.
pub fn encode_answer(question: &DnsQuestion, answer_ip: [u8; 4], ttl_seconds: u32) -> Vec<u8> {
    let qname = encode_qname(&question.qname).unwrap_or_default();
    let mut answer = Vec::with_capacity(HEADER_LEN + qname.len() + 4 + 16);
    answer.extend_from_slice(&question.id.to_be_bytes());
    answer.extend_from_slice(&(FLAG_RESPONSE | FLAG_RECURSION_DESIRED).to_be_bytes());
    answer.extend_from_slice(&1u16.to_be_bytes());
    answer.extend_from_slice(&1u16.to_be_bytes());
    answer.extend_from_slice(&0u16.to_be_bytes());
    answer.extend_from_slice(&0u16.to_be_bytes());
    answer.extend_from_slice(&qname);
    answer.extend_from_slice(&question.qtype.to_be_bytes());
    answer.extend_from_slice(&CLASS_IN.to_be_bytes());
    // The answer record points back at the question name.
    answer.extend_from_slice(&[0xc0, 0x0c]);
    answer.extend_from_slice(&TYPE_A.to_be_bytes());
    answer.extend_from_slice(&CLASS_IN.to_be_bytes());
    answer.extend_from_slice(&ttl_seconds.to_be_bytes());
    answer.extend_from_slice(&4u16.to_be_bytes());
    answer.extend_from_slice(&answer_ip);
    answer
}

#[cfg(test)]
mod tests {
    use super::*;

    fn question() -> DnsQuestion {
        DnsQuestion::a_record(0x4d46, "probe.example.com")
    }

    #[test]
    fn a_query_encodes_the_header_question_and_class() {
        let query = encode_query(&question()).expect("query");
        assert_eq!(query.len(), HEADER_LEN + 1 + 5 + 1 + 7 + 1 + 3 + 1 + 4);
        assert_eq!(u16::from_be_bytes([query[0], query[1]]), 0x4d46);
        assert_eq!(
            u16::from_be_bytes([query[2], query[3]]),
            FLAG_RECURSION_DESIRED
        );
        assert_eq!(u16::from_be_bytes([query[4], query[5]]), 1);
        assert_eq!(&query[6..8], &[0, 0], "ancount is zero in a query");
        assert_eq!(&query[12..13], &[5u8]);
        assert_eq!(&query[13..18], b"probe");
        assert_eq!(&query[18..19], &[7u8]);
        assert_eq!(&query[19..26], b"example");
        assert_eq!(
            &query[query.len() - 4..],
            &[0, TYPE_A as u8, 0, CLASS_IN as u8],
            "the question ends with type A / class IN"
        );
    }

    #[test]
    fn a_nameserver_answer_with_the_same_id_and_question_validates() {
        let question = question();
        let answer = encode_answer(&question, [203, 0, 113, 9], 60);
        assert_eq!(validate_response(&answer, &question), Ok(()));
    }

    #[test]
    fn a_wrong_id_is_rejected_before_anything_is_timed() {
        let question = question();
        let mut answer = encode_answer(&question, [203, 0, 113, 9], 60);
        answer[0..2].copy_from_slice(&0x1111u16.to_be_bytes());
        assert_eq!(
            validate_response(&answer, &question),
            Err(DnsWireError::IdMismatch {
                expected: 0x4d46,
                actual: 0x1111
            })
        );
    }

    #[test]
    fn a_query_a_truncated_datagram_and_a_mismatched_question_are_rejected() {
        let question = question();
        assert_eq!(
            validate_response(&[0u8; 4], &question),
            Err(DnsWireError::TooShort { len: 4 })
        );

        let mut query_like = encode_query(&question).expect("query");
        query_like.truncate(HEADER_LEN);
        assert_eq!(
            validate_response(&query_like, &question),
            Err(DnsWireError::NotAResponse)
        );

        let other = DnsQuestion::a_record(question.id, "other.example.com");
        let answer = encode_answer(&other, [203, 0, 113, 9], 60);
        assert_eq!(
            validate_response(&answer, &question),
            Err(DnsWireError::QuestionMismatch)
        );

        let mut headerless = encode_answer(&question, [203, 0, 113, 9], 60);
        headerless[4..6].copy_from_slice(&0u16.to_be_bytes());
        assert_eq!(
            validate_response(&headerless, &question),
            Err(DnsWireError::QuestionMismatch)
        );
    }

    #[test]
    fn an_unusable_probe_name_is_refused_instead_of_encoded() {
        assert_eq!(
            encode_qname(""),
            Err(DnsWireError::InvalidQname {
                qname: String::new()
            })
        );
        assert!(encode_qname("a..b").is_err());
        assert!(encode_qname(&"x".repeat(64)).is_err());
        // 5 × 61 + 1 = 306 encoded bytes, over the 255 byte name limit.
        assert!(encode_qname(&vec!["x".repeat(60); 5].join(".")).is_err());
        assert!(encode_qname(&vec!["x".repeat(60); 4].join(".")).is_ok());
        assert_eq!(
            encode_qname("probe.example.com.").expect("trailing dot"),
            encode_qname("probe.example.com").expect("plain")
        );
        assert_eq!(
            DnsQuestion::a_record(1, " Probe.Example.COM. ").qname,
            "Probe.Example.COM"
        );
        assert!(!DnsWireError::NotAResponse.to_string().is_empty());
    }
}
