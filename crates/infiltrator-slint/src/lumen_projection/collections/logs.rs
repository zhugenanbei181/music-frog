//! The logs page projection of the Slint surface.
//!
//! The logs page has exactly one state derivation and it is shared: the
//! query, the level filter, the page status and the search outcome live in
//! the shared page model. This module renders that model; it never folds the
//! published log window a second time.

use super::super::snapshot_language;
use crate::widget_vocabulary;
use crate::{AppWindow, ObservationRow, slint_model};
use infiltrator_contract::logs::LogStreamState;
use infiltrator_contract::surface_snapshot::SurfaceSnapshot;
use infiltrator_presentation::log_page_model::{LogLevelFilter, LogsPageModel};
use infiltrator_shared::locales::{Lang, Localizer};

/// Projects the logs page from the shared page model.
///
/// The model owns the query, the level filter, the page status and the typed
/// search outcome; the surface only renders the rows the model reports and
/// writes the owner's level readback back into the widget.
pub(super) fn project_logs(window: &AppWindow, snapshot: &SurfaceSnapshot) {
    let language = snapshot_language(snapshot);
    let lang = Lang(&language);
    let mut model = LogsPageModel::default();
    model.observe(snapshot.generation, None, &snapshot.pages.logs);
    model.edit_query(window.get_log_search().as_str());
    // The level filter is a separate owner readback: the page observation
    // never carries it, so the surface hands the owner's active level to the
    // model instead of deriving the filter here. Without a published page
    // there is no readback, and the level the user chose stays the model's
    // own selection.
    model.select_level(LogLevelFilter::from_identifier(
        window.get_log_level().as_str(),
    ));
    if let Some(page) = snapshot.pages.logs.data.as_ref() {
        model.observe_level(LogLevelFilter::from_active_level(
            page.active_level.as_deref(),
        ));
    }
    let active_level = model.active_level();
    let rows = model
        .rows()
        .iter()
        .filter(|row| row.visible)
        .filter(|row| active_level.is_none_or(|level| row.level == level))
        .map(|row| {
            let message: String = row.message.iter().map(|run| run.text.as_str()).collect();
            let timestamp: String = row.timestamp.iter().map(|run| run.text.as_str()).collect();
            let level: String = row
                .level_label
                .iter()
                .map(|run| run.text.as_str())
                .collect();
            let tag: String = row.tag.iter().map(|run| run.text.as_str()).collect();
            ObservationRow {
                id: row.id.to_string().into(),
                title: message.clone().into(),
                accessible_title: message.into(),
                flag_code: "".into(),
                flag_index: -1,
                delay_ms: -1,
                subtitle: format!("{timestamp} · {level}").into(),
                trailing: tag.into(),
                active: false,
            }
        })
        .collect::<Vec<_>>();
    window.set_log_level(widget_vocabulary::log_level_value(active_level).into());
    window.set_logs(slint_model(rows));
    window.set_log_stream_status(log_stream_copy(&model.stream(), &lang).into());
}

/// The localized copy of the typed log stream state the model reports.
///
/// The state is the shared model's; only the wording belongs to the surface.
fn log_stream_copy(stream: &LogStreamState, lang: &Lang<'_>) -> String {
    match stream {
        LogStreamState::Live => lang.tr("已连接").into_owned(),
        LogStreamState::Reconnecting(_) => lang.tr("正在重连").into_owned(),
        LogStreamState::Connecting => lang.tr("正在连接").into_owned(),
        LogStreamState::Failed(_) => lang.tr("读取失败").into_owned(),
        LogStreamState::Unsupported(_) => lang.tr("日志流不支持").into_owned(),
        LogStreamState::Cancelled => lang.tr("日志流已取消").into_owned(),
        LogStreamState::Idle => lang.tr("未运行").into_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::log_stream_copy;
    use infiltrator_contract::logs::LogStreamState;
    use infiltrator_shared::locales::Lang;
    use lurker_core_contract::error::Failure;

    #[test]
    fn unsupported_and_cancelled_log_streams_project_localized_terminal_copy() {
        let unsupported = LogStreamState::Unsupported(Failure::unsupported("not available"));
        assert_eq!(
            log_stream_copy(&unsupported, &Lang("zh-CN")),
            "日志流不支持"
        );
        assert_eq!(
            log_stream_copy(&unsupported, &Lang("en-US")),
            "Log stream is unsupported"
        );

        assert_eq!(
            log_stream_copy(&LogStreamState::Cancelled, &Lang("zh-CN")),
            "日志流已取消"
        );
        assert_eq!(
            log_stream_copy(&LogStreamState::Cancelled, &Lang("en-US")),
            "Log stream was cancelled"
        );
    }
}
