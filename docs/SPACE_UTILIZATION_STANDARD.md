# 跨表面空间利用率强制标准

> 层级：L2

本文是 Iced、Bevy、Slint、Android Compose 四类 UI 表面共享的**空间利用率**唯一强制标准。
它裁决一件事：一帧画面最多允许留出多少比例的空地，才算利用率达标。

判定入口唯一：`scripts/parity/space_utilization.py`。本文只裁决规则与阈值；
任何表面、任何页面都不得自行声明「这页留白是合理的」而绕过测量。

## 1. 为什么必须有比例上限

留白不是缺陷，超标留白才是。本仓过去只能逐页争论「这块空地是不是合理」，
没有共同标尺，于是同一类缺陷反复出现：

- 列表页主卡只按行数取高，卡下压着 51% 页面底色（已由 b134 的
  `CardHeight.page-body` 框架契约收口）；
- 概览卡网格用 `horizontal-stretch` 定宽，同排卡片内沿彼此错开（b133 收口）；
- 侧栏选中胶囊是 160u 定宽盒，标签墨迹只到 85u，胶囊自身右半 36.2% 是空气。

三条同源：**页面把「内容少」当成了「可以不管几何」**。比例上限把这件事变成
一次测量，而不是一次审美争论。

## 2. 唯一测量口径

标准只认像素，且必须排除三类假阳性，否则测量会奖励错误行为：

1. **测量盒是页身，不是窗口**。shell 自己保留的页面空气在盒外，永不计入浪费。
   **这份空气是「外壳档」的事实，不是「有没有底栏」的事实**：桌面外壳留上 8u、
   下 72u 的页面呼吸位；紧凑外壳**一点不留**——它唯一的底部空间属于浮动底栏，
   而底栏由外壳的列自己预留那一条带（`dock-band-height`），没有底栏的页面
   就不得替它留。空气真源见
   `crates/infiltrator-slint/ui/components/app_body.slint` 的
   `page-content-padding-top` / `page-content-padding-bottom`。
   （b150 实测修正：旧规则把底部空气挂在「底栏未预留」上，于是紧凑档每个
   无底栏页面都在最后一块下面压着 72u 死地——720×480 档是 480 行里的 72 行，
   12.9%，正是 framed 页框悬在空带上方的原因。改正后三帧 tail 由 12.9% 归 0.0%。）
   否则填满视口的列表卡会被判成违规。
   **b154 追加**：测量盒的具体像素不再由判定器持有。外壳在每帧采集末尾把自己的
   几何写进该帧的 `CAPTURE_CHROME` 回执行（typed 定义与线格式在
   `infiltrator_contract::shell_receipt`，由 `crates/infiltrator-slint/src/
   shell_receipt_host.rs` 从活的窗口读回），判定器逐帧解析后据此建盒：换皮肤、
   换缩放或改外壳-padding 都不会让测量盒与像素脱钩。帧若没有这行，判定器直接
   报违规而不是用记住的数——回执是判据的来源，不是判据的注释。
   **b173 追加**：回执行从 9 键扩到 12 键，新增 typed `sidebar_expansion`、
   `touch_surface` 与 `sidebar_toggle_px`。它们不是画面里猜出来的阶段，而是外壳
   从设置 owner 与表面身份读到的决议输入；判定器用前两个从导出表选择唯一合法
   sidebar variant，再判 `chrome_band` / `void` / `leading`，并用第三个把折叠钮
   自己的带排除在「导航起笔」之外。因此桌面折叠帧能证明 64px rail，却不可能
   冒充 expanded 240px 列；缺任一键仍按「帧没描述自己」直接判违规。
   **b152 追加**：判「哪一档」不得由判定器自己比宽度。档界、该档是否给出带标签
   侧栏、侧栏列宽与逐档兜底上限全部来自契约导出物
   `crates/infiltrator-contract/assets/page-structure.json` 的 `tiers[]`；
   判定器与 Slint 读同一份导出（见 §3.2），因此「测量盒」「页边距」「shell 空气」
   三者不可能各自认同一个不同的窗口档。
   **外壳档的判据是导出阶梯的 `labelled_sidebar`**（b152）：有带标签侧栏的档
   付空气，没有的档不付。判定器不再自己记「宽度 ≥1000 就算桌面」，见 §3.2。
2. **页面底色的参考取自页身左缘的 margin 条**，绝不取自盒内出现最多的颜色。
   填满视口的卡是它自己那一帧的多数色，用它当参考会把「卡外的地面」读成内容、
   把「空卡」读成地面。
3. **一行只有整行都是地面才算空地**，且内容跨度只认长度 ≥3 行的实带。
   窗口边缘的单行噪声不得把一帧死页伪装成满页。

两个指标，一帧取较差者：

- `tail`：最后一个内容实带之下的地面占页身比例；
- `band`：内容跨度内部最大的连续地面带占页身比例。

侧栏另用自己的轴。**桌面侧栏对标 FlClash 真实场景图与开源源码**
（`docs/reference/FLCLASH_DESKTOP_REFERENCE.md`，须由 L1/L2 公共入口向下导航）：
侧栏是纯导航，选中态是**全宽 8% 淡底 + 3dp 指示条**，右侧空地是合法呼吸位，
不是浪费。因此不再测量「胶囊死空气」——把结构性的全宽选中带当成内容盒量
`pill_tail` 是判据本身错了。真正该量的是：

- `chrome_band`：导航栈之下的非导航区块（状态卡 / 品牌块 / 版本行）占侧栏高度比例；
  这些事实已有标题栏与仪表盘卡作为唯一真源，再放一份到侧栏才是浪费；
- `void`：导航竖栈内部最大地面带占窗口高度比例。

## 3. 阈值（因地制宜）

阈值按档给定，不是一刀切，也不是逐页特批。

档位名与边界都来自共享契约的导出阶梯（§3.2），判定器不得另立档名：

| 档位 | 宽度区间 | 导航形态 | `unused` 兜底上限 | 采集帧 |
| --- | --- | --- | --- | --- |
| `compact` | <600 | 底部导航（无侧栏列） | **8%** | 400×800 |
| `medium` | 600–839 | 图标轨（rail） | **8%** | 720×480 |
| `expanded` | 840–1199 | 带标签侧栏 240 | **12%** | 1180×780 |
| `ultra` | ≥1200 | 带标签侧栏 280 | **12%** | — |

兜底上限仅用于**页面形态未知**的帧；真实上限一律由 §3.1 的形态表给定。

### 3.1 阈值按页面形态给定（b149 修订，这是「因地制宜」的核心）

**b144 的一刀切阈值（desktop 12%）已被实测推翻**：对 FlClash 桌面真实场景图
逐像素测量，其**代理页页身 80.6% 是空地**（内容止于 y357 / 1849px 页身），
其工具页同样按内容分组列表。一刀切阈值会把「顶级开源产品」自己判成严重违规，
说明它把**不同形态的页面混在一个数里**。

真正的分界是**页面形态**（typed 契约 `infiltrator-contract::page_structure`）：

| 形态 | 含义 | `unused` 上限 | 依据 |
| --- | --- | --- | --- |
| `framed` | 一张卡框住页身，列表在框内滚 | **4%** | b134 框架契约：卡高即页身，卡下不得有地面；4% 为卡自身描边/投影容差 |
| `dense` | 密铺卡片网格（概览） | **12%** | 参考产品仪表盘实测页身空地 **0.0%** |
| `content` | 按内容排布，止于最后一块 | **82%** | 参考产品同形态页面实测 80.6%（代理页），留 1.4pp 余量 |

形态由 capture manifest 的 page slug 决定（`logs` 与 `logs_workbench` 是**两种**
形态），映射表由 contract 导出为 `crates/infiltrator-contract/assets/page-structure.json`，
判定器只消费导出物，不保存第二份数字；页面形态与上限的裁决见共享契约的 `page_structure` 模块（L2 crate 公共契约）。

| 侧栏 | 上限 | 依据 |
| --- | --- | --- |
| `chrome_band` | **0%** | FlClash 桌面侧栏实测只有导航行（边界 x=429，5 个行带 y89–579，下方无非导航区块）；统计数据全部属于仪表盘卡片 |
| `void` | **12%** | stretch 空气必须留在栈尾，不得长在导航行之间；b164 起桌面四种导航形态统一由契约导出 `NavigationSeparator::None`，不画分组标题与它的额外上空气 |

侧栏形态与按钮可用性由 `page-structure.json` 的 `sidebar_variants` 16 行封闭表
给定：`preference × tier × touch_surface` 唯一决定 `nav_form`、列宽、标签与
`toggle_available`。b173 的折叠纯外壳帧（standard/compact 双视口）发布
`sidebar_expansion=collapsed touch_surface=0 nav_form=rail nav_column_px=64`，
按同一上限判导航-only rail；侧栏顶部的展开/收起按钮是导航 chrome，不是统计卡、
品牌块或版本行，既不增加 `chrome_band`，也不因右侧合法呼吸位被误判浪费。

`sidebar_toggle_px` 不是记下来的行高：b173 起它由绘制按钮的侧栏自己测量
`absolute-position.y + height + rail-item-gap`，沿 `AppBody` 上传，b174 起再由独立
组件 `ShellReceipt` 投影进回执，实测 1180×780 为 **106px**。接线初版在侧栏局部坐标
里按 token 算出 `18 + 44 + 4 = 66`，漏掉了外壳把侧栏挂在 40px 标题栏之下这一偏移，
于是按钮字形那一行被当成第一行导航——那是虚假的确定性。判定器只用这个键确定导航
测量的起点，
`chrome_band` 的 0% 上限不变，因此按钮带既不会被误算进导航 leading，
也不可能借「chrome」之名塞进统计卡。

达标判定：`unused ≤ 该页面形态的上限` 且侧栏两项不超标。

### 3.2 视口阶梯只有一个真源（b152）

「这个窗口算哪一档」此前由**三处各写一套边界**回答：共享契约
`responsive_viewport` 600/840/1200（Iced、Bevy 读它）、Slint 主题私有阶梯
600/800/1120、判定器 700/1000。实测后果两条：

1. 判定器把 800～999px 宽度当成「无侧栏」，于是把 240px 侧栏列当成页身内容测量，
   测量盒、页边距与 shell 空气三者同时错位；
2. 同一个 1180×780 业务帧，Slint 判 wide、契约判 expanded，代理节点列数 4 对 Iced 的 3
   ——同一事实在两个表面两个答案。

现在的链路唯一：

```
infiltrator-contract::responsive_viewport（边界 + SidebarForm + 列数的类型真源）
  → export_page_structure → assets/page-structure.json 的 tiers[]
    ├→ scripts/generate_viewport_ladder.py → ui/components/viewport-ladder.slint
    │    （Slint 只消费生成物：档界、导航列宽、节点列数、labelled_sidebar）
    └→ scripts/parity/space_utilization.py（测量盒、侧栏轴开关、兜底上限全部读导出物）
```

- 表面侧的门禁是 `scripts/quality/slint-viewport-tier-guard.py`：
  `theme.slint` 的阶梯链**不得再写数字字面量**、必须逐个读生成档界；生成物必须由
  生成器产出（手改即判 stale）；生成的查表函数无人读即违规；`ViewportTier` 的变体名
  必须逐个等于导出的档 id（一名一实，禁止表面私词）。
- 判定器侧的门禁是 `space_utilization.py --self-test` 的 planted 阶梯：换一个导出边界，
  `tier()` 与测量盒必须跟着换；导出缺 tiers 或缺口（gapped）直接拒绝判定。
- **收口进度（b154）**：
  ① **判定器不再自己记外壳几何**：页边距、shell 空气、导航列宽与底部带改为随帧
  回执 `CAPTURE_CHROME` 发布，由画出这些像素的外壳自己声明，判定器逐帧读取；
  外壳若说不清，该帧直接不判并报违规，绝不回落到记住的数（`SPACEUTIL-007` 交付）。
  ② **导航栈的纵向范围随帧发布，图标轨轴同步开启（`SPACEUTIL-009`，b157 交付）**：
  64px 轨道里 8 行图标在 480 高窗口上合法地铺满整列，判定器要按「导航行之上/之下」
  区分导航与第二块内容，就必须拿到真实范围而不是猜。b154 实测过一条来源（尾部占位块
  的 y）：1180×780 帧报 **466px**，而末行墨迹实际画到 **488px**——占位块的位置不等于
  栈的范围，这种数字进线格式比缺数字更坏（实测 `chrome_band` 从 0.0% 变成 2.3%），
  故当时撤回并立项。b157 用**末行自己的下沿**把它做成可证明的事实：桌面帧 **502**、
  轨帧 **398**、横栏 **0**，逐个与发布批像素对齐；判定范围由 123 帧扩到 **184 帧**。
  ③ **栈底随帧发布后，70% 启发式整体退役（b157）**：回执第八个键
  `nav_stack_bottom_px` 由侧栏用**末行自己的几何**测得（b154 的尾部占位块教训：
  占位块的 y 不等于导航栈的范围），带标签列与图标轨共用同一条轴——栈底以上是
  导航，栈底以下只在颜色**连续越过列右缘**（标题栏、toast、遮罩这类窗口级悬浮层）
  时豁免；列宽与栈底由契约钉成一对事实（一有一无即报违规），判定器文件里不再
  存在任何窗口高度分数，`slint-viewport-tier-guard.py` 对两边同时设岗。

### 3.3 外壳自己的列也必须受比例约束（b153 新增，直接抄 FlClash 的裁决）

§2 把页身的空地交给形态阈值，但**外壳自己占掉的宽度**以前无人管：契约说
`medium` 档给 64px 图标轨、`expanded` 给 240、`ultra` 给 280，凭什么这三个数
不是随手写的？本节把依据写成一条可门禁的规则，来源是 FlClash 的两处真源，
不是审美判断：

- `plugins/prism_ui/lib/sidebar.dart`：`_compactWidth = 48`、`_expandedWidth = 220`、
  `_itemSize = 40`、`_iconSize = 24`、`_indicatorWidth = 3`；
- `lib/common/layout.dart`：`getViewMode` 的断点 **600 / 840**（与本契约前两档界
  完全相同），以及 `canExpandSidebar(viewMode) => viewMode == desktop` 的理由原文
  ——「笔记本档展开侧栏会让内容比低一档的手机布局还窄，所以标签等到桌面档」。

由此得两条上限，落在 `ViewportTier::max_nav_column_px()` 并由导出物
`tiers[].max_nav_column_px` 发布（判定器与 Rust 测试都只读导出，不再各自记数）：

| 形态 | 约束 | 本仓数值 | FlClash 对照 |
| --- | --- | --- | --- |
| 图标轨 `rail` | 不超过该档**最窄宽度**的 `MAX_RAIL_COLUMN_SHARE_PCT = 12%` | 64 / 600 = **10.7%** | 48 / 600 = 8.0% |
| 带标签列 `standard` | 同时受 `30%` 份额与**内容地板**约束：`lower_bound − COMPACT_MAX_PX` | min(252, 840−600) = **240** | 220，且只在 >840 展开 |
| 带标签列 `wide` | 同上 | min(360, 1200−600) = **360**，实际 280 | — |
| 底部导航 | 无列（0） | 0 | 手机档同样无列 |

**内容地板是关键那一条**：带标签列不得让页身比「低一档最宽的窗口」更窄。
`expanded` 档算出来正好是 840 − 600 = 240，与本仓既有列宽**分毫不差**——
这说明 240 不是巧合而是被规则逼出来的唯一值，也让「最多只能留出多少比例」
有了可核对的答案：**轨道档 ≤12%，标签档按内容地板（840 档恰好 28.6%）**。

门禁三处，缺一不可：

1. `infiltrator-contract` 的 `every_navigation_column_fits_its_tier_ceiling`：
   逐档断言 `nav_column_px ≤ max_nav_column_px`，并把四档上限钉成具体数字，
   改边界必须在这里一并辩护；
2. 导出测试 `only_the_forms_with_a_column_reserve_one`：导出物里每档的列宽与
   上限同时可见，且「有列」与「上限非零」必须一致；
3. 判定器 `space_utilization.py --self-test` 的
   `test_the_exported_column_never_breaches_its_own_ceiling`：读**真实导出 JSON**
   复算同一条不变量，另钉住「触摸面每档都是底部导航」。

同一批还把「谁决定挂载哪种导航」收敛为一个 typed 事实：契约
`ViewportTier::shell_navigation(touch_surface)` 是唯一裁决点，经导出与生成器
发布为 `ViewportLadder.shell_navigation(tier, touch-surface) -> NavigationForm`；
`compact-layout` 从此只管页面密度，不再兼职回答导航形态（这正是 `medium`
档以前画出手机横栏的根因，见 `SPACEUTIL-006`）。

**b170–b172 追加：用户折叠偏好与视口形态分开裁决。** FlClash 把
`sidebarExpanded` 存在 app settings，却只在 `canExpandSidebar(desktop)` 时让
它生效；`expanded` 是这两个事实的唯一投影，不是 UI 自己记住的布尔。共享契约
`shell_sidebar_preference` 因此发布四个 typed 身份：偏好、能力、呈现、intent，
并由 `resolve_sidebar_expansion` 一次穷尽折叠。desktop 档用户选择折叠时落到
既有 `SidebarForm::Rail`，列宽仍遵守本节 12% 上限；窄档与底部导航不受偏好
影响，也不伪造第三种侧栏宽度。b171 已把该裁决接到 settings 唯一持久化字段、
typed 命令与页面快照权威读回；b172 再导出 16 行封闭
`preference × tier × touch-surface` 变体表，让生成器和判定器读同一份解析
结果。表面 toggle 后续只能消费 owner readback、导出解析与 typed intent，
禁止 UI 用 `Option`/null 组合藏阶段。

### 3.5 仪表盘密度也按页身比例裁决（b166 新增，求同 FlClash）

FlClash 不把「侧栏有没有标签」当成仪表盘卡片密度的依据；它在
`lib/views/dashboard/layout.dart` 用 `DashboardDensity.of(size)` 从
**`LayoutBuilder` 给仪表盘页身的可用区**一次性裁决 `mobile / compact /
standard`，再由 `DashboardDensityScope` 发布给所有卡片。四条可核对的界是：
页宽 `<640` 为 `mobile`；四列页宽 `<1040` 为 `compact`；页高 `<824` 为
`compact`；其余为 `standard`。本仓把这些阈值和 typed 三态统一归
`infiltrator-contract::responsive_viewport`，经 `page-structure.json` 的
`dashboard_density` 导出；生成器投影为
`ViewportLadder.dashboard_density(page-width, page-height)` 与
`dashboard_is_compact(density)`，`home_desktop_grid.slint` 只消费后者。
页面不得拿 `labelled_sidebar(tier)`、窗口宽高或私有布尔再算一次密度；
`slint-viewport-tier-guard.py` 把这条链和三个变体名同时钉死。

这是「求同存异」：求同的是上游按页身可用区裁决、单一 scope 发布、卡片
被动消费；存异的是本仓现有 `dense` 只切换卡片排布与字号角色，不引入上游
`DashboardDensityScope.scale` 的第二套缩放（本仓缩放唯一真源仍是
`LumenUiScale.unit`），也不把 640/1040/824 直接照搬成窗口档界——它们量的是
**页身**，不是整个窗口。

**b166 实测修正**：短窗口的 `compact` 密度**不得**兼任 span 网格的 2 份额开关。
上游 `DashboardDensity.of(room)` 与 `dashboardColumnsOf(room.width)` 是两个独立
裁决：780px 高帧会得 `compact`，但 940px 页身仍得四列。本仓第一版把
`dense` 直接接 `dashboard_is_compact`，桌面帧因此从 4 份额退回 2 份额，右侧
留下半页死地；逐像素复核后立即拆成 `compact-metrics`（字号、间距、控件高）
与 `wrap-cards = card-columns < 4`（行列 placement）两个只读事实，并由
`slint-viewport-tier-guard.py` 钉死两者不得互相代答。这正体现标准的比例硬门：
不是按上游名字照抄，而是让上游的「短窗口紧凑」在本仓真实页身上仍然达标。

### 3.4 侧栏的横向起始边界同样受比例约束（b159 新增，SPACEUTIL-010）

§3.3 管的是外壳**占掉多宽**，此处管的是外壳**在自己那一列里让出多少**：导航行
第一个字形盒之前的空地（leading）。此前没有任何判据管它，于是它带着一个无人
复核的遗留值（2 × `rail-item-padding` = 32u，占 240u 列的 13.3%）。

- **规则**：带标签列必须发布自己的 leading（回执第九键 `nav_content_offset_px`），
  且不得超过该列已保留宽度的 `MAX_NAV_LEADING_SHARE_PCT = 8%`；图标轨在列内
  **居中**而非缩进，故其 leading 恒为 0，发布非零值即是描述该形态不拥有的几何。
  上限随导出物逐档下发为 `nav_leading_limit_px`（240 → 19u），判定器只比较两个数，
  不重算同一条比例。
- **依据**：参考产品的真实场景图实测字形盒起于面板左缘 **7.2dp**、标签墨迹 **44.1dp**
  （429px 面板对 220dp）；其源码推导为 `min(12,(48−40)/2) + (40−24)/2 = 12dp` /
  48dp，占 5.5%。本仓取场景的那一对（8u / 44u），8% 上限同时容得下两种读法，
  且仍然拒绝任何超过参考物的 leading。
- **声明必须等于像素（b161 新增）**：只把回执数字与预算比较还不够——一帧可以声明
  8px 而把字形画在 32px，仍然浪费列的左沿。判定器逐帧从像素读第一个**导航墨迹带**
  的起笔 x（`painted_nav_leading`），与回执 `nav_content_offset_px` 相差超过 4px 即报违规；
  带标签列完全没有导航墨迹也拒判。跨过列沿且两侧同色的窗口浮层（标题栏、通知条、
  模态遮罩）用与 `chrome_band` 同一条几何条件排除，**判定器不记标题栏高度**。发布批
  197 帧实测 123 个带标签帧的起笔 x 分布为 **8（120 帧）/ 9（3 帧）**，与声明的 8px 在
  1px 内吻合；planted 自测 41 → **45**（声明 8 画 32 必挂、无导航墨迹必挂、标题栏
  产品名不得当成第一个字形、该腿必须接入 `--enforce`）。
  **b173 追加**：折叠钮接进侧栏后，这条腿读到的不再是「第一个墨迹带」，而是
  「第一个导航墨迹带」。接线初版用 theme token 在侧栏局部坐标里猜出 66px 截断，
  而按钮带真正占到 106px：截断只削掉按钮上半，剩下的按钮字形那一行被当成第一行
  导航，读出的起笔是 **x38**，于是每一个带标签帧的 leading 都被判违规。b173 的
  收口修正把按钮带交回绘制者——侧栏用 `absolute-position.y + height + rail-item-gap`
  实测发布——截断落在按钮之下，起笔 x 分布变为 **9（5 帧）/ 10（118 帧）**，这才是
  目的行自己的 leading，与声明的 8px 在 2px 内。判定器的 planted 腿同步改到 106，
  自测 **45 → 50**。**b174 追加**：该事实的投影不再是外壳里的一段 struct 字面量，
  而是独立组件 `components/shell_receipt.slint`；外壳只把侧栏/页身已发布的 typed
  事实喂给它，投影与 wire 的对应关系因此可被单独阅读与门禁。
- **方向不可颠倒**：leading 是**列内**的，因此它是比例；列自身宽度是 §3.3 的比例。
  两个比例都不得用固定像素常量替代，否则换档即失效。

同一原则适用于**仪表盘列数**（b160，`SPACEUTIL-008`）：`overview_card_columns`
（1/2/2/3，包裹流每行卡数，按档）与 `dashboard_grid_shares`（2/4，span 网格份额数，
按页身宽度，参考产品 `dashboardColumnsOf` 的 860 界）是两个 typed 事实，由契约枚举
`DashboardColumns` 区分并导出；任何一处再把其中一个 untyped 数字读成另一个都是
重犯「同一事实两个真源」。轨/手机档固定 2 份额不是第二个宽度规则，而是短窗口下
4 份额会把末卡挤到窗口外的形态裁决（b153 实测）。

## 4. 内容少不是豁免

这是本标准最容易被误解之处，必须写死：

- **数据量小不构成豁免理由**。节点列表只有 1 个节点、诊断只有 2 条问题时，
  页面仍不得把半屏以上留成地面。正确做法是本标准 §5 的三种收口之一，
  而不是「等数据多了就满了」。
- 唯一合法的地面是**已裁决的呼吸位**：页身边距、卡片间距、簇边界空气、
  shell 保留空气。它们都在盒外或按 §2 的实带规则计为内容边界。
- 顶部对齐 + 下方整片地面**不是**「顶级开源产品的做法」。实测参考产品的
  一级/二级/三级页面 12 帧，`tail` 全部 ≤6.3%；只有对话框与设置子面板
  这类内容型窗口才出现 9–24%。

## 5. 超标时的三种收口（按顺序选，不得自造第四种）

1. **填满**：页面主体是列表或长内容 → 用 `CardHeight.page-body` +
   共享 `ListBody`，卡成为页面框架、行在框内滚（b134 契约）。
2. **合并**：页面有多个同族区块 → 收进同一张卡的同一竖排，
   删除区块之间的独立边框与重复空气。
3. **提升密度**：页面区块数与行数都不变 → 收紧行距、字号档位与卡内边距，
   让既有内容占据更多页身。密度令牌唯一真源是
   `crates/infiltrator-slint/ui/components/theme.slint` 的 `LumenTheme`，
   页面不得自带第二套间距字面量。

禁止的第四种做法：在 UI 层加布尔开关、临时标志位或「数据少时特殊排布」的
私有推导来让测量通过。这类做法违反总纲的自底向上裁决铁律。

## 6. 门禁与欠账

- `scripts/parity/space_utilization.py --enforce` 判定最新发布整批；
  超标帧必须在 `scripts/quality/space_utilization_debt.json` 登记欠账，
  未登记即违规。
- 欠账台账是**精确清单，不是白名单**：`--update-debt` 只允许收紧，
  出现新登记条目直接失败；已消失的条目必须同步删除。
- 结构门 `scripts/quality/check-structure.sh` 同时跑 self-test 与 enforce 双腿；
  无发布批次时像素腿诚实跳过并说明原因，不写成通过。
- 身份变化（布局、密度、几何）后必须整批重采，旧像素即时淘汰；
  缺新回执的业务只能记「部分」。

## 7. 现状（首次测量）

首次全矩阵测量（发布批 `20261108T085245Z_9107f001fa16_dirty_3068240`，197 帧）：

| 档位 | 帧数 | 中位 | p90 | 最差 | 超标帧 |
| --- | --- | --- | --- | --- | --- |
| `desktop` | 123 | 3.7% | 40.6% | 59.4% | 26 |
| `compact` | 61 | 3.8% | 12.9% | 43.4% | 8 |
| `phone` | 13 | 2.3% | 47.6% | 47.8% | 4 |

侧栏：b145 的 `pill_tail` 判据已按 FlClash 桌面真实场景**整体作废**。
b146 起按「侧栏只放导航」重裁：发布批
`20261108T140818Z_766b53b0af31_dirty_1253742` 实测 `chrome_band` 15.0–16.3%
（状态卡 + 速率 + 版本行），导航 void 9.6%（达标）。该 16.3% 登记为只减欠账；
侧栏改为消费 typed 导航注册表、删除状态卡后应降到 0%。

**b149 修订**：按形态分判后，desktop 与 phone 档 **0 帧超标**；欠账从 38 帧
收紧到 3 帧，且只剩 compact 档 `framed` 页的搜索高亮态（各 12.9%，上限 4%）。
「代理节点页 / 关于页 / 诊断页 / 应用分流页大面积留白」这一结论**被撤回**：
同形态的参考产品留白更多（80.6%），本仓反而更紧凑（48–56%）。

## 8. 现状（b150 收口：欠账台账清零）

b150 把 compact 档那 3 帧 `framed` 页的最后一段死地收掉，**欠账台账归零**。
发布批 `20261108T164957Z_cda77dec2603_dirty_2413388`（197/197，425s）实测：

| 档位 | 帧数 | 中位 | p90 | 最差 | 超标帧 |
| --- | --- | --- | --- | --- | --- |
| `desktop` | 123 | 3.7% | 40.6% | 59.4% | 0 |
| `compact` | 61 | 3.5% | 6.1% | 43.4% | 0 |
| `phone` | 13 | 2.3% | 47.6% | 47.8% | 0 |

- compact 档 p90 由 12.9% 降到 **6.1%**：三帧 `framed` 页（`logs` / `rules` /
  `connections` 的搜索高亮态）tail 由 12.9% 归 **0.0%**，与桌面档同形态一致
  （桌面档同样 tail 0.0%、band 3.6%）。band 3.5% 是页头标题与卡片框之间的
  呼吸位，不是浪费。
- 侧栏 `chrome_band` **0.0%**、`void` 4.4%，两档均在上限内。
- `scripts/quality/space_utilization_debt.json` 的 `frames` 现为 **空**。台账
  只减不增的纪律未变：任何新登记仍会被 `--update-debt` 拒绝。

## 9. 现状（b173 折叠钮 + b174 组件化收口后的整批重测）

b173 把折叠钮接进侧栏（wire 从 9 键扩到 12 键，新增两张折叠纯外壳帧，矩阵升到 199
行并把 `interaction` 变成显式第 9 列），并把按钮带从「按 token 猜 66」改成「侧栏实测
106」；b174 再把这份 chrome 投影抽成独立 `ShellReceipt` 组件并把外壳压回 SLINT-001
预算，wire 值不变、像素复采。发布批
`20261110T063442Z_a58ed35be980_dirty_309370`（199/199，444s；与 b173 收口批
`20261110T060401Z_a58ed35be980_dirty_90070` 相比 198 帧逐字节相同，唯一差异是搜索
高亮动画帧）实测：

| 档位（导出档 id） | 帧数 | 中位 | p90 | 最差 | 超标帧 |
| --- | --- | --- | --- | --- | --- |
| `compact`（400×800） | 13 | 2.3% | 46.8% | 47.1% | **0** |
| `medium`（720×480） | 62 | 3.5% | 6.1% | 59.5% | **0** |
| `expanded`（1180×780） | 124 | 3.7% | 40.6% | 59.4% | **0** |

- 侧栏（只有 `expanded` 档绘制带标签列，判定器按导出 `labelled_sidebar` 取轴）：
  `chrome_band` **0.0%**（上限 0.0%）、`void` 最差 **6.2%**（上限 12.0%，186 帧拥有列），
  两项均在上限内；`space_utilization.py --enforce` 对 199 帧 violations=0，
  欠账台账 `frames` 保持为空。
- p90/max 落在 `content` 形态页（上限 82%）：同形态的参考产品实测留白 80.6%，
  本仓这些帧反而更紧；这不是超标，也不是豁免——它们仍受 §3.1 的形态上限约束。
- 两张折叠纯外壳帧（`sidebar-collapsed-standard` / `sidebar-collapsed-compact`）证明
  收起态确实落到 64px rail 且页面内容立即补位；compact 视口下导出表本就给 rail，
  故 collapsed 与 expanded 的像素逐字节相同，已由 `screenshot_aliases.tsv` 显式声明别名。
- 判据本身的变化：档名与档界不再由判定器记忆，因此 800～999px 区间「有没有侧栏」
  这类误判在结构上已不可能发生（判定器自测 24 项里 4 项专做阶梯验证：planted 换界即换档、无 tiers 拒绝、缺口拒绝、窄档帧不得当侧栏帧量）。
