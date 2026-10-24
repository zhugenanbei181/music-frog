// Modal backdrop blur pass (EFFECTS-024 #86–#88): a fullscreen-triangle
// gaussian that runs three times per blurred view — downsample-and-blur into
// the first quarter target, blur into the second, then a span-0 identity
// upscale back into the post-process destination.

struct BackdropBlurParams {
    texel_size: vec2<f32>,
    direction: vec2<f32>,
    span: f32,
};

@group(0) @binding(0) var<uniform> params: BackdropBlurParams;
@group(0) @binding(1) var source_texture: texture_2d<f32>;
@group(0) @binding(2) var source_sampler: sampler;

struct BlurVertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn blur_vertex(@builtin(vertex_index) vertex_index: u32) -> BlurVertexOutput {
    // Fullscreen triangle from the bare vertex index: (0,0) (2,0) (0,2) in UV
    // space, flipped so sample coordinates match texture space.
    let uv = vec2<f32>(f32(vertex_index >> 1u), f32(vertex_index & 1u)) * 2.0;
    var out: BlurVertexOutput;
    out.position = vec4<f32>(uv * vec2<f32>(2.0, -2.0) + vec2<f32>(-1.0, 1.0), 0.0, 1.0);
    out.uv = uv;
    return out;
}

@fragment
fn blur_fragment(frag: BlurVertexOutput) -> @location(0) vec4<f32> {
    // Nine-tap gaussian collapsed into five bilinear samples: the paired taps
    // sit at ±1.3846154 and ±3.2307692 texels and the weights sum to 1.0, so
    // span 0.0 degenerates to an identity bilinear copy — the upscale pass
    // relies on exactly that.
    let axis = params.direction * params.span * params.texel_size;
    let near = axis * 1.3846153846;
    let far = axis * 3.2307692308;
    return textureSample(source_texture, source_sampler, frag.uv) * 0.2270270270
        + (textureSample(source_texture, source_sampler, frag.uv + near)
            + textureSample(source_texture, source_sampler, frag.uv - near)) * 0.3162162162
        + (textureSample(source_texture, source_sampler, frag.uv + far)
            + textureSample(source_texture, source_sampler, frag.uv - far)) * 0.0702702703;
}
