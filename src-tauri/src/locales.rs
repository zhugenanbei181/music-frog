use std::borrow::Cow;

pub trait Localizer {
    fn tr(&self, key: &str) -> Cow<'static, str>;
}

pub struct Lang<'a>(pub &'a str);

impl<'a> Localizer for Lang<'a> {
    fn tr(&self, key: &str) -> Cow<'static, str> {
        match self.0 {
            "en-US" | "en" => translate_en(key),
            _ => translate_zh_cn(key),
        }
    }
}

fn translate_zh_cn(key: &str) -> Cow<'static, str> {
    match key {
        // Status Group
        "static_server" => "静态站点".into(),
        "controller_api" => "控制接口".into(),
        "admin_server" => "配置管理".into(),
        "not_started" => "未启动".into(),
        "starting" => "启动中...".into(),
        "initializing" => "初始化中".into(),
        "stopped" => "已停止".into(),
        
        // Privilege Group
        "admin_privilege" => "管理员权限".into(),
        "checking" => "检测中...".into(),
        "acquired" => "已获取".into(),
        "not_acquired" => "未获取（开机自启需管理员）".into(),
        "restart_admin" => "以管理员身份重启".into(),
        "factory_reset" => "恢复出厂设置".into(),
        "factory_reset_confirm_title" => "恢复出厂设置".into(),
        "factory_reset_confirm_msg" => "恢复出厂设置会清空所有配置、已下载内核、日志与应用设置，并重启服务。是否继续？".into(),
        "factory_reset_failed" => "恢复出厂设置失败".into(),
        "admin_restart_failed" => "以管理员身份重启失败".into(),
        "already_admin" => "当前已是管理员权限，无需重启".into(),

        // Settings Group
        "settings" => "设置".into(),
        "autostart" => "开机自启".into(),
        "autostart_admin_required" => "开机自启（需管理员）".into(),
        "open_webui_startup" => "启动时打开 Web UI".into(),
        "tun_mode" => "TUN 模式".into(),
        "tun_mode_admin_required" => "TUN 模式（需管理员）".into(),
        "tun_mode_disabled" => "TUN 模式（配置未启用）".into(),
        "system_proxy" => "系统代理".into(),
        "enabled" => "已开启".into(),
        "disabled" => "已关闭".into(),
        "open_browser" => "打开浏览器".into(),
        "open_config_manager" => "打开配置管理".into(),
        "advanced_settings" => "高级设置".into(),
        "dns_settings" => "DNS 设置".into(),
        "fake_ip_settings" => "Fake-IP 设置".into(),
        "fake_ip_flush" => "清理 Fake-IP 缓存".into(),
        "rules_settings" => "规则集设置".into(),
        "tun_settings" => "TUN 高级设置".into(),
        
        // Core Group
        "core_manager" => "内核管理".into(),
        "current_core" => "当前内核".into(),
        "default_core" => "默认内核".into(),
        "downloaded_version" => "已下载版本".into(),
        "update_status" => "更新状态".into(),
        "network_check" => "网络".into(),
        "update_to_stable" => "更新到最新 Stable".into(),
        "reading" => "读取中...".into(),
        "idle" => "空闲".into(),
        "updating" => "更新中...".into(),
        "not_set" => "未设置".into(),
        "empty" => "暂无已下载版本".into(),
        "use" => "启用".into(),
        "delete" => "删除".into(),
        "delete_confirm_title" => "删除内核版本".into(),
        "delete_confirm_msg" => "确定删除内核版本 {0} 吗？该操作无法撤销。".into(),
        
        // Runtime/Proxy Group
        "proxy_mode" => "代理模式".into(),
        "mode_rule" => "规则模式".into(),
        "mode_global" => "全局代理".into(),
        "mode_direct" => "直连模式".into(),
        "mode_script" => "脚本模式".into(),
        "profile_switch" => "配置切换".into(),
        "profile_read_failed" => "配置读取失败".into(),
        "profile_empty" => "暂无配置".into(),
        "more_profiles" => "更多配置".into(),
        "update_all_subs" => "立即更新所有订阅".into(),
        "auto_update_sub" => "自动更新当前订阅".into(),
        "subscription" => "订阅".into(),
        "proxy_groups" => "代理组".into(),
        "proxy_groups_read_failed" => "代理组读取失败".into(),
        "proxy_groups_empty" => "暂无可选代理组".into(),
        "more_groups" => "更多代理组".into(),
        "no_nodes" => "暂无节点".into(),
        "more_nodes" => "更多节点".into(),

        // Sync Group
        "sync_and_backup" => "同步与备份".into(),
        "webdav_sync" => "WebDAV 同步".into(),
        "sync_now" => "立即同步".into(),
        "sync_settings" => "同步设置".into(),
        "webdav_sync_success" => "同步完成".into(),
        "webdav_sync_failed" => "同步失败".into(),
        "webdav_sync_done" => "成功同步 {0} 个文件".into(),
        "webdav_sync_error" => "同步错误: {0}".into(),
        "webdav_sync_disabled" => "WebDAV 同步未启用".into(),
        "sync_now_failed" => "立即同步失败".into(),

        // About/Quit
        "about" => "关于".into(),
        "quit" => "退出".into(),
        "tray_init_failed" => "托盘菜单初始化失败".into(),
        "core_service" => "core-service".into(),
        "unknown" => "未知".into(),

        // Notifications
        "sub_update_success" => "订阅更新成功".into(),
        "sub_update_failed" => "订阅更新失败".into(),
        "sub_updated" => "配置 \"{0}\" 已更新".into(),
        "sub_failed_reason" => "配置 \"{0}\" 更新失败：{1}".into(),
        "sub_updating_title" => "订阅更新中".into(),
        "sub_updating_msg" => "正在更新所有订阅...".into(),
        "sub_update_done" => "订阅更新完成".into(),
        "sub_update_summary" => "成功 {0}，失败 {1}，跳过 {2}".into(),
        "switch_proxy_failed" => "切换代理失败".into(),
        "switch_mode_failed" => "切换代理模式失败".into(),
        "switch_profile_failed" => "切换配置失败".into(),
        "toggle_autostart_failed" => "切换开机自启失败".into(),
        "autostart_need_admin" => "开启开机自启需要管理员权限".into(),
        "toggle_webui_failed" => "切换启动打开 Web UI 失败".into(),
        "toggle_tun_failed" => "切换 TUN 模式失败".into(),
        "tun_need_admin" => "启用 TUN 需要管理员权限".into(),
        "core_update_failed" => "更新 Mihomo 内核失败".into(),
        "switch_core_failed" => "切换内核版本失败".into(),
        "delete_core_failed" => "删除内核版本失败".into(),
        "toggle_auto_update_failed" => "切换自动更新失败".into(),
        "core_startup_failed" => "无法启动 mihomo 服务".into(),
        "control_startup_failed" => "控制接口: 启动失败".into(),

        _ => key.to_string().into(),
    }
}

fn translate_en(key: &str) -> Cow<'static, str> {
    match key {
        // Status Group
        "static_server" => "Static Site".into(),
        "controller_api" => "Controller".into(),
        "admin_server" => "Config Mgr".into(),
        "not_started" => "Not Started".into(),
        "starting" => "Starting...".into(),
        "initializing" => "Initializing...".into(),
        "stopped" => "Stopped".into(),

        // Privilege Group
        "admin_privilege" => "Admin Privilege".into(),
        "checking" => "Checking...".into(),
        "acquired" => "Acquired".into(),
        "not_acquired" => "Not Acquired (Req for Autostart)".into(),
        "restart_admin" => "Restart as Admin".into(),
        "factory_reset" => "Factory Reset".into(),
        "factory_reset_confirm_title" => "Factory Reset".into(),
        "factory_reset_confirm_msg" => "This will clear all configs, downloaded cores, logs and settings. Continue?".into(),
        "factory_reset_failed" => "Factory Reset Failed".into(),
        "admin_restart_failed" => "Restart as Admin Failed".into(),
        "already_admin" => "Already running as admin".into(),

        // Settings Group
        "settings" => "Settings".into(),
        "autostart" => "Autostart".into(),
        "autostart_admin_required" => "Autostart (Admin Req)".into(),
        "open_webui_startup" => "Open Web UI on Startup".into(),
        "tun_mode" => "TUN Mode".into(),
        "tun_mode_admin_required" => "TUN Mode (Admin Req)".into(),
        "tun_mode_disabled" => "TUN Mode (Disabled in Config)".into(),
        "system_proxy" => "System Proxy".into(),
        "enabled" => "Enabled".into(),
        "disabled" => "Disabled".into(),
        "open_browser" => "Open Browser".into(),
        "open_config_manager" => "Open Config Manager".into(),
        "advanced_settings" => "Advanced Settings".into(),
        "dns_settings" => "DNS Settings".into(),
        "fake_ip_settings" => "Fake-IP Settings".into(),
        "fake_ip_flush" => "Flush Fake-IP Cache".into(),
        "rules_settings" => "Rules Settings".into(),
        "tun_settings" => "TUN Advanced".into(),

        // Core Group
        "core_manager" => "Core Manager".into(),
        "current_core" => "Current Core".into(),
        "default_core" => "Default Core".into(),
        "downloaded_version" => "Downloaded".into(),
        "update_status" => "Status".into(),
        "network_check" => "Network".into(),
        "update_to_stable" => "Update to Latest Stable".into(),
        "reading" => "Reading...".into(),
        "idle" => "Idle".into(),
        "updating" => "Updating...".into(),
        "not_set" => "Not Set".into(),
        "empty" => "No downloaded versions".into(),
        "use" => "Use".into(),
        "delete" => "Delete".into(),
        "delete_confirm_title" => "Delete Core Version".into(),
        "delete_confirm_msg" => "Delete core version {0}? This cannot be undone.".into(),

        // Runtime/Proxy Group
        "proxy_mode" => "Proxy Mode".into(),
        "mode_rule" => "Rule".into(),
        "mode_global" => "Global".into(),
        "mode_direct" => "Direct".into(),
        "mode_script" => "Script".into(),
        "profile_switch" => "Profiles".into(),
        "profile_read_failed" => "Failed to load profiles".into(),
        "profile_empty" => "No profiles".into(),
        "more_profiles" => "More Profiles".into(),
        "update_all_subs" => "Update All Subscriptions".into(),
        "auto_update_sub" => "Auto-update Current".into(),
        "subscription" => "Sub".into(),
        "proxy_groups" => "Proxy Groups".into(),
        "proxy_groups_read_failed" => "Failed to load groups".into(),
        "proxy_groups_empty" => "No groups available".into(),
        "more_groups" => "More Groups".into(),
        "no_nodes" => "No nodes".into(),
        "more_nodes" => "More Nodes".into(),

        // Sync Group
        "sync_and_backup" => "Sync & Backup".into(),
        "webdav_sync" => "WebDAV Sync".into(),
        "sync_now" => "Sync Now".into(),
        "sync_settings" => "Sync Settings".into(),
        "webdav_sync_success" => "Sync Complete".into(),
        "webdav_sync_failed" => "Sync Failed".into(),
        "webdav_sync_done" => "Synced {0} file(s)".into(),
        "webdav_sync_error" => "Sync error: {0}".into(),
        "webdav_sync_disabled" => "WebDAV sync not enabled".into(),
        "sync_now_failed" => "Sync Now Failed".into(),

        // About/Quit
        "about" => "About".into(),
        "quit" => "Quit".into(),
        "tray_init_failed" => "Tray Init Failed".into(),
        "core_service" => "core-service".into(),
        "unknown" => "Unknown".into(),

        // Notifications
        "sub_update_success" => "Subscription Updated".into(),
        "sub_update_failed" => "Update Failed".into(),
        "sub_updated" => "Profile \"{0}\" updated".into(),
        "sub_failed_reason" => "Profile \"{0}\" failed: {1}".into(),
        "sub_updating_title" => "Updating Subscriptions".into(),
        "sub_updating_msg" => "Updating all subscriptions...".into(),
        "sub_update_done" => "Update Completed".into(),
        "sub_update_summary" => "Success {0}, Failed {1}, Skipped {2}".into(),
        "switch_proxy_failed" => "Switch Proxy Failed".into(),
        "switch_mode_failed" => "Switch Mode Failed".into(),
        "switch_profile_failed" => "Switch Profile Failed".into(),
        "toggle_autostart_failed" => "Toggle Autostart Failed".into(),
        "autostart_need_admin" => "Autostart requires admin privilege".into(),
        "toggle_webui_failed" => "Toggle Web UI setting failed".into(),
        "toggle_tun_failed" => "Toggle TUN Mode Failed".into(),
        "tun_need_admin" => "TUN Mode requires admin privilege".into(),
        "core_update_failed" => "Core Update Failed".into(),
        "switch_core_failed" => "Switch Core Failed".into(),
        "delete_core_failed" => "Delete Core Failed".into(),
        "toggle_auto_update_failed" => "Toggle Auto-update Failed".into(),
        "core_startup_failed" => "Mihomo startup failed".into(),
        "control_startup_failed" => "Controller: Startup Failed".into(),

        _ => key.to_string().into(),
    }
}
