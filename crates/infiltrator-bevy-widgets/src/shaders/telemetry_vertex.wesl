// Mesh2d telemetry vertex stage.
//
// Binding layout verified against Bevy 0.20.0 GA `bevy_sprite_render`:
// `mesh2d/view_bindings.wesl` binds `View` at `@group(0) @binding(0)`
// (`ViewUniform`, visibility VERTEX_FRAGMENT), and the material bind group is
// `@group(2)` (`MATERIAL_2D_BIND_GROUP_INDEX`). The world-to-clip field is
// `clip_from_world`, the same matrix `mesh2d_position_world_to_clip` uses.
//
// Vertex locations follow the mesh2d pipeline's fixed attribute mapping:
// position -> 0, uv -> 2, color -> 4.
import bevy_render::view::View;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(4) color: vec4<f32>,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) uv: vec2<f32>,
}

@group(0) @binding(0) var<uniform> view: View;

@vertex
fn vertex(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.clip_position = view.clip_from_world * vec4<f32>(in.position, 1.0);
    out.color = in.color;
    out.uv = in.uv;
    return out;
}
