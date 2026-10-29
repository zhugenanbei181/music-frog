# Slint 表面契约

> 层级：L3

`infiltrator-slint` 是 MusicFrog 的第三套 UI。`ui/lumen-app.slint` 导出 Rust adapter 使用的 `AppWindow` 与 Lumen 组件树；Rust adapter 负责把 `SurfaceSnapshot` 投影到控件，把用户输入转成共享 `CommandIntent`，并把 owner 的 typed failure 显示出来。Slint 不创建 Core、不持有独立业务仓库，也不在 Android 侧创建第二份 application。

桌面由 `DesktopProductSession` 组合既有 application、Core 与系统 adapter。Android flavor 通过 `infiltrator-android` 的产品 IPC 提交相同 `CommandIntent`，快照由 Android 唯一 owner 发布；NativeActivity 只初始化 Slint renderer 并挂载页面。

同一组件根据可用宽度响应式切换：Android 恒用 compact 布局，桌面根据契约响应式视口阶梯切换（Compact `< 600px` 紧凑底栏导航与单列内容，Medium `600–839px` 导航栏，Expanded `840–1199px` 标准侧栏，Ultra `≥ 1200px` 宽屏展示）；较宽桌面保留侧边导航和更宽的内容区。控件尺寸随页面缩放因子调整，旗帜、图标、窗口图标、字体及其 license provenance 由共享资源目录提供。布局差异只属于呈现，不改变命令语义。

页面读取 owner 快照：

- 首页投影 Core 生命周期和错误、当前 Profile、可验证的活动出口与国家旗帜、公共 IP 探测结果、连接数和实时流量。实时速率用 `infiltrator-presentation` 的共享 projector；未知、未观测和失败状态不被填成演示值。
- 代理页展示 typed outbound inventory/details，支持组内选择、搜索、排序、单节点测速和整组测速。选择与探测请求携带 owner 快照 revision，避免把过期列表提交给 Core。
- 配置页读取共享 Profile 列表，可切换、刷新、删除订阅并导入 URL。订阅 URL、更新间隔和 User-Agent 使用共享命令；只读预览从 `LoadProfileDocument` 的共享读回显示，并通过 `infiltrator-presentation::profile_preview` 结构化遮蔽凭据字段。Age 凭据读取、设置、清除与加密订阅导入经由共享 Core owner capability / `DesktopProductSession` 执行，密钥仅限输入阶段且自动擦除，不进入 UI state/log；配置重命名、本地文件导入与配置/覆盖/日志导出已接入底层命令与平台文件适配器。
- 网络页提交 Rule/Global/Direct 路由意图和 desktop 系统代理/TUN 或 Android VPN 命令。TUN 开关以宿主 `Capability` 为准；网络模式切换序列会逐条使用 owner 返回的新 Core scope。
- 连接页筛选 owner 发布的活动连接，可查看当前快照中的端点、规则、链路、开始时间和累计字节，也可请求关闭连接。owner 未发布已关闭连接历史，因此“已关闭”切换会被还原并显示原因。
- 日志页由共享 owner 提供日志流和等级过滤，关键字搜索在 Slint adapter 中作用于当前已发布窗口。规则启停使用 owner 发布的 rule index，Provider 更新提交共享 batch 命令。
- 流量页展示共享实时采样；今日/周累计历史和清理入口在 owner 未发布历史存储前显示未知值，不伪造统计。
- 应用分流页 (`PageAppRouting`) 支持分流策略模式（全局 / 仅代理选中 / 选中直连）三段式切换，展示进程规则列表并派发规则与确认意图。
- 自愈诊断页 (`PageDoctor`) 展示诊断套件报告列表、异常等级与修复建议，支持一键体检与批量修复，并展示 Watchdog 看门狗生命周期。
- DNS 页 (`PageDns`) 承载 DNS 泄漏交叉探测、实时域名解析查询与重试、Fake-IP 缓存确认清理及 Hosts 静态条目编辑器。
- 同步页 (`PageSync`) 承载 WebDAV 远端同步、3-Way 差异冲突解决（保留本地/采用远端）及本地备份与快照恢复。
- 编辑器页 (`PageEditor`) 提供脚本指令 DSL 沙箱控制台与内置预设、YAML/AST 统一差异比对及快照历史还原评审。
- 过滤与聚合页 (`PageFilters`) 提供订阅规则过滤器（包含/排除/重命名）与配置聚合合并向导。
- 应用设置写入共享语言和 theme preference；Slint 运行时根据同一个 product gettext catalog 切换界面文案。桌面自启动复用共享平台 helper；自定义强调色与启动时刷新配置写入共享 settings 契约。
- 关于/许可证页面读取随应用发布的第三方依赖许可证说明，全面迁移为产品级词汇展示组件，去除原生 `TextEdit` 等 std 部件。Lumen 来源信息如实说明上游 README 的 MIT 声明以及导入 ZIP 缺少独立 LICENSE 文件。

共享快照明确发布上限的规则、活动连接和日志窗口会在对应页面显示本地化的已显示数/总数提示；本地搜索只缩小已发布窗口，不声称检索未发布内容。

配置目录排序、命名 Override 绑定、文件导入导出、流量历史清理与强调色设置已全部接入底层 owner 与 typed 命令；仍未接入特定平台能力的入口（如无二维码扫描器的宿主）会返回可见的 typed unsupported 状态，不伪装成功。用户配置名、节点名、日志正文等 runtime data 按原文呈现，不走翻译表。

## 截图取证、测试矩阵与组件树架构

1. **后台无感实机截图取证（Capture Matrix）**：
   - 支持通过 `--demo` 命令行标志或 `INFILTRATOR_DEMO=1`、`INFILTRATOR_PAGE`、`INFILTRATOR_SKIN`、`INFILTRATOR_WINDOW_SIZE` 环境变量激活演示与截图模式；
   - 窗口挂载与稳定首帧绘制后向 `INFILTRATOR_CAPTURE_MARKER` 写入标准 `CAPTURE_READY page=<p> skin=<s>\n` 标记；
   - 配合 `scripts/capture-slint-matrix.sh` 与 `scripts/capture_slint_scenarios.tsv`，在私有虚拟 Wayland 合成器（KWin + Niri IPC）内实现无弹窗、不夺焦点、绑定二进制与源码指纹的自动化像素级取证。

2. **平价场景取证（Parity Scenario Capture）**：
   - `INFILTRATOR_SCENARIO=<feature>` 不再只是把场景名写进标记：`infiltrator_slint::capture_scenario::activate` 会让 surface 打开该业务真实对应的界面状态（与用户手势走同一条路径），`infiltrator_slint::capture_scenario::bounds` 再读取该界面在窗口中的真实几何；
   - **激活是业务自己的手势，不是换路由**：`infiltrator_slint::capture_scenario::BusinessStep`（`Type(字段)` / `Press(控件)` / `MoveOverviewCard(卡片, 上移)`）为每个承载业务声明唯一手势序列——业务自己的输入框（`DraftField`：DNS 查询名、Hosts 行、脚本源码、过滤 include、聚合目标名、路由规则）与业务自己的控件（`dns-leak-probe`、`dns-query-run`、`dns-cache-confirm`、`dns-hosts-add`+`dns-hosts-save`、`script-run`、`script-export-prepare`、`diff-load`、`restore-prepare`、`filter-save`、`aggregate-create`、`aggregate-preview`、`doctor-diagnose`、`doctor-bootstrap`、`sync-take-remote`、`sync-restore`、`app-routing-rule`、概览卡片的 `↑`/`↓`）。手势通过 `AppWindow::invoke_workbench_draft_edited` / `invoke_workbench_action` / `invoke_move_overview_card` 触发，与用户点击走同一条回调路径；
   - **激活的判定是渲染真的变了**：`capture_scenario::Activation` 记录手势前的页面签名（workbench 面板的 id/kind/status/detail/trailing/label/enabled + 两个草稿 + owner 可见 notice），只有签名在 owner 终态回灌后真的变化，`activate` 才返回 `true`；owner 命令在途时（`Workbenches::has_pending_command()`）不算激活，因此回执证明的是「业务跑完了并显示了 owner 的答案」，不是「路由换了页」；
   - **演示宿主是真的 owner**：`infiltrator_slint::demo::DemoProductHost` 通过 `run_with_host_prepared` 复用产品接线（注册生产回调、起 owner worker、按 owner 节奏发布快照），并对每条业务命令回一个 typed 拒绝（`infiltrator_slint::demo::owner::respond`），因此取证里出现的是业务自己的失败路径与 owner 可见 notice，而不是一张空面板；概览卡片顺序是 owner 自己的事实，所以演示 owner 对 `MoveOverviewCard` / `ResetOverviewCardOrder` 应用移动并回 `CommandOutput::OverviewLayoutApplied`，取证回执因此证明的是「owner 真的改了顺序并被界面渲染」；
   - **弹层取证走用户的开法**：添加配置弹层由 `open-profile-add()` 打开（与「添加配置」磁贴同一条路径），因此弹层记住的 profile 数量与 owner 发布的一致，不会被随后的快照发布误关；
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
     - `overview-card-reorder`（`success`,`cancel`,`error`）——概览卡片排序：界面派发共享 `CommandIntent::MoveOverviewCard` / `ResetOverviewCardOrder`，渲染 owner 发布的顺序，owner 拒绝时顺序不变（`tests/overview_card_reorder_tests.rs`）。
      - **工作台业务面板**（`tests/business_*_tests.rs`）：每个业务一个面板，面板只渲染共享 owner 发布的事实与共享投影，手势经 `surface::action::workbench::business` 解析成共享 `CommandIntent`：
        - 连接工作台（`ConnectionsWorkbench`）：`connections-close-all-confirm`（`success`,`cancel`）、`connections-details-drawer`（`success`,`close`,`unsupported`,`reorder`,`removed`）、`connections-grouping`（`success`）；
        - 日志工作台（`LogsWorkbench`）：`logs-redacted-export`（`success`,`cancel`,`error`,`retry`,`stale`,`pending`,`unsupported`,`filtered`）、`logs-scroll-lock`（`success`,`lock`,`resume`,`history`,`growth`）；
        - 运行时工作台（`RuntimeWorkbench`）：`runtime-telemetry-observation`（`unknown`,`zero`,`error`,`stale`,`refresh`,`locale`）、`speedtest-progress-cancel`（`success`,`cancel`,`error`）、`speedtest-details-modal`（`success`,`close`,`unsupported`）、`speedtest-history-comparison`（`success`,`empty`）；
        - 代理工作台（`ProxiesWorkbench`）：`proxies-custom-node-modal`（`success`,`cancel`,`error`）、`proxies-group-expanded`（`success`,`close`,`reorder`,`added`,`removed`）、`proxies-group-reorder`（`success`,`cancel`,`error`,`retry`,`refresh`）、`proxies-node-detail-drawer`（`success`,`close`,`unsupported`,`reorder`,`removed`,`error`,`retry`）、`proxies-probe-settings`（`success`,`cancel`,`error`,`retry`,`refresh`）、`proxies-uri-import-preview`（`success`,`cancel`,`error`）；
        - 规则工作台（`RulesWorkbench`）：`rules-list-editor`（`success`,`cancel`,`error`,`retry`,`refresh`,`stale`,`focus`）、`rules-override-editor`（`success`,`cancel`,`error`）、`rules-provider-refresh-error`（`success`,`error`,`retry`）、`rules-statistics-inspector`（`unknown`,`success`,`cancel`,`error`,`retry`,`refresh`,`stale`,`pending`,`locale`,`cleanup`）、`rules-tracer-drawer`（`success`,`close`,`error`）；
        - 设置工作台（`SettingsWorkbench`）：`core-version-rollback-confirm`（`success`,`cancel`,`error`）、`settings-lan-security`（`success`,`error`）、`settings-language-choice`（`success`,`error`,`retry`,`refresh`,`focus`）、`settings-permission-panel`（`success`,`error`,`unsupported`）、`settings-port-conflict-repair`（`success`,`error`）、`settings-tun-permission`（`success`,`cancel`,`error`,`unsupported`）；
        - 外壳工作台（`ShellWorkbench`）：`shell-command-palette`（`success`,`cancel`,`empty`,`full-list`,`locale`,`refresh`）、`shell-routing-intent-control`（`success`,`error`,`retry`,`close`,`stale`,`refresh`,`pending`,`unsupported`,`guide`）、`shell-routing-intent-authentication`（`error`,`guide`,`close`）；
        - 概览工作台（`OverviewWorkbench`）：`overview-reconnect-mask`（`success`,`error`,`retry`）、`overview-topology-drilldown`（`success`,`close`）。
      - **产品页搜索框**（`tests/page_search_business_tests.rs`）：搜索是页面自己的搜索框，业务由共享过滤/搜索归约收窄列表：
        - 连接页（`Connections`）：`connections-search-highlight`（`success`,`empty`）；
        - 日志页（`Logs`）：`logs-search-highlight`（`success`,`empty`,`error`,`clear`,`refresh`,`locale`,`stale`）——无效正则的 typed 理由由共享搜索模型给出，页面显示该理由而不冒充「没有匹配」；
        - 规则页（`Rules`）：`rules-search-highlight`（`success`,`empty`）；
        - 代理页（`Proxy`）：`proxies-search-highlight`（`success`,`empty`,`clear`,`error`,`retry`,`refresh`）——查询经共享 `ApplyOutboundUiPreference` 交给 owner，页面渲染 owner 读回的收窄节点表。
      - **编辑器页与过滤页的业务面板**（`tests/business_editor_tests.rs`、`tests/business_filters_tests.rs`）：`editor-large-document-window`（`success`）、`editor-snippet-insertion`（`success`,`error`）、`editor-yaml-error-navigation`（`success`,`error`）、`profiles-subscription-error`（`error`,`retry`）。
   - 行为测试位于 `tests/workbench_page_tests.rs`（首批行为）、`tests/workbench_path_tests.rs`（DNS/编辑器/过滤业务的交互路径）与 `tests/workbench_service_path_tests.rs`（诊断/同步/路由业务的交互路径）；共享夹具位于 `tests/workbench_paths/fixture.rs`。

5. **Slint surface 承载的业务（scenario 名称）**：
   - 承载集合由 `infiltrator_slint::capture_scenario::CarriedScenario` 唯一声明，`docs/capture/parity_scenarios.tsv` 按 `<feature>-standard` / `<feature>-compact` 两视口逐条列出；
   - **DNS 页**（`Dns`）：`dns-leak-alert`、`dns-query-details`、`dns-fake-ip-flush-confirm`、`dns-hosts-editor`；
   - **编辑器页**（`Editor`）：`profiles-script-sandbox`、`profiles-script-export-review`、`profiles-diff-viewer`、`profiles-snapshot-restore-confirm`；
   - **过滤/聚合页**（`Filters`）：`profiles-filter-editor`、`profiles-aggregator-wizard`、`profiles-aggregator-preview`；
   - **诊断页**（`Doctor`）：`doctor-failure-recovery`、`doctor-repair-progress`、`lifecycle-recovery-panel`；
   - **同步页**（`Sync`）：`sync-conflict-resolver`、`sync-restore-confirm`；
   - **应用路由页**（`AppRouting`）：`app-routing-policy-editor`；
   - **概览页**（`Home`）：`overview-card-reorder`——概览卡片顺序是共享 owner 的事实，界面发共享意图并渲染 owner 发布的顺序，排序面板由 `ui/components/overview_order.slint` 承载；
   - **配置弹层**：`profiles-import-modal`、`profiles-delete-confirm`；
   - **工作台业务页**（`ui/pages/page_workbench.slint` + `src/lumen_projection/workbench/panels/`）：`ConnectionsWorkbench`、`LogsWorkbench`、`RuntimeWorkbench`、`ProxiesWorkbench`、`RulesWorkbench`、`SettingsWorkbench`、`ShellWorkbench`、`OverviewWorkbench` 各自承载本工作台的业务面板，面板动作经 `invoke_workbench_action` 走共享命令；
   - **产品页搜索框**：`Connections`、`Logs`、`Rules`、`Proxy` 四页的搜索框是共享过滤的入口，取证用手势写入页面自己的搜索框。

6. **在 Slint surface 上仍无承载载体的业务**：
   - 下列业务在共享平价目录中存在，但 Slint surface 没有承载它们的界面状态，取证路径会拒绝它们，因此它们没有像素回执，也不能登记 Slint 的 scenario 锚点：
   - `shell-keyboard-focus`——焦点环跟随焦点的像素证据需要能驱动真实指针或按键的取景台架，Slint 的取景路径只驱动产品回调，因此该业务在 Slint 表面上仍无像素回执；
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

4. **官方控件收口（UI-FRAMEWORK-001 的 Slint 半边）**：
   - **Profile-Settings 与 Profile-Add 类型下拉均改官方 `PopupMenu`**：`ui/components/profile_sheets.slint` 的 `ProfileSettingsSheet` 与 `ProfileAddSheet` 都不再自绘覆盖层与自持开-关布尔；下拉改为窗口级 `PopupMenu`（`ui/lumen-app.slint` 的 `profile-settings-type-menu` 与 `profile-add-type-menu`，组件在 `ui/components/app_chrome.slint`），与主题、语言、代理排序菜单同一形状（`items` / `current_index` / `activated(index)` / `close_policy: close_on_click_outside`），开关状态只由 popup 的 `is-open` 决定，返回手势按同一顺序关闭它们；
   - **Android 复核（同一 pin）**：Slint 1.18.1 的 Android 后端 `i-slint-backend-android-activity` 的 `create_window_adapter` 对每个窗口返回**同一个** adapter（`lib.rs`），即 Android 上 popup 从不成为独立原生窗口，而是画在同一张 surface 里；winit 后端同样注明「We don't render popups as separate windows yet」。因此「popup 不能嵌套所以必须内联」这一 Android 理由在当前 pin 上**不成立**：嵌套 popup 与顶层 popup 的渲染路径一致——两个配置弹层的下拉已据此完成迁移；
   - **返回手势补齐两个确认弹窗**：`ui/lumen-app.slint` 的 `go-back()` 现在先关 `clear-traffic-dialog` 与 `settings-overlays` 的网络模式确认弹窗（两者都是官方 `PopupWindow`，状态由 popup 自己持有），再依次关主题/语言菜单、代理排序菜单与各类弹层，语义与菜单一致；
   - **滑块改官方 `Slider`**：`ui/components/base_widgets.slint` 的手搓 `AppColorSlider`（自绘 focus scope、指针换算、步进钳制）已删除，`ui/components/settings_sheets.slint` 改用 Lumen 官方 `Slider`（`stop_count` / `value_changed` / `released`）。**皮肤契约**：滑块的外观（轨道、滑块手柄、按下/悬停态）由工具链唯一拥有，产品不再自绘 22px 手柄与可换肤轨道；产品保留的是语义契约——取值区间、步进（以 `stop_count` 表达）、可访问名称、拖动即预览。Lumen 调色板绑定产品的 `MaterialSchemes`，因此强调色仍随产品主题变化；
   - **关于/开源许可页接入产品词汇**：`ui/pages/page_about.slint` 的检索框改用 `SearchField`、动作改用 `PillButton`，错误文案颜色交还 `LumenTheme.error`，预览框的裸 `"monospace"` 字符串改用主题等宽族；`TextEdit` 只保留为许可证全文的多行只读查看器（官方件，产品无对应组件）。
   - **列表虚拟化（未迁移，原因如下）**：Slint 表面每页只有**一个**滚动面（`ui/components/app_body.slint` 的 `page-scroll`），所有列表都是 `VerticalLayout` 里的 `for` 重复，**不存在 `ScrollView { for … }` 这一模式**；标准 `ListView` 只接受**唯一一个** `for` 子元素（编译器报错「A ListView can just have a single 'for' as children」）且必须自己拥有视口，所以迁移的前提是把页面拆成「纯列表页」。候选是代理节点列表（行高 76px，一组可有数百节点）与连接列表。**行高契约不在共享域**：`infiltrator-contract` / `infiltrator-presentation` / `infiltrator-shared` 都没有行高常量，76px / 46px / 58px 都是本表面的皮肤常量；虚拟化需要共享域给出行高契约（共享 owner 改动），否则只能冻结表面自己的行高。这条属于共享改动前置，未在本切片落地。

