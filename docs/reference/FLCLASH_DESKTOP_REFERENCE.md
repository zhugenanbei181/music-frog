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
代理页参照图已纳入版本库
[flclash-desktop/flclash-desktop-proxies.png](flclash-desktop/flclash-desktop-proxies.png)
（3193×1949，SHA-256 `ebe094bae677d5419cb380e800d5eee74197d20a48891fbe25ba340af7f98e8e`），
侧栏边界实测 431px、底色 (14,14,17)/(12,14,15)。它此前只是 `.private/` 里的
一张 gitignored 缩放图；b201 把它提升为可复跑的门禁输入，
`scripts/quality/flclash-content-ground-guard.py` 逐像素复量页身 80.64% 页尾空地，
并把空间利用率标准 §3.1 的 `content` 82% 上限钉在实测值加 1.4pp 余量上。

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
| 导航项来源 | `NavigationPort.getItems()` typed 列表 + `modes`；`openLogs` / `hasProxies` 再在运行时裁剪 | `shell_navigation::desktop_items(surface)` typed 注册表（静态 `NavigationSurface::{Desktop, Dock}` 只对应上游的目的地形态/归属，不等价于 `modes` 动态能力门控），投影为 `NavigationSlot`（id/label/group/group-label/icon 语义名）供侧栏纯消费；`group` 仍是注册表事实，但桌面呈现由 `shell_sidebar_presentation::NavigationSeparator` 单独裁决是否画成标题 | **求同（b146 已落地，b164 补上呈现策略，b232 补上 surface owner）**：字面量清单已删；侧栏不读任何业务快照，图标只经设计权威的 canonical 名；上游 `_SidebarPane` 的 `Column(destinations.indexed)` 不插标题，本仓因此也不再花一行标题盒 + 16u 上空气把 8 行切成两簇。**存异与缺口**：上游 `NavigationItemMode::{always, needHasProxies, openLogs, more}` 与 `openLogs` / `hasProxies` / `moreFull`、当前页不可见时回退首项，在本仓尚无等价 typed capability snapshot / active-page fallback；这是 Core-first 待办，不得在 UI 层临时过滤或删行 |
| 折叠 | `expanded` / `compactWidth=48` 两档；`canExpandSidebar` 只在 desktop 档放行标签 | 导航形态由契约 `NavigationForm` 决定，`compact-layout` 只控制页面密度；但旧标题栏条件错误地跟随 `compact-layout` | **求同（b153 推翻 b145 的「存异」判断；b207 修正标题栏挂载）**：上游按档给形态，本仓同样由契约给——`compact` 底部导航、`medium` 64px 图标轨、`expanded/ultra` 带标签列。页面密度不能替代窗口 chrome 状态；frameless desktop shell 即使使用 rail，也必须挂载桌面标题栏与窗口控制。 |
| 窗口 chrome 与页身原点 | pinned v0.9.3 `lib/common/layout.dart::getWindowHeaderHeight`：桌面 Windows/Linux 32dp、macOS 28dp；真实场景图顶部保留 pin / minimize / maximize / close 控件带，截图实测约 27dp，与 macOS 分支相符 | `overview-dark`（1180×780）曾自绘 40px 标题栏、页身顶边另留 8px；旧 `CAPTURE_CHROME` 却只报 `shell_top_air_px=8`。`overview-compact-dark`（720×480）挂 rail，却因 `compact-layout` 条件漏掉标题栏并报 0px | **求同并删掉 Slint 第二真源（b207）**：标题栏挂载与窗饰由桌面宿主读 `WindowChrome::FRAMELESS` 一次规划；其 38px 被 Slint / Iced / Bevy 共用，但与 pinned FlClash 的 Windows/Linux 32dp 仍差 6dp（macOS 28dp，本仓当前 `Unsupported`），另立 `FLPARITY-050` 收口共同契约与三表面回执。`shell_top_air_px` 回报窗口坐标 `AppBody.y + page-content-padding-top`，底边同步包含窗口到 AppBody 的距离。标准帧回执从 8px 校正为 **46px**，rail / 窄桌面帧从 0px 校正为 **38px**；`compact-layout` 从此只决定页面密度。 |
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
| `void` 最大值（当时上限 12%，b204 起 8%） | **6.2%（预算 52%）** | **10.0%（预算 83%）** | +3.8pp |
| 内容列可用高度 | 不变 | 不变 | 0（`rail-row-height` 桌面无内容消费者） |
| 行内空气（图标 24u 盒） | 10u 上下 | 8u 上下 | 与参照物实测 10dp 的关系见下 |

两批的差是单向的：40u 只把导航列的内部地面变多，不换回任何内容高度；`void` 中位数
下降是因为部分帧原本就贴着 0，最大值的抬升才是这条轴的约束。参照物自己那 40dp 行里
墨迹只有 19–21dp（两侧各约 10dp 空气），本仓图标墨迹 22u——**同样 40u 在本仓只有 9u
空气，比参照物更紧**；44u 的 11u 空气才是与参照物同一条尺子下的等值。故裁决 **保留 44u**，
并把「不要照搬上游行高」的理由留在这里，防止后来者再拿 `_itemSize=40` 当依据改本仓令牌。

**b204 追记**：该上限此后被参照物像素重定。参照图侧栏（边界 x=429）导航行之间的
最大死带实测 **51px / 1665px = 3.06%**，由 `scripts/quality/flclash-sidebar-void-guard.py`
逐像素复量；本仓最短档 480px 画 8 项而非参照物 5 项，同一绝对缝在短窗口占比更大，
故 `void` 上限取参照实测的 **2.6x 有界缩放余量 = 8%**，而不是照抄 3.06%。
这条收紧同时**加强**本节的 44u 裁决：诊断批那 10.0% 在新上限下直接违规，
40u 不但不更贴参照物，还会把这条轴推出预算。

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

## 8. 侧栏展开是设置 owner，不是 UI 布尔（b170–b172）

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
`sidebar-expansion-owner-guard.py` 钉住十文件 owner 链。b172 再把上游的
`canExpand && sidebarExpanded` 一次投影导出为 16 行封闭变体表：偏好、视口档、
触摸面共同决定合法导航形态、列宽、标签与按钮可用性；Slint 生成器只读这张
表，判定器也复用同一导出真源。侧栏 toggle 仍是后续表面接线；三批都不改任何
像素，故沿用 b169 的 197 帧回执而如实记为未变更身份。

## 9. 折叠按钮与纯外壳双视口回执（b173）

b173 把上游桌面截图左上角的折叠/展开按钮真正接进本仓：按钮只发送
`CommandIntent::ApplySidebarExpansion { intent: Toggle }`，设置 owner 持久化偏好，
`SettingsPageSnapshot.sidebar_expansion` 读回后投影回表面；Slint 不保存私有
collapsed bool，也不从当前列宽反推偏好。按钮是否可用只读 b172 的 16 行封闭
`sidebar_variants` 表：非触摸桌面 `expanded/ultra` 档允许收起，触摸面与较窄档
保持底栏/轨形态且不显示桌面折叠按钮。收起态复用 64px rail 与 canonical
`chevron-right.svg`，展开态用 `chevron-left.svg` 与共享本地化的「收起」文案；
侧栏仍只画导航项，状态、速率、内存、版本等非导航事实继续只由标题栏与仪表盘
单一真源发布。

求同的是上游截图与源码的「左上导航折叠钮 + 220/48dp 两形态 + 纯导航侧栏」，
存异的是本仓保留 typed 设置 owner、typed intent、16 行视口/表面变体表和
64/240/280px 共享宽度阶梯，不把偏好藏在 UI 布尔里，也不把统计卡塞回侧栏。
可见身份与 `CAPTURE_CHROME` wire 同时变化，因此整批 199 帧重采：新增
`sidebar-collapsed-standard` / `sidebar-collapsed-compact` 两张纯外壳折叠帧，
两帧的 `CAPTURE_CHROME` 均发布
`sidebar_expansion=collapsed touch_surface=0 nav_form=rail nav_column_px=64 nav_content_offset_px=0`，
页面内容立即补位且 `chrome_band` 仍按 0% 判据。捕获矩阵把 `interaction` 升为
显式第 9 列，避免把纯外壳帧的名字误当成一个并不存在的业务场景；owner guard
以 planted 负例钉死「收起帧不得派生业务交互」。

## 10. 按钮带必须由侧栏测量，chrome 投影必须由组件持有（b173 收口 + b174）

上游把折叠钮放在导航列表的**第一个 item**（`layout.dart` 的
`SliverList` 首个子项，`sidebarExpanded` 决定它画 `chevron-left` 还是
`chevron-right`），而不是浮在 40px 应用栏之上的独立层。b173 首次接线时按 theme token
在**侧栏 pane 局部坐标**里算出 `rail-vertical-inset + rail-row-height + rail-item-gap =
18 + 44 + 4 = 66`，把 `sidebar_toggle_px` 猜成 66。实测像素证明这个猜测是错的：桌面
外壳把侧栏挂在 40px 标题栏**之下**，按钮带真正结束于窗口坐标 y≈106，于是判定器把按钮
自身的字形当成第一行导航，leading 腿整批误判（读出 x38）。**按钮带属于侧栏布局，也只
有绘制它的元素知道自己在窗口里的位置。**

b173 的收口修正因此把事实交回测量者：`DesktopSideBar` 在按钮盒上用
`absolute-position.y + height + rail-item-gap` 发布 `sidebar-toggle`（init 加
`changed absolute-position` / `changed height` 两条腿，缩放、皮肤与页面切换都会重发），
**b207 补记**：b173 的 40px / y≈106 是当时 Slint 自持 40px 标题栏的历史实测；b207
把 Slint 接回既有 38px 契约条带后，同一测量给出的当前值是 y≈104。
历史数值保留在此处作为那次接线缺陷的证据，不再作为当前几何的判据。
沿 `AppBody` 上传，回执实测 1180×780 = **106px**，导航 leading 回到 **9–10**，与声明的
8px 在 2px 容差内。

b174 收的是这条链的**呈现结构**：`receipt-chrome` 不再是 `lumen-app.slint` 里内联的一段
struct 字面量，而是独立组件 `crates/infiltrator-slint/ui/components/shell_receipt.slint`
的投影——外壳只把导航形态、偏好、触摸面、按钮带、栈尾、leading、页边距、页身空气、
底栏带与缩放喂给它。同一批把外壳压回 SLINT-001 预算（609 → 436 代码行），并按最终树
重采 199 帧发布 `20261110T063442Z_a58ed35be980_dirty_309370`（444s，与 b173 收口批
`20261110T060401Z_a58ed35be980_dirty_90070` 相比 198 帧逐字节相同，唯一差异是搜索高亮
动画帧）。求同的是上游「按钮即列表首项」的结构；存异的是本仓把它作为 typed chrome 事实
测量、投影并门禁，而不是让判定器或外壳各记一个数字。

## 11. 折叠钮只有一个字形，词进 hover tooltip（b176）

b173 接线时给折叠钮配了一个常驻的「收起」文字。本仓当时把它当作「和导航行一样
有标签」的证据，但上游两头都不是这个意思。pinned `be847366` 的
`test/widgets/sidebar_test.dart` 只用 `find.byTooltip('Expand')` /
`find.byTooltip('Collapse')` 定位这个控件，并钉
`expect(tester.getCenter(toggle).dx, sidebar.center.dx)`——rail 里它是一颗居中的
图标；而 `expect(_rowTooltip('Proxies'), findsNothing)` 明确断言**展开态的目的地行
没有 tooltip**，标签就是画在行上的。仓库里的发布帧同样只画一颗 chevron，墨迹
x18..57，其后没有任何文字。

b176 因此把可见身份改回上游的形状：

- 常驻 `Text { @tr("收起") }` 删除，词改由 Slint 原生 `Tooltip` 元素承载，
  展开态 `收起` / 收起态 `展开`（en 即上游的 Collapse / Expand，两词早已在产品
  文案目录里，本批不新增 key）。
- 按钮盒的几何不再自己拼一份内缩：它与下方目的地行使用**同一条表达式**——
  `padding-left: root.nav-content-offset`、`padding-right: rail-item-padding`、
  `spacing: rail-label-gap` 与同一对 stretch 单元。此前两种各自错误的拼法都被
  像素抓到：先是一颗 `rail-inset` 定宽占位块把图标推到 x28（比目的地行的 x10
  还内缩 18u），再是把它换成 stretch 后在 rail 里偏左 12u（图标中心 11.5 对
  行图标 23.5）。复用行的表达式之后实测图标中心
  **rail 23.5 == 行 23.5**、**pane 19.5 == 行 19.5**，两处都落在同一列中心。
- 折叠钮仍只发 `ApplySidebarExpansion { intent: Toggle }`，`sidebar_toggle_px`
  仍由按钮盒自己测量（本批实测仍是 106px），wire 与 12 键都不变。

目的地行的 tooltip 规则随后由 b197 按「底层先裁决」补齐（`FLPARITY-048`）：上游
prism_ui 的 `lib/sidebar.dart` 先算 `labelWidth = rowWidth - iconInset - iconSize -
2*labelGap`，再用 `TextPainter(maxLines: 1).layout(maxWidth: labelWidth)` 的
`didExceedMaxLines` 决定 tooltip；`sidebar_test.dart` 钉住 rail 每行有 tooltip、
展开态短标签无 tooltip、展开态超长西里尔标签保留 tooltip。本仓没有 Slint `Text`
的 elide 读回，也不允许 UI 用字号×字数猜宽度，因此新增
`infiltrator_contract::sidebar_label`（一处几何 + `plan_truncation` 规则）与
`infiltrator_presentation::sidebar_label_projection`（随包字体的半/全/零宽度量桥），
把 `label_truncated` 作为 typed readback 发布进 `NavigationSlot`；`sidebar.slint`
只按 `!labelled || label_truncated` 挂载原生 `Tooltip`。诚实边界：共享度量是字符
宽度类近似，不是 shaping/kerning 级 glyph advance；hover tooltip 本身仍不进像素
回执（捕获矩阵按设计不投指针 `capture-pointer-absent`），只有源码、单测与结构门
可证，b197 诊断帧与已发布帧逐字节相同。

可见身份变化（pane 少一行文字、rail 图标移位）按标准整批重采：发布批
`20261110T090346Z_8b86599df608_dirty_1204003`（199/199，499s），
`space-utilization --enforce` 199 帧 violations=0、`receipt-freshness 0`、
Slint 侧 `screenshot-identity` 别名摘要全部重新核对为新鲜。

## 12. 上游不画圆形国旗，本仓的圆旗是自造（b177）

用户反馈「一般国旗都是方形 最好不要做这种圆形旗帜图案容器」，并附了一张回执裁图：
香港旗帜是一枚红色奖章。对照上游源码（`/tmp/flclash_src2`，与本仓 pinned 参照同源）：

- `lib/widgets/icon.dart::CommonTargetIcon` 画节点/分组的身份图块，两条通道分别是
  base64 图标字节与远端图片缓存，**没有任何圆形裁剪**；空/坏图回落
  `_ImagePlaceholder`，`GroupMonogram` 则取名称首字母大写。两者都装在
  `SizedBox.square(dimension: size)` 里——上游给身份图块的形状是**方盒**。
- `lib/views/proxies/list.dart` 的三个图标样式（`contained` / `plain` / `hidden`）
  只是方盒的容器装饰（`AppShape.all(AppCorner.xl)` 圆角矩形）与显隐，没有 `ClipOval`。
- 全仓 `BoxShape.circle` 只有三处（`about.dart` 两枚、`backup_and_restore.dart` 一枚），
  都是头像/徽记，**没有一处用于国旗**。也就是说圆形国旗从来不是对标结论。

本仓的圆形来自两层叠加，都不是审美选择：

1. `FlagIcon` 把资产画进 `border-radius: diameter / 2` + `clip: true` 的容器；
2. 它消费的清单本身已经是圆形栅格（`variant: "circle-144"`，144px PNG），
   而 catalog 早已把同样 258 面旗帜按 `flag.<code>` / `variant: "source"` 登记为
   Twemoji 方形矢量——同一个事实两份库存，UI 又对圆形资产做了第二次圆形裁切。

b177 把形状与清单一次改回事实：`FlagImages.flags` 逐条消费 catalog 的方形矢量；
`circle-144` 的 258 条记录与 `assets/flags/circle-png/` 从 shared 资产面整体退役；
`FlagIcon` 画方盒（无 clip、无圆角、`image-fit: contain`），尺寸入口按事实改名
`size`，五个调用点同批迁移且不留 `diameter` 兼容分支。`lumen-source/` 内的圆形副本
是逐字节保存的供应商导入快照，由 `slint-source-import-guard.py` 钉住，不属于本仓清单。

实测判据（诊断批 `20261110T101632Z_4a1293928702_dirty_1685609`，`proxies-dark`）：
组胶囊三枚旗帜墨迹盒 **20×16px、连通率 0.975**，顶行跨度 16 对中行跨度 20——
圆形容器的顶行只会剩 2–3px 且连通率约 0.785。节点卡旗帜同为 18×14 / 0.984。
选中态对勾徽记仍是圆形：那是徽记，不是旗帜。

长效机制是 `scripts/quality/slint-flag-shape-guard.py`（SLINT-020，14 项 planted 自测）：
清单顺序、`flag_assets::index_for_code` 一致性、圆形派生物回归、clip/圆角/cover 与
退役入口回归全部被咬；`--mode enforce` 与 `--self-test` 双腿接入结构门。

## 13. 身份块就是旗面本身，圆角取旗面自己的圆角（b185，推翻 b183 的 31% 判定）

同一句用户反馈在第二次回执后仍然成立：b183 把**资产**改回方板、把**私有圆盘**收进
一个共享组件，但那个组件仍是一个 38u 见方的**地面**，圆角取参照物的 31%。旗面本身
不是方的：所有共享旗帜资产是 36×36 画布，旗面是其中第 5..31 行的 **36×26 矩形**，
四角 4px（把 `flags/hk.svg` 用 `rsvg-convert` 渲成 38×38 实测：红色像素盒 38×28、
顶行内缩约 1px——即 4/36≈11% 的圆角，而不是 31%）。于是方地面在旗面上下各露出
约 5px 的带子，圆角又比旗面圆得多，回执里那枚「奖章」并没有消失。

顺带核实一条渲染事实，避免再按错误模型修：Slint 1.18 **不会**用 `border-radius`
裁剪子元素——`i-slint-core-1.18.1` 的 `item_rendering::visit_clip` 只在
`clip: true` 生成 `Clip` 项时 `combine_clip`，圆角只影响矩形自身的填充与描边。
所以那两条带子是**画出来的地面**，不是被裁掉的旗面。

本仓据此把身份块改成**旗面自己的矩形**：高度取 `side * 26 / 36`，画布再上移自身
留白 `side * 5 / 36`，使旗面正好落在 chip 的边缘；地面只在「未解析出旗帜、要画国家
代码」时出现（`background` 条件于 `flag-index`），有旗时旗面就是全部视觉；圆角取旗面
自身的 4/36（38u 档 4u、32u 档 3.5u），`slint-flag-shape-guard.py` 的判据由「不超过
半边长」收紧为「不超过边长八分之一」，并新增「有旗时不得画地面」与「call site 容器
圆角不得超过八分之一」两条（planted 自测 16 → **21/21 OK**，enforce findings=0）。

整批重采 `20261110T190711Z_b35c2a99bf86_dirty_1464030`（199/199，1386s）实测
**14 帧**变化：桌面档四皮肤 + `overview-card-reorder-standard` + `sidebar-collapsed-standard`
的差异全部落在出口行带（x286–481 / y160–228），720×480 档（`overview-compact-dark`、
`overview-tablet-dark`、别名帧 `sidebar-collapsed-compact`）落在 x92–212 / y108–176；
400×800 移动档与吉祥物帧因 chip 矮了 9px 而整列上移（内容列重排，非换皮），
其余 185 帧逐字节不变。`space_utilization --enforce` 199 帧 `violations=0`、
`receipt-freshness 0`、Slint nextest 386/386；`sidebar-collapsed-compact` 别名摘要
按新字节刷新（`ecbbfb24…`，与 canonical `overview-compact-dark` 逐字节相等）。

## 14. Iced 侧栏发布自己的 chrome 回执，行几何对齐参照物（b187）

判定器的像素腿此前只覆盖 Slint：Iced 的 `marker.log` 只有
`CAPTURE_READY page=… skin=…`，没有 `CAPTURE_CHROME`，「Iced 侧的空间利用率」
一直没有判据。b187 补上这条腿的两半——Iced 会发布回执，判定器能读第二张表面
（`--surface {slint,iced}`，各自采集根与欠账台账）——并借这次把 Iced 的导航行
几何对齐到 §13 已经量过的参照行：

- **行几何**：参照的目的地行是「整列宽的行面 + 24dp 字形 + 起笔 8」——Slint 侧已
  实测 `rail-row-height` 44u、`nav-icon-size` 24u、`rail-content-leading` 8u
  （`space-2`），标签起笔 x44。Iced 侧此前是 38px 行、18px 字形、内容左内边距 24px：
  既不是参照行，起笔也超出该档导出预算（240 列 → `nav_leading_limit_px` 19u）。
  现在行面是整列（选中洗色起于列缘），指示条 3×16 画在 x0 的独立图层上，内容起笔
  恰好 `sidebar_chrome::NAV_CONTENT_OFFSET_PX`（共享阶梯的 8），字形 24，
  标签起笔 8 + 24 + 12 = 44，与参照物同列。
- **折叠钮**：参照的桌面场景在目的地列表之上放一枚折叠钮，字形随形态居中或左对齐，
  词只出现在 hover tooltip。Iced 侧此前没有这个控件，而导出变体表对
  `(expanded, expanded, touch=false)` 要求 `toggle_available=true`，所以它不只是
  观感缺口，还是回执的硬前提：现在钮只有一枚 chevron（展开态朝左、折叠态朝右），
  词走 tooltip，其带由回执发布（`sidebar_toggle_px`），判定器据此把钮的起笔排除在
  「导航起笔」之外。Iced 轨道行也补上了参照的 hover tooltip（此前注释声称有、
  代码没有），标签来自同一条注册表行。
- **偏好只有一个 owner**：`sidebar_expansion` 由设置 readback 持有，Iced 只镜像它，
  并把 `resolve_sidebar_expansion(偏好, 档位能力)` 的答案画成形态；钮按下时发出契约
  的 `ApplySidebarExpansion { Toggle }`，UI 不持布尔。于是「展开态 240 列 / 折叠态
  64 轨道」不是本地推导，而是与判定器读的同一张导出表。
- **回执的每个数都来自活的布局**：`crates/infiltrator-iced/src/sidebar_chrome.rs`
  定义三个 widget id（栈底、折叠钮、底栏），采集期的几何探针逐帧量出它们的窗口矩形；
  任一格没量到就不写 marker——残缺回执是拒绝，不是默认值。
- **判定器分账**：`--surface` 默认仍是 slint（结构门的裸调用口径不变），Iced 侧自己的
  台账是 `scripts/quality/space_utilization_debt_iced.json`；清单里的 `page` 若不在
  契约导出表内，判定器报「用了没人声明过的上限」，不再静默退回档位兜底。

诚实边界（b189 待办，不是本批结论）：Iced 侧矩阵只有 `1180x780` 与 `720x480`
两档（后者 medium 是 64 轨道、无折叠钮），**没有 compact 帧**，折叠态也还没有真实
像素回执；Iced 清单的 `page` 列还是路由名而非形态 slug（`filter`/`mixin`/
`rules-providers` 三处 12 帧不在导出表内），要先把这列改成契约形态（或为确实独立的
页补形态）才能让 `--surface iced --enforce` 通过。

## 15. 第三轮反馈：换掉资产族本身，旗帜才是矩形（b188）

同一条用户反馈在 b183（收掉两个调用点的私有圆盘）、b185（收掉方地面与 31% 圆角）之后
**第三次**出现（2026-11-11，随附当前回执裁图）。这次先把裁图量成事实，避免再按错误的
模型修：红色像素盒 **76×56**，正是本面身份块的 **38×28 在 2× 显示缩放**下的尺寸，顶行
内缩约 12 物理像素（≈6u）——即资产自带的 **4-of-36 圆角**加上抗锯齿。b185 之后组件已经
不再画地面，回执里剩下的「容器」感只可能来自**资产自己**：一幅矩形旗帜外面包着圆角，
那层圆角就是容器本身。

参照物侧再把三条硬事实对齐（`docs/reference/flclash-desktop/flclash-desktop-dashboard.png`，
2265×1665 真实桌面截图）：① 侧栏是「字形 + 文字」的目的地行，行内不出现任何旗帜；
② 活动出口的身份块是一枚 **squircle 底 + 两位国家代码文字**（实测 "US"），不是旗帜图；
③ 连接表行内是**系统 emoji 旗本身**，没有任何圆形容器。也就是说圆形/圆角旗容器在本仓
是自造物，参照物从来没有过——与前两轮结论一致。

本仓据此换掉资产族本身，而不是再调一次组件圆角：

- **资产**：`crates/infiltrator-shared/assets/flags/<code>.svg` 统一为 `0 0 640 480` 的
  **4:3 全出血直角矩形**。255 面取自 [flag-icons](https://github.com/lipis/flag-icons)
  `flags/4x3/`（MIT，许可文本 `LICENSE-FLAG-ICONS`）；flag-icons 未发布的 `ac`/`ea`/`ta`
  三面由 Twemoji 原作（CC BY 4.0，`LICENSE-GRAPHICS`）按同一矩形重新铺板。目录说明
  `flags/NOTICE.md`、根 根第三方声明文件 §6、`asset-catalog.json` 的
  `license`/`license_path`/`source` 与 About 页归属文案同批更新。
- **组件**：`FlagIcon` 画 4:3 盒（高 = `size * 3 / 4`，`image-fit: contain`、不 clip、
  不圆角）；`CountryChip` 取 `side * 3 / 4` 且**不声明任何圆角**；
  `LumenTheme.country-chip-radius` 作为退役符号删除。b185 的「不超过边长八分之一」随之
  零容忍化——资产自己都不带圆角了，任何容器圆角都是重犯。
- **门禁**（SLINT-020 第三版）：持旗容器声明**任何**圆角即违规；`FlagIcon` 必须是 4:3；
  `CountryChip` 必须取 `side * 3 / 4`；`country-chip-radius` 回到 theme 即违规；清单中
  每个 SVG 的 viewBox 必须是 `0 0 640 480` 且不得含 Twemoji 圆角模板曲线。
  planted 自测保持 **24/24 OK**（三条旧判据按新规则改写、新增四条负例），`--mode enforce` findings=0。
- **像素回执**：整批重采发布 `20261110T220049Z_7c92aaf890f3_dirty_2457766`
  （**199/199，358s**），**27 帧 PNG** 变化（外加 `manifest.tsv`）且全部是带旗帜或带归属文案的帧（`overview-*` 13、
  `proxies-*` 6、`about-*` 4 归属文案、`sidebar-collapsed-*` 2 与
  `proxies-search-highlight-*` 2），其余 172 帧逐字节不变。旗面指纹：可见红盒仍为
  **38×28**，但顶行跨度 **32 → 38px**（内缩 3 → 0），即四角完全方化；
  `space_utilization --enforce` 199 帧 **violations=0**、`receipt-freshness 0`、
  Slint 侧身份门 0 违规、Slint nextest **386/386**。

顺带记一条工程教训：这批的第一次采集用了工作树里上一轮未提交的**矩阵重排**版本
（`scripts/capture_slint_scenarios.tsv` 被生成器重写成 4 列、12 个手工附录场景行丢失），
采集脚本按「不在清单里即过期」删掉了 12 个已发布帧。发现后把该文件还原到 HEAD 再重采：
199 行齐全、27 帧 PNG 变化、三条判据全绿。生成器的「保留手工附录行」需要修在它自己身上
（这批不再夹带矩阵重排），记在 b189。

## 16. 浅色页身不再贴着白卡：一张地面一条事实（b190）

b190 收口 Iced 密度缺口时，同一张 `rules` 页出现一个不可能的结果：**暗色帧量得尾空
0.0%，浅色帧量得 13%**，而两帧的卡画在完全相同的几何上。逐像素定位到根因不在 Iced，
在共享契约的浅色令牌：`skin_core(Light).canvas` 是 `#F8F9FA`，白卡 `#FFFFFF` 与它只差
**2.1 个 L\***（通道和 18）——比判定器的地面容差还小，于是卡被读成地面，页身看着就是
一张整片白纸。这不是判据噪声：把同一帧放大看，浅色下卡与页**确实**看不出边界。

对照本仓自己的 Slint 台阶（`slint-surface-ladder-guard` 早已写下「参考产品用**白卡压在
暖灰地面**上；下沉的卡没有这种分离」）：Slint 浅色页身是 `zinc-0` = **`#EFF0F3`**，
实测卡/地面差 **5.2 个 L\***（1170px 长边上肉眼可分的边界）。也就是说产品的分离语言
早就在 Surface 上落地了，只有共享契约的浅色地面令牌没跟上——同一事实两份数字。

收口方式（求同，不是重新设计浅色皮肤）：

- `skin_core(Light).canvas` 与 `SurfaceElevation::Background` 的浅色项一起落到
  **`#EFF0F3`**（= Slint `zinc-0` 浅色，页身档）；Bevy 侧那份同值拷贝（`window_bg`）
  同批对齐，三处的差由既有的 `design_token_tests` 交叉断言钉住。
- 契约新增**逐皮肤**判据：卡必须至少高出自身地面 **3 个 L\***（M3 的 tone 就是 L\*，
  故判据与调色板同一单位）。原判据只有暗色一档（ratio ≥ 1.20），浅色从来没有等效判据，
  这正是 `#F8F9FA` 能活到今天的原因；新判据在改动前实测 **2.12** 会咬。
- 判定器侧一行不动：它本来量的是像素，"看不见的卡"是产品缺陷，不是测量口径问题。

实测（Iced 184 帧整批，`20261110T233706Z_c8002f35e646_dirty_3024494`）：`rules-light`
13.0% → **2.3%**、`overview-light` 13.2% → **6.8%**、`logs-light` 10.8% → **1.7%**、
`connections-light` 8.2% → **0.0%**，欠账台账归零。

**存异一条**：Iced 的 `connections` / `logs` 两个契约形态在 Slint 是独立页，在 Iced 仍是
runtime 页里的区块；本批只把卡栈缝收到预算内（`SP_LG` → `SP_MD`，求同 Slint 同帧的
3.5%），真正补齐这两张页是未开工的产品工作，不写成结论。

## 17. 代理节点网格按页宽连续裁决（b212）

上游 `lib/common/layout.dart::getProxiesColumns(viewWidth, proxiesLayout)` 发布
`max(ceil(viewWidth / 250), 2)`，再由 `tight / standard / loose` 三档用户偏好做
`+1 / 0 / -1`；`lib/views/proxies/tab.dart` 用 `max(width - 32, 0)` 把页身可用宽
传给它，网格自己另付 8dp 横向 seam。真实代理页场景图（3193×1949，约 1.5 缩放）
实测 5 列、每卡 371px ≈ 248dp，与 250dp 门槛同构。

本仓此前由 `ViewportTier::proxy_grid_columns` 在四个窗口档上固定
`1 / 2 / 3 / 4` 列：`proxies-dark`（1180×780）页身 910px 只画 3 列、每卡约
282px，而按上游标准档规则应为 4 列；`space_utilization` 对同帧实测 `unused 51.3%`
（`content` 形态 82% 上限放行，但并非同形态参考产品之上的密度）。b212 的求同是
把 `250dp` 最小卡宽、`32dp` 页身边距与两列地板收进
`infiltrator_contract::responsive_viewport::proxy_node_columns`，经 v14 导出
`proxy_grid` 与生成器投影为 `ViewportLadder.proxy_columns(form, page-width)`；
存异是**不引入上游三档偏好**：本仓 `OutboundLayout` 只有 Grid / CompactList 两个真实
语义，手机/底栏档继续以 typed `ProxyGridForm::Stacked` 保持单列可读，桌面档才按页宽
连续计算。因此 1180 标准帧从 3 列变 4 列，720 与 400 档仍单列，既不照抄 250 字面量
进 UI，也不让「窗口档」冒充「页身可用宽」。Iced 在 b213 已接入共享页身宽规则；
Bevy 于 b225 接入 `ProxyGridGeometry`，其场景、动态 resize 与虚拟列表都消费
`proxy_node_columns(form, page_body_width_px)`，旧 `Breakpoint 1/2/3/4` 与
`compact_view: bool` 同批退役。Bevy 192 帧像素仍由另一会话持有，源码接线与
像素验收分别记账。

## 18. 页身宽只有一个 owner，Iced 接入双网格（b213）

b208 的 Profiles 规则与 b212 的代理节点规则都要求「页身可用宽」，但本仓此前
只有窗口宽、导航列宽、页边距三个分散事实，没有一条共享减法。b213 在
`ViewportTier::page_body_width_px(window_width)` 一处发布：宽档扣掉侧栏列与
双侧 `content_padding_px`，底栏档只扣页边距；Iced 的 Profiles / 代理节点网格和
Slint 生成函数都消费同一结果，不再各自推导。

真实诊断抓到一次「照抄公式会把页面做坏」：720×480 页身 608px 按 300dp 规则
数值上能放 2 列，实测却把配置路径、按钮和标签挤成竖排碎字。故 v15 给
`profile_grid` 也引入与代理页同构的 typed `ProfileGridForm::{Stacked, Desktop}`：
带标签侧栏的桌面档按宽度算 1–3 列，底栏/图标轨档固定单列。Iced 新增
`profiles-grid-{standard,compact}` 两帧，用 typed `CaptureSection::ProfilesGrid`
把配置列表滚入视口；标准帧实测 2 列且卡高按内容收缩，720 帧单列无碎字。
Slint 同步按 `labelled_sidebar` 选择形态，其 720 帧整批重采后不再挤成双列。

同一批修掉 Iced 台架的一条真缺陷：`INFILTRATOR_CAPTURE_SCENARIOS` 过滤的
诊断批此前会逐帧覆盖正式 PNG，最后把 190 行 manifest 截成 4 行。现在
`PUBLISH_CORPUS=0` 令过滤批与矩阵覆盖批只写 `.evidence`，只有完整 canonical
矩阵才发布图像与索引；`iced-observation-routes-guard.py` 加 planted 自测钉住
「诊断能覆盖正式语料即违规」。这是回执管线本身的空间/身份正确性，不属放宽
任何页面阈值。Bevy 仍消费 tier 查表，文件所有权落地后由跨表面切片收口。

## 19. 折叠偏好只能改列宽，不能改页面形态（b216）

上游 `lib/views/navigation.dart` 与 `plugins/prism_ui/lib/sidebar.dart` 把
`sidebarExpanded` 只用于一处：`expanded ? _expandedWidth(220) : _compactWidth(48)`。
页面形态来自另一个事实——`lib/common/layout.dart::getViewMode(width)` 只看窗口
宽度，`DashboardLayout` / `proxies/tab.dart` 也从 `LayoutBuilder` 的页身可用区
推导密度与列数，**没有任何一处把折叠偏好读成「这是手机页」**。因此上游收起
1200px 桌面的侧栏，得到的是更宽的桌面仪表盘，而不是手机单列页。

本仓 `lumen-app.slint` 此前写 `compact-layout = android-platform || !labelled-sidebar`，
而 `labelled-sidebar` 来自 b172 的 `preference × tier × touch` 变体表——它跟着用户的
折叠偏好走。于是桌面档（`expanded`/`ultra`）一旦点折叠钮，`compact-layout` 翻成
`true`，整页切到手机形态：代理节点与配置卡退回单列、页边距 30→16、页身上下空气
46/72→38/0。b173 发布的 `sidebar-collapsed-standard` 帧正是这个错误形态的像素证据
（回执 `page_margin_px=16 shell_bottom_air_px=0`），而同一窗口在 Iced 上仍是
`margin=40 bottom_air=40` 的桌面页——两个表面对同一个动作给出两种页面形态。

`SPACEUTIL-006` 当年修掉的是反方向的混淆（密度布尔决定挂哪种导航），这一条是它
没覆盖到的另一半：导航偏好决定页面密度。收口按自底向上顺序一次做完：

1. **契约导出补一个只读档的事实**：`page-structure.json` 的 `tiers[].labelled_sidebar`
   本来就是「这一档有没有带标签列」，与偏好无关；生成器新增
   `ViewportLadder.labelled_sidebar(tier)` 直接投影它，不新增第二份数字。
2. **表面分两个事实**：`LumenTheme.labelled-sidebar`（偏好感知，仍由侧栏决定要不要画
   标签）与 `LumenTheme.desktop-page`（只读档，页面密度唯一来源）。`compact-layout`
   改读后者，两个网格页的 `ProxyGridForm` / `ProfileGridForm` 同批改读后者。
3. **守卫钉死方向**：`slint-viewport-tier-guard.py` 的 `page_density_findings()` 要求
   生成查表存在、`desktop-page` 只读 `labelled_sidebar(tier)`、`compact-layout` 不得
   再出现 `labelled-sidebar`、两个网格不得用偏好选形态、侧栏仍须用偏好感知事实；
   三条 planted 负例（把 `compact-layout` 换回偏好读法、把网格换回偏好读法、删掉
   生成查表）实测都会咬。

同批修掉取证台架的一条真缺陷（回执完整性，不是判据）：三个矩阵脚本都用
「第一次数到 >5000 字节」当作 niri 写完的信号，随后立刻 `sha256sum`。niri 的
`screenshot-window --write-to-disk true` 会在文件仍在刷新时就返回，于是索引可以
描述一个已经不存在的字节数——实测 `sidebar-collapsed-compact` 帧最终 75920 字节，
manifest 却记 65536 字节（恰为一个 pipe buffer）并给出对应的旧摘要，
`receipt-freshness-guard.py` 因此报 1 条违规。修法是把「等到尺寸连续两次轮询不再
变化，再一次性读出字节数与摘要」收进 `scripts/parity/capture_attempt.sh` 的
`capture_settled_image()`，三处调用点共用同一实现；`scripts/parity/test_capture_attempt.py`
新增 4 项自测（settled 摘要==最终字节、增长中的文件必须等到落定、缺失/过小文件
拒收、三个矩阵都不得再各自 `stat`/`sha256sum`）。

## 20. 页身左右留白：参考物只付一次 16dp，本仓付了四次不同数额（b217 实测立项）

用户要求「因地制宜订立设计标准：最多只能留出多少比例的空间」。侧栏与页尾已经
有判据与守卫，**页身左右留白（page gutter）至今没有任何判据量过**，实测发现它
同时违反「每个事实只有一个真源」与「不超过参考物」两条。

**上游真源（pinned `be847366`，逐文件核对）**：页面左右留白在所有页面、所有视口
档都是同一个 16dp，且**只付一次**——

| 上游位置 | 源码事实 |
| --- | --- |
| `lib/views/dashboard/dashboard.dart` | `EdgeInsetsDirectional.fromSTEB(16, contentTopPadding, 24, …)`，随后 `room = constraints.maxWidth − base.horizontal − gap*(columns−1)`，即页宽减一次左右留白后直接分列 |
| `lib/views/proxies/tab.dart` | `getProxiesColumns(max(width - 32, 0), …)`，32 = 左右各 16；`getProxiesColumns` 本体只做 `max(ceil(viewWidth/250), 2)`，不再减任何 inset |
| `lib/views/proxies/list.dart` | `const _groupInset = 16.0`，`width = max(maxWidth - _groupInset * 2, 0)`，`SliverPadding(horizontal: _groupInset)` |
| `lib/views/profiles/profiles.dart` | `static const _horizontalPadding = 16.0`，`_columnsFor` 传 `width - _horizontalPadding * 2` |
| `lib/views/dashboard/layout.dart` | `dashboardMobileSideOf(width) => max(16, (width - 560) / 2)`——手机档仍是 16 起步，只在超宽时居中 |

真实场景图逐像素复核同向：仪表盘页身左缘墨迹起于侧栏右缘 +30px（÷1.95 = **15.4dp**），
手机仪表盘图卡片左缘 48px（÷2.94 = **16.3dp**）、右缘 47px（**16.0dp**）。

**本仓现状（三份互不相等的副本）**：

| 表面 | 页身左右留白 | 真源位置 |
| --- | --- | --- |
| Slint 桌面 | **30u** | `theme.slint` 的 `page-margin-desktop` |
| Slint 紧凑 | 16u | `theme.slint` 的 `page-padding` |
| Iced | **16 / 24 / 40 / 48px 按档递增** | `responsive_viewport::ViewportTier::content_padding_px()` |
| Bevy | 12px | `shell_scene.rs` 的 `UiRect::horizontal(rem(space::S12))` |

同一个 1180×780 窗口实测：Slint 页身 880px、Iced 页身 860px（本批 b216 的两表面
回执普查），而参考物在 960px 页宽里只让出 32px。也就是说 Iced 在 expanded 档
**每侧多付 24px、两侧多 48px**，Slint 每侧多 14px——这些像素既不是内容也不是
已裁决的呼吸位，是纯粹没人审过的横向死地，正是用户指出的「既不紧凑也不美观」。

**还有一处双重扣减**：`proxy_node_columns()` 在 `page_body_width_px()`（已经扣过
2×`content_padding_px`）之后**再扣一次** `PROXY_GRID_INSET_PX = 32`，Slint 生成
查表同样如此。上游那个 32 就是页身留白本身（`max(width - 32, 0)` 里的 width 是
未加左右 padding 的页宽），本仓把它当成「留白之外的另一个 inset」，于是
1180 expanded 下 Iced 的网格实际拿到 `1180−240−80−32 = 828px`，参考物拿到
`1180−220−32 = 928px`。`profile_columns()` 没有再扣，两个网格对同一条留白规则
的口径也不一致。

**裁决与收口（`SPACEUTIL-018`，b218 交付）**：

1. 契约一处发布 `PAGE_PADDING_PX = 16`（依据上表五条源码与两张场景图），
   `ViewportTier::content_padding_px()` 的按档 ramp 同批退役，不留兼容轨道；
   `page-structure.json` 升 v16 导出该常量，生成器与判定器读同一份导出。
2. `proxy_node_columns()` 删除第二次扣减：入参就是页身宽，`PROXY_GRID_INSET_PX`
   连同导出键一并清退；`profile_columns()` 口径不变，两个网格从此同解。
3. Slint `page-margin-desktop` / `page-padding` 合成为生成的单一 `page-padding`，
   Iced `view_root` / `sidebar_chrome` / `rules_list` 与 Bevy `shell_scene` 都只读
   契约值；每帧仍由 `CAPTURE_CHROME` 的 `page_margin_px` 自述，判定器不记常量。
4. 这是**跨表面布局身份变化**，两表面整批重采复判：Slint 199/199 发布 run
   `20261112T052158Z_79bf9f28e20f_dirty_1094075`、Iced 192/192 发布 run
   `20261112T052858Z_79bf9f28e20f_dirty_1173512`，两侧 `--enforce` 都 **violations=0**、
   欠账台账两侧 `frames` 保持为空。实测页身宽（窗口 − 导航列 − 2×留白）逐档
   **完全一致**：expanded `908`、medium `624`、compact `368`、折叠桌面 `1084`；
   此前是 Slint 880 对 Iced 860。`dense` / `framed` 页没有因页身变宽暴露新的页尾
   地面（expanded 档 124 帧 `unused` 中位 3.9%，与上一批逐字相同），因此未动用 §5 的任何一种收口，
   也未放宽任何上限。
5. Bevy 文件与像素仍由另一会话持有（`FLPROFILES-002` 同一纪律）：其
   `shell_scene.rs` 的 12px 副本本批未碰，留作该表面接线时消费同一导出值。
6. 网格的第二次扣减已同批清退：`proxy_node_columns()` 不再减 32，
   `PROXY_GRID_INSET_PX` 与导出键 `proxy_grid.page_inset_px` 一并删除，契约与导出
   各有一条断言拒绝它们回来（`page_structure.rs` 的
   `the proxy grid must not carry a second page inset` 与
   `the page gutter is not a tier fact`）。列数在采集窗口上逐个不变
   （expanded 4 列、medium 3 列、手机 1 列），所以这次是**留白归位**而不是排布改道。

## 21. 旗面不是字形：上游用 emoji，本仓改用同一份矩形矢量清单（b219）

用户第五次附同一张裁图（「一般国旗都是方形 最好不要做这种圆形旗帜图案容器」）。
前四轮（b177/b185/b188/b193）都只审了 Slint 的容器与资产，并据此写下「四个表面都没
有圆形旗帜容器」的结论；本批复量**本仓自己的已发布 Iced 回执**，该结论与像素矛盾：

| 帧 | 实测 | 读法 |
| --- | --- | --- |
| `proxies-dark` HK 节点 | 红墨迹盒 **18×13**、顶行跨度 **15/18**、四角全空（既非 4:3 也非直角） | 波浪边 emoji 旗，不是直角矩形 |
| `proxies-dark` DMIT 节点 | 蓝色墨迹盒 **18×17**（长宽比 1.06）、四角全空而中心实、填充率 **0.703**（圆≈0.785、方≈1.0） | 字面圆形 🌐 地球 emoji 占据旗位 |
| `profiles-aggregator-preview-standard` | 聚合行以 `🇭🇰` 起头 | 行模板 `{flag}` 槽直接吃 emoji |

根因不是容器形状，而是**旗面是文本字形**：`country_flags::node_flag_emoji` 把
`Region::emoji()` 交给 `role_body_text`，于是旗面的形状、颜色与「是否存在」全部由宿主
emoji 字体决定（本机 Noto Color Emoji、Windows Segoe UI Emoji 各画各的；缺字体时退化为
方框或两枚区域指示字母）。

**上游事实（pinned `be847366`）**：FlClash 行内同样是 emoji 或两字母文字徽记
（`prism_ui` 的 `CommonTargetIcon` / `GroupMonogram`），并且随包 `Twemoji.Mozilla.ttf`
把字形固定下来。因此本批**不照抄**上游的 emoji 方案——那正是被驳回的形状——而是求同：
让 Iced 消费 Slint 早已使用的同一份 `asset-catalog.json` 4:3 直角矢量清单，几何与
`FlagIcon` 逐字相同，形状从此与宿主字体无关。

**同一轮实测的架构缺陷（三份地区真源）**：「这个节点属于哪个地区」在契约
`GeoRegion::from_name_heuristic`、shared `country_flags::match_region`、domain
`COUNTRY_DEFS`/`region_flag` 各有一份别名表。对 146 个真实节点名做差分，**65 个读数
不一致**：41 个是覆盖缺口（CH/SE/NO/FI/IT/ES/IE/PL/UA 九国在契约表里根本画不出旗，
`HKG`/`hong-kong`/台中/高雄/福冈/圣保罗等别名只存在于其中一份表），24 个是表示差异
（`None` 对 `Other`、DIRECT/REJECT/GLOBAL 是否算地区）。本批把真源合并到契约一处，
新增 typed `NodeRegion`（地理旗 vs 三个恒无旗的路由目标），并退役全部 emoji 入口。

**据实登记的下一半（`FLAGCODE-001`）**：契约词表只有 37 个变体，而清单有 258 面旗。
探针实测 258 面里 **222 面永远解析不到旗**（AC/AD/AF…，含葡萄牙、墨西哥、智利、新西兰
等常见机场名）。emoji 时代这个问题被宿主字体掩盖（任何国家都能画出个字形）；换成矢量
清单后它第一次变成用户可见缺陷——那些节点拿到方形「—」代码板。收口必须在契约词表
裁决，严禁在 UI 侧补第二份表。

## 22. 侧栏栈尾空气也是可量比例（b226，b228 双场景复裁）

上一轮侧栏判据只量 `chrome_band`（栈后是否混入统计卡）与 `void`（导航行之间的最大
死带），却没有量「最后一条导航带之下那一整片地面」。这意味着一个侧栏即使只画一两行
导航、把下半列全部留空，也能通过现有两项；用户说的「那么多空隙」在判据里恰好是盲区。
两张 tracked 桌面场景给出可复跑答案：仪表盘 `flclash-desktop-dashboard.png`
为 2265×1665、侧栏边界 x=429、导航实带 y89–579，行间最大死带
**51px = 3.06%**、栈尾合法呼吸位 **1085px = 65.17%**；代理页
`flclash-desktop-proxies.png` 为 3193×1949、侧栏边界 x=432、同样的导航栈
y89–579，行间最大死带 **68px = 3.49%**、栈尾合法呼吸位
**1369px = 70.24%**。两图栈后非导航墨迹均为 **0%**。

求同存异的裁决不是把尾空填成内容：FlClash 自己在较高的代理页里保留 **70.24%**
的栈尾，统计事实仍只能属于仪表盘，侧栏不得为了「看起来满」塞品牌块或状态卡。
真正要补的是**比例上限，而且必须复量全部 tracked 场景**：b226 只量 1665px
仪表盘图，把 `page_structure.json` v17 导出的
`navigation_limits.nav_tail_ground_limit_pct` 定成 **68%**；这会把同一导航栈在
1949px 代理页的合法 **70.24%** 呼吸位误判为违规。b228 把契约导出升 **v18**、
上限改为 **73%**（最坏真实场景 + 2.76pp 有界余量），判定器逐帧发布
`nav_tail_pct`，超过上限且未登记只减欠账即拒判。`flclash-sidebar-void-guard.py`
现在用同一把 ink 尺同时复量两图的 3.06% / 3.49% 行间缝、65.17% / 70.24%
栈尾与 0% 栈后墨迹，并钉住两张 PNG 的 SHA-256；planted 负例覆盖删除代理图、
退回 68%、摘要漂移、上限放宽与契约常量失配。现存 Slint 211 帧最差尾空
**47.0%**、Iced 192 帧最差 **40.1%**，两表面均低于两张参照物且无需登记豁免。

## 23. 旗面清单的 258 个码全部成为契约地区（`FLAGCODE-001`）

b219 把 emoji 旗换成 258 面 4:3 直角矢量后，契约 `GeoRegion` 仍只有 37 个变体，探针实测
**222 面旗永远解析不到**，葡萄牙、墨西哥、智利、新西兰等节点只能拿「—」代码板。本批不在
Iced/Slint 页面里补表，而让 `asset-catalog.json` 的 `flag.<code>` 同时生成契约码表：
`NodeRegion::Catalogued(&'static str)` 是清单码的 typed 身份，命名 `GeoRegion` 继续只拥有
本地化名称与自然语言别名。节点编号前缀（`PT-Lisbon-01`、`MX 01`、`[NZ] Auckland`）才触发
catalog 解析，避免 `BE Fast` / `IT Relay` 这类普通英文词被两字母 ISO 码误判。

同一批把 domain `normalize_country_code` / `extract_country_code` 改为消费 `NodeRegion`，
删除私有 `matches_alias` 地区匹配器：`COUNTRY_DEFS` 只保留显示词，不再回答「节点属于哪个
地区」。`flag-plate-guard.py` 新增契约码表等式、258 码覆盖测试与 domain 单 owner 三条门禁，
自测 **9/9**；contract/shared/domain/presentation lib nextest **1710/1710**，`cargo check`
覆盖 Iced/Slint 全目标。本批只改地区解析，不改任何旗资产几何或已发布 fixture；4:3 直角、
零圆角容器与方形 fallback 代码板保持不变。
