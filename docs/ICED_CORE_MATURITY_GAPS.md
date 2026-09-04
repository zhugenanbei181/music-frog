# Iced Core Maturity Gaps Ledger (Iced 前端深度成熟度全景台账)

本文档归档 MusicFrog Infiltrator 项目中 `infiltrator-iced` 前端在演进至成熟生产级桌面应用过程中的 4 大核心维度与 11 项深度缺口，并作为全量特性落地的权威交付台账。

> **双端对齐与主纲从属说明（2026-09-03）**：
> 本文档是最高主控台账 [DUAL_SURFACE_PARITY_MASTER_PLAN.md](DUAL_SURFACE_PARITY_MASTER_PLAN.md) 在 `infiltrator-iced` 前端的专属落地执行切片。依据双端同步演进原则，本文档列出的全部特性均在 `infiltrator-bevy-ui` 对应有严格对等的场景实现与无头测试。

---

## 一、4 大维度与 11 项核心缺口全景落地矩阵

| 维度 | 序号 | 核心缺口特性 | 升级目标与 Iced 原生释放路径 | 落地状态与代码模块 |
| :--- | :--- | :--- | :--- | :--- |
| **维度一：开发体验、扩展脚本与全球化生态** | 1 | Monaco 级 YAML/LSP 编辑器与智能感知 | 毫秒级 YAML 语法诊断、行号报错标记、结构化代码片段（SS/Vmess/Trojan/Hy2/Group/Rule）一键插入与格式化。 | **已落地**<br>`src/view/editor.rs` |
| | 2 | **★ 改进六：JS / TS 社区扩展脚本生态 (QuickJS 沙箱)** | 内嵌 QuickJS 脚本调试控制台，支持预设模板（国家分组、流媒体、直连）、<500ms 超时/64MB 内存熔断、`console.log` 捕获与 YAML 实时变换预览。 | **已落地**<br>`src/view/script_console.rs`<br>`src/update/ui.rs` |
| | 3 | **★ 全域强制 i18n 治理（100% 词条抽离 + 静态漏译守卫）** | 全仓 100% 词条统一由 `locales_table` 纳管，开发 `scripts/quality/i18n-guard.py` 静态门禁，双语严格对齐，零中文裸字面量。 | **已落地**<br>`scripts/quality/i18n-guard.py`<br>`infiltrator-shared` |
| **维度二：核心遥测、交互透视与链路诊断** | 4 | 连接审计的“深度链路透视”与 Slide-out 抽屉下钻 | 侧边浮层抽屉下钻，展示单连接生命周期（DNS、TCP、TLS、TTFB 耗时瀑布流）、目标 ASN、IP 与一键规则绑定。 | **已落地**<br>`src/view_root/connection_drawer.rs` |
| | 5 | **★ 改进一：可视化交互式分流追踪器 (Live Rule Tracer 沙盒)** | 挂载 Tracer 探针，支持输入域名/IP/端口/进程实时模拟分流链路，树状回放命中规则序号、表达式与出站策略决策。 | **已落地**<br>`src/view/rules_tracer.rs`<br>`src/view/rules.rs` |
| | 6 | **★ 改进二：应用级分流图形化网格 (Per-App Process Grid)** | 进程枚举与分类（Browser/Dev/Game/Media/System）、图标提取、搜索过滤，提供直连/代理/拦截单进程快捷绑定。 | **已落地**<br>`src/view/app_routing.rs`<br>`src/types/app_routing.rs` |
| **维度三：Iced 原生性能与图形渲染潜力** | 7 | **★ 改进三：真正的高性能虚拟列表 (Virtual Viewport Scrolling)** | $O(1)$ 视口几何裁剪算法，计算上下填充高度与可视窗口，轻松承载 50,000+ 规则与连接实时滚动，维持 60 FPS。 | **已落地**<br>`src/view/virtual_list.rs` |
| | 8 | GPU 硬件加速 Canvas 遥测渲染与动效波形 | 基于 WGPU Canvas Program 构建双通道发光渐变折线网速波形、侧栏 Mini Sparkline 与平滑插值。 | **已落地**<br>`src/view/waveform.rs` |
| | 9 | **★ 改进四：代理策略矩阵的顺序微调重排与骨架屏微动效** | 策略组支持上移/下移自定义优先级排序与一键重置；在节点测速与加载态呈现优雅的动态脉冲骨架屏占位。 | **已落地**<br>`src/view/proxies.rs`<br>`src/update/ui.rs` |
| **维度四：桌面沉浸体验与原生生产力** | 10 | **★ 改进五：操作系统原生沉浸与独立桌面网速迷你悬浮窗** | 紧凑型极简悬浮小窗 (260x90)，展示上下行双向速率、微波形图、当前活动节点、窗口置顶与快捷系统代理/TUN 切换。 | **已落地**<br>`src/view/mini_hud.rs`<br>`src/view_root.rs` |
| | 11 | 全局 Command Palette (`Ctrl+K`) 与全键盘极客流 | 按下 `Ctrl+K` 唤出居中浮层，拼音首字母模糊搜索节点、一键切换模式、清空 DNS、开启 TUN 与导航跳转。 | **已落地**<br>`src/view_root/command_palette.rs` |

---

## 二、六类新改进技术细节与测试契约

1. **类别一：Live Rule Tracer 沙盒 (`src/view/rules_tracer.rs`)**
   - 算法：下沉调用 `infiltrator-core::rules::tracer::trace_rules` 与 `TrafficContext::from_query`。
   - 验证：`tests/gui/iced_six_advancements_tests.rs::test_advancement_1_live_rule_tracer_contract`。
2. **类别二：应用级分流网格 (`src/view/app_routing.rs`)**
   - 算法：基于 `infiltrator-desktop::process_enumerator`，支持全局/白名单/黑名单三态切换与按类别过滤。
   - 验证：`tests/gui/iced_six_advancements_tests.rs::test_advancement_2_app_routing_grid_state_and_transitions`。
3. **类别三：高性能虚拟视口滚动引擎 (`src/view/virtual_list.rs`)**
   - 算法：$O(1)$ 数学几何投影，`top_spacer + bottom_spacer + rendered_height == total_content_height` 不变量。
   - 验证：`tests/gui/iced_six_advancements_tests.rs::test_advancement_3_virtual_viewport_scrolling_engine`。
4. **类别四：策略组微调重排与骨架屏 (`src/view/proxies.rs`)**
   - 算法：`proxy_group_order` 优先级重排与复位，加载与测速态挂载 `skeleton_box` 占位。
   - 验证：`tests/gui/iced_six_advancements_tests.rs::test_advancement_4_proxy_group_reordering_and_reset`。
5. **类别五：桌面网速迷你悬浮窗 (`src/view/mini_hud.rs`)**
   - 算法：极简无干扰桌面悬浮小窗，集成实时收发速率双通道 mini sparkline 与置顶模式。
   - 验证：`tests/gui/iced_six_advancements_tests.rs::test_advancement_5_mini_hud_mode_and_always_on_top`。
6. **类别六：QuickJS 扩展脚本沙箱控制台 (`src/view/script_console.rs`)**
   - 算法：`infiltrator-core::script_engine::ScriptEngine` 沙箱隔离执行，AST 自动变换，日志拦截。
   - 验证：`tests/gui/iced_six_advancements_tests.rs::test_advancement_6_quickjs_script_sandbox_console_lifecycle`。

---

## 三、第二批（Wave 2）6 类深度进阶改进矩阵与测试契约

| 序号 | 特性分类 | 核心业务价值与交互形态 | 落地模块 | 对应测试验证点 |
| :--- | :--- | :--- | :--- | :--- |
| **W2-01** | **DNS 泄漏多源交叉探测与公网 IP 隐私检测** | 多源并发检测出口公网 IP、地理位置与 ISP 运营商，结合伪随机子域检测 DNS 污染与出口泄露。 | `src/view/dns.rs`<br>`src/types/dns.rs` | `tests/gui/iced_six_advancements_wave2_tests.rs`<br>`::test_advancement_w2_1_dns_leak_privacy_probe_lifecycle` |
| **W2-02** | **自定义节点表单与通用 URI 编解码导入导出** | 表单化录入个人 VPS 节点（Vless Reality, SS, Hy2, Trojan），支持一键解析链接与导出标准节点分享 URI。 | `src/view_root/custom_node_modal.rs`<br>`src/update/ui.rs` | `tests/gui/iced_six_advancements_wave2_tests.rs`<br>`::test_advancement_w2_2_custom_node_modal_and_uri_codec` |
| **W2-03** | **多订阅配置聚合器与策略组自动拓扑生成** | 勾选多个订阅配置，自动去重节点并按国家地区生成自动测速与分流策略组（香港/日本/美国等），合并为新 Profile。 | `src/view_root/aggregator_modal.rs`<br>`src/update/ui.rs` | `tests/gui/iced_six_advancements_wave2_tests.rs`<br>`::test_advancement_w2_3_multi_profile_aggregator_workflow` |
| **W2-04** | **连接审计多维聚合与一键规则生成器** | 支持实时流 (Flat)、按进程 (ByProcess)、按域名 (ByHost) 聚合统计，并在详情抽屉支持一键将域名/IP 添加到分流规则。 | `src/view/runtime/connections.rs`<br>`src/view_root/connection_drawer.rs` | `tests/gui/iced_six_advancements_wave2_tests.rs`<br>`::test_advancement_w2_4_connection_grouping_and_quick_rule` |
| **W2-05** | **配置历史快照可视化 Diff 比对与安全回滚** | 可视化并排/行内呈现历史快照与当前配置的逐行 YAML / 节点差异高亮（Added/Removed/Modified），支持一键还原。 | `src/view_root/snapshot_diff_modal.rs`<br>`src/update/ui.rs` | `tests/gui/iced_six_advancements_wave2_tests.rs`<br>`::test_advancement_w2_5_snapshot_diff_and_rollback_dialog` |
| **W2-06** | **桌面全局快捷键管理与按键冲突规避面板** | 在系统设置面板可视化捕获和配置系统全局快捷键（切换系统代理、开闭 TUN、唤起迷你悬浮窗），支持单项启用与禁用。 | `src/view/settings.rs`<br>`src/types/app.rs` | `tests/gui/iced_six_advancements_wave2_tests.rs`<br>`::test_advancement_w2_6_global_hotkey_manager_state` |

---

## 四、第三批（Wave 3）6 类深度进阶改进矩阵与测试契约

| 序号 | 特性分类 | 核心业务价值与交互形态 | 落地模块 | 对应测试验证点 |
| :--- | :--- | :--- | :--- | :--- |
| **W3-01** | **PCAP 流量抓包与 Sniffer 审计面板** | 实时网络数据包抓取控制（开始/停止），一键导出 Wireshark 兼容的标准 `.pcap` 流量分析文件，统计包数与捕获体积。 | `src/view/pcap_panel.rs`<br>`src/view/runtime.rs` | `tests/gui/iced_six_advancements_wave3_tests.rs`<br>`::test_advancement_w3_1_pcap_capture_and_export_lifecycle` |
| **W3-02** | **Sub-Rules 复合逻辑分流可视化构建器** | 可视化组装 `AND`, `OR`, `NOT`, `SUB-RULE` 嵌套分流条件，支持快速增删子叶子节点、预览表达式并一键插入分流规则列表。 | `src/view/subrules_builder.rs`<br>`src/view/rules.rs` | `tests/gui/iced_six_advancements_wave3_tests.rs`<br>`::test_advancement_w3_2_subrules_logical_builder_workflow` |
| **W3-03** | **节点真实下行带宽测速与抖动率评估套件** | 对指定代理节点发起多线程真实数据块拉取，测定真实下行带宽（Mbps），结合高频采样计算网络抖动（Jitter ms）与丢包率评级。 | `src/view/speedtest_modal.rs`<br>`src/view/proxies.rs` | `tests/gui/iced_six_advancements_wave3_tests.rs`<br>`::test_advancement_w3_3_speedtest_and_jitter_benchmark_result` |
| **W3-04** | **GeoIP / GeoSite 数据库版本追踪与增量更新面板** | 监控展示当前 `geoip.metadb` 与 `geosite.dat` 本地版本和文件大小，支持在线检查远端 Release Tag 并触发增量下载与 SHA-256 校验。 | `src/view/geodata_card.rs`<br>`src/view/settings.rs` | `tests/gui/iced_six_advancements_wave3_tests.rs`<br>`::test_advancement_w3_4_geodata_version_and_updater_workflow` |
| **W3-05** | **Windows UWP 回环隔离解除管理工具** | 扫描并列举系统 UWP 应用（Microsoft Store, Xbox, Outlook 等），支持全选一键豁免 (Exempt All)、重置隔离及单个应用切换。 | `src/view/uwp_card.rs`<br>`src/view/settings.rs` | `tests/gui/iced_six_advancements_wave3_tests.rs`<br>`::test_advancement_w3_5_uwp_loopback_exemption_manager` |
| **W3-06** | **端到端加密备份包导出/导入器 (.encpkg Crypto Vault)** | 使用主密码基于 AES-256 加密打包当前所有订阅、自定义规则与 Mixin，支持带密导出 `.encpkg` 与从归档包解密导入。 | `src/view/sync.rs`<br>`src/update/ui_wave3.rs` | `tests/gui/iced_six_advancements_wave3_tests.rs`<br>`::test_advancement_w3_6_encrypted_backup_package_lifecycle` |

---

## 五、第四批（Wave 4）6 类深度进阶改进矩阵与测试契约

| 序号 | 特性分类 | 核心业务价值与交互形态 | 落地模块 | 对应测试验证点 |
| :--- | :--- | :--- | :--- | :--- |
| **W4-01** | **网卡漫游与网络切换自动感知/自愈探针** | 实时感知物理出站网卡、默认网关 IP 与最佳自适应 MTU，支持一键触发网关重连与 TUN 路由自愈。 | `src/view/net_roam_card.rs`<br>`src/view/settings.rs` | `tests/gui/iced_six_advancements_wave4_tests.rs`<br>`::test_advancement_w4_1_network_roaming_and_gateway_recovery` |
| **W4-02** | **崩溃自愈看门狗与事后脱敏取证查看器** | 监控异常退出与 Panic 状态，提供本地脱敏调用栈分析，一键清退孤儿状态并导出本地诊断 JSON。 | `src/view/crash_watchdog_card.rs`<br>`src/view/doctor.rs` | `tests/gui/iced_six_advancements_wave4_tests.rs`<br>`::test_advancement_w4_2_crash_watchdog_and_forensics_lifecycle` |
| **W4-03** | **外部 Web Dashboard 免密握手一键拉起器** | 内置 Metacubexd、Yacd-Meta 与 Razord 控制台卡片，一键拉起浏览器并携带 Secret 凭据完成免密连入。 | `src/view/web_dash_card.rs`<br>`src/view/settings.rs` | `tests/gui/iced_six_advancements_wave4_tests.rs`<br>`::test_advancement_w4_3_web_dashboard_launch_dispatch` |
| **W4-04** | **日志高级正则过滤与一键脱敏导出器** | 支持日志流 Regex 关键字高亮过滤与日志级别筛选，支持一键脱敏敏感凭据（Token/Bearer/Secret）导出本地日志。 | `src/view/runtime/logs.rs`<br>`src/update/ui_wave4.rs` | `tests/gui/iced_six_advancements_wave4_tests.rs`<br>`::test_advancement_w4_4_log_regex_and_redacted_export` |
| **W4-05** | **订阅配额与临期智能预警及 Cron 调度矩阵** | 实时解析订阅配额、剩余流量百分比与到期时间戳（三级预警），支持配置自定义定时轮询更新周期（6h/12h/24h）。 | `src/view/sub_quota_card.rs`<br>`src/view/profiles.rs` | `tests/gui/iced_six_advancements_wave4_tests.rs`<br>`::test_advancement_w4_5_subscription_quota_and_cron_matrix` |
| **W4-06** | **PAC 动态代理服务与绕过网段管理器** | 生成浏览器通用的 PAC (Proxy Auto-Config) 脚本，提供本地 PAC 服务 URL，支持用户自定义局域网直连白名单网段并编译验证。 | `src/view/pac_card.rs`<br>`src/view/settings.rs` | `tests/gui/iced_six_advancements_wave4_tests.rs`<br>`::test_advancement_w4_6_pac_auto_proxy_and_bypass_manager` |

---

## 六、第五批（Wave 5）6 类深度进阶改进矩阵与测试契约

| 序号 | 特性分类 | 核心业务价值与交互形态 | 落地模块 | 对应测试验证点 |
| :--- | :--- | :--- | :--- | :--- |
| **W5-01** | **分流规则命中统计与冷门僵尸规则清理** | 会话级规则命中频次统计，智能识别并展示 0 次命中冷门规则，支持一键快速停用以精简内核判定开销。 | `src/view/rule_hit_card.rs`<br>`src/view/rules.rs` | `tests/gui/iced_six_advancements_wave5_tests.rs`<br>`::test_advancement_w5_1_rule_hit_counter_and_stale_analyzer` |
| **W5-02** | **节点时序延迟走势图与多维稳定性雷达** | 多点时序采样分析选中节点的往返延迟 (RTT) 波动区间 (Min/Max)、平均延迟与网络抖动，计算五星稳定性等级。 | `src/view/latency_radar_card.rs`<br>`src/view/proxies.rs` | `tests/gui/iced_six_advancements_wave5_tests.rs`<br>`::test_advancement_w5_2_latency_time_series_and_stability_radar` |
| **W5-03** | **TUN 虚拟网卡多堆栈与 MTU 自适应协商** | 自由切换 gVisor（用户态沙盒）、System（原生内核高性能）与 Mixed（混合）驱动堆栈，提供物理 MTU 动态探测。 | `src/view/tun_stack_card.rs`<br>`src/view/dns.rs` | `tests/gui/iced_six_advancements_wave5_tests.rs`<br>`::test_advancement_w5_3_tun_multi_stack_and_mtu_negotiation` |
| **W5-04** | **规则集解构提取与本地规则转换器** | 将远程 Rule-Provider 规则条目一键解构导入为本地可编辑规则，并支持本地磁盘缓存一键清理以释放磁盘空间。 | `src/view/provider_unpack_card.rs`<br>`src/view/rules.rs` | `tests/gui/iced_six_advancements_wave5_tests.rs`<br>`::test_advancement_w5_4_rule_provider_lifecycle_and_unpack` |
| **W5-05** | **配置生效多阶段原子事务与回滚守卫** | 预检语法 -> 暂存配置 -> 核心热载 -> 健康探活 -> 提交，在网络探活失败或配置无效时自动触发原子级安全回滚。 | `src/view/apply_guard_card.rs`<br>`src/view/settings.rs` | `tests/gui/iced_six_advancements_wave5_tests.rs`<br>`::test_advancement_w5_5_config_apply_atomic_transaction_guard` |
| **W5-06** | **局域网共享代理与访问控制列表 (ACL)** | 开启局域网设备接入共享代理 (Allow LAN)，自定义混合监听端口，并提供基于 IP/CIDR 白名单的严格访问鉴权控制。 | `src/view/lan_sharing_card.rs`<br>`src/view/settings.rs` | `tests/gui/iced_six_advancements_wave5_tests.rs`<br>`::test_advancement_w5_6_lan_proxy_sharing_and_access_acl` |
