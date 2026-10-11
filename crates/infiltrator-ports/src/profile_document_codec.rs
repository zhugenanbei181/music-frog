//! Engine/format/schema-scoped inspection for opaque profile document bytes.

use crate::error::PortError;
use infiltrator_contract::capability::Capability;
use infiltrator_contract::core_identity::EngineId;
use infiltrator_contract::dns::DnsSettingsPatch;
use infiltrator_contract::dns_hosts::DnsHostEntry;
use infiltrator_contract::profile_document::DocumentValidation;
use infiltrator_contract::profile_document_identity::{
    ProfileDocumentIdentity, ProfileFormatId, ProfileSchemaVersion, ProfileWorkspaceIdentity,
};
use infiltrator_contract::provider_cache::KernelEtagSupportSnapshot;
use infiltrator_contract::rule_location::RuleLocation;
use infiltrator_contract::rule_source::RuleSourceIdentity;
use infiltrator_contract::subscription_filter_result::FilterReport;
use infiltrator_contract::subscription_import::SubscriptionFilterDraft;
use infiltrator_domain::{dns, fake_ip, proxy_providers, rules, sniffer, tun};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProfileDocumentCodecScope {
    pub engine_id: EngineId,
    pub format_id: ProfileFormatId,
    pub schema_version: ProfileSchemaVersion,
}

/// Sidecar schema owned by the same engine/format adapter as its profile
/// document codec.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProfileOptionsCodecScope {
    pub document: ProfileDocumentCodecScope,
    pub sidecar_format_id: ProfileFormatId,
    pub sidecar_schema_version: ProfileSchemaVersion,
}

/// Neutral editor projection of the adapter-owned options sidecar.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProfileOptionsProjection {
    pub mixin_yaml: String,
    pub filter: SubscriptionFilterDraft,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProfileMixinToggleId {
    Ipv6,
    AllowLan,
    DnsFakeIp,
    Tun,
    Sniffer,
    DebugLog,
}

impl ProfileMixinToggleId {
    pub const ALL: [Self; 6] = [
        Self::Ipv6,
        Self::AllowLan,
        Self::DnsFakeIp,
        Self::Tun,
        Self::Sniffer,
        Self::DebugLog,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Ipv6 => "ipv6",
            Self::AllowLan => "allow-lan",
            Self::DnsFakeIp => "dns-fake-ip",
            Self::Tun => "tun",
            Self::Sniffer => "sniffer",
            Self::DebugLog => "log-level-debug",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProfileMixinColumnRole {
    Base,
    Overlay,
    Composed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProfileMixinColumn {
    pub role: ProfileMixinColumnRole,
    pub content: String,
    pub editable: bool,
    pub line_count: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProfileMixinCascadeStageId {
    Base,
    Subscription,
    Merge,
    PreMixin,
    PostMixin,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProfileMixinStageDeclaration {
    Applied,
    Undeclared,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProfileMixinCascadeStage {
    pub id: ProfileMixinCascadeStageId,
    pub declaration: ProfileMixinStageDeclaration,
    pub line_count: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProfileMixinPreflight {
    Accepted,
    Blocked { reason: String },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProfileMixinCascade {
    Merged {
        stages: Vec<ProfileMixinCascadeStage>,
        line_count: usize,
    },
    Blocked {
        stages: Vec<ProfileMixinCascadeStage>,
        reason: String,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProfileMixinComposition {
    Produced(ProfileMixinColumn),
    Blocked { reason: String },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProfileMixinColumns {
    pub base: ProfileMixinColumn,
    pub overlay: ProfileMixinColumn,
    pub composed: ProfileMixinComposition,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProfileMixinToggle {
    pub id: ProfileMixinToggleId,
    pub enabled: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProfileMixinToggleProjection {
    Available(Vec<ProfileMixinToggle>),
    Unavailable { reason: String },
}

/// One engine-authored, format-neutral Mixin studio projection for the exact
/// document identity and buffer pair supplied by the editor.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProfileMixinStudioProjection {
    pub preflight: ProfileMixinPreflight,
    pub cascade: ProfileMixinCascade,
    pub columns: ProfileMixinColumns,
    pub toggles: ProfileMixinToggleProjection,
}

/// Result of a Mixin edit, ready for the source-bound workspace transaction.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProfileMixinMutation {
    pub document_bytes: Vec<u8>,
    pub sidecar_bytes: Option<Vec<u8>>,
    pub projection: ProfileOptionsProjection,
}

/// Result of applying a subscription filter, ready for the same atomic
/// document/sidecar transaction.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProfileFilterMutation {
    pub document_bytes: Vec<u8>,
    pub sidecar_bytes: Option<Vec<u8>>,
    pub projection: ProfileOptionsProjection,
    pub report: FilterReport,
}

/// Typed Mihomo-profile read operation. The codec is selected by the
/// immutable engine/format/schema identity before it receives the document.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProfileConfigurationQuery {
    Dns,
    FakeIp,
    Tun,
    Rules,
    RuleProviders,
    ProxyProviders,
    Sniffer,
    ProxyEndpoint,
    EtagSupport,
    Hosts,
}

/// Adapter-authored configuration readback. JSON-shaped provider members are
/// retained only inside their typed provider maps; the application never
/// accepts or patches an untyped configuration document.
#[derive(Clone, Debug)]
pub enum ProfileConfigurationReadout {
    Dns(dns::DnsConfig),
    FakeIp(fake_ip::FakeIpConfig),
    Tun(tun::TunConfig),
    Rules(Vec<rules::RuleEntry>),
    RuleProviders(rules::RuleProviders),
    ProxyProviders(proxy_providers::ProxyProviders),
    Sniffer(sniffer::SnifferConfig),
    ProxyEndpoint(Option<String>),
    EtagSupport(KernelEtagSupportSnapshot),
    Hosts {
        entries: Vec<DnsHostEntry>,
        legacy_entries: Vec<DnsHostEntry>,
    },
}

/// Typed update accepted by an engine/format/schema-specific profile codec.
#[derive(Clone, Debug)]
pub enum ProfileConfigurationUpdate {
    Dns(dns::DnsConfigPatch),
    DnsSettings(DnsSettingsPatch),
    FakeIp(fake_ip::FakeIpConfigPatch),
    Tun(tun::TunConfigPatch),
    Rules(Vec<rules::RuleEntry>),
    RuleProviders(rules::RuleProviders),
    ProxyProviders(proxy_providers::ProxyProviders),
    Sniffer(sniffer::SnifferConfig),
}

/// Pure document transformation plus the typed value decoded from the exact
/// output bytes. Storage and live-core effects belong to a higher layer.
#[derive(Clone, Debug)]
pub struct ProfileConfigurationMutation {
    pub document_bytes: Vec<u8>,
    pub readback: ProfileConfigurationReadout,
}

/// Engine-authored projection of a rule workspace. The application attaches
/// the exact sidecar revision from the coherent workspace read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProfileRuleWorkspaceProjection {
    pub source: RuleSourceIdentity,
    pub rules: Vec<rules::RuleEntry>,
    pub targets: Vec<String>,
    pub sub_rules: std::collections::BTreeMap<String, Vec<rules::RuleEntry>>,
}

#[derive(Clone, Debug)]
pub enum ProfileRuleWorkspaceUpdate {
    ReplaceRules(Vec<rules::RuleEntry>),
    ReplaceNamedRule {
        location: RuleLocation,
        expected_rule: String,
        replacement: String,
    },
}

#[derive(Clone, Debug)]
pub struct ProfileRuleWorkspaceMutation {
    pub document_bytes: Vec<u8>,
    pub readback: ProfileRuleWorkspaceProjection,
}

/// Character-based caret used by format adapters that support editor snippets.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProfileDocumentCaret {
    /// One-based line number.
    pub line: usize,
    /// Zero-based character column.
    pub column: usize,
}

/// Format-neutral result of a successful editor snippet splice.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProfileDocumentSnippetInsertion {
    pub snippet_id: String,
    pub content: String,
    pub cursor_line: usize,
    pub cursor_column: usize,
    pub validation: DocumentValidation,
}

/// A splice may be applied to an already-invalid document, but a splice that
/// makes a valid document invalid is rejected with neutral diagnostics.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProfileDocumentSnippetOutcome {
    Inserted(ProfileDocumentSnippetInsertion),
    Rejected { validation: DocumentValidation },
}

/// Pure format inspection for exact document bytes. Implementations must not
/// read or write the profile store or contact a running core.
pub trait ProfileDocumentCodecPort: Send + Sync {
    /// Return immutable adapter identity without performing I/O.
    fn scope(&self) -> ProfileDocumentCodecScope;
    /// Inspect the supplied bytes without mutating them or invoking runtime work.
    fn inspect(&self, raw_bytes: &[u8]) -> Result<DocumentValidation, PortError>;

    /// Decode one typed configuration view from opaque document bytes.
    fn read_configuration(
        &self,
        _identity: &ProfileDocumentIdentity,
        _raw_bytes: &[u8],
        _query: ProfileConfigurationQuery,
    ) -> Result<ProfileConfigurationReadout, PortError> {
        Err(PortError::unsupported(
            Capability::Profiles,
            "engine-scoped typed profile configuration read",
        ))
    }

    /// Apply a typed configuration update to opaque document bytes and
    /// decode its exact output before any store/runtime side effect occurs.
    fn apply_configuration(
        &self,
        _identity: &ProfileDocumentIdentity,
        _raw_bytes: &[u8],
        _update: ProfileConfigurationUpdate,
    ) -> Result<ProfileConfigurationMutation, PortError> {
        Err(PortError::unsupported(
            Capability::Profiles,
            "engine-scoped typed profile configuration update",
        ))
    }

    /// Build a rule-editor/tracer projection from the exact workspace and
    /// opaque document bytes. Format-specific rule tables remain in the codec.
    fn read_rule_workspace(
        &self,
        _workspace: &ProfileWorkspaceIdentity,
        _raw_bytes: &[u8],
    ) -> Result<ProfileRuleWorkspaceProjection, PortError> {
        Err(PortError::unsupported(
            Capability::Profiles,
            "engine-scoped rule workspace projection",
        ))
    }

    /// Apply a typed rule workspace edit to opaque bytes and decode the exact
    /// resulting document before the host persistence transaction begins.
    fn apply_rule_workspace(
        &self,
        _workspace: &ProfileWorkspaceIdentity,
        _raw_bytes: &[u8],
        _update: ProfileRuleWorkspaceUpdate,
    ) -> Result<ProfileRuleWorkspaceMutation, PortError> {
        Err(PortError::unsupported(
            Capability::Profiles,
            "engine-scoped rule workspace update",
        ))
    }

    /// Insert one adapter-owned editor snippet without touching profile
    /// storage or runtime state. Formats without snippet support fail closed.
    fn insert_snippet(
        &self,
        _content: &str,
        _snippet_id: &str,
        _caret: ProfileDocumentCaret,
    ) -> Result<ProfileDocumentSnippetOutcome, PortError> {
        Err(PortError::unsupported(
            Capability::Profiles,
            "profile editor snippets",
        ))
    }

    /// Return the adapter-owned options sidecar schema. Default adapters do
    /// not claim a schema by virtue of sharing the document codec trait.
    fn options_scope(&self) -> Result<ProfileOptionsCodecScope, PortError> {
        Err(PortError::unsupported(
            Capability::Profiles,
            "profile options sidecar schema",
        ))
    }

    /// Decode opaque sidecar bytes into the neutral editor projection.
    fn inspect_options(
        &self,
        _identity: &ProfileDocumentIdentity,
        _sidecar_bytes: Option<&[u8]>,
    ) -> Result<ProfileOptionsProjection, PortError> {
        Err(PortError::unsupported(
            Capability::Profiles,
            "profile options sidecar inspection",
        ))
    }

    /// Apply a Mixin edit without mutating storage or runtime state.
    fn apply_mixin(
        &self,
        _identity: &ProfileDocumentIdentity,
        _document_bytes: &[u8],
        _sidecar_bytes: Option<&[u8]>,
        _mixin_yaml: &str,
    ) -> Result<ProfileMixinMutation, PortError> {
        Err(PortError::unsupported(
            Capability::Profiles,
            "Mihomo Mixin schema operation",
        ))
    }

    /// Read one typed preset toggle from an engine-owned Mixin schema.
    fn inspect_mixin_toggle(
        &self,
        _identity: &ProfileDocumentIdentity,
        _mixin_yaml: &str,
        _toggle_id: ProfileMixinToggleId,
    ) -> Result<bool, PortError> {
        Err(PortError::unsupported(
            Capability::Profiles,
            "Mixin preset toggle inspection",
        ))
    }

    /// Update one typed preset toggle using the selected engine codec.
    fn set_mixin_toggle(
        &self,
        _identity: &ProfileDocumentIdentity,
        _mixin_yaml: &str,
        _toggle_id: ProfileMixinToggleId,
        _enabled: bool,
    ) -> Result<String, PortError> {
        Err(PortError::unsupported(
            Capability::Profiles,
            "Mixin preset toggle update",
        ))
    }

    /// Project the current Mixin studio facts using this exact schema adapter.
    fn inspect_mixin_studio(
        &self,
        _identity: &ProfileDocumentIdentity,
        _document_bytes: &[u8],
        _mixin_yaml: &str,
    ) -> Result<ProfileMixinStudioProjection, PortError> {
        Err(PortError::unsupported(
            Capability::Profiles,
            "Mixin studio schema projection",
        ))
    }

    /// Apply a filter draft without mutating storage or runtime state.
    fn apply_filter(
        &self,
        _identity: &ProfileDocumentIdentity,
        _document_bytes: &[u8],
        _sidecar_bytes: Option<&[u8]>,
        _draft: &SubscriptionFilterDraft,
    ) -> Result<ProfileFilterMutation, PortError> {
        Err(PortError::unsupported(
            Capability::Profiles,
            "Mihomo filter schema operation",
        ))
    }
}
