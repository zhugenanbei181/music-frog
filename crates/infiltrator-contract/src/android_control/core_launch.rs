//! Typed Android Core binary and process launch contract.

use crate::profile_document_identity::{ProfileDocumentRevision, ProfileId};
use lurker_core_contract::identity::{CoreVersion, EngineId};
use serde::de;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::{error, fmt};

/// Exact profile selected by the Rust owner for one Core process launch.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CoreLaunchConfigurationIdentity {
    pub engine_id: EngineId,
    pub profile_id: ProfileId,
    pub document_revision: ProfileDocumentRevision,
}

/// Android ABI target selected by the Rust build and engine-version adapter.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AndroidCoreBinaryTarget {
    Arm64V8a,
    X86_64,
}

impl AndroidCoreBinaryTarget {
    pub const fn target_triple(&self) -> &'static str {
        match self {
            Self::Arm64V8a => "aarch64-linux-android",
            Self::X86_64 => "x86_64-linux-android",
        }
    }

    pub fn from_target_triple(target: &str) -> Option<Self> {
        match target {
            "aarch64-linux-android" => Some(Self::Arm64V8a),
            "x86_64-linux-android" => Some(Self::X86_64),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub enum CoreLaunchBinary {
    MihomoBundled,
    SingBoxManaged {
        version: CoreVersion,
        target: AndroidCoreBinaryTarget,
    },
}

/// Rust-authored process launch inputs consumed by the narrow Android spawn
/// adapter. The selected engine and executable identity travel together so
/// Android can reject an unsupported or mismatched binary before spawning.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CoreLaunchSpec {
    configuration_identity: CoreLaunchConfigurationIdentity,
    binary: CoreLaunchBinary,
    working_directory: CoreLaunchWorkingDirectory,
    #[serde(rename = "launch_arguments")]
    launch_arguments: CoreLaunchArguments,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CoreLaunchSpecWire {
    configuration_identity: CoreLaunchConfigurationIdentity,
    binary: CoreLaunchBinary,
    working_directory: CoreLaunchWorkingDirectory,
    #[serde(rename = "launch_arguments")]
    launch_arguments: CoreLaunchArguments,
}

impl<'de> Deserialize<'de> for CoreLaunchSpec {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let wire = CoreLaunchSpecWire::deserialize(deserializer)?;
        Self::new(
            wire.configuration_identity,
            wire.binary,
            wire.working_directory.0,
            wire.launch_arguments,
        )
        .map_err(de::Error::custom)
    }
}

impl CoreLaunchSpec {
    pub fn new(
        configuration_identity: CoreLaunchConfigurationIdentity,
        binary: CoreLaunchBinary,
        working_directory: PathBuf,
        launch_arguments: CoreLaunchArguments,
    ) -> Result<Self, CoreLaunchSpecValidationError> {
        let binary_matches_engine = matches!(
            (&binary, configuration_identity.engine_id.as_str()),
            (CoreLaunchBinary::MihomoBundled, "mihomo")
                | (CoreLaunchBinary::SingBoxManaged { .. }, "sing-box")
        );
        if !binary_matches_engine {
            return Err(CoreLaunchSpecValidationError::BinaryEngineMismatch);
        }
        let arguments_match_engine = matches!(
            (&binary, &launch_arguments),
            (
                CoreLaunchBinary::MihomoBundled,
                CoreLaunchArguments::Mihomo { .. }
            ) | (
                CoreLaunchBinary::SingBoxManaged { .. },
                CoreLaunchArguments::SingBox { .. }
            )
        );
        if !arguments_match_engine {
            return Err(CoreLaunchSpecValidationError::ArgumentsEngineMismatch);
        }
        Ok(Self {
            configuration_identity,
            binary,
            working_directory: CoreLaunchWorkingDirectory::new(working_directory)?,
            launch_arguments,
        })
    }

    pub fn configuration_identity(&self) -> &CoreLaunchConfigurationIdentity {
        &self.configuration_identity
    }

    pub fn binary(&self) -> &CoreLaunchBinary {
        &self.binary
    }

    pub fn working_directory(&self) -> &CoreLaunchWorkingDirectory {
        &self.working_directory
    }

    pub fn launch_arguments(&self) -> &CoreLaunchArguments {
        &self.launch_arguments
    }
}

/// Absolute host path supplied to Android's process spawn adapter.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(transparent)]
pub struct CoreLaunchWorkingDirectory(PathBuf);

impl CoreLaunchWorkingDirectory {
    pub fn new(path: PathBuf) -> Result<Self, CoreLaunchSpecValidationError> {
        if !path.is_absolute() {
            return Err(CoreLaunchSpecValidationError::RelativeWorkingDirectory);
        }
        Ok(Self(path))
    }

    pub fn as_path(&self) -> &Path {
        &self.0
    }
}

impl<'de> Deserialize<'de> for CoreLaunchWorkingDirectory {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let path = PathBuf::deserialize(deserializer)?;
        Self::new(path).map_err(de::Error::custom)
    }
}

/// Absolute engine configuration path carried as one typed launch argument.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(transparent)]
pub struct CoreLaunchPath(String);

impl CoreLaunchPath {
    pub fn new(path: impl Into<String>) -> Result<Self, CoreLaunchSpecValidationError> {
        let path = path.into();
        if path.trim().is_empty() {
            return Err(CoreLaunchSpecValidationError::EmptyConfigurationPath);
        }
        if path.contains('\0') {
            return Err(CoreLaunchSpecValidationError::NullByteInConfigurationPath);
        }
        if !Path::new(&path).is_absolute() {
            return Err(CoreLaunchSpecValidationError::RelativeConfigurationPath);
        }
        Ok(Self(path))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl<'de> Deserialize<'de> for CoreLaunchPath {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let path = String::deserialize(deserializer)?;
        Self::new(path).map_err(de::Error::custom)
    }
}

/// Engine-specific command intent translated to argv only by the Android host.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub enum CoreLaunchArguments {
    Mihomo { configuration_path: CoreLaunchPath },
    SingBox { configuration_path: CoreLaunchPath },
}

impl CoreLaunchArguments {
    pub fn configuration_path(&self) -> &CoreLaunchPath {
        match self {
            Self::Mihomo { configuration_path } | Self::SingBox { configuration_path } => {
                configuration_path
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CoreLaunchSpecValidationError {
    BinaryEngineMismatch,
    ArgumentsEngineMismatch,
    RelativeWorkingDirectory,
    RelativeConfigurationPath,
    EmptyConfigurationPath,
    NullByteInConfigurationPath,
}

impl fmt::Display for CoreLaunchSpecValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BinaryEngineMismatch => {
                formatter.write_str("Core launch binary does not match its engine identity")
            }
            Self::ArgumentsEngineMismatch => {
                formatter.write_str("Core launch arguments do not match their engine binary")
            }
            Self::RelativeWorkingDirectory => {
                formatter.write_str("Core launch working directory must be absolute")
            }
            Self::RelativeConfigurationPath => {
                formatter.write_str("Core launch configuration path must be absolute")
            }
            Self::EmptyConfigurationPath => {
                formatter.write_str("Core launch configuration path must not be blank")
            }
            Self::NullByteInConfigurationPath => {
                formatter.write_str("Core launch configuration path contains a null byte")
            }
        }
    }
}

impl error::Error for CoreLaunchSpecValidationError {}

#[cfg(test)]
mod core_launch_spec_tests {
    use super::{
        AndroidCoreBinaryTarget, CoreLaunchArguments, CoreLaunchBinary,
        CoreLaunchConfigurationIdentity, CoreLaunchPath, CoreLaunchSpec,
        CoreLaunchSpecValidationError,
    };
    use crate::profile_document_identity::{ProfileDocumentRevision, ProfileId};
    use lurker_core_contract::identity::{CoreVersion, EngineId};
    use std::path::{Path, PathBuf};

    fn identity() -> CoreLaunchConfigurationIdentity {
        CoreLaunchConfigurationIdentity {
            engine_id: EngineId::new("mihomo").expect("engine ID"),
            profile_id: ProfileId::new("primary").expect("profile ID"),
            document_revision: ProfileDocumentRevision::new("sha256:abc123")
                .expect("configuration revision"),
        }
    }

    #[test]
    fn android_binary_target_mapping_is_exact_for_supported_abis() {
        for (target, triple) in [
            (AndroidCoreBinaryTarget::Arm64V8a, "aarch64-linux-android"),
            (AndroidCoreBinaryTarget::X86_64, "x86_64-linux-android"),
        ] {
            assert_eq!(target.target_triple(), triple);
            assert_eq!(
                AndroidCoreBinaryTarget::from_target_triple(triple),
                Some(target),
            );
        }
        assert_eq!(
            AndroidCoreBinaryTarget::from_target_triple("armv7-linux-androideabi"),
            None,
        );
    }

    #[test]
    fn mihomo_launch_spec_round_trips_typed_identity_and_configuration_path() {
        let spec = CoreLaunchSpec::new(
            identity(),
            CoreLaunchBinary::MihomoBundled,
            PathBuf::from("/data/app/files"),
            CoreLaunchArguments::Mihomo {
                configuration_path: CoreLaunchPath::new(
                    "/data/app/files/configs/engines/mihomo/primary.yaml".to_owned(),
                )
                .expect("absolute Mihomo profile path"),
            },
        )
        .expect("launch spec inputs are valid");

        let encoded = serde_json::to_string(&spec).expect("serialize launch spec");
        let value: serde_json::Value = serde_json::from_str(&encoded).expect("parse launch spec");
        assert_eq!(value["configuration_identity"]["engine_id"], "mihomo");
        assert_eq!(value["configuration_identity"]["profile_id"], "primary");
        assert_eq!(value["binary"], "MihomoBundled");
        assert_eq!(value["working_directory"], "/data/app/files");
        assert_eq!(
            value["launch_arguments"]["Mihomo"]["configuration_path"],
            "/data/app/files/configs/engines/mihomo/primary.yaml"
        );
        assert_eq!(
            spec.working_directory().as_path(),
            Path::new("/data/app/files")
        );
        assert_eq!(
            spec.launch_arguments().configuration_path().as_str(),
            "/data/app/files/configs/engines/mihomo/primary.yaml"
        );
        assert_eq!(
            serde_json::from_str::<CoreLaunchSpec>(&encoded).expect("deserialize launch spec"),
            spec,
        );
    }

    #[test]
    fn sing_box_launch_binary_is_bound_to_engine_version_and_android_target() {
        let spec = CoreLaunchSpec::new(
            CoreLaunchConfigurationIdentity {
                engine_id: EngineId::new("sing-box").expect("engine ID"),
                profile_id: ProfileId::new("primary").expect("profile ID"),
                document_revision: ProfileDocumentRevision::new("sha256:singbox")
                    .expect("configuration revision"),
            },
            CoreLaunchBinary::SingBoxManaged {
                version: CoreVersion::new("v1.14.3").expect("sing-box version"),
                target: AndroidCoreBinaryTarget::Arm64V8a,
            },
            PathBuf::from("/data/app/files"),
            CoreLaunchArguments::SingBox {
                configuration_path: CoreLaunchPath::new(
                    "/data/app/files/configs/engines/sing-box/primary.json".to_owned(),
                )
                .expect("absolute sing-box profile path"),
            },
        )
        .expect("matching managed sing-box launch spec");

        let encoded = serde_json::to_value(spec).expect("serialize launch spec");
        assert_eq!(encoded["binary"]["SingBoxManaged"]["version"], "v1.14.3");
        assert_eq!(encoded["binary"]["SingBoxManaged"]["target"], "arm64_v8a");
        assert_eq!(
            encoded["launch_arguments"]["SingBox"]["configuration_path"],
            "/data/app/files/configs/engines/sing-box/primary.json"
        );
    }

    #[test]
    fn core_launch_spec_rejects_invalid_paths_and_engine_argument_mismatch() {
        assert_eq!(
            CoreLaunchSpec::new(
                identity(),
                CoreLaunchBinary::MihomoBundled,
                PathBuf::from("relative"),
                CoreLaunchArguments::Mihomo {
                    configuration_path: CoreLaunchPath::new("/data/config.yaml".to_owned())
                        .expect("absolute Mihomo profile path"),
                },
            )
            .expect_err("relative working directory is rejected"),
            CoreLaunchSpecValidationError::RelativeWorkingDirectory,
        );
        assert_eq!(
            CoreLaunchPath::new(" ".to_owned()).expect_err("blank config path is rejected"),
            CoreLaunchSpecValidationError::EmptyConfigurationPath,
        );
        assert_eq!(
            CoreLaunchPath::new("relative/profile.json".to_owned())
                .expect_err("relative config path is rejected"),
            CoreLaunchSpecValidationError::RelativeConfigurationPath,
        );
        assert_eq!(
            CoreLaunchPath::new("/data/bad\0profile.json".to_owned())
                .expect_err("NUL cannot be passed to a process"),
            CoreLaunchSpecValidationError::NullByteInConfigurationPath,
        );
        assert_eq!(
            CoreLaunchSpec::new(
                identity(),
                CoreLaunchBinary::MihomoBundled,
                PathBuf::from("/data"),
                CoreLaunchArguments::SingBox {
                    configuration_path: CoreLaunchPath::new("/data/profile.json".to_owned())
                        .expect("absolute sing-box profile path"),
                },
            )
            .expect_err("Mihomo binary must use Mihomo launch arguments"),
            CoreLaunchSpecValidationError::ArgumentsEngineMismatch,
        );
        assert_eq!(
            CoreLaunchSpec::new(
                identity(),
                CoreLaunchBinary::SingBoxManaged {
                    version: CoreVersion::new("v1.14.3").expect("sing-box version"),
                    target: AndroidCoreBinaryTarget::Arm64V8a,
                },
                PathBuf::from("/data"),
                CoreLaunchArguments::SingBox {
                    configuration_path: CoreLaunchPath::new("/data/profile.json".to_owned())
                        .expect("absolute sing-box profile path"),
                },
            )
            .expect_err("sing-box binary cannot be paired with a Mihomo profile"),
            CoreLaunchSpecValidationError::BinaryEngineMismatch,
        );
    }

    #[test]
    fn core_launch_spec_deserialization_applies_the_same_validation() {
        let relative_directory = r#"{
            "configuration_identity": {
                "engine_id": "mihomo",
                "profile_id": "primary",
                "document_revision": "sha256:abc123"
            },
            "binary": "MihomoBundled",
            "working_directory": "relative",
            "launch_arguments": {
                "Mihomo": {"configuration_path": "/data/profile.yaml"}
            }
        }"#;
        assert!(serde_json::from_str::<CoreLaunchSpec>(relative_directory).is_err());

        let blank_argument = r#"{
            "configuration_identity": {
                "engine_id": "mihomo",
                "profile_id": "primary",
                "document_revision": "sha256:abc123"
            },
            "binary": "MihomoBundled",
            "working_directory": "/data/app/files",
            "launch_arguments": {
                "Mihomo": {"configuration_path": " "}
            }
        }"#;
        assert!(serde_json::from_str::<CoreLaunchSpec>(blank_argument).is_err());

        let mismatched_binary = r#"{
            "configuration_identity": {
                "engine_id": "mihomo",
                "profile_id": "primary",
                "document_revision": "sha256:abc123"
            },
            "binary": {"SingBoxManaged": {"version": "v1.14.3", "target": "arm64_v8a"}},
            "working_directory": "/data/app/files",
            "launch_arguments": {
                "SingBox": {"configuration_path": "/data/profile.json"}
            }
        }"#;
        assert!(serde_json::from_str::<CoreLaunchSpec>(mismatched_binary).is_err());

        let unknown_binary_field = r#"{
            "configuration_identity": {
                "engine_id": "sing-box",
                "profile_id": "primary",
                "document_revision": "sha256:abc123"
            },
            "binary": {
                "SingBoxManaged": {
                    "version": "v1.14.3",
                    "target": "arm64_v8a",
                    "channel": "stable"
                }
            },
            "working_directory": "/data/app/files",
            "launch_arguments": {
                "SingBox": {"configuration_path": "/data/profile.json"}
            }
        }"#;
        assert!(serde_json::from_str::<CoreLaunchSpec>(unknown_binary_field).is_err());
    }
}
