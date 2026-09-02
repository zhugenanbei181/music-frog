//! Mixin fidelity support on SourceDoc.

use super::{SourceDoc, YamlEditError};

/// Check if a mixin can be applied purely via [`SourceDoc`] text splicing.
pub fn can_apply_mixin_via_fidelity(mixin: &crate::mixin::MixinConfig) -> bool {
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

    if let Some(rules) = &mixin.rules
        && (!rules.replace.is_empty() || !rules.overrides.is_empty() || !rules.prepend.is_empty())
    {
        return false;
    }

    true
}

/// Apply compatible mixin settings onto a [`SourceDoc`] in place.
pub fn apply_mixin_to_doc(
    doc: &mut SourceDoc,
    mixin: &crate::mixin::MixinConfig,
) -> Result<(), YamlEditError> {
    if !can_apply_mixin_via_fidelity(mixin) {
        return Err(YamlEditError::Unsupported(
            "complex mixin fields require full AST merge".into(),
        ));
    }

    if let Some(mode) = &mixin.mode {
        doc.set_top_scalar("mode", mode)?;
    }
    if let Some(log_level) = &mixin.log_level {
        doc.set_top_scalar("log-level", log_level)?;
    }
    if let Some(ipv6) = mixin.ipv6 {
        doc.set_top_scalar("ipv6", if ipv6 { "true" } else { "false" })?;
    }
    if let Some(allow_lan) = mixin.allow_lan {
        doc.set_top_scalar("allow-lan", if allow_lan { "true" } else { "false" })?;
    }
    if let Some(mixed_port) = mixin.mixed_port {
        doc.set_top_scalar("mixed-port", &mixed_port.to_string())?;
    }
    if let Some(secret) = &mixin.secret {
        doc.set_top_scalar("secret", secret)?;
    }
    if let Some(external_controller) = &mixin.external_controller {
        doc.set_top_scalar("external-controller", external_controller)?;
    }
    if let Some(external_ui) = &mixin.external_ui {
        doc.set_top_scalar("external-ui", external_ui)?;
    }

    if let Some(rules) = &mixin.rules {
        for del in &rules.delete {
            let _ = doc.remove_rule(del);
        }
        for app in &rules.append {
            doc.append_rule(app)?;
        }
    }

    Ok(())
}
