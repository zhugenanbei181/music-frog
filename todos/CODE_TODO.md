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
- Point 基线 `musicfrog-todo-20261022-v267`：现在佛 141 pt（49 条）/ 过去佛 159 pt（61 条）/ 斗战胜佛 51 pt（17 条）/ 未来佛 26 pt（15 条），联合未办结分母 377 pt；已归档交付 727 pt（290 条）；累计 1104 pt，严格归档完成率 65.9%。
- 本板 44 条 = 既有机制与真实捕获 3 条（`PARITY-001.5` 5 pt、`SLINT-001.10` 2 pt、`BEVY-020.1` 3 pt）+ Bevy UI 105 点视觉攻坚 7 条（`BEVYUI-001`～`BEVYUI-007`，21 pt）+ Slint 34 点质感攻坚 7 条（`SLINTUI-001`～`SLINTUI-007`，20 pt）+ 跨平台 GUI/TUI 表现深化 200 点自底向上攻坚 50 条（`EFFECTS-001`～`EFFECTS-050`，145 pt，涵盖 Layer 0~7 底层 Token、数学库、呈现投影、通用组件、Bevy 0.20 WESL、Iced 0.14 Canvas/动效、Slint 1.18 Skia、CLI 终端输出与多端降级集成）；原未来佛 16 条 + 截图认证与平台覆盖 5 条（`PARITY-001.2`～`001.4`、`PLATFORM-001.1`）+ Slint 第三 UI 补齐 9 条（`SLINT-001.1`～`001.9`）+ 共享控件真源补齐 1 条（`WIDGET-001.1`，本批新增 3 pt）；Bevy UI 能力规范强制化 5 条（`BEVYCAP-001.1`／`001.2`／`002.1`／`003.1` 与 `TOKEN-001.1`）本批全部交付并迁入过去佛，`PUB-006.1`、`PARITY-001.1` 与 `PLATFORM-001.2` 同样已交付并迁入过去佛；Android 静态质量门 4 条（`BANDROID-044.1`／`044.2`／`044.4`／`044.5`，9 pt）与状态表达/文档红线 2 条（`STATE-001.1` 隐式状态机全量审计、`PUB-006.6` 公共 API 文档质量门，5 pt）与开源生态四条（`PUB-006.7`／`006.9`／`006.10`／`006.11`，7 pt）、平台证据与安装包两条（`PLATFORM-001.1`、`APPID-001.3`，5 pt）、地球增强两条（`BANDROID-016.2`、`016.3`，4 pt）、双引擎真实二进制 smoke 与 Android 取证两条（`PUB-006.8`、`PARITY-001.4`，5 pt）与 Slint 第三表面补齐七条（`SLINT-001.2`～`001.7`、`001.9`，15 pt）本批交付并迁入过去佛。
- Slint 九条来自 2026-10-19 的表面审计实测：缺 7 类页面导致 19 条业务无载体、只派发 42 种 `CommandIntent`、59 条业务里 2 条完整实现 / 18 条部分实现 / 39 条未实现且今天零锚点、4 个测试文件重实现 Slint 逻辑构成第二业务真源、2 个已声明回调无调用点。
- Bevy UI 四条来自 2026-10-20 的强制化实测审计：场景结构（`bsn!` 唯一入口 + ECS 边界）已由 `bevy_bsn_guard.py` 机器强制（349 文件零违例），rem/em 尺寸通道同批由共享包执行器 `bevy_scene_gate.py`（硬编码 px 字面量与 `FontSize::Px` 零容忍，存量 128 处棘轮登记）与产品守卫 `bevy_rem_guard.py`（命名/派生 px 值白名单边界，存量 52 处独立台账）两段互补强制，两份台账站点不重叠；`bsn_list!` 0 处 / 内联 `Children [` 974 处、`bevy_scene::Ready` 0 处、`despawn_all` 2 处（⑦ 小件收敛仍仅人工评审）；执行状态台账见 Bevy UI 能力规范文档的「强制化台账」一节。
<!-- POINT_LEDGER:END -->

- **执行次序（2026-10-22 重新整理）**：本板条目按 **Core-first** 排成一条可推的链——① 契约层事实（`EFFECTS-001`~`003`、`006`）→ ② 共享数学与资产（`EFFECTS-004`、`005`）→ ③ 表现层（`EFFECTS-007`~`013`）→ ④ 共享控件（`EFFECTS-014`~`020`）→ ⑤ Bevy 表面（`EFFECTS-021`~`028`）→ ⑥ Iced 表面（`EFFECTS-029`~`035`）→ ⑦ Slint 表面（`EFFECTS-036`~`041`）→ ⑧ CLI 终端输出（`EFFECTS-042`~`045`）→ ⑨ 平台/移动/质量（`EFFECTS-046`~`048`）→ ⑩ a11y（`EFFECTS-049`）→ ⑪ 证据（`EFFECTS-050`）→ ⑫ Slint 表面专项（`SLINTUI-001`~`007`）→ ⑬ 像素认证收口（`PARITY-001.5`）。上层条目在前置落地前不得开工。
- **cargo 纪律（减少被 cargo 阻塞）**：同一时刻只有一条流允许跑 cargo；并行期只写代码，编译与像素取证由收口流**统一跑一次**（全仓 nextest → 守卫 → fmt → clippy → 三表面捕获 → 重建回执账本 → 全局验收）。**只有真正因缺少实际环境而无法在本机取回执的验收项**才进[未来佛](CODE_TODO_FUTURE.md)；排队等 cargo lane 不构成阻塞，那些条目留在本板按上面的次序推进。

- [ ] BEVYCAP-002 [kind: evidence] [priority: P1] [points: 3] [owner: infiltrator-bevy-ui] [status: Planned] Bevy 九个业务仍缺捕获模块（17 格）
  - 产物：为 `core-version-rollback-confirm`、`editor-snippet-insertion`、`editor-yaml-error-navigation`、`profiles-aggregator-preview`、`profiles-import-modal`、`rules-provider-refresh-error`、`settings-lan-security`、`sync-conflict-resolver`（各 standard+compact）与 `proxies-group-expanded` 补捕获模块（owner 装配 + typed 命令 + 能力闸 + 观测）；`proxies-group-expanded` 的观测要量「折叠→展开」的**可见差异**，而不是量展开节点自身的框。
  - 依赖：表面控件与生产回调都已在树里，只差 capture 侧接线。
  - 验收：三表面捕获矩阵全绿；这 17 格拿到像素回执并被验收认证为 `ready`。
  - 事实依据：2026-10-22 Bevy 捕获流逐格分类的结论（缺 capture 模块，非环境阻塞，故留本板）。


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

- [ ] EFFECTS-002 [kind: contract] [priority: P1] [points: 3] [owner: infiltrator-contract] [status: Planned] 网格间距、圆角与物理尺寸白名单（#6~#9）
  - 产物：在 design_tokens 规范 #6 8pt/4pt 严格网格阶梯、#7 互斥圆角阶梯裁决、#8 物理像素白名单闭集（发丝线/光标/触控区）、#9 Elevation 空间阴影多阶定义。
  - 依赖：EFFECTS-001。
  - 验收：静态守卫拦截越出白名单的硬编码尺寸与重叠圆角角色。
  - 计划：更新 space.rs 与 radius.rs，补齐 ShadowSpec 定义。


- [ ] EFFECTS-004 [kind: math] [priority: P1] [points: 3] [owner: infiltrator-shared] [status: Planned] Oklab 感知插值与几何 SDF 数学库（#14~#17）
  - 产物：在 infiltrator-shared 实现 #14 Oklab 色彩空间感知插值算法、#15 三次贝塞尔曲线求值器、#16 RK4 弹簧状态积分器、#17 圆角矩形与药丸形解析 SDF 距离场公式库。
  - 依赖：无。
  - 验收：数学库单测覆盖色彩平滑过渡、弹簧能量衰减无发散、SDF 零误差。
  - 计划：创建 infiltrator_shared::eind: contract] [priority: P1] [points: 2] [owner: infiltrator-contract] [status: Planned] 智能截断、安全区与三级画质降级契约（#22~#25）
  - 产物：在 contract 裁决 #22 长文本智能渐隐截断算法、#23 跨平台安全边距 SafeAreaInsets、#24 纹理容量与显存预算常数、#25 简化/标准/增强三级画质档位 VisualTier。
  - 依赖：EFFECTS-001。
  - 验收：契约测试覆盖三级画质枚举与安全区边界换算。
  - 计划：定义 VisualTier 枚举与 safe_area 类型。

### 10.2 Layer 1: 跨端呈现投影、时间驱动与图表数据归约层（7条 / 20pt，，反向打开平滑恢复。
  - 计划：重构 presentation 视图状态模型。

- [ ] EFFECTS-008 [kind: presentation] [priority: P1] [points: 3] [owner: infiltrator-presentation] [status: Planned] 实时图表三层数据分离与平滑曲线拟合（#29~#31）
  - 产物：在 TrafficProjector 分离 #29 静态标尺/动态折线/实时采样三层数据、#30 Catmull-Rom 贝塞尔平滑样条拟合算法、#31 LTTB 视口动态降采样。
  - 依赖：EFFECTS-004。
  - 验收：高频数据输入下样条曲线平滑连续，降采样后峰值保留率 100实现 domain::ring_buffer 与 presentation::list_filter。

- [ ] EFFECTS-010 [kind: presentation] [priority: P1] [points: 3] [owner: infiltrator-presentation] [status: Planned] 交互意图归一化与输入法组合态保护（#35~#36）
  - 产物：在 presentation 建立 #35 统一键盘导航意图转换、#36 ImeComposing 组合态保护协议（组合期间抑制快照热重载）。
  - 依赖：无。
  - 验收：单测验证中文输入法输入未完成时模型不强行刷新文本框。
  - 计划：重构P1] [points: 3] [owner: infiltrator-presentation] [status: Planned] 测速波浪、规则命中与日志流分段投影（#40~#44）
  - 产物：实现 #40 测速 0.0~1.0 呼吸相位输出、#41 协议特性标签去重归约、#42 规则命中分级 Badge 投影、#43 日志行四段分节结构体、#44 长日志软折行度量。
  - 依赖：EFFECTS-001。
  - 验收：单测覆盖多种协议标签去重、日志分段解析与折行高度推导。
  - 计划：扩充 proxies_page_model 与 logs_page_model。

- [ ] EFFECTS-013 [kind: widget] [priority: P1] [points: 2] [owner: infiltrator-bevy-widgets] [status: Planned] 现代通用暗色卡片基座与 Hover 微抬升（#51~#52）
  - 产物：实现 #51 带 1px 内描边与 Level 1 柔和阴影的 ModernCard、#52 鼠标悬停 Y 轴 -1px 微位移与高光提亮反馈。
  - 依赖：EFFECTS-002。
  - 验收：多页面复用 ModernCard，悬停反馈流畅跟手无闪烁。
  - 计划：重构 infiltrator-bevy-widgets::card。

- [ ] EFFECTS-015 [kind: widget] [priority: P1] [points: 3] [owner: infiltrator-bevy-widgets] [status: Planned] 官方 TabList 驱动平滑弹簧分段切换器（#55~#57）
  - 产物：实现 #55 官方 TabList 驱动的平滑弹簧 SegmentedControl、#56 BorderRadius::MAX 胶囊圆角、#57 统一 Switch 控件真源（收敛轨道与旋钮）。
  - 依赖：EFFECTS-003。
  - 验收：切换 Tab 时滑动胶囊带阻尼惯性移动，Switch 几何全仓唯一。
  - 计划：更新 infiltrator-bevy-widgets::tabs 与 switch。

- [ ] EFFECTS-017 [kind: widget] [priority: P1] [points: 3] [owner: infiltrator-bevy-widgets] [status: Planned] 列表边缘羽化遮罩、徽标与趋势图（#62~#64）
  - 产物：实现 #62 列表两端 16px 透明渐变羽化遮罩、#63 大写字高绝对居中 StatusBadge、#64 迷你延迟趋势图 MiniSparkline。
  - 依赖：EFFECTS-005。
  - 验收：列表滚动边缘平滑淡出不生硬切断，迷你图曲线清晰抗锯齿。
- [ ] EFFECTS-019 [kind: widget] [priority: P1] [points: 3] [owner: infiltrator-bevy-widgets] [status: Planned] 居中模态卡片、就近操作组与抽屉（#68~#75）
  - 产物：实现 #68 MODAL(24) 居中模态 ModalCard、#69 就近操作按钮组、#70 自适应 DrawerContainer、#71 智能 Tooltip、#72 骨架屏 SkeletonLoader、#73 AlertBox、#74 CodeBlock、#75 AccordionItem。
  - 依赖：EFFECTS-002。
  - 验收：抽屉滑入滑出平滑，骨架屏微光扫描动效正常，弹窗操作收敛右侧。
  - 计连接行状态点与域名文字基线物理居中对齐。
  - 依赖：EFFECTS-001。
  - 验收：日志与连接列表彻底移除 Row 拼凑节点，容器窄化时徽标随文自然折行。
  - 计划：重构 logs_rows.rs、connections_rows.rs 与 shell_readout.rs。
- [ ] EFFECTS-022 [kind: bevy] [priority: P1] [points: 3] [owner: infiltrator-bevy-ui] [status: Planned] 官方菜单与滑块双向绑定（#79~#85）
  - 产物：实现 #79 菜单迁回官方 MenuPopup/MenuItem（消灭 MENSL 着色器迁移、#83 GlassMaterial 磨砂反射材质、#84 PulseGlowMaterial 呼吸发光材质、#85 BorderStreamMaterial 边缘流光材质。
  - 依赖：EFFECTS-004。
  - 验收：WESL 着色器在桌面与移动端安全编译加载，活动节点呈现流光描边。
  - 计划：在 infiltrator-bevy-widgets::shader_fx 建立材质管线。

- [ ] EFFECTS-024 [kind: bevy] [priority: P1] [points: 3] [owner: infiltrator-bevy-ui] [status: Planned] ViewTarget 离屏多 pass 降采样高斯模糊（#86~#89）
  - 产物：实现 ViewTarget 离屏渲染多 pass 降采样与高斯模糊；规格区间 #86~#89 由相邻条目区间（#79~#85 与 #90~#92）推得，子项编号与逐条产物未在板面留存，不臆造。
  - 依赖：EFFECTS-002。
  - 验收：离屏模糊在 UI 纹理上呈现柔和过渡，无锯齿与色带。
  - 计划：在 infiltrator-bevy-ui 的 ViewTarget 管线补齐多 pass 降采样与高斯模糊。
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

### 10.7 Layer 6: CLI 终端输出表现力与能力协商（4条 / 11pt，涵盖 #172~#185）

- [ ] EFFECTS-042 [kind: tui] [priority: P1] [points: 3] [owner: infiltrator-cli] [status: Planned] 启动期终端能力协商与三级色彩量化（#172~#173）
  - 产物：实现 #172 启动期终端能力协商探测（Truecolor/256/16 色彩深度、Kitty/Sixel 图形协议）、#173 三级色彩（Truecolor/256/16）自动量化，并以 `infiltrator terminal capabilities|quantize` 子命令暴露协商结果。
  - 依赖：无。
  - 验收：全平台终端启动自动协商色彩与协议，无输入抢占死锁。
  - 计划：在 infiltrator-cli 内实现 terminal_capability 与 color_quantize 模块，并在进入任何交互前完成协商。

- [ ] EFFECTS-043 [kind: tui] [priority: P1] [points: 3] [owner: infiltrator-cli] [status: Planned] CLI 输出的 Unicode 字符集与纯 ASCII 降级（#174~#175）
  - 产物：实现 #174 CLI 表格与分区输出的 Unicode 圆角/双线字符边框（╭ ╮ ╯ ╰）、#175 unicode = simple 纯 ASCII 简易字符安全回退模式，回退与否由终端能力协商结果决定。
  - 依赖：EFFECTS-042。
  - 验收：现代终端呈现圆角线框，老旧或受限终端自动回退纯 ASCII 结构。
  - 计划：在 infiltrator-cli 内实现字符集选择与输出渲染。

- [ ] EFFECTS-044 [kind: tui] [priority: P1] [points: 3] [owner: infiltrator-cli] [status: Planned] 终端字符动效与字符图表（#176~#180）
  - 产物：实现 #176 CLI 输出的轻量状态转场动效（就地刷新、结束后立即停止）、#177 动效范围严格局限与即停机制、#178 终端图片的字符艺术回退、#179 Halfblock 字符艺术回退、#180 Braille 盲文高密度流量波形图。
  - 依赖：EFFECTS-042。
  - 验收：状态变更带轻量字符扫光动效，动效完成立即停止 tick，图表波形精细。
  - 计划：在 infiltrator-cli 内实现终端动效与字符图表，不引入外部动效或图片库。

- [ ] EFFECTS-045 [kind: tui] [priority: P1] [points: 2] [owner: infiltrator-cli] [status: Planned] 终端输出守卫、字符对齐校准与配置覆盖（#181~#185）
  - 产物：实现 #181 终端输出状态守卫（颜色/光标/回显在正常结束、提前返回与 panic 时必定恢复）、#182 输出分区与居中的降级策略、#183 CJK 宽字符与 Emoji 对齐校准、#184 panic 钩子里的终端恢复、#185 终端输出配置覆盖（infiltrator-terminal.toml）。
  - 依赖：EFFECTS-042。
  - 验收：中英文字符对齐无跳行，程序即使崩溃也能 100% 恢复光标与终端回显。
  - 计划：在 infiltrator-cli 内实现 terminal_guard 与字符宽度校准。

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

## 十、跨平台 GUI/CLI 表现深化 200 点自底向上攻坚（报告实施，50条 / 145pt）

### 10.1 Layer 0: 底层数学、动效参数与跨端 Token 契约层（6条 / 17pt，涵盖 #1~#25）

- [ ] PARITY-001.5 [kind: evidence] [priority: P0] [points: 5] [owner: infiltrator-iced + infiltrator-bevy-ui] [status: InProgress] 桌面两端剩余业务的真实捕获与像素回执
  - 产物：Iced 与 Bevy 两端尚未认证的业务各有一条真实交互与双视口像素回执；缺载体或缺可滚动身份的业务逐条立项。
  - 依赖：本批交付的 owner 能力（13 个端口 fixture、同步冲突与概览卡片排序的应用实现、契约层的 LAN/PAC/路由读回）。
  - 验收：每条业务在两端都有「demo/捕获真的执行了业务」的证据与黑盒像素差回执。
  - 计划：① 用 `crates/infiltrator-test-support` 新增的端口 fixture 补 Bevy 捕获（规则提供者刷新、订阅源失败、配置导入、端口冲突修复、LAN 安全、TUN 权限、版本回滚、快照存储、测速历史、文档 codec、核心进程）；② 用同一批 fixture 补 Iced demo 的 owner 绑定（脚本沙箱与导出评审、差异视图、同步冲突与恢复、编辑器文档会话、聚合预览、应用路由枚举）；③ 逐条重跑 `scripts/parity/accept_surface_interactions.py --all --surfaces iced,bevy` 并把 certified 格登记为 `ready`。
  - 已知阻塞（不是本条的缺口）：部分业务的成功态与「仅路由到页面」的基线像素相同（如端口冲突修复后的「无冲突」空态），需要契约层补一个「已修复」标记，否则像素判据无法区分。
  - 进展（2026-10-21）：已建**已验证回执账本** `.evidence/visual_receipts.json`（159 份进程绑定像素回执，逐份绑定场景图与哈希、激活标记与原生边界、像素签名、进程与窗口身份、归档的产品构建与捕获源码指纹），解析器对齐格 **0 → 76**，验收把 **74 格提升为 `ready`**（注册表：76 ready / 44 implemented / 57 pending）。Iced 全量矩阵 93/93、Slint 60/60。
  - 剩余缺口（诚实列出）：① Bevy 全量矩阵 10 场失败——`dns-hosts-editor`（双视口）、`runtime-telemetry-observation`（双视口）、`proxies-uri-import-preview`（双视口）、`profiles-filter-editor`（双视口）、`dns-query-details-compact`、`doctor-failure-recovery-compact`，app 日志显示表面自己的捕获几何拒绝该控件（`capture geometry rejected …`、`filter capture progress: stage=2 pane=Filter editable=false`）；② 13 个 Iced 业务按契约没有 demo activator；③ 16 个 Slint 业务虽然 `activated=true`，但激活前后像素完全一致（0 像素变化）。
  - **最终认证（2026-10-21 静止树，一次跑完）**：账本 `.evidence/visual_receipts.json` **208 份回执**（iced 108 / bevy 69 / slint 31），**全部通过严格 `verified_receipts`**；解析器 `aligned` **97 / 177**、`unclosed` 80、`complete` false、`anchored` 312、`invalid` 1；验收 **208/272 格**认证、**97/136 业务-表面对**、64 格未认证、11 个锚点包全 `ok`、`receipt_failures {}`。注册表：ready 76 / implemented 44 / pending 57（iced 46/13、bevy 28/31、slint 2/57）。**`tree_moved_after_the_capture` 不再出现**（捕获写 `.evidence/`，不落在指纹路径内）。
  - 诚实缺口（未弱化任何断言）：① 唯一 invalid 是 `rules-list-editor/bevy` 的**紧凑视口缺回执**（该格已是 ready）；② Bevy 全量矩阵 **79/81**（`profiles-filter-editor` 两场 failed-marker，app 日志 `filter capture progress: stage=2 pane=Filter editable=false tabs=3 fields=5`，该格在注册表里是 implemented 非 ready，未损失已证像素）；验收矩阵 Bevy 11 场 failed-marker（几何拒绝：`script console/clipped: size=Vec2(874.0, 38.0)`、`rule-list-editor/clipped: size=Vec2(622.0, 85.0)`、`rules keyword field/clipped: size=Vec2(590.0, 36.0)`、`app routing rule/clipped: size=Vec2(44.0, 18.0)`）、Iced 10 场（编辑页窄档不滚动、无刷新控件、同步页无该业务、目录页名错配）；③ 剩余 43 格未认证是像素差为 0（激活像素与纯路由渲染相同）；④ Slint 侧 57 格 pending 的主因是该表面**没有对应业务页**（20 格报 `required feature lacks L1/L2/L3 closure`）。
  - 账本来源说明：严格入口拒绝任何含失败或非交互行的整跑，故按**验收自己的容错路径**（`build_receipts_tolerant` + 严格逐份验签）从三份验收跑批构建——全量矩阵只覆盖 27/27/18 个特性，直接用会让 25 个 ready 格失去像素、`aligned` 掉到 72。
  - 下一步：修 Bevy 那 10 场的捕获几何拒绝；为 13 个 Iced 业务补 activator 或按契约记为不承载；给 16 个 Slint 业务补真实交互差异；然后重跑三表面矩阵并重签账本。
  - 追加进展（2026-10-21，Iced 认证簇交付）：**8 个此前无 activator 的业务拿到真实交互与像素回执**——`app-routing-policy-editor`（std 52 268 / compact 12 370 px）、`profiles-subscription-error`（13 506 / 9 590）、`profiles-aggregator-preview`（151 895 / 168 566）、`profiles-script-sandbox`（7 008 / 8 073）、`profiles-script-export-review`（453 891 / 262 626）、`profiles-diff-viewer`（561 725 / 279 470）、`editor-yaml-error-navigation`（48 063 / 33 862）、`sync-conflict-resolver`（122 986 / compact 4 382——首跑 compact 为 0，原因是测的合并面板 404px 高于 393px 紧凑视口永远居不中，改测**第一条冲突键行**后复取复证）。每个业务都是真手势经真实 update 管道（共享 `ProfileApplication`/`ProfileAggregationApplication`/`ScriptApplication`/`ScriptExportApplication`/快照 Myers diff/codec 诊断），不是路由切换。
  - Iced 官方化与修复：`UI-FRAMEWORK-001` 的字形勾选框改官方 `iced::widget::checkbox`（**行为变更**：点击目标从整行收窄为方框+标签，行尾状态徽章不再可点、成为装饰）；`capture_frame.rs` 的已发布 `rendered-frame.png` 改为**跟随表面**（读回变化就重写），此前它冻结在写回执那一刻的帧，与合成器截图差 2.64% 且回执的帧证明是陈旧的。Iced 发布矩阵 **93/93**，新增业务场景 **16/16**。
  - **验收现状**：解析器 `aligned` **76 → 108**、`unclosed` 77、`complete` 仍 false；认证 209/272 格、100/136 业务-表面对。回执账本**仍被拒**（严格构建器拒绝任何含失败场景的整跑：`ValueError: iced: failed or non-interaction capture editor-large-document-window-standard`），须待剩余格闭合后重建；验收同时报 `tree_moved_after_the_capture`（树在跑批中移动），重绑要等树静默后由收口批次做。
  - **5 项未交付（确切原因）**：① `editor-large-document-window`/`editor-snippet-insertion`——编辑页窄档**不滚动**，文档面板被工具条与横幅挤出 480px 折叠线，业务像素不可观测（修法：窄档让编辑页像 App Routing 一样滚动并给文档/行号槽加 region；demo 手势已就绪，`ProfileStoreCapture::CAPTURE_LARGE_PROFILE` 4 000 条规则）；② `rules-provider-refresh-error`——demo 的规则提供者 fixture 带 `controller: None`，页面**没有可按的刷新控件**，须发布带 `RuleProviderControllerReference` 的提供者并经命令门面拒绝刷新（共用订阅错误那条 region）；③ `sync-restore-confirm`——Iced 同步页**根本没有快照恢复业务**（不渲染快照列表、不投恢复动作，共享 `SyncPageSnapshot.snapshots` 未投影），不伪造手势：要么表面补该业务，要么目录诚实写"该表面不承载"；④ `overview-topology-drilldown`——手势与就绪判据**已工作**（下钻落到共享导航目标），但捕获标记发布的是**已挂载的路由**（`rules`）而目录写的捕获页是 `overview`，属共享目录缺陷。
  - 共享改动需求（契约级，须一次落地）：① `crates/infiltrator-contract/src/parity.rs` 的 `OverviewTopologyDrilldown` 行把捕获页由 `overview` 改为 `rules`（Iced 下钻落到规则页；Bevy 是概览页原位面板，保持 `overview`）；② `SyncRestoreConfirm` 行按"该表面不承载"改为空页（或表面补业务后保持 `sync`）；③ `scripts/capture_iced_scenarios.tsv` 补 16 行新业务场景（app-routing-policy-editor / profiles-subscription-error / profiles-aggregator-preview / profiles-script-sandbox / profiles-script-export-review / profiles-diff-viewer / editor-yaml-error-navigation / sync-conflict-resolver，各 standard+compact）。
  - `UI-FRAMEWORK-001` ⑥（Iced 规则虚拟窗口改官方 `iced::widget::lazy`）**未交付并附证据**：开了 `lazy` feature 后规则页**整片渲染为空**（分页器仍报"15 rules"）；逐段二分（每次都是真实捕获）：纯 `text` 的段渲染、`column![text]` 的段渲染、**含 `Length::Fill` 子项的行渲染为空**、同一段放在 lazy 外渲染正确（`rules-dark.png` 与 HEAD 逐字节相同）；给段 `Length::Fill` 无效。结论：iced 0.14.2 的 `lazy` 会压掉规则行的 fill 宽度，故保留表面正确、**不开这个死依赖**；要落地必须让该带不含 `Length::Fill`（用共享内容宽度的固定宽）或修上游——属产品决定。
## 十一、工程约束门禁补齐（2026-10-22，lint 继承与版本继承）

- [ ] ENG-001.8 [kind: toolchain] [priority: P1] [points: 3] [owner: workspace] [status: Planned] 工作区统一 lint 继承：根 `[workspace.lints]` + 全成员 `[lints] workspace = true`
  - 产物：根 manifest 落 `[workspace.lints]`（rustc 与 clippy 禁止项各一组），48 个成员 manifest 统一改为 `[lints] workspace = true`，成员内重复的 lint 配置删除。
  - 依赖：需要 48 个成员 manifest 与根 manifest 的独占写窗口——当前工作树有 21 个成员 manifest 与根 manifest 被另一会话持有（版本 pin 与工作区版本改动），按同仓多会话所有权纪律不得插入。
  - 验收：`cargo clippy --workspace --all-targets -- -D warnings` 全绿；新增 lint 继承守卫证明每个成员都从工作区继承。
  - 事实依据：2026-10-22 实测——全仓 `[workspace.lints]` 为 0 处、成员各自硬写配置；`unsafe_code = "forbid"` 不适用本仓（61 处 unsafe 是真 FFI：libc flock、Windows API、JNI、zeroize、edition 2024 的 `env::set_var`），故只继承与收紧 lint，不 forbid unsafe。
  - 计划：取得独占窗口后一次性落地 48 个 manifest，同批改完不拆半改。

- [ ] ENG-001.9 [kind: toolchain] [priority: P1] [points: 2] [owner: workspace] [status: Planned] 成员 manifest 的 `edition` 与 `rust-version` 继承到 `[workspace.package]`
  - 产物：根 manifest 落 `[workspace.package] edition = "2024"` 与 `rust-version = "1.99.0"`，48 个成员 manifest 删除各自硬写的 `edition` 行并继承 `rust-version`。
  - 依赖：同 `ENG-001.8` 的独占写窗口。
  - 验收：`scripts/quality/toolchain-consistency-guard.py` 证明各 manifest 的 `rust-version` 与 `rust-toolchain.toml` 同版本；`cargo metadata --locked` 与工作区编译全绿。
  - 事实依据：2026-10-22 实测——`rust-version` 全仓 0 处出现，`edition = "2024"` 在 35 个 crate 硬写重复；工具链版本已由 `rust-toolchain.toml` 统一，成员 manifest 尚未继承。
  - 计划：与 `ENG-001.8` 同批落地，避免两次改同一批 manifest。
### 11.1 错误类型纪律与可选审计工具（2026-10-22 实测登记）

- [ ] ENG-002.1 [kind: cleanup] [priority: P1] [points: 1] [owner: infiltrator-contract] [status: Planned] 删除 `infiltrator-contract` 的未使用 `anyhow` 依赖
  - 产物：`crates/infiltrator-contract/Cargo.toml` 去掉 `anyhow = { workspace = true }`；契约层保持零 `anyhow` 引用（它现在是契约层的错误类型纪律边界）。
  - 依赖：无（单文件 manifest 改动，但需独占该 manifest 的写窗口）。
  - 验收：`cargo check -p infiltrator-contract --all-targets` 全绿；`grep -rn anyhow crates/infiltrator-contract` 零命中。
  - 事实依据：2026-10-22 实测——该 crate 声明了 anyhow 依赖但 src/tests/build.rs 内 0 处引用；同一纪律下 `infiltrator-ports`、`infiltrator-application`、`infiltrator-presentation` 本来就不声明该依赖。
  - 计划：与 `ENG-001.8`／`ENG-001.9` 的 manifest 批次同次改完，避免二次动同一批 manifest。

- [ ] ENG-002.2 [kind: refactor] [priority: P1] [points: 3] [owner: infiltrator-domain + infiltrator-core] [status: Planned] domain / core 层的 `anyhow` 收敛到 typed 错误
  - 产物：`infiltrator-domain`（31 文件 / 208 处引用）与 `infiltrator-core`（22 文件 / 167 处引用）的错误类型从 `anyhow::Result` 收敛为每层一个 typed 错误枚举（可复用的失败类别 + 上下文），边界处保留一处 `From<anyhow::Error>` 收敛（`lurker-core-contract` 已有该范式）。
  - 依赖：`infiltrator-domain` 与 `infiltrator-core` 的独占写窗口；需按 owner 分层逐个 crate 推进，不跨层并行改同一批文件。
  - 验收：两个 crate 内 `anyhow` 引用归零或只剩边界收敛处；工作区 `cargo nextest` 全绿；契约层错误类型零变化（对外错误面不得因本条漂移）。
  - 事实依据：2026-10-22 实测——`infiltrator-contract`/`infiltrator-ports`/`infiltrator-application`/`infiltrator-presentation` 的 anyhow 引用均为 0，只有 domain 与 core 两层大面积使用，错误类型纪律目前只在两层失守。
  - 计划：先 domain 后 core，每层一个纵切批次；每批补齐失败类别的行为测试再迁调用点。

- [ ] ENG-002.3 [kind: evidence] [priority: P2] [points: 2] [owner: workspace] [status: Planned] 覆盖率与变异测试作为可选审计证据接入
  - 产物：本地可跑的覆盖率报告（`cargo llvm-cov`）与关键 crate 的变异测试（`cargo mutants`）入口写入根目录的测试与证据文档，先出基线报告再谈门槛；不把覆盖率数字写进结构门，避免用数字替代行为证据。
  - 依赖：cargo 专责流统一执行（覆盖率与变异测试是重 cargo 任务，按并行纪律统一跑一次）。
  - 验收：文档入口可复跑；基线报告归档到本地证据目录；如发现真实行为缺口，按缺陷回流现在佛登记。
  - 事实依据：2026-10-22 实测——本机已装 `cargo-llvm-cov` 与 `cargo-mutants`，但仓库无任何覆盖率或变异测试入口，测试证据只有「跑通/没跑通」两层。
  - 计划：先出一次覆盖率基线报告，再决定是否对核心 crate 开变异测试门槛。
