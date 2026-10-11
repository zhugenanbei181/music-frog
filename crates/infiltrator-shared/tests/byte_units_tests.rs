use infiltrator_shared::byte_units::format_bytes;

#[test]
fn binary_unit_thresholds_are_stable_for_both_surfaces() {
    assert_eq!(format_bytes(0), "0 B");
    assert_eq!(format_bytes(1_023), "1023 B");
    assert_eq!(format_bytes(1_024), "1.00 KB");
    assert_eq!(format_bytes(1_048_576), "1.00 MB");
    assert_eq!(format_bytes(1_073_741_824), "1.00 GB");
    assert_eq!(format_bytes(1_099_511_627_776), "1.00 TB");
}
