//! Tests for the [`crate::proxy_nodes`] module, mounted via `#[cfg(test)]`
//! from the module root. Tests share the production file budget.

#[cfg(test)]
#[path = "proxy_nodes_test_cases.rs"]
mod tests;
