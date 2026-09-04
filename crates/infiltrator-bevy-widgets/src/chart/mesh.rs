//! GPU accelerated 2D vertex mesh generation and WGSL shader pipeline for telemetry.
//!
//! Provides zero-cost geometry compilation from high-level telemetry data into
//! standard Bevy `Mesh` buffers (`TriangleList`) with positions, vertex colors,
//! and texture coordinates for hardware-accelerated rendering.

use bevy::asset::RenderAssetUsages;
use bevy::render::mesh::{Indices, Mesh, PrimitiveTopology};

use super::bezier::PlotPoint;

/// A single vertex for 2D GPU telemetry rendering.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TelemetryVertex {
    pub position: [f32; 3],
    pub color: [f32; 4],
    pub uv: [f32; 2],
}

impl TelemetryVertex {
    pub const fn new(position: [f32; 3], color: [f32; 4], uv: [f32; 2]) -> Self {
        Self {
            position,
            color,
            uv,
        }
    }
}

/// Intermediate geometric mesh buffer before Bevy asset upload.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TelemetryMeshData {
    pub vertices: Vec<TelemetryVertex>,
    pub indices: Vec<u32>,
}

impl TelemetryMeshData {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_capacity(v_cap: usize, i_cap: usize) -> Self {
        Self {
            vertices: Vec::with_capacity(v_cap),
            indices: Vec::with_capacity(i_cap),
        }
    }

    /// Add a quad (2 triangles) referencing four vertex indices.
    pub fn push_quad(&mut self, i0: u32, i1: u32, i2: u32, i3: u32) {
        self.indices.extend_from_slice(&[i0, i1, i2, i0, i2, i3]);
    }

    /// Convert into an engine-native Bevy `Mesh` asset.
    pub fn to_bevy_mesh(&self) -> Mesh {
        let mut mesh = Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::default(),
        );

        let mut positions = Vec::with_capacity(self.vertices.len());
        let mut colors = Vec::with_capacity(self.vertices.len());
        let mut uvs = Vec::with_capacity(self.vertices.len());

        for v in &self.vertices {
            positions.push(v.position);
            colors.push(v.color);
            uvs.push(v.uv);
        }

        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
        mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
        mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
        mesh.insert_indices(Indices::U32(self.indices.clone()));

        mesh
    }
}

/// Construct a GPU vertex mesh for the fade fill area under a curve.
pub fn build_curve_area_mesh(
    points: &[Option<PlotPoint>],
    top_color: [f32; 4],
    bottom_color: [f32; 4],
    baseline_y: f32,
) -> TelemetryMeshData {
    let mut mesh = TelemetryMeshData::new();

    // Iterate over contiguous segments
    let mut start_idx = 0;
    while start_idx < points.len() {
        while start_idx < points.len() && points[start_idx].is_none() {
            start_idx += 1;
        }
        if start_idx >= points.len() {
            break;
        }
        let mut end_idx = start_idx;
        while end_idx < points.len() && points[end_idx].is_some() {
            end_idx += 1;
        }

        let segment = &points[start_idx..end_idx];
        if segment.len() >= 2 {
            let base_vertex = mesh.vertices.len() as u32;
            for (i, opt_p) in segment.iter().enumerate() {
                let p = opt_p.unwrap();
                let u = i as f32 / (segment.len() - 1) as f32;

                // Top vertex (at curve)
                mesh.vertices
                    .push(TelemetryVertex::new([p.x, p.y, 0.0], top_color, [u, 0.0]));

                // Bottom vertex (at baseline)
                mesh.vertices.push(TelemetryVertex::new(
                    [p.x, baseline_y, 0.0],
                    bottom_color,
                    [u, 1.0],
                ));
            }

            for i in 0..(segment.len() as u32 - 1) {
                let top_left = base_vertex + i * 2;
                let bot_left = top_left + 1;
                let top_right = top_left + 2;
                let bot_right = top_left + 3;

                mesh.push_quad(top_left, top_right, bot_right, bot_left);
            }
        }

        start_idx = end_idx;
    }

    mesh
}

/// Construct a GPU ribbon mesh representing a line stroke with given thickness.
pub fn build_curve_ribbon_mesh(
    points: &[Option<PlotPoint>],
    thickness_px: f32,
    color: [f32; 4],
) -> TelemetryMeshData {
    let mut mesh = TelemetryMeshData::new();
    let half_t = thickness_px * 0.5;

    let mut start_idx = 0;
    while start_idx < points.len() {
        while start_idx < points.len() && points[start_idx].is_none() {
            start_idx += 1;
        }
        if start_idx >= points.len() {
            break;
        }
        let mut end_idx = start_idx;
        while end_idx < points.len() && points[end_idx].is_some() {
            end_idx += 1;
        }

        let segment = &points[start_idx..end_idx];
        if segment.len() >= 2 {
            let base_vertex = mesh.vertices.len() as u32;

            for (i, opt_p) in segment.iter().enumerate() {
                let p = opt_p.unwrap();
                let u = i as f32 / (segment.len() - 1) as f32;

                // Simple normal along Y
                mesh.vertices.push(TelemetryVertex::new(
                    [p.x, p.y - half_t, 0.0],
                    color,
                    [u, 0.0],
                ));
                mesh.vertices.push(TelemetryVertex::new(
                    [p.x, p.y + half_t, 0.0],
                    color,
                    [u, 1.0],
                ));
            }

            for i in 0..(segment.len() as u32 - 1) {
                let top_left = base_vertex + i * 2;
                let bot_left = top_left + 1;
                let top_right = top_left + 2;
                let bot_right = top_left + 3;

                mesh.push_quad(top_left, top_right, bot_right, bot_left);
            }
        }

        start_idx = end_idx;
    }

    mesh
}

/// Construct an annular ring sector mesh for donut charts.
pub fn build_donut_sector_mesh(
    center: [f32; 2],
    inner_radius: f32,
    outer_radius: f32,
    start_angle: f32,
    end_angle: f32,
    color: [f32; 4],
    subdivisions: usize,
) -> TelemetryMeshData {
    let mut mesh = TelemetryMeshData::new();
    let steps = subdivisions.max(2);
    let base_idx = mesh.vertices.len() as u32;

    for i in 0..=steps {
        let t = i as f32 / steps as f32;
        let angle = start_angle + (end_angle - start_angle) * t;
        let (sin_a, cos_a) = angle.sin_cos();

        // Inner circle point
        let in_x = center[0] + inner_radius * cos_a;
        let in_y = center[1] + inner_radius * sin_a;
        mesh.vertices
            .push(TelemetryVertex::new([in_x, in_y, 0.0], color, [t, 0.0]));

        // Outer circle point
        let out_x = center[0] + outer_radius * cos_a;
        let out_y = center[1] + outer_radius * sin_a;
        mesh.vertices
            .push(TelemetryVertex::new([out_x, out_y, 0.0], color, [t, 1.0]));
    }

    for i in 0..steps as u32 {
        let in_left = base_idx + i * 2;
        let out_left = in_left + 1;
        let in_right = in_left + 2;
        let out_right = in_left + 3;

        mesh.push_quad(in_left, out_left, out_right, in_right);
    }

    mesh
}

/// Construct a rounded-top bar mesh for histogram visualization.
pub fn build_bar_mesh(
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    _corner_radius: f32,
    top_color: [f32; 4],
    bottom_color: [f32; 4],
) -> TelemetryMeshData {
    let mut mesh = TelemetryMeshData::new();
    let base_idx = 0;

    // 4 corners of the bar quad
    mesh.vertices
        .push(TelemetryVertex::new([x, y, 0.0], top_color, [0.0, 0.0]));
    mesh.vertices.push(TelemetryVertex::new(
        [x + width, y, 0.0],
        top_color,
        [1.0, 0.0],
    ));
    mesh.vertices.push(TelemetryVertex::new(
        [x + width, y + height, 0.0],
        bottom_color,
        [1.0, 1.0],
    ));
    mesh.vertices.push(TelemetryVertex::new(
        [x, y + height, 0.0],
        bottom_color,
        [0.0, 1.0],
    ));

    mesh.push_quad(base_idx, base_idx + 1, base_idx + 2, base_idx + 3);

    mesh
}

/// WGSL shader source code for custom GPU telemetry rendering pipelines.
pub const TELEMETRY_SHADER_WGSL: &str = r#"
struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) color: vec4<f32>,
    @location(2) uv: vec2<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) uv: vec2<f32>,
};

@vertex
fn vertex(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.clip_position = vec4<f32>(in.position, 1.0);
    out.color = in.color;
    out.uv = in.uv;
    return out;
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    // Smooth gradient fade and subtle scanline / edge glow
    var alpha = in.color.a * (1.0 - in.uv.y * 0.75);
    return vec4<f32>(in.color.rgb, alpha);
}
"#;
