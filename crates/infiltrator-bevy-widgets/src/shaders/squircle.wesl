// Minimum screen-space anti-aliasing width in device pixels. Mirrors
// shader_fx::MIN_EDGE_AA_PX so the GPU and CPU coverage paths agree.
const MIN_EDGE_AA_PX: f32 = 0.7;

// Superellipse distance approximation shared by card fill, border and shadow.
fn squircle_sdf(p: vec2<f32>, half_size: vec2<f32>, r: f32, smoothing: f32) -> f32 {
    let safe_r = clamp(r, 0.0, min(half_size.x, half_size.y));
    let q = abs(p) - (half_size - vec2<f32>(safe_r));
    let p_exp = 2.0 + 3.0 * clamp(smoothing, 0.0, 1.0);
    if (q.x > 0.0 && q.y > 0.0) {
        return pow(pow(q.x, p_exp) + pow(q.y, p_exp), 1.0 / p_exp) - safe_r;
    }
    return max(q.x, q.y) - safe_r;
}

// Analytic SDF edge/glyph coverage from a signed distance and its screen-space
// derivative. The CPU mirror is shader_fx::sdf_edge_coverage; the linear CPU
// fallback is shader_fx::sdf_edge_coverage_cpu.
fn sdf_edge_coverage(d: f32, screen_space_derivative: f32) -> f32 {
    let width = max(abs(screen_space_derivative), MIN_EDGE_AA_PX);
    let t = clamp((d + width * 0.5) / width, 0.0, 1.0);
    return 1.0 - t * t * (3.0 - 2.0 * t);
}
