//! Modal backdrop blur as a Core2d post-process (EFFECTS-024 #86–#88).
//!
//! [`BackdropBlur`] rides the camera the same way `Tonemapping` does:
//! presence enables the pass, `span` scales the gaussian reach. The render
//! system runs three fullscreen passes per blurred view —
//!
//! 1. `source → horizontal`: 4× downsample plus a horizontal gaussian,
//! 2. `horizontal → vertical`: vertical gaussian at quarter resolution,
//! 3. `vertical → destination`: span-0 identity bilinear upscale,
//!
//! calling [`ViewTarget::post_process_write`] exactly once, as the
//! post-process contract requires. The #89 foreground composite stays a
//! documented integration boundary: the modal dialog itself renders above
//! the blurred backdrop only as a foreground surface; wiring that layer is
//! the modal surface's slice, not the blur pipeline's.

use std::collections::HashMap;

use bevy::app::{App, Plugin, Update};
use bevy::asset::{AssetServer, Handle, load_embedded_asset};
use bevy::camera::{Camera, Camera2d};
use bevy::core_pipeline::Core2dSystems;
use bevy::core_pipeline::tonemapping::tonemapping;
use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::query::With;
use bevy::ecs::resource::Resource;
use bevy::ecs::schedule::IntoScheduleConfigs;
use bevy::ecs::system::{Commands, Local, Query, Res, ResMut};
use bevy::math::{UVec2, Vec2};
use bevy::render::camera::ExtractedCamera;
use bevy::render::extract_component::{ExtractComponent, ExtractComponentPlugin};
use bevy::render::render_resource::{
    AddressMode, BindGroupEntries, BindGroupLayout, BindGroupLayoutDescriptor,
    BindGroupLayoutEntries, BufferBinding, BufferInitDescriptor, BufferUsages,
    CachedRenderPipelineId, ColorTargetState, ColorWrites, Extent3d, FilterMode, FragmentState,
    LoadOp, Operations, PipelineCache, RenderPassColorAttachment, RenderPassDescriptor,
    RenderPipeline, RenderPipelineDescriptor, Sampler, SamplerBindingType, SamplerDescriptor,
    ShaderStages, ShaderType, StoreOp, TextureDescriptor, TextureDimension, TextureFormat,
    TextureSampleType, TextureUsages, TextureView, TextureViewDescriptor, VertexState,
    binding_types::{sampler, texture_2d, uniform_buffer},
    encase::UniformBuffer,
};
use bevy::render::renderer::{RenderContext, RenderDevice, ViewQuery};
use bevy::render::view::ViewTarget;
use bevy::render::{Render, RenderApp, RenderStartup};
use bevy::shader::Shader;
use infiltrator_bevy_widgets::modal::{ModalStage, ModalState};

/// Weight of the gaussian center tap in the collapsed nine-tap kernel.
pub const KERNEL_CENTER_WEIGHT: f32 = 0.227_027_03;
/// Weight of each tap pair at ±[`KERNEL_NEAR_OFFSET`] texels.
pub const KERNEL_NEAR_WEIGHT: f32 = 0.316_216_23;
/// Weight of each tap pair at ±[`KERNEL_FAR_OFFSET`] texels.
pub const KERNEL_FAR_WEIGHT: f32 = 0.070_270_27;
/// Texel reach of the near sample pair in the linear-sampling collapse.
pub const KERNEL_NEAR_OFFSET: f32 = 1.384_615_4;
/// Texel reach of the far sample pair in the linear-sampling collapse.
pub const KERNEL_FAR_OFFSET: f32 = 3.230_769_2;
/// The calibrated modal backdrop reach a gated camera carries.
pub const DEFAULT_SPAN: f32 = 1.0;

/// The collapsed nine-tap kernel weights: center, near pair, far pair. They
/// sum to 1.0 once the two pairs count twice, so a span of 0 degenerates to
/// an identity bilinear copy.
pub fn kernel_weights() -> [f32; 3] {
    [KERNEL_CENTER_WEIGHT, KERNEL_NEAR_WEIGHT, KERNEL_FAR_WEIGHT]
}

/// Quarter-resolution blur target size: each axis divides by four with a
/// one-texel floor so degenerate viewports still allocate a valid texture.
pub fn quarter_size(full: UVec2) -> UVec2 {
    UVec2::new(full.x.div_ceil(4).max(1), full.y.div_ceil(4).max(1))
}

/// Reciprocal texture size for one sampling pass over `size`.
pub fn pass_texel_size(size: UVec2) -> Vec2 {
    Vec2::new(1.0 / size.x as f32, 1.0 / size.y as f32)
}

/// Enables the modal backdrop blur on a camera, mirroring how `Tonemapping`
/// gates its pass: presence turns the pass on, `span` scales the gaussian
/// reach. [`ModalStage`] remains the single business authority; the main
/// world gate below only projects that typed stage onto this render toggle.
#[derive(Component, Clone, Copy, Debug, ExtractComponent, PartialEq)]
#[extract_component_filter(With<Camera>)]
#[extract_app(RenderApp)]
pub struct BackdropBlur {
    /// Gaussian reach multiplier; [`DEFAULT_SPAN`] is the calibrated modal
    /// backdrop and 0.0 degenerates every pass to an identity copy.
    pub span: f32,
}

impl Default for BackdropBlur {
    fn default() -> Self {
        Self { span: DEFAULT_SPAN }
    }
}

impl BackdropBlur {
    /// The calibrated modal backdrop reach.
    pub const fn modal() -> Self {
        Self { span: DEFAULT_SPAN }
    }
}

/// GPU ABI for `backdrop_blur.wesl`; field order and types must match the
/// WESL `BackdropBlurParams` struct exactly.
#[derive(ShaderType, Clone, Copy, Debug, PartialEq)]
pub struct BackdropBlurParams {
    /// Reciprocal of the sampled texture's size.
    pub texel_size: Vec2,
    /// Sampling axis of this pass; a horizontal or vertical unit vector.
    pub direction: Vec2,
    /// Gaussian reach multiplier; 0.0 degenerates to an identity copy.
    pub span: f32,
}

/// The two offscreen quarter-resolution intermediates for one blurred view.
/// They reallocate only when the quarter size or texture format changes.
pub struct QuarterTargets {
    size: UVec2,
    format: TextureFormat,
    horizontal: TextureView,
    vertical: TextureView,
}

impl QuarterTargets {
    fn new(render_device: &RenderDevice, size: UVec2, format: TextureFormat) -> Self {
        let descriptor = TextureDescriptor {
            label: Some("backdrop_blur_quarter_texture"),
            size: Extent3d {
                width: size.x,
                height: size.y,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format,
            usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        };
        Self {
            size,
            format,
            horizontal: render_device
                .create_texture(&descriptor)
                .create_view(&TextureViewDescriptor::default()),
            vertical: render_device
                .create_texture(&descriptor)
                .create_view(&TextureViewDescriptor::default()),
        }
    }
}

/// Shared GPU state for the blur: the bind group layout descriptor, the
/// linear sampler, the shader handle and one cached pipeline id per target
/// texture format. `PipelineCache::queue_render_pipeline` does not
/// deduplicate, so each format queues exactly once and the id is reused.
#[derive(Resource)]
pub struct BackdropBlurPipeline {
    layout: BindGroupLayoutDescriptor,
    sampler: Sampler,
    shader: Handle<Shader>,
    pipelines: HashMap<TextureFormat, CachedRenderPipelineId>,
}

impl BackdropBlurPipeline {
    /// The pipeline id for `format`, queueing the descriptor on first sight.
    fn pipeline_for(
        &mut self,
        pipeline_cache: &PipelineCache,
        format: TextureFormat,
    ) -> CachedRenderPipelineId {
        if let Some(id) = self.pipelines.get(&format) {
            return *id;
        }
        let descriptor = RenderPipelineDescriptor {
            label: Some("backdrop_blur_pipeline".into()),
            layout: vec![self.layout.clone()],
            vertex: VertexState {
                shader: self.shader.clone(),
                entry_point: Some("blur_vertex".into()),
                ..VertexState::default()
            },
            fragment: Some(FragmentState {
                shader: self.shader.clone(),
                entry_point: Some("blur_fragment".into()),
                targets: vec![Some(ColorTargetState {
                    format,
                    blend: None,
                    write_mask: ColorWrites::ALL,
                })],
                ..FragmentState::default()
            }),
            ..RenderPipelineDescriptor::default()
        };
        let id = pipeline_cache.queue_render_pipeline(descriptor);
        self.pipelines.insert(format, id);
        id
    }
}

/// Create the shared blur GPU state in the render world.
pub fn init_backdrop_blur_pipeline(
    mut commands: Commands,
    render_device: Res<RenderDevice>,
    asset_server: Res<AssetServer>,
) {
    let layout = BindGroupLayoutDescriptor::new(
        "backdrop_blur_bind_group_layout",
        &BindGroupLayoutEntries::sequential(
            ShaderStages::FRAGMENT,
            (
                uniform_buffer::<BackdropBlurParams>(false),
                texture_2d(TextureSampleType::Float { filterable: true }),
                sampler(SamplerBindingType::Filtering),
            ),
        ),
    );
    let sampler = render_device.create_sampler(&SamplerDescriptor {
        address_mode_u: AddressMode::ClampToEdge,
        address_mode_v: AddressMode::ClampToEdge,
        address_mode_w: AddressMode::ClampToEdge,
        mag_filter: FilterMode::Linear,
        min_filter: FilterMode::Linear,
        ..SamplerDescriptor::default()
    });
    let shader = load_embedded_asset!(asset_server.as_ref(), "backdrop_blur.wesl");
    commands.insert_resource(BackdropBlurPipeline {
        layout,
        sampler,
        shader,
        pipelines: HashMap::new(),
    });
}

/// Project the typed modal stage onto the camera's blur gate: a presented
/// dialog (Open, or the Dismissed fade-out) blurs the backdrop; Hidden
/// removes the pass. [`ModalStage`] stays the single business authority —
/// this gate holds no second copy of the stage, only the projection of it.
pub fn sync_backdrop_blur_gate(
    mut commands: Commands,
    cameras: Query<(Entity, Option<&BackdropBlur>), With<Camera2d>>,
    modals: Query<&ModalState>,
) {
    let blurred = modals.iter().any(|modal| modal.stage != ModalStage::Hidden);
    let mut to_insert = Vec::new();
    let mut to_remove = Vec::new();
    for (entity, blur) in cameras.iter() {
        match (blurred, blur) {
            (true, None) => to_insert.push(entity),
            (false, Some(_)) => to_remove.push(entity),
            _ => {}
        }
    }
    for entity in to_insert {
        commands.entity(entity).insert(BackdropBlur::modal());
    }
    for entity in to_remove {
        commands.entity(entity).remove::<BackdropBlur>();
    }
}

struct BlurPassStep<'a> {
    source_texture: &'a TextureView,
    destination: &'a TextureView,
    texel_size: Vec2,
    direction: Vec2,
    span: f32,
}

/// Run one fullscreen blur pass from `step.source_texture` into `step.destination`.
fn run_blur_pass(
    ctx: &mut RenderContext,
    pipeline: &RenderPipeline,
    bind_group_layout: &BindGroupLayout,
    sampler: &Sampler,
    step: BlurPassStep,
) {
    let render_device = ctx.render_device();
    let params = BackdropBlurParams {
        texel_size: step.texel_size,
        direction: step.direction,
        span: step.span,
    };
    let mut writer = UniformBuffer::new(Vec::new());
    writer
        .write(&params)
        .expect("blur params fit the uniform layout");
    let params_buffer = render_device.create_buffer_with_data(&BufferInitDescriptor {
        label: Some("backdrop_blur_params_buffer"),
        contents: &writer.into_inner(),
        usage: BufferUsages::UNIFORM,
    });
    let params_binding = BufferBinding {
        buffer: &params_buffer,
        offset: 0,
        size: None,
    };
    let bind_group = render_device.create_bind_group(
        "backdrop_blur_bind_group",
        bind_group_layout,
        &BindGroupEntries::sequential((params_binding, step.source_texture, sampler)),
    );
    let pass_descriptor = RenderPassDescriptor {
        label: Some("backdrop_blur_pass"),
        color_attachments: &[Some(RenderPassColorAttachment {
            view: step.destination,
            depth_slice: None,
            resolve_target: None,
            ops: Operations {
                load: LoadOp::Clear(Default::default()),
                store: StoreOp::Store,
            },
        })],
        depth_stencil_attachment: None,
        timestamp_writes: None,
        occlusion_query_set: None,
        multiview_mask: None,
    };
    let mut render_pass = ctx.command_encoder().begin_render_pass(&pass_descriptor);
    render_pass.set_pipeline(pipeline);
    render_pass.set_bind_group(0, &bind_group, &[]);
    render_pass.draw(0..3, 0..1);
}

/// Blur one view's backdrop: downsample-and-blur horizontally into the first
/// quarter target, blur vertically into the second, then upscale back into
/// the post-process destination with a span-0 identity pass. The system runs
/// in `Core2dSystems::PostProcess` before tonemapping and calls
/// `ViewTarget::post_process_write` exactly once.
///
/// One blurred view per app is supported: the shell mounts a single
/// `Camera2d`, and the quarter targets live in this system's `Local` state.
pub fn backdrop_blur(
    view: ViewQuery<(&ExtractedCamera, &ViewTarget, &BackdropBlur)>,
    pipeline_cache: Res<PipelineCache>,
    mut blur_pipeline: ResMut<BackdropBlurPipeline>,
    mut quarters: Local<Option<QuarterTargets>>,
    mut ctx: RenderContext,
) {
    let (camera, target, blur) = view.into_inner();
    let Some(full) = camera.physical_viewport_size else {
        return;
    };
    let quarter = quarter_size(full);
    let format = target.main_texture_format();
    let needs_new_targets = quarters
        .as_ref()
        .is_none_or(|targets| targets.size != quarter || targets.format != format);
    if needs_new_targets {
        *quarters = Some(QuarterTargets::new(ctx.render_device(), quarter, format));
    }
    let targets = quarters.as_ref().expect("quarter targets just ensured");

    let pipeline_id = blur_pipeline.pipeline_for(&pipeline_cache, format);
    let Some(pipeline) = pipeline_cache.get_render_pipeline(pipeline_id) else {
        return;
    };
    let post_process = target.post_process_write();
    let bind_group_layout = pipeline_cache.get_bind_group_layout(&blur_pipeline.layout);
    let full_texel = pass_texel_size(full);
    let quarter_texel = pass_texel_size(quarter);

    // Full-resolution source → quarter H: horizontal gaussian + downsample.
    run_blur_pass(
        &mut ctx,
        pipeline,
        &bind_group_layout,
        &blur_pipeline.sampler,
        BlurPassStep {
            source_texture: post_process.source,
            destination: &targets.horizontal,
            texel_size: full_texel,
            direction: Vec2::new(1.0, 0.0),
            span: blur.span,
        },
    );
    // Quarter H → quarter V: vertical gaussian at quarter resolution.
    run_blur_pass(
        &mut ctx,
        pipeline,
        &bind_group_layout,
        &blur_pipeline.sampler,
        BlurPassStep {
            source_texture: &targets.horizontal,
            destination: &targets.vertical,
            texel_size: quarter_texel,
            direction: Vec2::new(0.0, 1.0),
            span: blur.span,
        },
    );
    // Quarter V → destination: span-0 identity bilinear upscale.
    run_blur_pass(
        &mut ctx,
        pipeline,
        &bind_group_layout,
        &blur_pipeline.sampler,
        BlurPassStep {
            source_texture: &targets.vertical,
            destination: post_process.destination,
            texel_size: quarter_texel,
            direction: Vec2::new(1.0, 0.0),
            span: 0.0,
        },
    );
}

/// Mounts the modal backdrop blur post-process on the Bevy surface. The
/// render wiring only installs when the render sub-app exists, so renderless
/// tests keep the gate inert.
pub struct BackdropBlurPlugin;

impl Plugin for BackdropBlurPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(ExtractComponentPlugin::<BackdropBlur>::default());
        app.add_systems(Update, sync_backdrop_blur_gate);
        let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
            return;
        };
        render_app
            .add_systems(RenderStartup, init_backdrop_blur_pipeline)
            .add_systems(
                Render,
                backdrop_blur
                    .in_set(Core2dSystems::PostProcess)
                    .before(tonemapping),
            );
    }
}
