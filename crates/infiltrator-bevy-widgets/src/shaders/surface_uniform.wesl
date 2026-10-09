// Field order and types must match shader_fx::ModernSurfaceUniform (ShaderType).
struct ModernSurfaceUniform {
    color: vec4<f32>,
    border_color: vec4<f32>,
    shadow_color: vec4<f32>,
    dimensions: vec2<f32>,
    radius: f32,
    border_width: f32,
    smoothing: f32,
    shadow_offset: vec2<f32>,
    shadow_blur: f32,
    shadow_spread: f32,
    ambient_alpha: f32,
    key_alpha: f32,
}

// Per-instance ABI for the batched card draw. Field order, types and byte
// offsets must match shader_fx::CardInstance and shader_fx::CARD_INSTANCE_LAYOUT
// (translation@0, size@8, shape@16, fill@32, border@48, stride 64).
struct CardInstance {
    translation: vec2<f32>,
    size: vec2<f32>,
    shape: vec4<f32>,
    fill: vec4<f32>,
    border: vec4<f32>,
}
