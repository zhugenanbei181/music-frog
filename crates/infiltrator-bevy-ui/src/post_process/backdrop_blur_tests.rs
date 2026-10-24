use super::backdrop_blur::{
    BackdropBlur, DEFAULT_SPAN, KERNEL_FAR_OFFSET, KERNEL_NEAR_OFFSET, kernel_weights,
    pass_texel_size, quarter_size,
};
use bevy::math::{UVec2, Vec2};

#[test]
fn kernel_weights_sum_to_one() {
    let [center, near, far] = kernel_weights();
    let sum = center + 2.0 * near + 2.0 * far;
    assert!(
        (sum - 1.0).abs() < 1e-6,
        "collapsed kernel must stay normalized: {sum}"
    );
    assert!(near > center && center > far);
}

#[test]
fn kernel_offsets_are_the_linear_sampling_reach() {
    // Paired-tap offsets of the linear-sampling collapse; mirrored in
    // backdrop_blur.wesl.
    assert!((KERNEL_NEAR_OFFSET - 1.3846153846).abs() < 1e-9);
    assert!((KERNEL_FAR_OFFSET - 3.2307692308).abs() < 1e-9);
    assert!(KERNEL_FAR_OFFSET > KERNEL_NEAR_OFFSET);
}

#[test]
fn quarter_size_divides_with_a_one_texel_floor() {
    assert_eq!(quarter_size(UVec2::new(1920, 1080)), UVec2::new(480, 270));
    assert_eq!(quarter_size(UVec2::new(1180, 760)), UVec2::new(295, 190));
    assert_eq!(quarter_size(UVec2::new(3, 7)), UVec2::new(1, 2));
    assert_eq!(quarter_size(UVec2::new(4, 1)), UVec2::new(1, 1));
}

#[test]
fn quarter_size_never_returns_zero() {
    for width in 0..12u32 {
        for height in 0..12u32 {
            let size = quarter_size(UVec2::new(width, height));
            assert!(
                size.x >= 1 && size.y >= 1,
                "quarter of {width}x{height} must stay drawable"
            );
        }
    }
}

#[test]
fn pass_texel_size_reciprocates() {
    let texel = pass_texel_size(UVec2::new(480, 270));
    assert!((texel.x - 1.0 / 480.0).abs() < 1e-9);
    assert!((texel.y - 1.0 / 270.0).abs() < 1e-9);
    let zero_free = pass_texel_size(UVec2::new(1, 1));
    assert_eq!(zero_free, Vec2::new(1.0, 1.0));
}

#[test]
fn modal_backdrop_span_is_positive() {
    assert!(BackdropBlur::modal().span > 0.0);
    assert!((DEFAULT_SPAN - 1.0).abs() < 1e-9);
}
