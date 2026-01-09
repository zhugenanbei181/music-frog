use anyhow::anyhow;
use log::warn;

#[cfg(target_os = "windows")]
use std::ffi::OsStr;
#[cfg(target_os = "windows")]
use std::os::windows::ffi::OsStrExt;
#[cfg(target_os = "windows")]
use windows_sys::Win32::UI::Shell::ShellExecuteW;
#[cfg(target_os = "windows")]
use windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOW;

pub(crate) fn open_in_browser(url: &str) -> anyhow::Result<()> {
    webbrowser::open(url).map_err(|err| anyhow!(err.to_string()))
}

pub(crate) fn confirm_dialog(message: &str, title: &str) -> bool {
    let result = rfd::MessageDialog::new()
        .set_title(title)
        .set_description(message)
        .set_buttons(rfd::MessageButtons::OkCancel)
        .set_level(rfd::MessageLevel::Warning)
        .show();
    matches!(
        result,
        rfd::MessageDialogResult::Ok | rfd::MessageDialogResult::Yes
    )
}

pub(crate) fn pick_editor_path() -> Option<String> {
    let dialog = rfd::FileDialog::new().set_title("选择编辑器");
    #[cfg(target_os = "windows")]
    let dialog = dialog.add_filter("可执行文件", &["exe", "cmd", "bat"]);
    dialog
        .pick_file()
        .map(|path| path.to_string_lossy().to_string())
}

pub(crate) fn show_error_dialog(message: impl Into<String>) {
    let body = message.into();
    let result = rfd::MessageDialog::new()
        .set_title("Mihomo Despicable Infiltrator")
        .set_description(&body)
        .set_buttons(rfd::MessageButtons::Ok)
        .set_level(rfd::MessageLevel::Error)
        .show();
    if !matches!(result, rfd::MessageDialogResult::Ok) {
        warn!("startup issue: {body}");
    }
}

#[cfg(target_os = "windows")]
pub(crate) fn is_running_as_admin() -> bool {
    is_elevated::is_elevated()
}

#[cfg(not(target_os = "windows"))]
pub(crate) fn is_running_as_admin() -> bool {
    false
}

#[cfg(target_os = "windows")]
pub(crate) fn restart_as_admin(
    static_port: Option<u16>,
    admin_port: Option<u16>,
) -> anyhow::Result<()> {
    let exe = std::env::current_exe()?;
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    args.retain(|arg| {
        !arg.starts_with("--static-port=") && !arg.starts_with("--admin-port=")
    });
    if let Some(port) = static_port {
        args.push(format!("--static-port={port}"));
    }
    if let Some(port) = admin_port {
        args.push(format!("--admin-port={port}"));
    }

    let args_str = args
        .iter()
        .map(|arg| format!("\"{}\"", arg.replace('"', "\\\""))) // Simple escaping
        .collect::<Vec<_>>()
        .join(" ");

    let file_path = to_u16(&exe.to_string_lossy());
    let params = to_u16(&args_str);
    let operation = to_u16("runas");

    let ret = unsafe {
        ShellExecuteW(
            std::ptr::null_mut(),
            operation.as_ptr(),
            file_path.as_ptr(),
            if args_str.is_empty() {
                std::ptr::null()
            } else {
                params.as_ptr()
            },
            std::ptr::null(),
            SW_SHOW,
        )
    };

    if (ret as usize) <= 32 {
        return Err(anyhow!("以管理员身份重启失败 (ShellExecuteW error: {:?})", ret));
    }

    Ok(())
}

#[cfg(not(target_os = "windows"))]
pub(crate) fn restart_as_admin(
    _static_port: Option<u16>,
    _admin_port: Option<u16>,
) -> anyhow::Result<()> {
    Err(anyhow!("以管理员身份重启仅支持 Windows"))
}

#[cfg(target_os = "windows")]
fn to_u16(s: &str) -> Vec<u16> {
    OsStr::new(s).encode_wide().chain(Some(0)).collect()
}
