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
