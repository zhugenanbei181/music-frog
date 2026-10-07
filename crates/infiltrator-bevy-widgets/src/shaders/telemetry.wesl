// Prototype only: BANDROID-013 still needs Mesh2d view transforms and a pipeline.
import package::shaders::telemetry_vertex::{VertexOutput, vertex};

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let alpha = in.color.a * (1.0 - in.uv.y * 0.75);
    return vec4<f32>(in.color.rgb, alpha);
}
