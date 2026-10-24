//! The wgpu custom-shader surface with a tiny-skia vector twin
//! (EFFECTS-033 #128-#131).
//!
//! `flow_shader` mounts an animated light-flow material through iced's
//! `widget::shader` (#128); its pipeline is built once per primitive type by
//! the wgpu `Storage` cache (#129). `flow_fallback` draws the same shimmer
//! as plain canvas vectors for the tiny-skia software renderer (#130), and
//! `GpuSurfaceKind` is the typed switch a view asks before choosing (#131):
//! when the wgpu compositor cannot start, iced falls back to tiny-skia at
//! runtime and the vector twin keeps the surface alive.

use crate::types::message::Message;
use crate::view::theme;
use iced::widget::canvas::{self, canvas};
use iced::widget::shader::{Pipeline, Primitive, Program, Shader, Viewport};
use iced::{Color, Element, Length, Rectangle, Renderer, Theme, mouse};
use iced::wgpu;

/// Which drawing engine a surface may use (#131). The wgpu backend carries
/// the shader material; the tiny-skia software renderer keeps the vector
/// twin, so a surface never dies with the backend.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GpuSurfaceKind {
    /// The wgpu shader material.
    Shader,
    /// The tiny-skia vector twin.
    Vector,
}

/// The flow uniform ABI: little-endian f32 words shared with the WGSL.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FlowUniform {
    /// Seconds since the material mounted; drives the band phase.
    pub time: f32,
    /// Widget size in pixels: `(width, height)`.
    pub size: [f32; 2],
    /// Accent tint of the bands: `(r, g, b)`.
    pub tint: [f32; 3],
    /// Base alpha of the material.
    pub alpha: f32,
}

impl FlowUniform {
    /// The packed little-endian upload bytes — 32 bytes, matching the WGSL
    /// struct layout (vec2 + vec3 padding rules included).
    pub fn to_bytes(self) -> [u8; 32] {
        let mut bytes = [0u8; 32];
        bytes[0..4].copy_from_slice(&self.time.to_le_bytes());
        bytes[4..8].copy_from_slice(&self.size[0].to_le_bytes());
        bytes[8..12].copy_from_slice(&self.size[1].to_le_bytes());
        bytes[12..16].copy_from_slice(&self.tint[0].to_le_bytes());
        bytes[16..20].copy_from_slice(&self.tint[1].to_le_bytes());
        bytes[20..24].copy_from_slice(&self.tint[2].to_le_bytes());
        bytes[24..28].copy_from_slice(&self.alpha.to_le_bytes());
        bytes
    }
}

/// The flow material WGSL (#128), exposed for source-hygiene tests.
pub const FLOW_WGSL: &str = r#"
struct FlowUniform {
    time: f32,
    size: vec2<f32>,
    tint: vec3<f32>,
    alpha: f32,
};

@group(0) @binding(0) var<uniform> flow: FlowUniform;

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    // Fullscreen quad in clip space; uv runs 0..1 across the widget.
    var corners = array<vec2<f32>, 6>(
        vec2<f32>(-1.0, -1.0), vec2<f32>(1.0, -1.0), vec2<f32>(1.0, 1.0),
        vec2<f32>(-1.0, -1.0), vec2<f32>(1.0, 1.0), vec2<f32>(-1.0, 1.0),
    );
    let corner = corners[vertex_index];
    var out: VertexOutput;
    out.position = vec4<f32>(corner, 0.0, 1.0);
    out.uv = corner * vec2<f32>(0.5, -0.5) + vec2<f32>(0.5, 0.5);
    return out;
}

@fragment
fn fs_main(frag: VertexOutput) -> @location(0) vec4<f32> {
    // Diagonal light bands sweeping with time; phase and falloff both read
    // the same uv so the software twin can mirror the composition.
    let diagonal = frag.uv.x + frag.uv.y;
    let phase = fract(diagonal * 1.5 - flow.time * 0.35);
    let band = smoothstep(0.42, 0.5, phase) * (1.0 - smoothstep(0.5, 0.58, phase));
    let body = pow(1.0 - abs(diagonal - 1.0) * 0.5, 2.0);
    let ink = flow.tint * (0.10 + 0.65 * band + 0.10 * body);
    return vec4<f32>(ink, flow.alpha * (0.35 + 0.65 * band));
}
"#;

/// The one-time GPU state of the flow material (#129): shader module,
/// pipeline layout and pipeline are created in `Pipeline::new`, which the
/// wgpu `Storage` calls exactly once per primitive type; every frame after
/// reuses them.
#[derive(Debug)]
pub struct FlowPipeline {
    pipeline: wgpu::RenderPipeline,
    uniform: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
}

impl Pipeline for FlowPipeline {
    fn new(device: &wgpu::Device, queue: &wgpu::Queue, format: wgpu::TextureFormat) -> Self
    where
        Self: Sized,
    {
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("infiltrator-flow-shader"),
            source: wgpu::ShaderSource::Wgsl(FLOW_WGSL.into()),
        });
        let bind_group_layout = device.create_bind_group_layout(
            &wgpu::BindGroupLayoutDescriptor {
                label: Some("infiltrator-flow-bind-group-layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            },
        );
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("infiltrator-flow-pipeline-layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("infiltrator-flow-pipeline"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });
        let uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("infiltrator-flow-uniform"),
            size: 32,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("infiltrator-flow-bind-group"),
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                    buffer: &uniform,
                    offset: 0,
                    size: None,
                }),
            }],
        });
        // Upload a neutral first frame so the material never samples
        // uninitialized uniform memory.
        let neutral = FlowUniform {
            time: 0.0,
            size: [1.0, 1.0],
            tint: [0.4, 0.6, 1.0],
            alpha: 0.0,
        };
        queue.write_buffer(&uniform, 0, &neutral.to_bytes());
        Self {
            pipeline,
            uniform,
            bind_group,
        }
    }
}

/// The per-frame flow material instance: the animated facts only.
#[derive(Debug)]
pub struct FlowMaterial {
    uniform: FlowUniform,
}

impl FlowMaterial {
    /// A material tinted by `tint`, animated at `time`.
    pub fn new(time: f32, size: [f32; 2], tint: Color, alpha: f32) -> Self {
        Self {
            uniform: FlowUniform {
                time,
                size,
                tint: [tint.r, tint.g, tint.b],
                alpha,
            },
        }
    }
}

impl Primitive for FlowMaterial {
    type Pipeline = FlowPipeline;

    fn prepare(
        &self,
        _pipeline: &mut Self::Pipeline,
        _device: &wgpu::Device,
        queue: &wgpu::Queue,
        _bounds: &Rectangle,
        _viewport: &Viewport,
    ) {
        queue.write_buffer(&_pipeline.uniform, 0, &self.uniform.to_bytes());
    }

    fn draw(
        &self,
        pipeline: &Self::Pipeline,
        render_pass: &mut wgpu::RenderPass<'_>,
    ) -> bool {
        render_pass.set_pipeline(&pipeline.pipeline);
        render_pass.set_bind_group(0, &pipeline.bind_group, &[]);
        render_pass.draw(0..6, 0..1);
        true
    }
}

/// The shader-program state: only the animation clock travels per widget.
#[derive(Debug)]
pub struct FlowProgram {
    pub time: f32,
    pub size: [f32; 2],
    pub tint: Color,
    pub alpha: f32,
}

impl Default for FlowProgram {
    fn default() -> Self {
        Self {
            time: 0.0,
            size: [1.0, 1.0],
            tint: Color::BLACK,
            alpha: 0.0,
        }
    }
}

impl Program<Message> for FlowProgram {
    type State = ();
    type Primitive = FlowMaterial;

    fn draw(
        &self,
        _state: &(),
        _cursor: mouse::Cursor,
        _bounds: Rectangle,
    ) -> Self::Primitive {
        FlowMaterial::new(self.time, self.size, self.tint, self.alpha)
    }
}

/// The flow surface on the wgpu backend (#128).
pub fn flow_shader(program: FlowProgram) -> Element<'static, Message> {
    Shader::new(program).width(Length::Fill).height(Length::Fill).into()
}

/// The tiny-skia vector twin (#130): the same diagonal band composition in
/// canvas vectors — static (the software renderer repaints on tick anyway),
/// never a shader.
pub struct FlowFallback {
    pub tint: Color,
    pub alpha: f32,
}

impl<Message> canvas::Program<Message> for FlowFallback {
    type State = ();

    fn draw(
        &self,
        _state: &(),
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let mut frame = canvas::Frame::new(renderer, bounds.size());
        // Three diagonal bands across the diagonal, mirroring the WGSL's
        // band math at phase 0.5.
        for (index, strength) in [0.65_f32, 0.35, 0.18].into_iter().enumerate() {
            let offset = index as f32 * 0.28;
            let center = bounds.width * (0.5 - offset);
            let band = canvas::Path::new(|path| {
                path.move_to(iced::Point::new(center - bounds.width * 0.12, 0.0));
                path.line_to(iced::Point::new(center + bounds.width * 0.12, 0.0));
                path.line_to(iced::Point::new(
                    center + bounds.width * 0.12 + bounds.height,
                    bounds.height,
                ));
                path.line_to(iced::Point::new(
                    center - bounds.width * 0.12 + bounds.height,
                    bounds.height,
                ));
                path.close();
            });
            frame.fill(
                &band,
                Color {
                    a: self.alpha * strength,
                    ..self.tint
                },
            );
        }
        vec![frame.into_geometry()]
    }
}

/// The vector-twin surface element.
pub fn flow_fallback(flow: FlowFallback) -> Element<'static, Message> {
    canvas(flow).width(Length::Fill).height(Length::Fill).into()
}

/// Pick the flow surface for the engine kind (#131): the shader material on
/// wgpu, the canvas twin otherwise.
pub fn flow_surface(kind: GpuSurfaceKind, flow: FlowProgram) -> Element<'static, Message> {
    match kind {
        GpuSurfaceKind::Shader => flow_shader(flow),
        GpuSurfaceKind::Vector => flow_fallback(FlowFallback {
            tint: flow.tint,
            alpha: flow.alpha,
        }),
    }
}

/// The palette tint of the flow material for the current theme.
pub fn flow_tint(theme: &Theme) -> Color {
    theme::tokens(theme).accent
}
