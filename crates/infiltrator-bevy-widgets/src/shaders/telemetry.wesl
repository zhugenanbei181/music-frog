import package::shaders::telemetry_vertex::VertexOutput;
import package::shaders::telemetry_uniform::TelemetryUniform;

// Material2d binds this material's uniform at `@group(2) @binding(0)`
// (`MATERIAL_2D_BIND_GROUP_INDEX` in bevy_sprite_render mesh2d).
@group(2) @binding(0) var<uniform> telemetry: TelemetryUniform;

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let ink = in.color * telemetry.tint;
    let alpha = ink.a * telemetry.opacity * (1.0 - in.uv.y * telemetry.fade);
    return vec4<f32>(ink.rgb, alpha);
}
