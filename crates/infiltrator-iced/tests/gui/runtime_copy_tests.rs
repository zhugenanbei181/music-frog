//! Real TEA terminal paths resolve shared runtime copy and preserve typed cancellation.
//! test-intent: behavior
use crate::state::AppState;
use crate::types::app::ToastStatus;
use crate::types::message::Message;
use futures_util::StreamExt;
use iced::Task;
use iced_runtime::{Action, task::into_stream};
use infiltrator_contract::error::{ErrorCode, Failure, FailureReason, InfiltratorError};
use infiltrator_contract::version::CoreArtifactVerification;
use infiltrator_contract::vpn::{VpnSessionSnapshot, VpnSessionState};
use infiltrator_shared::locales::{Lang, Localizer};
use tokio::runtime::Builder;

fn notice(task: Task<Message>) -> (String, ToastStatus) {
    Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            let mut stream = into_stream(task).expect("native notification task");
            let Some(Action::Output(Message::ShowToast(copy, status))) = stream.next().await else {
                panic!("actual localized notification required")
            };
            assert!(stream.next().await.is_none());
            (copy, status)
        })
}

#[test]
fn runtime_and_vpn_terminal_messages_use_the_current_shared_locale_and_keep_opaque_values() {
    for locale in ["zh-CN", "en-US"] {
        let (mut state, _) = AppState::new();
        state.shell.lang = locale.into();
        let (copy, status) = notice(state.update(Message::SetIpv6Routing(true)));
        assert_eq!(status, ToastStatus::Error);
        assert!(copy.contains(Lang(locale).tr("runtime_action_ipv6").as_ref()));
        assert!(state.runtime.runtime.is_none());
        assert!(state.runtime.pending_runtime_patch.is_none());
        let (copy, status) = notice(state.update(Message::TestWebDavConnection));
        assert_eq!(copy, Lang(locale).tr("webdav_credentials_required"));
        assert_eq!(status, ToastStatus::Error);
        assert!(!state.profile.is_testing_webdav);
        let snapshot = VpnSessionSnapshot {
            state: VpnSessionState::Unsupported {
                reason: "opaque {reason} 用户".into(),
            },
            ..Default::default()
        };
        let (copy, status) = notice(state.update(Message::VpnSessionUpdated(Ok(snapshot))));
        assert!(copy.contains("opaque {reason} 用户"));
        assert_eq!(status, ToastStatus::Info);
        assert!(copy.starts_with(if locale == "en-US" {
            "Android VPN is unsupported"
        } else {
            "当前宿主不支持 Android VPN"
        }));
        let (copy, status) = notice(state.update(Message::OpenConfigDirFinished(Err(
            InfiltratorError::Io("opaque {reason}".into()),
        ))));
        assert!(copy.contains("opaque {reason}"));
        assert_eq!(status, ToastStatus::Error);
        assert!(copy.starts_with(if locale == "en-US" {
            "Could not open"
        } else {
            "无法打开"
        }));
    }
}

#[test]
fn current_download_cancellation_uses_the_typed_result_and_stale_replies_cannot_stop_a_new_download()
 {
    let failure = Failure::new(ErrorCode::Canceled, "opaque transport detail", false)
        .with_reason(FailureReason::DownloadCanceled);
    for locale in ["zh-CN", "en-US"] {
        let (mut state, _) = AppState::new();
        state.shell.lang = locale.into();
        state.runtime.core_download_token = 7;
        state.runtime.is_downloading_core = true;
        let task = state.update(Message::CoreDownloadFinished(Err(failure.clone()), 6));
        assert_eq!(task.units(), 0);
        assert!(state.runtime.is_downloading_core);
        let (copy, severity) =
            notice(state.update(Message::CoreDownloadFinished(Err(failure.clone()), 7)));
        assert_eq!(copy, Lang(locale).tr("download_canceled"));
        assert_eq!(severity, ToastStatus::Warning);
        assert!(!state.runtime.is_downloading_core);
        assert!(
            state.shell.error_msg.is_none(),
            "cancellation is not a product failure"
        );
        assert_eq!(
            state.runtime.core_integrity,
            CoreArtifactVerification::Rejected {
                version: "unknown".into(),
                failure: failure.clone(),
            }
        );
    }
}
