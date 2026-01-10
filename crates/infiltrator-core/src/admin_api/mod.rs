pub mod handlers;
pub mod models;
pub mod state;

use axum::{
    middleware,
    routing::{get, post},
    Router,
};

use self::handlers::*;
pub use self::models::*;
pub use self::state::*;

pub fn router<C: AdminApiContext>(state: AdminApiState<C>) -> Router {
    Router::new()
        .route("/admin/api/profiles", get(list_profiles_http::<C>))
        .route(
            "/admin/api/profiles/:name",
            get(get_profile_http::<C>).delete(delete_profile_http::<C>),
        )
        .route(
            "/admin/api/profiles/:name/subscription",
            post(set_profile_subscription_http::<C>)
                .delete(clear_profile_subscription_http::<C>),
        )
        .route(
            "/admin/api/profiles/:name/update-now",
            post(update_profile_now_http::<C>),
        )
        .route("/admin/api/profiles/switch", post(switch_profile_http::<C>))
        .route("/admin/api/profiles/save", post(save_profile_http::<C>))
        .route("/admin/api/profiles/import", post(import_profile_http::<C>))
        .route("/admin/api/profiles/clear", post(clear_profiles_http::<C>))
        .route("/admin/api/profiles/open", post(open_profile_in_editor_http::<C>))
        .route(
            "/admin/api/editor",
            get(get_editor_config_http::<C>).post(set_editor_config_http::<C>),
        )
        .route(
            "/admin/api/editor/pick",
            post(pick_editor_path_http::<C>),
        )
        .route(
            "/admin/api/settings",
            get(get_app_settings_http::<C>).post(save_app_settings_http::<C>),
        )
        .route("/admin/api/webdav/sync", post(sync_webdav_now_http::<C>))
        .route("/admin/api/webdav/test", post(test_webdav_conn_http::<C>))
        .route("/admin/api/rebuild/status", get(get_rebuild_status_http::<C>))
        .route("/admin/api/core/versions", get(list_core_versions_http::<C>))
        .route("/admin/api/core/activate", post(activate_core_version_http::<C>))
        .with_state(state)
        .layer(middleware::from_fn(log_admin_request))
}
