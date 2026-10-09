import bevy_ui_render::ui_vertex_output::UiVertexOutput;
import package::shaders::surface_uniform::ModernSurfaceUniform;
import package::shaders::squircle::{squircle_sdf, sdf_edge_coverage};
import package::shaders::shadow::surface_shadow;

@group(1) @binding(0)
var<uniform> surface: ModernSurfaceUniform;

@fragment
fn fragment(in: UiVertexOutput) -> @location(0) vec4<f32> {
    let half_size = surface.dimensions * 0.5;
    let p = (in.uv - vec2<f32>(0.5)) * surface.dimensions;
    let shadow_alpha = surface_shadow(p, half_size, surface);
    let d_surface = squircle_sdf(p, half_size, surface.radius, surface.smoothing);
    let surface_mask = sdf_edge_coverage(d_surface, fwidth(d_surface));

    var border_mask = 0.0;
    if (surface.border_width > 0.0) {
        let d_inner = d_surface + surface.border_width;
        let inner_mask = sdf_edge_coverage(d_inner, fwidth(d_inner));
        border_mask = surface_mask * (1.0 - inner_mask);
    }
    let card_rgb = mix(surface.color.rgb, surface.border_color.rgb, border_mask);
    let card_alpha = surface_mask * surface.color.a;
    let out_rgb = mix(surface.shadow_color.rgb, card_rgb, card_alpha);
    let out_a = max(card_alpha, shadow_alpha * (1.0 - card_alpha));
    return vec4<f32>(out_rgb, out_a);
}
