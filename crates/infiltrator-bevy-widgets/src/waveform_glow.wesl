import bevy_ui_render::ui_vertex_output::UiVertexOutput;

struct WaveformUniform {
    samples: array<vec4<f32>, 16>,
    ink: vec4<f32>,
    sample_count: f32,
    opacity: f32,
    line_width: f32,
    glow_width: f32,
    glow_alpha: f32,
    fill_alpha: f32,
}

@group(1) @binding(0)
var<uniform> waveform: WaveformUniform;

fn sample_at(x: f32) -> f32 {
    if (waveform.sample_count < 1.0) {
        return 0.0;
    }
    if (waveform.sample_count < 2.0) {
        return waveform.samples[0].x;
    }
    let last = u32(waveform.sample_count) - 1u;
    let position = clamp(x, 0.0, 1.0) * f32(last);
    let left_index = min(u32(floor(position)), last);
    let right_index = min(left_index + 1u, last);
    let fraction = position - floor(position);
    let left = waveform.samples[left_index / 4u][left_index % 4u];
    let right = waveform.samples[right_index / 4u][right_index % 4u];
    return mix(left, right, fraction);
}

@fragment
fn fragment(in: UiVertexOutput) -> @location(0) vec4<f32> {
    if (waveform.opacity <= 0.0) {
        return vec4<f32>(0.0);
    }
    let curve_y = 1.0 - sample_at(in.uv.x);
    let distance = in.uv.y - curve_y;
    let line_alpha = 1.0 - smoothstep(0.0, waveform.line_width, abs(distance));
    let glow_alpha = exp(-abs(distance) / waveform.glow_width) * waveform.glow_alpha;
    let fill_alpha = smoothstep(0.0, waveform.glow_width, distance) * waveform.fill_alpha;
    let alpha = min(line_alpha + glow_alpha + fill_alpha, 1.0)
        * waveform.ink.a
        * waveform.opacity;
    return vec4<f32>(waveform.ink.rgb, alpha);
}
