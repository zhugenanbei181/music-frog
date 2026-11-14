//! The Connections page's table morphology, projected once for every surface.
//!
//! The list answers "what is happening right now"; the table answers "which
//! facts does this user want side by side". The visible columns are the
//! user's persisted preference, so this projection never invents a column,
//! never reorders the user's choice and never fills a cell with a guess:
//!
//! * a column the kernel does not publish at all renders the honest unknown,
//!   because the contract's `ConnectionColumn::support()` is the only place
//!   that knows which facts this engine reports;
//! * a fact the owner did not report for *this* row renders the same
//!   unknown, so an empty cell never means "empty" and never means zero;
//! * an instantaneous rate stays a `Live` cell, so a surface can render it
//!   apart from a cumulative total without re-deriving which one it holds.
//!
//! The projection is pure: it reads the typed preference and the observed
//! rows and publishes the table. It performs no I/O, keeps no state and
//! cannot dispatch a command.

use crate::connection_view_projection::ConnectionViewColumnProjection;
use chrono::DateTime;
use infiltrator_contract::connection::ConnectionRateObservation;
use infiltrator_contract::connection_view_preference::{ConnectionColumn, ConnectionColumnSupport};
use infiltrator_domain::connection_observation::ConnectionObservationInstant;
use infiltrator_domain::connection_view::{
    ConnectionSortKey, ConnectionView, RatedConnection, route_chain,
};
use infiltrator_shared::byte_units::format_bytes;
use infiltrator_shared::effect_assets::text_width_em;
use infiltrator_shared::elapsed_time::format_duration_ms;

/// The honest value of a fact nobody published.
pub const UNAVAILABLE: &str = "—";

/// How much room a column wants, so a surface lays the table out without
/// re-deriving which column carries what.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ConnectionColumnWidth {
    /// A status dot, a protocol, a port or a counter.
    Narrow,
    /// A host, a process name, a rule or an address.
    Regular,
    /// A process path or a route chain.
    Wide,
}

/// Whether a row is still live or already closed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ConnectionTableStatus {
    /// A connection the kernel is still carrying.
    Active,
    /// A record of a connection that already ended.
    Closed,
}

/// How a surface presents one column's cells.
///
/// The tone belongs to the column, not to the surface: the matched rule reads
/// as the accent everywhere, a route chain reads as a secondary fact
/// everywhere, and a counter reads as a number everywhere. A surface only
/// maps the tone onto its own roles, so no surface decides on its own that
/// one column is special.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ConnectionTableTone {
    /// An ordinary fact.
    Plain,
    /// The matched rule, the page's accent.
    Accent,
    /// A secondary fact, such as the route chain or the process path.
    Muted,
    /// A number, such as a rate or a counter.
    Numeric,
}

/// One table cell, typed by what it carries.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ConnectionTableCell {
    /// A value the owner reported and that does not change between frames.
    Text(String),
    /// An instantaneous value that changes between frames.
    Live(String),
    /// The row's status, rendered as a status dot.
    Status(ConnectionTableStatus),
    /// The route chain: its first hop is the outbound the connection left
    /// through, the rest are the hops after it.
    ///
    /// The outbound is its own value because the reference product renders it
    /// as a chip the reader can press to narrow the list to that proxy; a
    /// plain string would force every surface to split the chain again.
    Chain {
        /// The outbound the connection left through.
        outbound: String,
        /// The hops after the outbound, already joined for display.
        rest: String,
    },
    /// Nobody published this fact.
    Unavailable,
}

impl ConnectionTableCell {
    /// The text this cell renders, when it renders text at all.
    pub fn text(&self) -> Option<&str> {
        match self {
            Self::Text(value) | Self::Live(value) => Some(value.as_str()),
            Self::Chain { outbound, rest } => Some(if rest.is_empty() {
                outbound.as_str()
            } else {
                rest.as_str()
            }),
            Self::Status(_) | Self::Unavailable => None,
        }
    }

    /// The outbound this cell names, when the cell names one.
    pub fn outbound(&self) -> Option<&str> {
        match self {
            Self::Chain { outbound, .. } => Some(outbound.as_str()),
            _ => None,
        }
    }

    /// Whether this cell is the honest unknown.
    pub fn is_unavailable(&self) -> bool {
        matches!(self, Self::Unavailable)
    }
}

/// One header cell of the table.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ConnectionTableHeader {
    /// The contract's column identity.
    pub column: ConnectionColumn,
    /// The stable semantic ID, which doubles as the shared-copy key.
    pub semantic_id: &'static str,
    /// How much room the column wants.
    pub width: ConnectionColumnWidth,
    /// How the column's cells read.
    pub tone: ConnectionTableTone,
    /// Whether the kernel publishes this column's fact at all.
    pub support: ConnectionColumnSupport,
    /// The rank this column's header applies when the reader presses it.
    ///
    /// The reference product's table header IS the sort affordance: a column
    /// either publishes a sort key or it does not, and only a column with one
    /// is pressable. Publishing the key here rather than in the surface keeps
    /// "which fact can the table be ranked by" one answer for every surface,
    /// and keeps the vocabulary the page model already owns.
    pub sort: Option<ConnectionSortKey>,
    /// Whether this column's sort is the order the rows arrive in.
    pub sorted: bool,
}

/// One data row of the table.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConnectionTableRow {
    /// The connection identity, so a surface can key the row.
    pub id: String,
    /// The cells, in the header's order.
    pub cells: Vec<ConnectionTableCell>,
}

/// The whole table: the header first, then the rows in page order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConnectionTableProjection {
    pub headers: Vec<ConnectionTableHeader>,
    pub rows: Vec<ConnectionTableRow>,
}

/// Project the user's visible columns and the observed rows into a table.
///
/// The rows arrive in the page's own order; the projection keeps that order
/// and never sorts or filters on its own. The instant of the observation is
/// the owner's: a row that publishes its start is measured against it, so no
/// surface has to read a clock of its own to know how long a connection has
/// been alive.
pub fn project_connection_view_table<C: ConnectionView>(
    columns: &[ConnectionViewColumnProjection],
    rows: &[RatedConnection<'_, C>],
    status: ConnectionTableStatus,
    instant: ConnectionObservationInstant,
    sort: Option<ConnectionSortKey>,
) -> ConnectionTableProjection {
    let headers: Vec<ConnectionTableHeader> = columns
        .iter()
        .map(|column| ConnectionTableHeader {
            column: column.column,
            semantic_id: column.semantic_id,
            width: connection_column_width(column.column),
            tone: connection_column_tone(column.column),
            support: column.column.support(),
            sort: connection_column_sort(column.column),
            sorted: sort.is_some_and(|key| connection_column_sort(column.column) == Some(key)),
        })
        .collect();
    let rows = rows
        .iter()
        .map(|row| ConnectionTableRow {
            id: row.row.view_id().to_owned(),
            cells: headers
                .iter()
                .map(|header| cell_for(header.column, row, status, instant))
                .collect(),
        })
        .collect();
    ConnectionTableProjection { headers, rows }
}

/// How one column's cells read.
pub const fn connection_column_tone(column: ConnectionColumn) -> ConnectionTableTone {
    match column {
        ConnectionColumn::Rule => ConnectionTableTone::Accent,
        ConnectionColumn::ProxyChain | ConnectionColumn::ProcessPath => ConnectionTableTone::Muted,
        ConnectionColumn::EstablishTime
        | ConnectionColumn::SourceIp
        | ConnectionColumn::SourcePort
        | ConnectionColumn::DestinationIp
        | ConnectionColumn::DestinationPort
        | ConnectionColumn::UploadSpeed
        | ConnectionColumn::DownloadSpeed
        | ConnectionColumn::Upload
        | ConnectionColumn::Download
        | ConnectionColumn::Duration => ConnectionTableTone::Numeric,
        ConnectionColumn::Status
        | ConnectionColumn::Type
        | ConnectionColumn::Host
        | ConnectionColumn::SniffHost
        | ConnectionColumn::Process
        | ConnectionColumn::InboundIp
        | ConnectionColumn::InboundPort
        | ConnectionColumn::Dscp
        | ConnectionColumn::RemoteDestination
        | ConnectionColumn::DnsMode => ConnectionTableTone::Plain,
    }
}

/// How much room one column wants.
pub const fn connection_column_width(column: ConnectionColumn) -> ConnectionColumnWidth {
    match column {
        ConnectionColumn::Status
        | ConnectionColumn::Type
        | ConnectionColumn::SourcePort
        | ConnectionColumn::DestinationPort
        | ConnectionColumn::InboundPort
        | ConnectionColumn::UploadSpeed
        | ConnectionColumn::DownloadSpeed
        | ConnectionColumn::Upload
        | ConnectionColumn::Download
        | ConnectionColumn::Dscp
        | ConnectionColumn::DnsMode
        | ConnectionColumn::Duration => ConnectionColumnWidth::Narrow,
        ConnectionColumn::EstablishTime
        | ConnectionColumn::SniffHost
        | ConnectionColumn::Process
        | ConnectionColumn::Rule
        | ConnectionColumn::SourceIp
        | ConnectionColumn::DestinationIp
        | ConnectionColumn::InboundIp => ConnectionColumnWidth::Regular,
        ConnectionColumn::Host
        | ConnectionColumn::ProcessPath
        | ConnectionColumn::ProxyChain
        | ConnectionColumn::RemoteDestination => ConnectionColumnWidth::Wide,
    }
}

/// The rank a column's header applies when the reader presses it.
///
/// The reference table puts its sort affordance in the header: a column that
/// has a rank reads as pressable, one that has none does not. This is that
/// answer in one place, expressed in the vocabulary the page model already
/// owns, so a surface cannot decide on its own which facts are rankable and
/// two surfaces cannot rank the same column by different facts.
pub const fn connection_column_sort(column: ConnectionColumn) -> Option<ConnectionSortKey> {
    match column {
        ConnectionColumn::EstablishTime => Some(ConnectionSortKey::LatestDesc),
        ConnectionColumn::Host => Some(ConnectionSortKey::HostAsc),
        ConnectionColumn::Upload => Some(ConnectionSortKey::UploadDesc),
        ConnectionColumn::Download => Some(ConnectionSortKey::DownloadDesc),
        ConnectionColumn::UploadSpeed => Some(ConnectionSortKey::UploadRateDesc),
        ConnectionColumn::DownloadSpeed => Some(ConnectionSortKey::DownloadRateDesc),
        ConnectionColumn::Status
        | ConnectionColumn::Type
        | ConnectionColumn::SniffHost
        | ConnectionColumn::Process
        | ConnectionColumn::ProcessPath
        | ConnectionColumn::Rule
        | ConnectionColumn::ProxyChain
        | ConnectionColumn::SourceIp
        | ConnectionColumn::SourcePort
        | ConnectionColumn::DestinationIp
        | ConnectionColumn::DestinationPort
        | ConnectionColumn::InboundIp
        | ConnectionColumn::InboundPort
        | ConnectionColumn::Dscp
        | ConnectionColumn::RemoteDestination
        | ConnectionColumn::DnsMode
        | ConnectionColumn::Duration => None,
    }
}

/// The cell one column publishes for one row.
fn cell_for<C: ConnectionView>(
    column: ConnectionColumn,
    row: &RatedConnection<'_, C>,
    status: ConnectionTableStatus,
    instant: ConnectionObservationInstant,
) -> ConnectionTableCell {
    let support = column.support();
    if support == ConnectionColumnSupport::Unreported {
        return ConnectionTableCell::Unavailable;
    }
    match column {
        ConnectionColumn::Status => ConnectionTableCell::Status(status),
        ConnectionColumn::EstablishTime => establish_time(row, status),
        ConnectionColumn::Type => reported(&row.view_network().to_ascii_uppercase()),
        ConnectionColumn::Host => reported(row.view_host()),
        ConnectionColumn::Process => reported(&row.view_process_name()),
        ConnectionColumn::ProcessPath => reported(row.view_process_path()),
        ConnectionColumn::Rule => reported(row.view_rule()),
        ConnectionColumn::ProxyChain => chain_cell(row),
        ConnectionColumn::SourceIp => reported(row.view_source_ip()),
        ConnectionColumn::SourcePort => reported(row.view_source_port()),
        ConnectionColumn::DestinationIp => reported(row.view_destination_ip()),
        ConnectionColumn::DestinationPort => reported(row.view_destination_port()),
        ConnectionColumn::UploadSpeed => speed(row, true),
        ConnectionColumn::DownloadSpeed => speed(row, false),
        ConnectionColumn::Upload => {
            ConnectionTableCell::Text(format_bytes(row.view_upload_total()))
        }
        ConnectionColumn::Download => {
            ConnectionTableCell::Text(format_bytes(row.view_download_total()))
        }
        ConnectionColumn::RemoteDestination => reported(&row.view_remote_destination()),
        ConnectionColumn::DnsMode => reported(row.view_dns_mode()),
        ConnectionColumn::Duration => duration_cell(row, status, instant),
        // The contract already refused these four; the match stays exhaustive
        // over the closed registry so a future column cannot slip through.
        ConnectionColumn::SniffHost
        | ConnectionColumn::InboundIp
        | ConnectionColumn::InboundPort
        | ConnectionColumn::Dscp => {
            debug_assert_eq!(support, ConnectionColumnSupport::Unreported);
            ConnectionTableCell::Unavailable
        }
    }
}

/// The route chain cell: the outbound first, the hops after it second.
fn chain_cell<C: ConnectionView>(row: &RatedConnection<'_, C>) -> ConnectionTableCell {
    let chain = route_chain(row.row);
    let Some(outbound) = chain.first() else {
        return ConnectionTableCell::Unavailable;
    };
    let rest = chain.hops().iter().skip(1).cloned().collect::<Vec<_>>();
    ConnectionTableCell::Chain {
        outbound: outbound.to_owned(),
        rest: rest.join(" → "),
    }
}

/// The duration of one row.
///
/// A live row publishes its start, so its age is measured against the instant
/// of the observation that carried it. A closed record publishes the length
/// it lived, so it answers the question itself. A row that publishes neither
/// keeps the honest unknown: a zero would read like a connection that just
/// opened.
fn duration_cell<C: ConnectionView>(
    row: &RatedConnection<'_, C>,
    status: ConnectionTableStatus,
    instant: ConnectionObservationInstant,
) -> ConnectionTableCell {
    let duration_ms = match status {
        ConnectionTableStatus::Active => instant.age_of(row.view_start()),
        ConnectionTableStatus::Closed => row.view_duration().duration_ms(),
    };
    match duration_ms {
        Some(duration_ms) => ConnectionTableCell::Text(format_duration_ms(duration_ms)),
        None => ConnectionTableCell::Unavailable,
    }
}

/// The reported value, or the honest unknown when nobody reported one.
fn reported(value: &str) -> ConnectionTableCell {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        ConnectionTableCell::Unavailable
    } else {
        ConnectionTableCell::Text(trimmed.to_owned())
    }
}

/// The establishment instant of a live connection.
///
/// A closed record publishes the instant the connection *ended*, not the one
/// it started, so the column stays the unknown for a closed row instead of
/// answering a different question with the same number.
fn establish_time<C: ConnectionView>(
    row: &RatedConnection<'_, C>,
    status: ConnectionTableStatus,
) -> ConnectionTableCell {
    if status == ConnectionTableStatus::Closed {
        return ConnectionTableCell::Unavailable;
    }
    let start = row.view_start().trim();
    if start.is_empty() {
        return ConnectionTableCell::Unavailable;
    }
    // The kernel reports the instant in RFC3339; a table reads it as a clock
    // reading, not as a raw wire string, and an unparseable stamp keeps the
    // owner's own text rather than becoming a guess.
    match DateTime::parse_from_rfc3339(start) {
        Ok(stamp) => ConnectionTableCell::Text(stamp.format("%m-%d %H:%M:%S").to_string()),
        Err(_) => ConnectionTableCell::Text(start.to_owned()),
    }
}

/// One instantaneous rate, or the unknown when no window was measured.
fn speed<C: ConnectionView>(row: &RatedConnection<'_, C>, upload: bool) -> ConnectionTableCell {
    if row.rate.observation != ConnectionRateObservation::Measured {
        return ConnectionTableCell::Unavailable;
    }
    let bytes_per_second = if upload {
        row.rate.upload_bps
    } else {
        row.rate.download_bps
    };
    if !bytes_per_second.is_finite() || bytes_per_second < 0.0 {
        return ConnectionTableCell::Unavailable;
    }
    ConnectionTableCell::Live(format!("{}/s", format_bytes(bytes_per_second as u64)))
}

/// Cut a cell's text to the width its column allows.
///
/// The width is measured in em, so a full-width character counts twice as
/// much as a half-width one and a CJK cell is cut at the same visual width
/// as a latin one. The cut keeps the text honest: it marks the truncation
/// with an ellipsis instead of ending the word silently.
pub fn ellipsize(text: &str, max_width_em: f32) -> String {
    if !max_width_em.is_finite() || max_width_em <= 0.0 {
        return text.to_owned();
    }
    if text_width_em(text) <= max_width_em {
        return text.to_owned();
    }
    let ellipsis = "…";
    let budget = max_width_em - text_width_em(ellipsis);
    let mut kept = String::new();
    let mut width = 0.0_f32;
    for ch in text.chars() {
        let char_width = text_width_em(&ch.to_string());
        if width + char_width > budget {
            break;
        }
        kept.push(ch);
        width += char_width;
    }
    kept.push_str(ellipsis);
    kept
}

#[cfg(test)]
#[path = "connection_view_table_projection_tests.rs"]
mod tests;
