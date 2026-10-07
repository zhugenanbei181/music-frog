//! Pure document identity used at reader, command and host transaction boundaries.
use crate::profile_source::hash_document_bytes;
use infiltrator_contract::rule_source::RuleSourceIdentity;
pub fn identify_rules_document(profile: String, content: &str) -> RuleSourceIdentity {
    RuleSourceIdentity {
        profile,
        document_hash: hash_document_bytes(content),
    }
}
