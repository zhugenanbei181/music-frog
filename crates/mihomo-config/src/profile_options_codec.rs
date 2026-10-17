//! Mihomo-owned Mixin/filter sidecar schema operations.

use crate::profile_document_codec::{PROFILE_FORMAT, PROFILE_SCHEMA_VERSION};
use crate::profile_option_store;
use infiltrator_contract::profile_mixin_studio::{
    ProfileMixinCascade, ProfileMixinCascadeStage, ProfileMixinCascadeStageId, ProfileMixinColumn,
    ProfileMixinColumnRole, ProfileMixinColumns, ProfileMixinComposition, ProfileMixinPreflight,
    ProfileMixinStageDeclaration, ProfileMixinStudioObservation, ProfileMixinToggle,
    ProfileMixinToggleId, ProfileMixinToggleObservation,
};
use infiltrator_contract::subscription_filter_result::FilterReport;
use infiltrator_contract::subscription_import::SubscriptionFilterDraft;
use infiltrator_domain::filter::SubscriptionFilterPipeline;
use infiltrator_domain::filter_policy_form;
use infiltrator_domain::mixin::{
    CascadeOverlayPipeline, MixinConfig, merge_profile_with_config_fidelity,
};
use infiltrator_domain::profile_options::{ProfileOptions, compose_content, strip_rule_lines};
use infiltrator_ports::profile_document_codec::{
    ProfileFilterMutation, ProfileMixinMutation, ProfileOptionsCodecScope, ProfileOptionsProjection,
};
use lurker_core_contract::capability::Capability;
use lurker_core_contract::error::{ErrorCode, Failure, FailureReason};
use lurker_core_contract::profile_document_identity::ProfileDocumentIdentity;
use lurker_core_contract::profile_document_identity::{ProfileFormatId, ProfileSchemaVersion};
use lurker_core_ports::error::PortError;
use lurker_core_ports::profile_document_codec::ProfileDocumentCodecScope;
use std::fmt::Display;
use std::path::Path;
use std::str;
use yaml_rust2::YamlLoader;

pub const SIDECAR_FORMAT: &str = "mihomo-profile-options-yaml";
pub const SIDECAR_SCHEMA_VERSION: u32 = 1;

pub fn scope(document: ProfileDocumentCodecScope) -> ProfileOptionsCodecScope {
    ProfileOptionsCodecScope {
        document,
        sidecar_format_id: ProfileFormatId::new(SIDECAR_FORMAT)
            .expect("static Mihomo options format ID is valid"),
        sidecar_schema_version: ProfileSchemaVersion::new(SIDECAR_SCHEMA_VERSION)
            .expect("Mihomo options schema version is positive"),
    }
}

pub fn inspect(
    identity: &ProfileDocumentIdentity,
    sidecar_bytes: Option<&[u8]>,
) -> Result<ProfileOptionsProjection, PortError> {
    ensure_document_scope(identity)?;
    project(&decode(sidecar_bytes)?)
}

pub fn apply_mixin(
    identity: &ProfileDocumentIdentity,
    document_bytes: &[u8],
    sidecar_bytes: Option<&[u8]>,
    mixin_yaml: &str,
) -> Result<ProfileMixinMutation, PortError> {
    ensure_document_scope(identity)?;
    let old = decode(sidecar_bytes)?;
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
    let content = str::from_utf8(document_bytes).map_err(|error| {
        PortError::Rejected(Failure::new(
            ErrorCode::Storage,
            format!("Mihomo profile document is not UTF-8: {error}"),
            false,
        ))
    })?;
    let base = strip_rule_lines(content, &removals);
    let merged = merge_profile_with_config_fidelity(&base, &mixin).map_err(|error| {
        PortError::Rejected(Failure::new(
            ErrorCode::Configuration,
            error.to_string(),
            false,
        ))
    })?;
    validate_mihomo_yaml(&merged)?;
    let options = ProfileOptions {
        mixin,
        filter: old.filter,
    };
    let sidecar_bytes = encode_if_changed(sidecar_bytes, &options)?;
    Ok(ProfileMixinMutation {
        document_bytes: merged.into_bytes(),
        sidecar_bytes,
        projection: project(&options)?,
    })
}

pub fn inspect_mixin_toggle(
    identity: &ProfileDocumentIdentity,
    mixin_yaml: &str,
    toggle_id: ProfileMixinToggleId,
) -> Result<bool, PortError> {
    ensure_document_scope(identity)?;
    if mixin_yaml.trim().is_empty() {
        return Ok(false);
    }
    let config = parse_mixin_yaml(mixin_yaml).map_err(invalid_input)?;
    Ok(mixin_toggle_enabled(&config, toggle_id))
}

pub fn set_mixin_toggle(
    identity: &ProfileDocumentIdentity,
    mixin_yaml: &str,
    toggle_id: ProfileMixinToggleId,
    enabled: bool,
) -> Result<String, PortError> {
    ensure_document_scope(identity)?;
    let mut config = if mixin_yaml.trim().is_empty() {
        MixinConfig::default()
    } else {
        parse_mixin_yaml(mixin_yaml).map_err(invalid_input)?
    };
    apply_mixin_toggle(&mut config, toggle_id, enabled);
    serde_yaml_ng::to_string(&config).map_err(invalid_input)
}

pub fn inspect_mixin_studio(
    identity: &ProfileDocumentIdentity,
    document_bytes: &[u8],
    mixin_yaml: &str,
) -> Result<ProfileMixinStudioObservation, PortError> {
    ensure_document_scope(identity)?;
    let preflight = match apply_mixin(identity, document_bytes, None, mixin_yaml) {
        Ok(_) => ProfileMixinPreflight::Accepted,
        Err(error)
            if matches!(
                error.error_code(),
                ErrorCode::InvalidInput | ErrorCode::Configuration
            ) =>
        {
            ProfileMixinPreflight::Blocked {
                reason: error.to_string(),
            }
        }
        Err(error) => return Err(error),
    };
    let base = str::from_utf8(document_bytes).map_err(|error| {
        PortError::Rejected(Failure::new(
            ErrorCode::Storage,
            format!("Mihomo profile document is not UTF-8: {error}"),
            false,
        ))
    })?;
    let review = review_mixin_cascade(base, mixin_yaml);
    let (cascade, composed) = match review {
        MixinCascadeReview::Merged { stages, output } => {
            let line_count = output.lines().count();
            (
                ProfileMixinCascade::Merged { stages, line_count },
                ProfileMixinComposition::Produced(mixin_column(
                    ProfileMixinColumnRole::Composed,
                    output,
                    false,
                )),
            )
        }
        MixinCascadeReview::Blocked { stages, reason } => (
            ProfileMixinCascade::Blocked {
                stages,
                reason: reason.clone(),
            },
            ProfileMixinComposition::Blocked { reason },
        ),
    };
    let columns = ProfileMixinColumns {
        base: mixin_column(ProfileMixinColumnRole::Base, base.to_owned(), false),
        overlay: mixin_column(ProfileMixinColumnRole::Overlay, mixin_yaml.to_owned(), true),
        composed,
    };
    let toggles = match parse_mixin_yaml(mixin_yaml) {
        Ok(config) => ProfileMixinToggleObservation::Available(
            ProfileMixinToggleId::ALL
                .into_iter()
                .map(|id| ProfileMixinToggle {
                    id,
                    enabled: mixin_toggle_enabled(&config, id),
                })
                .collect(),
        ),
        Err(error) => ProfileMixinToggleObservation::Unavailable {
            reason: error.to_string(),
        },
    };
    Ok(ProfileMixinStudioObservation {
        preflight,
        cascade,
        columns,
        toggles,
    })
}

fn parse_mixin_yaml(text: &str) -> Result<MixinConfig, String> {
    if text.trim().is_empty() {
        return Ok(MixinConfig::default());
    }
    serde_yaml_ng::from_str(text).map_err(|error| error.to_string())
}

fn mixin_toggle_enabled(config: &MixinConfig, id: ProfileMixinToggleId) -> bool {
    match id {
        ProfileMixinToggleId::Ipv6 => config.ipv6 == Some(true),
        ProfileMixinToggleId::AllowLan => config.allow_lan == Some(true),
        ProfileMixinToggleId::DnsFakeIp => config.dns.is_some(),
        ProfileMixinToggleId::Tun => config.tun.is_some(),
        ProfileMixinToggleId::Sniffer => config.sniffer.is_some(),
        ProfileMixinToggleId::DebugLog => config.log_level.as_deref() == Some("debug"),
    }
}

fn apply_mixin_toggle(config: &mut MixinConfig, id: ProfileMixinToggleId, enabled: bool) {
    match id {
        ProfileMixinToggleId::Ipv6 => config.ipv6 = Some(enabled),
        ProfileMixinToggleId::AllowLan => config.allow_lan = Some(enabled),
        ProfileMixinToggleId::DnsFakeIp => {
            config.dns = enabled.then(|| {
                serde_yaml_ng::from_str(
                    "enable: true\nenhanced-mode: fake-ip\nfake-ip-range: 198.18.0.1/16\nnameserver:\n  - 223.5.5.5\n  - 119.29.29.29\n",
                )
                .expect("static DNS preset is valid YAML")
            });
        }
        ProfileMixinToggleId::Tun => {
            config.tun = enabled.then(|| {
                serde_yaml_ng::from_str("enable: true\nstack: system\nauto-route: true\n")
                    .expect("static TUN preset is valid YAML")
            });
        }
        ProfileMixinToggleId::Sniffer => {
            config.sniffer = enabled.then(|| {
                serde_yaml_ng::from_str("enable: true\n")
                    .expect("static sniffer preset is valid YAML")
            });
        }
        ProfileMixinToggleId::DebugLog => {
            config.log_level = enabled.then(|| "debug".to_owned());
        }
    }
}

enum MixinCascadeReview {
    Merged {
        stages: Vec<ProfileMixinCascadeStage>,
        output: String,
    },
    Blocked {
        stages: Vec<ProfileMixinCascadeStage>,
        reason: String,
    },
}

fn review_mixin_cascade(base: &str, mixin_yaml: &str) -> MixinCascadeReview {
    let pre_mixin = if mixin_yaml.trim().is_empty() {
        None
    } else {
        match parse_mixin_yaml(mixin_yaml) {
            Ok(config) => Some(config),
            Err(error) => {
                return MixinCascadeReview::Blocked {
                    stages: Vec::new(),
                    reason: format!("Mixin overlay is not valid YAML: {error}"),
                };
            }
        }
    };
    if !base.trim().is_empty() {
        if let Err(error) = YamlLoader::load_from_str(base) {
            return MixinCascadeReview::Blocked {
                stages: Vec::new(),
                reason: format!("Base document is not valid YAML: {error}"),
            };
        }
    }

    let mut stages = Vec::with_capacity(5);
    let mut pipeline = CascadeOverlayPipeline::new().with_base_profile(base.to_owned());
    if let Err(reason) = append_mixin_stage(
        &pipeline,
        &mut stages,
        ProfileMixinCascadeStageId::Base,
        ProfileMixinStageDeclaration::Applied,
    ) {
        return MixinCascadeReview::Blocked { stages, reason };
    }

    if let Err(reason) = append_mixin_stage(
        &pipeline,
        &mut stages,
        ProfileMixinCascadeStageId::Subscription,
        ProfileMixinStageDeclaration::Undeclared,
    ) {
        return MixinCascadeReview::Blocked { stages, reason };
    }
    if let Err(reason) = append_mixin_stage(
        &pipeline,
        &mut stages,
        ProfileMixinCascadeStageId::Merge,
        ProfileMixinStageDeclaration::Undeclared,
    ) {
        return MixinCascadeReview::Blocked { stages, reason };
    }

    if let Some(config) = pre_mixin {
        pipeline = pipeline.with_pre_mixin(config);
    }
    if let Err(reason) = append_mixin_stage(
        &pipeline,
        &mut stages,
        ProfileMixinCascadeStageId::PreMixin,
        if mixin_yaml.trim().is_empty() {
            ProfileMixinStageDeclaration::Undeclared
        } else {
            ProfileMixinStageDeclaration::Applied
        },
    ) {
        return MixinCascadeReview::Blocked { stages, reason };
    }
    match append_mixin_stage(
        &pipeline,
        &mut stages,
        ProfileMixinCascadeStageId::PostMixin,
        ProfileMixinStageDeclaration::Undeclared,
    ) {
        Ok(output) => MixinCascadeReview::Merged { stages, output },
        Err(reason) => MixinCascadeReview::Blocked { stages, reason },
    }
}

fn append_mixin_stage(
    pipeline: &CascadeOverlayPipeline,
    stages: &mut Vec<ProfileMixinCascadeStage>,
    id: ProfileMixinCascadeStageId,
    declaration: ProfileMixinStageDeclaration,
) -> Result<String, String> {
    let output = pipeline.execute().map_err(|error| error.to_string())?;
    stages.push(ProfileMixinCascadeStage {
        id,
        declaration,
        line_count: output.lines().count(),
    });
    Ok(output)
}

fn mixin_column(
    role: ProfileMixinColumnRole,
    content: String,
    editable: bool,
) -> ProfileMixinColumn {
    ProfileMixinColumn {
        role,
        line_count: content.lines().count(),
        content,
        editable,
    }
}

fn validate_mihomo_yaml(content: &str) -> Result<(), PortError> {
    if content.trim().is_empty() {
        return Err(configuration_error("配置内容不能为空"));
    }
    YamlLoader::load_from_str(content)
        .map(|_| ())
        .map_err(configuration_error)
}

pub fn apply_filter(
    identity: &ProfileDocumentIdentity,
    document_bytes: &[u8],
    sidecar_bytes: Option<&[u8]>,
    draft: &SubscriptionFilterDraft,
) -> Result<ProfileFilterMutation, PortError> {
    ensure_document_scope(identity)?;
    let old = decode(sidecar_bytes)?;
    let parsed = filter_policy_form::filter_spec_from_draft(draft).map_err(invalid_input)?;
    let spec = filter_policy_form::filter_spec_with_form_fields(
        old.filter.as_ref(),
        parsed,
        draft.advanced_policy.is_some(),
    );
    let content = str::from_utf8(document_bytes).map_err(|error| {
        PortError::Rejected(Failure::new(
            ErrorCode::Storage,
            format!("Mihomo profile document is not UTF-8: {error}"),
            false,
        ))
    })?;
    let (filtered, report) =
        SubscriptionFilterPipeline::new(spec.to_rule().map_err(configuration_error)?)
            .apply_to_yaml(content)
            .map_err(configuration_error)?;
    validate_mihomo_yaml(&filtered)?;
    let options = ProfileOptions {
        mixin: old.mixin,
        filter: (!spec.is_empty()).then_some(spec),
    };
    let sidecar_bytes = encode_if_changed(sidecar_bytes, &options)?;
    Ok(ProfileFilterMutation {
        document_bytes: filtered.into_bytes(),
        sidecar_bytes,
        projection: project(&options)?,
        report,
    })
}

/// Compose stored options onto newly fetched Mihomo subscription content.
/// Core callers never decode the Mihomo sidecar schema themselves.
pub async fn apply_saved(
    config_dir: &Path,
    profile: &str,
    content: &str,
) -> Result<(String, Option<FilterReport>), PortError> {
    let sidecar_bytes = profile_option_store::read_options_bytes(config_dir, profile).await?;
    let options = decode(sidecar_bytes.as_deref())?;
    compose_content(content, &options).map_err(configuration_error)
}

fn ensure_document_scope(identity: &ProfileDocumentIdentity) -> Result<(), PortError> {
    if identity.engine_id != crate::engine_id()
        || identity.format_id.as_str() != PROFILE_FORMAT
        || identity.schema_version.value() != PROFILE_SCHEMA_VERSION
    {
        return Err(PortError::unsupported(
            Capability::Profiles,
            format!(
                "Mihomo options codec does not support engine `{}`, format `{}`, schema {}",
                identity.engine_id.as_str(),
                identity.format_id.as_str(),
                identity.schema_version.value()
            ),
        ));
    }
    Ok(())
}

pub(crate) fn decode(sidecar_bytes: Option<&[u8]>) -> Result<ProfileOptions, PortError> {
    let Some(sidecar_bytes) = sidecar_bytes else {
        return Ok(ProfileOptions::default());
    };
    let text = str::from_utf8(sidecar_bytes).map_err(|error| {
        PortError::Rejected(Failure::new(ErrorCode::Storage, error.to_string(), false))
    })?;
    if text.trim().is_empty() {
        return Ok(ProfileOptions::default());
    }
    serde_yaml_ng::from_str(text).map_err(configuration_error)
}

pub(crate) fn encode(options: &ProfileOptions) -> Result<Option<Vec<u8>>, PortError> {
    if options.is_empty() {
        return Ok(None);
    }
    serde_yaml_ng::to_string(options)
        .map(|document| Some(document.into_bytes()))
        .map_err(configuration_error)
}

fn encode_if_changed(
    current_bytes: Option<&[u8]>,
    options: &ProfileOptions,
) -> Result<Option<Vec<u8>>, PortError> {
    if decode(current_bytes)? == *options {
        return Ok(current_bytes.map(<[u8]>::to_vec));
    }
    encode(options)
}

fn project(options: &ProfileOptions) -> Result<ProfileOptionsProjection, PortError> {
    let mixin_yaml = serde_yaml_ng::to_string(&options.mixin).map_err(configuration_error)?;
    let filter = options
        .filter
        .as_ref()
        .map(filter_policy_form::filter_spec_to_draft)
        .transpose()
        .map_err(configuration_error)?
        .unwrap_or_default();
    Ok(ProfileOptionsProjection { mixin_yaml, filter })
}

fn invalid_input(error: impl Display) -> PortError {
    PortError::Rejected(Failure::new(
        ErrorCode::InvalidInput,
        error.to_string(),
        false,
    ))
}

fn configuration_error(error: impl Display) -> PortError {
    PortError::Rejected(Failure::new(
        ErrorCode::Configuration,
        error.to_string(),
        false,
    ))
}

#[cfg(test)]
#[path = "profile_options_codec_tests.rs"]
mod tests;
