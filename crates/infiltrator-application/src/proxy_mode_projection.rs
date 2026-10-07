//! Shared labels for observed proxy modes; native surfaces only replay copy.
use crate::proxy_mode_actions::ProxyModeActions;
use infiltrator_contract::command::ProxyMode;
use infiltrator_contract::proxy_mode::{ProxyModeSnapshot, ProxyModeStatus};
use infiltrator_domain::redact::redact_line;
use infiltrator_shared::i18n_interpolator::interpolate;
use infiltrator_shared::locales::{Lang, Localizer};

pub fn mode_copy_key(mode: ProxyMode) -> &'static str {
    match mode {
        ProxyMode::Rule => "proxy_mode_rule",
        ProxyMode::Global => "proxy_mode_global",
        ProxyMode::Direct => "proxy_mode_direct",
        ProxyMode::Script => "proxy_mode_script",
    }
}
pub fn mode_copy(mode: ProxyMode, locale: &str) -> String {
    Lang(locale).tr(mode_copy_key(mode)).into_owned()
}

pub fn mode_failure_copy(actions: &ProxyModeActions, locale: &str) -> String {
    let Some(failure) = &actions.failure else {
        return String::new();
    };
    let target = actions
        .requested
        .map(|mode| mode_copy(mode, locale))
        .unwrap_or_else(|| Lang(locale).tr("shell_readout_unknown").into_owned());
    let reason = redact_line(&failure.message, &[]);
    interpolate(
        Lang(locale).tr("proxy_mode_change_failed").as_ref(),
        &[("target", &target), ("reason", &reason)],
    )
}

pub fn mode_status_copy(snapshot: &ProxyModeSnapshot, locale: &str) -> String {
    let value = snapshot.current.map(|mode| mode_copy(mode, locale));
    let key = match snapshot.status {
        ProxyModeStatus::Ready => {
            return value.unwrap_or_else(|| Lang(locale).tr("shell_readout_unknown").into_owned());
        }
        ProxyModeStatus::Pending => "proxy_mode_pending",
        ProxyModeStatus::Unobserved | ProxyModeStatus::Failed | ProxyModeStatus::Unsupported => {
            if value.is_some() {
                "shell_readout_stale"
            } else {
                "shell_readout_unknown"
            }
        }
    };
    let value = value.unwrap_or_else(|| Lang(locale).tr("shell_readout_unknown").into_owned());
    interpolate(Lang(locale).tr(key).as_ref(), &[("value", &value)])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mode_caption_marks_pending_retained_and_missing_facts_in_both_languages() {
        let mut snapshot = ProxyModeSnapshot::default();
        assert_eq!(mode_status_copy(&snapshot, "en"), "Not observed");
        snapshot.current = Some(ProxyMode::Global);
        snapshot.status = ProxyModeStatus::Ready;
        assert_eq!(mode_status_copy(&snapshot, "en"), "Global");
        snapshot.status = ProxyModeStatus::Pending;
        assert_eq!(mode_status_copy(&snapshot, "en"), "Global (changing)");
        assert_eq!(mode_status_copy(&snapshot, "zh-CN"), "全局（切换中）");
        snapshot.status = ProxyModeStatus::Failed;
        assert_eq!(mode_status_copy(&snapshot, "en"), "Global (stale)");
        assert_eq!(mode_status_copy(&snapshot, "zh-CN"), "全局（已失效）");
        snapshot.current = None;
        assert_eq!(mode_status_copy(&snapshot, "en"), "Not observed");
    }
}
