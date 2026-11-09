# FlClash 桌面参考基准与逐项对照

> 层级：L3

本文是 Slint **桌面**表面对标 FlClash 的**外部参考基准记录**。它登记上游源码结构、
真实场景图的逐像素测量、本仓逐项对照与「求同存异」裁决；它不是本仓验收证据，
不能据此宣称任何像素或行为已验证。本仓结论只以最新整批发布回执为准。

上游版本：`chen08209/FlClash` v0.9.3（`be84736605527034689c95bf27828ff27d912c5e`）；
侧栏组件来自子模块 `chen08209/prism_ui`（`63450c0a`，本次以 `5d8ed7a` 的
`lib/sidebar.dart` 对照）。桌面场景图见
[flclash-desktop/flclash-desktop-dashboard.png](flclash-desktop/flclash-desktop-dashboard.png)
（2265×1665，SHA-256 `f6406be498613bc0249e9ae2350125a0e670e9620d0ae4b1be1aec7353372bcc`），
另有两张同源缩放图在 `.private/`（`flclash-dashboard-reference.png`、
`target-proxies-reference.png`），侧栏边界实测 430–440px、底色 (14,14,17)/(12,14,15)。

## 1. 上游架构（要求对标的「高级架构」）

| 层 | 上游真源 | 不变量 |
| --- | --- | --- |
| 导航契约 | `lib/common/app_ports.dart` 的 `NavigationPort`、`lib/models/common.dart` 的 `NavigationItem` | 导航项是 typed 数据（glyph + label + builder + modes），不是页面写死的按钮列表 |
| 导航装配 | `lib/views/navigation.dart` 的 `Navigation.getItems()` | 一处按能力/`modes` 生成 8 项，桌面/手机/更多各取所需 |
| 侧栏组件 | `plugins/prism_ui/lib/sidebar.dart` 的 `NavigationSidebar` / `SidebarDestination` | 组件只消费 `destinations + selectedIndex + expanded`，不读任何业务快照 |
| 仪表盘契约 | `lib/enum/enum.dart` 的 `DashboardWidget` enum（11 变体） | 每张卡是 enum 变体，带 `platforms` 能力面 |
| 仪表盘装配 | `lib/views/dashboard/widget_registry.dart` | `switch (widget)` 一处把变体映射到卡片组件；`removable` 决定可否重排 |
| 布局裁决 | `lib/views/dashboard/layout.dart`、`lib/common/layout.dart` | `DashboardDensity.of(size)` 三档、`dashboardColumnsOf` 2/4 列、`getProxiesColumns` 按宽度 |
| Core 方法面 | `core/protocol.go` 的 `CoreMethod` + `core/method.go` 的 `methodHandlers` | `area.verb` 命名、typed 参数、一张分发表 |

**关键裁决**：FlClash 的统计（状态、速率、内存、服务探测、连接数）全部是**仪表盘卡片**，
由 `DashboardWidget` 注册表装配；侧栏 `NavigationSidebar` 只接收导航项，**没有任何
状态/速率/版本属性**。本仓 b145 之前把同一批事实又发布到侧栏状态卡，是第二份真源。

## 2. 侧栏逐像素对照

真实场景图（2265×1665）实测：侧栏边界 x=429（18.9% 宽），底色 (12,14,15)；
选中行填充 (30,33,34) 覆盖 **y170–249 / x0–428 全宽**，即整条 429px 宽的选中带，
不是内容宽小胶囊；导航行墨迹 x18–144（图标+两字标签），行带 y279–316 / y366–405 /
y455–492 / y546–579，间距约 88–91px。

上游源码常量（`sidebar.dart`）：`_compactWidth=48`、`_expandedWidth=220`、`_itemSize=40`、
`_itemGap=4`、`_iconSize=24`、`_labelGap=12`、`_indicatorWidth=3`、`_indicatorHeight=16`；
选中态为 `selectedFill = onSurface.withOpacity(0.08)` **铺满整行**，外加 3dp 宽的
primary 色竖向指示条（16dp 高，随选中项动画移动），没有强调色实心胶囊。

| 维度 | FlClash | 本仓 b145 后 | 裁决 |
| --- | --- | --- | --- |
| 侧栏内容 | 纯 5–8 项导航 + 折叠钮 | 导航 + 分组标题 + 品牌块 + 状态/配置/速率/版本卡 | **求同**：删状态卡、品牌块、版本行；统计归仪表盘（本仓标题栏与仪表盘已有单一真源） |
| 选中态 | 全宽 8% 底 + 3dp 指示条 | 120u 内容宽强调色胶囊 | **求同**：改全宽淡底 + 指示条；b145 的 `pill_tail` 判据按此作废 |
| 行几何 | 40dp 高 / 4dp 间距 / 24dp 图标 / 12dp 标签间隙 | 44u 高 / 20u 图标 / 三套列 | **求同**：收敛到一份 `NavItemMetrics` |
| 导航项来源 | `NavigationPort.getItems()` typed 列表 + `modes` | 9 条 `NavButton` 字面量写死在 `sidebar.slint` | **求同**：由底层投影发布 typed 导航项 |
| 折叠 | `expanded` / `compactWidth=48` 两档；`canExpandSidebar` 只在 desktop 档放行标签 | `compact-layout` 一处布尔兼职决定「有无侧栏」 | **求同（b153 推翻 b145 的「存异」判断）**：上游按档给形态，本仓同样由契约给——`compact` 底部导航、`medium` 64px 图标轨、`expanded/ultra` 带标签列。b145 当时以「避免过度工程」为由拒绝桌面折叠档，实测代价是同一 720×480 窗口在 Iced 得竖轨、在 Slint 得横栏，短窗口多付一整条 72u 底部带（480 行的 15%）；真源早已裁决（契约 `SidebarForm::Rail`），缺的是组件而非理由，故按上游补齐 |
| 标签何时出现 | `labelOpacity = (progress − 0.4)/0.6`，轨形态不画标签，靠 tooltip 补 | 与上游同判：`ViewportLadder.labelled_sidebar(tier)` 决定行内是否画标签与分组标题 | **求同**：轨形态只画图标（图标 24u 居中、选中全宽淡底 + 3u 指示条），标签与分组标题留给带标签列 |
| 列宽的比例依据 | 48/600 = 8.0%，220 只在 >840 使用 | 64/600 = 10.7%；240 = 840 − 600（内容地板）；280 ≤ 1200 − 600 | **求同并成标准**：上限由 `ViewportTier::max_nav_column_px()` 一处裁决并导出，判定器与 Rust 测试复算同一条不变量（比例条款归 L2 空间利用率强制标准 §3.3） |

## 3. 仪表盘模块对照

上游 11 变体：`currentOutbound, networkSpeed, outboundMode, vpnButton(Android only),
networkTakeover(desktop only), serviceStatus, profiles, proxyGroups, connectionStats,
intranetIp, memoryInfo`。

本仓 `crates/infiltrator-slint/src/lumen_projection/dashboard.rs` 的 `DashboardFacts`
已把同一批事实从共享 `SurfaceSnapshot` 一次性投影，页面只消费 typed 值；
`home_desktop_grid.slint` 已有 hero / 上传 / 下载 / 内网IP / 配置 / 服务状态 /
内存信息 / 连接表 / 策略组网格等卡片。

**b148 复核修订（存异：本仓已更强，不写码）**：`infiltrator-contract::overview_layout`
已有 typed `OverviewCardKind`（8 变体）、`region` 把 shell 侧栏控件与概览卡分开
（上游 `DashboardWidget.platforms` 无此裁决）、`DEFAULT_ORDER` 默认顺序、
`OverviewLayoutSnapshot` 可重排 owner 与 4 项契约测试；`project_overview_layout`
纯消费。上游按平台门控的两张卡（`vpnButton` Android only / `networkTakeover`
desktop only）在本仓没有对应真实差异——`MasterSwitches` 四表面都渲染
（Android 走 VPNService）。因此 b146 登记的「缺 typed enum 注册表」是误判，
`FLCLASH-002` 直接办结，不新增第二套卡片清单、不动 wire/快照/UI。

## 4. Core 能力面差距（求同抽象，b146 复核修订）

上游 `CoreMethod` 约 60 个 `area.verb` 方法，登记在 `core/protocol.go`，
由 `core/method.go` 的一张 `methodHandlers` 分发表消费。本仓
`CoreOperation` 已 typed 覆盖大部分且更细（Profile/Dns/Rules/Subscription/
Provider/Outbound/Connection/Logs/FakeIpCache/Tun/Lifecycle/ProxyControl/
GC/Version）。

**b146 首版把三项登记为「真实缺口」，逐层复核后确认是误判，现更正**——
这是「求同存异」里"存异"的一侧，不得为办结任务而造重复能力：

| 上游方法 | 复核后的本仓等价真源 | 裁决 |
| --- | --- | --- |
| `route.watch` / `route.check` | `CoreSnapshot.revision` 每次事务递增 + `ActiveExitSnapshot{generation, revision}` 的权威读回；规则命中校验由 `CoreOperation::Rules(RulesOperation::Trace)` + `RuleTraceRun` / `RuleHitAuditSnapshot` 承担 | **存异（更强）**：本仓用 typed 快照读回 + revision 表达选路变更，不需要额外订阅通道与轮询计时器；上游 `routeTracker` 的 1s 轮询是因 mihomo 无 URLTest/Fallback 选路 hook，本仓不受此限 |
| `shareLink.encode` / `shareLink.decode` | `ProtocolCodecApplication::draft_from_uri` / `uri_from_draft`（DUAL-05 typed 契约，带 `ProtocolFidelity` 不可表达字段审计） | **存异（更强）**：本仓有完整草稿 → URI 往返与保真审计，非裸编解码 |
| `config.decrypt` / `profile.clearEffect` | 由 profile 生命周期与 `ProfileDocument` 的 typed 迁移器覆盖 | **存异**：语义不同，登记为不采纳 |
| `probe.outboundIp` / `probe.services` | `PublicIpProbeSnapshot` / `SpeedtestSnapshot` / `DashboardFacts` 的 4 服务延迟投影 | **存异（更强）** |
| `mitm.*`（8 个） | 无 | **显式不采纳**：本仓不做 MITM/脚本注入，产品边界不同 |
| `core.crash` | 无 | **显式不采纳**：崩溃注入是上游测试钩子，不进产品 |
| `traffic.reset` / `connection.reset` / `network.changed` | `network.changed` 由 `NetworkRoamingSnapshot` + `FakeIpCacheOperation::Flush` 覆盖；`traffic.reset` 本仓有 `CommandIntent::ClearTrafficHistory` | **b147 已收口**：该意图 dispatch 的 `Ok(())` 空操作改为调用共享波形 owner，capability 由 `ConnectionOperation::Close` 纠正为 `LocalOnly`；复核确认 Mihomo HTTP API 无 traffic-reset endpoint，波形历史是应用层 owner，故不新增 CoreOperation / 不动 adapter 矩阵 |

**审计产物（b147 已闭环）**：三个方法逐项核对后，只有 `traffic.reset`
对应的 `CommandIntent::ClearTrafficHistory` 存在两处真缺陷（dispatch 空操作
+ capability 错映射）。复核上游实现后**修正了 b146 的收口计划**：Mihomo
HTTP API 没有 traffic-reset endpoint（上游在嵌入 Go core 内进程内调
`statistic.DefaultManager`），本仓 `TrafficWaveformSnapshot` 是应用层采样
历史，因此正确做法是 `LocalOnly` + 共享 owner，而不是新增
`ConnectionOperation::ResetStatistics` 并铺进 adapter 矩阵（那会把不存在的
引擎能力写成假真源）。最终链路：`CoreApplication` 单一持有
`TrafficWaveformApplication` → `ProductSurfaceReader` 与三个 composition 的
`CommandApplication` 从同一 getter 派生 `Clone`（共享 `Arc<Mutex<_>>`）→
`domain::reset`（保留 generation/session、递增 revision、不伪造零样本）→
dispatch 真实调用 + 无 owner 时 typed refusal → 4 项 nextest 验收。
`FLCLASH-001` 已办结，`route.watch` 缺失之类的误判结论不留。

**纪律**：任何 core 缺口必须先建/认领底层任务，在 domain/contract/ports →
application → adapter → presentation 逐层收口并用 mock/nextest 验收，
再让 UI 消费；严禁在 UI 层打补丁或伪造业务事实。

## 5. 对既有空间利用率标准的修订

b144 立的「跨表面空间利用率强制标准」（L2 公共标准）用
YumeBox 手机帧定阈值、把侧栏选中盒的 `pill_tail` 当浪费判据，**参照物与判据都错**：
全宽选中带是导航结构，不是内容盒，右侧空地是合法呼吸位。该腿必须按 FlClash
桌面真实场景重裁：侧栏只判「导航行之间的地面带」与「侧栏是否混入非导航区块」，
不再用 `pill_tail` 惩罚选中态几何。页身 `unused` 三档阈值继续有效。
