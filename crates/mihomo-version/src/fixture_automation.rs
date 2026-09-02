use crate::capability::MihomoCapability;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct CapabilityDiff {
    pub added_features: Vec<String>,
    pub removed_features: Vec<String>,
    pub is_breaking_change: bool,
}

pub struct VersionFixtureGenerator;

impl VersionFixtureGenerator {
    pub fn generate_fixture_for_version(version_tag: &str) -> MihomoCapability {
        crate::capability::capability_snapshot(version_tag)
            .unwrap_or_else(|_| panic!("Failed to generate fixture for version: {}", version_tag))
    }

    pub fn generate_known_version_fixtures() -> Vec<(String, MihomoCapability)> {
        vec![
            (
                "v1.18.0".to_string(),
                Self::generate_fixture_for_version("v1.18.0"),
            ),
            (
                "v1.19.0".to_string(),
                Self::generate_fixture_for_version("v1.19.0"),
            ),
            (
                "Alpha-v1.19.2".to_string(),
                Self::generate_fixture_for_version("Alpha-v1.19.2"),
            ),
            (
                "Nightly".to_string(),
                Self::generate_fixture_for_version("Nightly"),
            ),
        ]
    }

    pub fn diff_capabilities(from: &MihomoCapability, to: &MihomoCapability) -> CapabilityDiff {
        let from_caps: std::collections::HashSet<_> = from
            .capabilities
            .iter()
            .map(|c| c.name().to_string())
            .collect();
        let to_caps: std::collections::HashSet<_> = to
            .capabilities
            .iter()
            .map(|c| c.name().to_string())
            .collect();

        let mut added_features: Vec<String> = to_caps.difference(&from_caps).cloned().collect();
        let mut removed_features: Vec<String> = from_caps.difference(&to_caps).cloned().collect();

        added_features.sort();
        removed_features.sort();

        let is_breaking_change = !removed_features.is_empty();

        CapabilityDiff {
            added_features,
            removed_features,
            is_breaking_change,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_fixture_standard_tags() {
        let v1_18 = VersionFixtureGenerator::generate_fixture_for_version("v1.18.0");
        assert_eq!(v1_18.core, Some([1, 18, 0]));

        let v1_19 = VersionFixtureGenerator::generate_fixture_for_version("v1.19.0");
        assert_eq!(v1_19.core, Some([1, 19, 0]));

        let alpha = VersionFixtureGenerator::generate_fixture_for_version("Alpha-v1.19.2");
        assert_eq!(alpha.core, None); // Since parsing fallback treats this as no numeric core unless starting with "v1.19.2"
        assert!(alpha.assumed_latest);
    }

    #[test]
    fn test_diff_capabilities_added() {
        let from = VersionFixtureGenerator::generate_fixture_for_version("v1.18.0");
        let to = VersionFixtureGenerator::generate_fixture_for_version("v1.19.0");

        let diff = VersionFixtureGenerator::diff_capabilities(&from, &to);

        assert!(!diff.is_breaking_change);
        assert!(diff.removed_features.is_empty());
        assert!(diff.added_features.contains(&"mrs_rule_set".to_string()));
        assert!(
            diff.added_features
                .contains(&"wire_guard_outbound".to_string())
        );
    }

    #[test]
    fn test_diff_capabilities_breaking() {
        let from = VersionFixtureGenerator::generate_fixture_for_version("v1.19.0");
        let to = VersionFixtureGenerator::generate_fixture_for_version("v1.18.0");

        let diff = VersionFixtureGenerator::diff_capabilities(&from, &to);

        assert!(diff.is_breaking_change);
        assert!(!diff.removed_features.is_empty());
        assert!(diff.added_features.is_empty());
        assert!(diff.removed_features.contains(&"mrs_rule_set".to_string()));
        assert!(
            diff.removed_features
                .contains(&"wire_guard_outbound".to_string())
        );
    }
}
