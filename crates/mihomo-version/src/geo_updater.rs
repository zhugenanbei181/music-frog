use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Represents the type of geographic data asset.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum GeoAssetType {
    GeoIp,
    GeoSite,
    Mmdb,
}

/// Metadata about a geo database to be updated.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GeoDatabaseInfo {
    pub asset_type: GeoAssetType,
    pub filename: String,
    pub expected_sha256: Option<String>,
    pub version_tag: Option<String>,
}

/// The result of a geo database update operation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GeoUpdateResult {
    pub asset_type: GeoAssetType,
    pub updated: bool,
    pub sha256: String,
    pub size_bytes: u64,
}

/// Utility for validating and updating geo databases.
pub struct GeoDatabaseUpdater;

impl GeoDatabaseUpdater {
    /// Computes the SHA256 of the given bytes and verifies it against an expected hash (if provided).
    pub fn verify_database_bytes(bytes: &[u8], expected_sha256: Option<&str>) -> Result<String> {
        let mut hasher = Sha256::new();
        hasher.update(bytes);
        
        let result = hasher.finalize();
        let mut computed_hash = String::with_capacity(result.len() * 2);
        for byte in result {
            computed_hash.push_str(&format!("{:02x}", byte));
        }

        if let Some(expected) = expected_sha256
            && !expected.eq_ignore_ascii_case(&computed_hash) {
                return Err(anyhow!(
                    "SHA256 mismatch. Expected: {}, Computed: {}",
                    expected,
                    computed_hash
                ));
            }

        Ok(computed_hash)
    }

    /// Gets the default filename for a given asset type.
    pub fn get_default_filename(asset_type: GeoAssetType) -> &'static str {
        match asset_type {
            GeoAssetType::GeoIp => "geoip.dat",
            GeoAssetType::GeoSite => "geosite.dat",
            GeoAssetType::Mmdb => "Country.mmdb",
        }
    }

    /// Checks whether the provided bytes look like a valid database file based on heuristics.
    pub fn is_valid_database_header(asset_type: GeoAssetType, bytes: &[u8]) -> bool {
        if bytes.is_empty() {
            return false;
        }

        match asset_type {
            GeoAssetType::GeoIp | GeoAssetType::GeoSite => {
                // V2Ray geodata files don't have a strict magic number at the beginning,
                // but they are typically serialized protobuf structures and generally > 16 bytes.
                bytes.len() > 16
            }
            GeoAssetType::Mmdb => {
                // MaxMind DBs are typically larger than a few bytes and contain a specific
                // magic sequence "\xab\xcd\xefMaxMind.com" somewhere near the end of the file.
                // For a quick sanity check, we ensure it meets a minimum size.
                bytes.len() > 32
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_verify_database_bytes_success() {
        let data = b"hello world";
        // SHA256 of "hello world"
        let expected_hash = "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9";
        
        let result = GeoDatabaseUpdater::verify_database_bytes(data, Some(expected_hash));
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), expected_hash);
    }

    #[test]
    fn test_verify_database_bytes_case_insensitive() {
        let data = b"hello world";
        let expected_hash = "B94D27B9934D3E08A52E52D7DA7DABFAC484EFE37A5380EE9088F7ACE2EFCDE9";
        
        let result = GeoDatabaseUpdater::verify_database_bytes(data, Some(expected_hash));
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), expected_hash.to_lowercase());
    }

    #[test]
    fn test_verify_database_bytes_mismatch() {
        let data = b"hello world";
        let expected_hash = "deadbeef934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9";
        
        let result = GeoDatabaseUpdater::verify_database_bytes(data, Some(expected_hash));
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            format!(
                "SHA256 mismatch. Expected: {}, Computed: b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9",
                expected_hash
            )
        );
    }

    #[test]
    fn test_verify_database_bytes_no_expected() {
        let data = b"test data";
        let result = GeoDatabaseUpdater::verify_database_bytes(data, None);
        assert!(result.is_ok());
        // SHA256 of "test data"
        assert_eq!(
            result.unwrap(),
            "916f0027a575074ce72a331777c3478d6513f786a591bd892da1a577bf2335f9"
        );
    }

    #[test]
    fn test_get_default_filename() {
        assert_eq!(GeoDatabaseUpdater::get_default_filename(GeoAssetType::GeoIp), "geoip.dat");
        assert_eq!(GeoDatabaseUpdater::get_default_filename(GeoAssetType::GeoSite), "geosite.dat");
        assert_eq!(GeoDatabaseUpdater::get_default_filename(GeoAssetType::Mmdb), "Country.mmdb");
    }

    #[test]
    fn test_is_valid_database_header() {
        let empty_data: &[u8] = &[];
        assert!(!GeoDatabaseUpdater::is_valid_database_header(GeoAssetType::GeoIp, empty_data));
        assert!(!GeoDatabaseUpdater::is_valid_database_header(GeoAssetType::GeoSite, empty_data));
        assert!(!GeoDatabaseUpdater::is_valid_database_header(GeoAssetType::Mmdb, empty_data));

        let small_data = b"small data"; // 10 bytes
        assert!(!GeoDatabaseUpdater::is_valid_database_header(GeoAssetType::GeoIp, small_data));
        assert!(!GeoDatabaseUpdater::is_valid_database_header(GeoAssetType::Mmdb, small_data));

        let valid_geodata = vec![0u8; 20];
        assert!(GeoDatabaseUpdater::is_valid_database_header(GeoAssetType::GeoIp, &valid_geodata));
        assert!(GeoDatabaseUpdater::is_valid_database_header(GeoAssetType::GeoSite, &valid_geodata));

        let invalid_mmdb = vec![0u8; 20];
        assert!(!GeoDatabaseUpdater::is_valid_database_header(GeoAssetType::Mmdb, &invalid_mmdb));

        let valid_mmdb = vec![0u8; 40];
        assert!(GeoDatabaseUpdater::is_valid_database_header(GeoAssetType::Mmdb, &valid_mmdb));
    }
}
