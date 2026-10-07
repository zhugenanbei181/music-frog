//! Canonical error messages and recovery suggestions.
use std::borrow::Cow;

pub(super) fn translate_errors_zh(key: &str) -> Cow<'static, str> {
    match key {
        "admin_settings_save_failed" => "管理端设置保存失败：{reason}".into(),
        "connections_refresh_failed" => "连接刷新失败：{reason}".into(),
        "core_boot_ports_failed" => "启动失败（已尝试控制端口 {ports}）：{reason}".into(),
        "core_egress_not_running" => "内核未运行，无法探测代理出口 IP".into(),
        "core_rebuild_restored" => "重建失败，已恢复上一份配置：{reason}".into(),
        "core_reset_stop_timeout" => "停止内核超时，未执行恢复出厂".into(),
        "elevation_restart_unavailable" => {
            "当前平台不支持自动提权重启，请手动以管理员（root）权限运行本程序".into()
        }
        "lan_access_verified" => "局域网 ACL 与认证设置已应用并完成回读".into(),
        "lan_listen_verified" => "局域网监听设置已应用并完成回读".into(),
        "memory_refresh_failed" => "内存刷新失败：{reason}".into(),
        "mtu_probe_failed_notice" => "MTU 探测失败：{reason}".into(),
        "mtu_probe_unavailable" => "当前宿主未提供物理链路 MTU 探测能力".into(),
        "mtu_probe_unsupported" => "当前宿主不支持物理链路 MTU 探测".into(),
        "network_roaming_unavailable" => "当前宿主未提供物理网卡漫游能力".into(),
        "network_route_repair_unavailable" => "当前宿主未提供网卡漫游路由修复能力".into(),
        "pac_service_unavailable" => "当前宿主未提供 PAC 本地服务能力".into(),
        "pac_service_verified" => "PAC 脚本已生成，本地服务状态已回读".into(),
        "port_check_completed" => "端口检查完成，未执行未确认进程终止".into(),
        "port_conflict_repaired" => "端口冲突已修复，已安全避让到可用端口".into(),
        "privileged_network_unavailable" => "当前宿主未注入特权网络回归适配器".into(),
        "privileged_network_verified" => "特权网络回归已注入、回读并完成清理".into(),
        "profile_folder_open_failed" => "无法打开配置文件夹：{reason}".into(),
        "profile_folder_opened" => "配置文件夹已打开".into(),
        "profile_import_requires_stopped_core" => {
            "内核运行时不能直接覆盖当前配置，请先停止内核后再导入".into()
        }
        "runtime_action_gateway_repair" => "修复 TUN 默认网关路由".into(),
        "runtime_action_ipv6" => "修改 IPv6 内核流量策略".into(),
        "runtime_action_lan" => "应用局域网共享设置".into(),
        "runtime_action_lan_access" => "应用局域网 ACL 与认证设置".into(),
        "runtime_action_mtu_probe" => "探测物理链路 MTU".into(),
        "runtime_action_network_probe" => "探测物理网卡与默认网关".into(),
        "runtime_action_pac" => "应用 PAC 本地服务".into(),
        "runtime_action_requires_core" => "内核未运行，无法{operation}".into(),
        "runtime_action_sniffer" => "修改嗅探器状态".into(),
        "runtime_action_tun" => "修改 TUN 状态".into(),
        "runtime_action_tun_auto_route" => "修改 TUN 自动路由".into(),
        "runtime_action_tun_stack" => "修改 TUN 堆栈".into(),
        "runtime_action_tun_strict_route" => "修改 TUN 严格路由".into(),
        "runtime_action_vpn_start" => "申请 Android VPN 服务".into(),
        "runtime_action_vpn_stop" => "停止 Android VPN 服务".into(),
        "subscription_cron_invalid" => "Cron 表达式无效：{reason}".into(),
        "system_proxy_control_unavailable" => "当前宿主未提供系统代理控制能力".into(),
        "system_proxy_external_preserved" => "检测到外部系统代理修改，未覆盖该设置".into(),
        "system_proxy_orphan_restored" => "已恢复上次异常退出遗留的系统代理设置".into(),
        "system_proxy_owned_restored" => "系统代理设置被其他程序修改，已自动恢复".into(),
        "system_proxy_port_missing" => "当前配置未提供 port 或 mixed-port".into(),
        "system_proxy_recovery_failed_notice" => "系统代理启动恢复失败：{reason}".into(),
        "system_proxy_requires_core" => "内核未运行，无法确定系统代理端口".into(),
        "tun_prepare_requires_core" => "内核未运行，无法准备 TUN 服务".into(),
        "tun_service_ready" => "TUN 服务已就绪".into(),
        "tun_service_unavailable" => "当前平台未提供 TUN 服务模式".into(),
        "tun_start_requires_core" => "内核未运行，无法启用 TUN".into(),
        "uwp_loopback_verified" => "UWP 回环豁免已应用并完成回读".into(),
        "uwp_scan_count" => "已扫描 {count} 个 UWP AppContainer".into(),
        "vpn_running_verified" => "Android VPN 隧道已启动并完成前台 readback".into(),
        "vpn_service_starting" => "Android VPN 前台服务已启动，等待隧道 FD".into(),
        "vpn_service_unavailable" => "当前宿主未提供 Android VpnService 能力".into(),
        "vpn_status_updated" => "Android VPN 状态已更新".into(),
        "vpn_stopped_notice" => "Android VPN 已停止".into(),
        "vpn_unsupported_reason" => "当前宿主不支持 Android VPN：{reason}".into(),
        "vpn_waiting_permission" => "等待 Android VPN 用户授权".into(),
        "webdav_connection_success" => "WebDAV 连接成功".into(),
        "webdav_credentials_required" => "WebDAV 地址和用户名不能为空".into(),
        "download_canceled" => "下载已取消".into(),
        "failure_yaml_syntax" => "YAML 语法错误（第 {line} 行，第 {column} 列）：{detail}".into(),
        "failure_mixin_yaml" => "Mixin 覆盖不是有效的 YAML：{detail}".into(),
        "failure_active_profile_inconsistent" => "活动配置观测不一致：{detail}".into(),
        "failure_quota_source_changed" => "配额读取期间订阅来源已变化：{detail}".into(),
        "failure_current_time_unavailable" => "无法取得当前时间：{detail}".into(),
        "error_port_in_use_message" => "端口 {port} 已被占用".into(),
        "error_port_in_use_suggestion" => {
            "端口 {port} 已被占用，请检查是否已运行其他代理客户端或在设置中更换端口".into()
        }
        "error_kernel_crash_message" => "内核已崩溃".into(),
        "error_kernel_crash_suggestion" => "请尝试重启应用或检查日志获取更多信息。".into(),
        "error_kernel_not_found_message" => "未找到内核文件".into(),
        "error_kernel_not_found_suggestion" => "请重新安装应用或手动下载内核。".into(),
        "error_config_invalid_message" => "配置文件无效: {reason}".into(),
        "error_config_invalid_suggestion" => {
            "请检查您的配置文件语法是否有误，或重置为默认配置。".into()
        }
        "error_readiness_timeout_message" => "内核启动超时".into(),
        "error_readiness_timeout_suggestion" => {
            "内核启动时间过长，可能是系统资源不足或配置有误。".into()
        }
        "error_secret_mismatch_message" => "API 密钥不匹配".into(),
        "error_secret_mismatch_suggestion" => "请确保客户端与内核使用的 API 密钥一致。".into(),
        "error_subscription_fetch_failed_message" => "订阅获取失败: {reason}".into(),
        "error_subscription_fetch_failed_suggestion" => {
            "请检查网络连接或订阅链接是否仍然有效。".into()
        }
        "error_subscription_empty_message" => "订阅内容为空".into(),
        "error_subscription_empty_suggestion" => {
            "获取到的订阅未包含任何节点，请联系订阅提供商。".into()
        }
        "error_invalid_subscription_url_message" => "无效的订阅链接: {reason}".into(),
        "error_invalid_subscription_url_suggestion" => {
            "请确保填写的订阅链接格式正确（例如以 http:// 或 https:// 开头）。".into()
        }
        "error_subscription_decode_error_message" => "订阅解析失败: {reason}".into(),
        "error_subscription_decode_error_suggestion" => {
            "无法识别订阅格式，可能是该订阅已被加密或格式不受支持。".into()
        }
        "error_web_dav_auth_failed_message" => "WebDAV 认证失败".into(),
        "error_web_dav_auth_failed_suggestion" => "请检查您的 WebDAV 账号和密码是否正确。".into(),
        "error_web_dav_network_error_message" => "WebDAV 网络错误: {reason}".into(),
        "error_web_dav_network_error_suggestion" => {
            "无法连接到 WebDAV 服务器，请检查网络或服务器状态。".into()
        }
        "error_web_dav_conflict_message" => "WebDAV 冲突".into(),
        "error_web_dav_conflict_suggestion" => {
            "远程数据与本地数据发生冲突，请手动解决冲突后重试。".into()
        }
        "error_tun_privilege_missing_message" => "缺少 TUN 模式权限".into(),
        "error_tun_privilege_missing_suggestion" => {
            "启用 TUN 模式需要管理员权限，请以管理员身份运行本程序。".into()
        }
        "error_system_proxy_failed_message" => "系统代理设置失败: {reason}".into(),
        "error_system_proxy_failed_suggestion" => {
            "无法自动配置系统代理，请尝试手动设置或检查系统权限。".into()
        }
        "error_keyring_error_message" => "密钥环错误: {reason}".into(),
        "error_keyring_error_suggestion" => {
            "无法访问系统安全存储。在 Linux 上请确保安装并启动了 gnome-keyring 或 kwallet。".into()
        }
        "error_autostart_failed_message" => "开机自启设置失败: {reason}".into(),
        "error_autostart_failed_suggestion" => {
            "无法配置开机自启动，可能是权限不足或系统不支持。".into()
        }
        "error_internal_message" => "内部错误: {reason}".into(),
        "error_internal_suggestion" => "发生了未知错误，请报告此问题以便我们修复。".into(),
        "error_network_timeout_message" => "网络请求超时".into(),
        "error_network_timeout_suggestion" => "请检查您的网络连接并重试。".into(),
        _ => key.to_owned().into(),
    }
}
