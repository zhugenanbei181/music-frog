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
