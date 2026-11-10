//! The dashboard facts projection of the Slint surface.
//!
//! Every dashboard card binds to one typed [`DashboardFacts`] value that this
//! module projects from the shared owner snapshots. The surface keeps no
//! second copy of any dashboard fact and never invents a value: an unobserved
//! fact stays an explicit "—".

use crate::AppWindow;
use crate::DashboardFacts;
use infiltrator_contract::design_tokens::OUTBOUND_STATUS_PALETTE;
use infiltrator_contract::relative_time::RelativeTimePresentation;
use infiltrator_contract::resources::{CoreResourceSnapshot, ResourceAvailability};
use infiltrator_contract::surface_snapshot::SurfaceSnapshot;
use infiltrator_contract::ui_fact;
use infiltrator_presentation::dashboard_hero_projection::{
    DashboardHeroProjection, ProfileQuickCardProjection, QuickSwitchesProjection,
    TrafficWaveformProjection,
};
use infiltrator_presentation::relative_time_projection::copy_of;
use infiltrator_shared::byte_units::format_bytes;
use infiltrator_shared::i18n_interpolator::localize;
use infiltrator_shared::locales::{Lang, Localizer};

pub(super) fn project_dashboard_facts(window: &AppWindow, snapshot: &SurfaceSnapshot) {
    let hero = DashboardHeroProjection::from_snapshot(snapshot);
    let profile = ProfileQuickCardProjection::from_snapshot(snapshot);
    let language = super::snapshot_language(snapshot);
    let updated_relative = copy_of(
        profile.update_age,
        RelativeTimePresentation::Detailed,
        &Lang(&language),
    );
    let updated_relative_compact = copy_of(
        profile.update_age,
        RelativeTimePresentation::Compact,
        &Lang(&language),
    );
    let profile_source_label = profile.source.copy(&Lang(&language));

    let quick = QuickSwitchesProjection::from_snapshot(snapshot);
    let (memory_total, memory_limit, memory_fraction) = memory_facts(snapshot);
    let waveform = TrafficWaveformProjection::from_snapshot(snapshot);
    let (latencies, latency_colors, services_summary) = service_latency_facts(snapshot);
    let (cpu_text, cpu_fraction) = cpu_facts(snapshot);
    let (upload_wave, download_wave) = (
        waveform_path(&waveform.normalized_up_points),
        waveform_path(&waveform.normalized_down_points),
    );

    window.set_dashboard_facts(DashboardFacts {
        quality_tag: hero.quality_tag.into(),
        hero_fact_uptime: fact_cell(&hero.fact_row.uptime),
        hero_fact_organization: fact_cell(&hero.fact_row.organization),
        traffic_peak_up: waveform.peak_up_formatted.clone().into(),
        traffic_peak_down: waveform.peak_down_formatted.clone().into(),
        traffic_total_up: waveform.total_up_formatted.clone().into(),
        traffic_total_down: waveform.total_down_formatted.clone().into(),
        connection_count: snapshot.core.active_connections.to_string().into(),
        request_count: snapshot.requests.total_requests_observed.to_string().into(),
        dns_query_count: snapshot
            .dns_query_history
            .total_queries_observed
            .to_string()
            .into(),
        // The owner's LAN projection names the real primary endpoint. The
        // settings `bind_address` is a bind wildcard ("*"), not an IP the user
        // can copy, so showing it as 内网 IP was a semantic mismatch.
        lan_ip: quick.lan_ip_text.into(),
        lan_shared: quick.lan_share_enabled,
        profile_groups_count: profile.policy_groups_count as i32,
        profile_proxies_count: profile.proxies_count as i32,
        profile_rules_count: profile.rules_count as i32,
        profile_updated_relative: updated_relative.into(),
        profile_updated_relative_compact: updated_relative_compact.into(),
        profile_source_label: profile_source_label.into(),
        // The four common-service probes come from the owner's speedtest
        // snapshot in its published order; an absent probe stays an honest "—".
        latency_google: latencies[0].clone().into(),
        latency_github: latencies[1].clone().into(),
        latency_youtube: latencies[2].clone().into(),
        latency_openai: latencies[3].clone().into(),
        services_summary: services_summary.into(),
        latency_google_color: latency_colors[0],
        latency_github_color: latency_colors[1],
        latency_youtube_color: latency_colors[2],
        latency_openai_color: latency_colors[3],
        memory_total: memory_total.into(),
        memory_limit: memory_limit.into(),
        memory_fraction,
        cpu_text: cpu_text.into(),
        cpu_fraction,
        upload_wave: upload_wave.into(),
        download_wave: download_wave.into(),
    });
}

/// Project one typed fact cell into the surface's cell.
///
/// The cell is the only writer of a fact's text: the shared contract owns the
/// unobserved marker, so a surface can never print a second fallback for a
/// value nobody published.
fn fact_cell(cell: &ui_fact::FactCell) -> crate::FactCell {
    crate::FactCell {
        value: cell.value.text().into(),
        observed: cell.value.is_observed(),
        alignment: match cell.alignment {
            ui_fact::FactAlignment::Start => crate::FactAlignment::Start,
            ui_fact::FactAlignment::End => crate::FactAlignment::End,
        },
    }
}

/// The four published probe results as display-ready latency strings, the
/// shared ladder's ink, and one honest availability summary.
///
/// The contract grades each measurement; the surface never re-derives a
/// threshold or prints a fixed "全部可用" badge over probes it did not confirm.
fn service_latency_facts(snapshot: &SurfaceSnapshot) -> ([String; 4], [slint::Color; 4], String) {
    let language = super::snapshot_language(snapshot);
    let lang = Lang(&language);
    let mut facts = [
        "—".to_string(),
        "—".to_string(),
        "—".to_string(),
        "—".to_string(),
    ];
    let mut colors = [slint::Color::from_rgb_u8(0x6b, 0x72, 0x80); 4];
    let mut answered = 0_usize;
    let mut reachable = 0_usize;
    for ((slot, ink), result) in facts
        .iter_mut()
        .zip(colors.iter_mut())
        .zip(snapshot.speedtest.results.iter())
    {
        let delay = result.delay_ms();
        if delay.is_some() {
            answered += 1;
        }
        if delay.is_some() {
            reachable += 1;
        }
        *slot = result
            .delay_ms()
            .map(|delay| format!("{delay} ms"))
            .unwrap_or_else(|| "—".to_string());
        // The reference's service dot answers "was this probe observed?", not
        // "how slow was it?". Latency quality is already carried by the value
        // and the summary; painting every slow-but-available probe red made a
        // healthy dashboard look like four failures.
        let token = if delay.is_some() {
            OUTBOUND_STATUS_PALETTE.running
        } else {
            OUTBOUND_STATUS_PALETTE.offline
        };
        // The contract token is linear 0..1; Slint's encoded constructor
        // takes 0..255 u8 channels.
        let channel = |value: f32| (value.clamp(0.0, 1.0) * 255.0).round() as u8;
        *ink = slint::Color::from_argb_u8(
            channel(token.a),
            channel(token.r),
            channel(token.g),
            channel(token.b),
        );
    }
    // "全部可用" requires every one of the four slots to carry a measured
    // probe; a partial read states the count, and no read at all stays an
    // explicit unknown.
    let summary = if answered == 0 {
        lang.tr("overview_service_tile_unobserved").into_owned()
    } else if answered == 4 && reachable == 4 {
        lang.tr("overview_services_all_available").into_owned()
    } else {
        localize(
            &language,
            "overview_services_partial_pattern",
            &[
                ("reachable", reachable.to_string()),
                ("total", "4".to_string()),
            ],
        )
    };
    (facts, colors, summary)
}

/// The owner's observed CPU load and its fraction of one full core.
fn cpu_facts(snapshot: &SurfaceSnapshot) -> (String, f32) {
    let CoreResourceSnapshot::Observed { cpu_percent, .. } = &snapshot.resources else {
        return ("—".to_string(), 0.0);
    };
    match cpu_percent {
        ResourceAvailability::Available(percent) => (
            format!("{:.1}%", percent.clamp(0.0, 100.0)),
            percent.clamp(0.0, 100.0) / 100.0,
        ),
        _ => ("—".to_string(), 0.0),
    }
}

/// One smoothed SVG path from the owner's normalized series. Each path starts
/// at the left edge and ends at the right edge, with a small baseline
/// clearance so the stroke is never clipped by the card's bottom inset.
///
/// The surface does not choose a scale. Normalization is the owner's fact
/// (`TrafficWaveformProjection::normalized_*_points`), and it is per series:
/// each rate card prints its own `峰值`, so the curve has to reach the top of
/// its band at that same peak. Dividing both series by a shared maximum made
/// the quieter card draw its own peak at a fraction of the band and left the
/// rest of the card as dead ground.
fn waveform_path(points: &[f32]) -> String {
    const WIDTH: f64 = 100.0;
    const BASELINE: f64 = 30.0;
    if points.len() < 2 {
        return String::new();
    }
    let step = WIDTH / (points.len() - 1) as f64;
    // Geometry robustness, not a scale decision: a non-finite or out-of-range
    // point has to become a finite coordinate or the emitted path string would
    // read "NaN" and the whole band would fail to draw.
    let y_for = |value: f32| {
        let share = if value.is_finite() {
            value.clamp(0.0, 1.0)
        } else {
            0.0
        };
        BASELINE - f64::from(share) * (BASELINE - 2.0)
    };
    let mut path = format!("M 0 {:.1}", y_for(points[0]));
    for index in 0..points.len() - 1 {
        let x = (index + 1) as f64 * step;
        let previous_y = y_for(points[index]);
        let y = y_for(points[index + 1]);
        let mid = x - step / 2.0;
        path.push_str(&format!(
            " C {mid:.1} {previous_y:.1}, {mid:.1} {y:.1}, {x:.1} {y:.1}"
        ));
    }
    path.push_str(&format!(" L {WIDTH:.1} {BASELINE:.1} L 0 {BASELINE:.1} Z"));
    path
}

#[cfg(test)]
mod tests {
    use super::waveform_path;
    use infiltrator_contract::traffic_waveform::{TrafficSample, TrafficWaveformSnapshot};
    use infiltrator_presentation::dashboard_hero_projection::TrafficWaveformProjection;

    fn sample(upload: f64, download: f64) -> TrafficSample {
        TrafficSample {
            sampled_at_epoch_ms: None,
            upload_bps: upload,
            download_bps: download,
        }
    }

    /// The path is geometry only: it maps the owner's normalized series onto
    /// the band, so a series that reaches 1.0 reaches the top of the band.
    #[test]
    fn a_normalized_series_spans_the_full_band_and_keeps_a_baseline() {
        let path = waveform_path(&[0.1, 1.0, 0.35, 0.75]);
        assert!(path.starts_with("M 0 27.2"));
        assert!(path.contains("C 16.7 27.2, 16.7 2.0, 33.3 2.0"));
        assert!(path.ends_with("L 100.0 30.0 L 0 30.0 Z"));
        assert!(!path.contains(" L 0 32.0"));
    }

    #[test]
    fn one_sample_is_not_a_drawable_waveform() {
        assert_eq!(waveform_path(&[0.4]), String::new());
        assert_eq!(waveform_path(&[]), String::new());
    }

    /// A non-finite reading must never reach the path string: an SVG path
    /// containing "NaN" draws nothing, so the whole band would disappear.
    #[test]
    fn a_non_finite_point_still_draws_a_finite_path() {
        let path = waveform_path(&[0.5, f32::NAN, f32::INFINITY, -1.0]);
        assert!(!path.contains("NaN"), "{path}");
        assert!(!path.contains("inf"), "{path}");
        assert!(path.ends_with("L 100.0 30.0 L 0 30.0 Z"));
    }

    /// The fact this slice moved: each rate card scales on its own peak, so
    /// the quieter direction still fills its band instead of squatting at the
    /// baseline while its header prints its own `峰值`.
    #[test]
    fn each_series_reaches_the_top_of_its_own_band() {
        let waveform = TrafficWaveformSnapshot {
            generation: 1,
            revision: 4,
            samples: vec![
                sample(1.0e6, 2.0e6),
                sample(2.1e6, 4.0e6),
                sample(1.4e6, 6.2e6),
                sample(0.8e6, 5.0e6),
            ],
        };
        let projection = TrafficWaveformProjection::from_waveform(&waveform, 0, 0);
        let up = waveform_path(&projection.normalized_up_points);
        let down = waveform_path(&projection.normalized_down_points);
        for path in [&up, &down] {
            // 2.0 is the top of the band: the series' own peak must reach it.
            assert!(path.contains(" 2.0,"), "{path} never reaches the band top");
        }
    }
}

/// The observed core memory, the owner's soft limit and the share between
/// them. One reading, one limit: no invented app/kernel split.
fn memory_facts(snapshot: &SurfaceSnapshot) -> (String, String, f32) {
    match &snapshot.resources {
        CoreResourceSnapshot::Observed {
            memory: ResourceAvailability::Available(bytes),
            memory_soft_limit_bytes,
            ..
        } => {
            let formatted = format_bytes(*bytes);
            let limit = if *memory_soft_limit_bytes == 0 {
                0.0
            } else {
                ((*bytes as f32) / (*memory_soft_limit_bytes as f32)).clamp(0.0, 1.0)
            };
            let limit_text = if *memory_soft_limit_bytes == 0 {
                "—".to_string()
            } else {
                format_bytes(*memory_soft_limit_bytes)
            };
            (formatted, limit_text, limit)
        }
        _ => ("—".to_string(), "—".to_string(), 0.0),
    }
}
