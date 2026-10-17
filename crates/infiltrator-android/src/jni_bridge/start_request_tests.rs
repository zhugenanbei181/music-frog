use super::CoreStartRequestWire;
use super::*;
use infiltrator_contract::android_control::{
    CoreLaunchArguments, CoreLaunchBinary, CoreLaunchConfigurationIdentity, CoreLaunchPath,
    CoreLaunchSpec,
};
use lurker_core_contract::core_process::CoreProcessStartContext;
use lurker_core_contract::identity::{CoreGeneration, CoreIdentity, CoreInstanceId, EngineId};
use lurker_core_contract::profile_document_identity::{ProfileDocumentRevision, ProfileId};
use lurker_core_contract::session::SessionToken;
use lurker_core_ports::error::PortError;
use std::path::PathBuf;

#[test]
fn core_start_wire_carries_typed_binary_profile_and_managed_owner_context() {
    let launch_spec = CoreLaunchSpec::new(
        CoreLaunchConfigurationIdentity {
            engine_id: EngineId::new("mihomo").expect("engine ID"),
            profile_id: ProfileId::new("primary").expect("profile ID"),
            document_revision: ProfileDocumentRevision::new("sha256:abc")
                .expect("document revision"),
        },
        CoreLaunchBinary::MihomoBundled,
        PathBuf::from("/data/app/files"),
        CoreLaunchArguments::Mihomo {
            configuration_path: CoreLaunchPath::new("/data/app/files/config.yaml".to_owned())
                .expect("absolute config path"),
        },
    )
    .expect("launch spec");
    let context = CoreProcessStartContext::managed(
        CoreIdentity::new(
            EngineId::new("mihomo").expect("engine ID"),
            CoreInstanceId::new("android-active").expect("instance ID"),
            None,
        ),
        CoreGeneration::new(7),
        SessionToken::new(42),
    );
    let request = CoreStartRequestWire {
        launch_spec: &launch_spec,
        start_context: &context,
    };

    assert_eq!(
        serde_json::to_value(request).expect("serialize Android Core start request"),
        serde_json::json!({
            "launch_spec": {
                "configuration_identity": {
                    "engine_id": "mihomo",
                    "profile_id": "primary",
                    "document_revision": "sha256:abc"
                },
                "binary": "MihomoBundled",
                "working_directory": "/data/app/files",
                "launch_arguments": {
                    "Mihomo": {
                        "configuration_path": "/data/app/files/config.yaml"
                    }
                }
            },
            "start_context": {
                "Managed": {
                    "identity": {
                        "engine": "mihomo",
                        "instance": "android-active",
                        "version": null
                    },
                    "generation": 7,
                    "session_token": 42
                }
            }
        }),
    );
}
