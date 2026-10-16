//! Shared domain model and typed operations for Named YAML Overrides (SLINT-004).
//!
//! Named Overrides provide modular, reusable YAML configuration snippets that can be
//! created, edited, imported, exported, ordered, and bound/unbound per profile.
//!
//! Persistence is owned by the shared profile/config owner, and multi-profile isolation,
//! revision CAS, and validation ensure neither Slint nor any other UI holds a private
//! secondary repository.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};
use std::error::Error;
use std::fmt;

/// Validated unique identifier for a named override.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct NamedOverrideId(String);

impl NamedOverrideId {
    pub fn new(id: impl Into<String>) -> Result<Self, NamedOverrideError> {
        let id = id.into();
        let trimmed = id.trim();
        if trimmed.is_empty() {
            return Err(NamedOverrideError::EmptyId);
        }
        if trimmed.len() > 64 {
            return Err(NamedOverrideError::IdTooLong);
        }
        if !trimmed.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
            return Err(NamedOverrideError::InvalidIdCharacters);
        }
        Ok(Self(trimmed.to_lowercase()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for NamedOverrideId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NamedOverrideError {
    EmptyId,
    IdTooLong,
    InvalidIdCharacters,
    EmptyName,
    NameTooLong,
    InvalidYaml(String),
}

impl fmt::Display for NamedOverrideError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyId => write!(f, "override ID must not be empty"),
            Self::IdTooLong => write!(f, "override ID exceeds 64 characters"),
            Self::InvalidIdCharacters => write!(f, "override ID may only contain letters, numbers, dashes, and underscores"),
            Self::EmptyName => write!(f, "override name must not be empty"),
            Self::NameTooLong => write!(f, "override name exceeds 128 characters"),
            Self::InvalidYaml(err) => write!(f, "invalid YAML content: {err}"),
        }
    }
}

impl Error for NamedOverrideError {}

/// One named YAML override definition.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NamedOverride {
    pub id: NamedOverrideId,
    pub name: String,
    pub content: String,
    pub updated_at: String,
}

impl NamedOverride {
    pub fn new(
        id: NamedOverrideId,
        name: impl Into<String>,
        content: impl Into<String>,
        updated_at: impl Into<String>,
    ) -> Result<Self, NamedOverrideError> {
        let name = name.into();
        let trimmed_name = name.trim();
        if trimmed_name.is_empty() {
            return Err(NamedOverrideError::EmptyName);
        }
        if trimmed_name.len() > 128 {
            return Err(NamedOverrideError::NameTooLong);
        }
        let content = content.into();
        Ok(Self {
            id,
            name: trimmed_name.to_owned(),
            content,
            updated_at: updated_at.into(),
        })
    }
}

/// Binding of a named override to a specific profile.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProfileOverrideBinding {
    pub override_id: NamedOverrideId,
    pub enabled: bool,
    pub order: usize,
}

/// Complete catalog of named overrides and their per-profile bindings.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct NamedOverrideCatalog {
    pub overrides: Vec<NamedOverride>,
    pub profile_bindings: BTreeMap<String, Vec<ProfileOverrideBinding>>,
    pub revision: u64,
}

impl NamedOverrideCatalog {
    pub fn new() -> Self {
        Self::default()
    }

    /// Retrieve an override by ID.
    pub fn get_override(&self, id: &NamedOverrideId) -> Option<&NamedOverride> {
        self.overrides.iter().find(|o| &o.id == id)
    }

    /// Count how many profiles currently bind this override.
    pub fn bound_profile_count(&self, id: &NamedOverrideId) -> usize {
        self.profile_bindings
            .values()
            .filter(|bindings| bindings.iter().any(|b| &b.override_id == id && b.enabled))
            .count()
    }

    /// Retrieve ordered bindings for a given profile.
    pub fn bindings_for_profile(&self, profile_id: &str) -> Vec<&ProfileOverrideBinding> {
        let mut bindings: Vec<&ProfileOverrideBinding> = self
            .profile_bindings
            .get(profile_id)
            .map(|list| list.iter().collect())
            .unwrap_or_default();
        bindings.sort_by_key(|b| b.order);
        bindings
    }

    /// Upsert an override with CAS revision check.
    pub fn upsert_override(
        &mut self,
        expected_revision: u64,
        override_item: NamedOverride,
    ) -> Result<u64, NamedOverrideMutationError> {
        if self.revision != expected_revision {
            return Err(NamedOverrideMutationError::StaleRevision {
                expected: expected_revision,
                actual: self.revision,
            });
        }
        if let Some(existing) = self.overrides.iter_mut().find(|o| o.id == override_item.id) {
            *existing = override_item;
        } else {
            self.overrides.push(override_item);
        }
        self.revision = self.revision.wrapping_add(1);
        Ok(self.revision)
    }

    /// Delete an override with CAS revision check and remove all its profile bindings.
    pub fn delete_override(
        &mut self,
        expected_revision: u64,
        id: &NamedOverrideId,
    ) -> Result<u64, NamedOverrideMutationError> {
        if self.revision != expected_revision {
            return Err(NamedOverrideMutationError::StaleRevision {
                expected: expected_revision,
                actual: self.revision,
            });
        }
        let pos = self
            .overrides
            .iter()
            .position(|o| &o.id == id)
            .ok_or_else(|| NamedOverrideMutationError::NotFound(id.clone()))?;
        self.overrides.remove(pos);

        // Clean up bindings across all profiles
        for bindings in self.profile_bindings.values_mut() {
            bindings.retain(|b| &b.override_id != id);
            for (idx, b) in bindings.iter_mut().enumerate() {
                b.order = idx;
            }
        }

        self.revision = self.revision.wrapping_add(1);
        Ok(self.revision)
    }

    /// Toggle or set a profile's binding to an override.
    pub fn set_profile_binding(
        &mut self,
        expected_revision: u64,
        profile_id: &str,
        override_id: &NamedOverrideId,
        enabled: bool,
    ) -> Result<u64, NamedOverrideMutationError> {
        if self.revision != expected_revision {
            return Err(NamedOverrideMutationError::StaleRevision {
                expected: expected_revision,
                actual: self.revision,
            });
        }
        if !self.overrides.iter().any(|o| &o.id == override_id) {
            return Err(NamedOverrideMutationError::NotFound(override_id.clone()));
        }

        let bindings = self
            .profile_bindings
            .entry(profile_id.to_string())
            .or_default();
        if let Some(b) = bindings.iter_mut().find(|b| &b.override_id == override_id) {
            b.enabled = enabled;
        } else {
            let next_order = bindings.len();
            bindings.push(ProfileOverrideBinding {
                override_id: override_id.clone(),
                enabled,
                order: next_order,
            });
        }

        self.revision = self.revision.wrapping_add(1);
        Ok(self.revision)
    }

    /// Reorder a profile's bindings using a complete target list.
    pub fn reorder_profile_bindings(
        &mut self,
        expected_revision: u64,
        profile_id: &str,
        target_order: &[NamedOverrideId],
    ) -> Result<u64, NamedOverrideMutationError> {
        if self.revision != expected_revision {
            return Err(NamedOverrideMutationError::StaleRevision {
                expected: expected_revision,
                actual: self.revision,
            });
        }
        let bindings = self
            .profile_bindings
            .get_mut(profile_id)
            .ok_or_else(|| NamedOverrideMutationError::ProfileNotFound(profile_id.to_string()))?;

        let existing_set: HashSet<NamedOverrideId> = bindings.iter().map(|b| b.override_id.clone()).collect();
        let target_set: HashSet<NamedOverrideId> = target_order.iter().cloned().collect();

        if existing_set != target_set {
            return Err(NamedOverrideMutationError::InvalidReorderPermutation);
        }

        for (new_order, id) in target_order.iter().enumerate() {
            if let Some(b) = bindings.iter_mut().find(|b| &b.override_id == id) {
                b.order = new_order;
            }
        }
        bindings.sort_by_key(|b| b.order);

        self.revision = self.revision.wrapping_add(1);
        Ok(self.revision)
    }

    /// Compose all enabled overrides for a profile in order into a single YAML string.
    pub fn compose_overrides_for_profile(&self, profile_id: &str) -> String {
        let bindings = self.bindings_for_profile(profile_id);
        let mut composed = String::new();
        for b in bindings {
            if !b.enabled {
                continue;
            }
            if let Some(item) = self.get_override(&b.override_id) {
                let trimmed = item.content.trim();
                if !trimmed.is_empty() {
                    if !composed.is_empty() {
                        composed.push_str("\n---\n");
                    }
                    composed.push_str(trimmed);
                }
            }
        }
        composed
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NamedOverrideMutationError {
    StaleRevision { expected: u64, actual: u64 },
    NotFound(NamedOverrideId),
    ProfileNotFound(String),
    InvalidReorderPermutation,
}

impl fmt::Display for NamedOverrideMutationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::StaleRevision { expected, actual } => {
                write!(f, "stale revision: expected {expected}, actual {actual}")
            }
            Self::NotFound(id) => write!(f, "override not found: {id}"),
            Self::ProfileNotFound(profile) => write!(f, "profile not found: {profile}"),
            Self::InvalidReorderPermutation => write!(f, "invalid reorder permutation"),
        }
    }
}

impl Error for NamedOverrideMutationError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn override_id_validation() {
        assert!(NamedOverrideId::new("valid-id_1").is_ok());
        assert_eq!(NamedOverrideId::new("My_Override").unwrap().as_str(), "my_override");
        assert!(NamedOverrideId::new("").is_err());
        assert!(NamedOverrideId::new("invalid id with spaces").is_err());
        assert!(NamedOverrideId::new("invalid$special").is_err());
    }

    #[test]
    fn override_creation_and_validation() {
        let id = NamedOverrideId::new("dns-tweak").unwrap();
        assert!(NamedOverride::new(id.clone(), "DNS Tweak", "dns:\n  enable: true\n", "2026-10-15").is_ok());

        // Empty name rejected
        assert_eq!(
            NamedOverride::new(id, "   ", "dns: true", "2026-10-15").unwrap_err(),
            NamedOverrideError::EmptyName
        );
    }

    #[test]
    fn catalog_crud_cas_and_profile_isolation() {
        let mut catalog = NamedOverrideCatalog::new();
        assert_eq!(catalog.revision, 0);

        let id1 = NamedOverrideId::new("ov1").unwrap();
        let ov1 = NamedOverride::new(id1.clone(), "Override 1", "mode: rule", "2026-10-15").unwrap();

        // Stale revision rejected
        assert!(matches!(
            catalog.upsert_override(1, ov1.clone()),
            Err(NamedOverrideMutationError::StaleRevision { .. })
        ));

        // Successful insert at revision 0 advances to 1
        let rev1 = catalog.upsert_override(0, ov1).unwrap();
        assert_eq!(rev1, 1);
        assert_eq!(catalog.overrides.len(), 1);

        // Bind to profile "alpha" and "beta"
        catalog.set_profile_binding(1, "alpha", &id1, true).unwrap();
        catalog.set_profile_binding(2, "beta", &id1, false).unwrap();
        assert_eq!(catalog.bound_profile_count(&id1), 1); // Only alpha is enabled

        let id2 = NamedOverrideId::new("ov2").unwrap();
        let ov2 = NamedOverride::new(id2.clone(), "Override 2", "ipv6: false", "2026-10-15").unwrap();
        catalog.upsert_override(3, ov2).unwrap();
        catalog.set_profile_binding(4, "alpha", &id2, true).unwrap();

        // Alpha has both ov1 and ov2 enabled; composition merges them
        let composed = catalog.compose_overrides_for_profile("alpha");
        assert!(composed.contains("mode: rule"));
        assert!(composed.contains("ipv6: false"));

        // Reordering alpha's overrides
        catalog.reorder_profile_bindings(5, "alpha", &[id2.clone(), id1.clone()]).unwrap();
        let alpha_bindings = catalog.bindings_for_profile("alpha");
        assert_eq!(alpha_bindings[0].override_id, id2);
        assert_eq!(alpha_bindings[1].override_id, id1);

        // Beta bindings are untouched (profile isolation)
        let beta_bindings = catalog.bindings_for_profile("beta");
        assert_eq!(beta_bindings.len(), 1);
        assert_eq!(beta_bindings[0].override_id, id1);
        assert!(!beta_bindings[0].enabled);

        // Deleting ov1 cleans up bindings in both profiles
        catalog.delete_override(6, &id1).unwrap();
        assert_eq!(catalog.overrides.len(), 1);
        assert_eq!(catalog.bindings_for_profile("alpha").len(), 1);
        assert_eq!(catalog.bindings_for_profile("beta").len(), 0);
    }
}
