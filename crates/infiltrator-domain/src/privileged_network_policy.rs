//! Pure validation for privileged network regression requests.

use infiltrator_contract::privileged_network::{
    PrivilegedNetworkOperation, PrivilegedNetworkRequest,
};

pub fn validate_request(request: &PrivilegedNetworkRequest) -> Result<(), String> {
    if request.operations.is_empty() {
        return Err("privileged network regression requires at least one operation".to_owned());
    }
    for (index, operation) in request.operations.iter().enumerate() {
        if request.operations[..index].contains(operation) {
            return Err(format!(
                "privileged network regression contains duplicate operation: {operation:?}"
            ));
        }
    }
    if !request
        .operations
        .iter()
        .any(|operation| matches!(operation, PrivilegedNetworkOperation::TunService))
    {
        return Err("privileged network regression must include TUN service coverage".to_owned());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn standard_request_is_accepted() {
        assert!(validate_request(&PrivilegedNetworkRequest::standard()).is_ok());
    }

    #[test]
    fn empty_and_duplicate_requests_are_rejected_before_host_io() {
        assert!(validate_request(&PrivilegedNetworkRequest { operations: vec![] }).is_err());
        let mut request = PrivilegedNetworkRequest::standard();
        request
            .operations
            .push(PrivilegedNetworkOperation::TunService);
        assert!(validate_request(&request).is_err());
    }
}
