//! Mixin fidelity support on SourceDoc.

use super::rules_fidelity::apply_rule_list;
use super::{SourceDoc, YamlEditError};
use crate::mixin::{MixinConfig, merge_profile_with_config};
use crate::rules::load_rules_from_yaml;
use serde_yaml_ng::Value;

/// Check if a mixin can be applied purely via [`SourceDoc`] text splicing.
pub fn can_apply_mixin_via_fidelity(mixin: &MixinConfig) -> bool {
    if mixin.dns.is_some()
        || mixin.tun.is_some()
        || mixin.sniffer.is_some()
        || mixin.proxies.is_some()
        || mixin.proxy_groups.is_some()
        || mixin.proxy_providers.is_some()
        || mixin.rule_providers.is_some()
        || mixin.custom_yaml.is_some()
    {
        return false;
    }

    true
}

/// Apply supported text edits and verify against the shared structural semantic owner.
pub fn apply_mixin_to_doc(doc: &mut SourceDoc, mixin: &MixinConfig) -> Result<(), YamlEditError> {
    let reference = merge_profile_with_config(&doc.render(), mixin)
        .map_err(|error| YamlEditError::Unsupported(error.to_string()))?;
    apply_mixin_with_reference(doc, mixin, &reference)
}

pub(crate) fn apply_mixin_with_reference(
    doc: &mut SourceDoc,
    mixin: &MixinConfig,
    reference: &str,
) -> Result<(), YamlEditError> {
    if !can_apply_mixin_via_fidelity(mixin) {
        return Err(YamlEditError::Unsupported(
            "complex mixin fields require full AST merge".into(),
        ));
    }

    let mut candidate = doc.clone();
    if let Some(mode) = &mixin.mode {
        candidate.set_top_scalar("mode", mode)?;
    }
    if let Some(log_level) = &mixin.log_level {
        candidate.set_top_scalar("log-level", log_level)?;
    }
    if let Some(ipv6) = mixin.ipv6 {
        candidate.set_top_scalar("ipv6", if ipv6 { "true" } else { "false" })?;
    }
    if let Some(allow_lan) = mixin.allow_lan {
        candidate.set_top_scalar("allow-lan", if allow_lan { "true" } else { "false" })?;
    }
    if let Some(mixed_port) = mixin.mixed_port {
        candidate.set_top_scalar("mixed-port", &mixed_port.to_string())?;
    }
    if let Some(secret) = &mixin.secret {
        candidate.set_top_scalar("secret", secret)?;
    }
    if let Some(external_controller) = &mixin.external_controller {
        candidate.set_top_scalar("external-controller", external_controller)?;
    }
    if let Some(external_ui) = &mixin.external_ui {
        candidate.set_top_scalar("external-ui", external_ui)?;
    }

    if mixin.rules.is_some() {
        let rules = load_rules_from_yaml(reference)
            .map_err(|error| YamlEditError::Unsupported(error.to_string()))?;
        apply_rule_list(&mut candidate, &rules)?;
    }
    let actual: Value = serde_yaml_ng::from_str(&candidate.render())
        .map_err(|error| YamlEditError::Unsupported(error.to_string()))?;
    let expected: Value = serde_yaml_ng::from_str(reference)
        .map_err(|error| YamlEditError::Unsupported(error.to_string()))?;
    if actual != expected {
        return Err(YamlEditError::Unsupported(
            "Mixin text edit differs from structural semantics".into(),
        ));
    }
    *doc = candidate;
    Ok(())
}
