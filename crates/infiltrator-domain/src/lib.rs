//! Pure domain logic for MusicFrog Infiltrator.
//!
//! This crate is deliberately independent from Tokio, network clients,
//! filesystems, operating-system APIs, and UI toolkits. It may be embedded by
//! any native surface or tested without an executor.

pub mod backoff_strategy;
pub mod app_routing;
pub mod apply;
pub mod backup;
pub mod config;
pub mod core_state;
pub mod diagnostics;
pub mod dns;
pub mod dns_topology;
pub mod dns_tester;
pub mod failover_arbiter;
pub mod fake_ip;
pub mod filter;
pub mod geo_lookup_cache;
pub mod hosts_engine;
pub mod idle_connection_sweeper;
pub mod mrs;
pub mod mixin;
pub mod mtu_optimizer;
pub mod packet_loss_tracker;
pub mod pcap_exporter;
pub mod profile_converter;
pub mod profile_options;
pub mod profiles;
pub mod proxy;
pub mod proxy_providers;
pub mod proxy_nodes;
pub mod rule_hit_counter;
pub mod rules;
pub mod runtime;
pub mod sandbox;
pub mod pac_generator;
pub mod redact;
pub mod script_engine;
pub mod settings;
pub mod snapshots;
pub mod sync;
pub mod sniffer;
pub mod sub_rules;
pub mod subscription;
pub mod traffic_audit;
pub mod tun;
pub mod vector_clock;
pub mod zeroize_guard;
pub mod yaml_edit;
