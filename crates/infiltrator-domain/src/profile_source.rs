//! Byte identities are folded once for all profile readers and commit adapters.
use infiltrator_contract::profile_source::ProfileSourceIdentity;
use sha2::{Digest, Sha256};

pub fn identify_profile_source(
    profile: String,
    content: &str,
    options: Option<&str>,
) -> ProfileSourceIdentity {
    ProfileSourceIdentity {
        profile,
        document_hash: hash_document_bytes(content),
        options_hash: options.map(hash_document_bytes),
    }
}

pub fn hash_document_bytes(text: &str) -> String {
    hash_bytes(text.as_bytes())
}

pub fn hash_bytes(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}
