//! Extended zh-CN copy table backing [`super::Localizer`].
//! Modular table split to ensure strict conformance with the 800-line source budget.

use std::borrow::Cow;

pub(super) fn translate_zh_cn_ext(key: &str) -> Cow<'static, str> {
    match key {
        // Category 1: DNS Leak & Privacy Probe
        "dns_leak_probe_title" => "DNS 泄漏与公网 IP 隐私检测".into(),
        "dns_leak_probe_desc" => {
            "多源并发检测当前出口公网 IP、地理位置与运营商，排查 DNS 真实解析泄露".into()
        }
        "dns_leak_btn_run" => "发起隐私检测".into(),
        "dns_leak_status_secure" => "DNS 与出站网络安全，未发现泄漏".into(),
        "dns_leak_status_leaked" => "检测到潜在 DNS 泄漏风险".into(),
        "dns_leak_public_ip" => "出口公网 IP".into(),
        "dns_leak_location" => "IP 归属地".into(),
        "dns_leak_isp" => "运营商 / ASN".into(),
        "dns_leak_tested_servers" => "实际响应 DNS 节点".into(),

        // Category 1b: DNS workbench form parity & cache flush (DUAL-14)
        "dns_form_issues" => "表单校验未通过".into(),
        "dns_form_err_scheme" => "字段 {field} 的上游协议不受支持: {entry}".into(),
        "dns_form_err_bootstrap" => "bootstrap 解析器必须是纯 IP: {entry}".into(),
        "dns_form_err_cidr" => "fallback 触发网段不是合法 CIDR: {entry}".into(),
        "dns_form_err_geoip_code" => "geoip-code 必须是两位国家代码: {value}".into(),
        "dns_flush_target_fakeip" => "Fake-IP 缓存".into(),
        "dns_flush_target_os" => "系统 DNS 缓存".into(),
        "dns_flush_not_requested" => "尚未执行".into(),
        "dns_flush_flushed" => "已清空".into(),
        "dns_flush_unsupported" => "宿主不支持".into(),
        "dns_flush_failed" => "清理失败".into(),

        // Category 1c: Fake-IP pool, hosts editor & latency policy (DUAL-14-06/10/11)
        "dns_fakeip_pool_title" => "Fake-IP 映射池实时检索".into(),
        "dns_fakeip_pool_desc" => "检索运行中内核实时连接的域名 ↔ 虚拟 IP 绑定".into(),
        "dns_fakeip_pool_search" => "搜索域名或虚拟 IP".into(),
        "dns_fakeip_pool_empty" => "当前没有可观察的 Fake-IP 绑定".into(),
        "dns_fakeip_pool_observed" => "实时连接观测".into(),
        "dns_fakeip_pool_unsupported" => "宿主未提供映射事实".into(),
        "dns_fakeip_pool_total" => "显示 {shown} / 共观测 {total} 条".into(),
        "dns_latency_title" => "逐 Nameserver 延迟测速".into(),
        "dns_latency_ready" => "宿主提供真实延迟事实".into(),
        "dns_latency_unsupported" => "宿主未提供逐 Nameserver 延迟事实，不填充假延迟".into(),
        "dns_hosts_title" => "自定义 Hosts 映射编辑".into(),
        "dns_hosts_desc" => "写入 dns.hosts：值为 IP、lan 或别名域名；同一域名可配置多个 IP".into(),
        "dns_hosts_address" => "地址 (IP / lan / 别名域名)".into(),
        "dns_hosts_domain" => "域名".into(),
        "dns_hosts_add" => "添加映射".into(),
        "dns_hosts_apply" => "应用 Hosts (Apply)".into(),
        "dns_hosts_empty" => "尚未配置任何静态域名映射".into(),
        "dns_hosts_saving" => "正在提交 Hosts 映射".into(),
        "dns_hosts_pending" => "有未应用的 Hosts 修改".into(),
        "dns_hosts_saved" => "Hosts 映射与当前配置一致".into(),
        "dns_hosts_issue_address" => "地址必须是 IP、lan 或含点的别名域名: {value}".into(),
        "dns_hosts_issue_domain" => "域名不合法: {value}".into(),

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
        "custom_node_secret" => "口令 / UUID".into(),
        "custom_node_cipher" => "加密方式 (Shadowsocks 密码族)".into(),
        "custom_node_flow" => "VLESS 流控 (Vision)".into(),
        "custom_node_mux_enabled" => "多路复用".into(),
        "custom_node_mux_protocol" => "复用协议 (Smux/Yamux/H2Mux)".into(),
        "custom_node_mux_max" => "最大连接数".into(),
        "custom_node_mux_min_streams" => "最小流数".into(),
        "custom_node_mux_max_streams" => "最大流数".into(),
        "custom_node_mux_padding" => "填充混淆".into(),
        "custom_node_skip_verify" => "跳过证书校验".into(),
        "custom_node_issues_hint" => "问题必须先解决才能写入配置".into(),
        "custom_node_uri_gap" => "分享链接不携带 · {field}".into(),
        "custom_node_uri_placeholder" => {
            "粘贴 vless://, ss://, hysteria2://, trojan:// 节点链接...".into()
        }

        // Category 3: Multi-Profile Aggregator & Topology Generator
        "aggregator_title" => "多订阅配置聚合器".into(),
        "aggregator_desc" => {
            "勾选多个订阅配置，自动去重节点并按国家地区生成自动测速与分流策略组".into()
        }
        "aggregator_btn_merge" => "执行合并聚合".into(),
        "aggregator_name_placeholder" => "聚合配置名称 (例如: Aggregated-All)".into(),
        "aggregator_selected_count" => "已选中 {count} 个订阅".into(),
        "aggregator_result_nodes" => "已合并 {count} 个有效节点".into(),
        "aggregator_btn_preview" => "预览聚合结果".into(),
        "aggregator_btn_save" => "保存为新配置".into(),
        "aggregator_dedup" => "跨订阅节点自动去重".into(),
        "aggregator_geo_cluster" => "区域节点自动归类".into(),
        "aggregator_generate_groups" => "生成区域测速策略组".into(),
        "aggregator_remove_emojis" => "清洗节点名 emoji".into(),
        "aggregator_preview_title" => "聚合预览".into(),
        "aggregator_preview_nodes" => {
            "节点 {total} 个 · 去重 {removed} 个 · 归一化 {renamed} 个".into()
        }
        "aggregator_preview_input" => "输入节点 {count} 个".into(),
        "aggregator_preview_regions" => "区域分组 ({count})".into(),
        "aggregator_preview_groups" => "策略组拓扑 ({count})".into(),
        "aggregator_preview_master" => "主选择器级联".into(),
        "aggregator_region_nodes" => "{count} 节点".into(),
        "aggregator_missing_sources" => "未能读取: {names}".into(),
        "aggregator_preview_hint" => "勾选订阅源并选择清洗选项后点击预览".into(),
        "aggregator_group_select" => "手动选择".into(),
        "aggregator_group_urltest" => "自动测速".into(),
        "aggregator_group_custom" => "自定义".into(),
        // DUAL-08-07/08/09/10/11/12/13: wizard extensions
        "aggregator_availability_precheck" => "节点可用性预检过滤".into(),
        "aggregator_activate_after_create" => "创建后设为当前配置".into(),
        "aggregator_renames_label" => "节点重命名规则（正则）".into(),
        "aggregator_renames_ph" => "模式 => 替换，多条用 ; 分隔（如 -广告$ => ）".into(),
        "aggregator_rename_invalid" => "重命名规则格式错误（应为 模式 => 替换）: {line}".into(),
        "aggregator_custom_group_label" => "自定义策略组编排".into(),
        "aggregator_custom_name_ph" => "策略组名称（如: 流媒体专用）".into(),
        "aggregator_custom_keywords_ph" => "成员关键词，逗号分隔（留空 = 全部节点）".into(),
        "aggregator_custom_add" => "追加策略组".into(),
        "aggregator_custom_empty" => "尚未追加自定义策略组".into(),
        "aggregator_custom_all_nodes" => "全部节点".into(),
        "aggregator_custom_remove" => "移除".into(),
        "aggregator_custom_name_required" => "请填写自定义策略组名称".into(),
        "aggregator_preview_cleaning" => "规则重命名 {rules} 个 · 预检剔除 {invalid} 个".into(),
        "aggregator_preview_yaml" => "聚合 YAML 结构 · 共 {lines} 行".into(),
        "aggregator_select_source_required" => "请至少勾选一个订阅源".into(),
        "aggregator_name_required" => "请填写聚合配置名称".into(),
        "aggregator_created" => "聚合配置 '{name}' 已创建 · {state}".into(),
        "aggregator_refreshed" => "聚合配置 '{name}' 已重新聚合 · {state}".into(),
        "aggregator_state_saved" => "未激活".into(),
        "aggregator_state_switched" => "已设为当前配置".into(),
        "aggregator_state_reloaded" => "已激活并热载入内核".into(),
        "aggregator_template_title" => "历史聚合模板".into(),
        "aggregator_template_name_ph" => "模板名称".into(),
        "aggregator_template_save" => "保存为模板".into(),
        "aggregator_template_saved" => "模板 '{name}' 已保存".into(),
        "aggregator_template_missing" => "模板 '{name}' 不存在".into(),
        "aggregator_template_empty" => "暂无已保存模板".into(),
        "aggregator_template_updated" => "更新于 {time}".into(),
        "aggregator_template_use" => "复用".into(),
        "aggregator_template_reaggregate" => "重新聚合".into(),
        "aggregator_template_delete" => "删除".into(),

        // Category 4: Connection Grouping & Quick-Rule Generator
        "conn_grouping_mode" => "连接聚合模式".into(),
        "conn_group_flat" => "实时流".into(),
        "conn_group_process" => "按进程聚合".into(),
        "conn_group_host" => "按域名聚合".into(),
        "conn_aggregate_count" => "{count} 条连接".into(),
        "conn_aggregate_empty" => "暂无聚合数据".into(),
        "conn_close_filtered_btn" => "断开筛选结果".into(),
        "quick_rule_btn" => "一键添加分流规则".into(),
        "quick_rule_success" => "已将目标添加至分流规则".into(),
        "conn_idle_timeout_label" => "空闲超时".into(),
        "conn_idle_sweep_btn" => "清理空闲连接".into(),
        "conn_idle_last_sweep_none" => "上次清理: 尚未执行".into(),
        "conn_idle_last_sweep" => "上次清理: 清理 {count} 条空闲连接".into(),

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
        "subrules_no_conditions" => "尚未添加子条件".into(),
        "subrules_validate_ok" => "语法校验通过".into(),
        "subrules_validate_failed" => "语法校验未通过".into(),

        // Category 3: Speedtest & Jitter Benchmark
        "speedtest_title" => "节点带宽与抖动率测速".into(),
        "speedtest_btn_start" => "开始真实测速".into(),
        "speedtest_measuring" => "测速中...".into(),
        "speedtest_bandwidth" => "下行带宽".into(),
        "speedtest_jitter" => "网络抖动 (Jitter)".into(),
        "speedtest_packet_loss" => "丢包率".into(),
        "speedtest_stability" => "节点稳定性等级".into(),
        "speedtest_cancel" => "取消测速".into(),
        "speedtest_dead_archive" => "超时/不可用节点归档".into(),
        "speedtest_history_title" => "历史测速记录".into(),
        "speedtest_history_empty" => "暂无历史记录".into(),
        "speedtest_scope_all_groups" => "全部节点".into(),
        "speedtest_scope_group" => "分组".into(),
        "speedtest_scope_node" => "节点".into(),
        "speedtest_history_alive" => "存活".into(),
        "speedtest_history_latency" => "平均延迟".into(),
        "speedtest_history_bandwidth" => "平均带宽".into(),
        "speedtest_history_jitter" => "平均抖动".into(),
        "speedtest_target_url_label" => "测速目标 URL".into(),
        "speedtest_target_url_placeholder" => "留空使用共享引擎默认目标".into(),
        "speedtest_concurrency_label" => "并发数".into(),
        "speedtest_detail_open" => "结果透视".into(),
        "speedtest_detail_title" => "测速结果明细".into(),
        "speedtest_detail_empty" => "暂无测速结果".into(),
        "speedtest_detail_failed" => "测速失败".into(),
        "speedtest_detail_egress" => "出口".into(),
        "speedtest_detail_delay" => "延迟".into(),
        "speedtest_detail_stars" => "星级".into(),
        "speedtest_detail_match" => "归属一致".into(),
        "speedtest_detail_mismatch" => "归属不一致".into(),
        "speedtest_detail_unlabelled" => "无标签国家".into(),
        "speedtest_detail_unknown" => "出口未探测".into(),

        // Category 4: Geo Database Updater
        "geodata_title" => "GeoIP / GeoSite 数据库管理".into(),
        "geodata_btn_check" => "检查在线更新".into(),
        "geodata_btn_update" => "立即增量更新".into(),
        "geodata_geoip_status" => "GeoIP 数据库".into(),
        "geodata_geosite_status" => "GeoSite 数据库".into(),
        "geodata_updated" => "已是最新版本".into(),
        "geodata_updating" => "正在更新数据库...".into(),
        "geodata_version_unknown" => "版本未知".into(),
        "geodata_check_unavailable" => "内核未提供 Geo 数据库版本查询接口，无法核对当前版本".into(),
        "geodata_unsupported_host" => "当前宿主不支持 Geo 数据库更新".into(),
        "geodata_update_triggered" => "已触发 Geo 数据库更新，内核将在后台完成下载".into(),

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
        "net_roam_status_unknown" => "未探测".into(),
        "net_roam_status_stable" => "链路稳定".into(),
        "net_roam_status_recovering" => "正在修复 TUN 路由".into(),
        "net_roam_status_degraded" => "降级".into(),
        "net_roam_status_unsupported" => "宿主不支持".into(),
        "net_roam_status_failed" => "修复失败".into(),
        "net_roam_event_initial" => "已观测".into(),
        "net_roam_event_gateway_changed" => "网关切换".into(),
        "net_roam_event_address_changed" => "地址变化".into(),
        "net_roam_event_routes_repaired" => "路由已修复".into(),
        "net_roam_event_repair_skipped" => "未修复".into(),
        "net_roam_event_repair_failed" => "修复失败".into(),
        "net_roam_active_badge" => "活跃".into(),

        // Category 7: Android VpnService
        "vpn_card_title" => "Android VpnService 与前台保活".into(),
        "vpn_card_desc" => "通过系统 VPN 授权、前台服务和 TUN 隧道保护移动端流量".into(),
        "vpn_start_active" => "VPN 已启动".into(),
        "vpn_start" => "启动 VPN".into(),
        "vpn_stop" => "停止 VPN".into(),
        "vpn_stopped" => "已停止".into(),
        "vpn_status_idle" => "未启动".into(),
        "vpn_status_permission" => "等待授权".into(),
        "vpn_status_starting" => "前台服务启动中".into(),
        "vpn_status_running" => "运行中".into(),
        "vpn_status_stopping" => "停止中".into(),
        "vpn_status_stopped" => "已停止".into(),
        "vpn_status_revoked" => "授权已撤销".into(),
        "vpn_status_unsupported" => "宿主不支持".into(),
        "vpn_status_failed" => "失败".into(),

        // Category 8: Privileged network regression
        "privileged_network_title" => "特权网络无头回归".into(),
        "privileged_network_desc" => "注入、回读并清理宿主特权网络适配器，失败时验证回滚".into(),
        "privileged_network_run" => "运行回归".into(),
        "privileged_network_status_idle" => "未运行".into(),
        "privileged_network_status_injecting" => "注入中".into(),
        "privileged_network_status_active" => "已注入".into(),
        "privileged_network_status_rolling_back" => "回滚清理中".into(),
        "privileged_network_status_cleaned" => "已清理".into(),
        "privileged_network_status_unsupported" => "宿主不支持".into(),
        "privileged_network_status_failed" => "失败".into(),

        "uwp_found_count" => "已发现 {count} 个 UWP AppContainer".into(),

        // Category 2: Crash Watchdog & Forensic Viewer
        "crash_watchdog_title" => "崩溃自愈看门狗与脱敏取证".into(),
        "crash_watchdog_desc" => {
            "监控异常退出与 Panic 状态，提供本地脱敏调用栈分析与孤儿状态恢复".into()
        }
        "crash_watchdog_btn_recover" => "一键恢复网络与清退孤儿状态".into(),
        "crash_watchdog_btn_export" => "导出脱敏取证包".into(),
        "crash_watchdog_clean" => "系统运行正常，未检测到异常退出与孤儿状态".into(),
        "crash_watchdog_recovered" => "已成功清退孤儿状态并恢复系统网络".into(),

        // Category 3: External Web Dashboard
        "web_dash_title" => "外部 Web 仪表盘 (Web Dashboard)".into(),
        "web_dash_desc" => {
            "内置 Metacubexd、Yacd 与 Razord 控制台，免密携带 Token 自动握手拉起".into()
        }
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
        "rule_hit_desc" => {
            "统计当前会话各规则累计命中频次，快速识别并清理 0 次命中的冷门规则".into()
        }
        "rule_hit_btn_audit" => "审计冷门规则".into(),
        "rule_hit_btn_clean" => "一键停用 0 命中规则".into(),
        "rule_hit_total_hits" => "累计总命中次数".into(),
        "rule_hit_stale_count" => "发现 {count} 条冷门规则".into(),
        "rule_hit_btn_clear" => "清空命中计数".into(),
        "rule_hit_dead_count" => "冷门/被遮蔽规则".into(),
        "rule_hit_cidr_conflicts" => "CIDR 掩码重叠".into(),
        "rule_hit_match_latency" => "平均匹配耗时".into(),
        "rule_hit_last_hit" => "最近命中规则".into(),
        "rule_hit_none" => "暂无命中数据".into(),

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
        "provider_unpack_desc" => {
            "将远程 Rule-Provider 规则条目一键解构导入为本地可编辑规则".into()
        }
        "provider_btn_unpack" => "解构 {provider} 为自定义规则".into(),
        "provider_btn_unpack_idle" => "无可解构的规则集".into(),
        "provider_btn_purge_cache" => "清理规则集本地缓存".into(),
        "provider_unpack_total" => "已解构 {count} 条规则".into(),
        "provider_unpack_active" => "规则集生效中".into(),
        "provider_cache_ready" => "{dir} · {count} 个缓存文件 · {bytes} 字节".into(),
        "provider_cache_unsupported" => "宿主未声明规则集缓存目录".into(),
        "provider_cache_failed" => "规则集缓存不可读".into(),
        "provider_cache_unknown" => "规则集缓存状态未知".into(),
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
        "lan_sharing_desc" => {
            "允许局域网设备接入当前代理连接，并严格基于 IP/CIDR 白名单授权".into()
        }
        "lan_sharing_enable" => "开启局域网共享 (Allow LAN)".into(),
        "lan_sharing_port" => "局域网混合代理端口".into(),
        "lan_sharing_bind" => "局域网绑定地址 (Bind Address)".into(),
        "lan_sharing_apply" => "应用局域网监听设置".into(),
        "lan_sharing_acl" => "允许接入的客户端 IP 白名单 (CIDR)".into(),
        "lan_security_title" => "局域网 ACL 与 HTTP 基本认证".into(),
        "lan_security_desc" => {
            "仅允许白名单网段接入，并可为 HTTP/SOCKS/Mixed 代理启用账号认证".into()
        }
        "lan_security_allowed" => "允许网段 (CIDR)".into(),
        "lan_security_disallowed" => "拒绝网段 (CIDR)".into(),
        "lan_security_skip_auth" => "免认证网段 (CIDR)".into(),
        "lan_security_auth" => "HTTP 基本认证".into(),
        "lan_security_username" => "用户名".into(),
        "lan_security_password" => "密码".into(),
        "lan_security_enabled" => "已启用".into(),
        "lan_security_disabled" => "未启用".into(),
        "lan_security_apply" => "应用 ACL 与认证设置".into(),
        "settings_ipv6_routing" => "允许 IPv6 内核流量".into(),
        "settings_ipv6_routing_desc" => {
            "关闭后由 Mihomo 内核拒绝 IPv6 流量，降低双栈旁路泄漏风险".into()
        }
        "common_copy" => "复制".into(),
        "overview_current_ip" => "当前出口 IP".into(),
        "overview_scale_max" => "最大".into(),
        "tracer_src_ip_label" => "来源 IP:".into(),
        "tracer_src_ip_placeholder" => "模拟来源 IP (例如: 192.168.1.100)".into(),
        "tracer_override_label" => "反向应用出站 (修改此规则出站)".into(),
        "tracer_override_placeholder" => {
            "出站目标或策略组名称 (例如: PROXY / DIRECT / REJECT)".into()
        }
        "tracer_override_apply" => "修改此规则出站".into(),
        "profiles_user_agent_placeholder" => {
            "User-Agent (例如 Clash.Meta / ClashVerge / Shadowrocket)".into()
        }
        "profiles_insecure_skip_verify" => "跳过 TLS 证书校验 (Insecure Skip Verify)".into(),
        "profiles_insecure_skip_verify_hint" => {
            "仅对当前订阅生效；跳过证书校验会降低安全性，请仅在自签证书源站使用".into()
        }
        "profiles_auto_reload_core" => "更新后自动重载内核 (Auto Reload Core)".into(),
        "profiles_auto_reload_core_hint" => {
            "仅对当前生效配置生效；开启后在订阅更新成功后按热重载应用，宿主无重载能力时会明确拒绝"
                .into()
        }
        "sub_reload_core_unsupported" => {
            "此宿主未提供内核重载能力，更新已保存但未应用到运行中的内核".into()
        }
        "sub_reload_core_failed" => "内核重载失败（更新已保存）".into(),
        "profiles_cron_placeholder" => "Cron 表达式 (例如 0 */6 * * *，留空则按小时)".into(),
        "profiles_cron_hint" => {
            "5 段 UTC Cron（分 时 日 月 周），支持 @daily / @hourly 等宏；填写后优先于小时周期"
                .into()
        }
        "profiles_conditional_request" => "条件请求已缓存".into(),
        "profiles_conditional_request_empty" => "条件请求：尚无 ETag / Last-Modified 缓存".into(),
        "sub_update_not_modified" => "订阅未变更 (304 Not Modified)".into(),
        "sub_update_quota_warning" => "配额或到期预警".into(),
        "profiles_update_all" => "一键更新全部订阅".into(),
        "profiles_backup_available" => "安全备份已就绪：保存订阅配置时自动生成 .bak".into(),
        "profiles_backup_none" => "安全备份：暂无（保存订阅配置时自动生成）".into(),
        "profiles_restore_backup" => "还原安全备份".into(),
        "profiles_backup_restored" => "已还原上次写入前的配置备份".into(),
        "profiles_backup_missing" => "没有可还原的安全备份".into(),
        "mrs_accel_title" => "MRS 二进制加速状态".into(),
        "mrs_accel_ready" => "加速就绪".into(),
        "mrs_accel_empty" => "当前配置未声明二进制规则集".into(),
        "mrs_accel_unsupported" => "不受支持".into(),
        "mrs_accel_failed" => "加速失败".into(),
        "mrs_accel_unavailable" => "不可用".into(),
        "mrs_accel_providers" => "个规则集".into(),
        "mrs_accel_rules" => "条规则".into(),
        "mrs_accel_memory_saved" => "节省内存".into(),
        "mrs_accel_mmap_on" => "mmap 已启用".into(),
        "mrs_accel_mmap_off" => "mmap 未启用".into(),
        "mrs_accel_valid" => "校验通过".into(),
        "mrs_accel_invalid" => "校验失败".into(),
        "mrs_accel_no_digest" => "无摘要".into(),
        "snapshot_diff_open" => "对比".into(),
        "snapshot_diff_loading" => "正在计算快照差异...".into(),
        "snapshot_diff_empty" => "该快照与当前配置内容一致，或没有可显示的差异。".into(),
        "snapshot_diff_fidelity" => "保真级别".into(),
        "snapshot_diff_inline" => "行内".into(),
        "snapshot_diff_split" => "并排".into(),
        "snapshot_diff_confirm_hint" => "回滚会通过应用事务覆写当前配置，此操作不可撤销。".into(),
        "snapshot_diff_confirm_btn" => "确认回滚".into(),
        "editor_protection_unlock" => "仍要直接编辑".into(),
        "editor_protection_lock" => "恢复只读保护".into(),
        "editor_protection_use_mixin" => "前往 Mixin 覆写".into(),
        "editor_restore_confirm" => "确认恢复".into(),
        _ => key.to_string().into(),
    }
}
