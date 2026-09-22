//! Pure domain logic for MusicFrog Infiltrator.
//!
//! This crate is deliberately independent from Tokio, network clients,
//! filesystems, operating-system APIs, and UI toolkits. It may be embedded by
//! any native surface or tested without an executor.

pub mod active_exit;
pub mod app_routing;
pub mod apply;
pub mod backoff_strategy;
pub mod backup;
pub mod config;
pub mod connection_activity;
pub mod connection_view;
pub mod core_state;
pub mod diagnostics;
pub mod dns;
pub mod dns_tester;
pub mod dns_topology;
pub mod failover_arbiter;
pub mod fake_ip;
pub mod filter;
pub mod geo_lookup_cache;
pub mod hosts_engine;
pub mod idle_connection_sweeper;
pub mod lan_security;
pub mod mixin;
pub mod mrs;
pub mod mtu_optimizer;
pub mod network_roaming;
pub mod pac_generator;
pub mod pac_policy;
pub mod packet_loss_tracker;
pub mod pcap_exporter;
pub mod privileged_network_policy;
pub mod profile_aggregator;
pub mod profile_converter;
pub mod profile_options;
pub mod profiles;
pub mod proxy;
pub mod proxy_nodes;
pub mod proxy_providers;
pub mod redact;
pub mod rule_hit_counter;
pub mod rules;
pub mod runtime;
pub mod sandbox;
pub mod script_engine;
pub mod settings;
pub mod snapshots;
pub mod sniffer;
pub mod sub_rules;
pub mod subscription;
pub mod subscription_quota;
pub mod subscription_scheduler_policy;
pub mod sync;
pub mod traffic_audit;
pub mod traffic_scale;
pub mod traffic_topology;
pub mod traffic_waveform;
pub mod tun;
pub mod uwp;
pub mod vector_clock;
pub mod vpn_policy;
pub mod watchdog;
pub mod yaml_edit;
pub mod zeroize_guard;

pub mod myers_diff;
