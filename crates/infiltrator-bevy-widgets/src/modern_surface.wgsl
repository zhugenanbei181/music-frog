#import bevy_ui::ui_vertex_output::UiVertexOutput

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

@group(1) @binding(0)
var<uniform> surface: ModernSurfaceUniform;

// G2 Continuous Curvature Superellipse (Squircle) Signed Distance Field
fn squircle_sdf(p: vec2<f32>, half_size: vec2<f32>, r: f32, smoothing: f32) -> f32 {
    let safe_r = clamp(r, 0.0, min(half_size.x, half_size.y));
    let q = abs(p) - (half_size - vec2<f32>(safe_r));
    let p_exp = 2.0 + 3.0 * clamp(smoothing, 0.0, 1.0);

    if (q.x > 0.0 && q.y > 0.0) {
        // Superellipse corner: (q.x^p + q.y^p)^(1/p) - r
        let d = pow(pow(q.x, p_exp) + pow(q.y, p_exp), 1.0 / p_exp) - safe_r;
        return d;
    } else {
        // Straight edge & inner region
        return max(q.x, q.y) - safe_r;
    }
}

// Hermite cubic analytical falloff for soft penumbra
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

@fragment
fn fragment(in: UiVertexOutput) -> @location(0) vec4<f32> {
    let half_size = surface.dimensions * 0.5;
    let p = (in.uv - vec2<f32>(0.5)) * surface.dimensions;

    // 1. Analytical Soft Drop Shadow (Dual-Layer: Key Light + Ambient Light)
    var shadow_alpha = 0.0;
    if (surface.shadow_blur > 0.0) {
        // Key Light Shadow (Directional Offset)
        let p_key = p - surface.shadow_offset;
        let half_key = half_size + vec2<f32>(surface.shadow_spread);
        let r_key = surface.radius + surface.shadow_spread;
        let d_key = squircle_sdf(p_key, half_key, r_key, surface.smoothing);
        let key_factor = shadow_falloff(d_key, surface.shadow_blur) * surface.key_alpha;

        // Ambient Light Shadow (Non-directional surrounding diffuse)
        let half_amb = half_size + vec2<f32>(surface.shadow_spread * 0.5);
        let r_amb = surface.radius + surface.shadow_spread * 0.5;
        let d_amb = squircle_sdf(p, half_amb, r_amb, surface.smoothing);
        let amb_factor = shadow_falloff(d_amb, surface.shadow_blur * 1.5) * surface.ambient_alpha;

        shadow_alpha = min(key_factor + amb_factor, 1.0) * surface.shadow_color.a;
    }

    // 2. Surface Card Distance & Subpixel Antialiasing
    let d_surface = squircle_sdf(p, half_size, surface.radius, surface.smoothing);
    let aa = max(fwidth(d_surface), 0.7);
    let surface_mask = 1.0 - smoothstep(-aa * 0.5, aa * 0.5, d_surface);

    // 3. Border Layer
    var border_mask = 0.0;
    var fill_mask = surface_mask;
    if (surface.border_width > 0.0) {
        let d_inner = d_surface + surface.border_width;
        let inner_mask = 1.0 - smoothstep(-aa * 0.5, aa * 0.5, d_inner);
        border_mask = surface_mask * (1.0 - inner_mask);
        fill_mask = inner_mask;
    }

    // 4. Single-Pass Physical Composite
    let card_rgb = mix(surface.color.rgb, surface.border_color.rgb, border_mask);
    let card_alpha = surface_mask * surface.color.a;

    let shadow_rgb = surface.shadow_color.rgb;
    let out_rgb = mix(shadow_rgb, card_rgb, card_alpha);
    let out_a = max(card_alpha, shadow_alpha * (1.0 - card_alpha));

    return vec4<f32>(out_rgb, out_a);
}
