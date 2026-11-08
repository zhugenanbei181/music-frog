//! Static proof that contract binds the shared easing owner and keeps no
//! second cubic-Bézier solver.

use std::fs;
use std::path::PathBuf;

const MOTION_SPEC: &str = "src/motion_spec.rs";

fn motion_spec() -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(MOTION_SPEC);
    fs::read_to_string(path).expect("motion_spec.rs must be readable")
}

#[test]
fn contract_imports_the_shared_easing_owner() {
    let source = motion_spec();
    assert!(
        source.contains("use zhuge_ui_core::easing::CubicBezier;"),
        "contract must import the owner type directly"
    );
    assert!(
        source.contains("CubicBezier::new("),
        "contract must declare its product curves through the owner constructor"
    );
    assert!(
        source.contains(".sample(t)"),
        "contract must evaluate curves through the owner solve"
    );
}

#[test]
fn contract_retired_its_second_solver() {
    let source = motion_spec();
    for retired in [
        "pub struct CubicBezier",
        "pub fn solve(&self",
        "fn sample_x(&self",
        "fn sample_dx(&self",
        "fn sample_y(&self",
    ] {
        assert!(
            !source.contains(retired),
            "retired second solver surface `{retired}` has reappeared"
        );
    }
    assert!(
        source.contains("zhuge_ui_core::easing::CubicBezier"),
        "the owner reference must remain documented next to the binding"
    );
}
