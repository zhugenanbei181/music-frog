//! Emit the page-shape table from the contract's typed registry.

use infiltrator_contract::page_structure::page_structure_json;
use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::exit;

fn main() {
    let mut args = env::args_os().skip(1);
    let Some(output) = args.next() else {
        eprintln!("usage: export_page_structure <output.json>");
        exit(2);
    };
    if args.next().is_some() {
        eprintln!("usage: export_page_structure <output.json>");
        exit(2);
    }
    let output = PathBuf::from(output);
    let json = page_structure_json();
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent).expect("page structure output directory");
    }
    fs::write(&output, json).expect("page structure output");
}
