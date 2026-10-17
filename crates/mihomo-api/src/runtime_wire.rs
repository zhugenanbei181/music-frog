//! Conversion from neutral controller wire facts to product runtime snapshots.
//!
//! Both types are foreign to this crate (the driver owns the wire DTO, the
//! domain crate owns the product snapshot), so the conversion is an explicit
//! function instead of a `From` implementation.

use infiltrator_domain::runtime;
use mihomo_driver::types::{
    Connection, ConnectionMetadata, ConnectionSnapshot, ConnectionsResponse, TrafficData,
};

pub(crate) fn connection_metadata(value: ConnectionMetadata) -> runtime::ConnectionMetadata {
    runtime::ConnectionMetadata {
        network: value.network,
        connection_type: value.connection_type,
        source_ip: value.source_ip,
        destination_ip: value.destination_ip,
        source_port: value.source_port,
        destination_port: value.destination_port,
        host: value.host,
        dns_mode: value.dns_mode,
        process_path: value.process_path,
        special_proxy: value.special_proxy,
        destination_geo_ip: value.destination_geo_ip,
        destination_ip_asn: value.destination_ip_asn,
    }
}

pub(crate) fn connection(value: Connection) -> runtime::Connection {
    runtime::Connection {
        id: value.id,
        metadata: connection_metadata(value.metadata),
        upload: value.upload,
        download: value.download,
        start: value.start,
        rule: value.rule,
        rule_payload: value.rule_payload,
        chains: value.chains,
    }
}

pub(crate) fn connection_snapshot(value: ConnectionsResponse) -> runtime::ConnectionSnapshot {
    runtime::ConnectionSnapshot {
        download_total: value.download_total,
        upload_total: value.upload_total,
        connections: value.connections.into_iter().map(connection).collect(),
    }
}

pub(crate) fn streamed_connection_snapshot(
    value: ConnectionSnapshot,
) -> runtime::ConnectionSnapshot {
    runtime::ConnectionSnapshot {
        download_total: value.download_total,
        upload_total: value.upload_total,
        connections: value.connections.into_iter().map(connection).collect(),
    }
}

pub(crate) fn traffic_data(value: TrafficData) -> runtime::TrafficData {
    runtime::TrafficData {
        up: value.up,
        down: value.down,
    }
}
