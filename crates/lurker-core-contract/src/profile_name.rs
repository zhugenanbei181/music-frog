//! Engine-neutral profile-name validation.
//!
//! Both engine stores and every surface must agree on which names are portable
//! across filesystems, so the rule lives in the neutral contract.

use std::error::Error;
use std::fmt::{self, Display, Formatter};

/// Why a profile name was rejected as a portable storage key.
///
/// The variants are typed so a surface can explain the exact rule that was
/// broken instead of showing a generic "invalid name" message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProfileNameError {
    /// The name was empty.
    Empty,
    /// The name was longer than 128 characters.
    TooLong,
    /// The name started or ended with whitespace.
    EdgeWhitespace,
    /// The name contained a relative path segment such as `..`.
    RelativeSegment,
    /// The name contained a character that is illegal on a target filesystem.
    IllegalCharacter(char),
    /// The name ended with a dot or a space, which Windows silently drops.
    TrailingWindowsCharacter,
}

impl Display for ProfileNameError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => formatter.write_str("配置名称不能为空"),
            Self::TooLong => formatter.write_str("配置名称不能超过 128 个字符"),
            Self::EdgeWhitespace => formatter.write_str("配置名称首尾不能包含空白"),
            Self::RelativeSegment => formatter.write_str("配置名称不能包含相对路径片段"),
            Self::IllegalCharacter(character) => {
                write!(formatter, "配置名称不能包含特殊字符 {character:?}")
            }
            Self::TrailingWindowsCharacter => formatter.write_str("配置名称不能以点或空格结尾"),
        }
    }
}

impl Error for ProfileNameError {}

/// Validate one portable profile name before using it as a storage key or
/// filesystem component.
pub fn validate_profile_name(name: &str) -> Result<(), ProfileNameError> {
    if name.is_empty() {
        return Err(ProfileNameError::Empty);
    }
    if name.chars().count() > 128 {
        return Err(ProfileNameError::TooLong);
    }
    if name.trim() != name {
        return Err(ProfileNameError::EdgeWhitespace);
    }
    if name == "." || name == ".." || name.contains("..") {
        return Err(ProfileNameError::RelativeSegment);
    }
    if let Some(character) = name.chars().find(|character| {
        matches!(
            character,
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|'
        ) || character.is_control()
    }) {
        return Err(ProfileNameError::IllegalCharacter(character));
    }
    if name.ends_with('.') || name.ends_with(' ') {
        return Err(ProfileNameError::TrailingWindowsCharacter);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{ProfileNameError, validate_profile_name};

    #[test]
    fn names_are_portable_across_engine_file_stores() {
        for invalid in ["release..backup", "trailing.", "trailing ", "a?b"] {
            assert!(validate_profile_name(invalid).is_err(), "{invalid:?}");
        }
        assert!(validate_profile_name("regional.v1").is_ok());
    }

    #[test]
    fn path_like_names_are_rejected() {
        assert_eq!(
            validate_profile_name("invalid/name"),
            Err(ProfileNameError::IllegalCharacter('/'))
        );
        assert_eq!(
            validate_profile_name("invalid\\name"),
            Err(ProfileNameError::IllegalCharacter('\\'))
        );
    }

    #[test]
    fn empty_and_overlong_names_are_rejected() {
        assert_eq!(validate_profile_name(""), Err(ProfileNameError::Empty));
        assert_eq!(
            validate_profile_name(&"a".repeat(129)),
            Err(ProfileNameError::TooLong)
        );
        assert_eq!(
            validate_profile_name(" padded"),
            Err(ProfileNameError::EdgeWhitespace)
        );
        assert_eq!(
            validate_profile_name(".."),
            Err(ProfileNameError::RelativeSegment)
        );
    }
}
