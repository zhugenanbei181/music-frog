//! sing-box Clash API boundaries: HTTP transport, typed client, controller
//! preflight and the CoreScope fence shared by the runtime adapters.

pub mod clash_api;
pub mod controller_configuration;
pub mod delay;
pub mod http;
pub(crate) mod scope_fence;
