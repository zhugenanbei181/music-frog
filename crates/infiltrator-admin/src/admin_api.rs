pub mod events;
pub mod handlers;
pub mod models;
pub mod state;

use axum::{
    Router, middleware,
    routing::{delete, get, post},
};

pub use self::events::*;
use self::handlers::*;
pub use self::models::*;
pub use self::state::*;

pub fn router<C: AdminApiContext>(state: AdminApiState<C>) -> Router {
    Router::new()
        .route("/admin/api/profiles", get(list_profiles_http::<C>))
        .route(
            "/admin/api/profiles/{name}",
            get(get_profile_http::<C>).delete(delete_profile_http::<C>),
        )
        .route(
            "/admin/api/profiles/{name}/subscription",
            post(set_profile_subscription_http::<C>).delete(clear_profile_subscription_http::<C>),
        )
        .route(
            "/admin/api/profiles/{name}/update-now",
            post(update_profile_now_http::<C>),
        )
        .route("/admin/api/profiles/switch", post(switch_profile_http::<C>))
        .route("/admin/api/profiles/save", post(save_profile_http::<C>))
        .route("/admin/api/profiles/import", post(import_profile_http::<C>))
        .route("/admin/api/profiles/clear", post(clear_profiles_http::<C>))
        .route(
            "/admin/api/profiles/open",
            post(open_profile_in_editor_http::<C>),
        )
        .route(
            "/admin/api/editor",
            get(get_editor_config_http::<C>).post(set_editor_config_http::<C>),
        )
        .route("/admin/api/editor/pick", post(pick_editor_path_http::<C>))
        .route(
            "/admin/api/settings",
            get(get_app_settings_http::<C>).post(save_app_settings_http::<C>),
        )
        .route("/admin/api/capabilities", get(get_capabilities_http::<C>))
        .route("/admin/api/proxies", get(get_proxies_http::<C>))
        .route("/admin/api/proxy/mode", post(set_proxy_mode_http::<C>))
        .route("/admin/api/proxy/select", post(select_proxy_http::<C>))
        .route(
            "/admin/api/dns",
            get(get_dns_config_http::<C>).post(save_dns_config_http::<C>),
        )
        .route(
            "/admin/api/fake-ip",
            get(get_fake_ip_config_http::<C>).post(save_fake_ip_config_http::<C>),
        )
        .route(
            "/admin/api/fake-ip/flush",
            post(flush_fake_ip_cache_http::<C>),
        )
        .route(
            "/admin/api/rule-providers",
            get(get_rule_providers_http::<C>).post(save_rule_providers_http::<C>),
        )
        .route(
            "/admin/api/proxy-providers",
            get(get_proxy_providers_http::<C>).post(save_proxy_providers_http::<C>),
        )
        .route(
            "/admin/api/sniffer",
            get(get_sniffer_config_http::<C>).post(save_sniffer_config_http::<C>),
        )
        .route(
            "/admin/api/rules",
            get(get_rules_http::<C>).post(save_rules_http::<C>),
        )
        .route(
            "/admin/api/tun",
            get(get_tun_config_http::<C>).post(save_tun_config_http::<C>),
        )
        .route("/admin/api/webdav/sync", post(sync_webdav_now_http::<C>))
        .route("/admin/api/webdav/test", post(test_webdav_conn_http::<C>))
        .route("/admin/api/events", get(stream_admin_events_http::<C>))
        .route(
            "/admin/api/rebuild/status",
            get(get_rebuild_status_http::<C>),
        )
        .route(
            "/admin/api/runtime/connections",
            get(list_runtime_connections_http::<C>).delete(close_all_runtime_connections_http::<C>),
        )
        .route("/admin/api/runtime/start", post(start_runtime_http::<C>))
        .route("/admin/api/runtime/stop", post(stop_runtime_http::<C>))
        .route("/admin/api/runtime/status", get(get_runtime_status_http::<C>))
        .route(
            "/admin/api/runtime/connections/{id}",
            delete(close_runtime_connection_http::<C>),
        )
        .route(
            "/admin/api/runtime/proxies",
            get(list_runtime_proxy_delays_http::<C>),
        )
        .route(
            "/admin/api/runtime/delay/test",
            post(test_runtime_proxy_delay_http::<C>),
        )
        .route(
            "/admin/api/runtime/delay/test-all",
            post(test_all_runtime_proxy_delays_http::<C>),
        )
        .route(
            "/admin/api/runtime/logs",
            get(stream_runtime_logs_http::<C>),
        )
        .route(
            "/admin/api/runtime/traffic",
            get(get_runtime_traffic_http::<C>),
        )
        .route(
            "/admin/api/runtime/memory",
            get(get_runtime_memory_http::<C>),
        )
        .route("/admin/api/runtime/ip", get(get_runtime_ip_http::<C>))
        .route(
            "/admin/api/core/versions",
            get(list_core_versions_http::<C>),
        )
        .route(
            "/admin/api/core/latest-stable",
            get(get_latest_stable_core_http::<C>),
        )
        .route(
            "/admin/api/core/download",
            post(download_core_version_http::<C>),
        )
        .route(
            "/admin/api/core/update-stable",
            post(update_stable_core_http::<C>),
        )
        .route(
            "/admin/api/core/activate",
            post(activate_core_version_http::<C>),
        )
        .with_state(state)
        .layer(middleware::from_fn(log_admin_request))
}

#[cfg(test)]
mod admin_api_test;
