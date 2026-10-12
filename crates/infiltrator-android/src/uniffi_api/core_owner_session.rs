//! Typed lifecycle boundary for the Android `:vpn` Core owner session.

use crate::ffi::{FfiErrorCode, FfiStatus};
use crate::host_session::{SharedCoreOwnerSessionBegin, begin_shared_core_owner_session};
use crate::product::{begin_owner_product_session, close_owner_product_session};
use infiltrator_contract::android_control::AndroidHostProcessRole;
use mihomo_platform::android_bridge::get_android_bridge;

/// Begin or reattach the explicit Rust Core owner session for Android `:vpn`.
#[uniffi::export]
pub async fn android_begin_core_owner_session() -> FfiStatus {
    let Some(bridge) = get_android_bridge() else {
        return FfiStatus::err(FfiErrorCode::NotReady, "Android host bridge is unavailable");
    };
    if bridge.process_role() != AndroidHostProcessRole::CoreOwner {
        return FfiStatus::err(
            FfiErrorCode::NotReady,
            "only the Android :vpn process can begin the Core owner session",
        );
    }
    match begin_shared_core_owner_session().await {
        Ok(SharedCoreOwnerSessionBegin::Ready) => {}
        Ok(SharedCoreOwnerSessionBegin::RetryClose) => {
            if let Err(status) = close_owner_product_session().await {
                return status;
            }
            match begin_shared_core_owner_session().await {
                Ok(SharedCoreOwnerSessionBegin::Ready) => {}
                Ok(SharedCoreOwnerSessionBegin::RetryClose) => {
                    return FfiStatus::err(
                        FfiErrorCode::NotReady,
                        "Android Core owner terminal close remains pending",
                    );
                }
                Err(status) => return status,
            }
        }
        Err(status) => return status,
    }
    begin_owner_product_session().await;
    FfiStatus::ok()
}

/// Stop the VPN and Core in order, retaining the owner fence on any failed
/// terminal readback.
#[uniffi::export]
pub async fn android_close_core_owner() -> FfiStatus {
    let Some(bridge) = get_android_bridge() else {
        return FfiStatus::err(FfiErrorCode::NotReady, "Android host bridge is unavailable");
    };
    if bridge.process_role() != AndroidHostProcessRole::CoreOwner {
        return FfiStatus::err(
            FfiErrorCode::NotReady,
            "only the Android :vpn process can close the Core owner session",
        );
    }
    close_owner_product_session()
        .await
        .map(|()| FfiStatus::ok())
        .unwrap_or_else(|status| status)
}
