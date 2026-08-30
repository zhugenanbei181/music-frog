use anyhow::{bail, Context, Result};
use std::convert::TryInto;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Behavior {
    Domain,
    IpCidr,
    Classical,
    Unknown(u8),
}

#[derive(Debug, Clone)]
pub struct MrsMetadata {
    pub behavior: Behavior,
    pub rule_count: u32,
    pub version: u8,
    pub payload_size: u32,
    pub description: String,
}

#[derive(Debug, Clone)]
pub struct MrsValidationReport {
    pub is_valid: bool,
    pub errors: Vec<String>,
}

const MAGIC_HEADER: &[u8] = b"MRS\x01";

pub fn parse_mrs_header(bytes: &[u8]) -> Result<MrsMetadata> {
    if bytes.len() < 4 || &bytes[0..4] != MAGIC_HEADER {
        bail!("Invalid magic header");
    }

    if bytes.len() < 16 {
        bail!("Header too short");
    }

    let version = bytes[4];
    
    let behavior = match bytes[5] {
        0 => Behavior::Domain,
        1 => Behavior::IpCidr,
        2 => Behavior::Classical,
        x => Behavior::Unknown(x),
    };

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

pub fn validate_mrs_bytes(bytes: &[u8]) -> Result<MrsValidationReport> {
    let mut errors = Vec::new();
    
    if bytes.len() < 4 || &bytes[0..4] != MAGIC_HEADER {
        errors.push("Invalid magic header".to_string());
        return Ok(MrsValidationReport { is_valid: false, errors });
    }

    if bytes.len() < 16 {
        errors.push("Header too short".to_string());
        return Ok(MrsValidationReport { is_valid: false, errors });
    }

    let desc_len = u16::from_le_bytes(bytes[14..16].try_into().unwrap()) as usize;
    if bytes.len() < 16 + desc_len {
        errors.push("Incomplete description".to_string());
    } else if std::str::from_utf8(&bytes[16..16 + desc_len]).is_err() {
        errors.push("Invalid UTF-8 in description".to_string());
    }

    let is_valid = errors.is_empty();
    Ok(MrsValidationReport { is_valid, errors })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_valid_header(desc: &str) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(MAGIC_HEADER);
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
    fn test_parse_invalid_magic() {
        let mut bytes = create_valid_header("Test");
        bytes[0] = b'X';
        assert!(parse_mrs_header(&bytes).is_err());
    }

    #[test]
    fn test_validate_valid() {
        let bytes = create_valid_header("Test");
        let report = validate_mrs_bytes(&bytes).unwrap();
        assert!(report.is_valid);
        assert!(report.errors.is_empty());
    }

    #[test]
    fn test_validate_invalid() {
        let mut bytes = create_valid_header("Test");
        bytes.truncate(10);
        let report = validate_mrs_bytes(&bytes).unwrap();
        assert!(!report.is_valid);
        assert_eq!(report.errors.len(), 1);
        assert_eq!(report.errors[0], "Header too short");
    }
}
