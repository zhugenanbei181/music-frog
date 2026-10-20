# Slint 表面契约

> 层级：L3

`infiltrator-slint` 是 MusicFrog 的第三套 UI。`ui/lumen-app.slint` 导出 Rust adapter 使用的 `AppWindow` 与 Lumen 组件树；Rust adapter 负责把 `SurfaceSnapshot` 投影到控件，把用户输入转成共享 `CommandIntent`，并把 owner 的 typed failure 显示出来。Slint 不创建 Core、不持有独立业务仓库，也不在 Android 侧创建第二份 application。

桌面由 `DesktopProductSession` 组合既有 application、Core 与系统 adapter。Android flavor 通过 `infiltrator-android` 的产品 IPC 提交相同 `CommandIntent`，快照由 Android 唯一 owner 发布；NativeActivity 只初始化 Slint renderer 并挂载页面。

同一组件根据可用宽度响应式切换：Android 恒用 compact 布局，桌面窗口低于 `760px × UI scale` 也切换为紧凑导航和单列内容；较宽桌面保留侧边导航和更宽的内容区。控件尺寸随页面缩放因子调整，旗帜、图标、窗口图标、字体及其 license provenance 由共享资源目录提供。布局差异只属于呈现，不改变命令语义。

页面读取 owner 快照：

- 首页投影 Core 生命周期和错误、当前 Profile、可验证的活动出口与国家旗帜、公共 IP 探测结果、连接数和实时流量。实时速率用 `infiltrator-presentation` 的共享 projector；未知、未观测和失败状态不被填成演示值。
- 代理页展示 typed outbound inventory/details，支持组内选择、搜索、排序、单节点测速和整组测速。选择与探测请求携带 owner 快照 revision，避免把过期列表提交给 Core。
- 配置页读取共享 Profile 列表，可切换、刷新、删除订阅并导入 URL。订阅 URL、更新间隔和 User-Agent 使用共享命令；只读预览从 `LoadProfileDocument` 的共享读回显示，并通过 `infiltrator-presentation::profile_preview` 结构化遮蔽凭据字段。Age 凭据读取、设置、清除与加密订阅导入经由共享 Core owner capability / `DesktopProductSession` 执行，密钥仅限输入阶段且自动擦除，不进入 UI state/log；配置重命名、本地文件导入与配置/覆盖/日志导出已接入底层命令与平台文件适配器。
- 网络页提交 Rule/Global/Direct 路由意图和 desktop 系统代理/TUN 或 Android VPN 命令。TUN 开关以宿主 `Capability` 为准；网络模式切换序列会逐条使用 owner 返回的新 Core scope。
- 连接页筛选 owner 发布的活动连接，可查看当前快照中的端点、规则、链路、开始时间和累计字节，也可请求关闭连接。owner 未发布已关闭连接历史，因此“已关闭”切换会被还原并显示原因。
- 日志页由共享 owner 提供日志流和等级过滤，关键字搜索在 Slint adapter 中作用于当前已发布窗口。规则启停使用 owner 发布的 rule index，Provider 更新提交共享 batch 命令。
- 流量页展示共享实时采样；今日/周累计历史和清理入口在 owner 未发布历史存储前显示未知值，不伪造统计。
- 应用设置写入共享语言和 theme preference；Slint 运行时根据同一个 product gettext catalog 切换界面文案。桌面自启动复用共享平台 helper；自定义强调色与启动时刷新配置写入共享 settings 契约。
- 关于/许可证页面读取随应用发布的第三方依赖许可证说明。Lumen 来源信息如实说明上游 README 的 MIT 声明以及导入 ZIP 缺少独立 LICENSE 文件。

共享快照明确发布上限的规则、活动连接和日志窗口会在对应页面显示本地化的已显示数/总数提示；本地搜索只缩小已发布窗口，不声称检索未发布内容。

配置目录排序、命名 Override 绑定、文件导入导出、流量历史清理与强调色设置已全部接入底层 owner 与 typed 命令；仍未接入特定平台能力的入口（如无二维码扫描器的宿主）会返回可见的 typed unsupported 状态，不伪装成功。用户配置名、节点名、日志正文等 runtime data 按原文呈现，不走翻译表。

## 截图取证、测试矩阵与组件树架构

1. **后台无感实机截图取证（Capture Matrix）**：
   - 支持通过 `--demo` 命令行标志或 `INFILTRATOR_DEMO=1`、`INFILTRATOR_PAGE`、`INFILTRATOR_SKIN`、`INFILTRATOR_WINDOW_SIZE` 环境变量激活演示与截图模式；
   - 窗口挂载与稳定首帧绘制后向 `INFILTRATOR_CAPTURE_MARKER` 写入标准 `CAPTURE_READY page=<p> skin=<s>\n` 标记；
   - 配合 `scripts/capture-slint-matrix.sh` 与 `scripts/capture_slint_scenarios.tsv`，在私有虚拟 Wayland 合成器（KWin + Niri IPC）内实现无弹窗、不夺焦点、绑定二进制与源码指纹的自动化像素级取证。

2. **平价场景取证（Parity Scenario Capture）**：
   - `INFILTRATOR_SCENARIO=<feature>` 不再只是把场景名写进标记：`infiltrator_slint::capture_scenario::activate` 会让 surface 打开该业务真实对应的界面状态（与用户手势走同一条路径），`infiltrator_slint::capture_scenario::bounds` 再读取该界面在窗口中的真实几何；
   - surface 能承载的业务集合是**封闭且带类型**的：`infiltrator_slint::capture_scenario::CarriedScenario` 是唯一真源，`activate` 只认识这些变体；集合之外的业务名一律返回 `false`，标记因此永远不会为 surface 没有的业务背书；
   - 标记格式为 `CAPTURE_READY page=<p> skin=<s> scenario=<feature> activated=true bounds=<x>,<y>,<w>,<h>\n`。`bounds` 是被激活界面在窗口中的真实像素矩形，surface 未打开该界面时写 `activated=false` 且不带 `bounds`，因此截图链路无法为 surface 未实现的业务背书；
   - 视口沿用平价约定：`<feature>-standard` 为 1180×780，`<feature>-compact` 为 720×480；场景矩阵见 `docs/capture/parity_scenarios.tsv`，已发布的像素回执见 `docs/screenshots/parity/`；
   - 单个业务的取证命令：`INFILTRATOR_CAPTURE_MATRIX=crates/infiltrator-slint/docs/capture/parity_scenarios.tsv bash scripts/capture-slint-matrix.sh <feature>-standard,<feature>-compact`；
   - 全部 18 个承载业务的取证命令：`INFILTRATOR_CAPTURE_MATRIX=crates/infiltrator-slint/docs/capture/parity_scenarios.tsv INFILTRATOR_CAPTURE_OUT_DIR=crates/infiltrator-slint/docs/screenshots/parity bash scripts/capture-slint-matrix.sh`。

3. **行为测试矩阵覆盖**：
   - 所有行为用例都通过 `infiltrator_slint::surface::harness::SurfaceHarness` 驱动**真实 Slint 回调**：harness 在无头软件渲染平台上注册生产回调、派发真实手势，并回报 surface 实际发出的 typed `CommandIntent`、typed `Failure` 与 owner 可见投影（notice、列表行、待定状态）。测试不再本地复刻路由映射、网络模式时序、主题白名单、十六进制解析或连接排序；
   - `infiltrator_slint::surface::action` 是这些行为的唯一实现：surface 回调与测试调用的是同一函数，测试无法与运行中的产品产生分叉；
   - `core_and_network_mode_tests`：覆盖路由意图分发、网络模式切换序列（系统代理/TUN/仅核心切换时的停机重启时序）、未知模式与缺失快照的 typed failure，以及 Core 生命周期投影标签；
   - `workbench_path_tests` / `workbench_service_path_tests`：覆盖 workbench 面板的**每一条交互路径**——取消（不发命令、丢弃草稿）、owner 拒绝（typed failure 落到共享 reducer 的相位机）、重试（共享 reducer 用同一身份重发）、pending（命令在途时拒绝第二个命令）、stale（外来读回不覆盖当前事实）、empty/unsupported（共享快照的诚实空态与 typed 不支持）、migration（Hosts 旧格式迁移）、presets（内置脚本预设）、locale（语言切换后业务事实不变）、refresh（界面请求 owner 重新读取）；
   - `outbound_and_probe_tests`：覆盖代理组节点选择的 revision 强校验、单节点测速与整组批量测速（concurrency=8）意图、排序与搜索偏好分发，以及未知组/节点与探测设置缺失的 typed failure；
   - `connections_and_logs_tests`：覆盖指定连接中断命令分发、活动与已关闭连接的四维排序投影、日志级别过滤投影、发布窗口截断告警，以及**共享页面模型**的两条差分断言——日志查询按共享正则折叠（`api\.example|timeout` 命中「交替」与「转义点」，非法模式 `(` 命中零行，plain `contains` 实现无法通过），连接行序等于同一共享键的共享归约结果，控件词表 `"time"`→`LatestDesc`、`"host"`→`HostAsc`；
   - `rules_and_settings_tests`：覆盖规则开关幂等分发与待定行投影、Rule Provider 批量刷新、语言偏好解析、主题模式白名单、强调色合法性与反差度判定；
   - `profile_scenario_tests`：覆盖平价场景锚点的 surface 行为——添加配置弹层的成功导入、取消与非法 Age 私钥 typed failure，删除配置确认的成功分发、取消与 owner 拒绝的 typed failure 投影，以及取证场景确实打开对应弹层；
   - `capture_scenario_tests`：覆盖取证场景集合——18 个承载业务逐个真的打开自己的载体页/弹层并投影出业务面板，集合之外的业务一律拒绝且不改变 surface 状态，取证矩阵表与 `CarriedScenario` 集合逐条一致；
   - `capture_marker_tests`：覆盖取证标记只在界面真的打开时写 `activated=true` 与真实 `bounds`；
   - `platform_adapter_tests` / `profile_catalog_action_tests` / `age_credential_tests` / `traffic_history_tests`：覆盖平台权限、窗口生命周期、配置管理与敏感凭据。

4. **平价场景锚点（Scenario Anchor）判定**：
   - 一个业务只有在该 surface 上真实实现了 `required_paths` 里的每一条交互路径时，才能登记 scenario 锚点；partial 实现不构成锚点；
   - 锚点测试必须驱动真实 surface 回调并断言 typed owner 命令与 owner 可见投影，且 nextest id 必须可在 `cargo nextest list` 中发现；
   - 像素回执必须由上面的平价场景取证链路产生，标准与紧凑两个视口都要有真实像素；
   - **owner 终态回灌**：workbench 面板派发的每条共享命令都由 `infiltrator_slint::surface::workbenches::Workbenches::record` 记账，owner 的 typed 终态（`Result<CommandOutput, Failure>`）由 `Workbenches::settle` 交给**发起该命令的那一个共享 reducer**，因此 `error`/`retry` 路径由共享 reducer 的真实相位机判定，而不是由界面猜测；
   - **owner 协议携带 typed 读回**：`SlintProductHost::execute_typed` 返回共享 `CommandOutput`，使带 owner 身份的读回（导出评审 `ScriptExportReview`、快照恢复评审 `SnapshotRestoreReview`、订阅过滤回执 `SubscriptionFilterApplied`）可以落到共享 reducer；不携带 typed 读回的宿主（例如 Android 宿主）继续使用默认实现，界面据此知道该业务只能到达命令派发这一步；
   - 当前 Slint surface 已实现并可锚点的业务与路径：
     - `dns-leak-alert`（`success`,`error`,`unsupported`）、`dns-query-details`（`success`,`cancel`,`error`,`retry`,`unsupported`,`stale`,`empty`）、`dns-fake-ip-flush-confirm`（`success`,`cancel`,`error`,`retry`,`unsupported`,`stale`）、`dns-hosts-editor`（`success`,`cancel`,`error`,`retry`,`migration`,`stale`,`empty`,`unsupported`）；
     - `profiles-script-sandbox`（`unknown`,`success`,`error`,`retry`,`pending`,`clear`,`presets`,`locale`）、`profiles-script-export-review`（`success`,`cancel`,`error`,`retry`,`pending`,`unsupported`,`locale`）、`profiles-diff-viewer`（`success`,`close`）、`profiles-snapshot-restore-confirm`（`success`,`cancel`,`error`）；
     - `profiles-filter-editor`（`success`,`cancel`,`error`）、`profiles-aggregator-wizard`（`success`,`cancel`,`error`）、`profiles-aggregator-preview`（`success`,`cancel`,`error`）；
     - `doctor-failure-recovery`（`success`,`error`,`retry`）、`lifecycle-recovery-panel`（`success`,`error`,`retry`）；
     - `sync-conflict-resolver`（`success`,`cancel`,`error`）、`sync-restore-confirm`（`success`,`cancel`,`error`）、`app-routing-policy-editor`（`success`,`cancel`,`error`,`refresh`,`unsupported`）；
     - `profiles-import-modal`（`success`,`cancel`,`error`）与 `profiles-delete-confirm`（`success`,`cancel`,`error`）。
   - 行为测试位于 `tests/workbench_page_tests.rs`（首批行为）、`tests/workbench_path_tests.rs`（DNS/编辑器/过滤业务的交互路径）与 `tests/workbench_service_path_tests.rs`（诊断/同步/路由业务的交互路径）；共享夹具位于 `tests/workbench_paths/fixture.rs`。

5. **Slint surface 承载的业务（scenario 名称）**：
   - 承载集合由 `infiltrator_slint::capture_scenario::CarriedScenario` 唯一声明，`docs/capture/parity_scenarios.tsv` 按 `<feature>-standard` / `<feature>-compact` 两视口逐条列出；
   - **DNS 页**（`Dns`）：`dns-leak-alert`、`dns-query-details`、`dns-fake-ip-flush-confirm`、`dns-hosts-editor`；
   - **编辑器页**（`Editor`）：`profiles-script-sandbox`、`profiles-script-export-review`、`profiles-diff-viewer`、`profiles-snapshot-restore-confirm`；
   - **过滤/聚合页**（`Filters`）：`profiles-filter-editor`、`profiles-aggregator-wizard`、`profiles-aggregator-preview`；
   - **诊断页**（`Doctor`）：`doctor-failure-recovery`、`lifecycle-recovery-panel`；
   - **同步页**（`Sync`）：`sync-conflict-resolver`、`sync-restore-confirm`；
   - **应用路由页**（`AppRouting`）：`app-routing-policy-editor`；
   - **配置弹层**：`profiles-import-modal`、`profiles-delete-confirm`。

6. **在 Slint surface 上仍无承载载体的业务**：
   - 下列业务在共享平价目录中存在，但 Slint surface 没有承载它们的界面状态，取证路径会拒绝它们，因此它们没有像素回执，也不能登记 Slint 的 scenario 锚点：
   - `connections-close-all-confirm`、`connections-details-drawer`、`connections-grouping`、`connections-search-highlight`、`core-version-rollback-confirm`、`editor-large-document-window`、`editor-snippet-insertion`、`editor-yaml-error-navigation`、`logs-redacted-export`、`logs-scroll-lock`、`logs-search-highlight`、`overview-card-reorder`、`overview-reconnect-mask`、`overview-topology-drilldown`、`profiles-subscription-error`、`proxies-custom-node-modal`、`proxies-group-expanded`、`proxies-group-reorder`、`proxies-node-detail-drawer`、`proxies-probe-settings`、`proxies-search-highlight`、`proxies-uri-import-preview`、`rules-list-editor`、`rules-override-editor`、`rules-provider-refresh-error`、`rules-search-highlight`、`rules-statistics-inspector`、`rules-tracer-drawer`、`runtime-telemetry-observation`、`settings-language-choice`、`settings-lan-security`、`settings-permission-panel`、`settings-port-conflict-repair`、`settings-tun-permission`、`shell-command-palette`、`shell-keyboard-focus`、`shell-routing-intent-authentication`、`shell-routing-intent-control`、`speedtest-details-modal`、`speedtest-history-comparison`、`speedtest-progress-cancel`；
   - 缺口随共享平价登记表 `scripts/parity/feature_evidence.tsv` 与 `scripts/parity/cross_surface_manifest.tsv` 演进，本文档只声明「Slint 是否有承载载体」这一事实。


3. **Lumen 组件树模块化与质量规范**：
   - 彻底打破单文件巨石结构，建立清晰的分层模块（每个文件严格 $\le 600$ 行）：
     - `ui/components/theme.slint`：全局 `LumenTheme` 主题系统与语义颜色；
     - `ui/components/models.slint`：跨端结构体定义；
     - `ui/components/base_widgets.slint`：基础交互原子组件（按钮、开关、滑块、输入框等）；
     - `ui/components/cards.slint`：领域卡片与容器；
     - `ui/components/sidebar.slint`：桌面侧边导航栏；
     - `ui/components/profile_sheets.slint` 与 `profile_overlays.slint`：配置添加、操作、分享、删除与设置抽屉；
     - `ui/components/settings_sheets.slint` 与 `settings_overlays.slint`：配色、缩放、User-Agent 抽屉与网络接管切换确认弹窗；
     - `ui/components/app_chrome.slint`：移动端悬浮底栏、清空流量弹窗、通知/消息气泡与节点排序菜单；
     - `ui/components/app_body.slint`：桌面/移动端弹性主体容器与滚动锚定；
     - `ui/pages/page_*.slint` 与 `pages_*.slint`：分领域页面与路由集群；
     - `ui/lumen-app.slint`：轻量级主页面根骨架（从 3587 行解耦并缩减至 599 行）；
   - 所有触控交互控件均遵循移动端 $\ge 48\text{px} \times \text{LumenUiScale.factor}$ 最小触控热区规范；
   - 由 `scripts/quality/slint-syntax-guard.py` 门禁执行单文件 $\le 600$ 行预算与严禁硬编码状态色约束（100% 通过）。

