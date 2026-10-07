use super::client::ServiceClient;
use super::server::{MockServiceHarness, ServiceServer};
use super::*;
use std::sync::Arc;
use std::time::Duration;
use tempfile::TempDir;

#[path = "service_test/auth.rs"]
mod auth;
#[path = "service_test/client.rs"]
mod client;
#[path = "service_test/command.rs"]
mod command;
#[path = "service_test/constant.rs"]
mod constant;
#[path = "service_test/default.rs"]
mod default;
#[path = "service_test/ipc.rs"]
mod ipc;
#[path = "service_test/lifecycle.rs"]
mod lifecycle;
#[path = "service_test/linux.rs"]
mod linux;
#[path = "service_test/macos.rs"]
mod macos;
#[path = "service_test/mock.rs"]
mod mock;
#[path = "service_test/privilege.rs"]
mod privilege;
#[path = "service_test/protocol.rs"]
mod protocol;
#[path = "service_test/real.rs"]
mod real;
#[path = "service_test/send.rs"]
mod send;
#[path = "service_test/service.rs"]
mod service;
#[path = "service_test/windows.rs"]
mod windows;
