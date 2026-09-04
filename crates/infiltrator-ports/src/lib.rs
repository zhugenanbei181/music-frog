//! Outbound ports used by the 0.30 application layer.
//!
//! Traits here describe capabilities, not implementations. They deliberately
//! contain no Tokio channels, task handles, HTTP response types, or UI types.

pub mod capability_provider;
pub mod core_lifecycle;
pub mod core_process;
pub mod data_dir;
pub mod data_store;
pub mod endpoint;
pub mod error;
pub mod overview;
pub mod secure_store;

#[cfg(test)]
mod tests {
    use super::error::PortError;
    use infiltrator_contract::capability::Capability;
    use infiltrator_contract::error::{ErrorCode, Failure};

    #[test]
    fn adapter_error_maps_to_contract_failure_without_runtime_types() {
        let failure: Failure = PortError::unsupported(
            Capability::SystemProxy,
            "Android VpnService owns proxy routing",
        )
        .into();
        assert_eq!(failure.code, ErrorCode::Unsupported);
        assert!(!failure.retryable);
        assert!(failure.message.contains("SystemProxy"));
    }
}
