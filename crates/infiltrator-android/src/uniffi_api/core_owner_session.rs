//! Typed lifecycle boundary for the Android `:vpn` Core owner session.

use crate::ffi::{FfiErrorCode, FfiStatus};
use crate::host_session::{
    SharedCoreOwnerSessionBegin, begin_shared_core_owner_session,
    complete_shared_core_owner_session, recover_android_owner_before_binding,
};
use crate::product::{begin_owner_product_session, close_owner_product_session};
use crate::vpn_service::AndroidVpnServicePort;
use infiltrator_application::vpn_application::VpnServiceApplication;
use infiltrator_contract::android_control::AndroidHostProcessRole;
use mihomo_platform::android_bridge::get_android_bridge;
use std::sync::Arc;

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
    let mut session = match begin_shared_core_owner_session().await {
        Ok(session) => session,
        Err(status) => return status,
    };
    if session == SharedCoreOwnerSessionBegin::RetryClose {
        if let Err(status) = close_owner_product_session().await {
            return status;
        }
        session = match begin_shared_core_owner_session().await {
            Ok(session) => session,
            Err(status) => return status,
        };
    }
    match session {
        SharedCoreOwnerSessionBegin::AlreadyReady => {}
        SharedCoreOwnerSessionBegin::NewSession => {
            let vpn = VpnServiceApplication::new(Arc::new(AndroidVpnServicePort::shared()));
            if let Err(status) =
                recover_android_owner_before_binding(Arc::clone(&bridge), vpn).await
            {
                return status;
            }
        }
        SharedCoreOwnerSessionBegin::RecoveryInProgress => {
            return FfiStatus::err(
                FfiErrorCode::NotReady,
                "Android Core owner startup recovery is already in progress",
            );
        }
        SharedCoreOwnerSessionBegin::RetryClose => {
            return FfiStatus::err(
                FfiErrorCode::NotReady,
                "Android Core owner terminal close remains pending",
            );
        }
    }
    begin_owner_product_session().await;
    if session == SharedCoreOwnerSessionBegin::NewSession
        && let Err(status) = complete_shared_core_owner_session().await
    {
        return status;
    }
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
