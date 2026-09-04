use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::convert::TryInto;

/// Magic bytes recognized by standard Mihomo / Clash MRS formats.
pub const MAGIC_STANDARD_MRS: [u8; 4] = [0x00, 0x4D, 0x52, 0x53]; // \x00MRS
pub const MAGIC_LEGACY_MRS_V1: [u8; 4] = [b'M', b'R', b'S', 0x01]; // MRS\x01
pub const MAGIC_MRS_PREFIX: &[u8; 3] = b"MRS";

/// Rule-set matching behavior type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Behavior {
    Domain,
    IpCidr,
    Classical,
    Unknown(u8),
}

impl Behavior {
    pub fn as_str(&self) -> &'static str {
        match self {
            Behavior::Domain => "domain",
            Behavior::IpCidr => "ipcidr",
            Behavior::Classical => "classical",
            Behavior::Unknown(_) => "unknown",
        }
    }

    pub fn from_u8(code: u8) -> Self {
        match code {
            0 => Behavior::Domain,
            1 => Behavior::IpCidr,
            2 => Behavior::Classical,
            x => Behavior::Unknown(x),
        }
    }

    pub fn to_u8(&self) -> u8 {
        match self {
            Behavior::Domain => 0,
            Behavior::IpCidr => 1,
            Behavior::Classical => 2,
            Behavior::Unknown(x) => *x,
        }
    }

    pub fn parse_behavior_str(s: &str) -> Self {
        match s.trim().to_lowercase().as_str() {
            "domain" => Behavior::Domain,
            "ipcidr" | "ip-cidr" | "cidr" => Behavior::IpCidr,
            "classical" => Behavior::Classical,
            _ => Behavior::Unknown(255),
        }
    }
}

/// Compression format applied to the MRS binary payload.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum CompressionType {
    #[default]
    None,
    Zstd,
    Gzip,
    Unknown(u8),
}

impl CompressionType {
    pub fn as_str(&self) -> &'static str {
        match self {
            CompressionType::None => "none",
            CompressionType::Zstd => "zstd",
            CompressionType::Gzip => "gzip",
            CompressionType::Unknown(_) => "unknown",
        }
    }

    pub fn from_u8(code: u8) -> Self {
        match code {
            0 => CompressionType::None,
            1 => CompressionType::Zstd,
            2 => CompressionType::Gzip,
            x => CompressionType::Unknown(x),
        }
    }

    pub fn to_u8(&self) -> u8 {
        match self {
            CompressionType::None => 0,
            CompressionType::Zstd => 1,
            CompressionType::Gzip => 2,
            CompressionType::Unknown(x) => *x,
        }
    }

    /// Sniff compression format from leading payload bytes (e.g. Zstd or Gzip magic header).
    pub fn detect_payload_compression(payload: &[u8]) -> Self {
        if payload.len() >= 4 && payload[0..4] == [0x28, 0xB5, 0x2F, 0xFD] {
            CompressionType::Zstd
        } else if payload.len() >= 2 && payload[0..2] == [0x1F, 0x8B] {
            CompressionType::Gzip
        } else {
            CompressionType::None
        }
    }
}

/// Parsed metadata for a Mihomo Rule Set file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MrsMetadata {
    pub behavior: Behavior,
    pub rule_count: u32,
    pub version: u8,
    pub payload_size: u32,
    pub description: String,
}

/// Detailed binary header info for an MRS file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MrsHeader {
    pub magic: [u8; 4],
    pub version: u8,
    pub behavior: Behavior,
    pub compression: CompressionType,
    pub rule_count: u32,
    pub payload_size: u32,
    pub description: String,
    pub timestamp: Option<u64>,
    pub crc32: Option<u32>,
    pub sha256: Option<String>,
}

/// Verification report returned by `validate_mrs_bytes`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MrsValidationReport {
    pub is_valid: bool,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
    pub metadata: Option<MrsMetadata>,
    pub header: Option<MrsHeader>,
    pub sha256_digest: Option<String>,
    pub crc32_checksum: Option<u32>,
}

/// Check if the 4-byte prefix is a recognized MRS magic header.
pub fn is_valid_mrs_magic(magic: &[u8]) -> bool {
    if magic.len() < 4 {
        return false;
    }
    magic == MAGIC_STANDARD_MRS || magic == MAGIC_LEGACY_MRS_V1 || &magic[0..3] == MAGIC_MRS_PREFIX
}

/// Parse standard 16-byte fixed header plus UTF-8 description from MRS binary content.
pub fn parse_mrs_header(bytes: &[u8]) -> Result<MrsMetadata> {
    if bytes.len() < 4 || !is_valid_mrs_magic(&bytes[0..4]) {
        bail!("Invalid magic header");
    }

    if bytes.len() < 16 {
        bail!("Header too short");
    }

    let version = bytes[4];
    let behavior = Behavior::from_u8(bytes[5]);
    let rule_count = u32::from_le_bytes(bytes[6..10].try_into().unwrap());
    let payload_size = u32::from_le_bytes(bytes[10..14].try_into().unwrap());
    let desc_len = u16::from_le_bytes(bytes[14..16].try_into().unwrap()) as usize;

    if bytes.len() < 16 + desc_len {
        bail!("Incomplete description");
    }

    let description = String::from_utf8(bytes[16..16 + desc_len].to_vec())
        .context("Invalid UTF-8 in description")?;

    Ok(MrsMetadata {
        behavior,
        rule_count,
        version,
        payload_size,
        description,
    })
}

/// Parse extended MRS header details including detected compression, timestamp and digests.
pub fn parse_mrs_header_extended(bytes: &[u8]) -> Result<MrsHeader> {
    let meta = parse_mrs_header(bytes)?;
    let magic: [u8; 4] = bytes[0..4].try_into().unwrap();
    let desc_len = u16::from_le_bytes(bytes[14..16].try_into().unwrap()) as usize;
    let header_len = 16 + desc_len;

    let payload = if bytes.len() > header_len {
        &bytes[header_len..]
    } else {
        &[]
    };

    let compression = CompressionType::detect_payload_compression(payload);
    let crc32 = if !payload.is_empty() {
        Some(compute_crc32(payload))
    } else {
        None
    };
    let sha256 = if !bytes.is_empty() {
        Some(compute_sha256(bytes))
    } else {
        None
    };

    Ok(MrsHeader {
        magic,
        version: meta.version,
        behavior: meta.behavior,
        compression,
        rule_count: meta.rule_count,
        payload_size: meta.payload_size,
        description: meta.description,
        timestamp: None,
        crc32,
        sha256,
    })
}

/// Validate raw MRS bytes against format specifications and compute integrity reports.
pub fn validate_mrs_bytes(bytes: &[u8]) -> Result<MrsValidationReport> {
    let mut errors = Vec::new();
    let mut warnings = Vec::new();

    if bytes.len() < 4 || !is_valid_mrs_magic(&bytes[0..4]) {
        errors.push("Invalid magic header".to_string());
        return Ok(MrsValidationReport {
            is_valid: false,
            errors,
            warnings,
            metadata: None,
            header: None,
            sha256_digest: None,
            crc32_checksum: None,
        });
    }

    if bytes.len() < 16 {
        errors.push("Header too short".to_string());
        return Ok(MrsValidationReport {
            is_valid: false,
            errors,
            warnings,
            metadata: None,
            header: None,
            sha256_digest: None,
            crc32_checksum: None,
        });
    }

    let version = bytes[4];
    let behavior_code = bytes[5];
    let behavior = Behavior::from_u8(behavior_code);
    if let Behavior::Unknown(code) = behavior {
        warnings.push(format!("Unknown rule behavior code: {code}"));
    }

    let rule_count = u32::from_le_bytes(bytes[6..10].try_into().unwrap());
    let payload_size = u32::from_le_bytes(bytes[10..14].try_into().unwrap());
    let desc_len = u16::from_le_bytes(bytes[14..16].try_into().unwrap()) as usize;

    let mut description = String::new();
    if bytes.len() < 16 + desc_len {
        errors.push("Incomplete description".to_string());
    } else {
        match std::str::from_utf8(&bytes[16..16 + desc_len]) {
            Ok(s) => description = s.to_string(),
            Err(_) => errors.push("Invalid UTF-8 in description".to_string()),
        }
    }

    let header_size = 16 + desc_len;
    let actual_payload_len = if bytes.len() >= header_size {
        bytes.len() - header_size
    } else {
        0
    };

    if payload_size > 0 && actual_payload_len < payload_size as usize {
        errors.push(format!(
            "Payload truncated: expected {payload_size} bytes, found {actual_payload_len} bytes"
        ));
    }

    let sha256_digest = Some(compute_sha256(bytes));
    let crc32_checksum = Some(compute_crc32(bytes));

    let (metadata, header) = if errors.is_empty() {
        let meta = MrsMetadata {
            behavior,
            rule_count,
            version,
            payload_size,
            description: description.clone(),
        };
        let payload = &bytes[header_size..];
        let compression = CompressionType::detect_payload_compression(payload);
        let magic: [u8; 4] = bytes[0..4].try_into().unwrap();
        let hdr = MrsHeader {
            magic,
            version,
            behavior,
            compression,
            rule_count,
            payload_size,
            description,
            timestamp: None,
            crc32: if !payload.is_empty() {
                Some(compute_crc32(payload))
            } else {
                None
            },
            sha256: sha256_digest.clone(),
        };
        (Some(meta), Some(hdr))
    } else {
        (None, None)
    };

    let is_valid = errors.is_empty();
    Ok(MrsValidationReport {
        is_valid,
        errors,
        warnings,
        metadata,
        header,
        sha256_digest,
        crc32_checksum,
    })
}

/// Verify integrity of MRS binary data against expected SHA-256 and/or CRC-32 checksums.
pub fn verify_mrs_integrity(
    bytes: &[u8],
    expected_sha256: Option<&str>,
    expected_crc32: Option<u32>,
) -> Result<bool> {
    if let Some(expected_sha) = expected_sha256 {
        let actual_sha = compute_sha256(bytes);
        if !actual_sha.eq_ignore_ascii_case(expected_sha) {
            return Ok(false);
        }
    }

    if let Some(expected_crc) = expected_crc32 {
        let actual_crc = compute_crc32(bytes);
        if actual_crc != expected_crc {
            return Ok(false);
        }
    }

    Ok(true)
}

/// Compute standard IEEE 802.3 CRC32 checksum for a byte slice.
pub fn compute_crc32(bytes: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for &b in bytes {
        crc ^= b as u32;
        for _ in 0..8 {
            if crc & 1 != 0 {
                crc = (crc >> 1) ^ 0xEDB8_8320;
            } else {
                crc >>= 1;
            }
        }
    }
    !crc
}

/// Compute SHA-256 hexadecimal digest for a byte slice.
pub fn compute_sha256(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let result = hasher.finalize();
    let mut out = String::with_capacity(64);
    for b in result {
        use std::fmt::Write;
        let _ = write!(out, "{:02x}", b);
    }
    out
}

/// Helper to serialize a valid binary MRS structure for testing or packaging.
pub fn build_mrs_bytes(
    behavior: Behavior,
    version: u8,
    rule_count: u32,
    description: &str,
    payload: &[u8],
    magic: Option<[u8; 4]>,
) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&magic.unwrap_or(MAGIC_STANDARD_MRS));
    bytes.push(version);
    bytes.push(behavior.to_u8());
    bytes.extend_from_slice(&rule_count.to_le_bytes());
    bytes.extend_from_slice(&(payload.len() as u32).to_le_bytes());

    let desc_bytes = description.as_bytes();
    bytes.extend_from_slice(&(desc_bytes.len() as u16).to_le_bytes());
    bytes.extend_from_slice(desc_bytes);
    bytes.extend_from_slice(payload);
    bytes
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_valid_header(desc: &str) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&MAGIC_LEGACY_MRS_V1);
        bytes.push(1); // Version
        bytes.push(0); // Behavior (Domain)
        bytes.extend_from_slice(&100u32.to_le_bytes()); // Rule count
        bytes.extend_from_slice(&2048u32.to_le_bytes()); // Payload size

        let desc_bytes = desc.as_bytes();
        bytes.extend_from_slice(&(desc_bytes.len() as u16).to_le_bytes());
        bytes.extend_from_slice(desc_bytes);
        bytes
    }

    #[test]
    fn test_parse_valid() {
        let bytes = create_valid_header("Test ruleset");
        let meta = parse_mrs_header(&bytes).unwrap();
        assert_eq!(meta.version, 1);
        assert_eq!(meta.behavior, Behavior::Domain);
        assert_eq!(meta.rule_count, 100);
        assert_eq!(meta.payload_size, 2048);
        assert_eq!(meta.description, "Test ruleset");
    }

    #[test]
    fn test_parse_standard_mrs_magic() {
        let payload = b"example.com\ngoogle.com\n";
        let bytes = build_mrs_bytes(
            Behavior::Domain,
            1,
            2,
            "Standard MRS",
            payload,
            Some(MAGIC_STANDARD_MRS),
        );
        let meta = parse_mrs_header(&bytes).unwrap();
        assert_eq!(meta.behavior, Behavior::Domain);
        assert_eq!(meta.rule_count, 2);
        assert_eq!(meta.payload_size, payload.len() as u32);
        assert_eq!(meta.description, "Standard MRS");
    }

    #[test]
    fn test_parse_invalid_magic() {
        let mut bytes = create_valid_header("Test");
        bytes[0] = b'X';
        assert!(parse_mrs_header(&bytes).is_err());
    }

    #[test]
    fn test_validate_valid() {
        let payload = vec![0u8; 100];
        let bytes = build_mrs_bytes(
            Behavior::IpCidr,
            2,
            50,
            "IP CIDR list",
            &payload,
            Some(MAGIC_STANDARD_MRS),
        );
        let report = validate_mrs_bytes(&bytes).unwrap();
        assert!(report.is_valid);
        assert!(report.errors.is_empty());
        assert_eq!(report.metadata.unwrap().rule_count, 50);
        assert!(report.sha256_digest.is_some());
        assert!(report.crc32_checksum.is_some());
    }

    #[test]
    fn test_validate_invalid_short() {
        let mut bytes = create_valid_header("Test");
        bytes.truncate(10);
        let report = validate_mrs_bytes(&bytes).unwrap();
        assert!(!report.is_valid);
        assert_eq!(report.errors.len(), 1);
        assert_eq!(report.errors[0], "Header too short");
    }

    #[test]
    fn test_validate_truncated_payload() {
        let bytes = create_valid_header("Test");
        // Payload size is 2048 in header, but bytes end immediately after description
        let report = validate_mrs_bytes(&bytes).unwrap();
        assert!(!report.is_valid);
        assert!(
            report
                .errors
                .iter()
                .any(|e| e.contains("Payload truncated"))
        );
    }

    #[test]
    fn test_compression_detection() {
        let zstd_payload = [0x28, 0xB5, 0x2F, 0xFD, 0x00, 0x01];
        let bytes = build_mrs_bytes(
            Behavior::Classical,
            1,
            10,
            "Zstd rules",
            &zstd_payload,
            None,
        );
        let header = parse_mrs_header_extended(&bytes).unwrap();
        assert_eq!(header.compression, CompressionType::Zstd);

        let gzip_payload = [0x1F, 0x8B, 0x08, 0x00];
        let gzip_bytes = build_mrs_bytes(
            Behavior::Classical,
            1,
            10,
            "Gzip rules",
            &gzip_payload,
            None,
        );
        let gzip_header = parse_mrs_header_extended(&gzip_bytes).unwrap();
        assert_eq!(gzip_header.compression, CompressionType::Gzip);
    }

    #[test]
    fn test_crc32_and_sha256_verification() {
        let data = b"music-frog-test-payload";
        let crc = compute_crc32(data);
        let sha = compute_sha256(data);

        assert!(verify_mrs_integrity(data, Some(&sha), Some(crc)).unwrap());
        assert!(!verify_mrs_integrity(data, Some("wrongsha256"), Some(crc)).unwrap());
        assert!(!verify_mrs_integrity(data, Some(&sha), Some(crc + 1)).unwrap());
    }

    #[test]
    fn test_behavior_conversions() {
        assert_eq!(Behavior::from_u8(0), Behavior::Domain);
        assert_eq!(Behavior::from_u8(1), Behavior::IpCidr);
        assert_eq!(Behavior::from_u8(2), Behavior::Classical);
        assert_eq!(Behavior::from_u8(99), Behavior::Unknown(99));

        assert_eq!(Behavior::parse_behavior_str("domain"), Behavior::Domain);
        assert_eq!(Behavior::parse_behavior_str("ipcidr"), Behavior::IpCidr);
        assert_eq!(Behavior::parse_behavior_str("IP-CIDR"), Behavior::IpCidr);
        assert_eq!(
            Behavior::parse_behavior_str("classical"),
            Behavior::Classical
        );
        assert_eq!(
            Behavior::parse_behavior_str("other"),
            Behavior::Unknown(255)
        );

        assert_eq!(Behavior::Domain.as_str(), "domain");
        assert_eq!(Behavior::IpCidr.as_str(), "ipcidr");
        assert_eq!(Behavior::Classical.as_str(), "classical");
        assert_eq!(Behavior::Unknown(10).as_str(), "unknown");
    }

    #[test]
    fn test_compression_type_conversions() {
        assert_eq!(CompressionType::from_u8(0), CompressionType::None);
        assert_eq!(CompressionType::from_u8(1), CompressionType::Zstd);
        assert_eq!(CompressionType::from_u8(2), CompressionType::Gzip);
        assert_eq!(CompressionType::from_u8(99), CompressionType::Unknown(99));

        assert_eq!(CompressionType::None.to_u8(), 0);
        assert_eq!(CompressionType::Zstd.to_u8(), 1);
        assert_eq!(CompressionType::Gzip.to_u8(), 2);
        assert_eq!(CompressionType::Unknown(99).to_u8(), 99);

        assert_eq!(CompressionType::None.as_str(), "none");
        assert_eq!(CompressionType::Zstd.as_str(), "zstd");
        assert_eq!(CompressionType::Gzip.as_str(), "gzip");
        assert_eq!(CompressionType::Unknown(99).as_str(), "unknown");
    }

    #[test]
    fn test_invalid_utf8_description() {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&MAGIC_LEGACY_MRS_V1);
        bytes.push(1); // Version
        bytes.push(0); // Behavior
        bytes.extend_from_slice(&10u32.to_le_bytes()); // Rule count
        bytes.extend_from_slice(&0u32.to_le_bytes()); // Payload size
        bytes.extend_from_slice(&2u16.to_le_bytes()); // Desc len = 2
        bytes.push(0xFF); // Invalid UTF-8
        bytes.push(0xFE);

        assert!(parse_mrs_header(&bytes).is_err());
        let report = validate_mrs_bytes(&bytes).unwrap();
        assert!(!report.is_valid);
        assert!(
            report
                .errors
                .iter()
                .any(|e| e.contains("Invalid UTF-8 in description"))
        );
    }
}
