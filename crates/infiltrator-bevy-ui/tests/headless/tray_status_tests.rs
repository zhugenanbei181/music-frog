//! Headless tests for the Bevy tray capability report (DUAL-15-02): the
//! surface states the typed "no tray host" boundary instead of fabricating a
//! live rate badge, while still using the shared badge/cadence vocabulary.

use bevy::MinimalPlugins;
use bevy::app::App;
use infiltrator_bevy_ui::tray_status::{TrayStatusPlugin, TrayStatusReport, support};
use infiltrator_contract::traffic_waveform::{TrafficSample, TrafficWaveformSnapshot};
use infiltrator_contract::tray_status::TRAY_RATE_REFRESH_INTERVAL_MS;

fn mounted_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(TrayStatusPlugin);
    app.update();
    app
}

fn live_waveform() -> TrafficWaveformSnapshot {
    TrafficWaveformSnapshot {
        generation: 4,
        revision: 2,
        samples: vec![TrafficSample {
            sampled_at_epoch_ms: Some(1_700_000_000_000),
            upload_bps: 2_048.0,
            download_bps: 1_024.0,
        }],
    }
}

#[test]
fn the_bevy_surface_reports_no_tray_host_instead_of_a_badge() {
    let app = mounted_app();
    let report = *app.world().resource::<TrayStatusReport>();
    assert_eq!(report.support, support());
    assert!(!report.support.is_hosted(), "Bevy has no tray host");
    assert!(!report.support.live_rate_badge());
    assert_eq!(
        report.support.unsupported_reason(),
        Some("bevy-shell-has-no-tray-host")
    );

    // Even with a real live sample, no badge is claimed: the surface cannot
    // display one, and saying otherwise would be a fabrication.
    let waveform = live_waveform();
    assert!(report.live_badge(&waveform).is_none());
    assert!(
        report
            .live_badge(&TrafficWaveformSnapshot::default())
            .is_none()
    );
}

#[test]
fn the_report_carries_the_shared_refresh_cadence() {
    let app = mounted_app();
    let report = *app.world().resource::<TrayStatusReport>();
    assert_eq!(
        report.refresh_interval_ms, TRAY_RATE_REFRESH_INTERVAL_MS,
        "a future Bevy tray host must not invent its own push interval"
    );

    // Flip the capability exactly the way a host would, and the shared badge
    // projection becomes available without touching the format.
    let hosted = TrayStatusReport {
        support: infiltrator_contract::tray_status::TraySupport::Hosted {
            live_rate_badge: true,
        },
        refresh_interval_ms: TRAY_RATE_REFRESH_INTERVAL_MS,
    };
    let badge = hosted.live_badge(&live_waveform()).expect("real sample");
    assert_eq!(badge.badge_text(), "↑ 2.0 KB/s · ↓ 1.0 KB/s");
}
