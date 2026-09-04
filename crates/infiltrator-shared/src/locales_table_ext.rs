//! Extended zh-CN copy table backing [`super::Localizer`].
//! Modular table split to ensure strict conformance with the 800-line source budget.

use std::borrow::Cow;

pub(super) fn translate_zh_cn_ext(key: &str) -> Cow<'static, str> {
    match key {
        // Category 1: DNS Leak & Privacy Probe
        "dns_leak_probe_title" => "DNS 泄漏与公网 IP 隐私检测".into(),
        "dns_leak_probe_desc" => "多源并发检测当前出口公网 IP、地理位置与运营商，排查 DNS 真实解析泄露".into(),
        "dns_leak_btn_run" => "发起隐私检测".into(),
        "dns_leak_status_secure" => "DNS 与出站网络安全，未发现泄漏".into(),
        "dns_leak_status_leaked" => "检测到潜在 DNS 泄漏风险".into(),
        "dns_leak_public_ip" => "出口公网 IP".into(),
        "dns_leak_location" => "IP 归属地".into(),
        "dns_leak_isp" => "运营商 / ASN".into(),
        "dns_leak_tested_servers" => "实际响应 DNS 节点".into(),

        // Category 2: Custom Node Editor & Universal URI Codec
        "custom_node_title" => "自定义节点表单与 URI 编解码".into(),
        "custom_node_btn_add" => "添加自建节点".into(),
        "custom_node_btn_import_uri" => "从分享链接导入 (URI)".into(),
        "custom_node_btn_export_uri" => "导出分享链接".into(),
        "custom_node_type" => "节点协议类型".into(),
        "custom_node_name" => "节点名称".into(),
        "custom_node_server" => "服务器地址".into(),
        "custom_node_port" => "端口".into(),
        "custom_node_uuid_pass" => "UUID / 密码".into(),
        "custom_node_sni" => "TLS / Reality SNI 伪装".into(),
        "custom_node_uri_placeholder" => "粘贴 vless://, ss://, hysteria2://, trojan:// 节点链接...".into(),

        // Category 3: Multi-Profile Aggregator & Topology Generator
        "aggregator_title" => "多订阅配置聚合器".into(),
        "aggregator_desc" => "勾选多个订阅配置，自动去重节点并按国家地区生成自动测速与分流策略组".into(),
        "aggregator_btn_merge" => "执行合并聚合".into(),
        "aggregator_name_placeholder" => "聚合配置名称 (例如: Aggregated-All)".into(),
        "aggregator_selected_count" => "已选中 {count} 个订阅".into(),
        "aggregator_result_nodes" => "已合并 {count} 个有效节点".into(),

        // Category 4: Connection Grouping & Quick-Rule Generator
        "conn_grouping_mode" => "连接聚合模式".into(),
        "conn_group_flat" => "实时流".into(),
        "conn_group_process" => "按进程聚合".into(),
        "conn_group_host" => "按域名聚合".into(),
        "quick_rule_btn" => "一键添加分流规则".into(),
        "quick_rule_success" => "已将目标添加至分流规则".into(),

        // Category 5: Config Snapshot Visual Diff & Rollback
        "snapshot_diff_title" => "配置历史快照差异比对与回滚".into(),
        "snapshot_diff_compare_with" => "比对历史快照版本".into(),
        "snapshot_diff_rollback_btn" => "安全回滚至此版本".into(),
        "snapshot_diff_no_changes" => "当前配置与该快照内容一致，无变更".into(),

        // Category 6: Global Hotkey Manager & Keybinding Customizer
        "hotkey_manager_title" => "桌面全局快捷键管理".into(),
        "hotkey_manager_desc" => "在后台运行与游戏全屏时通过全局热键瞬时调控核心行为".into(),
        "hotkey_system_proxy" => "切换系统代理".into(),
        "hotkey_tun_mode" => "切换 TUN 虚拟网卡".into(),
        "hotkey_mini_hud" => "唤出/收起迷你网速悬浮窗".into(),
        "hotkey_speed_test" => "触发全节点并发测速".into(),
        "hotkey_mode_switch" => "循环切换分流/全局/直连模式".into(),
        "hotkey_conflict_warning" => "快捷键冲突：该按键已被系统或其它软件占用".into(),

        // Category 1: PCAP Exporter & Sniffer
        "pcap_title" => "PCAP 抓包与 Sniffer 流量审计".into(),
        "pcap_btn_start" => "开始抓包".into(),
        "pcap_btn_stop" => "停止抓包".into(),
        "pcap_btn_export" => "导出 .pcap".into(),
        "pcap_capturing" => "抓包中 ({count} 个数据包 / {bytes})".into(),
        "pcap_idle" => "未开启抓包".into(),

        // Category 2: Logical Sub-Rules Builder
        "subrules_title" => "复合逻辑规则构建器 (Sub-Rules)".into(),
        "subrules_operator" => "逻辑操作符 (AND / OR / NOT)".into(),
        "subrules_btn_add_leaf" => "添加子条件".into(),
        "subrules_target" => "目标策略".into(),
        "subrules_result_preview" => "规则表达式预览".into(),
        "subrules_btn_insert" => "插入分流规则".into(),

        // Category 3: Speedtest & Jitter Benchmark
        "speedtest_title" => "节点带宽与抖动率测速".into(),
        "speedtest_btn_start" => "开始真实测速".into(),
        "speedtest_measuring" => "测速中...".into(),
        "speedtest_bandwidth" => "下行带宽".into(),
        "speedtest_jitter" => "网络抖动 (Jitter)".into(),
        "speedtest_packet_loss" => "丢包率".into(),
        "speedtest_stability" => "节点稳定性等级".into(),

        // Category 4: Geo Database Updater
        "geodata_title" => "GeoIP / GeoSite 数据库管理".into(),
        "geodata_btn_check" => "检查在线更新".into(),
        "geodata_btn_update" => "立即增量更新".into(),
        "geodata_geoip_status" => "GeoIP 数据库".into(),
        "geodata_geosite_status" => "GeoSite 数据库".into(),
        "geodata_updated" => "已是最新版本".into(),
        "geodata_updating" => "正在更新数据库...".into(),

        // Category 5: UWP Loopback Utility
        "uwp_title" => "Windows UWP 本地回环隔离管理".into(),
        "uwp_desc" => "一键解除微软商店应用 (UWP) 的本地代理回环限制".into(),
        "uwp_btn_scan" => "扫描 UWP 应用".into(),
        "uwp_btn_exempt_all" => "全选豁免 (Exempt All)".into(),
        "uwp_btn_clear_all" => "重置隔离 (Clear All)".into(),
        "uwp_search" => "搜索 UWP 应用名称或 PackageId...".into(),
        "uwp_exempted_count" => "已豁免 {count} 个应用".into(),

        // Category 6: Encrypted Backup (.encpkg)
        "encpkg_title" => "端到端加密备份包 (.encpkg)".into(),
        "encpkg_desc" => "使用强密码基于 AES-256 算法加密打包所有订阅、自定义规则与 Mixin".into(),
        "encpkg_pass_placeholder" => "输入备份保护密码 (至少 6 位)...".into(),
        "encpkg_btn_export" => "导出加密备份包".into(),
        "encpkg_btn_import" => "导入加密备份包".into(),
        "encpkg_success" => "加密备份包操作成功".into(),
        // Category 1: Network Interface Roaming & Gateway Recovery
        "net_roam_title" => "物理网卡漫游与网关自愈".into(),
        "net_roam_desc" => "感知 Wi-Fi/以太网漫游切换，动态自适应最佳 MTU 并自愈路由表".into(),
        "net_roam_btn_reconnect" => "强制网关重连自愈".into(),
        "net_roam_active_iface" => "活跃出站网卡".into(),
        "net_roam_gateway" => "默认网关 IP".into(),
        "net_roam_mtu" => "自适应最佳 MTU".into(),

        // Category 2: Crash Watchdog & Forensic Viewer
        "crash_watchdog_title" => "崩溃自愈看门狗与脱敏取证".into(),
        "crash_watchdog_desc" => "监控异常退出与 Panic 状态，提供本地脱敏调用栈分析与孤儿状态恢复".into(),
        "crash_watchdog_btn_recover" => "一键恢复网络与清退孤儿状态".into(),
        "crash_watchdog_btn_export" => "导出脱敏取证包".into(),
        "crash_watchdog_clean" => "系统运行正常，未检测到异常退出与孤儿状态".into(),
        "crash_watchdog_recovered" => "已成功清退孤儿状态并恢复系统网络".into(),

        // Category 3: External Web Dashboard
        "web_dash_title" => "外部 Web 仪表盘 (Web Dashboard)".into(),
        "web_dash_desc" => "内置 Metacubexd、Yacd 与 Razord 控制台，免密携带 Token 自动握手拉起".into(),
        "web_dash_btn_metacubexd" => "打开 Metacubexd".into(),
        "web_dash_btn_yacd" => "打开 Yacd".into(),
        "web_dash_btn_razord" => "打开 Razord".into(),

        // Category 4: Log Regex Highlighting & Redacted Export
        "logs_regex_placeholder" => "正则高亮过滤 (例如: connect|error|dns)...".into(),
        "logs_btn_export_redacted" => "一键脱敏导出".into(),
        "logs_level_all" => "全部".into(),
        "logs_export_success" => "脱敏日志已导出至本地文件".into(),

        // Category 5: Subscription Quota & Cron Scheduler
        "sub_quota_title" => "订阅配额与临期智能预警".into(),
        "sub_quota_desc" => "实时监控机场订阅已用/剩余流量与有效期，智能分级预警".into(),
        "sub_quota_used" => "已用流量".into(),
        "sub_quota_remaining" => "剩余可用".into(),
        "sub_quota_expire" => "到期时间".into(),
        "sub_quota_cron" => "自动轮询更新周期".into(),

        // Category 6: PAC Auto-Proxy & Bypass CIDR Manager
        "pac_title" => "PAC 自动代理与绕过网段管理".into(),
        "pac_desc" => "生成浏览器通用的 PAC (Proxy Auto-Config) 脚本，精准旁路局域网".into(),
        "pac_url_label" => "本地 PAC 服务地址".into(),
        "pac_bypass_cidrs" => "自定义绕过网段列表 (逗号或分号分隔)".into(),
        "pac_btn_compile" => "编译并验证 PAC".into(),
        "pac_compile_success" => "PAC 脚本编译成功并已热加载".into(),
        // Wave 5 Category 1: Rule Hit Counter & Stale Rule Analyzer
        "rule_hit_title" => "分流规则命中统计与冷门审计".into(),
        "rule_hit_desc" => "统计当前会话各规则累计命中频次，快速识别并清理 0 次命中的冷门规则".into(),
        "rule_hit_btn_audit" => "审计冷门规则".into(),
        "rule_hit_btn_clean" => "一键停用 0 命中规则".into(),
        "rule_hit_total_hits" => "累计总命中次数".into(),
        "rule_hit_stale_count" => "发现 {count} 条冷门规则".into(),

        // Wave 5 Category 2: Latency Time-Series & Stability Radar
        "latency_radar_title" => "节点时序延迟与稳定性雷达".into(),
        "latency_radar_desc" => "多点时序采样分析节点往返延迟波动、抖动率与可用性评级".into(),
        "latency_radar_avg" => "平均延迟".into(),
        "latency_radar_min_max" => "波动区间 (Min/Max)".into(),
        "latency_radar_score" => "稳定性评分".into(),

        // Wave 5 Category 3: TUN Multi-Stack & MTU Negotiator
        "tun_stack_title" => "TUN 虚拟网卡多堆栈与 MTU 自适应协商".into(),
        "tun_stack_desc" => "选择内核或用户态网络驱动堆栈，动态协商最佳物理 MTU".into(),
        "tun_stack_gvisor" => "gVisor (用户态安全沙盒)".into(),
        "tun_stack_system" => "System (原生内核高性能)".into(),
        "tun_stack_mixed" => "Mixed (混合分流模式)".into(),
        "tun_mtu_probe_btn" => "探测最佳 MTU".into(),

        // Wave 5 Category 4: Rule-Provider Lifecycle & Rule Unpacker
        "provider_unpack_title" => "规则集解构与本地规则提取".into(),
        "provider_unpack_desc" => "将远程 Rule-Provider 规则条目一键解构导入为本地可编辑规则".into(),
        "provider_btn_unpack" => "一键解构成自定义规则".into(),
        "provider_btn_purge_cache" => "清理规则集本地缓存".into(),
        "provider_cache_purged" => "规则集本地磁盘缓存已清理完毕".into(),

        // Wave 5 Category 5: Config Apply Multi-Stage Transaction Guard
        "apply_guard_title" => "配置生效多阶段原子事务守卫".into(),
        "apply_guard_desc" => "预检语法 -> 暂存配置 -> 核心热载 -> 健康探活 -> 失败原子回滚".into(),
        "apply_guard_stage_preflight" => "语法结构预检".into(),
        "apply_guard_stage_reloading" => "核心热重载中".into(),
        "apply_guard_stage_probing" => "网络连通性探活".into(),
        "apply_guard_status_committed" => "原子事务提交成功".into(),
        "apply_guard_status_rolled_back" => "探活失败，已自动安全回滚".into(),

        // Wave 5 Category 6: LAN Proxy Sharing & Client Access Whitelist
        "lan_sharing_title" => "局域网共享代理与访问控制列表 (ACL)".into(),
        "lan_sharing_desc" => "允许局域网设备接入当前代理连接，并严格基于 IP/CIDR 白名单授权".into(),
        "lan_sharing_enable" => "开启局域网共享 (Allow LAN)".into(),
        "lan_sharing_port" => "局域网混合代理端口".into(),
        "lan_sharing_acl" => "允许接入的客户端 IP 白名单 (CIDR)".into(),
        _ => key.to_string().into(),
    }
}
