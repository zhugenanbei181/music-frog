import package::shaders::surface_uniform::ModernSurfaceUniform;
import package::shaders::squircle::squircle_sdf;

fn shadow_falloff(d: f32, blur: f32) -> f32 {
    if (d <= 0.0) {
        return 1.0;
    }
    if (d >= blur || blur <= 0.001) {
        return 0.0;
    }
    let t = clamp(d / blur, 0.0, 1.0);
    return 1.0 - t * t * (3.0 - 2.0 * t);
}

fn surface_shadow(p: vec2<f32>, half_size: vec2<f32>, surface: ModernSurfaceUniform) -> f32 {
    if (surface.shadow_blur <= 0.0) {
        return 0.0;
    }
    let d_key = squircle_sdf(
        p - surface.shadow_offset,
        half_size + vec2<f32>(surface.shadow_spread),
        surface.radius + surface.shadow_spread,
        surface.smoothing,
    );
    let key_factor = shadow_falloff(d_key, surface.shadow_blur) * surface.key_alpha;
    let d_amb = squircle_sdf(
        p,
        half_size + vec2<f32>(surface.shadow_spread * 0.5),
        surface.radius + surface.shadow_spread * 0.5,
        surface.smoothing,
    );
    let amb_factor = shadow_falloff(d_amb, surface.shadow_blur * 1.5) * surface.ambient_alpha;
    return min(key_factor + amb_factor, 1.0) * surface.shadow_color.a;
}
