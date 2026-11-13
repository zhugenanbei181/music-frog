//! Typed, user-owned Connections view preference.
//!
//! The pinned ClashParty source persists `connectionViewMode` as `list` or
//! `table` and `connectionTableColumns` as an ordered array of column keys.
//! This module owns the complete typed vocabulary, the stable wire identity,
//! and the only legal transitions. Parsing is fail-closed: an empty column
//! list, an unknown ID, or a duplicate column never constructs a preference.

use serde::de::{self, SeqAccess, Visitor};
use serde::ser::SerializeSeq;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::error;
use std::fmt::{self, Display, Formatter};

/// The mutually exclusive morphologies of the Connections page.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConnectionViewMode {
    #[default]
    List,
    Table,
}

impl ConnectionViewMode {
    pub const ALL: [Self; 2] = [Self::List, Self::Table];

    pub const fn wire_id(self) -> &'static str {
        match self {
            Self::List => "list",
            Self::Table => "table",
        }
    }

    pub fn from_wire_id(raw: &str) -> Result<Self, ConnectionViewPreferenceError> {
        match raw {
            "list" => Ok(Self::List),
            "table" => Ok(Self::Table),
            _ => Err(ConnectionViewPreferenceError::UnknownMode(raw.to_owned())),
        }
    }
}

/// The complete closed set of selectable Connections table columns.
///
/// The order is the upstream `CONNECTION_TABLE_COLUMNS` registry order: the
/// owner uses it to publish an ordered column vocabulary, while a user
/// preference keeps its own ordered visible subset. `DestinationIp` uses the
/// pinned source's `destinationIP` detail-modal identity, so the closed set
/// has 23 entries while the source table registry has 21 data columns. The
/// last entry, `Duration`, is the one column this product adds: the
/// reference desktop client publishes the age of a live connection, and this
/// kernel publishes the start and the observation instant it needs to do so.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ConnectionColumn {
    Status,
    EstablishTime,
    Type,
    Host,
    SniffHost,
    Process,
    ProcessPath,
    Rule,
    ProxyChain,
    SourceIp,
    SourcePort,
    DestinationIp,
    DestinationPort,
    InboundIp,
    InboundPort,
    UploadSpeed,
    DownloadSpeed,
    Upload,
    Download,
    Dscp,
    RemoteDestination,
    DnsMode,
    /// How long the connection has been alive.
    ///
    /// The kernel publishes the start of every connection and the Core
    /// publishes the instant of the observation that carried it, so the age
    /// is a fact of the product and not a number a surface invents from its
    /// own clock. A closed record publishes its own length instead.
    Duration,
}

impl ConnectionColumn {
    pub const ALL: [Self; 23] = [
        Self::Status,
        Self::EstablishTime,
        Self::Type,
        Self::Host,
        Self::SniffHost,
        Self::Process,
        Self::ProcessPath,
        Self::Rule,
        Self::ProxyChain,
        Self::SourceIp,
        Self::SourcePort,
        Self::DestinationIp,
        Self::DestinationPort,
        Self::InboundIp,
        Self::InboundPort,
        Self::UploadSpeed,
        Self::DownloadSpeed,
        Self::Upload,
        Self::Download,
        Self::Dscp,
        Self::RemoteDestination,
        Self::DnsMode,
        Self::Duration,
    ];

    /// The columns a reader sees before they choose their own.
    ///
    /// The duration column is deliberately not here: it answers the same
    /// question as the establishment time, so it is one column among the
    /// reader's own choices rather than a second time column every table
    /// carries by default. Upstream hard-codes it as the live table's last
    /// column; this product lets the reader decide.
    pub const DEFAULT_VISIBLE: [Self; 12] = [
        Self::Status,
        Self::EstablishTime,
        Self::Type,
        Self::Host,
        Self::Process,
        Self::Rule,
        Self::ProxyChain,
        Self::RemoteDestination,
        Self::UploadSpeed,
        Self::DownloadSpeed,
        Self::Upload,
        Self::Download,
    ];

    pub const fn wire_id(self) -> &'static str {
        match self {
            Self::Status => "status",
            Self::EstablishTime => "establishTime",
            Self::Type => "type",
            Self::Host => "host",
            Self::SniffHost => "sniffHost",
            Self::Process => "process",
            Self::ProcessPath => "processPath",
            Self::Rule => "rule",
            Self::ProxyChain => "proxyChain",
            Self::SourceIp => "sourceIP",
            Self::SourcePort => "sourcePort",
            Self::DestinationIp => "destinationIP",
            Self::DestinationPort => "destinationPort",
            Self::InboundIp => "inboundIP",
            Self::InboundPort => "inboundPort",
            Self::UploadSpeed => "uploadSpeed",
            Self::DownloadSpeed => "downloadSpeed",
            Self::Upload => "upload",
            Self::Download => "download",
            Self::Dscp => "dscp",
            Self::RemoteDestination => "remoteDestination",
            Self::DnsMode => "dnsMode",
            Self::Duration => "duration",
        }
    }

    pub fn from_wire_id(raw: &str) -> Result<Self, ConnectionViewPreferenceError> {
        match raw {
            "status" => Ok(Self::Status),
            "establishTime" => Ok(Self::EstablishTime),
            "type" => Ok(Self::Type),
            "host" => Ok(Self::Host),
            "sniffHost" => Ok(Self::SniffHost),
            "process" => Ok(Self::Process),
            "processPath" => Ok(Self::ProcessPath),
            "rule" => Ok(Self::Rule),
            "proxyChain" => Ok(Self::ProxyChain),
            "sourceIP" => Ok(Self::SourceIp),
            "sourcePort" => Ok(Self::SourcePort),
            "destinationIP" => Ok(Self::DestinationIp),
            "destinationPort" => Ok(Self::DestinationPort),
            "inboundIP" => Ok(Self::InboundIp),
            "inboundPort" => Ok(Self::InboundPort),
            "uploadSpeed" => Ok(Self::UploadSpeed),
            "downloadSpeed" => Ok(Self::DownloadSpeed),
            "upload" => Ok(Self::Upload),
            "download" => Ok(Self::Download),
            "dscp" => Ok(Self::Dscp),
            "remoteDestination" => Ok(Self::RemoteDestination),
            "dnsMode" => Ok(Self::DnsMode),
            "duration" => Ok(Self::Duration),
            _ => Err(ConnectionViewPreferenceError::UnknownColumn(raw.to_owned())),
        }
    }

    /// Whether the kernel publishes the fact this column names.
    ///
    /// The closed registry carries every column the peer product offers, but
    /// this kernel does not report all of them: there is no inbound address,
    /// no DSCP marking and no sniffed host in the observed metadata. A column
    /// the owner does not report renders the honest unknown - it never guesses
    /// a value, and a surface cannot invent one to fill a cell.
    pub const fn support(self) -> ConnectionColumnSupport {
        match self {
            Self::SniffHost | Self::InboundIp | Self::InboundPort | Self::Dscp => {
                ConnectionColumnSupport::Unreported
            }
            Self::Status
            | Self::EstablishTime
            | Self::Type
            | Self::Host
            | Self::Process
            | Self::ProcessPath
            | Self::Rule
            | Self::ProxyChain
            | Self::SourceIp
            | Self::SourcePort
            | Self::DestinationIp
            | Self::DestinationPort
            | Self::UploadSpeed
            | Self::DownloadSpeed
            | Self::Upload
            | Self::Download
            | Self::RemoteDestination
            | Self::DnsMode
            | Self::Duration => ConnectionColumnSupport::Reported,
        }
    }

    /// Whether a surface may render a real value in this column.
    pub const fn is_reported(self) -> bool {
        matches!(self.support(), ConnectionColumnSupport::Reported)
    }
}

/// Whether the kernel publishes the fact a Connections column names.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConnectionColumnSupport {
    /// The owner publishes this fact; a surface renders it.
    Reported,
    /// The kernel never publishes this fact; a surface renders the unknown.
    Unreported,
}

impl Serialize for ConnectionColumn {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.wire_id())
    }
}

impl<'de> Deserialize<'de> for ConnectionColumn {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = String::deserialize(deserializer)?;
        Self::from_wire_id(&raw).map_err(de::Error::custom)
    }
}

/// An ordered, non-empty, duplicate-free column vocabulary or selection.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConnectionColumnSet(Vec<ConnectionColumn>);

impl ConnectionColumnSet {
    pub fn new(columns: Vec<ConnectionColumn>) -> Result<Self, ConnectionViewPreferenceError> {
        if columns.is_empty() {
            return Err(ConnectionViewPreferenceError::EmptyColumns);
        }
        let mut seen = [false; ConnectionColumn::ALL.len()];
        for column in &columns {
            let index = Self::index(*column);
            if seen[index] {
                return Err(ConnectionViewPreferenceError::DuplicateColumn(*column));
            }
            seen[index] = true;
        }
        Ok(Self(columns))
    }

    pub fn all() -> Self {
        Self(ConnectionColumn::ALL.to_vec())
    }

    pub fn as_slice(&self) -> &[ConnectionColumn] {
        &self.0
    }

    pub fn contains(&self, column: ConnectionColumn) -> bool {
        self.0.contains(&column)
    }

    pub fn wire_ids(&self) -> Vec<&'static str> {
        self.0.iter().map(|column| column.wire_id()).collect()
    }

    pub fn from_wire_ids<'a, I>(raw: I) -> Result<Self, ConnectionViewPreferenceError>
    where
        I: IntoIterator<Item = &'a str>,
    {
        let mut columns = Vec::new();
        for value in raw {
            columns.push(ConnectionColumn::from_wire_id(value)?);
        }
        Self::new(columns)
    }

    fn index(column: ConnectionColumn) -> usize {
        ConnectionColumn::ALL
            .iter()
            .position(|candidate| *candidate == column)
            .expect("complete column registry")
    }
}

impl Serialize for ConnectionColumnSet {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut sequence = serializer.serialize_seq(Some(self.0.len()))?;
        for column in &self.0 {
            sequence.serialize_element(column)?;
        }
        sequence.end()
    }
}

impl<'de> Deserialize<'de> for ConnectionColumnSet {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct ColumnSetVisitor;

        impl<'de> Visitor<'de> for ColumnSetVisitor {
            type Value = ConnectionColumnSet;

            fn expecting(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
                formatter.write_str("a non-empty ordered column set with unique known IDs")
            }

            fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
            where
                A: SeqAccess<'de>,
            {
                let mut columns = Vec::with_capacity(sequence.size_hint().unwrap_or_default());
                while let Some(column) = sequence.next_element::<ConnectionColumn>()? {
                    columns.push(column);
                }
                ConnectionColumnSet::new(columns).map_err(de::Error::custom)
            }
        }

        deserializer.deserialize_seq(ColumnSetVisitor)
    }
}

/// The complete persisted Connections view preference.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConnectionViewPreference {
    pub mode: ConnectionViewMode,
    pub visible_columns: ConnectionColumnSet,
}

impl ConnectionViewPreference {
    pub fn new(
        mode: ConnectionViewMode,
        visible_columns: ConnectionColumnSet,
    ) -> Result<Self, ConnectionViewPreferenceError> {
        if visible_columns.as_slice().is_empty() {
            return Err(ConnectionViewPreferenceError::EmptyColumns);
        }
        Ok(Self {
            mode,
            visible_columns,
        })
    }

    pub fn from_wire(
        mode: &str,
        visible_columns: &[&str],
    ) -> Result<Self, ConnectionViewPreferenceError> {
        Self::new(
            ConnectionViewMode::from_wire_id(mode)?,
            ConnectionColumnSet::from_wire_ids(visible_columns.iter().copied())?,
        )
    }

    pub fn from_default() -> Self {
        Self {
            mode: ConnectionViewMode::default(),
            visible_columns: default_visible_columns(),
        }
    }
}

impl Default for ConnectionViewPreference {
    fn default() -> Self {
        Self::from_default()
    }
}

fn default_visible_columns() -> ConnectionColumnSet {
    ConnectionColumnSet(ConnectionColumn::DEFAULT_VISIBLE.to_vec())
}

/// One typed command from a surface to the preference owner.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "intent", rename_all = "snake_case", deny_unknown_fields)]
pub enum ConnectionViewIntent {
    SetViewMode { mode: ConnectionViewMode },
    ToggleColumn { column: ConnectionColumn },
    ResetColumns,
}

/// Why a wire value or preference mutation cannot be accepted.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "reason", rename_all = "snake_case")]
pub enum ConnectionViewPreferenceError {
    UnknownMode(String),
    UnknownColumn(String),
    ColumnOutsideRegistry(ConnectionColumn),
    EmptyColumns,
    DuplicateColumn(ConnectionColumn),
    WouldRemoveLastColumn,
}

impl Display for ConnectionViewPreferenceError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownMode(raw) => write!(formatter, "unknown connection view mode `{raw}`"),
            Self::UnknownColumn(raw) => write!(formatter, "unknown connection column `{raw}`"),
            Self::ColumnOutsideRegistry(column) => write!(
                formatter,
                "connection column `{}` is outside the ordered registry",
                column.wire_id()
            ),
            Self::EmptyColumns => write!(formatter, "connection columns must not be empty"),
            Self::DuplicateColumn(column) => {
                write!(
                    formatter,
                    "connection column `{}` repeats",
                    column.wire_id()
                )
            }
            Self::WouldRemoveLastColumn => {
                write!(
                    formatter,
                    "the last visible connection column cannot be removed"
                )
            }
        }
    }
}

impl error::Error for ConnectionViewPreferenceError {}

/// Apply exactly one typed intent to the current authoritative preference.
pub fn apply_connection_view_intent(
    current: &ConnectionViewPreference,
    ordered_columns: &ConnectionColumnSet,
    intent: ConnectionViewIntent,
) -> Result<ConnectionViewPreference, ConnectionViewPreferenceError> {
    match intent {
        ConnectionViewIntent::SetViewMode { mode } => Ok(ConnectionViewPreference {
            mode,
            visible_columns: current.visible_columns.clone(),
        }),
        ConnectionViewIntent::ToggleColumn { column } => {
            if !ordered_columns.contains(column) {
                return Err(ConnectionViewPreferenceError::ColumnOutsideRegistry(column));
            }
            let mut remaining: Vec<ConnectionColumn> = current
                .visible_columns
                .as_slice()
                .iter()
                .copied()
                .filter(|visible| *visible != column)
                .collect();
            if remaining.len() == current.visible_columns.as_slice().len() {
                remaining.push(column);
            } else if remaining.is_empty() {
                return Err(ConnectionViewPreferenceError::WouldRemoveLastColumn);
            }
            Ok(ConnectionViewPreference {
                mode: current.mode,
                visible_columns: ConnectionColumnSet::new(remaining)?,
            })
        }
        ConnectionViewIntent::ResetColumns => Ok(ConnectionViewPreference {
            mode: current.mode,
            visible_columns: default_visible_columns(),
        }),
    }
}

#[cfg(test)]
#[path = "connection_view_preference_test.rs"]
mod tests;
