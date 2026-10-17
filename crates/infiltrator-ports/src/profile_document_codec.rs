//! Engine/format/schema-scoped typed profile configuration operations.
//!
//! The neutral inspection trait (`scope`/`inspect`/`insert_snippet`) lives in
//! `lurker-core-ports`. This product extension adds typed configuration,
//! workspace, mixin and subscription-filter operations that depend on
//! product-owned contract types.

use infiltrator_contract::dns::DnsSettingsPatch;
use infiltrator_contract::dns_hosts::DnsHostEntry;
use infiltrator_contract::profile_document_identity::{
    ProfileDocumentIdentity, ProfileWorkspaceIdentity,
};
use infiltrator_contract::profile_mixin_studio::{
    ProfileMixinStudioObservation, ProfileMixinToggleId,
};
use infiltrator_contract::protocol_fidelity::{CodecAudit, ProtocolDraft};
use infiltrator_contract::protocol_trust::TlsTrustParams;
use infiltrator_contract::provider_cache::KernelEtagSupportSnapshot;
use infiltrator_contract::rule_location::RuleLocation;
use infiltrator_contract::rule_source::RuleSourceIdentity;
use infiltrator_contract::subscription_filter_result::FilterReport;
use infiltrator_contract::subscription_import::SubscriptionFilterDraft;
use infiltrator_domain::{dns, fake_ip, proxy_providers, rules, sniffer, tun};
use lurker_core_contract::capability::Capability;
use lurker_core_contract::profile_document_identity::{ProfileFormatId, ProfileSchemaVersion};
use lurker_core_ports::error::PortError;
use lurker_core_ports::profile_document_codec::{
    ProfileDocumentCodecPort, ProfileDocumentCodecScope,
};
use std::collections::BTreeMap;

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
    /// Typed protocol nodes in the exact engine-owned profile document.
    ProtocolNodes,
    /// The engine's profile-level path based trust-anchor configuration.
    TrustAnchors,
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
    ProtocolNodes(Vec<ProtocolDraft>),
    /// Exact readback plus the engine-authored audit for one protocol-node
    /// upsert. Kept separate from a query result so callers must handle the
    /// mutation receipt explicitly.
    ProtocolNodeMutation(ProfileProtocolNodeMutationReadback),
    /// Mihomo profile-level CA path readback. Inline PEM and per-peer
    /// fingerprints remain node-draft values, not profile trust anchors.
    TrustAnchors(TlsTrustParams),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProfileProtocolNodeMutationReadback {
    /// Protocol drafts decoded from the exact output document bytes.
    pub nodes: Vec<ProtocolDraft>,
    pub audit: CodecAudit,
    pub replaced_existing: bool,
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
    UpsertProtocolNode(ProtocolDraft),
    /// Update the profile-level path carrier only. Non-empty `ca_str` or
    /// `fingerprint` is rejected by adapters because they belong to a node
    /// draft and cannot be represented by this global list.
    TrustAnchors(TlsTrustParams),
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
    pub sub_rules: BTreeMap<String, Vec<rules::RuleEntry>>,
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

/// Typed product-level extensions of the neutral document inspection port.
///
/// Implementations must not read or write the profile store or contact a
/// running core: they transform bytes and return adapter-authored readback.
pub trait ProfileConfigurationCodecPort: ProfileDocumentCodecPort {
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

    /// Observe the current Mixin studio facts using this exact schema adapter.
    fn inspect_mixin_studio(
        &self,
        _identity: &ProfileDocumentIdentity,
        _document_bytes: &[u8],
        _mixin_yaml: &str,
    ) -> Result<ProfileMixinStudioObservation, PortError> {
        Err(PortError::unsupported(
            Capability::Profiles,
            "Mixin studio schema observation",
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
