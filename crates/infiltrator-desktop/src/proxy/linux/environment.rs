/// Linux 桌面环境分类。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DesktopEnvironment {
    Gnome,
    Kde,
    Xfce,
    Generic,
}

impl DesktopEnvironment {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Gnome => "GNOME",
            Self::Kde => "KDE",
            Self::Xfce => "XFCE",
            Self::Generic => "Generic",
        }
    }
}

impl std::fmt::Display for DesktopEnvironment {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// 探测当前 Linux 桌面环境。
pub fn detect_desktop_environment() -> DesktopEnvironment {
    detect_desktop_environment_with(|k| std::env::var(k).ok())
}

/// 基于自定义环境变量获取函数的桌面环境探测器（便于单元测试与沙箱探测）。
pub fn detect_desktop_environment_with<F>(get_env: F) -> DesktopEnvironment
where
    F: Fn(&str) -> Option<String>,
{
    // 1. 检查 XDG_CURRENT_DESKTOP（支持冒号分隔多值，如 "ubuntu:GNOME", "KDE:Plasma", "XFCE"）
    if let Some(current) = get_env("XDG_CURRENT_DESKTOP") {
        for part in current.split(':') {
            let part_lower = part.trim().to_ascii_lowercase();
            if part_lower.contains("xfce") || part_lower == "x-generic" {
                return DesktopEnvironment::Xfce;
            }
            if part_lower.contains("kde") || part_lower.contains("plasma") {
                return DesktopEnvironment::Kde;
            }
            if part_lower.contains("gnome")
                || part_lower.contains("unity")
                || part_lower.contains("pantheon")
                || part_lower.contains("budgie")
                || part_lower.contains("cinnamon")
                || part_lower.contains("mate")
            {
                return DesktopEnvironment::Gnome;
            }
        }
    }

    // 2. 检查 XDG_SESSION_DESKTOP
    if let Some(session) = get_env("XDG_SESSION_DESKTOP") {
        let session_lower = session.trim().to_ascii_lowercase();
        if session_lower.contains("xfce") || session_lower.contains("xubuntu") {
            return DesktopEnvironment::Xfce;
        }
        if session_lower.contains("kde")
            || session_lower.contains("plasma")
            || session_lower.contains("plasmashell")
            || session_lower.contains("kubuntu")
        {
            return DesktopEnvironment::Kde;
        }
        if session_lower.contains("gnome")
            || session_lower == "ubuntu"
            || session_lower == "pop"
            || session_lower == "pop-os"
        {
            return DesktopEnvironment::Gnome;
        }
    }

    // 3. 检查 DESKTOP_SESSION
    if let Some(session) = get_env("DESKTOP_SESSION") {
        let session_lower = session.trim().to_ascii_lowercase();
        if session_lower.contains("xfce") || session_lower.contains("xubuntu") {
            return DesktopEnvironment::Xfce;
        }
        if session_lower.contains("kde")
            || session_lower.contains("plasma")
            || session_lower.contains("plasma-wayland")
            || session_lower.contains("plasma-x11")
            || session_lower.contains("kubuntu")
        {
            return DesktopEnvironment::Kde;
        }
        if session_lower.contains("gnome")
            || session_lower == "ubuntu"
            || session_lower == "pop"
            || session_lower == "pop-os"
            || session_lower.contains("cinnamon")
            || session_lower.contains("mate")
        {
            return DesktopEnvironment::Gnome;
        }
    }

    // 4. 检查 KDE / GNOME 专属环境变量
    if let Some(kde_full) = get_env("KDE_FULL_SESSION") {
        if kde_full.trim().eq_ignore_ascii_case("true") {
            return DesktopEnvironment::Kde;
        }
    }
    if get_env("KDE_SESSION_VERSION").is_some() {
        return DesktopEnvironment::Kde;
    }
    if let Some(gnome_id) = get_env("GNOME_DESKTOP_SESSION_ID") {
        if !gnome_id.trim().is_empty() {
            return DesktopEnvironment::Gnome;
        }
    }

    DesktopEnvironment::Generic
}
