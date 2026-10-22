//! The business panels of the runtime workbench page.

use super::Panel;
use crate::surface::business::BusinessId;
use infiltrator_contract::surface_snapshot::SurfaceSnapshot;
use infiltrator_presentation::speedtest_detail_projection::project_details;
use infiltrator_presentation::speedtest_summary_projection::{history_lines, metrics_label};
use infiltrator_shared::locales::Lang;
use lurker_ui_presentation::copy::TextCatalog;

/// The business panels the runtime page carries.
pub(super) fn panels(snapshot: &SurfaceSnapshot, catalog: &Lang<'_>) -> Vec<Panel> {
    let readout = &snapshot.shell_readout;
    let upload = readout
        .upload_bps
        .value()
        .map(|value| format!("{value:.0}"))
        .unwrap_or_else(|| catalog.text("shell_readout_unknown").into_owned());
    let download = readout
        .download_bps
        .value()
        .map(|value| format!("{value:.0}"))
        .unwrap_or_else(|| catalog.text("shell_readout_unknown").into_owned());
    let public_ip = snapshot.public_ip.ip.clone().unwrap_or_default();
    let speedtest = &snapshot.speedtest;
    let details = project_details(speedtest);
    let history = history_lines(speedtest, catalog);
    let metrics = metrics_label(speedtest.results.first(), catalog);
    let failure = snapshot
        .shell_readout
        .rate_failure
        .as_ref()
        .map(|failure| failure.message.clone());
    let empty = speedtest.recent_history.is_empty();

    vec![
        {
            let mut panel = Panel::new(
                BusinessId::RuntimeTelemetryObservation,
                catalog.text("runtime_title"),
                format!("↑ {upload} · ↓ {download}"),
                vec![catalog.text("shell_readout_stale").into_owned(), public_ip],
            );
            panel.failure = failure.clone();
            panel
        },
        Panel::new(
            BusinessId::SpeedtestProgressCancel,
            catalog.text("speedtest_title"),
            metrics,
            vec![
                catalog.text("speedtest_start_action").into_owned(),
                catalog.text("speedtest_phase_probing").into_owned(),
            ],
        ),
        {
            let mut panel = Panel::new(
                BusinessId::SpeedtestDetailsModal,
                catalog.text("speedtest_detail_title"),
                catalog.text("speedtest_detail_open").into_owned(),
                vec![
                    details.rows.len().to_string(),
                    catalog.text("speedtest_detail_targets").into_owned(),
                ],
            );
            panel.failure = failure.clone();
            panel
        },
        {
            let mut panel = Panel::new(
                BusinessId::SpeedtestHistoryComparison,
                catalog.text("speedtest_history_title"),
                history.len().to_string(),
                history,
            );
            panel.failure = failure;
            panel.empty = empty;
            panel
        },
    ]
}
