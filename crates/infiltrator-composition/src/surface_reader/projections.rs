//! Small shared projections and failure helpers for the surface reader.
//!
//! These are pure functions over already-read facts, split out of the
//! reader module so the page assemblers and the port adapters can share
//! them without the parent file owning every helper.

use super::*;
use infiltrator_application::runtime_configuration_application::RuntimeConfigurationReadout;
use infiltrator_contract::mtu::MtuNegotiationSnapshot;
use infiltrator_ports::runtime_configuration::ObservedValue;
use lurker_core_contract::page::PageData;
#[cfg(test)]
use lurker_core_contract::page::PageStatus;

pub(super) fn missing(what: &str) -> Failure {
    Failure::new(
        ErrorCode::Unsupported,
        format!("{what} is not composed for this host"),
        false,
    )
}

pub(super) fn merge_applied_mtu(
    mut snapshot: MtuNegotiationSnapshot,
    runtime_config: Option<&Result<RuntimeConfigurationReadout, Failure>>,
) -> MtuNegotiationSnapshot {
    if let Some(Ok(config)) = runtime_config {
        if let ObservedValue::Reported(mtu) = &config.tun.value.mtu {
            snapshot.applied_tun_mtu = Some(*mtu);
        }
    }
    snapshot
}

pub(super) fn page_from_result<T, U, E>(
    result: Option<Result<T, E>>,
    map: impl FnOnce(T) -> U,
    what: &str,
) -> PageData<U>
where
    E: Into<Failure>,
{
    match result {
        Some(Ok(value)) => PageData::ready(map(value)),
        Some(Err(error)) => {
            let mut failure = error.into();
            failure.message = format!("{what}: {}", failure.message);
            PageData::failed(failure)
        }
        None => PageData::unavailable(missing(what)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn failed_pages_preserve_the_real_category_and_retry_policy_instead_of_relabeling_every_error_network()
     {
        for failure in [
            Failure::new(ErrorCode::InvalidState, "poisoned state", false),
            Failure::new(ErrorCode::Permission, "read denied", false),
            Failure::new(ErrorCode::Network, "connection lost", true),
        ] {
            let page: PageData<u32> = page_from_result(
                Some(Err::<u32, _>(failure.clone())),
                |value| value,
                "proxy read",
            );
            let PageStatus::Failed { failure: actual } = page.status else {
                panic!("read must remain failed")
            };
            assert_eq!(actual.code, failure.code);
            assert_eq!(actual.retryable, failure.retryable);
            assert_eq!(actual.message, format!("proxy read: {}", failure.message));
            assert_eq!(page.data, None);
        }
        let page: PageData<u32> = page_from_result(
            Some(Err::<u32, _>(PortError::PermissionDenied(
                "grant permission".into(),
            ))),
            |value| value,
            "proxy read",
        );
        let PageStatus::Failed { failure } = page.status else {
            panic!("permission read must remain failed")
        };
        assert_eq!(failure.code, ErrorCode::Permission);
        assert!(!failure.retryable);
    }
}
