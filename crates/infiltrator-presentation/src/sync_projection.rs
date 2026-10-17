//! Shared synchronization presentation. Configuration is not proof of a connection.
use crate::text::localize;
use infiltrator_contract::surface_snapshot::PageStatus;
use infiltrator_contract::sync::SyncStatus;
use lurker_ui_presentation::copy::TextCatalog;

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
pub fn summary(status: SyncStatus, catalog: &dyn TextCatalog) -> String {
    localize(
        catalog,
        "sync_observation_summary",
        &[("status", catalog.text(status_key(status)).into_owned())],
    )
}
pub fn last_sync(value: Option<&str>, catalog: &dyn TextCatalog) -> String {
    value
        .map(|value| localize(catalog, "sync_observation_last", &[("time", value.into())]))
        .unwrap_or_else(|| catalog.text("sync_observation_last_unknown").into_owned())
}
pub fn server(value: &str, catalog: &dyn TextCatalog) -> String {
    localize(
        catalog,
        "sync_observation_server",
        &[("server", value.into())],
    )
}
pub fn username(value: &str, catalog: &dyn TextCatalog) -> String {
    localize(
        catalog,
        "sync_observation_username",
        &[("username", value.into())],
    )
}
pub fn conflict(remote: &str, time: &str, count: usize, catalog: &dyn TextCatalog) -> String {
    localize(
        catalog,
        "sync_observation_conflict_summary",
        &[
            ("device", remote.into()),
            ("time", time.into()),
            ("count", count.to_string()),
        ],
    )
}
pub fn conflict_field(key: &str, local: &str, remote: &str, catalog: &dyn TextCatalog) -> String {
    localize(
        catalog,
        "sync_observation_field",
        &[
            ("key", key.into()),
            ("local", local.into()),
            ("remote", remote.into()),
        ],
    )
}

pub fn history_notice(status: &PageStatus, catalog: &dyn TextCatalog) -> String {
    let key = match status {
        PageStatus::Loading => "sync_history_unknown",
        PageStatus::Ready => "sync_observation_history_hint",
        PageStatus::Empty => "sync_history_empty",
        PageStatus::Failed { failure } | PageStatus::Unavailable { failure } => {
            return localize(
                catalog,
                "sync_history_failed",
                &[("reason", failure.message.clone())],
            );
        }
    };
    catalog.text(key).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use infiltrator_shared::locales::Lang;
    #[test]
    fn configured_is_not_connected_and_fields_preserve_opaque_values() {
        assert_eq!(
            summary(SyncStatus::Configured, &Lang("en-US")),
            "Sync · Configured; connection not verified"
        );
        assert_eq!(
            summary(SyncStatus::Unknown, &Lang("en-US")),
            "Sync · Not observed"
        );
        assert_ne!(
            status_key(SyncStatus::Configured),
            status_key(SyncStatus::Connected)
        );
        assert_eq!(
            conflict_field(
                "key {local}/中文🙂",
                "local {remote}\n0",
                "remote {key}\nfalse",
                &Lang("en-US")
            ),
            "key {local}/中文🙂\nLocal: local {remote}\n0\nRemote: remote {key}\nfalse"
        );
        assert_eq!(last_sync(None, &Lang("en-US")), "Last sync: not observed");
        assert_eq!(
            last_sync(Some("{time}/中文🙂"), &Lang("en-US")),
            "Last sync: {time}/中文🙂"
        );
    }
}
