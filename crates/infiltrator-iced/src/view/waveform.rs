//! Canvas-based waveform and traffic charts for the Infiltrator UI.

use iced::widget::canvas;
use iced::{Color, Element, Point, Rectangle, Renderer, Theme, mouse};
use std::collections::VecDeque;

use crate::view::theme;
use infiltrator_contract::traffic_scale::TrafficScaleSnapshot;
use infiltrator_contract::traffic_waveform::TrafficWaveformSnapshot;

// ---------------------------------------------------------------------------
// High-Fidelity GPU Canvas Traffic Chart & Waveforms
// ---------------------------------------------------------------------------

pub struct TrafficChart {
    pub history: VecDeque<(u64, u64)>,
    /// Complete surface samples, when the application pump supplied them.
    /// `None` keeps the legacy stream path working for the desktop shell.
    pub shared: Option<TrafficWaveformSnapshot>,
    pub scale: Option<TrafficScaleSnapshot>,
}

impl<Message> canvas::Program<Message> for TrafficChart {
    type State = ();

    fn draw(
        &self,
        _state: &(),
        renderer: &Renderer,
        theme: &Theme,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let tk = theme::tokens(theme);
        let accent = tk.accent;
        let success = tk.success;
        let grid_color = Color {
            a: 0.12,
            ..tk.card_border
        };

        let mut frame = canvas::Frame::new(renderer, bounds.size());
        let (width, height) = (bounds.width, bounds.height);

        // 1. Draw subtle background horizontal grid lines (25%, 50%, 75%)
        for ratio in [0.25, 0.50, 0.75] {
            let y = height * ratio;
            let grid_line = canvas::Path::line(Point::new(0.0, y), Point::new(width, y));
            frame.stroke(
                &grid_line,
                canvas::Stroke::default()
                    .with_color(grid_color)
                    .with_width(1.0),
            );
        }

        let (upload_raw, download_raw) = self.raw_series();
        if upload_raw.len() < 2 || download_raw.len() < 2 {
            return vec![frame.into_geometry()];
        }

        let (upload_curve, download_curve) =
            infiltrator_domain::traffic_waveform::smooth_dual_series(
                &upload_raw,
                &download_raw,
                4,
            );
        let curve_count = upload_curve.len().max(download_curve.len());
        let x_step = width / (curve_count.saturating_sub(1).max(1)) as f32;

        let max_speed = self
            .resolved_scale(&upload_raw, &download_raw)
            .max_bps;

        let scale = |speed: f64| {
            let ratio = (speed / max_speed).clamp(0.0, 1.0) as f32;
            height - (ratio * (height - 8.0)) - 4.0
        };

        // 2. Download area fill & glowing curve
        let down_path = canvas::Path::new(|p| {
            p.move_to(Point::new(0.0, height));
            for (i, down) in download_curve.iter().enumerate() {
                p.line_to(Point::new(i as f32 * x_step, scale(*down as f64)));
            }
            p.line_to(Point::new(
                (download_curve.len() - 1) as f32 * x_step,
                height,
            ));
            p.close();
        });
        frame.fill(&down_path, Color { a: 0.12, ..accent });

        let down_line = canvas::Path::new(|p| {
            for (i, down) in download_curve.iter().enumerate() {
                let pt = Point::new(i as f32 * x_step, scale(*down as f64));
                if i == 0 {
                    p.move_to(pt);
                } else {
                    p.line_to(pt);
                }
            }
        });
        frame.stroke(
            &down_line,
            canvas::Stroke::default()
                .with_color(Color { a: 0.16, ..accent })
                .with_width(7.0),
        );
        frame.stroke(
            &down_line,
            canvas::Stroke::default()
                .with_color(accent)
                .with_width(2.5),
        );

        // 3. Upload area fill & glowing curve
        let up_path = canvas::Path::new(|p| {
            p.move_to(Point::new(0.0, height));
            for (i, up) in upload_curve.iter().enumerate() {
                p.line_to(Point::new(i as f32 * x_step, scale(*up as f64)));
            }
            p.line_to(Point::new(
                (upload_curve.len() - 1) as f32 * x_step,
                height,
            ));
            p.close();
        });
        frame.fill(&up_path, Color { a: 0.08, ..success });

        let up_line = canvas::Path::new(|p| {
            for (i, up) in upload_curve.iter().enumerate() {
                let pt = Point::new(i as f32 * x_step, scale(*up as f64));
                if i == 0 {
                    p.move_to(pt);
                } else {
                    p.line_to(pt);
                }
            }
        });
        frame.stroke(
            &up_line,
            canvas::Stroke::default()
                .with_color(Color { a: 0.14, ..success })
                .with_width(6.0),
        );
        frame.stroke(
            &up_line,
            canvas::Stroke::default()
                .with_color(success)
                .with_width(2.0),
        );

        // 4. Interactive cursor tracking crosshair
        if let Some(cursor_pos) = cursor.position_in(bounds) {
            let scan_x = cursor_pos.x.clamp(0.0, width);
            let raw_x_step = width / (upload_raw.len() - 1) as f32;
            let sample_idx = ((scan_x / raw_x_step).round() as usize)
                .min(upload_raw.len().saturating_sub(1));

            if let (Some(&up_val), Some(&down_val)) = (
                upload_raw.get(sample_idx),
                download_raw.get(sample_idx),
            ) {
                // Vertical scan line
                let scan_line = canvas::Path::line(Point::new(scan_x, 0.0), Point::new(scan_x, height));
                frame.stroke(
                    &scan_line,
                    canvas::Stroke::default()
                        .with_color(Color { a: 0.35, ..tk.text_primary })
                        .with_width(1.0),
                );

                // Highlight points on curves
                let down_pt = Point::new(sample_idx as f32 * raw_x_step, scale(down_val));
                let up_pt = Point::new(sample_idx as f32 * raw_x_step, scale(up_val));

                frame.fill(
                    &canvas::Path::circle(down_pt, 4.0),
                    accent,
                );
                frame.fill(
                    &canvas::Path::circle(up_pt, 3.5),
                    success,
                );
            }
        }

        vec![frame.into_geometry()]
    }
}

impl TrafficChart {
    fn resolved_scale(&self, upload: &[f64], download: &[f64]) -> TrafficScaleSnapshot {
        self.scale.clone().unwrap_or_else(|| {
            infiltrator_domain::traffic_scale::compute_from_rates(upload, download, 0)
        })
    }

    fn raw_series(&self) -> (Vec<f64>, Vec<f64>) {
        if let Some(shared) = &self.shared
            && shared.is_drawable()
        {
            return (
                shared.samples.iter().map(|sample| sample.upload_bps).collect(),
                shared
                    .samples
                    .iter()
                    .map(|sample| sample.download_bps)
                    .collect(),
            );
        }
        (
            self.history.iter().map(|(up, _)| *up as f64).collect(),
            self.history.iter().map(|(_, down)| *down as f64).collect(),
        )
    }
}

/// Compact 60x24 Canvas sparkline for sidebar speed or KPI card traffic preview.
pub struct MiniWaveform {
    pub samples: Vec<u64>,
}

impl<Message> canvas::Program<Message> for MiniWaveform {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &Renderer,
        theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let tk = theme::tokens(theme);
        let mut frame = canvas::Frame::new(renderer, bounds.size());
        let (width, height) = (bounds.width, bounds.height);

        if self.samples.is_empty() {
            let mid_y = height / 2.0;
            let baseline = canvas::Path::line(Point::new(0.0, mid_y), Point::new(width, mid_y));
            frame.stroke(
                &baseline,
                canvas::Stroke::default()
                    .with_color(Color {
                        a: 0.25,
                        ..tk.text_tertiary
                    })
                    .with_width(1.0),
            );
            return vec![frame.into_geometry()];
        }

        if self.samples.len() == 1 {
            let mid_y = height / 2.0;
            let baseline = canvas::Path::line(Point::new(0.0, mid_y), Point::new(width, mid_y));
            frame.stroke(
                &baseline,
                canvas::Stroke::default()
                    .with_color(tk.accent)
                    .with_width(1.5),
            );
            return vec![frame.into_geometry()];
        }

        let max_val = *self.samples.iter().max().unwrap_or(&1).max(&1);
        let pad_y = 2.0_f32;
        let usable_h = (height - pad_y * 2.0).max(1.0);
        let step = width / (self.samples.len() - 1) as f32;
        let scale_y = |val: u64| -> f32 {
            let ratio = (val as f32 / max_val as f32).clamp(0.0, 1.0);
            height - pad_y - (ratio * usable_h)
        };

        let area_path = canvas::Path::new(|p| {
            p.move_to(Point::new(0.0, height));
            for (i, &val) in self.samples.iter().enumerate() {
                p.line_to(Point::new(i as f32 * step, scale_y(val)));
            }
            p.line_to(Point::new(width, height));
            p.close();
        });
        frame.fill(&area_path, Color { a: 0.12, ..tk.accent });

        let line_path = canvas::Path::new(|p| {
            for (i, &val) in self.samples.iter().enumerate() {
                let pt = Point::new(i as f32 * step, scale_y(val));
                if i == 0 {
                    p.move_to(pt);
                } else {
                    p.line_to(pt);
                }
            }
        });
        frame.stroke(
            &line_path,
            canvas::Stroke::default()
                .with_color(tk.accent)
                .with_width(1.5),
        );

        vec![frame.into_geometry()]
    }
}

/// Compact 60x24 Canvas sparkline for sidebar speed or KPI card traffic preview.
pub fn mini_waveform<'a, Message: 'a>(samples: &[u64]) -> Element<'a, Message> {
    canvas::Canvas::new(MiniWaveform {
        samples: samples.to_vec(),
    })
    .width(60)
    .height(24)
    .into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use infiltrator_contract::traffic_waveform::{TrafficSample, TrafficWaveformSnapshot};

    #[test]
    fn shared_live_samples_are_the_input_to_the_same_bezier_projection() {
        let chart = TrafficChart {
            history: VecDeque::new(),
            shared: Some(TrafficWaveformSnapshot {
                generation: 1,
                revision: 2,
                samples: vec![
                    TrafficSample {
                        sampled_at_epoch_ms: None,
                        upload_bps: 1.0,
                        download_bps: 2.0,
                    },
                    TrafficSample {
                        sampled_at_epoch_ms: None,
                        upload_bps: 3.0,
                        download_bps: 5.0,
                    },
                ],
            }),
            scale: None,
        };
        let (upload, download) = chart.raw_series();
        let (smooth_upload, smooth_download) =
            infiltrator_domain::traffic_waveform::smooth_dual_series(&upload, &download, 4);
        assert_eq!(upload, vec![1.0, 3.0]);
        assert_eq!(download, vec![2.0, 5.0]);
        assert_eq!(smooth_upload.len(), 5);
        assert_eq!(smooth_download.len(), 5);
    }

    #[test]
    fn canvas_uses_the_application_scale_instead_of_a_fixed_floor() {
        let chart = TrafficChart {
            history: VecDeque::from([(1, 2), (3, 5)]),
            shared: None,
            scale: Some(
                infiltrator_domain::traffic_scale::compute_from_peak(20_000.0, 7),
            ),
        };
        let (upload, download) = chart.raw_series();
        let scale = chart.resolved_scale(&upload, &download);
        assert_eq!(scale.revision, 7);
        assert_eq!(scale.peak_bps, 20_000.0);
        assert!(scale.max_bps > 20_000.0);
    }
}
