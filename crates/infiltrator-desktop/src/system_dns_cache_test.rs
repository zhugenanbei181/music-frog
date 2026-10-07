use super::*;
#[cfg(unix)]
use std::process::ExitStatus;

#[test]
fn linux_flush_prefers_resolvectl() {
    let invocations = flush_invocations("linux");
    assert_eq!(invocations[0], ("resolvectl", &["flush-caches"][..]));
    assert_eq!(invocations[1], ("systemd-resolve", &["--flush-caches"][..]));
}

#[test]
fn macos_flush_covers_dscacheutil_and_mdnsresponder() {
    let invocations = flush_invocations("macos");
    assert_eq!(invocations[0], ("dscacheutil", &["-flushcache"][..]));
    assert_eq!(invocations[1], ("killall", &["-HUP", "mDNSResponder"][..]));
}

#[test]
fn windows_flush_uses_ipconfig() {
    assert_eq!(
        flush_invocations("windows"),
        vec![("ipconfig", &["/flushdns"][..])]
    );
}

#[test]
fn an_unknown_platform_reports_typed_unsupported() {
    assert!(flush_invocations("redox").is_empty());
    assert!(flush_invocations("").is_empty());
}

#[cfg(unix)]
fn output(code: i32, message: &str) -> Output {
    use std::os::unix::process::ExitStatusExt;
    Output {
        status: ExitStatus::from_raw(code << 8),
        stdout: Vec::new(),
        stderr: message.as_bytes().to_vec(),
    }
}
#[cfg(unix)]
#[test]
fn macos_requires_both_cache_layers_and_preserves_the_second_permission_failure() {
    let mut calls = Vec::new();
    assert!(
        flush_with("macos", |program, _| {
            calls.push(program.to_owned());
            Ok(output(0, ""))
        })
        .unwrap()
    );
    assert_eq!(calls, ["dscacheutil", "killall"]);
    let mut calls = Vec::new();
    let failure = flush_with("macos", |program, _| {
        calls.push(program.to_owned());
        Ok(output(
            if program == "killall" { 1 } else { 0 },
            "Permission denied",
        ))
    })
    .unwrap_err();
    assert_eq!(calls, ["dscacheutil", "killall"]);
    assert_eq!(Failure::from(failure).code, ErrorCode::Permission);
}
#[cfg(unix)]
#[test]
fn linux_alternatives_cannot_overwrite_permission_or_nonzero_failure_with_a_missing_tool() {
    let mut calls = Vec::new();
    let failure = flush_with("linux", |program, _| {
        calls.push(program.to_owned());
        Ok(output(1, "Access denied"))
    })
    .unwrap_err();
    assert_eq!(calls, ["resolvectl"]);
    assert_eq!(Failure::from(failure).code, ErrorCode::Permission);
    let failure = flush_with("linux", |program, _| {
        if program == "resolvectl" {
            Ok(output(1, "resolver daemon refused the request"))
        } else {
            Err(io::Error::from(ErrorKind::NotFound))
        }
    })
    .unwrap_err();
    let failure = Failure::from(failure);
    assert!(
        failure
            .message
            .contains("resolver daemon refused the request")
    );
    assert!(failure.retryable);
    assert!(!flush_with("linux", |_, _| Err(io::Error::from(ErrorKind::NotFound))).unwrap());
    assert_eq!(
        Failure::from(
            flush_with("linux", |_, _| Err(io::Error::from(
                ErrorKind::PermissionDenied
            )))
            .unwrap_err()
        )
        .code,
        ErrorCode::Permission
    );
}
#[cfg(unix)]
#[test]
fn linux_uses_a_working_alternative_and_partial_macos_execution_is_never_success() {
    let mut calls = Vec::new();
    assert!(
        flush_with("linux", |program, _| {
            calls.push(program.to_owned());
            if program == "resolvectl" {
                Err(io::Error::from(ErrorKind::NotFound))
            } else {
                Ok(output(0, ""))
            }
        })
        .unwrap()
    );
    assert_eq!(calls, ["resolvectl", "systemd-resolve"]);
    let failure = flush_with("macos", |program, _| {
        if program == "dscacheutil" {
            Ok(output(0, ""))
        } else {
            Err(io::Error::from(ErrorKind::NotFound))
        }
    })
    .unwrap_err();
    assert_eq!(Failure::from(failure).code, ErrorCode::NotReady);
}
