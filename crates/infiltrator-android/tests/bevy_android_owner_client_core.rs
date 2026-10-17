#![allow(clippy::large_enum_variant, clippy::single_match)]
//! Runs the production Android owner IPC client without compiling Bevy UI.
//!
//! The test target supplies only the small UI command/source traits consumed by
//! the driver module. The client implementation and its fake-transport tests
//! are included directly from the Android driver source tree.

extern crate self as infiltrator_bevy_ui;

pub mod command {
    use crate::command_events::CommandExecutedEvent;
    use infiltrator_contract::android_control::AndroidControlIntent;
    use infiltrator_contract::command::{CommandIntent, RequestId};

    #[derive(Clone, Debug, PartialEq, Eq)]
    pub enum UiCommand {
        StartCore,
        StopCore,
        RestartCore,
        StartVpn,
        StopVpn,
        CloseAllConnections,
    }

    impl UiCommand {
        pub fn android_control_intent(&self) -> Option<AndroidControlIntent> {
            match self {
                Self::StartCore => Some(AndroidControlIntent::ResumeKernel),
                Self::StopCore => Some(AndroidControlIntent::PauseKernel),
                Self::RestartCore => Some(AndroidControlIntent::RestartKernel),
                Self::StartVpn => Some(AndroidControlIntent::StartProxy),
                Self::StopVpn | Self::CloseAllConnections => None,
            }
        }

        pub fn to_intent(&self) -> Option<CommandIntent> {
            match self {
                Self::StartCore => Some(CommandIntent::StartCore),
                Self::StopCore => Some(CommandIntent::StopCore),
                Self::RestartCore => Some(CommandIntent::RestartCore),
                Self::StartVpn => Some(CommandIntent::StartVpn),
                Self::StopVpn => Some(CommandIntent::StopVpn),
                Self::CloseAllConnections => Some(CommandIntent::CloseAllConnections),
            }
        }
    }

    pub trait UiCommandSink: Send + Sync {
        fn submit(&self, command: UiCommand);
        fn submit_tracked(&self, command: UiCommand) -> Option<RequestId>;
        fn drain_results(&self) -> Vec<CommandExecutedEvent>;
    }
}

pub mod command_events {
    use crate::command::UiCommand;
    use infiltrator_contract::command::RequestId;
    use infiltrator_contract::command_output::CommandOutput;
    use lurker_core_contract::error::Failure;

    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct CommandExecutedEvent {
        pub command: UiCommand,
        pub request_id: RequestId,
        pub result: Result<CommandOutput, Failure>,
    }
}

pub mod projection {
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct OverviewProjection;

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum SourceKind {
        LiveCore,
    }

    pub trait OverviewSource: Send + Sync {
        fn current(&self) -> OverviewProjection;
        fn kind(&self) -> SourceKind;
    }
}

pub mod surface {
    use crate::projection::OverviewProjection;
    use infiltrator_contract::surface_snapshot::SurfaceSnapshot;

    pub trait SurfaceSource: Send + Sync {
        fn surface_snapshot(&self) -> SurfaceSnapshot;
    }

    pub fn overview_projection(_: &SurfaceSnapshot) -> OverviewProjection {
        OverviewProjection
    }
}

#[path = "../../../android/bevy-host/driver/src/owner_client.rs"]
mod owner_client;
