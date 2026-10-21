# MusicFrog 工程 TODO · 现在佛

> 层级：L4

## 0. 编写规则

- 唯一用途：登记全部**可推**未办结工作（待编码、待实现、待补证），覆盖 0.60 → 1.0 收口路径：crates.io 发布流水线、0.6x 生态与文档、0.7x 质量红线与安装包、0.9x 视觉与性能，以及五大平台 × 三大 UI 表现架构的 360° 覆盖与逐业务截图认证。已实现待验证的条目在[过去佛](CODE_TODO_PAST.md)；只剩外部环境可解锁的条目在[斗战胜佛](CODE_TODO_BLOCKED.md)。
- 每条仅保留唯一 owner、产物、依赖、验收、计划与证据要求；header ≤240 字符、详情 ≤6 行、整块 ≤1200 字符。证据留 nextest run ID / guard 回执；事实变化改写原条，不追加过程叙事。
- 状态词汇：`Planned / Blocked / Ready / InProgress`（本板）与 `Integrated`（过去佛，已交付待验证）。`Verified` 只是办结过渡态，不驻留任何看板。
- **执行标准（2026-10-20 起强制）：** 底层结构不够就扩 owner 结构，禁止再塞 `bool`/`Option`；组合优于继承；field 归 field、表现归表现、事务归 owner；每个事实只有一个真源，不留第二份缓存；非身份改动不碰 wire/快照/语料，身份改动集中一次做完。
- **可推性准入（2026-10-19 起强制）：**本板与过去佛只放「下一个可执行动作能在当前环境完成」的条目；剩余闭环动作全部依赖外部环境（ARM64 真机、受控 tag CI、用户 secrets/keystore、crates.io 上架凭证、异构目标机与被禁平台 target）的条目整条迁入[斗战胜佛](CODE_TODO_BLOCKED.md)，并写明阻塞方与解锁条件。
- **截图认证硬门：**每条业务能力在每个桌面 UI 必须同时具备 contract 锚点、scenario 锚点与 standard/compact 双视口真实像素回执；Android 三 flavor 另需实际交互截图回执。缺任一项只能写「部分」，不得称已验收。判定入口唯一：`scripts/parity/resolve_surface_evidence.py`（锚点与单元格）与 `scripts/parity/visual_receipts.py`（像素回执）。
- Rust 测试唯一执行器 `cargo nextest`（统一收口带 `--workspace --no-fail-fast`）；Clippy 用 `--all-targets -D warnings`；禁止裸 `cargo test`。多阶段操作必须 typed enum/sealed，禁止 bool/Option 组合状态机（总纲强制条款）。
- 迁移看板必须同次移出原文件、移入目标文件，保持稳定 task ID、owner、依赖、验收与 points。
- **并行工流与验证纪律（2026-10-20 起强制）：** 并行开发按文件所有权切分，编译不通过的半成品不得放进会被 `cargo nextest --workspace`、结构门或 capture 构建扫到的路径（WIP 先放草稿或 `.evidence/wip/`）；开发期只跑受影响包，全仓 `cargo nextest --workspace` 与全量结构门统一放到收尾批次只跑一次；共享 `/tmp` 配额、cargo lane 争用与合成器起栈失败等外部故障先判定归属，只修本切片能修的那一类。
- points 合同：`1` 局部证据/文档/配置；`2` 单 owner focused 包；`3` 跨 owner 纵切或普通机制包；`5` 产品采用/终局门。超过 5pt 先拆。priority 与看板位置不改变 points。
- 严格交付率只计 archived points；执行态报告对 Planned/Blocked/Ready 计 0%、InProgress 计 50%、Integrated 计 75%，不以执行态进度代替验收办结。
- 等价映射（避免重复条目）：`BANDROID-001.2 ≡ 040.5`；`BANDROID-004.2 ≡ 040.3`；`BANDROID-036.1` 的语义回归与 `042.3/042.4` 同批验证。
- **自底向上逐层裁决铁律（底层先决，严禁上层修补）：**若遇架构未决（领域模型模糊、所有权未定、状态二义性、缺少权威读回或跨端契约不一致），必须先在 Contract/Domain/Core 完成裁决与真实实现，再逐步上推到 Presentation 与 UI 表面。严禁在 UI 层（Iced/Bevy/Slint/Android）增加临时布尔/状态标志位、容错垫片、自行推导业务事实或写空实现/TODO 注释就地修补；违者一律打回重做。
- **Core-first 阶段门：**先完成所有可独立推进的底层 Core、双后端 conformance、Android 唯一 owner/IPC/VPNService 工作。若只剩现有 UI 消费者的旧调用阻塞 Core 契约退役，允许把 typed API 边界迁移作为对应 Core 切片一次完成；禁止同时改页面布局、视觉或新增 UI 功能，迁移完再集中构建验收。
- **不可解依赖不阻塞：**仅受缺失物理设备、受控 CI tag、用户 secrets/keystore 等外部不可解依赖约束的条目整条在[斗战胜佛](CODE_TODO_BLOCKED.md)；它们不阻塞其它条目选取、统一验证批次执行与已达标条目办结登记。

<!-- POINT_LEDGER:BEGIN -->
- Point 基线 `musicfrog-todo-20261021-v263`：现在佛 196 pt（67 条）/ 过去佛 106 pt（42 条）/ 斗战胜佛 72 pt（29 条），联合未办结分母 368 pt；已归档交付 715 pt（278 条）；累计 1080 pt，严格归档完成率 80.2%。
- 本板 67 条 = 既有机制与真实捕获 3 条（`PARITY-001.5` 5 pt、`SLINT-001.10` 2 pt、`BEVY-020.1` 3 pt）+ Bevy UI 105 点视觉攻坚 7 条（`BEVYUI-001`～`BEVYUI-007`，21 pt）+ Slint 34 点质感攻坚 7 条（`SLINTUI-001`～`SLINTUI-007`，20 pt）+ 跨平台 GUI/TUI 表现深化 200 点自底向上攻坚 50 条（`EFFECTS-001`～`EFFECTS-050`，145 pt，涵盖 Layer 0~7 底层 Token、数学库、呈现投影、通用组件、Bevy 0.20 WESL、Iced 0.14 Canvas/动效、Slint 1.18 Skia、Ratatui 0.30 终端与多端降级集成）；原未来佛 16 条 + 截图认证与平台覆盖 5 条（`PARITY-001.2`～`001.4`、`PLATFORM-001.1`）+ Slint 第三 UI 补齐 9 条（`SLINT-001.1`～`001.9`）+ 共享控件真源补齐 1 条（`WIDGET-001.1`，本批新增 3 pt）；Bevy UI 能力规范强制化 5 条（`BEVYCAP-001.1`／`001.2`／`002.1`／`003.1` 与 `TOKEN-001.1`）本批全部交付并迁入过去佛，`PUB-006.1`、`PARITY-001.1` 与 `PLATFORM-001.2` 同样已交付并迁入过去佛；Android 静态质量门 4 条（`BANDROID-044.1`／`044.2`／`044.4`／`044.5`，9 pt）与状态表达/文档红线 2 条（`STATE-001.1` 隐式状态机全量审计、`PUB-006.6` 公共 API 文档质量门，5 pt）与开源生态四条（`PUB-006.7`／`006.9`／`006.10`／`006.11`，7 pt）、平台证据与安装包两条（`PLATFORM-001.1`、`APPID-001.3`，5 pt）、地球增强两条（`BANDROID-016.2`、`016.3`，4 pt）、双引擎真实二进制 smoke 与 Android 取证两条（`PUB-006.8`、`PARITY-001.4`，5 pt）与 Slint 第三表面补齐七条（`SLINT-001.2`～`001.7`、`001.9`，15 pt）本批交付并迁入过去佛。
- Slint 九条来自 2026-10-19 的表面审计实测：缺 7 类页面导致 19 条业务无载体、只派发 42 种 `CommandIntent`、59 条业务里 2 条完整实现 / 18 条部分实现 / 39 条未实现且今天零锚点、4 个测试文件重实现 Slint 逻辑构成第二业务真源、2 个已声明回调无调用点。
- Bevy UI 四条来自 2026-10-20 的强制化实测审计：场景结构（`bsn!` 唯一入口 + ECS 边界）已由 `bevy_bsn_guard.py` 机器强制（349 文件零违例），rem/em 尺寸通道同批由共享包执行器 `bevy_scene_gate.py`（硬编码 px 字面量与 `FontSize::Px` 零容忍，存量 128 处棘轮登记）与产品守卫 `bevy_rem_guard.py`（命名/派生 px 值白名单边界，存量 52 处独立台账）两段互补强制，两份台账站点不重叠；`bsn_list!` 0 处 / 内联 `Children [` 974 处、`bevy_scene::Ready` 0 处、`despawn_all` 2 处（⑦ 小件收敛仍仅人工评审）；执行状态台账见 Bevy UI 能力规范文档的「强制化台账」一节。
<!-- POINT_LEDGER:END -->

## 一、大计划主线与推进顺序

| 主线 | 覆盖里程碑 | 条目 | 出口判据 |
| --- | --- | --- | --- |
| A 发布流水线主线 | 0.60 / 0.69 / 1.0.0 | `PUB-006.1`、`PUB-006.6`～`PUB-006.11` | 七个中性 crate 可按顺序 dry-run 发布，docs.rs 构建通过，版本拓扑无环 |
| B 表现与截图认证主线 | 0.44～0.49 / 0.92 | `PARITY-001.2`～`001.4`、`PLATFORM-001.1`～`001.2`、`SLINT-001.1`～`001.9`、`GAP-4.1` | 59 条业务 × 桌面三 UI 锚点齐全、standard/compact 双视口像素回执全绿；Android 三 flavor 交互截图全绿 |
| C 质量红线与安装包主线 | 0.70～0.73 | `BANDROID-044.1`、`044.2`、`044.4`、`044.5`、`APPID-001.3`、`BEVYCAP-001.1`、`001.2`、`002.1`、`003.1` | Lint/ktlint/Detekt/架构守卫强制生效且零违规；Linux deb/AppImage 安装冒烟通过；Bevy UI 的 rem 尺寸通道与场景律法成为唯一机器红线 |
| D 视觉与模型统一主线 | 0.48 / 0.90 / 0.91 | `GAP-7.1`、`BANDROID-016.2`、`BANDROID-016.3`、`BEVYUI-001`～`BEVYUI-007`、`SLINTUI-001`～`SLINTUI-007`、`EFFECTS-001`～`EFFECTS-050` | 三大 UI 共用页面模型；地球增强与帧报告；Bevy UI 105 点、Slint 34 点与跨端深化 200 点自底向上整改闭环 |

**推进顺序：B → A → C/D。**B 是「每个业务都真跑过」的判定器，先把它做成可复跑的流水线；A 解锁 0.69/1.0.0 的发布顺序门；C/D 与 B 并行但不得挤占 B 的取证窗口。

## 二、五大平台 × 三大 UI 表现架构覆盖矩阵（360° 无死角口径）

| 平台 | 表现层 | 当前状态 | 收口条目 | 关闭证据 |
| --- | --- | --- | --- | --- |
| Linux 桌面 | Iced / Bevy / Slint | 源码可编译；parity 注册表已含三表面，但 Slint 侧 59 个 scenario 全为 pending、场景表只覆盖 Slint 自身 24 个场景 | `PARITY-001.2`、`PLATFORM-001.1`、`APPID-001.3`、`SLINT-001.1`～`001.9` | 59 业务 × 三 UI 双视口像素回执 + deb/AppImage 安装冒烟 |
| Windows 桌面 | Iced / Bevy / Slint | 本机仅有交叉 `cargo check`，无产物 | `PLATFORM-001.1`（交叉腿）、`PARITY-001.2`（截图腿） | 交叉编译回执 + 与平台无关的像素回执；NSIS 产物腿在斗战胜佛（`APPID-001.4`、`GAP-2.1`） |
| macOS 桌面 | Iced / Bevy（Slint 未启用） | Apple target 被 `apple-ci-deferral-guard.py` 显式禁建 | 斗战胜佛（`GAP-2.2`） | 平台矩阵登记 `Unsupported` + 斗战胜佛阻塞方登记；本板不假装已覆盖 |
| Android | Compose / Bevy / Slint flavor | 源码链路已通，APK 与交互截图未认证 | `PARITY-001.4`、`PLATFORM-001.1` | 模拟器 M1 交互截图与行为回执 + APK identity；真机腿在斗战胜佛 §一 |
| iOS | Rust host seam（626 行 stub） | 无 target、无包、无 NetworkExtension | 斗战胜佛（`GAP-3.2`） | 平台矩阵登记 `Unsupported` + 阻塞方登记 |

**覆盖口径规则：**任一平台 × UI × 业务单元格若没有可复跑证据，一律记为「缺口」并建条目；禁止用「其它端已通过」外推。平台结论以平台矩阵文档（由 L1 入口导航）为唯一事实源。

## 三、逐业务 360° 覆盖清单（59 条业务能力）

| 业务域 | feature 数 | 桌面三 UI 锚点 | 双视口像素回执 | Android 三 flavor 截图 |
| --- | ---: | --- | --- | --- |
| 总览与运行时（overview / runtime / lifecycle / doctor / core） | 7 | 缺 Slint | 缺 Slint | 缺 |
| 代理与节点（proxies） | 7 | 缺 Slint | 缺 Slint | 缺 |
| 配置档与订阅（profiles / sync） | 12 | 缺 Slint | 缺 Slint | 缺 |
| 规则与提供者（rules） | 6 | 缺 Slint | 缺 Slint | 缺 |
| DNS 与编辑器（dns / editor） | 7 | 缺 Slint | 缺 Slint | 缺 |
| 连接与日志（connections / logs） | 7 | 缺 Slint | 缺 Slint | 缺 |
| 测速与命令面板（speedtest / shell） | 7 | 缺 Slint | 缺 Slint | 缺 |
| 设置（settings） | 5 | 缺 Slint | 缺 Slint | 缺 |
| 应用与路由策略（app） | 1 | 缺 Slint | 缺 Slint | 缺 |
| **合计** | **59** | 三表面全部登记：Iced contract 59/59 + scenario 59/59；Bevy 59/59 + 59/59；Slint contract 58/59、scenario 2/59（另 16 条业务已有载体与表面行为测试，交互路径未齐故仍 pending） | 177 个清单格：implemented 120 / ready 2 / pending 55 | 0 个 flavor 截图回执（取证流已交付） |

缺口来源与实测结论（2026-10-19 全量审计）：`scripts/parity/feature_evidence.tsv` 与 `scripts/parity/cross_surface_manifest.tsv` 原先只登记 iced/bevy 两表面（各 118 单元格），`scripts/parity/visual_receipts.py` 只接受 `--iced-run`/`--bevy-run`。现已补齐第三表面：**177 单元格（59 业务 × 3 表面）全部登记并可解析**，Slint 侧 57 格为 `pending` + 逐条原因，2 格已转 `ready`（`profiles-import-modal` 与 `profiles-delete-confirm`：真实表面行为测试 + 标准/紧凑双视口真实像素回执，PNG 摘要与原生 bounds 全部绑定）。四个 pending 行为锚点已全部 anchored（含运行中测速控件的取消路径）。Slint 表面审计得到五条硬事实：① 页面词汇只有 Home/Proxy/Config/Settings/Network/Connections/Logs/Rules/Traffic/AppSettings/About，**dns、editor、filter、mixin、sync、app_routing、doctor 七类页面不存在**，按契约页面归属共 **19 条业务在第三 UI 无载体**（产品缺口）；② Slint 只派发 42 种 `CommandIntent`，DNS、编辑器、脚本沙箱、测速、快照、聚合器、命令面板等 30 余种意图从未派发；③ 59 条业务里 **2 条完整实现、18 条部分实现、39 条未实现**（2026-10-19 实测），今天这 2 条已各自带上面行为测试与像素回执，其余 57 条仍不能合法标记为 Slint scenario 锚点；④ 四个测试文件在用例里**重实现 Slint 逻辑**（路由模式映射、网络模式序列、主题白名单、十六进制解析、连接排序），构成仓库禁止的「第二业务真源」，且永远无法当表面锚点；⑤ 两个已声明回调 `refresh-profile(string)`、`refresh-home-egress()` 在任何 `.slint` 文件里都没有调用点。收口由 `PARITY-001.2`～`001.4`、`PLATFORM-001.1`～`001.2` 与 `SLINT-001.1`～`SLINT-001.9` 共同承担。

## 四、质量红线：隐式状态机全量审计与逐条改写


## 五、A 主线：crates.io 发布流水线与开发者生态


## 六、B 主线：截图认证与平台证据（每个业务都真跑过）


### 5.1 Slint 第三 UI 的 360° 补齐（7条 / 16pt）




## 七、C 主线：质量红线与安装包


### 7.1 Bevy UI 能力规范强制化（2026-10-20 实测立项：bsn! 已是机器红线，rem 部分机器强制）

立项依据（2026-10-20 实测审计，证据等级 L）：场景结构口径已闭合——`scripts/quality/bevy_bsn_guard.py` 扫描两个 Bevy crate 生产树 349 文件零违例并接入 `scripts/quality/check-structure.sh` 与 CI；rem/em 尺寸通道同批由共享包执行器 `scripts/quality/bevy_scene_gate.py` 接上机器红线（硬编码 px 字面量与 `FontSize::Px` 零容忍，存量 128 处棘轮登记），但 token/计算值通道的白名单边界与能力规范 ⑦ 小件收敛**仍仅人工评审**，生产树另有 263 处 `Val::Px` 写入（其中初筛 183 处非白名单、字号通道已全量 `FontSize::Rem`）、`bsn_list!` 0 处 / 内联 `Children [` 974 处、`bevy_scene::Ready` 0 处、`despawn_all` 2 处。执行状态台账见 Bevy UI 能力规范文档的「强制化台账」一节。



- [ ] PARITY-001.5 [kind: evidence] [priority: P0] [points: 5] [owner: infiltrator-iced + infiltrator-bevy-ui] [status: Planned] 桌面两端剩余业务的真实捕获与像素回执
  - 产物：Iced 与 Bevy 两端尚未认证的业务各有一条真实交互与双视口像素回执；缺载体或缺可滚动身份的业务逐条立项。
  - 依赖：本批交付的 owner 能力（13 个端口 fixture、同步冲突与概览卡片排序的应用实现、契约层的 LAN/PAC/路由读回）。
  - 验收：每条业务在两端都有「demo/捕获真的执行了业务」的证据与黑盒像素差回执。
  - 计划：① 用 `crates/infiltrator-test-support` 新增的端口 fixture 补 Bevy 捕获（规则提供者刷新、订阅源失败、配置导入、端口冲突修复、LAN 安全、TUN 权限、版本回滚、快照存储、测速历史、文档 codec、核心进程）；② 用同一批 fixture 补 Iced demo 的 owner 绑定（脚本沙箱与导出评审、差异视图、同步冲突与恢复、编辑器文档会话、聚合预览、应用路由枚举）；③ 逐条重跑 `scripts/parity/accept_surface_interactions.py --all --surfaces iced,bevy` 并把 certified 格登记为 `ready`。
  - 已知阻塞（不是本条的缺口）：部分业务的成功态与「仅路由到页面」的基线像素相同（如端口冲突修复后的「无冲突」空态），需要契约层补一个「已修复」标记，否则像素判据无法区分。

- [ ] SLINT-001.10 [kind: mechanism] [priority: P2] [points: 2] [owner: infiltrator-slint] [status: Planned] 概览卡片排序手势与控件
  - 产物：Slint 概览页的卡片可按共享 `CommandIntent::MoveOverviewCard` 排序（Iced 与 Bevy 两端已有，Slint 端缺手势与控件）。
  - 依赖：共享命令与 owner 已就位（`OverviewLayoutApplication` + `CommandIntent::MoveOverviewCard`）。
  - 验收：Slint 派发同一 typed intent，页面渲染 owner 发布的顺序；排序手势有行为测试。
  - 计划：在 `surface/action/workbench.rs` 增加上下移动手势并在概览页挂控件；注意 `ui/lumen-app.slint` 已接近 600 行预算，新控件走页面文件。
  - 缺口来源：应用层实现批次的报告（Slint 端没有任何概览排序动作）。



- [ ] BEVY-020.1 [kind: mechanism] [priority: P1] [points: 3] [owner: infiltrator-bevy-widgets + infiltrator-bevy-ui] [status: InProgress] Bevy 0.20 未采用能力收口（滚动条 / 滚动入视 / 官方菜单）
  - 产物：① 列表与规则页挂官方 `bevy_ui_widgets::Scrollbar` + `ScrollbarThumb`（滑块与 `ScrollArea` 的滚动位置绑定），替掉"只有一行文字页码"的现状；② 用官方 `ScrollIntoView` 事件替掉 `infiltrator-bevy-widgets/src/list.rs` 里自算偏移的 `scroll_to_item`；③ 菜单层迁回官方 `MenuPopup`/`MenuItem`/`MenuFocusState`/`MenuLayout`，删除本地 `MenuState`/`MenuNavEvent`/`MenuOutcome` 状态机与旧回归路径。
  - 依赖：0.20 官方 `bevy_ui_widgets` 的 `ScrollbarPlugin`、`ScrollIntoView` 观察者、`MenuLayout` 键盘导航已就位（已核对上游 0.20.0 源码）；本地 rem 通道与 token 体系不变（滑块颜色走 `ThemeBackgroundColor`/`ThemeBorderColor`）。
  - 验收：全仓 nextest 全绿；官方滑块可拖动且与滚动位置一致；键盘导航能滚动到聚焦项；菜单的方向键导航与焦点圈闭由官方提供；「Bevy 0.20 UI 能力规范」的「0.20 未采用能力台账」里 SCROLL-01/SCROLL-02/MENU-01 三行清零。
  - 剩余缺口（NAV-01 收口）：官方方向导航已接上并有 7 条测试，但**可见焦点真源仍是本地的**（`crates/infiltrator-bevy-ui/src/shell_focus.rs` 的 `ShellFocusState`/`GamepadFocusController`/本地空间搜索），控件也没有官方 `TabIndex`，所以官方指针焦点够不到按钮、可见焦点环不跟随。收口需要「一个焦点真源」：`shell_focus.rs` 改为从官方 `InputFocus` 派生可见焦点并退役本地空间搜索，控件补官方 `TabIndex`。
  - 推进方式：按文件所有权拆成互不相干的三条流并行推进——官方菜单（`menu.rs`/`context_menu.rs`）、滚动条与滚动入视（`list.rs`）、行内文字与方向导航（`text.rs`/`shell_readout.rs`）；Iced/Slint/Android 三个表面的同类「框架能力自研重造」由第四条流审计并给出整改清单。
  - 事实依据：2026-10-21 在两个生产树上按官方符号名实测——`Scrollbar`/`ScrollbarThumb`/`ScrollbarPlugin` 0 处、`ScrollIntoView` 0 处（官方 `ScrollAreaPlugin` 已注册 `on_scroll_into_view` 观察者，本仓从不触发）、`MenuPopup`/`MenuItem`/`MenuFocusState`/`MenuLayout` 0 处（仅头注释提及）。0.20 官方菜单已带键盘导航与焦点圈闭，本地实现"官方仅指针驱动"的旧理由已失效。

## 八、Bevy UI 表现层与视觉质感 105 点攻坚（GPUI 标杆收口，7条 / 21pt）

- **推进方式（2026-10-21 决定）**：七条按**页面簇**切分并行推进，文件所有权互不相交——001 侧边栏/标题栏、002 总览+地球、003 代理、004 规则、005 连接、006 日志、007 DNS+诊断+设置。共享文件（theme/palette/page_models/text/list/menu/shell_readout）对各流**只读**，需要共享改动的一律报给收口流成批落地，避免并行流互相覆盖。
- **cargo 纪律（减少 cargo 次数）**：并行期**禁止各流跑 nextest/clippy/build**，每条流的编译验证与像素取证统一走捕获脚本 `bash scripts/capture-bevy-matrix.sh <场景>`（脚本自建 Bevy 二进制，兼作编译检查），全仓 nextest 与结构门由收口流在末批**只跑一次**；捕获是共享资源，同一时刻只允许一条流在跑。
- **依赖与次序**：001（`shell_readout.rs`）、004（规则列表接官方 Scrollbar）、006（日志行内排版接 InlineBox）与 BEVY-020.1 的三条流**抢同一批文件**，按"0.20 流先落、视觉整改后落"的次序排队；002/003/005/007 与 0.20 流文件不重叠，立即并行开工。


- [ ] BEVYUI-001 [kind: visual] [priority: P1] [points: 3] [owner: infiltrator-bevy-ui] [status: Planned] 侧边栏与标题栏外壳表现整改（#1~#18）
  - 产物：解决 #1 路由药丸三行重叠、#2 紧凑模式底部图标裁切、#3 侧边栏电光蓝对比过载、#4 未激活项缺少 Hover 缓动、#5 窗口三金刚键一体化、#6 顶栏 1px 分割线、#7 导航箭头比例、#8 Follow rules 胶囊禁用态二义、#9 系统代理/TUN 卡片拥挤、#10 On 徽标文字垂直对齐、#11 订阅卡片折行基线、#12 数量徽标溢出、#13 流量速率排版、#14 rem 物理对齐抖动、#15 图标文字间距统一、#16 Iced/Bevy 导航架构对齐、#17 侧边栏微阶底色、#18 纯图标模式 Tooltip。
  - 依赖：BEVY-020.1，infiltrator-bevy-widgets。
  - 验收：双视口像素回执（720x480/1180x780）消除重叠与截断，通过 visual_receipts 校验。
  - 计划：重构 shell_scene.rs 与 shell_readout.rs 布局约束，添加 Tooltip 与 Hover 状态。



- [ ] BEVYUI-004 [kind: visual] [priority: P1] [points: 3] [owner: infiltrator-bevy-ui] [status: Planned] 规则系统与分段标签收口（#53~#68）
  - 产物：解决 #53 头部堆砌技术日志长文本、#54 辅助操作按钮体积过大、#55 Rules 分段标签切换生硬、#56 Retained draft for 悬挂句号、#57 草稿保存丢弃禁用态对比度过低、#58 分页工具栏插在搜索栏右侧压缩空间、#59 长规则列表缺失滚动条退化分页、#60 规则行圆角梯级白线框杂乱、#61 规则序号未等宽对齐、#62 匹配类型徽标色彩单调、#63 目标动作 DIRECT/REJECT 缺乏语义色彩、#64 Toggle rule 样式割裂、#65 重排序箭头蓝底黑图对比度反转、#66 命中统计与状态纯文本混杂、#67 编辑抽屉无渐入动效、#68 JSON Editor 缺乏代码高亮与行号槽。
  - 依赖：BEVY-020.1，infiltrator-bevy-widgets。
  - 验收：消除长文本堆砌，规则列表接入官方 Scrollbar，双视口像素回执通过。
  - 计划：重构 rules_tabs.rs 与 rules_view.rs，将分页工具栏移出并接入滚动条。


- [ ] BEVYUI-006 [kind: visual] [priority: P1] [points: 3] [owner: infiltrator-bevy-ui] [status: Planned] 实时日志与控制台流式重构（#83~#94）
  - 产物：解决 #83 日志条目整行大圆角胶囊堆叠臃肿、#84 时间戳未等宽且颜色过亮、#85 日志级别标签缺乏底衬被底灰冲淡、#86 模块标签方括号间距松散、#87 级别过滤栏未走 TabList、#88 控制台顶部按钮主次不分、#89 正则输入框 Placeholder 对比度过低、#90 长日志无软折行开关撑破卡片、#91 代码字体行高偏小易串行、#92 刷新追加缺乏渐隐指示、#93 缺少行号与单行快速复制、#94 紧凑模式下日志卡片右侧冲出屏幕。
  - 依赖：BEVY-020.1，infiltrator-bevy-widgets。
  - 验收：日志页双视口像素回执呈现流式控制台质感，消除胶囊卡片堆叠。
  - 计划：重构 logs_rows.rs 与 logs_virtual.rs，采用单行平铺流与 InlineBox 排版。


## 九、Slint UI 表现层与现代架构 34 点攻坚（Qt/GPUI 对标收口，7条 / 20pt）

- [ ] SLINTUI-001 [kind: infra] [priority: P1] [points: 3] [owner: infiltrator-slint] [status: Planned] 渲染管线升级 —— 接入 Skia GPU 硬件加速与 VSync（#1~#4）
  - 产物：解决 #1 桌面端脱离 CPU 软件光栅化引入 renderer-skia、#2 保留 renderer-femtovg 轻量通道、#3 预留 backend-qt 原生桥接支持、#4 开启 GPU VSync 垂直同步消除宽屏撕裂。
  - 依赖：infiltrator-slint/Cargo.toml。
  - 验收：桌面运行确认 Skia GPU 渲染器加载，帧率稳定在 60~120fps，CPU 占用降至个位数。
  - 计划：更新 Cargo.toml 配置桌面 target 的 renderer-skia 特性，配置 Winit 后端。

- [ ] SLINTUI-002 [kind: visual] [priority: P1] [points: 3] [owner: infiltrator-slint] [status: Planned] 桌面设计系统重构 —— 响应式多列网格与 Zinc 空间光影（#5~#10）
  - 产物：解决 #5 终结移动端竖屏拉伸重构为桌面多列仪表板、#6 根除 1180x780 桌面端 70% 纯黑空洞、#7 建立 Zinc 8 阶微差暗色底板系统、#8 引入 Slint 原生 drop-shadow 柔和漫反射两阶阴影、#9 卡片增加 1px 微发光半透明内描边、#10 规范化全局圆角阶梯与内边距 Token。
  - 依赖：SLINTUI-001。
  - 验收：1180x780 桌面视口下呈现饱满双列/三列仪表板，消灭纯黑 void，双视口回执通过。
  - 计划：重构 theme.slint 与 cards.slint，在 page_home 与 page_proxies 引入 GridLayout。

- [ ] SLINTUI-003 [kind: visual] [priority: P1] [points: 3] [owner: infiltrator-slint] [status: Planned] 声明式状态机与微交互 —— 原生 states 贝塞尔动效（#11~#15）
  - 产物：解决 #11 按钮/卡片/开关/导航全面注入原生 states 状态机、#12 状态迁移配置 120ms ease-out 与 cubic-bezier 缓动、#13 按钮与卡片悬停增加 Y 轴 -1px 微抬升与边缘高光、#14 路由分段切换器增加弹性平移胶囊动画、#15 测速与状态切换增加脉冲呼吸动效消除瞬间跳色。
  - 依赖：SLINTUI-002。
  - 验收：鼠标悬停与点击交互平滑跟手，动画无掉帧，通过表面行为测试与交互捕获。
  - 计划：在 base_widgets.slint 与 cards.slint 注入 states 与 transitions 声明块。

- [ ] SLINTUI-004 [kind: mechanism] [priority: P1] [points: 3] [owner: infiltrator-slint] [status: Planned] 列表虚拟化与增量模型 —— ListView 视口回收与零拷贝管道（#16~#20）
  - 产物：解决 #16 代理节点列表迁移为原生虚拟化 ListView、#17 规则/连接/日志长列表改用 ListView 视口回收委托、#18 废除 Rust 轮询每次全量销毁重建 VecModel、#19 接入官方 FilterModel 与 SortModel 零拷贝增量管道、#20 日志流实现自定义零拷贝流式 Model。
  - 依赖：infiltrator-presentation。
  - 验收：500+ 代理节点长列表流畅滚动达 120fps，即时搜索与排序无内存突增与卡顿。
  - 计划：重构 page_proxies.slint 与 lumen_projection/collections，接入 ListView 与 FilterModel。

- [ ] SLINTUI-005 [kind: refactor] [priority: P1] [points: 3] [owner: infiltrator-slint] [status: Planned] 现代 Slint 语法与工程清理 —— 消除占位 Hack 与强类型枚举（#21~#25）
  - 产物：解决 #21 全面消除透明 Rectangle 占位 hack 改用原生 spacing/padding、#22 清理全仓散落的 * LumenUiScale.factor 手动乘法改走系统原生 DPI、#23 在 .slint 声明领域强类型 export enum 替代弱类型字符串、#24 全面改用现代 <=> 双向属性绑定、#25 收敛规范 lumen-app.slint 中堆积的全局属性。
  - 依赖：无。
  - 验收：ui/ 目录下无任何透明占位 Rectangle，编译后无语法弃用警告，单测全绿。
  - 计划：系统性重构 ui/*.slint 文件的布局声明，定义 domain enums。

- [ ] SLINTUI-006 [kind: visual] [priority: P1] [points: 3] [owner: infiltrator-slint] [status: Planned] 现代操作系统沉浸式集成 —— 一体化无边框与 Mica/Vibrancy 特效（#26~#30）
  - 产物：解决 #26 启用 no-frame 自绘现代一体化标题栏与拖拽区、#27 标题栏集成历史导航与面包屑、#28 Windows 11 下通过 Winit 启用系统级 Mica/Acrylic 亚克力底衬、#29 macOS 下启用 Vibrancy 毛玻璃底色、#30 宽屏多列/中屏折叠/窄屏紧凑底栏的优雅响应式切换。
  - 依赖：SLINTUI-001。
  - 验收：多平台窗口边框与底衬呈现操作系统原生现代高级质感，响应式断点平滑。
  - 计划：重构 app_chrome.slint 与 desktop.rs 窗口构建属性。

- [ ] SLINTUI-007 [kind: visual] [priority: P1] [points: 2] [owner: infiltrator-slint] [status: Planned] 排版网格与数据指标美化 —— 等宽数字防抖与基线对齐（#31~#34）
  - 产物：解决 #31 实时上下行速率与连接数启用等宽数字（Tabular Figures）消除刷新抖动、#32 统一长文本末尾渐隐与自动省略（overflow: elide）杜绝撑破、#33 优化代码字体行高与字距消除日志串行、#34 节点卡片内国旗/协议/延迟严格基线中心对齐。
  - 依赖：infiltrator-slint。
  - 验收：双视口下动态数据刷新无字符拉扯跳动，长字符安全截断，基线对齐。
  - 计划：引入 tnum 字体配置与 elide 属性，校准 cards.slint 的 HorizontalLayout 对齐。

## 十、跨平台 GUI/TUI 表现深化 200 点自底向上攻坚（报告实施，50条 / 145pt）

### 10.1 Layer 0: 底层数学、动效参数与跨端 Token 契约层（6条 / 17pt，涵盖 #1~#25）

- [ ] EFFECTS-001 [kind: contract] [priority: P1] [points: 3] [owner: infiltrator-contract] [status: Planned] 语义色彩与 Zinc 多阶暗底契约（#1~#5）
  - 产物：在 design_tokens.rs 裁决 #1 Zinc 8阶表面系统、#2 双色标线性渐变规范、#3 高光描边与半透明语义、#4 延迟与生命周期状态色阶、#5 WCAG AA 文本对比度梯阶。
  - 依赖：无。
  - 验收：契约测试断言颜色值与对比度合法，各表面统一从 contract 获取语义色。
  - 计划：扩展 design_tokens::palette 与 SemanticToken 枚举。

- [ ] EFFECTS-002 [kind: contract] [priority: P1] [points: 3] [owner: infiltrator-contract] [status: Planned] 网格间距、圆角与物理尺寸白名单（#6~#9）
  - 产物：在 design_tokens 规范 #6 8pt/4pt 严格网格阶梯、#7 互斥圆角阶梯裁决、#8 物理像素白名单闭集（发丝线/光标/触控区）、#9 Elevation 空间阴影多阶定义。
  - 依赖：EFFECTS-001。
  - 验收：静态守卫拦截越出白名单的硬编码尺寸与重叠圆角角色。
  - 计划：更新 space.rs 与 radius.rs，补齐 ShadowSpec 定义。

- [ ] EFFECTS-003 [kind: contract] [priority: P1] [points: 3] [owner: infiltrator-contract] [status: Planned] 统一物理动效参数契约（#10~#13）
  - 产物：在 contract 设立 motion_spec 模块，裁决 #10 全局动效时长（120/200/250ms）、#11 阻尼弹簧参数（stiffness:170/damping:26）、#12 动效可中断性继承规则、#13 减少动态效果偏好。
  - 依赖：无。
  - 验收：契约导出 MotionSpec 结构体，单测验证参数合法性与默认减弱模式。
  - 计划：新建 infiltrator_contract::motion_spec 模块。

- [ ] EFFECTS-004 [kind: math] [priority: P1] [points: 3] [owner: infiltrator-shared] [status: Planned] Oklab 感知插值与几何 SDF 数学库（#14~#17）
  - 产物：在 infiltrator-shared 实现 #14 Oklab 色彩空间感知插值算法、#15 三次贝塞尔曲线求值器、#16 RK4 弹簧状态积分器、#17 圆角矩形与药丸形解析 SDF 距离场公式库。
  - 依赖：无。
  - 验收：数学库单测覆盖色彩平滑过渡、弹簧能量衰减无发散、SDF 零误差。
  - 计划：创建 infiltrator_shared::effect_math 模块。

- [ ] EFFECTS-005 [kind: asset] [priority: P1] [points: 3] [owner: infiltrator-shared] [status: Planned] 统一 SVG 矢量白模与等宽字体嵌入（#18~#21）
  - 产物：在 shared 建立 #18 统一 24x24 SVG 矢量图标白模管线、#19 支持 tnum 特性的等宽字体嵌入、#20 HarmonyOS/Inter 多字重排版字体打包、#21 全半角字符宽度度量表。
  - 依赖：无。
  - 验收：各平台渲染图标清晰无栅格毛刺，等宽数字在多端均实现物理等宽。
  - 计划：更新 assets 目录与 font 资源加载逻辑。

- [ ] EFFECTS-006 [kind: contract] [priority: P1] [points: 2] [owner: infiltrator-contract] [status: Planned] 智能截断、安全区与三级画质降级契约（#22~#25）
  - 产物：在 contract 裁决 #22 长文本智能渐隐截断算法、#23 跨平台安全边距 SafeAreaInsets、#24 纹理容量与显存预算常数、#25 简化/标准/增强三级画质档位 VisualTier。
  - 依赖：EFFECTS-001。
  - 验收：契约测试覆盖三级画质枚举与安全区边界换算。
  - 计划：定义 VisualTier 枚举与 safe_area 类型。

### 10.2 Layer 1: 跨端呈现投影、时间驱动与图表数据归约层（7条 / 20pt，涵盖 #26~#50）

- [ ] EFFECTS-007 [kind: presentation] [priority: P1] [points: 3] [owner: infiltrator-presentation] [status: Planned] 呈现生命周期解耦与可组合交互状态（#26~#28）
  - 产物：在 presentation 建立 #26 UiPresence（Absent/Entering/Visible/Exiting）生命周期模型、#27 退出中交互阻断机制、#28 hover/press/focus/select/disabled 可组合交互位状态。
  - 依赖：EFFECTS-003。
  - 验收：行为测试断言退出阶段点击不触发命令，反向打开平滑恢复。
  - 计划：重构 presentation 视图状态模型。

- [ ] EFFECTS-008 [kind: presentation] [priority: P1] [points: 3] [owner: infiltrator-presentation] [status: Planned] 实时图表三层数据分离与平滑曲线拟合（#29~#31）
  - 产物：在 TrafficProjector 分离 #29 静态标尺/动态折线/实时采样三层数据、#30 Catmull-Rom 贝塞尔平滑样条拟合算法、#31 LTTB 视口动态降采样。
  - 依赖：EFFECTS-004。
  - 验收：高频数据输入下样条曲线平滑连续，降采样后峰值保留率 100%。
  - 计划：实现 presentation::chart 投影管线。

- [ ] EFFECTS-009 [kind: domain] [priority: P1] [points: 3] [owner: infiltrator-domain + presentation] [status: Planned] 监控指标环形缓冲与增量列表排序（#32~#34）
  - 产物：在 domain 引入 #32 固定容量 RingBuffer 环形缓冲、#33 增量 FilterProjector 索引缓存、#34 多维连接四维排序纯投影计算。
  - 依赖：无。
  - 验收：每秒高频推流无额外堆内存分配，增量过滤耗时 <1ms。
  - 计划：实现 domain::ring_buffer 与 presentation::list_filter。

- [ ] EFFECTS-010 [kind: presentation] [priority: P1] [points: 3] [owner: infiltrator-presentation] [status: Planned] 交互意图归一化与输入法组合态保护（#35~#36）
  - 产物：在 presentation 建立 #35 统一键盘导航意图转换、#36 ImeComposing 组合态保护协议（组合期间抑制快照热重载）。
  - 依赖：无。
  - 验收：单测验证中文输入法输入未完成时模型不强行刷新文本框。
  - 计划：重构 presentation 的 input 映射层。

- [ ] EFFECTS-011 [kind: presentation] [priority: P1] [points: 3] [owner: infiltrator-presentation] [status: Planned] 平滑渲染时钟与仪表盘数值补间器（#37~#39）
  - 产物：实现 #37 呈现层微秒级独立平滑时钟 RenderClock、#38 空闲状态主动停止下发帧请求、#39 速率与连接数 120ms 数值滚动补间器。
  - 依赖：EFFECTS-004。
  - 验收：数值跳变时输出连续插值帧，静止时时钟立即休眠。
  - 计划：实现 presentation::clock 与 tween 模块。

- [ ] EFFECTS-012 [kind: presentation] [priority: P1] [points: 3] [owner: infiltrator-presentation] [status: Planned] 测速波浪、规则命中与日志流分段投影（#40~#44）
  - 产物：实现 #40 测速 0.0~1.0 呼吸相位输出、#41 协议特性标签去重归约、#42 规则命中分级 Badge 投影、#43 日志行四段分节结构体、#44 长日志软折行度量。
  - 依赖：EFFECTS-001。
  - 验收：单测覆盖多种协议标签去重、日志分段解析与折行高度推导。
  - 计划：扩充 proxies_page_model 与 logs_page_model。

- [ ] EFFECTS-013 [kind: presentation] [priority: P1] [points: 2] [owner: infiltrator-presentation] [status: Planned] 快照 Diff、地球极坐标与错误 Banner 模型（#45~#50）
  - 产物：实现 #45 配置快照 Diff 差异分块投影、#46 地球经纬度极坐标转换、#47 跨国连接贝塞尔飞行光点相位、#48 ErrorBannerState、#49 抽屉焦点栈、#50 自愈步骤旋转信号。
  - 依赖：EFFECTS-004。
  - 验收：单测断言经纬度转三维球面坐标精确，Diff 分块正确识别增删改。
  - 计划：编写 presentation::diff 与 geo 模块。

### 10.3 Layer 2: 共享通用组件库与材质着色基座（7条 / 20pt，涵盖 #51~#75）

- [ ] EFFECTS-014 [kind: widget] [priority: P1] [points: 3] [owner: infiltrator-bevy-widgets] [status: Planned] 现代通用暗色卡片基座与 Hover 微抬升（#51~#52）
  - 产物：实现 #51 带 1px 内描边与 Level 1 柔和阴影的 ModernCard、#52 鼠标悬停 Y 轴 -1px 微位移与高光提亮反馈。
  - 依赖：EFFECTS-002。
  - 验收：多页面复用 ModernCard，悬停反馈流畅跟手无闪烁。
  - 计划：重构 infiltrator-bevy-widgets::card。

- [ ] EFFECTS-015 [kind: widget] [priority: P1] [points: 3] [owner: infiltrator-bevy-widgets] [status: Planned] 主次动作按钮层级与按下态弹性缩放（#53~#54）
  - 产物：实现 #53 ButtonHierarchy（Primary/Secondary/Tertiary 明确层级）、#54 按下态 0.98 比例弹性缩放与 80ms 回弹。
  - 依赖：EFFECTS-003。
  - 验收：按钮交互层次分明，按下松开弹簧反馈细腻。
  - 计划：重构 infiltrator-bevy-widgets::button。

- [ ] EFFECTS-016 [kind: widget] [priority: P1] [points: 3] [owner: infiltrator-bevy-widgets] [status: Planned] 官方 TabList 驱动平滑弹簧分段切换器（#55~#57）
  - 产物：实现 #55 官方 TabList 驱动的平滑弹簧 SegmentedControl、#56 BorderRadius::MAX 胶囊圆角、#57 统一 Switch 控件真源（收敛轨道与旋钮）。
  - 依赖：EFFECTS-003。
  - 验收：切换 Tab 时滑动胶囊带阻尼惯性移动，Switch 几何全仓唯一。
  - 计划：更新 infiltrator-bevy-widgets::tabs 与 switch。

- [ ] EFFECTS-017 [kind: widget] [priority: P1] [points: 3] [owner: infiltrator-bevy-widgets] [status: Planned] 虚拟化列表容器与悬浮细滚动条控件（#58~#61）
  - 产物：实现 #58 原生虚拟化列表基座 VirtualListContainer、#59 闲置自动淡出的悬浮细滚动条 SleekScrollbar、#60 触控惯性减速、#61 平滑滚动入视 SmoothScrollIntoView。
  - 依赖：EFFECTS-009。
  - 验收：千级列表流畅滚动，滚轮与键盘聚焦均能平滑定位到项。
  - 计划：重构 infiltrator-bevy-widgets::list 与 scrollarea。

- [ ] EFFECTS-018 [kind: widget] [priority: P1] [points: 3] [owner: infiltrator-bevy-widgets] [status: Planned] 列表边缘羽化遮罩、徽标与趋势图（#62~#64）
  - 产物：实现 #62 列表两端 16px 透明渐变羽化遮罩、#63 大写字高绝对居中 StatusBadge、#64 迷你延迟趋势图 MiniSparkline。
  - 依赖：EFFECTS-005。
  - 验收：列表滚动边缘平滑淡出不生硬切断，迷你图曲线清晰抗锯齿。
  - 计划：实现 infiltrator-bevy-widgets::mask 与 sparkline。

- [ ] EFFECTS-019 [kind: widget] [priority: P1] [points: 3] [owner: infiltrator-bevy-widgets] [status: Planned] 现代外发光输入框与集成式 SearchBar（#65~#67）
  - 产物：实现 #65 带清除按钮与 2px 外发光环的 TextField、#66 清除操作淡出动画、#67 集成搜索图标与筛选标签的 SearchBar。
  - 依赖：EFFECTS-002。
  - 验收：聚焦时发光环平滑亮起，清除按钮点击平稳清空文本。
  - 计划：重构 infiltrator-bevy-widgets::text_input。

- [ ] EFFECTS-020 [kind: widget] [priority: P1] [points: 2] [owner: infiltrator-bevy-widgets] [status: Planned] 居中模态卡片、就近操作组与抽屉（#68~#75）
  - 产物：实现 #68 MODAL(24) 居中模态 ModalCard、#69 就近操作按钮组、#70 自适应 DrawerContainer、#71 智能 Tooltip、#72 骨架屏 SkeletonLoader、#73 AlertBox、#74 CodeBlock、#75 AccordionItem。
  - 依赖：EFFECTS-002。
  - 验收：抽屉滑入滑出平滑，骨架屏微光扫描动效正常，弹窗操作收敛右侧。
  - 计划：扩充 infiltrator-bevy-widgets 弹出组件家族。

### 10.4 Layer 3: Bevy UI 0.20 渲染管线与 WESL 材质深化（8条 / 24pt，涵盖 #76~#115）

- [ ] EFFECTS-021 [kind: bevy] [priority: P0] [points: 3] [owner: infiltrator-bevy-ui] [status: Planned] InlineBox 全面攻坚与图文徽标混排（#76~#78）
  - 产物：彻底攻坚 #76 bevy_text::InlineBox（消灭 TEXT-01）、#77 日志行级别 Badge 随文混排、#78 连接行状态点与域名文字基线物理居中对齐。
  - 依赖：EFFECTS-001。
  - 验收：日志与连接列表彻底移除 Row 拼凑节点，容器窄化时徽标随文自然折行。
  - 计划：重构 logs_rows.rs、connections_rows.rs 与 shell_readout.rs。

- [ ] EFFECTS-022 [kind: bevy] [priority: P1] [points: 3] [owner: infiltrator-bevy-ui] [status: Planned] 官方菜单与滑块双向绑定（#79~#81）
  - 产物：实现 #79 菜单迁回官方 MenuPopup/MenuItem（消灭 MENU-01）、#80 接入官方 Scrollbar 控件（消灭 SCROLL-01）、#81 接入官方 ScrollIntoView 事件（消灭 SCROLL-02）。
  - 依赖：infiltrator-bevy-widgets。
  - 验收：规则与连接页滑块可拖拽且与内容联动，方向键导航可呼出官方菜单。
  - 计划：重构 menu.rs 与 list.rs。

- [ ] EFFECTS-023 [kind: bevy] [priority: P1] [points: 3] [owner: infiltrator-bevy-ui] [status: Planned] WESL 材质管线、发光与流光着色器（#82~#85）
  - 产物：实现 #82 Bevy 0.20 WESL 着色器迁移、#83 GlassMaterial 磨砂反射材质、#84 PulseGlowMaterial 呼吸发光材质、#85 BorderStreamMaterial 边缘流光材质。
  - 依赖：EFFECTS-004。
  - 验收：WESL 着色器在桌面与移动端安全编译加载，活动节点呈现流光描边。
  - 计划：在 infiltrator-bevy-widgets::shader_fx 建立材质管线。

- [ ] EFFECTS-024 [kind: bevy] [priority: P1] [points: 3] [owner: infiltrator-bevy-ui] [status: Planned] ViewTarget 离屏多 pass 降采样高斯模糊（#86~#89）
  - 产物：实现 #86 基于 RenderApp 与 ViewTarget 的离屏高斯模糊、#87 4x 降采样双 pass 优化、#88 输入输出纹理防冲突分离、#89 前景清晰文字最后顶层合成。
  - 依赖：EFFECTS-023。
  - 验收：弹窗背景呈现平滑高斯模糊，文字刀刻般清晰，显存带宽占用降低 75%。
  - 计划：在 infiltrator-bevy-ui 建立后处理渲染调度子图。

- [ ] EFFECTS-025 [kind: bevy] [priority: P1] [points: 3] [owner: infiltrator-bevy-ui] [status: Planned] 官方 BoxShadow、渐变与 UiTransform 位移（#90~#92）
  - 产物：实现 #90 全量卡片应用 0.20 原生 BoxShadow、#91 BackgroundGradient/BorderGradient、#92 动效位移走 UiTransform 避免触发 Relayout。
  - 依赖：EFFECTS-002。
  - 验收：悬停位移不触发布局树重新计算，阴影平滑多阶。
  - 计划：更新各页面的卡片定义。

- [ ] EFFECTS-026 [kind: bevy] [priority: P1] [points: 3] [owner: infiltrator-bevy-ui] [status: Planned] ECS 调度 UiMotionPlugin 弹簧组件系统（#93~#97）
  - 产物：实现 #93 UiMotionPlugin 动画系统调度、#94 SpringPosition 弹簧物理积分、#95 挂载全量订阅 bevy_scene::Ready、#96 commands.despawn_all 批量退役、#97 bsn_list! 列表宏。
  - 依赖：EFFECTS-003。
  - 验收：动效由 ECS 系统统一调度，首帧挂载无异步竞态，批量销毁彻底。
  - 计划：实现 infiltrator-bevy-ui::motion_plugin。

- [ ] EFFECTS-027 [kind: bevy] [priority: P1] [points: 3] [owner: infiltrator-bevy-ui] [status: Planned] 3D 地球 UI 渲染到纹理与射线拾取映射（#98~#99）
  - 产物：实现 #98 Geo Globe 3D 相机 UI 渲染到纹理（RTT）、#99 视口点击坐标向 3D 球面 UV 与鼠标射线逆变换映射。
  - 依赖：infiltrator-bevy-widgets::globe。
  - 验收：3D 地球作为独立卡片内嵌渲染，点击球体节点准确高亮并弹出详情。
  - 计划：重构 pages/globe.rs。

- [ ] EFFECTS-028 [kind: bevy] [priority: P1] [points: 3] [owner: infiltrator-bevy-ui] [status: Planned] 核心页面表现重构与 WgpuSettings 动态限额（#100~#115）
  - 产物：收口 #100~#113 全部 14 项页面缺陷（侧栏文字重叠、截断、电光蓝重构、大蓝块平衡、法律文本折叠等）及 #114 弹窗 MODAL 圆角，#115 WgpuSettings 移动端动态限额。
  - 依赖：EFFECTS-021～027。
  - 验收：Bevy 7 大页面在标准/紧凑双视口下全部消除像素缺陷，移动端内存稳定。
  - 计划：逐页应用新材质与组件。

### 10.5 Layer 4: Iced 0.14 原生动画、Canvas 三层缓存与 Shader（7条 / 21pt，涵盖 #116~#145）

- [ ] EFFECTS-029 [kind: iced] [priority: P1] [points: 3] [owner: infiltrator-iced] [status: Planned] Iced 0.14 官方 Animation 状态插值与帧订阅（#116~#118）
  - 产物：实现 #116 官方 iced::Animation 接入、#117 按需订阅 window::frames()、#118 模态抽屉退出动画保留（Presence）。
  - 依赖：EFFECTS-003。
  - 验收：更新消息触发 go_mut 重定向，动画活跃时推进帧，静止时注销订阅。
  - 计划：在 infiltrator-iced 建立 animation 协调器。

- [ ] EFFECTS-030 [kind: iced] [priority: P1] [points: 3] [owner: infiltrator-iced] [status: Planned] container::Style 语义映射与多色标线性渐变（#119~#121）
  - 产物：实现 #119 container::Style 映射语义 Token Catalog、#120 最多 8 色标线性渐变应用、#121 Shadow 分阶投影。
  - 依赖：EFFECTS-001。
  - 验收：控件外观由主题驱动，按钮与顶栏呈现柔和渐变与高光边框。
  - 计划：重构 infiltrator-iced::style。

- [ ] EFFECTS-031 [kind: iced] [priority: P1] [points: 3] [owner: infiltrator-iced] [status: Planned] 图表三层 Canvas 绘制分离与增量失效机制（#122~#125）
  - 产物：实现 #122 图表三层 Canvas 分离、#123 坐标轴 BackgroundCache 静态缓存、#124 数据曲线 DataGeometryCache 增量失效、#125 悬浮十字星零缓存即时绘制。
  - 依赖：EFFECTS-008。
  - 验收：鼠标高频移动时仅重绘交互层，静态标尺与折线零开销。
  - 计划：重构 view/overview/chart.rs。

- [ ] EFFECTS-032 [kind: iced] [priority: P1] [points: 3] [owner: infiltrator-iced] [status: Planned] 贝塞尔平滑吞吐曲线与拓扑流动水流自绘（#126~#127）
  - 产物：实现 #126 Canvas 贝塞尔平滑流量曲线与渐变阴影填充、#127 代理节点连线与活跃水流脉冲粒子自绘。
  - 依赖：EFFECTS-031。
  - 验收：实时吞吐平滑流动，连线光点根据流量速率动态调节流动频率。
  - 计划：实现 view/chart/spline.rs。

- [ ] EFFECTS-033 [kind: iced] [priority: P1] [points: 3] [owner: infiltrator-iced] [status: Planned] wgpu 自定义 Shader 控件与 tiny-skia 兜底（#128~#131）
  - 产物：实现 #128 widget::Shader 控件扩展、#129 Pipeline/BindGroup 预编译缓存、#130 tiny-skia 软件光栅化静态替代画面、#131 运行时后端降级侦测。
  - 依赖：EFFECTS-004。
  - 验收：wgpu 下显示动态流光材质，软件渲染模式下平滑回退至 Canvas 矢量画面。
  - 计划：实现 view/shader/primitive.rs。

- [ ] EFFECTS-034 [kind: iced] [priority: P1] [points: 3] [owner: infiltrator-iced] [status: Planned] 侧边栏与代理页面卡片网格对齐重构（#132~#136）
  - 产物：实现 #132 侧边栏导航架构与 Bevy 完全对齐、#133 路由模式文字拥挤调整、#134 Toggle 比例微调、#135 节点多列网格视图模式、#136 测速波浪细线呼吸。
  - 依赖：EFFECTS-030。
  - 验收：Iced 与 Bevy 导航项一致，节点支持整行与网格双重视图切换。
  - 计划：重构 view/sidebar.rs 与 view/proxies.rs。

- [ ] EFFECTS-035 [kind: iced] [priority: P1] [points: 3] [owner: infiltrator-iced] [status: Planned] 规则行悬浮淡入与命令面板毛玻璃（#137~#145）
  - 产物：实现 #137 规则行操作悬浮淡入、#138 分类彩色徽标、#139 域名端口弱化、#140 连接断开按钮悬浮保护、#141 命令面板半透明遮罩、#142 联想项平移、#143 Wayland 文件导入兜底、#144 IME 候选框定位、#145 iced_anim 弹簧试验。
  - 依赖：EFFECTS-029。
  - 验收：复杂列表视觉噪点大幅收敛，命令面板呼出呈现优雅毛玻璃层级。
  - 计划：重构 view/rules.rs、view/connections.rs 与 view/shell.rs。

### 10.6 Layer 5: Slint 1.18+ Skia 硬件加速、桌面多列网格与原生状态机（6条 / 17pt，涵盖 #146~#170）

- [ ] EFFECTS-036 [kind: slint] [priority: P1] [points: 3] [owner: infiltrator-slint] [status: Planned] Winit+Skia 硬件加速升级与 GPU VSync 垂直同步（#146~#149）
  - 产物：实现 #146 桌面端启用 renderer-skia 特性、#147 开启 GPU VSync 垂直同步、#148 保留 renderer-femtovg 轻量通道、#149 预留 backend-qt 原生桥接。
  - 依赖：无。
  - 验收：桌面确认 Skia GPU 加载，帧率稳定在 60~120fps，CPU 占用降至个位数。
  - 计划：更新 Cargo.toml 与 build.rs。

- [ ] EFFECTS-037 [kind: slint] [priority: P1] [points: 3] [owner: infiltrator-slint] [status: Planned] Zinc 多阶暗底系统与桌面多列仪表板网格（#150~#154）
  - 产物：实现 #150 Zinc 8 阶微差暗色底板系统、#151 原生 drop-shadow 漫反射双阶阴影、#152 1px 半透明内描边、#153 桌面多列响应式 GridLayout、#154 消除 70% 纯黑空洞。
  - 依赖：EFFECTS-036。
  - 验收：1180x780 桌面呈现饱满双列网格，消灭纯黑 void，空间利用率大幅提升。
  - 计划：重构 theme.slint、page_home.slint 与 page_proxies.slint。

- [ ] EFFECTS-038 [kind: slint] [priority: P1] [points: 3] [owner: infiltrator-slint] [status: Planned] 现代 Slint 语法清理、强类型枚举与双向绑定（#155~#159）
  - 产物：消除 #155 透明 Rectangle 间距 hack、#156 剥离全仓手写 * LumenUiScale.factor、#157 声明领域强类型 export enum、#158 全面改用 <=> 双向绑定、#159 收敛全局属性。
  - 依赖：无。
  - 验收：ui/ 目录下无任何透明占位 Rectangle，编译后无语法弃用警告，单测全绿。
  - 计划：系统性重构 ui/*.slint 布局声明。

- [ ] EFFECTS-039 [kind: slint] [priority: P1] [points: 3] [owner: infiltrator-slint] [status: Planned] 交互组件声明式 states 注入与贝塞尔平滑动效（#160~#162）
  - 产物：实现 #160 交互组件全量注入声明式 states 状态机、#161 120ms ease-out 贝塞尔平滑动效、#162 SegmentedButton 弹性滑动胶囊动画。
  - 依赖：EFFECTS-037。
  - 验收：按钮卡片鼠标悬停与点击呈现丝滑微抬升与高光，切换 Tab 滑轨顺畅。
  - 计划：在 base_widgets.slint 与 cards.slint 注入 states 与 transitions。

- [ ] EFFECTS-040 [kind: slint] [priority: P1] [points: 3] [owner: infiltrator-slint] [status: Planned] 列表原生 ListView 虚拟化与增量模型管道（#163~#166）
  - 产物：实现 #163 代理节点列表迁移为原生虚拟化 ListView、#164 规则/连接/日志长列表改用 ListView、#165 Rust 接入 FilterModel/SortModel 零拷贝管道、#166 自定义零拷贝流式 Model。
  - 依赖：EFFECTS-038。
  - 验收：500+ 节点秒级开屏无卡顿，增量搜索即时刷新，内存分配平稳。
  - 计划：重构 page_proxies.slint 与 lumen_projection/collections。

- [ ] EFFECTS-041 [kind: slint] [priority: P1] [points: 2] [owner: infiltrator-slint] [status: Planned] 一体化无边框窗口、Mica/Vibrancy 与软键盘适配（#167~#170）
  - 产物：实现 #167 no-frame 自绘现代标题栏与拖拽区、#168 Windows 11 Mica 亚克力底衬、#169 macOS Vibrancy 毛玻璃底色、#170 移动软键盘避让与安全区。
  - 依赖：EFFECTS-036。
  - 验收：桌面窗口呈现现代操作系统原生材质，移动端输入框自动顶起避让键盘。
  - 计划：更新 desktop.rs 与 app_chrome.slint。

### 10.7 Layer 6: TUI / Ratatui 0.30 终端高阶表现与图片协议（4条 / 11pt，涵盖 #171~#185）

- [ ] EFFECTS-042 [kind: tui] [priority: P1] [points: 3] [owner: infiltrator-cli] [status: Planned] Ratatui 0.30.2 升级与启动期终端协议协商（#171~#173）
  - 产物：实现 #171 升级至 Ratatui 0.30.2 + Crossterm 0.29、#172 启动期终端能力协商探测（Truecolor/Kitty/Sixel）、#173 三级色彩（Truecolor/256/16）自动量化。
  - 依赖：无。
  - 验收：全平台终端启动自动协商色彩与协议，无输入抢占死锁。
  - 计划：更新 infiltrator-cli/Cargo.toml 与终端初始化逻辑。

- [ ] EFFECTS-043 [kind: tui] [priority: P1] [points: 3] [owner: infiltrator-cli] [status: Planned] Unicode/Halfblock 现代边框与 ASCII 回退（#174~#175）
  - 产物：实现 #174 Unicode 与 Halfblock 现代圆角字符边框（╭ ╮ ╯ ╰ 与双线框）、#175 unicode = simple 纯 ASCII 简易字符安全回退模式。
  - 依赖：EFFECTS-042。
  - 验收：现代终端呈现精致圆角线框，老旧或受限终端自动回退纯 ASCII 结构。
  - 计划：实现 cli/ui/borders.rs。

- [ ] EFFECTS-044 [kind: tui] [priority: P1] [points: 3] [owner: infiltrator-cli] [status: Planned] tachyonfx 局部单元格动效与 ratatui-image 协议（#176~#180）
  - 产物：实现 #176 tachyonfx 0.25.2 局部轻量状态转场动效、#177 动效范围严格局限与即停机制、#178 ratatui-image 11.1.0 协议图片渲染、#179 Halfblock 字符艺术回退、#180 Braille 盲文高密度流量波形图。
  - 依赖：EFFECTS-042。
  - 验收：状态变更带轻量字符扫光动效，动效完成立即停止 tick，图表波形精细。
  - 计划：实现 cli/ui/fx.rs 与 cli/ui/chart.rs。

- [ ] EFFECTS-045 [kind: tui] [priority: P1] [points: 2] [owner: infiltrator-cli] [status: Planned] 全键盘效率操作、居中浮动弹窗与恢复守卫（#181~#185）
  - 产物：实现 #181 Vim 风格全键盘快捷键常驻提示、#182 居中浮动弹窗与半透明暗色遮罩、#183 CJK 宽字符与 Emoji 对齐校准、#184 Panic Raw Mode 自动恢复守卫、#185 infiltrator-tui.toml 配置覆盖。
  - 依赖：EFFECTS-042。
  - 验收：中英文字符对齐无跳行，程序即使崩溃也能 100% 恢复光标与终端回显。
  - 计划：实现 cli/ui/key_hints.rs 与 terminal_guard.rs。

### 10.8 Layer 7: 跨平台集成、动态降级策略与实机取证（5条 / 15pt，涵盖 #186~#200）

- [ ] EFFECTS-046 [kind: platform] [priority: P1] [points: 3] [owner: infiltrator-desktop] [status: Planned] 现代无边框标题栏与多系统模糊扩展集成（#186~#189）
  - 产物：实现 #186 三桌面无边框现代一体化标题栏、#187 Windows 11 Dwm 亚克力/云母模糊扩展、#188 macOS NSVisualEffectView 材质混成、#189 Wayland Niri/KWin 窗口模糊扩展。
  - 依赖：无。
  - 验收：Windows/macOS/Wayland 窗口呈现操作系统原生现代高级质感。
  - 计划：在 infiltrator-desktop 实现平台窗口特性扩展。

- [ ] EFFECTS-047 [kind: mobile] [priority: P1] [points: 3] [owner: infiltrator-android] [status: Planned] 移动端安全区避让、手势消歧与旋转自适应（#190~#192）
  - 产物：实现 #190 Android Surface 销毁重建安全恢复、#191 触摸滑动与点击事件仲裁（10px 容差门限）、#192 横竖屏旋转自适应重新布局。
  - 依赖：无。
  - 验收：列表滑动绝不误触发子项点击，Activity 挂起恢复无黑屏崩溃。
  - 计划：更新 infiltrator-android 驱动生命周期处理。

- [ ] EFFECTS-048 [kind: quality] [priority: P1] [points: 3] [owner: infiltrator-presentation + desktop] [status: Planned] 动态画质自适应切换与大纹理显存配额管理（#193~#195）
  - 产物：实现 #193 后台最小化自动暂停高频刷新（1fps/休眠）、#194 帧率自适应动态画质降级引擎（>16.6ms 自动退至 Standard）、#195 显存 64MB 上限限额管理。
  - 依赖：EFFECTS-006。
  - 验收：低性能机器上自动降低采样率与阴影等级，后台挂起 CPU 占用归零。
  - 计划：实现 presentation::performance_watchdog。

- [ ] EFFECTS-049 [kind: a11y] [priority: P1] [points: 3] [owner: 三大 UI owner] [status: Planned] 辅助功能读屏树（AccessKit）标准角色注册（#196）
  - 产物：实现 #196 为 Iced、Bevy、Slint 常用控件注册 AccessibleRole、AccessibleName 与操作动作。
  - 依赖：无。
  - 验收：系统读屏软件（如 Orca / NVDA）能正确读出核心按钮角色、标签与状态。
  - 计划：在各前端适配器注册 a11y 语义树。

- [ ] EFFECTS-050 [kind: evidence] [priority: P1] [points: 3] [owner: scripts/parity] [status: Planned] 虚拟 Wayland 离屏取证与双视口像素级回归矩阵（#197~#200）
  - 产物：实现 #197 虚拟 Wayland（Niri IPC）离屏截屏管线、#198 双视口全业务黑盒像素差自动化回归、#199 Android 三 Flavor 交互截图自动化、#200 全平台矩阵验收报告生成器。
  - 依赖：三大 UI 实施完成。
  - 验收：CI 流水线一键复跑所有 UI 双视口像素差比对，无感知视觉退化被拒。
  - 计划：更新 scripts/parity/ 自动化取证套件。

- [ ] UI-FRAMEWORK-001 [kind: mechanism] [priority: P1] [points: 3] [owner: infiltrator-iced + infiltrator-slint] [status: Planned] 跨表面「框架能力自研重造」余项收口
  - 产物：① Iced 字形勾选框（`view_root/aggregator_modal.rs` 的 `checkbox_glyph`/`toggle_row`、`view/uwp_card.rs` 的 `glyph`）改官方 `iced::widget::checkbox`（点击目标从整行收窄到方框是产品决定，需先定）；② Slint `profile_sheets.slint` 的 Profile-Settings 类型下拉改官方 `PopupWindow`/`PopupMenu`（同族的主题/语言/代理排序菜单已是官方件；S-3 记录的 Android 理由要先在同一 pin 上复核）；③ Slint `base_widgets.slint` 的 `AppColorSlider` 手搓滑块改标准 `Slider`（`step`/`changed`/`released`）或 Lumen `Slider`（`stop_count`），需定 22px 滑块与可换肤轨道的皮肤契约；④ Slint 的 `ScrollView{for …}` 列表改标准 `ListView` 做虚拟化（行高契约以共享域为准）；⑤ Slint 返回手势补齐两个确认弹窗的关闭（与菜单同形）；⑥ Iced 规则虚拟窗口改 `iced::widget::lazy`（需在 pinned 依赖开 `lazy` feature，属共享 owner 层改动）。
  - 依赖：Iced/Slint 表面各自可在自己的视图层内推进；⑥ 需共享 owner 决策。
  - 验收：三个表面各自 nextest 全绿；每项都有「改前/改后」证据；Iced 开关像素回执需重取（改用工具链 2:1 药丸后像素变了，`source_fingerprint` 绑定要重绑）。
  - 事实依据：2026-10-21 按 pinned 依赖源码符号名实测（iced 0.14 / iced_widget 0.14.2 / slint 1.18.1 / Compose M3）：Iced 手绘开关（`canvas::Program`）已改官方 `toggler` 并补 4 条测试；Slint 两个 `inherits Rectangle` 手搓模态已改官方 `PopupWindow` 并删掉产品层 open bool，补 2 条测试；Android Compose 零命中。

- [ ] BEVYUI-003R [kind: visual] [priority: P1] [points: 3] [owner: infiltrator-bevy-ui + infiltrator-bevy-widgets + infiltrator-shared] [status: Planned] 代理页余项与共享改动落地
  - 产物：① #47 延迟迷你折线图（需 owner 的延迟采样历史进投影：`surface_projection.rs` 的 `ProxyNode` 增加 `latency_samples`）；② #39 Custom Node Editor 栏目条加描边与尾部箭头（`business_panel.rs` 的 `launcher_scene`）；③ #52 长名的像素渐隐（需 `palette.rs` 新增不透明 `card_fill()`，否则渐变会二次合成出色带）；④ locale 表新增四个键（分组类型 Fallback/LoadBalance/Relay、测速就绪提示）；⑤ `ProxiesProjection` 增加 `sort_order`，排序条由 owner 顺序播种而非采用备忘；⑥ `widgets/src/tabs.rs` 增加带自定义标记的分段控件入口；⑦ 把"卡片填色必须跑 `Last` 调度"写进共享控件层说明。
  - 依赖：BEVYUI-003 已交付的部分（现在佛已无此条）。
  - 验收：代理页双视口像素回执；#47/#48/#52 的像素证据到位（#48 需带指针的捕获台架，取不到就明确标注为代码级证据）。
  - 事实依据：BEVYUI-003 交付时逐条列出的共享改动需求（文件 + 符号 + 代码片段）。

## 十一、办结与登记

- 本板条目全部验收通过后，按 [TODO.md](../TODO.md) 流转规则同次移入[过去佛](CODE_TODO_PAST.md)（实现已交付、只剩统一批次）或直接办结退出并登记 archived points。
- 需要实机、tag CI、用户 secrets、上架凭证或异构目标机才能收口的条目，在迁移前先完成本板所有可推动作，再整条迁入[斗战胜佛](CODE_TODO_BLOCKED.md)。

### 7.2 共享控件真源补齐（rem 迁移暴露的重复事实）

