//! Application services for MusicFrog Infiltrator.
//!
//! The application layer is allowed to use Tokio internally. Its public
//! surface is deliberately expressed in contract values and standard Rust
//! methods, so a UI or FFI consumer never needs to know about Tokio channels,
//! task handles, or concrete Mihomo clients.

pub mod core_application;
pub mod overview;

/// Validate a logical routing rule without exposing the domain error type to
/// a surface. The application owns the public error boundary; the pure AST
/// parser remains in `infiltrator-domain`.
pub fn validate_logical_rule_syntax(rule: &str) -> Result<(), String> {
    infiltrator_domain::sub_rules::validate_logical_rule_syntax(rule)
        .map_err(|error| error.to_string())
}
