//! Shared synchronization presentation. Configuration is not proof of a connection.
use infiltrator_contract::surface_snapshot::PageStatus;
use infiltrator_contract::sync::SyncStatus;
use infiltrator_shared::i18n_interpolator::localize;
use infiltrator_shared::locales::{Lang, Localizer};

pub fn status_key(status: SyncStatus) -> &'static str {
    match status {
        SyncStatus::Unknown => "sync_observation_unknown",
        SyncStatus::Configured => "sync_observation_configured",
        SyncStatus::Connected => "sync_observation_connected",
        SyncStatus::Disconnected => "sync_observation_disconnected",
        SyncStatus::Syncing => "sync_observation_running",
        SyncStatus::Conflict => "sync_observation_conflict",
        SyncStatus::Error => "sync_observation_failed",
    }
}
pub fn summary(status: SyncStatus, locale: &str) -> String {
    localize(
        locale,
        "sync_observation_summary",
        &[("status", Lang(locale).tr(status_key(status)).into_owned())],
    )
}
pub fn last_sync(value: Option<&str>, locale: &str) -> String {
    value
        .map(|value| localize(locale, "sync_observation_last", &[("time", value.into())]))
        .unwrap_or_else(|| {
            Lang(locale)
                .tr("sync_observation_last_unknown")
                .into_owned()
        })
}
pub fn server(value: &str, locale: &str) -> String {
    localize(
        locale,
        "sync_observation_server",
        &[("server", value.into())],
    )
}
pub fn username(value: &str, locale: &str) -> String {
    localize(
        locale,
        "sync_observation_username",
        &[("username", value.into())],
    )
}
pub fn conflict(remote: &str, time: &str, count: usize, locale: &str) -> String {
    localize(
        locale,
        "sync_observation_conflict_summary",
        &[
            ("device", remote.into()),
            ("time", time.into()),
            ("count", count.to_string()),
        ],
    )
}
pub fn conflict_field(key: &str, local: &str, remote: &str, locale: &str) -> String {
    localize(
        locale,
        "sync_observation_field",
        &[
            ("key", key.into()),
            ("local", local.into()),
            ("remote", remote.into()),
        ],
    )
}

pub fn history_notice(status: &PageStatus, locale: &str) -> String {
    let key = match status {
        PageStatus::Loading => "sync_history_unknown",
        PageStatus::Ready => "sync_observation_history_hint",
        PageStatus::Empty => "sync_history_empty",
        PageStatus::Failed { failure } | PageStatus::Unavailable { failure } => {
            return localize(
                locale,
                "sync_history_failed",
                &[("reason", failure.message.clone())],
            );
        }
    };
    Lang(locale).tr(key).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn configured_is_not_connected_and_fields_preserve_opaque_values() {
        assert_eq!(
            summary(SyncStatus::Configured, "en-US"),
            "Sync · Configured; connection not verified"
        );
        assert_eq!(summary(SyncStatus::Unknown, "en-US"), "Sync · Not observed");
        assert_ne!(
            status_key(SyncStatus::Configured),
            status_key(SyncStatus::Connected)
        );
        assert_eq!(
            conflict_field(
                "key {local}/中文🙂",
                "local {remote}\n0",
                "remote {key}\nfalse",
                "en-US"
            ),
            "key {local}/中文🙂\nLocal: local {remote}\n0\nRemote: remote {key}\nfalse"
        );
        assert_eq!(last_sync(None, "en-US"), "Last sync: not observed");
        assert_eq!(
            last_sync(Some("{time}/中文🙂"), "en-US"),
            "Last sync: {time}/中文🙂"
        );
    }
}
