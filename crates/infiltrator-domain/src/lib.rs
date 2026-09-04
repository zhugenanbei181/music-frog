//! Pure domain logic for MusicFrog Infiltrator.
//!
//! This crate is deliberately independent from Tokio, network clients,
//! filesystems, operating-system APIs, and UI toolkits. It may be embedded by
//! any native surface or tested without an executor.

pub mod backoff_strategy;
pub mod core_state;
pub mod dns_tester;
pub mod failover_arbiter;
pub mod filter;
pub mod geo_lookup_cache;
pub mod mrs;
pub mod mixin;
pub mod mtu_optimizer;
pub mod packet_loss_tracker;
pub mod pcap_exporter;
pub mod profile_converter;
pub mod profile_options;
pub mod proxy_nodes;
pub mod rule_hit_counter;
pub mod rules;
pub mod pac_generator;
pub mod script_engine;
pub mod sub_rules;
pub mod traffic_audit;
pub mod vector_clock;
pub mod zeroize_guard;
pub mod yaml_edit;
