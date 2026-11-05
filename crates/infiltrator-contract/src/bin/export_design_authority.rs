//! Emit the cross-language design authority from the contract's typed tables.

use infiltrator_contract::design_resources::design_authority_json;
use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::exit;

fn main() {
    let mut args = env::args_os().skip(1);
    let Some(output) = args.next() else {
        eprintln!("usage: export_design_authority <output.json>");
        exit(2);
    };
    if args.next().is_some() {
        eprintln!("usage: export_design_authority <output.json>");
        exit(2);
    }
    let output = PathBuf::from(output);
    let json = design_authority_json();
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent).expect("design authority output directory");
    }
    fs::write(&output, format!("{json}\n")).expect("design authority output");
}
