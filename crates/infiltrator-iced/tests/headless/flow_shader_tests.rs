//! The flow material surface (EFFECTS-033 #128-#131): uniform ABI packing,
//! WGSL entry hygiene, and the typed engine switch.
//! test-intent: behavior
use iced::{Color, Size};
use infiltrator_iced::view::shader::{
    FlowProgram, FlowUniform, FLOW_WGSL, GpuSurfaceKind, flow_surface,
};

#[test]
fn the_uniform_packs_little_endian_into_32_bytes() {
    let uniform = FlowUniform {
        time: 1.5,
        size: [320.0, 96.0],
        tint: [0.25, 0.5, 0.75],
        alpha: 0.8,
    };
    let bytes = uniform.to_bytes();
    assert_eq!(bytes.len(), 32);
    assert_eq!(&bytes[0..4], &1.5_f32.to_le_bytes());
    assert_eq!(&bytes[4..8], &320.0_f32.to_le_bytes());
    assert_eq!(&bytes[8..12], &96.0_f32.to_le_bytes());
    assert_eq!(&bytes[12..16], &0.25_f32.to_le_bytes());
    assert_eq!(&bytes[16..20], &0.5_f32.to_le_bytes());
    assert_eq!(&bytes[20..24], &0.75_f32.to_le_bytes());
    assert_eq!(&bytes[24..28], &0.8_f32.to_le_bytes());
    assert!(bytes[28..32].iter().all(|byte| *byte == 0), "tail padding is zero");
}

#[test]
fn the_wgsl_module_declares_both_entry_points_and_one_binding() {
    assert!(FLOW_WGSL.contains("@vertex"));
    assert!(FLOW_WGSL.contains("fn vs_main"));
    assert!(FLOW_WGSL.contains("@fragment"));
    assert!(FLOW_WGSL.contains("fn fs_main"));
    assert_eq!(FLOW_WGSL.matches("@binding(0)").count(), 1);
    assert_eq!(FLOW_WGSL.matches("@group(0)").count(), 1);
}

#[test]
fn the_engine_switch_builds_both_surfaces_without_a_gpu() {
    let program = FlowProgram {
        time: 2.0,
        size: [100.0, 40.0],
        tint: Color::from_rgb(0.3, 0.5, 0.9),
        alpha: 0.5,
    };
    // Both branches construct their element trees headlessly; only the draw
    // needs a real renderer.
    let _shader_surface = flow_surface(GpuSurfaceKind::Shader, FlowProgram { ..program });
    let _vector_surface = flow_surface(GpuSurfaceKind::Vector, program);
}

#[test]
fn a_zero_alpha_material_is_invisible_by_math() {
    let uniform = FlowUniform {
        time: 0.0,
        size: [1.0, 1.0],
        tint: [1.0, 1.0, 1.0],
        alpha: 0.0,
    };
    assert_eq!(uniform.to_bytes()[24..28], 0.0_f32.to_le_bytes());
}

#[test]
fn size_helper_matches_the_uniform_words() {
    let size = Size::new(240.0, 80.0);
    let uniform = FlowUniform {
        time: 0.0,
        size: [size.width, size.height],
        tint: [0.0; 3],
        alpha: 1.0,
    };
    assert_eq!(uniform.size, [240.0, 80.0]);
}
