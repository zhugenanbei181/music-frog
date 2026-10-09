//! The product pages of the Bevy frontend.
//!
//! **Page contract** (taskmanager-proven shape, one module per page):
//! a page exposes one scene constructor that takes the projection data it
//! renders plus the [`UiPalette`](infiltrator_bevy_widgets::palette::
//! UiPalette), and returns an `impl Scene` — the *only* entry a route's
//! scene table knows. Page roots carry a
//! [`PageRoot`](crate::route::PageRoot) marker naming their route.
//!
//! - Static structure composes declaratively with `bsn!` through the
//!   widget layer's `*_scene` adapters; the bsn! guard enforces it.
//! - Data never bakes into structure the page can't update: mutable text
//!   and fill nodes carry typed markers, and each page plugin registers its
//!   refresh observers once during product assembly. They restamp those
//!   components in place when the typed
//!   projection event fires. No polling, no tree rebuilds.
//! - All colors come from palette tokens; fonts come from text roles.
//!   Pages never touch bevy color/asset constructors directly.

pub mod app_routing;
mod app_routing_copy;
pub mod app_routing_uwp;
mod app_routing_uwp_copy;
pub mod business_panel;
pub mod connections;
pub mod connections_clipboard;
pub mod connections_confirm;
pub mod connections_demo;
pub mod connections_drawer;
pub mod connections_groups;
pub mod connections_idle;
pub mod connections_pulse;
pub mod connections_row;
pub mod connections_rows;
pub mod connections_search;
pub mod connections_view;
pub mod dns;
pub mod dns_cache;
pub mod dns_cache_focus;
pub mod dns_cache_scene;
pub mod dns_demo;
pub mod dns_edit;
pub mod dns_fakeip;
pub(crate) mod dns_form;
pub mod dns_hosts;
pub mod dns_hosts_actions;
pub mod dns_hosts_rows;
pub mod dns_hosts_scene;
pub mod dns_hosts_sync;
pub mod dns_leak;
pub mod dns_leak_actions;
pub mod dns_leak_rows;
pub mod dns_self_heal;
pub mod dns_servers;
pub mod dns_stun;
pub mod doctor;
pub mod doctor_actions;
pub mod doctor_rows;
pub mod logs;
pub mod logs_export;
pub mod logs_export_focus;
pub mod logs_export_render;
pub mod logs_export_scene;
pub mod logs_follow;
pub mod logs_rows;
pub mod logs_search;
pub mod overview;
pub mod overview_cards;
pub mod overview_lifecycle;
pub mod overview_public_ip;
pub mod overview_rates;
pub mod overview_restamp;
pub mod overview_speedtest;
pub mod overview_topology;
pub mod plugin;
pub mod profiles;
pub mod profiles_aggregator;
pub mod profiles_aggregator_wizard;
pub mod profiles_diff;
pub mod profiles_diff_history;
pub mod profiles_editor;
pub mod profiles_editor_body;
pub mod profiles_editor_copy;
pub mod profiles_editor_filter;
pub mod profiles_editor_mixin_studio;
pub mod profiles_editor_panes;
pub mod profiles_editor_panes_sync;
pub mod profiles_editor_state;
pub mod profiles_filter_dedup;
pub mod profiles_filter_form;
pub mod profiles_import;
pub mod profiles_import_channels;
pub mod profiles_mixin_copy;
pub mod profiles_script_scene;
mod profiles_script_view;
pub mod profiles_script_workbench;
pub mod profiles_subscription_copy;
pub mod profiles_subscription_policy;
pub mod proxies;
pub mod proxies_card;
pub mod proxies_custom;
pub mod proxies_filter;
pub mod proxies_form;
pub mod proxies_highlight;
pub mod proxies_identity;
pub mod proxies_preferences;
pub mod proxies_reconcile;
pub mod proxies_refresh;
pub mod proxies_search;
pub mod proxies_virtual;
pub mod proxy_group_order;
pub mod proxy_inspection;
pub mod proxy_probe;
pub mod proxy_probe_settings;
pub mod rules;
pub mod rules_builder;
pub mod rules_builder_input;
pub mod rules_demo;
pub mod rules_draft;
pub mod rules_edit;
pub mod rules_fact_copy;
pub mod rules_json;
pub mod rules_mrs;
pub mod rules_projection;
pub mod rules_row_copy;
mod rules_subrule_copy;
pub mod rules_subrules;
pub mod rules_tabs;
pub mod rules_tracer;
pub mod rules_tracer_confirm;
pub mod rules_tracer_sandbox;
pub mod rules_view;
pub mod settings;
pub mod settings_language;
pub mod sync;
pub mod sync_merge;

pub mod dns_query;
pub mod dns_query_scene;

pub mod rules_statistics;
mod rules_statistics_render;
mod rules_statistics_scene;

mod rules_statistics_focus;

pub mod snapshot_restore;
pub mod snapshot_restore_scene;
pub mod snapshot_restore_view;

pub mod profiles_snapshot_copy;

pub mod overview_quota_copy;

pub mod profiles_editor_transactions;
