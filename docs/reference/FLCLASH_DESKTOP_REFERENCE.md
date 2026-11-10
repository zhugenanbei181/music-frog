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

本仓同一套测量（b158，对已发布批次
`20261109T120842Z_9d58ffe1dea7_dirty_963715` 的 `app-routing-dark` 帧，1180×780
1:1 无缩放）：侧栏边界 x=240（契约 `standard` 档列宽），选中带 y234–277（**44u 高**、
全宽、无胶囊），图标列墨迹 **22u** 高（图标盒 24u），相邻行首墨迹间距 **48u**
（44u 行 + 4u 间距）。两侧数字逐条可对：参照物 429px/1.95 = 220dp 对 240u、
80px/1.95 = 41dp 选中带对 44u、88–91px/1.95 = 45–47dp 行距对 48u。

补测（b159，同一张图同一把尺子 429px/220dp = 1.950）：选中带实测 x4–421（≈2.1–215.9dp，
即几乎满列、两端约 2dp 的圆角内缩）、折叠钮 x10–89（5.1–45.6dp，恰为 `sideInset` 4 + 40dp 宽）；
第二行字形盒起于 **7.2dp**、标签墨迹 **44.1dp**。

上游源码常量（`sidebar.dart`）：`_compactWidth=48`、`_expandedWidth=220`、`_itemSize=40`、
`_itemGap=4`、`_iconSize=24`、`_labelGap=12`、`_indicatorWidth=3`、`_indicatorHeight=16`；
选中态为 `selectedFill = onSurface.withOpacity(0.08)` **铺满整行**，外加 3dp 宽的
primary 色竖向指示条（16dp 高，随选中项动画移动），没有强调色实心胶囊。

| 维度 | FlClash | 本仓现状（b158 复核） | 裁决 |
| --- | --- | --- | --- |
| 侧栏内容 | 纯 5–8 项导航 + 折叠钮 | 只有导航：`for item in navigation-items` 渲染 8 项；状态卡/品牌块/速率/版本行与两个分组标题均已删 | **求同（b164 已落地）**：统计归仪表盘，标题栏与仪表盘是这些事实的单一真源；判定器 `chrome_band` 0% 把这条钉成门禁（导航栈之下不得有任何区块），契约 `NavigationSeparator::None` 再把「不画分组标题」钉成跨表面策略 |
| 选中态 | 全宽 8% 底 + 3dp 指示条 | 全宽 `nav-selected-fill`（text-primary 8%）+ 3u 宽 16u 高指示条；b145 的 120u 强调色胶囊已废 | **求同（已落地）**：`pill_tail` 判据作废，右侧空地按合法呼吸位测量 |
| 行几何 | 40dp 高 / 4dp 间距 / 24dp 图标 / 12dp 标签间隙；行内墨迹带 37–40px（19–21dp），即两侧各约 10dp 空气；参照物实测 5 行 | 44u 高 / 4u 间距 / 24u 图标 / 12u 标签间隙 / 3u×16u 指示条（`LumenTheme` 一处令牌，无第二套列规格）；行内图标墨迹 22u，即两侧各 11u 空气；本仓轨道实测 8 项 | **求同 + 一处存异（b158 实测裁决，见 §2.1）**：间距、图标、标签间隙、指示条四项与上游同数；行高 **保留 44u**，理由是三条实测量而不是观感——① 参照物那 40dp 行里的墨迹只有 19–21dp，两侧各约 10dp 空气，本仓 44u 行内 22u 墨迹给出 11u 空气；**落到 40u 会把两侧空气压到 9u，比参照物更紧**；② 40u 在这条栈上少用 32u，而 `rail-row-height` 在桌面**没有内容消费者**（实测 0 处非 compact `NavButton` 实例），这 32u 全部变成导航列内部地面，判定器按 `void` 记账（实测 6.2% → 10.0%，上限 12%，预算占用 52% → 83%），`chrome_band` 两种行高都是 0.0%；③ 本仓轨道 8 项对参照物实测 5 行，同一行高差在更长的栈上代价更大。**所以 40dp 不可移植：它在参照物里是对的，在本仓会更紧且更空。** |
| 列内起始边界（leading） | 字形盒起于列缘 7.2dp、标签墨迹 44.1dp（场景实测）；源码推导为 12dp / 48dp（`min(12,(48−40)/2) + (40−24)/2`） | b158 实测 x32（= 2 × `rail-item-padding`）、标签 x68，占 240u 列的 13.3%，是状态卡时代的嵌套遗留，从未被任何判据审过 | **求同（b159 落地，`SPACEUTIL-010`）**：令牌改 `space-2`（8u），得字形盒 x8、标签 x44 —— **与场景实测的 7.2dp / 44.1dp 逐条对齐在 1u 内**（比源码推导的 12dp/48dp 更贴真实发布物）；列现在逐帧发布 `nav_content_offset_px`，判定器按导出的 `nav_leading_limit_px`（列宽的 8%，240 → 19u）判定，源码那组较宽松的读法（12u，5.5%）仍在预算内 |
| 导航项来源 | `NavigationPort.getItems()` typed 列表 + `modes` | `shell_navigation::NavigationItem::items(surface)` typed 注册表（`NavigationSurface::{Desktop, Dock}` 即上游 `modes`），投影为 `NavigationSlot`（id/label/group/group-label/icon 语义名）供侧栏纯消费；`group` 仍是注册表事实，但桌面呈现由 `shell_sidebar_presentation::NavigationSeparator` 单独裁决是否画成标题 | **求同（b146 已落地，b164 补上呈现策略）**：字面量清单已删；侧栏不读任何业务快照，图标只经设计权威的 canonical 名；上游 `_SidebarPane` 的 `Column(destinations.indexed)` 不插标题，本仓因此也不再花一行标题盒 + 16u 上空气把 8 行切成两簇 |
| 折叠 | `expanded` / `compactWidth=48` 两档；`canExpandSidebar` 只在 desktop 档放行标签 | `compact-layout` 一处布尔兼职决定「有无侧栏」 | **求同（b153 推翻 b145 的「存异」判断）**：上游按档给形态，本仓同样由契约给——`compact` 底部导航、`medium` 64px 图标轨、`expanded/ultra` 带标签列。b145 当时以「避免过度工程」为由拒绝桌面折叠档，实测代价是同一 720×480 窗口在 Iced 得竖轨、在 Slint 得横栏，短窗口多付一整条 72u 底部带（480 行的 15%）；真源早已裁决（契约 `SidebarForm::Rail`），缺的是组件而非理由，故按上游补齐 |
| 标签何时出现 | `labelOpacity = (progress − 0.4)/0.6`，轨形态不画标签，靠 tooltip 补 | 与上游同判：`ViewportLadder.labelled_sidebar(tier)` 决定行内是否画标签 | **求同**：轨形态只画图标（图标 24u 居中、选中全宽淡底 + 3u 指示条），标签留给带标签列；分组标题不再由「有标签」推导 |
| 列宽的比例依据 | 48/600 = 8.0%，220 只在 >840 使用 | 64/600 = 10.7%；240 = 840 − 600（内容地板）；280 ≤ 1200 − 600 | **求同并成标准**：上限由 `ViewportTier::max_nav_column_px()` 一处裁决并导出，判定器与 Rust 测试复算同一条不变量（比例条款归 L2 空间利用率强制标准 §3.3） |

### 2.1 行高 40dp / 44u 的实测对照（b158 裁决记录）

`FLCLASH-003` 要求把这一处差用实测裁决掉，不许用「更美观」。做法：把侧栏行高临时
改成上游的 40u，用同一套 capture 台架只采 720×480 轨道帧（`INFILTRATOR_CAPTURE_SCENARIOS`
过滤器，诊断运行**不发布**、不动已发布语料），再让判定器分别读两批像素。诊断运行目录
`.evidence/captures/slint/20261109T132841Z_3aa52d773553_dirty_1233973`（60 帧，
`CAPTURE DIAGNOSTIC PASS: 60 scenario(s) in 262s`），对照批是已发布的
`20261109T120842Z_9d58ffe1dea7_dirty_963715`（197 帧）。

| 量 | 44u（已发布） | 40u（诊断） | 差 |
| --- | --- | --- | --- |
| 轨道帧导航栈下沿 `nav_stack_bottom_px` | **398** | **366** | −32u（8 行 × 4u） |
| `chrome_band`（上限 0%） | 0.0% | 0.0% | 不变 |
| `void` 中位数 | 6.2% | 5.4% | −0.8pp |
| `void` 最大值（上限 12%） | **6.2%（预算 52%）** | **10.0%（预算 83%）** | +3.8pp |
| 内容列可用高度 | 不变 | 不变 | 0（`rail-row-height` 桌面无内容消费者） |
| 行内空气（图标 24u 盒） | 10u 上下 | 8u 上下 | 与参照物实测 10dp 的关系见下 |

两批的差是单向的：40u 只把导航列的内部地面变多，不换回任何内容高度；`void` 中位数
下降是因为部分帧原本就贴着 0，最大值的抬升才是这条轴的约束。参照物自己那 40dp 行里
墨迹只有 19–21dp（两侧各约 10dp 空气），本仓图标墨迹 22u——**同样 40u 在本仓只有 9u
空气，比参照物更紧**；44u 的 11u 空气才是与参照物同一条尺子下的等值。故裁决 **保留 44u**，
并把「不要照搬上游行高」的理由留在这里，防止后来者再拿 `_itemSize=40` 当依据改本仓令牌。

## 3. 仪表盘模块对照

上游 11 变体：`currentOutbound, networkSpeed, outboundMode, vpnButton(Android only),
networkTakeover(desktop only), serviceStatus, profiles, proxyGroups, connectionStats,
intranetIp, memoryInfo`。

本仓 `crates/infiltrator-slint/src/lumen_projection/dashboard.rs` 的 `DashboardFacts`
已把同一批事实从共享 `SurfaceSnapshot` 一次性投影，页面只消费 typed 值；
`home_desktop_grid.slint` 已有 hero / 上传 / 下载 / 内网IP / 配置 / 服务状态 /
内存信息 / 连接表 / 策略组网格等卡片。

**b167 注册表逐条可门禁（求同抽象的机器可读半条）**：上游
`lib/enum/enum.dart` 的 `DashboardWidget` 恰好 11 个变体；其中 `vpnButton`
仅 Android、`networkTakeover` 仅 `desktopPlatforms`（Linux/macOS/Windows），
`widget_registry.dart` 对每个变体给出唯一卡片组件、glyph、label 与可选
description，并规定 `currentOutbound` 不可移除（它承载启动按钮）。本批把这
11 个身份、平台门控、可移除性、组件名与**一处本仓真源**逐条落到
[flclash-dashboard-widgets.tsv](flclash-dashboard-widgets.tsv)，排序词表
SHA-256 `711a2507…`，由 `scripts/quality/flclash-dashboard-widget-guard.py`
离线校验并接入结构门。裁决仍是「求同存异」：上游按 UI 注册表拼卡，本仓
按 `OverviewCardKind` / `DashboardFacts` / 各 presentation 投影从 Core 快照
发布 typed 事实；Android 的 VPN 与桌面的网络接管不是第二份侧栏状态，而是
`VpnSurfaceObservation` 与 `MasterSwitches` 各自唯一 owner。守卫 planted 自测
10/10 覆盖上游卡片删增、平台门控漂移、不可移除规则、组件名、owner 缺失与
结构门接线，上游版本一变或台账出现未裁决项立即失败。

**b148 复核修订（存异：本仓已更强，不写码）**：`infiltrator-contract::overview_layout`
已有 typed `OverviewCardKind`（8 变体）、`region` 把 shell 侧栏控件与概览卡分开
（上游 `DashboardWidget.platforms` 无此裁决）、`DEFAULT_ORDER` 默认顺序、
`OverviewLayoutSnapshot` 可重排 owner 与 4 项契约测试；`project_overview_layout`
纯消费。上游按平台门控的两张卡（`vpnButton` Android only / `networkTakeover`
desktop only）在本仓没有对应真实差异——`MasterSwitches` 四表面都渲染
（Android 走 VPNService）。因此 b146 登记的「缺 typed enum 注册表」是误判，
`FLCLASH-002` 直接办结，不新增第二套卡片清单、不动 wire/快照/UI。

**b160 复核修订（求同：`SPACEUTIL-008` 的语义分歧在契约裁决）**：上游
`lib/views/dashboard/layout.dart` 把仪表盘列数写成一条**页身宽度**规则：
`dashboardColumnsOf(width) => width < 860 ? 2 : 4`，另外用
`DashboardDensity.of(size)` 给窄四列窗口降密度（1040/824 两条界）。本仓此前把
`ViewportTier::overview_card_columns`（1/2/2/3，Bevy 包裹流每行卡数）与 Slint 的
`overview-card-columns`（`compact-layout ? 2 : 4`，span 网格份额数）都叫「概览卡列数」，
于是同一个 1180×780 窗口被读成两种度量。裁决不是把 4 改成 2：契约新增 typed
`DashboardColumns::{WrappedCards, GridShares}` 与导出的
`dashboard_columns.four_column_min_px = 860`，生成器把 2/4 规则生成为
`ViewportLadder.dashboard_grid_shares(page-width, tier)` 供 Slint 的 span 网格消费；
轨/手机档固定 2 份额（720px 轨帧用 4 份额会把末卡挤到窗口外，b153 实测），
带标签列才按 860 页身规则给 4。Bevy 的 1/2/2/3 仍是包裹流的另一个 typed 事实，
Iced 的概览是纵向卡片流、不消费列数。**两种度量都成立，错的是共用一个 untyped `usize`。**
对已发布 197 帧复核：三档（400/720/1180）的 2/2/4 结果逐个不变，故本批不改像素、
不重采；`SPACEUTIL-008` 的登记前提由「待裁决」转为「已裁决」。

**b166 复核修订（求同：密度策略下沉为共享契约）**：上游把
`DashboardDensity.{standard, compact, mobile}` 与四条阈值集中在
`lib/views/dashboard/layout.dart`，并在 `dashboard.dart` 的 `LayoutBuilder`
里以 `constraints.biggest / scale`（页身可用区，已扣除外壳）调用
`DashboardDensity.of(room)`，经 `DashboardDensityScope` 注入每张卡片；
`traffic.dart` / `service_status.dart` / `proxy_groups.dart` /
`connections_table.dart` / `card.dart` 等只问 density 与 scale，不各自比较窗口。
本仓此前在 `home_desktop_grid.slint` 用 `!ViewportLadder.labelled_sidebar(tier)`
推导 `dense`，把「导航有没有标签」当成「仪表盘卡片要多密」——同一 1180×780
窗口若高度变短，前者不变而上游会降为 compact，这是真实的语义缺口。

b166 把三态与 `640 / 860 / 1040 / 824 / 560` 五个数字收进
`infiltrator-contract::responsive_viewport::DashboardDensity` 及
`page_structure` 导出 `dashboard_density`，生成器投影为
`ViewportLadder.dashboard_density(page-width, page-height)`；Slint 桌面网格
只以 `dashboard_is_compact(density)` 给 `dense` 取一个局部别名。存异项明确：
本仓缩放唯一真源仍是 `LumenUiScale.unit`，不引入上游 `scale` 第二通道；
阈值量的是页身可用区，不冒充窗口档界。三档现有采集帧的结果与迁移前一致，
因此这次只换架构真源，不改卡片几何、不重采；未来要收紧短窗口卡片高度时，
必须先给标准/compact 双视口真实像素回执再动视觉。

**b166 发布帧实测复核（存异的关键一处）**：第一次把 `dense` 直接接到
`dashboard_is_compact` 后，1180×780 帧页身高约 700u < 824，密度确实变
`compact`，但 `dashboard_grid_shares` 仍按 940u 页身给 4 份额；旧 `dense`
同时兼职「2 份额换行」，于是上传/下载与两个开关卡改用 2 份额 placement，
整页右半变成死地。上游从不这么用密度：`dashboard.dart` 始终把
`columns: dashboardColumnsOf(room.width)` 单独传给 `_DesktopDashboard`，
`compact` 只改 `topHeight/controlHeight/middleHeight` 等卡片度量。故本仓把
组件拆成 `compact-metrics`（typed density）与
`wrap-cards = card-columns < 4`（导出的页宽份额）两个只读事实，并补守卫
planted 禁止再用密度决定行 placement。整批 197 帧按该裁决发布后，短窗口
恢复四份额，仍从同一 scope 取紧凑字号/间距；这是「按真实场景求同存异」，
不是把上游常量照抄进 UI。

## 4. Core 能力面差距（求同抽象，b146 复核修订）

上游 `CoreMethod` 约 60 个 `area.verb` 方法，登记在 `core/protocol.go`，
由 `core/method.go` 的一张 `methodHandlers` 分发表消费。本仓
`CoreOperation` 已 typed 覆盖大部分且更细（Profile/Dns/Rules/Subscription/
Provider/Outbound/Connection/Logs/FakeIpCache/Tun/Lifecycle/ProxyControl/
GC/Version）。

**b163 盘点改为可门禁（求同抽象的机器可读半条）**：上游 v0.9.3
（`be847366`）的 `core/protocol.go` 实测恰有 **56** 个 `CoreMethod` 常量（此前写
「约 60」是目测）。这 56 个方法连同逐条裁决现在登记在
[flclash-core-methods.tsv](flclash-core-methods.tsv)（按方法名排序的 SHA-256
`a91a3a98…`：每条必须有 `same` / `stronger` / `local` / `unsupported` / `folded`
之一与一处本仓真源）。`scripts/quality/flclash-core-parity-guard.py` 在结构门里
校验数量、命名、去重、摘要与「无未裁决方法」；上游版本一变或有人悄悄加一条
未经裁决的方法，门会直接失败而不是让缺口沉默。裁决分布：8 条 `mitm.*` 与
`core.crash` 显式不采纳（产品边界），2 条 `traffic.reset` / `connection.reset`
为 `local`（b147 的 `LocalOnly` 真 owner），`route.watch` / `shareLink.*` /
`config.decrypt` 等为 `stronger` / `folded`，其余为 typed `CoreOperation` 同义覆盖。

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

## 6. 配置卡更新时间（b168）

上游 `lib/common/datetime.dart` 的 `getLastUpdateTimeDesc` 按
`<1m / 分钟 / 小时 / 天 / 30 天 / 365 天` 六档把 `lastUpdateDate` 解析成
相对时间；`lib/widgets/last_update_time_text.dart` 的
`LastUpdateTimeText` 以一分钟节拍刷新；
`lib/views/dashboard/widgets/profiles.dart` 在普通密度用
`profileUpdatedAt(time)` 加「更新于」前缀，并用 `computeTextSize` 测量放不下前缀时才只留 `time` 本身。

本仓逐层求同存异：contract 的 `RelativeTimeBucket`、
`RelativeTimeObservation` 与 `RelativeTimePresentation` 只持有六档、两个
owner 时刻与详细/紧凑模板选择；`SharedCopyKey`、shared zh/en 表、产品 `.po`
与 Android 三语言资源持有同一组词表；presentation 的
`relative_time_projection` 只解析 owner 发布的更新时间，并从 Core session
已经发布的起始时刻与测得时长得到观测时刻，产出 typed
`ProfileUpdateAge`；Slint 的 `project_dashboard_facts` 只把 typed 事实经共享
目录本地化后交给页面，不读 `SystemTime::now()`、不拼接绝对时间、不在 UI 里
推导档位。求同的是参考产品六档、双模板与一分钟可读的相对时间；存异的是
本仓把时钟事实归 Core owner、把文案归共享目录，把刷新留给下一次 owner
readback，也不把页面密度冒充上游 `computeTextSize` 得到的文字宽度事实——
桌面卡消费详细模板，紧凑模板保留给真正发布测量结论的表面。

## 7. 配置卡来源标签（b169）

上游真实场景图 `docs/reference/flclash-desktop/flclash-desktop-dashboard.png`
的配置卡在名称 `3980.yaml` 右侧显示「本地」；源码
`lib/views/dashboard/widgets/profiles.dart` 的 `_sourceOf` 用
`ProfileType.{url,file,custom}` 映射到 `remote/local/custom`。上游 `Profile`
的 `type` 是配置身份本身，而不是通过 `url.isEmpty()` 临时猜出来的展示字段。

本仓已有更细的 owner 事实：`ProfileSnapshot.filter_source` 携带
`ProfileWorkspaceIdentity.document.source.kind`，即 engine-neutral 的
`ProfileDocumentSourceKind::{Local,Subscription,Imported,Synchronized}`。
b169 不新增 wire 字段，只把这份已发布事实投影为
`ProfileSourceObservation`，由 `SharedCopyKey` 与四份本地化资源给每个来源
一个稳定文案；`ProfileQuickCardProjection` 删除 `is_local: bool` 与 URL
字符串判定；Slint 新增桌面/手机共用的 `ProfileIdentityRow`，在配置名同一行
右侧绑定 owner 本地化后的来源标签。求同的是「名称 + 来源」同一行组件与
typed 来源身份；存异的是本仓保留四种真实来源和显式 `Unobserved`，不伪造
上游 `custom` 档，也不把写保护、URL 或平台形态当成第二个来源真源。

最终 b169 发布批 `20261110T002611Z_536f4a1c8930_dirty_56001`（197 帧，999s）
的标准桌面与手机 `overview` 实图均可见配置名、来源「远程」与 b168 的
「更新于 17 小时前」；本仓 active fixture 是订阅配置，所以来源应显示「远程」
而非参考图里的「本地」，两者都由各自配置身份发布。该批实测空间利用率
197 帧 0 违规、回执时效 0、截图身份 574/10/0、设计资源 0 违规。

## 8. 侧栏展开是设置 owner，不是 UI 布尔（b170–b171）

pinned `be847366` 的桌面截图左上折叠按钮后面有四层事实：
`lib/models/config.dart` 持久化 `sidebarExpanded`（默认 `true`，并把旧
`showLabel` 键经 `_readSidebarExpanded` 显式迁移）；`lib/common/layout.dart`
用 `canExpandSidebar(viewMode) => viewMode == desktop` 禁止笔记本档展开标签；
`lib/manager/app_manager.dart` 以 `canExpand && sidebarExpanded` 生成唯一
`expanded` 事实，toggle 只写 `!sidebarExpanded`；`test/widgets/sidebar_test.dart`
逐像素钉住折叠 48px、展开 220px、glyph/tooltip 随形态切换。机器可读账本见
[flclash-sidebar-expansion.tsv](flclash-sidebar-expansion.tsv)，四份源码
SHA-256 与 pinned commit 由 `flclash-sidebar-expansion-guard.py` 离线校验。

本仓求同的底层裁决是
`infiltrator-contract::shell_sidebar_preference`：设置身份是 typed
`SidebarExpansionPreference`，视口能力是 `SidebarExpansionCapability`，呈现结果
是 `SidebarExpansionPresentation::{NoColumn,Collapsed,Expanded}`，用户动作是
`SidebarExpansionIntent::{Set,Toggle}`，唯一 resolver 把 desktop 档的
`Collapsed` 折叠成共享 `SidebarForm::Rail`。存异的是本仓不在 UI 保存私有
bool、不把「能否展开」藏在 Option/null 里，也不新增第二套 48/220 宽度：折叠
复用既有 64px rail 与空间利用率上限。b170 只交付 Core 契约、源码台账与
离线守卫；b171 继续按 Core-first 把偏好落到 `AppSettings.sidebar_expansion`
唯一持久化真源、`ApplySidebarExpansion` typed 命令、settings store 原子更新
与 `SettingsPageSnapshot.sidebar_expansion` 权威读回，并由
`sidebar-expansion-owner-guard.py` 钉住十文件 owner 链。侧栏 toggle 仍是
后续表面接线；两批都不改任何像素，故沿用 b169 的 197 帧回执而如实记为
未变更身份。
