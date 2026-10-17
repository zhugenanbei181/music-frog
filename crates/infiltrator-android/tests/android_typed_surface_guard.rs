#![allow(
    clippy::collapsible_if,
    clippy::needless_lifetimes,
    clippy::map_identity
)]
//! Structural regression guard for the Android owner and VPN FFI contract.
//!
//! Rust declarations are inspected as syntax trees, then compared with the
//! checked-in UniFFI Kotlin output. UDL remains guarded separately because it
//! is still a published input to the Android binding pipeline.

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;
use syn::visit::visit_expr_match;

use syn::visit::Visit;
use syn::{
    Attribute, Expr, ExprMatch, FnArg, GenericArgument, ImplItem, Item, ItemEnum, ItemFn, Pat,
    PathArguments, ReturnType, Type,
};

const CRATE_ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../..");
const BINDINGS: &str = "android/app/src/main/java/infiltrator_android/infiltrator_android.kt";
const CONTROL: &str = "crates/infiltrator-android/src/uniffi_api/control.rs";
const OWNER: &str = "crates/infiltrator-android/src/uniffi_api/owner_operation.rs";
const OWNER_WIRE: &str = "crates/infiltrator-android/src/uniffi_api/owner_operation/wire.rs";
const UDL: &str = "crates/infiltrator-android/src/infiltrator_android.udl";
const UI_ROOT: &str = "android/app/src/main/java/com/musicfrog/infiltrator/ui";

fn repository_file(relative: &str) -> String {
    fs::read_to_string(Path::new(CRATE_ROOT).join(relative))
        .unwrap_or_else(|error| panic!("could not read {relative}: {error}"))
}

fn parse_rust(source: &str) -> syn::File {
    syn::parse_file(source).unwrap_or_else(|error| panic!("invalid Rust fixture: {error}"))
}

fn enum_item<'a>(file: &'a syn::File, name: &str) -> Option<&'a ItemEnum> {
    file.items.iter().find_map(|item| match item {
        Item::Enum(item) if item.ident == name => Some(item),
        _ => None,
    })
}

fn enum_variants(item: &ItemEnum) -> BTreeSet<String> {
    item.variants
        .iter()
        .map(|variant| variant.ident.to_string())
        .collect()
}

fn snake_case(name: &str) -> String {
    let mut result = String::new();
    let chars = name.chars().collect::<Vec<_>>();
    for (index, current) in chars.iter().copied().enumerate() {
        if current.is_ascii_uppercase()
            && index > 0
            && (chars[index - 1].is_ascii_lowercase()
                || chars[index - 1].is_ascii_digit()
                || (index + 1 < chars.len() && chars[index + 1].is_ascii_lowercase()))
        {
            result.push('_');
        }
        result.push(current.to_ascii_uppercase());
    }
    result
}

fn kotlin_body<'a>(source: &'a str, declaration: &str, name: &str) -> Option<&'a str> {
    let marker = format!("{declaration} {name}");
    let start = source.find(&marker)?;
    let after_name = start + marker.len();
    let open = source[after_name..].find('{')? + after_name;
    let close = matching_kotlin_brace(source, open)?;
    Some(&source[open + 1..close])
}

fn matching_kotlin_brace(source: &str, open: usize) -> Option<usize> {
    #[derive(Clone, Copy)]
    enum LexicalState {
        Code,
        LineComment,
        BlockComment(usize),
        String,
        Char,
    }

    let bytes = source.as_bytes();
    let mut state = LexicalState::Code;
    let mut depth = 0usize;
    let mut index = open;
    while index < bytes.len() {
        let current = bytes[index];
        let next = bytes.get(index + 1).copied();
        match state {
            LexicalState::Code => match (current, next) {
                (b'/', Some(b'/')) => {
                    state = LexicalState::LineComment;
                    index += 1;
                }
                (b'/', Some(b'*')) => {
                    state = LexicalState::BlockComment(1);
                    index += 1;
                }
                (b'"', _) => state = LexicalState::String,
                (b'\'', _) => state = LexicalState::Char,
                (b'{', _) => depth += 1,
                (b'}', _) => {
                    depth = depth.checked_sub(1)?;
                    if depth == 0 {
                        return Some(index);
                    }
                }
                _ => {}
            },
            LexicalState::LineComment => {
                if current == b'\n' {
                    state = LexicalState::Code;
                }
            }
            LexicalState::BlockComment(block_depth) => match (current, next) {
                (b'/', Some(b'*')) => {
                    state = LexicalState::BlockComment(block_depth + 1);
                    index += 1;
                }
                (b'*', Some(b'/')) if block_depth == 1 => {
                    state = LexicalState::Code;
                    index += 1;
                }
                (b'*', Some(b'/')) => {
                    state = LexicalState::BlockComment(block_depth - 1);
                    index += 1;
                }
                _ => {}
            },
            LexicalState::String | LexicalState::Char => {
                if current == b'\\' {
                    index += 1;
                } else if (matches!(state, LexicalState::String) && current == b'"')
                    || (matches!(state, LexicalState::Char) && current == b'\'')
                {
                    state = LexicalState::Code;
                }
            }
        }
        index += 1;
    }
    None
}

fn kotlin_enum_variants(source: &str, name: &str) -> Option<BTreeSet<String>> {
    let body = kotlin_body(source, "enum class", name)?;
    let mut variants = BTreeSet::new();
    for line in body.lines() {
        let line = line.trim();
        let candidate = line.trim_end_matches([',', ';']).trim();
        if !candidate.is_empty()
            && candidate
                .chars()
                .all(|character| character.is_ascii_alphanumeric() || character == '_')
            && candidate
                .chars()
                .next()
                .is_some_and(|character| character.is_ascii_uppercase())
        {
            variants.insert(candidate.to_owned());
        }
    }
    Some(variants)
}

fn kotlin_sealed_variants(source: &str, name: &str) -> Option<BTreeSet<String>> {
    let body = kotlin_body(source, "sealed class", name)?;
    let mut variants = BTreeSet::new();
    for line in body.lines() {
        let line = line.trim();
        let rest = line
            .strip_prefix("object ")
            .or_else(|| line.strip_prefix("data class "));
        let Some(rest) = rest else { continue };
        let candidate = rest
            .split(|character: char| !character.is_ascii_alphanumeric() && character != '_')
            .next()
            .unwrap_or_default();
        if !candidate.is_empty() {
            variants.insert(candidate.to_owned());
        }
    }
    Some(variants)
}

fn lifecycle_name(name: &str) -> bool {
    let words = name
        .split(|character: char| !character.is_ascii_alphanumeric())
        .map(str::to_ascii_lowercase)
        .collect::<BTreeSet<_>>();
    let owner_axis = ["vpn", "proxy", "core", "kernel", "service", "tunnel"]
        .iter()
        .any(|word| words.contains(*word));
    let lifecycle_action = [
        "start",
        "stop",
        "pause",
        "resume",
        "restart",
        "shutdown",
        "revoke",
        "running",
        "stopped",
        "started",
        "active",
        "enabled",
        "ready",
        "permission",
        "authorization",
        "authorized",
        "lifecycle",
        "foreground",
        "established",
        "prepare",
        "preparation",
    ]
    .iter()
    .any(|word| words.contains(*word));
    owner_axis && lifecycle_action
}

fn type_contains_ident(ty: &Type, expected: &str) -> bool {
    match ty {
        Type::Path(path) => path.path.segments.iter().any(|segment| {
            segment.ident == expected
                || match &segment.arguments {
                    PathArguments::AngleBracketed(arguments) => {
                        arguments.args.iter().any(|argument| match argument {
                            GenericArgument::Type(inner) => type_contains_ident(inner, expected),
                            _ => false,
                        })
                    }
                    _ => false,
                }
        }),
        Type::Reference(reference) => type_contains_ident(&reference.elem, expected),
        Type::Tuple(tuple) => tuple
            .elems
            .iter()
            .any(|inner| type_contains_ident(inner, expected)),
        Type::Paren(paren) => type_contains_ident(&paren.elem, expected),
        Type::Group(group) => type_contains_ident(&group.elem, expected),
        Type::Ptr(pointer) => type_contains_ident(&pointer.elem, expected),
        _ => false,
    }
}

fn is_uniffi_export(attributes: &[Attribute]) -> bool {
    attributes.iter().any(|attribute| {
        let segments = attribute
            .path()
            .segments
            .iter()
            .map(|segment| segment.ident.to_string())
            .collect::<Vec<_>>();
        segments == ["uniffi", "export"]
    })
}

fn exported_contract_problems(source: &str) -> Vec<String> {
    let file = parse_rust(source);
    file.items
        .iter()
        .filter_map(|item| match item {
            Item::Fn(function) if is_uniffi_export(&function.attrs) => Some(function),
            _ => None,
        })
        .flat_map(exported_function_problems)
        .collect()
}

fn exported_function_problems(function: &ItemFn) -> Vec<String> {
    let mut problems = Vec::new();
    let name = function.sig.ident.to_string();
    let lifecycle = lifecycle_name(&name);
    if lifecycle {
        if matches!(&function.sig.output, ReturnType::Type(_, ty)
            if type_contains_ident(ty, "bool") || type_contains_ident(ty, "String") || type_contains_ident(ty, "str"))
        {
            problems.push(format!(
                "{name}: lifecycle FFI result must use a typed outcome"
            ));
        }
        if function.sig.inputs.iter().any(|argument| match argument {
            FnArg::Typed(argument) => type_contains_ident(&argument.ty, "bool"),
            FnArg::Receiver(_) => false,
        }) {
            problems.push(format!(
                "{name}: lifecycle FFI input must not encode state as a boolean"
            ));
        }
    }
    for argument in &function.sig.inputs {
        let FnArg::Typed(argument) = argument else {
            continue;
        };
        let Pat::Ident(pattern) = argument.pat.as_ref() else {
            continue;
        };
        let parameter = pattern.ident.to_string();
        if ["action", "command", "intent", "operation"].contains(&parameter.as_str())
            && (type_contains_ident(&argument.ty, "String")
                || type_contains_ident(&argument.ty, "str"))
        {
            problems.push(format!(
                "{name}({parameter}): action/operation FFI input must use a typed enum"
            ));
        }
    }
    problems
}

fn udl_contract_problems(source: &str) -> Vec<String> {
    let mut problems = Vec::new();
    for statement in source.split(';') {
        let statement = statement
            .lines()
            .map(|line| line.split("//").next().unwrap_or(""))
            .collect::<Vec<_>>()
            .join(" ");
        let statement = statement.trim();
        let Some((head, arguments)) = statement.split_once('(') else {
            continue;
        };
        let mut fields = head.split_whitespace().rev();
        let Some(name) = fields.next() else { continue };
        let Some(return_type) = fields.next() else {
            continue;
        };
        if lifecycle_name(name) && matches!(return_type, "boolean" | "string") {
            problems.push(format!(
                "{name}: UDL lifecycle result must use a typed outcome"
            ));
        }
        let arguments = arguments.split(')').next().unwrap_or_default();
        for argument in arguments.split(',') {
            let fields = argument.split_whitespace().collect::<Vec<_>>();
            if lifecycle_name(name)
                && fields
                    .first()
                    .is_some_and(|field| field.trim_end_matches('?') == "boolean")
            {
                problems.push(format!(
                    "{name}: UDL lifecycle input must not encode state as a boolean"
                ));
            }
            if fields.len() >= 2
                && ["action", "command", "intent", "operation"].contains(&fields[1])
                && fields[0].trim_end_matches('?') == "string"
            {
                problems.push(format!(
                    "{name}({}): UDL action/operation input must use a typed enum",
                    fields[1]
                ));
            }
        }
    }
    problems
}

fn kotlin_function_signature<'a>(source: &'a str, kotlin_name: &str) -> Option<&'a str> {
    let marker = format!("fun `{kotlin_name}`");
    let start = source.find(&marker)?;
    let tail = &source[start..];
    let end = tail.find('{')?;
    Some(&tail[..end])
}

fn ui_fallback_problems(sources: &[(&str, &str)]) -> Vec<String> {
    let contracts = [
        "AndroidHostProtocolVersion",
        "AndroidHostRequest",
        "AndroidHostReply",
        "AndroidOwnerOperation",
    ];
    let mut problems = Vec::new();
    for (path, source) in sources {
        for line in source.lines() {
            let line = line.trim();
            for contract in contracts {
                if [
                    "enum class ",
                    "sealed class ",
                    "sealed interface ",
                    "data class ",
                    "typealias ",
                ]
                .iter()
                .any(|prefix| has_declared_name(line, prefix, contract))
                {
                    problems.push(format!(
                        "{path}: UI must not define local {contract} fallback"
                    ));
                }
            }
        }
    }
    problems
}

fn has_declared_name(line: &str, prefix: &str, name: &str) -> bool {
    let marker = format!("{prefix}{name}");
    line.match_indices(&marker).any(|(start, _)| {
        let after = start + marker.len();
        line[after..]
            .chars()
            .next()
            .is_none_or(|character| !character.is_ascii_alphanumeric() && character != '_')
    })
}

fn kotlin_ui_sources(root: &Path) -> Vec<(String, String)> {
    let mut files = Vec::new();
    let ui_root = root.join(UI_ROOT);
    let mut pending = vec![ui_root];
    while let Some(directory) = pending.pop() {
        let Ok(entries) = fs::read_dir(&directory) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                pending.push(path);
            } else if path.extension().is_some_and(|extension| extension == "kt") {
                if let Ok(source) = fs::read_to_string(&path) {
                    files.push((path.display().to_string(), source));
                }
            }
        }
    }
    files
}

fn exported_functions<'a>(file: &'a syn::File) -> Vec<&'a ItemFn> {
    file.items
        .iter()
        .filter_map(|item| match item {
            Item::Fn(function) if is_uniffi_export(&function.attrs) => Some(function),
            _ => None,
        })
        .collect()
}

fn collect_exported_functions(root: &Path) -> Vec<(String, ItemFn)> {
    let api_root = root.join("crates/infiltrator-android/src/uniffi_api");
    let mut pending = vec![api_root];
    let mut functions = Vec::new();
    while let Some(directory) = pending.pop() {
        let Ok(entries) = fs::read_dir(&directory) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                if path.file_name().is_some_and(|name| name == "tests") {
                    continue;
                }
                pending.push(path);
            } else if path.extension().is_some_and(|extension| extension == "rs")
                && !path
                    .file_name()
                    .is_some_and(|name| name.to_string_lossy().contains("test"))
            {
                let source = fs::read_to_string(&path)
                    .unwrap_or_else(|error| panic!("could not read {}: {error}", path.display()));
                let file = parse_rust(&source);
                let relative = path
                    .strip_prefix(root)
                    .unwrap_or(&path)
                    .display()
                    .to_string();
                for function in exported_functions(&file) {
                    functions.push((relative.clone(), function.clone()));
                }
            }
        }
    }
    functions
}

struct MatchFinder<'a> {
    target: &'static str,
    matches: Vec<&'a ExprMatch>,
}

impl<'ast> Visit<'ast> for MatchFinder<'ast> {
    fn visit_expr_match(&mut self, node: &'ast ExprMatch) {
        if let Expr::Path(expression) = node.expr.as_ref()
            && expression
                .path
                .segments
                .last()
                .is_some_and(|segment| segment.ident == self.target)
        {
            self.matches.push(node);
        }
        visit_expr_match(self, node);
    }
}

#[derive(Default)]
struct PatternVariants {
    variants: Vec<String>,
    has_wildcard: bool,
}

fn collect_pattern_variants(pattern: &Pat, owner_name: &str, output: &mut PatternVariants) {
    match pattern {
        Pat::Path(path) => push_variant(&path.path, owner_name, output),
        Pat::Struct(pattern) => push_variant(&pattern.path, owner_name, output),
        Pat::TupleStruct(pattern) => push_variant(&pattern.path, owner_name, output),
        Pat::Or(pattern) => {
            for alternative in &pattern.cases {
                collect_pattern_variants(alternative, owner_name, output);
            }
        }
        Pat::Wild(_) => output.has_wildcard = true,
        Pat::Paren(pattern) => collect_pattern_variants(&pattern.pat, owner_name, output),
        Pat::Reference(pattern) => collect_pattern_variants(&pattern.pat, owner_name, output),
        _ => {}
    }
}

fn push_variant(path: &syn::Path, owner_name: &str, output: &mut PatternVariants) {
    let segments = path.segments.iter().collect::<Vec<_>>();
    if segments.len() >= 2
        && (segments[segments.len() - 2].ident == owner_name
            || segments[segments.len() - 2].ident == "Self")
    {
        output
            .variants
            .push(segments[segments.len() - 1].ident.to_string());
    }
}

fn match_coverage(
    block: &syn::Block,
    target: &'static str,
    owner_name: &str,
) -> Result<PatternVariants, String> {
    let mut finder = MatchFinder {
        target,
        matches: Vec::new(),
    };
    finder.visit_block(block);
    if finder.matches.len() != 1 {
        return Err(format!(
            "expected one `match {target}` expression, found {}",
            finder.matches.len()
        ));
    }
    let mut coverage = PatternVariants::default();
    for arm in &finder.matches[0].arms {
        collect_pattern_variants(&arm.pat, owner_name, &mut coverage);
    }
    Ok(coverage)
}

fn owner_method<'a>(file: &'a syn::File, method_name: &str) -> Option<&'a syn::ImplItemFn> {
    file.items.iter().find_map(|item| {
        let Item::Impl(implementation) = item else {
            return None;
        };
        let Type::Path(path) = implementation.self_ty.as_ref() else {
            return None;
        };
        if path
            .path
            .segments
            .last()
            .is_none_or(|segment| segment.ident != "AndroidOwnerOperation")
        {
            return None;
        }
        implementation.items.iter().find_map(|item| match item {
            ImplItem::Fn(function) if function.sig.ident == method_name => Some(function),
            _ => None,
        })
    })
}

fn operation_coverage_problems(operation_source: &str, wire_source: &str) -> Vec<String> {
    let operation_file = parse_rust(operation_source);
    let enum_item = enum_item(&operation_file, "AndroidOwnerOperation")
        .expect("AndroidOwnerOperation declaration must remain a Rust enum");
    let expected = enum_variants(enum_item);
    let wire_file = parse_rust(wire_source);
    let mut problems = Vec::new();

    for (method_name, target, owner_name, source) in [
        (
            "data_domain",
            "self",
            "Self",
            "AndroidOwnerOperation::data_domain",
        ),
        (
            "data_mutation",
            "self",
            "Self",
            "AndroidOwnerOperation::data_mutation",
        ),
    ] {
        let Some(method) = owner_method(&operation_file, method_name) else {
            problems.push(format!(
                "{source}: required typed registration method is missing"
            ));
            continue;
        };
        if let Err(error) = check_match_coverage(&method.block, target, owner_name, &expected) {
            problems.push(format!("{source}: {error}"));
        }
    }

    let execute_operation = wire_file.items.iter().find_map(|item| match item {
        Item::Fn(function) if function.sig.ident == "execute_operation" => Some(function),
        _ => None,
    });
    match execute_operation {
        Some(function) => {
            if let Err(error) = check_match_coverage(
                &function.block,
                "operation",
                "AndroidOwnerOperation",
                &expected,
            ) {
                problems.push(format!("execute_operation: {error}"));
            }
        }
        None => problems.push("execute_operation: typed owner dispatcher is missing".to_owned()),
    }
    problems
}

fn check_match_coverage(
    block: &syn::Block,
    target: &'static str,
    owner_name: &str,
    expected: &BTreeSet<String>,
) -> Result<(), String> {
    let coverage = match_coverage(block, target, owner_name)?;
    if coverage.has_wildcard {
        return Err(
            "wildcard fallback hides unregistered AndroidOwnerOperation variants".to_owned(),
        );
    }
    let actual = coverage.variants.iter().cloned().collect::<BTreeSet<_>>();
    if actual.len() != coverage.variants.len() {
        return Err("a typed operation variant is registered more than once".to_owned());
    }
    if &actual != expected {
        let missing = expected.difference(&actual).cloned().collect::<Vec<_>>();
        let extra = actual.difference(expected).cloned().collect::<Vec<_>>();
        return Err(format!(
            "variant registration mismatch; missing={missing:?}, extra={extra:?}"
        ));
    }
    Ok(())
}

fn kotlin_ui_declarations(root: &Path) -> Vec<(String, String)> {
    kotlin_ui_sources(root)
        .into_iter()
        .map(|(path, source)| (path, source))
        .collect()
}

#[path = "android_typed_surface_guard/cases.rs"]
mod cases;
