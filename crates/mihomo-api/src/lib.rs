//! Product adapters for the Mihomo (Clash.Meta) core.
//!
//! The engine-neutral controller client, wire types and process supervision
//! live in `mihomo-driver`. This crate only owns the product adapters that
//! implement `infiltrator-ports` on top of those neutral types: connection
//! control and streaming, runtime configuration, rule providers, Geo database
//! triggers, DNS queries, resource observation, capability declarations and
//! owner-scope binding.

pub mod connection_port;
pub mod controller_fake_ip_cache;
pub mod core_capability_provider;
mod core_log_level;
pub mod core_log_level_port;
pub mod dns_query;
pub mod geo_database;
pub mod overview;
pub mod resource_port;
pub mod rule_provider;
pub mod runtime_configuration;
pub mod runtime_stream;
mod runtime_wire;
