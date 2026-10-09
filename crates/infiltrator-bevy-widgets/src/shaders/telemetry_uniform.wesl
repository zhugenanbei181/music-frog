// GPU ABI for telemetry_gpu::TelemetryUniform (ShaderType). Field order, types
// and alignment must stay in lockstep with the Rust struct.
struct TelemetryUniform {
    tint: vec4<f32>,
    opacity: f32,
    fade: f32,
}
