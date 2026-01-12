use anyhow::anyhow;
use axum::{
    http::StatusCode,
    response::Redirect,
    routing::get,
    Router,
};
use mihomo_config::port::find_available_port;
use std::net::SocketAddr;
use std::path::PathBuf;
use tokio::{
    net::TcpListener,
    sync::oneshot,
};
use tower_http::services::{ServeDir, ServeFile};

use crate::admin_api::{self, AdminApiContext, AdminApiState, AdminEventBus};

pub struct StaticServerHandle {
    pub url: String,
    shutdown: Option<oneshot::Sender<()>>,
}

impl StaticServerHandle {
    pub fn stop(mut self) {
        if let Some(tx) = self.shutdown.take() {
            let _ = tx.send(());
        }
    }
}

pub struct AdminServerHandle {
    pub url: String,
    shutdown: Option<oneshot::Sender<()>>,
}

impl AdminServerHandle {
    pub fn stop(mut self) {
        if let Some(tx) = self.shutdown.take() {
            let _ = tx.send(());
        }
    }
}

pub async fn start_static_server(
    root_dir: PathBuf,
    preferred_port: Option<u16>,
    default_port: u16,
) -> anyhow::Result<StaticServerHandle> {
    let port = match preferred_port {
        Some(port) => port,
        None => find_available_port(default_port).ok_or_else(|| {
            anyhow!(
                "没有可用端口用于静态站点"
            )
        })?,
    };
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let listener = TcpListener::bind(addr).await?;

    let main_service = ServeDir::new(root_dir.clone())
        .append_index_html_on_directories(true)
        .fallback(ServeFile::new(root_dir.join("index.html")));

    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    tokio::spawn(async move {
        let server = axum::serve(listener, Router::new().fallback_service(main_service))
            .with_graceful_shutdown(async move {
                let _ = shutdown_rx.await;
            });

        if let Err(err) = server.await {
            log::warn!("static server exited: {err}");
        }
    });

    Ok(StaticServerHandle {
        url: format!("http://127.0.0.1:{port}"),
        shutdown: Some(shutdown_tx),
    })
}

pub async fn start_admin_server<C: AdminApiContext>(
    admin_dir: PathBuf,
    ctx: C,
    preferred_port: Option<u16>,
    default_port: u16,
    events: AdminEventBus,
) -> anyhow::Result<AdminServerHandle> {
    let port = match preferred_port {
        Some(port) => port,
        None => find_available_port(default_port).ok_or_else(|| {
            anyhow!(
                "没有可用端口用于配置管理界面"
            )
        })?,
    };
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let listener = TcpListener::bind(addr).await?;

    let admin_static_service = ServeDir::new(admin_dir.clone())
        .append_index_html_on_directories(true)
        .fallback(ServeFile::new(admin_dir.join("index.html")));

    let api_state = AdminApiState::new(ctx, events);
    let router = Router::new()
        .merge(admin_api::router(api_state))
        .nest_service("/admin", admin_static_service)
        .route("/", get(|| async { Redirect::temporary("/admin/") }))
        .fallback(|| async {
            (
                StatusCode::NOT_FOUND,
                "请访问 /admin/",
            )
        });

    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    tokio::spawn(async move {
        let server = axum::serve(listener, router).with_graceful_shutdown(async move {
            let _ = shutdown_rx.await;
        });

        if let Err(err) = server.await {
            log::warn!("admin server exited: {err}");
        }
    });

    Ok(AdminServerHandle {
        url: format!("http://127.0.0.1:{port}/admin/"),
        shutdown: Some(shutdown_tx),
    })
}
