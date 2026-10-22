//! The static guard of the design-token contract (EFFECTS-002 #6, #7, #8).
//!
//! The guard reads the crate's own sources and proves two things:
//!
//! 1. every hard-coded length the contract declares belongs to a closed
//!    channel — the spacing ladder, the radius ladder, the physical-size
//!    whitelist or the chrome heights — so a size nobody declared is a
//!    failure instead of a style choice;
//! 2. the radius roles stay mutually exclusive: every role owns exactly one
//!    rung, no two roles share a rung and no rung is declared without an
//!    owner.
//!
//! The guard reads files, it does not run the product. The behaviour of the
//! ladders themselves is proven by the unit tests of the modules that own
//! them.

use std::fs;
use std::path::{Path, PathBuf};

use infiltrator_contract::design_tokens::radius::RadiusRole;
use infiltrator_contract::design_tokens::space::{
    ChromeHeight, PHYSICAL_SIZE_WHITELIST, PhysicalSize, SpaceStep, is_on_grid,
};

/// The names of the type-scale constants.
///
/// A font size is not a spatial length: the type ladder is its own channel,
/// so these names are the only ones the token file may spend on a value that
/// is not a spacing, radius, physical or chrome length.
const TYPOGRAPHY_NAMES: [&str; 10] = [
    "DISPLAY",
    "TITLE",
    "HEADING",
    "BODY",
    "BODY_LARGE",
    "LABEL_LARGE",
    "CAPTION",
    "LABEL_SMALL",
    "TAG",
    "MONO",
];

/// The file that owns the shared design tokens.
const TOKEN_FILE: &str = "src/design_tokens.rs";

/// One hard-coded length declaration found in a source file.
#[derive(Clone)]
struct Declaration {
    /// The path of the file, relative to the crate root.
    file: String,
    /// The name of the constant.
    name: String,
    /// The literal value of the constant.
    value: f32,
}

/// The rule that decides which channel a declaration of one file may spend.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Rule {
    /// The value must be a rung of the spacing ladder.
    Layout,
    /// The value must be a rung of the radius ladder.
    Radius,
    /// The value must be a whitelisted physical size or a chrome height.
    PhysicalOrChrome,
    /// The value must be a whitelisted physical size.
    Physical,
    /// The name must end with `_ALPHA` and the value a translucent ratio.
    Alpha,
    /// The value must be a viewport breakpoint on the 4pt grid.
    Viewport,
}

/// The rule that owns the hard-coded lengths of one file.
fn rule_of(file: &str) -> Option<Rule> {
    match file {
        "src/space.rs" => Some(Rule::Layout),
        "src/radius.rs" => Some(Rule::Radius),
        TOKEN_FILE => Some(Rule::PhysicalOrChrome),
        "src/ime.rs" => Some(Rule::Physical),
        "src/text_truncation.rs" => Some(Rule::Layout),
        "src/responsive_viewport.rs" => Some(Rule::Viewport),
        "src/palette.rs" => Some(Rule::Alpha),
        _ => None,
    }
}

/// Whether a rung of the spacing ladder carries this length.
fn is_layout_value(value: f32) -> bool {
    for step in SpaceStep::ALL {
        if step.px() == value {
            return true;
        }
    }
    false
}

/// Whether a rung of the radius ladder carries this length.
fn is_radius_value(value: f32) -> bool {
    for role in RadiusRole::ALL {
        if role.rung() == value {
            return true;
        }
    }
    false
}

/// Whether a whitelisted physical size carries this length.
fn is_physical_value(value: f32) -> bool {
    for size in PhysicalSize::ALL {
        if size.px() == value {
            return true;
        }
    }
    false
}

/// Whether a reserved chrome height carries this length.
fn is_chrome_value(value: f32) -> bool {
    for height in ChromeHeight::ALL {
        if height.px() == value {
            return true;
        }
    }
    false
}

/// Whether one declaration stays inside the channel its file owns.
fn accepts(rule: Rule, declaration: &Declaration) -> bool {
    let value = declaration.value;
    if declaration.file == TOKEN_FILE && TYPOGRAPHY_NAMES.contains(&declaration.name.as_str()) {
        // A font size is not a spatial length: the type ladder is its own
        // channel inside the token file.
        return value > 0.0 && value.is_finite();
    }
    match rule {
        Rule::Layout => is_layout_value(value),
        Rule::Radius => is_radius_value(value),
        Rule::Physical => is_physical_value(value),
        Rule::PhysicalOrChrome => is_physical_value(value) || is_chrome_value(value),
        Rule::Alpha => declaration.name.ends_with("_ALPHA") && value > 0.0 && value < 1.0,
        Rule::Viewport => value > 0.0 && is_on_grid(value),
    }
}

/// The root of the crate under test.
fn crate_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).to_path_buf()
}

/// Every Rust source file of the crate.
fn source_files(root: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    collect_source_files(&root.join("src"), &mut files);
    files.sort();
    files
}

fn collect_source_files(directory: &Path, files: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(directory) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_source_files(&path, files);
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            files.push(path);
        }
    }
}

/// Every hard-coded length the crate declares.
fn all_declarations(root: &Path) -> Vec<Declaration> {
    let mut declarations = Vec::new();
    for file in source_files(root) {
        let Ok(text) = fs::read_to_string(&file) else {
            continue;
        };
        let relative = file
            .strip_prefix(root)
            .unwrap_or(file.as_path())
            .to_string_lossy()
            .replace('\\', "/");
        declarations.extend(declarations_of(&relative, &text));
    }
    declarations
}

/// The declarations of one file.
fn declarations_of_file(declarations: &[Declaration], file: &str) -> Vec<Declaration> {
    let mut selected = Vec::new();
    for declaration in declarations {
        if declaration.file == file {
            selected.push(declaration.clone());
        }
    }
    selected
}

/// How many declarations carry the given length.
fn declarations_with(declarations: &[Declaration], value: f32) -> usize {
    let mut count = 0;
    for declaration in declarations {
        if declaration.value == value {
            count += 1;
        }
    }
    count
}

/// The `pub const NAME: f32 = LITERAL;` declarations of one file.
///
/// The comments are stripped first: a declaration that only exists inside a
/// doc comment is documentation, not a hard-coded size.
fn declarations_of(relative: &str, text: &str) -> Vec<Declaration> {
    let mut declarations = Vec::new();
    let code = strip_comments(text);
    let mut cursor = 0usize;
    while let Some(offset) = code[cursor..].find("pub const ") {
        let start = cursor + offset + "pub const ".len();
        let after = &code[start..];
        let Some(end) = after.find(';') else {
            break;
        };
        cursor = start + end + 1;
        let Some((name, value)) = parse_length(&after[..end]) else {
            continue;
        };
        declarations.push(Declaration {
            file: relative.to_owned(),
            name,
            value,
        });
    }
    declarations
}

/// The source without its line comments.
fn strip_comments(text: &str) -> String {
    let mut code = String::with_capacity(text.len());
    for line in text.lines() {
        match line.find("//") {
            Some(index) => code.push_str(&line[..index]),
            None => code.push_str(line),
        }
        code.push('\n');
    }
    code
}

/// Parse `NAME: f32 = 12.5` into the name and the literal value.
fn parse_length(statement: &str) -> Option<(String, f32)> {
    let (head, tail) = statement.split_once(':')?;
    let name = head.trim();
    if name.is_empty()
        || !name
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
    {
        return None;
    }
    let (kind, value) = tail.split_once('=')?;
    if kind.trim() != "f32" {
        return None;
    }
    let value = value.trim();
    let parsed = value.parse::<f32>().ok()?;
    Some((name.to_owned(), parsed))
}

#[test]
fn every_hard_coded_length_belongs_to_a_declared_channel() {
    let root = crate_root();
    let declarations = all_declarations(&root);
    assert!(
        !declarations.is_empty(),
        "the guard found no hard-coded length to check"
    );
    for declaration in &declarations {
        let Some(rule) = rule_of(&declaration.file) else {
            panic!(
                "{}:{} declares {}px, but the file owns no length channel",
                declaration.file, declaration.name, declaration.value
            );
        };
        assert!(
            accepts(rule, declaration),
            "{}:{} declares {}px, which is outside the channel this file owns",
            declaration.file,
            declaration.name,
            declaration.value
        );
    }
}

#[test]
fn every_rung_of_the_ladders_is_declared_in_the_source() {
    let root = crate_root();
    let declarations = all_declarations(&root);
    let space = declarations_of_file(&declarations, "src/space.rs");
    for step in SpaceStep::ALL {
        assert!(
            declarations_with(&space, step.px()) > 0,
            "the rung {step:?} is not declared in src/space.rs"
        );
    }
    for size in PhysicalSize::ALL {
        assert!(
            declarations_with(&declarations, size.px()) > 0,
            "the physical size {size:?} is not declared in the token file"
        );
    }
}

#[test]
fn the_radius_roles_stay_mutually_exclusive() {
    let root = crate_root();
    let declarations = all_declarations(&root);
    let radius = declarations_of_file(&declarations, "src/radius.rs");
    assert_eq!(
        radius.len(),
        RadiusRole::ALL.len(),
        "the radius ladder declares a rung that no role owns"
    );
    for (index, role) in RadiusRole::ALL.iter().enumerate() {
        for other in RadiusRole::ALL.iter().skip(index + 1) {
            assert_ne!(
                role.rung(),
                other.rung(),
                "the roles {role:?} and {other:?} share one rung"
            );
        }
    }
    for role in RadiusRole::ALL {
        let owners = declarations_with(&radius, role.rung());
        assert_eq!(owners, 1, "the rung of {role:?} is declared {owners} times");
    }
}

#[test]
fn the_physical_whitelist_lists_every_physical_length_once() {
    for allowed in PHYSICAL_SIZE_WHITELIST {
        assert!(is_physical_value(allowed));
    }
    for size in PhysicalSize::ALL {
        assert!(PHYSICAL_SIZE_WHITELIST.contains(&size.px()));
    }
    // The focus ring shares the caret's length, so the whitelist is shorter
    // than the number of physical sizes.
    assert!(PHYSICAL_SIZE_WHITELIST.len() < PhysicalSize::ALL.len());
}

#[test]
fn the_channels_of_one_file_never_overlap() {
    // A physical size is not a rung of the spacing ladder, and a chrome
    // height is neither: the closed sets must not blur into each other.
    for step in SpaceStep::ALL {
        assert!(!is_physical_value(step.px()));
        assert!(!is_chrome_value(step.px()));
    }
    for size in PhysicalSize::ALL {
        assert!(!is_chrome_value(size.px()));
    }
}
