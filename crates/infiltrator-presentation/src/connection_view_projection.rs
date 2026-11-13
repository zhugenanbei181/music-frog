//! Pure projection of the typed Connections view preference.
//!
//! The projection never owns a column vocabulary and never infers visibility.
//! It validates each persisted, ordered visible column against the contract's
//! registry, then publishes stable semantic IDs in that exact preference order.

use infiltrator_contract::connection_view_preference::{
    ConnectionColumn, ConnectionColumnSet, ConnectionViewMode, ConnectionViewPreference,
};

/// Stable shared-copy keys for the Connections view controls.
pub const CONNECTION_VIEW_MODE_LIST_KEY: &str = "connection.view_mode.list";
pub const CONNECTION_VIEW_MODE_TABLE_KEY: &str = "connection.view_mode.table";
pub const CONNECTION_VIEW_RESET_COLUMNS_KEY: &str = "connection.view.reset_columns";
pub const CONNECTION_VIEW_TITLE_KEY: &str = "connection.view.title";

/// One visible column with its stable semantic ID.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ConnectionViewColumnProjection {
    pub column: ConnectionColumn,
    pub semantic_id: &'static str,
}

/// One selectable column in the contract's complete registry order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ConnectionColumnChoiceProjection {
    pub column: ConnectionColumn,
    pub semantic_id: &'static str,
    pub label_key: &'static str,
}

/// Ordered visible columns plus the typed view mode.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConnectionViewProjection {
    pub mode: ConnectionViewMode,
    pub columns: Vec<ConnectionViewColumnProjection>,
}

/// Project the preference's ordered columns through the contract registry.
pub fn project_connection_view(
    registry: &ConnectionColumnSet,
    preference: &ConnectionViewPreference,
) -> ConnectionViewProjection {
    let columns = preference
        .visible_columns
        .as_slice()
        .iter()
        .copied()
        .filter(|column| registry.contains(*column))
        .map(|column| ConnectionViewColumnProjection {
            column,
            semantic_id: semantic_id(column),
        })
        .collect();
    ConnectionViewProjection {
        mode: preference.mode,
        columns,
    }
}

/// Publish every selectable column once, in the contract registry order.
///
/// The semantic ID doubles as the shared-copy key, so a surface resolves it
/// through `Lang.tr()` instead of owning a second label vocabulary.
pub fn project_connection_column_choices() -> Vec<ConnectionColumnChoiceProjection> {
    ConnectionColumn::ALL
        .into_iter()
        .map(|column| {
            let semantic_id = semantic_id(column);
            ConnectionColumnChoiceProjection {
                column,
                semantic_id,
                label_key: semantic_id,
            }
        })
        .collect()
}

/// Resolve the typed view mode to its shared-copy key.
pub const fn connection_view_mode_label_key(mode: ConnectionViewMode) -> &'static str {
    match mode {
        ConnectionViewMode::List => CONNECTION_VIEW_MODE_LIST_KEY,
        ConnectionViewMode::Table => CONNECTION_VIEW_MODE_TABLE_KEY,
    }
}

#[cfg(test)]
#[path = "connection_view_projection_tests.rs"]
mod tests;

const fn semantic_id(column: ConnectionColumn) -> &'static str {
    match column {
        ConnectionColumn::Status => "connection.column.status",
        ConnectionColumn::EstablishTime => "connection.column.establish_time",
        ConnectionColumn::Type => "connection.column.type",
        ConnectionColumn::Host => "connection.column.host",
        ConnectionColumn::SniffHost => "connection.column.sniff_host",
        ConnectionColumn::Process => "connection.column.process",
        ConnectionColumn::ProcessPath => "connection.column.process_path",
        ConnectionColumn::Rule => "connection.column.rule",
        ConnectionColumn::ProxyChain => "connection.column.proxy_chain",
        ConnectionColumn::SourceIp => "connection.column.source_ip",
        ConnectionColumn::SourcePort => "connection.column.source_port",
        ConnectionColumn::DestinationIp => "connection.column.destination_ip",
        ConnectionColumn::DestinationPort => "connection.column.destination_port",
        ConnectionColumn::InboundIp => "connection.column.inbound_ip",
        ConnectionColumn::InboundPort => "connection.column.inbound_port",
        ConnectionColumn::UploadSpeed => "connection.column.upload_speed",
        ConnectionColumn::DownloadSpeed => "connection.column.download_speed",
        ConnectionColumn::Upload => "connection.column.upload",
        ConnectionColumn::Download => "connection.column.download",
        ConnectionColumn::Dscp => "connection.column.dscp",
        ConnectionColumn::RemoteDestination => "connection.column.remote_destination",
        ConnectionColumn::DnsMode => "connection.column.dns_mode",
        ConnectionColumn::Duration => "connection.column.duration",
    }
}
