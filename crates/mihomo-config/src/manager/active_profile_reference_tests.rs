use super::*;
use crate::profile_option_store::write_options_bytes;
use infiltrator_domain::profile_options::options_path;
use lurker_core_contract::core_process::CoreProcessStartContext;
use lurker_core_contract::engine_switch::{
    EngineOwnerState, EngineSwitchRecovery, EngineSwitchRevision, EngineSwitchStage,
};
use lurker_core_contract::identity::{CoreInstanceId, CoreVersion};
use lurker_core_contract::lifecycle::{CoreLifecycle, CoreLifecycleSnapshot};
use lurker_core_contract::session::SessionToken;
use lurker_core_ports::engine_switch::EngineSwitchJournal;
use lurker_profile_store::owner_switch::owner::ActiveCoreOwnerLaunchProof;
use std::env;
use std::process::{Child, Command, ExitStatus};
use std::sync::Arc;
use std::time::Duration;
use tempfile::TempDir;
use tokio::fs;
use tokio::task::{spawn_blocking, yield_now};
use tokio::time::{sleep, timeout};

#[path = "active_profile_reference_tests/common.rs"]
mod common;
use common::*;

#[path = "active_profile_reference_tests/part1.rs"]
mod part1;

#[path = "active_profile_reference_tests/part2.rs"]
mod part2;
