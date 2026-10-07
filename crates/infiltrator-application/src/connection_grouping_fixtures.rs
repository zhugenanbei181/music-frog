//! Shared deterministic facts for native grouping scenarios and contract tests.
use crate::connection_rate_application::connections_page_snapshot;
use crate::shell_readout_application::ShellReadoutApplication;
use infiltrator_contract::snapshot::CoreLifecycle;
use infiltrator_contract::surface_snapshot::{PageData, SurfaceOrigin, SurfaceSnapshot};
use infiltrator_domain::runtime::{Connection, ConnectionMetadata, ConnectionSnapshot};

/// Publish fixture facts as one coherent surface, including shell and overview counts.
pub fn grouping_surface(mut surface: SurfaceSnapshot) -> SurfaceSnapshot {
    let connections = grouping_snapshot();
    let page = connections_page_snapshot(&connections, &Default::default());
    surface.origin = SurfaceOrigin::Demo;
    surface.failure = None;
    surface.core.failure = None;
    surface.core.lifecycle = CoreLifecycle::Running;
    surface.revision += 1;
    surface.core.revision += 1;
    surface.core.active_connections = page.total_connections as u32;
    if let Some(overview) = surface.pages.overview.data.as_mut() {
        overview.active_connections = surface.core.active_connections;
    }
    surface.pages.connections = PageData::ready(page);
    surface.shell_readout = ShellReadoutApplication::default().project(&surface);
    surface
}

pub fn grouping_snapshot() -> ConnectionSnapshot {
    let mut connections: Vec<Connection> = (0..8)
        .map(|index| Connection {
            id: format!("group-{index}"),
            start: format!("2026-10-06T00:00:0{index}Z"),
            metadata: ConnectionMetadata {
                host: format!("api-{index}.example.org"),
                destination_port: "443".into(),
                process_path: format!("/usr/bin/client-{index}"),
                network: "tcp".into(),
                destination_ip: format!("203.0.113.{}", index + 1),
                source_ip: "192.0.2.5".into(),
                source_port: "51000".into(),
                ..Default::default()
            },
            upload: (index + 1) * 1024,
            download: (index + 1) * 2048,
            ..Default::default()
        })
        .collect();
    let mut duplicate = connections[0].clone();
    duplicate.id = "group-duplicate".into();
    connections.push(duplicate);
    ConnectionSnapshot {
        upload_total: connections.iter().map(|row| row.upload).sum(),
        download_total: connections.iter().map(|row| row.download).sum(),
        connections,
    }
}
