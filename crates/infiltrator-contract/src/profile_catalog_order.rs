//! Snapshot-fenced ordering for an engine-scoped profile catalog.

use lurker_core_contract::identity::EngineId;
use lurker_core_contract::profile_document_identity::ProfileId;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::error;
use std::fmt::{self, Display, Formatter};

/// Complete ordered identity list observed from one engine's profile catalog.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProfileCatalogSnapshot {
    pub engine_id: EngineId,
    pub ordered_profile_ids: Vec<ProfileId>,
}

impl ProfileCatalogSnapshot {
    pub fn new(engine_id: EngineId, ordered_profile_ids: Vec<ProfileId>) -> Self {
        Self {
            engine_id,
            ordered_profile_ids,
        }
    }
}

/// A complete target permutation bound to the exact catalog order the user
/// observed. The profile-store owner rechecks this snapshot under its write
/// fence before publishing the target.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProfileCatalogOrderRequest {
    pub expected: ProfileCatalogSnapshot,
    pub target_order: Vec<ProfileId>,
}

impl ProfileCatalogOrderRequest {
    pub fn new(expected: ProfileCatalogSnapshot, target_order: Vec<ProfileId>) -> Self {
        Self {
            expected,
            target_order,
        }
    }

    pub fn validate_for_engine(
        &self,
        engine_id: &EngineId,
    ) -> Result<(), ProfileCatalogOrderValidationError> {
        if &self.expected.engine_id != engine_id {
            return Err(ProfileCatalogOrderValidationError::ForeignEngine {
                request_engine: self.expected.engine_id.clone(),
                owner_engine: engine_id.clone(),
            });
        }

        let mut expected_ids = HashSet::with_capacity(self.expected.ordered_profile_ids.len());
        for profile_id in &self.expected.ordered_profile_ids {
            if !expected_ids.insert(profile_id.as_str().to_owned()) {
                return Err(
                    ProfileCatalogOrderValidationError::DuplicateExpectedProfile(
                        profile_id.clone(),
                    ),
                );
            }
        }

        let mut target_ids = HashSet::with_capacity(self.target_order.len());
        for profile_id in &self.target_order {
            if !target_ids.insert(profile_id.as_str().to_owned()) {
                return Err(ProfileCatalogOrderValidationError::DuplicateTargetProfile(
                    profile_id.clone(),
                ));
            }
        }

        for profile_id in &self.target_order {
            if !expected_ids.contains(profile_id.as_str()) {
                return Err(ProfileCatalogOrderValidationError::ForeignTargetProfile(
                    profile_id.clone(),
                ));
            }
        }

        for profile_id in &self.expected.ordered_profile_ids {
            if !target_ids.contains(profile_id.as_str()) {
                return Err(ProfileCatalogOrderValidationError::MissingTargetProfile(
                    profile_id.clone(),
                ));
            }
        }

        Ok(())
    }
}

/// Result of a compare-and-set catalog order write.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum ProfileCatalogOrderMutation {
    Unchanged {
        current: ProfileCatalogSnapshot,
    },
    Applied {
        previous: ProfileCatalogSnapshot,
        current: ProfileCatalogSnapshot,
    },
    Stale {
        expected: ProfileCatalogSnapshot,
        observed: ProfileCatalogSnapshot,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "reason", content = "details", rename_all = "snake_case")]
pub enum ProfileCatalogOrderValidationError {
    ForeignEngine {
        request_engine: EngineId,
        owner_engine: EngineId,
    },
    DuplicateExpectedProfile(ProfileId),
    DuplicateTargetProfile(ProfileId),
    MissingTargetProfile(ProfileId),
    ForeignTargetProfile(ProfileId),
}

impl Display for ProfileCatalogOrderValidationError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::ForeignEngine {
                request_engine,
                owner_engine,
            } => write!(
                formatter,
                "catalog order belongs to engine `{}`, not `{}`",
                request_engine.as_str(),
                owner_engine.as_str()
            ),
            Self::DuplicateExpectedProfile(profile_id) => write!(
                formatter,
                "expected catalog repeats profile `{}`",
                profile_id.as_str()
            ),
            Self::DuplicateTargetProfile(profile_id) => write!(
                formatter,
                "target catalog repeats profile `{}`",
                profile_id.as_str()
            ),
            Self::MissingTargetProfile(profile_id) => write!(
                formatter,
                "target catalog omits profile `{}`",
                profile_id.as_str()
            ),
            Self::ForeignTargetProfile(profile_id) => write!(
                formatter,
                "target catalog contains unknown profile `{}`",
                profile_id.as_str()
            ),
        }
    }
}

impl error::Error for ProfileCatalogOrderValidationError {}

#[cfg(test)]
#[path = "profile_catalog_order_tests.rs"]
mod tests;
