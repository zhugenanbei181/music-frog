use crate::rule_workspace_test_support::rule_workspace_with_identity;
use infiltrator_contract::capability::Capability;
use infiltrator_contract::core_identity::EngineId;
use infiltrator_contract::dns::DnsSettingsPatch;
use infiltrator_contract::dns_hosts::validate_hosts;
use infiltrator_contract::error::{ErrorCode, Failure, FailureReason};
use infiltrator_contract::profile_document::DocumentValidation;
use infiltrator_contract::profile_document_identity::{
    ProfileDocumentIdentity, ProfileFormatId, ProfileSchemaVersion, ProfileWorkspaceIdentity,
};
use infiltrator_contract::provider_cache::KernelEtagSupportSnapshot;
use infiltrator_contract::subscription_import::SubscriptionFilterDraft;
use infiltrator_contract::yaml_snippets::{SnippetCaret, insert_at_caret};
use infiltrator_domain::config::validate_yaml;
use infiltrator_domain::filter::SubscriptionFilterPipeline;
use infiltrator_domain::filter_policy_form;
use infiltrator_domain::mixin::{MixinConfig, merge_profile_with_config_fidelity};
use infiltrator_domain::profile_options::{ProfileOptions, strip_rule_lines};
use infiltrator_domain::yaml_edit::rule_location::replace_rule_at_location;
use infiltrator_domain::{dns, dns_hosts, fake_ip, proxy_providers, rules, sniffer, tun};
use infiltrator_ports::error::PortError;
use infiltrator_ports::profile_document_codec::{
    ProfileConfigurationMutation, ProfileConfigurationQuery, ProfileConfigurationReadout,
    ProfileConfigurationUpdate, ProfileDocumentCaret, ProfileDocumentCodecPort,
    ProfileDocumentCodecScope, ProfileDocumentSnippetInsertion, ProfileDocumentSnippetOutcome,
    ProfileFilterMutation, ProfileMixinMutation, ProfileOptionsCodecScope,
    ProfileOptionsProjection, ProfileRuleWorkspaceMutation, ProfileRuleWorkspaceProjection,
    ProfileRuleWorkspaceUpdate,
};
use std::collections::VecDeque;
use std::fmt::Display;
use std::str::from_utf8;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

pub struct TestProfileDocumentCodec {
    scope: ProfileDocumentCodecScope,
    inspections: Arc<AtomicUsize>,
    inspection_fixtures: Arc<Mutex<VecDeque<DocumentValidation>>>,
    insertions: Arc<AtomicUsize>,
    options_calls: Arc<AtomicUsize>,
    configuration_reads: Arc<AtomicUsize>,
    configuration_updates: Arc<AtomicUsize>,
    configuration_readouts:
        Arc<Mutex<VecDeque<(ProfileConfigurationQuery, ProfileConfigurationReadout)>>>,
    configuration_mutations: Arc<Mutex<VecDeque<Result<ProfileConfigurationMutation, PortError>>>>,
}

impl TestProfileDocumentCodec {
    pub fn mihomo() -> Self {
        Self::for_scope("mihomo", "mihomo-yaml", 1)
    }

    pub fn for_scope(engine: &str, format: &str, schema: u32) -> Self {
        Self {
            scope: ProfileDocumentCodecScope {
                engine_id: EngineId::new(engine).expect("valid test engine ID"),
                format_id: ProfileFormatId::new(format).expect("valid test format ID"),
                schema_version: ProfileSchemaVersion::new(schema).expect("positive test schema"),
            },
            inspections: Arc::new(AtomicUsize::new(0)),
            inspection_fixtures: Arc::new(Mutex::new(VecDeque::new())),
            insertions: Arc::new(AtomicUsize::new(0)),
            options_calls: Arc::new(AtomicUsize::new(0)),
            configuration_reads: Arc::new(AtomicUsize::new(0)),
            configuration_updates: Arc::new(AtomicUsize::new(0)),
            configuration_readouts: Arc::new(Mutex::new(VecDeque::new())),
            configuration_mutations: Arc::new(Mutex::new(VecDeque::new())),
        }
    }

    pub fn inspection_count(&self) -> usize {
        self.inspections.load(Ordering::SeqCst)
    }

    pub fn queue_inspection(&self, validation: DocumentValidation) {
        self.inspection_fixtures
            .lock()
            .expect("test codec inspection fixtures")
            .push_back(validation);
    }

    pub fn insertion_count(&self) -> usize {
        self.insertions.load(Ordering::SeqCst)
    }

    pub fn options_call_count(&self) -> usize {
        self.options_calls.load(Ordering::SeqCst)
    }

    pub fn configuration_read_count(&self) -> usize {
        self.configuration_reads.load(Ordering::SeqCst)
    }

    pub fn configuration_update_count(&self) -> usize {
        self.configuration_updates.load(Ordering::SeqCst)
    }

    pub fn queue_configuration_readout(
        &self,
        query: ProfileConfigurationQuery,
        readout: ProfileConfigurationReadout,
    ) {
        self.configuration_readouts
            .lock()
            .expect("configuration readout fixtures")
            .push_back((query, readout));
    }

    pub fn queue_configuration_mutation(
        &self,
        mutation: Result<ProfileConfigurationMutation, PortError>,
    ) {
        self.configuration_mutations
            .lock()
            .expect("configuration mutation fixtures")
            .push_back(mutation);
    }
}

impl ProfileDocumentCodecPort for TestProfileDocumentCodec {
    fn scope(&self) -> ProfileDocumentCodecScope {
        self.scope.clone()
    }

    fn inspect(&self, raw_bytes: &[u8]) -> Result<DocumentValidation, PortError> {
        self.inspections.fetch_add(1, Ordering::SeqCst);
        from_utf8(raw_bytes).map_err(|error| PortError::Failed(error.to_string()))?;
        Ok(self
            .inspection_fixtures
            .lock()
            .expect("test codec inspection fixtures")
            .pop_front()
            .unwrap_or(DocumentValidation::Valid))
    }

    fn read_configuration(
        &self,
        _identity: &ProfileDocumentIdentity,
        raw_bytes: &[u8],
        query: ProfileConfigurationQuery,
    ) -> Result<ProfileConfigurationReadout, PortError> {
        self.configuration_reads.fetch_add(1, Ordering::SeqCst);
        let mut fixtures = self
            .configuration_readouts
            .lock()
            .expect("configuration readout fixtures");
        if let Some(index) = fixtures.iter().position(|(queued, _)| *queued == query) {
            return Ok(fixtures.remove(index).expect("located fixture").1);
        }
        drop(fixtures);
        fake_read_configuration(raw_bytes, query)
    }

    fn apply_configuration(
        &self,
        identity: &ProfileDocumentIdentity,
        raw_bytes: &[u8],
        update: ProfileConfigurationUpdate,
    ) -> Result<ProfileConfigurationMutation, PortError> {
        self.configuration_updates.fetch_add(1, Ordering::SeqCst);
        let mut fixtures = self
            .configuration_mutations
            .lock()
            .expect("configuration mutation fixtures");
        if let Some(mutation) = fixtures.pop_front() {
            return mutation;
        }
        drop(fixtures);
        fake_apply_configuration(identity, raw_bytes, update)
    }

    fn read_rule_workspace(
        &self,
        workspace: &ProfileWorkspaceIdentity,
        raw_bytes: &[u8],
    ) -> Result<ProfileRuleWorkspaceProjection, PortError> {
        self.configuration_reads.fetch_add(1, Ordering::SeqCst);
        let content = from_utf8(raw_bytes).map_err(|error| PortError::Failed(error.to_string()))?;
        let parsed = rule_workspace_with_identity(
            workspace.document.profile_id.as_str().to_owned(),
            content,
            workspace.clone(),
        )
        .map_err(PortError::Rejected)?;
        Ok(ProfileRuleWorkspaceProjection {
            source: parsed.source,
            rules: parsed.rules,
            targets: parsed.targets,
            sub_rules: parsed.sub_rules,
        })
    }

    fn apply_rule_workspace(
        &self,
        workspace: &ProfileWorkspaceIdentity,
        raw_bytes: &[u8],
        update: ProfileRuleWorkspaceUpdate,
    ) -> Result<ProfileRuleWorkspaceMutation, PortError> {
        let content = from_utf8(raw_bytes).map_err(|error| PortError::Failed(error.to_string()))?;
        let updated = match update {
            ProfileRuleWorkspaceUpdate::ReplaceRules(entries) => {
                rules::apply_rules_to_yaml(content, &entries).map_err(|error| {
                    PortError::Rejected(Failure::new(
                        ErrorCode::Configuration,
                        error.to_string(),
                        false,
                    ))
                })?
            }
            ProfileRuleWorkspaceUpdate::ReplaceNamedRule {
                location,
                expected_rule,
                replacement,
            } => replace_rule_at_location(content, &location, &expected_rule, &replacement)
                .map_err(|error| {
                    PortError::Rejected(Failure::new(ErrorCode::NotReady, error.to_string(), true))
                })?,
        };
        let projection = self.read_rule_workspace(workspace, updated.as_bytes())?;
        Ok(ProfileRuleWorkspaceMutation {
            document_bytes: updated.into_bytes(),
            readback: projection,
        })
    }

    fn insert_snippet(
        &self,
        content: &str,
        snippet_id: &str,
        caret: ProfileDocumentCaret,
    ) -> Result<ProfileDocumentSnippetOutcome, PortError> {
        self.insertions.fetch_add(1, Ordering::SeqCst);
        let previous = self.inspect(content.as_bytes())?;
        let insertion = insert_at_caret(
            content,
            snippet_id,
            SnippetCaret {
                line: caret.line,
                column: caret.column,
            },
        )
        .map_err(|error| PortError::Failed(error.to_string()))?;
        let validation = self.inspect(insertion.content.as_bytes())?;
        if matches!(&previous, DocumentValidation::Valid)
            && matches!(&validation, DocumentValidation::Invalid { .. })
        {
            return Ok(ProfileDocumentSnippetOutcome::Rejected { validation });
        }
        Ok(ProfileDocumentSnippetOutcome::Inserted(
            ProfileDocumentSnippetInsertion {
                snippet_id: insertion.snippet_id.to_owned(),
                content: insertion.content,
                cursor_line: insertion.cursor_line,
                cursor_column: insertion.cursor_column,
                validation,
            },
        ))
    }

    fn options_scope(&self) -> Result<ProfileOptionsCodecScope, PortError> {
        Ok(ProfileOptionsCodecScope {
            document: self.scope.clone(),
            sidecar_format_id: ProfileFormatId::new("test-profile-options-yaml")
                .expect("test options format"),
            sidecar_schema_version: ProfileSchemaVersion::new(1).expect("test schema"),
        })
    }

    fn inspect_options(
        &self,
        _identity: &ProfileDocumentIdentity,
        sidecar_bytes: Option<&[u8]>,
    ) -> Result<ProfileOptionsProjection, PortError> {
        self.options_calls.fetch_add(1, Ordering::SeqCst);
        project_options(&decode_options(sidecar_bytes)?)
    }

    fn apply_mixin(
        &self,
        _identity: &ProfileDocumentIdentity,
        document_bytes: &[u8],
        sidecar_bytes: Option<&[u8]>,
        mixin_yaml: &str,
    ) -> Result<ProfileMixinMutation, PortError> {
        self.options_calls.fetch_add(1, Ordering::SeqCst);
        let old = decode_options(sidecar_bytes)?;
        let mixin: MixinConfig = serde_yaml_ng::from_str(mixin_yaml).map_err(|error| {
            PortError::Rejected(
                Failure::new(ErrorCode::InvalidInput, error.to_string(), false)
                    .with_reason(FailureReason::MixinYaml),
            )
        })?;
        let removals = old
            .mixin
            .rules
            .iter()
            .flat_map(|rules| rules.prepend.iter().chain(rules.append.iter()).cloned())
            .collect::<Vec<_>>();
        let content =
            from_utf8(document_bytes).map_err(|error| PortError::Io(error.to_string()))?;
        let base = strip_rule_lines(content, &removals);
        let merged = merge_profile_with_config_fidelity(&base, &mixin)
            .map_err(|error| PortError::Failed(error.to_string()))?;
        validate_yaml(&merged).map_err(|error| PortError::Failed(error.to_string()))?;
        let options = ProfileOptions {
            mixin,
            filter: old.filter,
        };
        Ok(ProfileMixinMutation {
            document_bytes: merged.into_bytes(),
            sidecar_bytes: encode_options(sidecar_bytes, &options)?,
            projection: project_options(&options)?,
        })
    }

    fn apply_filter(
        &self,
        _identity: &ProfileDocumentIdentity,
        document_bytes: &[u8],
        sidecar_bytes: Option<&[u8]>,
        draft: &SubscriptionFilterDraft,
    ) -> Result<ProfileFilterMutation, PortError> {
        self.options_calls.fetch_add(1, Ordering::SeqCst);
        let old = decode_options(sidecar_bytes)?;
        let parsed = filter_policy_form::filter_spec_from_draft(draft).map_err(|error| {
            PortError::Rejected(Failure::new(
                ErrorCode::InvalidInput,
                error.to_string(),
                false,
            ))
        })?;
        let spec = filter_policy_form::filter_spec_with_form_fields(
            old.filter.as_ref(),
            parsed,
            draft.advanced_policy.is_some(),
        );
        let content =
            from_utf8(document_bytes).map_err(|error| PortError::Io(error.to_string()))?;
        let (filtered, report) = SubscriptionFilterPipeline::new(
            spec.to_rule()
                .map_err(|error| PortError::Failed(error.to_string()))?,
        )
        .apply_to_yaml(content)
        .map_err(|error| PortError::Failed(error.to_string()))?;
        validate_yaml(&filtered).map_err(|error| PortError::Failed(error.to_string()))?;
        let options = ProfileOptions {
            mixin: old.mixin,
            filter: (!spec.is_empty()).then_some(spec),
        };
        Ok(ProfileFilterMutation {
            document_bytes: filtered.into_bytes(),
            sidecar_bytes: encode_options(sidecar_bytes, &options)?,
            projection: project_options(&options)?,
            report,
        })
    }
}

fn decode_options(bytes: Option<&[u8]>) -> Result<ProfileOptions, PortError> {
    let Some(bytes) = bytes else {
        return Ok(ProfileOptions::default());
    };
    let text = from_utf8(bytes).map_err(|error| PortError::Io(error.to_string()))?;
    if text.trim().is_empty() {
        return Ok(ProfileOptions::default());
    }
    serde_yaml_ng::from_str(text).map_err(|error| PortError::Failed(error.to_string()))
}

fn encode_options(
    current: Option<&[u8]>,
    options: &ProfileOptions,
) -> Result<Option<Vec<u8>>, PortError> {
    if decode_options(current)? == *options {
        return Ok(current.map(<[u8]>::to_vec));
    }
    if options.is_empty() {
        return Ok(None);
    }
    serde_yaml_ng::to_string(options)
        .map(|value| Some(value.into_bytes()))
        .map_err(|error| PortError::Failed(error.to_string()))
}

fn project_options(options: &ProfileOptions) -> Result<ProfileOptionsProjection, PortError> {
    let mixin_yaml = serde_yaml_ng::to_string(&options.mixin)
        .map_err(|error| PortError::Failed(error.to_string()))?;
    let filter = options
        .filter
        .as_ref()
        .map(filter_policy_form::filter_spec_to_draft)
        .transpose()
        .map_err(|error| PortError::Failed(error.to_string()))?
        .unwrap_or_default();
    Ok(ProfileOptionsProjection { mixin_yaml, filter })
}

fn fake_read_configuration(
    raw_bytes: &[u8],
    query: ProfileConfigurationQuery,
) -> Result<ProfileConfigurationReadout, PortError> {
    if matches!(
        &query,
        ProfileConfigurationQuery::ProtocolNodes | ProfileConfigurationQuery::TrustAnchors
    ) {
        return Err(PortError::unsupported(
            Capability::Profiles,
            "test fake typed protocol and CA readouts must be queued explicitly",
        ));
    }
    let content = from_utf8(raw_bytes).map_err(|error| PortError::Failed(error.to_string()))?;
    let document: serde_yaml_ng::Value =
        serde_yaml_ng::from_str(content).map_err(|error| PortError::Failed(error.to_string()))?;
    match query {
        ProfileConfigurationQuery::Dns => Ok(ProfileConfigurationReadout::Dns(
            dns::extract_dns_config_from_doc(&document).map_err(configuration_failure)?,
        )),
        ProfileConfigurationQuery::FakeIp => Ok(ProfileConfigurationReadout::FakeIp(
            fake_ip::extract_fake_ip_config_from_doc(&document).map_err(configuration_failure)?,
        )),
        ProfileConfigurationQuery::Tun => Ok(ProfileConfigurationReadout::Tun(
            tun::extract_tun_config_from_doc(&document).map_err(configuration_failure)?,
        )),
        ProfileConfigurationQuery::Rules => Ok(ProfileConfigurationReadout::Rules(
            rules::load_rules_from_yaml(content).map_err(configuration_failure)?,
        )),
        ProfileConfigurationQuery::RuleProviders => Ok(ProfileConfigurationReadout::RuleProviders(
            rules::extract_rule_providers_from_doc(&document).map_err(configuration_failure)?,
        )),
        ProfileConfigurationQuery::ProxyProviders => {
            Ok(ProfileConfigurationReadout::ProxyProviders(
                proxy_providers::extract_proxy_providers_from_doc(&document)
                    .map_err(configuration_failure)?,
            ))
        }
        ProfileConfigurationQuery::Sniffer => Ok(ProfileConfigurationReadout::Sniffer(
            sniffer::extract_sniffer_config(&document).map_err(configuration_failure)?,
        )),
        ProfileConfigurationQuery::ProxyEndpoint => {
            let endpoint = [
                ("mixed-port", "socks5"),
                ("socks-port", "socks5"),
                ("port", "http"),
            ]
            .into_iter()
            .find_map(|(key, scheme)| {
                document.get(key).and_then(|value| {
                    let port = match value {
                        serde_yaml_ng::Value::Number(number) => {
                            number.as_u64().and_then(|port| u16::try_from(port).ok())
                        }
                        serde_yaml_ng::Value::String(raw) => raw.trim().parse::<u16>().ok(),
                        _ => None,
                    }?;
                    (port > 0).then(|| format!("{scheme}://127.0.0.1:{port}"))
                })
            });
            Ok(ProfileConfigurationReadout::ProxyEndpoint(endpoint))
        }
        ProfileConfigurationQuery::EtagSupport => Ok(ProfileConfigurationReadout::EtagSupport(
            KernelEtagSupportSnapshot::from_declared(
                document
                    .get("etag-support")
                    .and_then(serde_yaml_ng::Value::as_bool),
            ),
        )),
        ProfileConfigurationQuery::Hosts => Ok(ProfileConfigurationReadout::Hosts {
            entries: dns_hosts::entries_from_document(&document, false)
                .map_err(configuration_failure)?,
            legacy_entries: dns_hosts::entries_from_document(&document, true)
                .map_err(configuration_failure)?,
        }),
        ProfileConfigurationQuery::ProtocolNodes | ProfileConfigurationQuery::TrustAnchors => {
            unreachable!("typed protocol and CA queries were rejected above")
        }
    }
}

fn fake_apply_configuration(
    identity: &ProfileDocumentIdentity,
    raw_bytes: &[u8],
    update: ProfileConfigurationUpdate,
) -> Result<ProfileConfigurationMutation, PortError> {
    if matches!(
        &update,
        ProfileConfigurationUpdate::UpsertProtocolNode(_)
            | ProfileConfigurationUpdate::TrustAnchors(_)
    ) {
        return Err(PortError::unsupported(
            Capability::Profiles,
            "test fake typed protocol and CA mutations must be queued explicitly",
        ));
    }
    let content = from_utf8(raw_bytes).map_err(|error| PortError::Failed(error.to_string()))?;
    serde_yaml_ng::from_str::<serde_yaml_ng::Value>(content)
        .map_err(|error| PortError::Failed(error.to_string()))?;
    let (output, query) = match update {
        ProfileConfigurationUpdate::Dns(patch) => (
            dns::apply_dns_patch_to_yaml(content, patch).map_err(configuration_failure)?,
            ProfileConfigurationQuery::Dns,
        ),
        ProfileConfigurationUpdate::DnsSettings(patch) => (
            fake_apply_dns_settings(content, patch)?,
            ProfileConfigurationQuery::Dns,
        ),
        ProfileConfigurationUpdate::FakeIp(patch) => (
            fake_ip::apply_fake_ip_patch_to_yaml(content, patch).map_err(configuration_failure)?,
            ProfileConfigurationQuery::FakeIp,
        ),
        ProfileConfigurationUpdate::Tun(patch) => (
            tun::apply_tun_patch_to_yaml(content, patch).map_err(configuration_failure)?,
            ProfileConfigurationQuery::Tun,
        ),
        ProfileConfigurationUpdate::Rules(entries) => {
            rules::validate_rules(&entries).map_err(invalid_configuration)?;
            (
                rules::apply_rules_to_yaml(content, &entries).map_err(configuration_failure)?,
                ProfileConfigurationQuery::Rules,
            )
        }
        ProfileConfigurationUpdate::RuleProviders(providers) => (
            rules::apply_rule_providers_to_yaml(content, &providers)
                .map_err(configuration_failure)?,
            ProfileConfigurationQuery::RuleProviders,
        ),
        ProfileConfigurationUpdate::ProxyProviders(providers) => (
            proxy_providers::apply_proxy_providers_to_yaml(content, &providers)
                .map_err(configuration_failure)?,
            ProfileConfigurationQuery::ProxyProviders,
        ),
        ProfileConfigurationUpdate::Sniffer(config) => (
            sniffer::apply_typed_sniffer_to_yaml(content, &config)
                .map_err(configuration_failure)?,
            ProfileConfigurationQuery::Sniffer,
        ),
        ProfileConfigurationUpdate::UpsertProtocolNode(_) => {
            return Err(PortError::unsupported(
                Capability::Profiles,
                "test fake protocol node mutation must be queued explicitly",
            ));
        }
        ProfileConfigurationUpdate::TrustAnchors(_) => {
            return Err(PortError::unsupported(
                Capability::Profiles,
                "test fake CA trust-anchor mutation must be queued explicitly",
            ));
        }
    };
    let document_bytes = output.into_bytes();
    let readback = fake_read_configuration(&document_bytes, query)?;
    let _ = identity;
    Ok(ProfileConfigurationMutation {
        document_bytes,
        readback,
    })
}

fn fake_apply_dns_settings(content: &str, patch: DnsSettingsPatch) -> Result<String, PortError> {
    if patch.clear_hosts && patch.hosts.is_some()
        || patch.remove_legacy_hosts && patch.hosts.is_none() && !patch.clear_hosts
    {
        return Err(invalid_configuration("ambiguous Hosts patch"));
    }
    if let Some(rows) = &patch.hosts
        && let Some(issue) = validate_hosts(rows).first()
    {
        return Err(invalid_configuration(format!(
            "invalid Hosts mapping: {issue:?}"
        )));
    }
    let hosts = if patch.clear_hosts {
        Some(Vec::new())
    } else {
        patch.hosts.clone()
    };
    let remove_legacy = patch.remove_legacy_hosts;
    let dns_patch = fake_dns_patch_from_settings(patch);
    let mut output = if dns_patch == dns::DnsConfigPatch::default() {
        content.to_owned()
    } else {
        dns::apply_dns_patch_to_yaml(content, dns_patch).map_err(configuration_failure)?
    };
    if let Some(hosts) = hosts {
        output = dns_hosts::apply_hosts_patch_to_yaml(&output, &hosts, remove_legacy)
            .map_err(configuration_failure)?;
    }
    Ok(output)
}

fn fake_dns_patch_from_settings(patch: DnsSettingsPatch) -> dns::DnsConfigPatch {
    let mut output = dns::DnsConfigPatch::default();
    if let Some(switches) = patch.switches {
        output.enable = Some(switches.enable);
        output.ipv6 = Some(switches.ipv6);
        output.cache = Some(switches.cache);
        output.use_hosts = Some(switches.use_hosts);
        output.use_system_hosts = Some(switches.use_system_hosts);
        output.respect_rules = Some(switches.respect_rules);
    }
    if let Some(mode) = patch.enhanced_mode {
        match mode.config_value() {
            Some(value) => output.enhanced_mode = Some(value.into()),
            None => output.clear_enhanced_mode = true,
        }
    }
    if let Some(mode) = patch.filter_mode {
        output.fake_ip_filter_mode = Some(mode.config_value().into());
    }
    output.nameserver = patch.nameserver;
    output.fallback = patch.fallback;
    if let Some(policy) = patch.fallback_policy {
        output.fallback_filter_partial = Some(dns::FallbackFilterPatch {
            geoip: Some(policy.geoip),
            geoip_code: (!policy.geoip_code.is_empty()).then_some(policy.geoip_code),
            ipcidr: Some(policy.trigger_ipcidr),
        });
    }
    output.fake_ip_range = patch.fake_ip_range;
    output.clear_fake_ip_range = patch.clear_fake_ip_range;
    output.fake_ip_filter = patch.fake_ip_filter;
    output.proxy_server_nameserver = patch.proxy_server_nameserver;
    output.direct_nameserver = patch.direct_nameserver;
    output
}

fn invalid_configuration(error: impl Display) -> PortError {
    PortError::Rejected(Failure::new(
        ErrorCode::InvalidInput,
        error.to_string(),
        false,
    ))
}

fn configuration_failure(error: impl Display) -> PortError {
    PortError::Rejected(Failure::new(
        ErrorCode::Configuration,
        error.to_string(),
        false,
    ))
}
