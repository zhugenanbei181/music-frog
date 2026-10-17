use super::*;
use crate::profile_option_store::write_options_bytes;
use infiltrator_contract::core_identity::{CoreInstanceId, CoreVersion};
use infiltrator_contract::core_process::CoreProcessStartContext;
use infiltrator_contract::snapshot::{CoreLifecycle, CoreLifecycleSnapshot};
use infiltrator_domain::profile_options::options_path;
use infiltrator_ports::active_profile_reference_store::ActiveProfileOwnerSwitchState;
use infiltrator_profile_store::active_reference_owner::ActiveCoreOwnerLaunchProof;
use lurker_core_contract::session::SessionToken;
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
