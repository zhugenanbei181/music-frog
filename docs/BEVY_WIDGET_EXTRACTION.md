# 与 taskmanager 共同 bevy 控件抽取评估（BEVY-007）

本文是 BEVY-007 的正式评估产物，回答章程（`docs/BEVY_UI_FRONTEND.md` §3）预留的
问题：**"同一个控件在两个项目中第二次出现时，下沉"——现在哪些控件已第二次出现，
以什么机制下沉，分几批走。** 全文证据驱动：每个判定附双方 file:line，行号对应当前
工作树（music-frog @ widgets crate；TaskForest @ `crates/taskmanager-bevy-ui`）。

下文 **MF** = 本仓 `crates/infiltrator-bevy-widgets`（standalone workspace），
**TM** = `/run/media/zhugenanbei/TiPro9000/rustdev/taskmanager` 仓的
`crates/taskmanager-bevy-ui` + `crates/taskmanager-theme`。

## 1. 结论先行

**前提已核实**：两仓 bevy 均精确锁 `=0.19.1`（MF `Cargo.toml` 三张 per-target 表、
TM `Cargo.toml:46`；两侧 `Cargo.lock` 均解析 bevy 0.19.1 + bevy_ui_widgets 0.19.1）；
两仓控件都是"零 bevy 纯函数核 + `bsn!` 场景适配器 + typed marker + 观察者/同步系统"
同一套模式；`bsn!` 宏限制、官方原语不可无头驱动等坑两边记录互相印证。抽取条件成熟。

**判定汇总**：双侧重合控件共 9 项，全部判 **对齐后下沉**——没有一项达到"立即下沉"
（语义几乎一致），因为每对都存在已枚举的 API 形状差异（详见 §2 矩阵与 §3 分析）；
独有控件 10 项暂不下沉。推荐路径：

1. **机制**：vendored 子树 + 同步脚本 + 双仓 CI 校验（方案 [b]，§4）；
2. **第一批**（对齐后即可下沉）：pill button、surface 卡片、checkbox 行、radio 行、
   TextRole/字体章（5 项）；
3. **第二批**：nav 项、图标位图缝、scrollarea、stat chip（4 项，等 IconId 归属等
   决策，§6）；
4. **token 层不整体下沉**：共享包以解析后的 `UiPalette`（bevy 值快照）为输入边界，
   token→palette 的解析留在各仓（§3.1）。

| 判定 | 数量 | 清单 |
| --- | --- | --- |
| 立即下沉 | 0 | ——（唯一逐字重复的是 `theme_color` 4 行函数，随 token 边界决策一并处理） |
| 对齐后下沉 | 9 | 第一批：pill / surface / checkbox / radio / TextRole+fonts；第二批：nav / icon / scrollarea / stat_chip |
| 暂不下沉 | 10 | slider、text_input、icon_tile、ThemeSwitch、sparkline/chart、table、menu、dialog、layout 断点、control_contract |

## 2. 对照矩阵

语义重合度：高 = 双方解决同一控件语义且行为原语相同；中 = 同意图不同形状；
— = 单侧独有。

| 控件/层 | MF 证据 | TM 证据 | 重合 | API 形状差异（需对齐的点） | 判定 |
| --- | --- | --- | --- | --- | --- |
| pill 按钮 | `src/button.rs:63-114`（`pill_scene`/`pill_caption_scene`）、`:41-52`（`control_fill` 纯函数）、`:146-169`（compare-and-set 标签/描边同步） | `src/widgets/controls.rs:165-184`（`pill_scene`）、`:62-85`（`control_background`）、`src/window.rs:531-555`（PostUpdate 同步） | 高 | 官方 `Button` + selected 位 + token 填充逐点对应；差异：`ControlVisual(bool)` vs `ControlVisual(ControlTone, bool)`；MF 有 hairline 描边 + `PillLabel` 墨色跟随 + caption 变体，TM 无；pressed 语义 MF 读 `PickingInteraction::Pressed`、TM 另查 `Has<Pressed>` 组件 | 对齐后下沉（第一批） |
| surface 卡片 | `src/surface.rs:24-39`（`surface_scene`）、`:43-52`（同步） | `src/widgets/controls.rs:102-120`（`surface_scene(tone, …)`）、`:267-288`（`graph_card_scene`） | 高 | 同名同形（列布局 + S16 内距 + 卡片圆角 + 动态子场景）；差异：TM 多 `SurfaceTone`（elevated/content）参数且无 marker/重漆系统，MF 有 `SurfacePanel` marker | 对齐后下沉（第一批） |
| checkbox 行 | `src/checkbox.rs:61-167`（checked/unchecked 分支场景 + 视觉盒 + `sync_checkbox_visuals`） | `src/pages/settings.rs:423-453`（`*_checkbox_shape` 内联分支）、`src/pages/alerts.rs:357,383` | 高 | 结构同款（官方 `Checkbox`+`Checked`+分支函数+TextRole 标签，"第二次出现"证据最强的一对）；差异：MF 有视觉盒/圆环 + compare-and-set 重漆，TM 内联版刻意无视觉盒 | 对齐后下沉（第一批；TM 删内联副本） |
| radio 行/组 | `src/radio.rs:62-184`（行/组场景 + `RadioGroup` + 同步） | `src/pages/settings.rs:392-421`（`*_radio_shape` 内联分支） | 高 | 同 checkbox；TM 无组容器封装（`RadioGroup` 散在页面） | 对齐后下沉（第一批；TM 删内联副本） |
| TextRole/字体章 | `src/text.rs:23-94`（6 角色 `Role` + `role_typography` 纯函数 + `On<Add,TextRole>` 盖章）、`src/fonts.rs:24-79`（嵌入 Inter×3 + JetBrains Mono） | `src/window.rs:85-99`（4 角色）、`:505-525`（盖章 observer）、`:461-500`（MiSans VF + Roboto Mono 注册） | 高 | 同名 `TextRole`/`Role` 不同枚举（MF 多 Display/BodyStrong）；face 表：MF 绑死 `include_bytes!`，TM 经 `taskmanager_assets::embedded_fonts()` 注入——下沉后 face 必须改为 host 注入；MF 墨色进 `UiPalette`，TM 的 `TextFont` 预置进 palette | 对齐后下沉（第一批） |
| nav 项 | `src/nav.rs:66-110`（`nav_item_scene`，**非** Button，`nav.rs:11-13` 记录理由） | `src/app.rs:595-694`（`nav_tab_scene`/`nav_trailing_scene`/`nav_strip_scene`，官方 Button+`Activate`+icon+`NavTarget`）、`:248-254`、`:361-403` | 高 | 选中=accent 填充+on-accent 墨的语义一致；差异：MF 无 press 语义（刻意的），TM 有路由激活 + 图标 + NoWrap 截断；形状（侧栏纵列 vs 顶栏横条）| 对齐后下沉（第二批） |
| 图标位图缝 | `src/icon.rs:39-199`（`IconId`/`icon_path`/`IconSources`/`IconPlate`+`IconTint`/`stamp_icon_plate`） | `src/icons.rs:96-201`（`IconPlates` 嵌入 bytes 直接注册/`IconPlate`+`IconInk`/同形 observer） | 高 | 同一法律（位图永不走字形码位）+ 同形 observer；差异：`IconId` 归属（MF 在 widgets 内 vs TM 在中立 ui-contract crate）、plate 分发（AssetServer 路径 vs 嵌入 bytes）、`IconTint` vs `IconInk` 命名 | 对齐后下沉（第二批；IconId 归属需拍板） |
| scrollarea | `src/scrollarea.rs:25-67`（`clamp_scroll` 纯函数 + 卡片视口场景） | 无封装；4 处内联裸用 `ScrollArea`（`src/pages/performance/scene.rs:46`、`scene/sidebar.rs:369,428`、`services/log_panel.rs:315`） | 高 | TM 页面各自内联视口几何，无 clamp 契约封装——4 处内联即 4 次重复 | 对齐后下沉（第二批） |
| stat chip/行 | `src/stat_chip.rs:51-97`（icon-tile 卡片：tile+caption 标签+mono 值） | `src/widgets/controls.rs:125-161`（`stat_row_scene`：label/value 两列行） | 中 | 同为"紧凑数据展示"但卡片 vs 行，值节点 marker（`StatChipValue`）vs 动态子场景 | 对齐后下沉（第二批） |
| slider | `src/slider.rs:36-124`（官方 `Slider` 包装 + `slider_fraction`） | 无实现（`src/capabilities.rs:59` 仅能力表登记） | — | 一侧独有 | 暂不下沉 |
| text_input | `src/text_input.rs:41-273`（零 bevy `TextFieldState` 状态机 + 场景；官方 `EditableText` 不可无头驱动记录 `:3-15`） | 无 | — | 一侧独有（TM 的 SearchInput 亦仅能力表登记） | 暂不下沉 |
| icon_tile | `src/icon_tile.rs:31-88` | 无 | — | 一侧独有 | 暂不下沉 |
| 运行时换肤 | `src/switch.rs:31-69`（`ThemeSwitch(LightDark)` 事件 + `apply_theme` 原地 restamp） | 无（palette 启动定格 `src/window.rs:139-149`；capture 换 skin 靠重启） | — | 一侧独有；MF 领先项 | 暂不下沉（接口为 TM 预留） |
| sparkline/chart | 无（MF Overview 曲线规划中） | `src/widgets/sparkline.rs:49-138`、`src/widgets/chart.rs:41-195`（gap-aware 投影纯核 + 段矩形渲染） | — | TM 独有；纯核零 bevy，MF 需要时是现成下沉候补 | 暂不下沉 |
| table | 无 | `src/widgets/table.rs:32-259`（列词汇来自 `taskmanager_ui_contract::PROCESS_COLUMNS`，虚拟滚动纯核） | — | TM 独有且与 ui-contract crate 耦合 | 暂不下沉（业务列语义强） |
| menu | 无（BEVY-010 开放） | `src/widgets/menu.rs:77-196`（`MenuState` 键盘优先纯核）+ `menu_modal.rs`（W4 未接 call site，`menu.rs:1-5`） | — | TM 独有且未接线 | 暂不下沉 |
| dialog | 无 | `src/widgets/dialog.rs:40-173`（双回声 `ConfirmationDialog`）+ `control_contract.rs:19-48` | — | TM 独有；`ControlTarget/ControlVerb` 是业务枚举（Process/Service/…）——"业务语义强不下沉"的典型 | 暂不下沉 |
| layout 断点 | 无 | `src/widgets/layout.rs:9-36` | — | TM 页面专属 | 暂不下沉 |
| token 层 | `src/theme.rs:16-176`（token 在 widgets 内）+ `src/palette.rs:14-83` | `taskmanager-theme`（中立 crate：8 skins×light/dark+高对比+平台补偿，`src/lib.rs:1-34`）+ bevy-ui `src/palette.rs:36-176`（token→bevy 适配） | 高 | 架构分歧，见 §3.1 | 架构决策（不作为控件下沉） |

两仓测试形态同源不同挂法：MF 用集成测试（`tests/headless.rs:1-27` + 15 个
`tests/headless/*.rs`，`MinimalPlugins`+`AssetPlugin`+`ScenePlugin`+`TextPlugin`
组装 `WidgetsPlugin`，`tests/headless/support.rs:16-22`）；TM 用
`#[cfg(test)] #[path = "../tests/headless/…"] mod tests` 模块内挂接（如
`src/palette.rs:178-180`）。两者都满足"纯核无头可测"；差异根因是可见性（§3.3）。

## 3. 关键差异分析

### 3.1 主题架构：token 放哪

- MF：token 就住在 widgets crate 内（`src/theme.rs:1-11` 自称"未来抽取候选"），
  双模式 iOS 设计语言写死（`Theme::dark()/light()`，`theme.rs:95-137`），唯一
  token→bevy 适配点是 `src/palette.rs`。
- TM：token 在独立中立 crate `taskmanager-theme`（8 skins × light/dark × 高对比轴，
  含 gpui/iced feature 绑定），bevy-ui 内 `palette.rs` 只做映射；bevy-ui 的
  `UiPalette` 字段全部 `pub(crate)`，palette 类型不出 crate。
- 两仓法律一致：**token→bevy 值只发生在一处**；字面量禁止出现在调用点。

**评估**：共享控件包不应携带任何一方的具体 token 值。两仓的 token 体系（iOS 双模式
vs 8-skin 梯度）是各自产品决策，强行合并会迫使一方改设计语言。正确边界是：共享包
消费**解析后的 bevy 值快照**（MF 现行 `UiPalette` 形状，字段 pub），token→palette
解析留在各仓——MF 已天然满足；TM 现行 `pub(crate)` + `TextFont` 预置形状需要外翻
成共享包的 `UiPalette`。风险点：TM palette 里预置 `TextFont`（含字重）而共享包的
排版走 `TextRole` 盖章，两者只能留一条路（见 §6-Q2）。

### 3.2 换肤机制：compare-and-set vs 启动定格

- MF：每个控件配一个每帧 compare-and-set 同步系统（如 `checkbox.rs:148-167`、
  `nav.rs:88-110`），`ThemeSwitch` 事件只换 `UiPalette` 资源 + restamp 文本/pill，
  其余系统靠"每 pass 对比现 palette"自愈（`switch.rs:36-38` 成文）。代价：约 11 个
  Update 系统每帧空转查询（未变更帧零写噪声）。
- TM：palette 启动构造一次，控件同步是 `Changed`-驱动且挂在 PostUpdate
  （`window.rs:373-376`、`:531-555`），无运行时换肤需求。

**评估**：下沉包必须统一为 MF 的 compare-and-set 形态——它是"换肤不需要控件感知"
的结构性解法，且 TM 将来的 light/dark 支持免费获得。TM 需要接受：同步系统从
PostUpdate/Changed 驱动改为 Update/全量对比，及每帧多几次空查询（成本可忽略，但
要在 TM 的验收里以现有 headless 断言证明无回归）。`Pressed` 语义一并统一：TM 的
`Has<Pressed>`（官方组件）与 MF 的 `PickingInteraction::Pressed` 变体应合并为
"两者任一"或官方组件一种（§6-Q4）。

### 3.3 marker 组件命名空间与可见性

同名不同形的陷阱清单（不先对齐就直接下沉 = 同名组件语义漂移）：

| 名字 | MF 形状 | TM 形状 |
| --- | --- | --- |
| `ControlVisual` | `(pub bool)`（`button.rs:30`） | `(ControlTone, bool)`（`controls.rs:58`） |
| `TextRole`/`Role` | 6 角色（`text.rs:23-41`） | 4 角色（`window.rs:89-99`） |
| `IconPlate` | 同名同形（`icon.rs:139` / `icons.rs:137`） | 同名同形（唯一已对齐对） |
| tint marker | `IconTint`（`icon.rs:145`） | `IconInk`（`icons.rs:149`） |
| `pill_scene` | 返回 `impl Scene + use<>`，带描边 | 同签名，无描边（`controls.rs:165`） |

可见性差异是结构性障碍：MF 全部 `pub`（集成测试要求），TM 全部 `pub(crate)`
（模块内测试即可）。下沉包必须 `pub` + 集成测试形态，TM 迁移时其调用点本就只经
scene 函数与 marker，`pub` 化不破坏封装（palette 边界除外，§3.1）。

### 3.4 字体与图标资产

字体：MF 绑死四张 `include_bytes!` OFL face（`fonts.rs:24-30`），TM 经
`taskmanager_assets` 注入 MiSans VF/Roboto Mono。共享包只能拥有 **Role→face 槽位
接口**（现 `FontSources` 四槽结构可直接沿用），face 字节由 host 注册——MF 的
嵌入实现退化为 host 侧的默认注入器。图标：同 law 两种分发（路径加载 vs 嵌入
bytes），`IconId` 集合是产品语义（MF 的 Lucide chrome 集 vs TM 的 ui-contract
注册表），归属须拍板（§6-Q3）；分发机制可双轨保留（`IconSources::load(server)`
与 `IconPlates::build(images)` 皆为 host 侧装配，共享包只认 handle 表）。

## 4. 抽取方案对比

法律前提：bevy 锁 `=0.19.1`，升级属架构与发布评审（两仓章程成文）。共享包因此
**必须**延续 MF 现行的独立 workspace + per-target feature 闭包 + `=0.19.1` 精确锁
形态，任何方案都不得引入 `version = "0.19"` 浮动解析。

| 方案 | 做法 | 利 | 弊 | 结论 |
| --- | --- | --- | --- | --- |
| [a] git 依赖 pin commit | 共享包留在一仓（或新仓），对方 `Cargo.toml` 以 `git = …, rev = <sha>` 引用 | 零拷贝、升级=改一行；天然单一事实源 | 跨账号 remote（`zhugenanbei181/music-frog` vs `YellowWhiteBlackCat/TaskForest`）使 CI credential/ssh alias 双方都要配置；rev pin 的 diff 审查要跨仓操作；TM 主 workspace 引入外部 git 源后其 lock 独立性叙述变复杂；任一仓网络策略收紧即断供 | 备选，不推荐 |
| **[b] vendored 子树 + 同步脚本 + 双仓 CI 校验** | 共享包以独立 workspace 形态**同时**活在两仓固定路径（如 `third-party/<crate>/` 或沿用 MF 现路径）；`scripts/sync-widgets` 脚本按记录的基准 commit 做带 hash 校验的定向拷贝；两仓 CI 各自跑全量 nextest+clippy+bsn 守卫，另加"树内副本与基准一致"校验步 | 零发布流程、离线可构建、审计即 `git diff`；延续两仓 `publish = false` 现状；MF 的独立 lock/Android feature 闭包原样保留；单侧紧急修复可先落本仓再同步，不被对方 release 节奏卡住 | 双份工作树需要纪律（脚本 + CI hash 校验即护栏）；"哪个方向是权威"需要成文（见 §6-Q1）；同步遗漏靠 CI 兜底而非编译期 | **推荐** |
| [c] 发布 crates.io | 去掉 `publish = false`，以 `0.x` 版本发布，两仓按版本依赖 | 消费体验最标准；版本边界清晰 | 两仓现均为私有协作仓（跨两个 GitHub 账号），公开发布需双方 owner 同意私有性变更；控件层仍在快变期（TM menu/dialog W4 未接线、MF BEVY-010 未做），每个补丁都要发版+两仓升级，版本节奏被最快一方绑架；`=0.19.1` 锁在 registry 上虽可表达，但"升级走评审"的流程约束在公共 registry 上更难执行 | 暂不采用；控件层稳定后（menu/text field 落地、bevy 升级节奏明确）可复议 |

**推荐路径**：方案 [b]。落地形态——共享包源码以 MF `crates/infiltrator-bevy-widgets`
为权威起点（它已满足全部前提：业务无关成文、独立 lock、无头测试、Android 闭包），
TM 侧 vendored 副本 + 同步脚本 + CI 校验；`theme.rs` 具体 token 值**不进**共享包
（§3.1），由各仓 host 提供 `UiPalette`。

## 5. 分阶段计划

### 阶段 0：对齐 PR（两仓各一，不引入共享包）

1. marker 对齐：`ControlVisual` 取 `(bool)` 单字段（TM 的 tone 并入 scene 函数参数
   或独立 marker）；`IconInk`→`IconTint` 改名或成文别名决策；`Pressed` 语义统一
   （§6-Q4）。
2. TM 的 `pill_scene`/`surface_scene` 补 hairline 描边与 `SurfacePanel` 式 marker，
   或成文声明共享包提供带描边变体、TM 视觉验收更新。
3. `Role` 枚举并集化准备：共享包取 6 角色超集，TM 继续只用 4 角色子集（渲染无影响，
   headless 断言按角色显式匹配）。
4. 验收：两仓 `bash scripts/test-bevy.sh`（MF）与 TM 等价全绿；MF 截图闭环
   （`scripts/capture-bevy.sh`）无视觉 diff；TM capture `BEVY_CAPTURE_MARKER`
   链路无回归。

### 阶段 1：第一批下沉（pill / surface / checkbox / radio / TextRole+fonts）

1. 共享包内完成：face 表改 host 注入（`FontSources` 保持四槽资源接口，`include_bytes!`
   移到 MF host 侧注入器）；`UiPalette` 成为唯一主题入参；全部 `pub` + 集成测试。
2. TM 迁移：删除 `pages/settings.rs:392-453` 与 `pages/alerts.rs` 的内联
   checkbox/radio shape 副本，改调共享层；`controls.rs` 的 `control_background`/
   `pill_scene`/`surface_scene` 删除，改调共享层；同步系统切 compare-and-set。
3. 验收标准：两仓 nextest 全绿且**测试计数不降**（TM 内联行为的断言迁到共享包集成
   测试）；TM settings/alerts/performance 页截图与基准一致（描边等有意视觉变化须
   在 PR 里逐条声明）；`cargo tree` 证明共享包零业务依赖。
4. 回归风险：TM 同步系统换时机（PostUpdate→Update）可能改变同帧填充顺序——用 TM
   现有 `tests/headless/window.rs` 断言兜底；TM palette `TextFont` 预置与 `TextRole`
   盖章并存的过渡期要一次性切换，禁止双轨。

### 阶段 2：第二批下沉（nav / icon / scrollarea / stat_chip）

前置：§6-Q3（IconId 归属）拍板。TM 删除 4 处内联 `ScrollArea` 视口几何改调
`scrollarea_scene`；nav 项形状差异（侧栏纵列 vs 顶栏横条）以 scene 参数表达而非
分叉两个控件；stat chip 与 stat_row 先并存（卡片/行两个场景函数进同一模块），不
强行合并形状。

### 阶段 3：TM 侧 vendored 落地 + CI 双向校验

同步脚本进两仓 CI（树内副本 hash == 基准记录）；TM 文档成文引用本评估与章程 §3。
此后"第二次出现即下沉"的执行动作 = 对齐 PR（阶段 0 模板）→ 共享包 → 同步脚本
反向入 MF。

## 6. 决策清单（需双方 owner 拍板）

1. **权威方向**：vendored 双副本中，共享包权威在 MF 仓（推荐：MF 已是独立 workspace
   且业务无关成文）还是提升为第三方独立仓？同步脚本的基准记录放哪？
2. **主题边界终态**：共享包 `UiPalette` 字段集 = MF 现行 27 字段快照，TM 的
   `TextFont` 预置字段（`heading/body/caption/mono`）废弃、统一走 `TextRole`
   盖章——TM 是否接受？（不接受的替代案：共享包排版入口参数化，成本更高。）
3. **IconId 归属**：图标 id 枚举进共享包（两仓 id 并集、各自 id 表可以稀疏），
   还是各自 host 定义、共享包只认 `usize`/host 枚举？plate 分发（AssetServer 路径
   vs 嵌入 bytes）是否双轨常驻？
4. **Pressed 语义**：官方 `Pressed` 组件（TM 形态）还是 `PickingInteraction::Pressed`
   变体（MF 形态），或两者任一？涉及 `control_fill` 纯函数签名，第一批前定。
5. **`Role` 枚举**：共享包取 6 角色超集（TM 弃用 Display/BodyStrong 不实现）还是
   TM 补齐两角色？影响 `FontSources` 槽数。
6. **TM 运行时换肤时间表**：`ThemeSwitch`/compare-and-set 随第一批进入 TM（推荐，
   免费获得 light/dark 能力）还是 TM 先按启动定格消费、换肤延后？
7. **bevy 升级协议**：共享包 `=0.19.1` 锁的变更必须两仓同窗评审——是否成文进双方
   章程（MF 章程已约，TM 侧需对应条款）。
