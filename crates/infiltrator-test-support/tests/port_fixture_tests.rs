//! Behaviour tests for the isolated port fixtures.
//!
//! Every fixture must answer the owner's question with its real published
//! snapshot, refuse a request it cannot honour with a typed failure, and
//! reject a stale revision instead of overwriting a newer state.

#[path = "port_fixture_tests/core_process.rs"]
mod core_process;

#[path = "port_fixture_tests/lan_configuration.rs"]
mod lan_configuration;

#[path = "port_fixture_tests/port_conflict.rs"]
mod port_conflict;

#[path = "port_fixture_tests/privileged_network.rs"]
mod privileged_network;

#[path = "port_fixture_tests/profile_document_codec.rs"]
mod profile_document_codec;

#[path = "port_fixture_tests/profile_store.rs"]
mod profile_store;

#[path = "port_fixture_tests/rule_provider.rs"]
mod rule_provider;

#[path = "port_fixture_tests/service_mode.rs"]
mod service_mode;

#[path = "port_fixture_tests/snapshot_store.rs"]
mod snapshot_store;

#[path = "port_fixture_tests/speedtest_history.rs"]
mod speedtest_history;

#[path = "port_fixture_tests/subscription_import.rs"]
mod subscription_import;

#[path = "port_fixture_tests/subscription_source.rs"]
mod subscription_source;

#[path = "port_fixture_tests/version.rs"]
mod version;
