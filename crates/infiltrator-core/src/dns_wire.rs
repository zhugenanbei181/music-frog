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

/// `TXT` record type (RFC 1035 §3.3.14).
pub const TYPE_TXT: u16 = 16;

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

    /// A `TXT` question, used by the echo probe's TXT extraction rules.
    pub fn txt(id: u16, qname: &str) -> Self {
        Self {
            id,
            qname: qname.trim().trim_end_matches('.').to_owned(),
            qtype: TYPE_TXT,
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

/// The response code of a datagram (`0` = `NOERROR`), or `None` when it is
/// shorter than a DNS header. Used by the echo probe to tell "the authority
/// answered for this name" from a real observation.
pub fn response_rcode(bytes: &[u8]) -> Option<u8> {
    // A datagram shorter than a DNS header is not a response at all; reading
    // the flag bits out of a stub would report a bogus NOERROR.
    if bytes.len() < HEADER_LEN {
        return None;
    }
    let flags = u16::from_be_bytes([bytes[2], bytes[3]]);
    Some((flags & 0x000f) as u8)
}

/// Read the first `A` record from an answer, after the strict
/// [`validate_response`] checks. The recorded address is the echo authority's
/// observation of the resolver; anything else (an empty answer section, a
/// non-`A` record) yields `None` rather than a guess.
pub fn extract_a_record(
    bytes: &[u8],
    question: &DnsQuestion,
) -> Result<Option<std::net::IpAddr>, DnsWireError> {
    validate_response(bytes, question)?;
    let qname = encode_qname(&question.qname)?;
    // ANCOUNT lives at offset 6; offset 4 is QDCOUNT (the question section
    // this probe already validated), so reading it here made a record-less
    // answer walk one bogus record.
    let ancount = u16::from_be_bytes([bytes[6], bytes[7]]);
    let mut offset = HEADER_LEN + qname.len() + 4;
    for _ in 0..ancount {
        offset = skip_name(bytes, offset)?;
        let header = bytes
            .get(offset..offset + 10)
            .ok_or(DnsWireError::TooShort { len: bytes.len() })?;
        let record_type = u16::from_be_bytes([header[0], header[1]]);
        let record_class = u16::from_be_bytes([header[2], header[3]]);
        let rdlength = u16::from_be_bytes([header[8], header[9]]) as usize;
        offset += 10;
        if record_type == TYPE_A && record_class == CLASS_IN && rdlength == 4 {
            let octets = bytes
                .get(offset..offset + 4)
                .ok_or(DnsWireError::TooShort { len: bytes.len() })?;
            return Ok(Some(std::net::IpAddr::from([
                octets[0], octets[1], octets[2], octets[3],
            ])));
        }
        offset = offset
            .checked_add(rdlength)
            .ok_or(DnsWireError::TooShort { len: bytes.len() })?;
        if offset > bytes.len() {
            return Err(DnsWireError::TooShort { len: bytes.len() });
        }
    }
    Ok(None)
}

/// Read every character-string of every `TXT` record from an answer, after the
/// strict [`validate_response`] checks.
///
/// The outer `Vec` holds one entry per `TXT` record in answer order; each
/// inner `Vec` holds that record's length-prefixed character-strings in wire
/// order (RFC 1035 §3.3.14 — a `TXT` rdata is one or more such strings).
/// Grouping by record keeps a `"key" "<value>"` pair attributable to the
/// record that carried it. An answer with no `TXT` record yields `None`, never
/// a fabricated value.
pub fn extract_txt_records(
    bytes: &[u8],
    question: &DnsQuestion,
) -> Result<Option<Vec<Vec<String>>>, DnsWireError> {
    validate_response(bytes, question)?;
    let qname = encode_qname(&question.qname)?;
    let ancount = u16::from_be_bytes([bytes[6], bytes[7]]);
    let mut offset = HEADER_LEN + qname.len() + 4;
    let mut records: Vec<Vec<String>> = Vec::new();
    for _ in 0..ancount {
        offset = skip_name(bytes, offset)?;
        let header = bytes
            .get(offset..offset + 10)
            .ok_or(DnsWireError::TooShort { len: bytes.len() })?;
        let record_type = u16::from_be_bytes([header[0], header[1]]);
        let record_class = u16::from_be_bytes([header[2], header[3]]);
        let rdlength = u16::from_be_bytes([header[8], header[9]]) as usize;
        offset += 10;
        let rdata_end = offset
            .checked_add(rdlength)
            .ok_or(DnsWireError::TooShort { len: bytes.len() })?;
        if rdata_end > bytes.len() {
            return Err(DnsWireError::TooShort { len: bytes.len() });
        }
        if record_type == TYPE_TXT && record_class == CLASS_IN {
            records.push(decode_txt_rdata(&bytes[offset..rdata_end])?);
        }
        offset = rdata_end;
    }
    if records.is_empty() {
        Ok(None)
    } else {
        Ok(Some(records))
    }
}

/// Decode a `TXT` rdata into its length-prefixed character-strings. A length
/// prefix that runs past the rdata is a malformed record, not an empty value.
fn decode_txt_rdata(rdata: &[u8]) -> Result<Vec<String>, DnsWireError> {
    let mut strings = Vec::new();
    let mut offset = 0usize;
    while offset < rdata.len() {
        let length = rdata[offset] as usize;
        offset += 1;
        let end = offset
            .checked_add(length)
            .ok_or(DnsWireError::TooShort { len: rdata.len() })?;
        let value = rdata
            .get(offset..end)
            .ok_or(DnsWireError::TooShort { len: rdata.len() })?;
        strings.push(String::from_utf8_lossy(value).into_owned());
        offset = end;
    }
    Ok(strings)
}

/// Skip one (possibly compressed) name in the answer section.
fn skip_name(bytes: &[u8], mut offset: usize) -> Result<usize, DnsWireError> {
    loop {
        let length = *bytes
            .get(offset)
            .ok_or(DnsWireError::TooShort { len: bytes.len() })? as usize;
        if length & 0xc0 == 0xc0 {
            return if bytes.get(offset + 1).is_some() {
                Ok(offset + 2)
            } else {
                Err(DnsWireError::TooShort { len: bytes.len() })
            };
        }
        if length == 0 {
            return Ok(offset + 1);
        }
        offset = offset
            .checked_add(1 + length)
            .ok_or(DnsWireError::TooShort { len: bytes.len() })?;
        if offset > bytes.len() {
            return Err(DnsWireError::TooShort { len: bytes.len() });
        }
    }
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

/// Encode the `TXT` answer a real nameserver would return for `question`, with
/// one record carrying `strings` in order. Used by the offline tests so the
/// TXT extraction rules are exercised against real wire bytes.
pub fn encode_txt_answer(question: &DnsQuestion, strings: &[&str], ttl_seconds: u32) -> Vec<u8> {
    let qname = encode_qname(&question.qname).unwrap_or_default();
    let mut rdata = Vec::new();
    for value in strings {
        rdata.push(value.len().min(255) as u8);
        rdata.extend_from_slice(&value.as_bytes()[..value.len().min(255)]);
    }
    let mut answer = Vec::with_capacity(HEADER_LEN + qname.len() + 4 + 16 + rdata.len());
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
    answer.extend_from_slice(&TYPE_TXT.to_be_bytes());
    answer.extend_from_slice(&CLASS_IN.to_be_bytes());
    answer.extend_from_slice(&ttl_seconds.to_be_bytes());
    answer.extend_from_slice(&(rdata.len() as u16).to_be_bytes());
    answer.extend_from_slice(&rdata);
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

    #[test]
    fn the_echo_observation_is_read_from_the_real_answer_record() {
        let question = question();
        let answer = encode_answer(&question, [203, 0, 113, 9], 60);
        assert_eq!(response_rcode(&answer), Some(0));
        assert_eq!(
            extract_a_record(&answer, &question),
            Ok(Some(std::net::IpAddr::from([203, 0, 113, 9])))
        );
    }

    #[test]
    fn an_answer_without_an_a_record_yields_nothing_instead_of_a_guess() {
        let question = question();
        let mut empty = encode_answer(&question, [203, 0, 113, 9], 60);
        let qname = encode_qname(&question.qname).expect("qname");
        empty.truncate(HEADER_LEN + qname.len() + 4);
        empty[6..8].copy_from_slice(&0u16.to_be_bytes());
        assert_eq!(extract_a_record(&empty, &question), Ok(None));

        // A truncated answer is rejected, never read as an identity.
        let truncated = encode_answer(&question, [203, 0, 113, 9], 60);
        let cut = truncated.len() - 2;
        assert!(matches!(
            extract_a_record(&truncated[..cut], &question),
            Err(DnsWireError::TooShort { .. })
        ));

        // A non-zero response code is visible to the probe.
        let mut refused = encode_answer(&question, [203, 0, 113, 9], 60);
        let flags = u16::from_be_bytes([refused[2], refused[3]]) | 0x0003;
        refused[2..4].copy_from_slice(&flags.to_be_bytes());
        assert_eq!(response_rcode(&refused), Some(3));
        assert_eq!(response_rcode(&[0u8; 4]), None);
    }

    #[test]
    fn a_txt_question_asks_for_type_txt() {
        let txt = DnsQuestion::txt(0x4d46, "whoami.ds.akahelp.net");
        assert_eq!(txt.qtype, TYPE_TXT);
        let query = encode_query(&txt).expect("txt query");
        assert_eq!(
            u16::from_be_bytes([query[query.len() - 4], query[query.len() - 3]]),
            TYPE_TXT
        );
    }

    #[test]
    fn txt_character_strings_are_read_in_record_order() {
        let question = DnsQuestion::txt(0x4d46, "whoami.ds.akahelp.net");
        let answer = encode_txt_answer(&question, &["ip", "203.0.113.9"], 60);
        assert_eq!(validate_response(&answer, &question), Ok(()));
        assert_eq!(
            extract_txt_records(&answer, &question),
            Ok(Some(vec![vec!["ip".to_owned(), "203.0.113.9".to_owned()]]))
        );
    }

    #[test]
    fn a_single_string_txt_answer_is_read_verbatim() {
        let question = DnsQuestion::txt(0x4d46, "o-o.myaddr.l.google.com");
        let answer = encode_txt_answer(&question, &["198.51.100.7"], 30);
        assert_eq!(
            extract_txt_records(&answer, &question),
            Ok(Some(vec![vec!["198.51.100.7".to_owned()]]))
        );
    }

    #[test]
    fn a_txt_answer_with_no_txt_record_is_none_not_a_guess() {
        let question = DnsQuestion::txt(0x4d46, "echo.example.org");
        // An A answer to a TXT question fails the strict question check; a
        // record-less TXT answer is a clean `None`.
        let mut empty = encode_txt_answer(&question, &["ip", "203.0.113.9"], 60);
        let qname = encode_qname(&question.qname).expect("qname");
        empty.truncate(HEADER_LEN + qname.len() + 4);
        empty[6..8].copy_from_slice(&0u16.to_be_bytes());
        assert_eq!(extract_txt_records(&empty, &question), Ok(None));

        // A truncated character-string is rejected, never decoded as a value.
        let mut malformed = encode_txt_answer(&question, &["ip", "203.0.113.9"], 60);
        let length = malformed.len();
        malformed.truncate(length - 3);
        assert!(matches!(
            extract_txt_records(&malformed, &question),
            Err(DnsWireError::TooShort { .. })
        ));
    }

    #[test]
    fn a_txt_question_does_not_accept_an_a_answer() {
        let txt = DnsQuestion::txt(0x4d46, "probe.example.com");
        let a_answer = encode_answer(
            &DnsQuestion::a_record(0x4d46, "probe.example.com"),
            [1, 2, 3, 4],
            60,
        );
        assert_eq!(
            extract_txt_records(&a_answer, &txt),
            Err(DnsWireError::QuestionMismatch)
        );
    }
}
