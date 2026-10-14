//! Stable profile identity and its independently editable display label.

use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::error;
use std::fmt::{self, Display, Formatter};

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ProfileDisplayName(String);

impl ProfileDisplayName {
    pub fn new(value: impl Into<String>) -> Result<Self, ProfileDisplayNameError> {
        let value = value.into();
        if value.trim().is_empty() {
            return Err(ProfileDisplayNameError::Empty);
        }
        if value.chars().count() > 128 {
            return Err(ProfileDisplayNameError::TooLong);
        }
        if value.trim() != value {
            return Err(ProfileDisplayNameError::EdgeWhitespace);
        }
        if value.chars().any(char::is_control) {
            return Err(ProfileDisplayNameError::ControlCharacter);
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn into_string(self) -> String {
        self.0
    }
}

impl Display for ProfileDisplayName {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl Serialize for ProfileDisplayName {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for ProfileDisplayName {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::new(value).map_err(D::Error::custom)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProfileDisplayNameError {
    Empty,
    TooLong,
    EdgeWhitespace,
    ControlCharacter,
}

impl Display for ProfileDisplayNameError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => formatter.write_str("profile display name must not be empty"),
            Self::TooLong => formatter.write_str("profile display name exceeds 128 characters"),
            Self::EdgeWhitespace => {
                formatter.write_str("profile display name must not have edge whitespace")
            }
            Self::ControlCharacter => {
                formatter.write_str("profile display name must not contain control characters")
            }
        }
    }
}

impl error::Error for ProfileDisplayNameError {}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProfileDisplayNameTarget {
    Identity,
    Custom(ProfileDisplayName),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProfileDisplayNameObservation {
    Identity,
    Custom(ProfileDisplayName),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum ProfileDisplayNameMutation {
    Unchanged {
        current: ProfileDisplayNameObservation,
    },
    Applied {
        previous: ProfileDisplayNameObservation,
        current: ProfileDisplayNameObservation,
    },
}

#[cfg(test)]
#[path = "profile_display_name_tests.rs"]
mod tests;
