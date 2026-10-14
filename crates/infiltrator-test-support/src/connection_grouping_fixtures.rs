//! Deterministic connection records shared by application and surface tests.
use infiltrator_domain::runtime::{Connection, ConnectionMetadata, ConnectionSnapshot};

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
