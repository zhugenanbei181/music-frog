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
- Point 基线 `musicfrog-todo-20261024-v289`：现在佛 32 pt（12 条）/ 过去佛 271 pt（101 条）/ 斗战胜佛 51 pt（17 条）/ 未来佛 26 pt（15 条），联合未办结分母 380 pt；已归档交付 727 pt（290 条）；累计 1107 pt，严格归档完成率 65.7%。
- 本板 15 条 = 像素认证收口 1 条（`PARITY-001.5` 5 pt）+ Bevy 捕获证据 1 条（`BEVYCAP-002` 3 pt）+ 200 点表现攻坚现存 1 条（`EFFECTS-041`，2 pt）+ Slint 34 点质感攻坚 6 条（`SLINTUI-002`~`007`，17 pt）+ 工程约束门禁 3 条（`ENG-002.2`、`ENG-003.2`、`ENG-003.3`，5 pt）。
- 2026-10-24 第二十一交付批次：`EFFECTS-040`（#18 持久模型管道本批交付；#163/#164 ListView 随滚动架构切片、#165 FilterModel 第二真源裁决不引入、#166 真增量随流架构，见过去佛 §12.17）；第二十批次见 v288 行（E-039）。
- 2026-10-24 第二十交付批次：`EFFECTS-039`（#160/#161 states 注入 + #162 弹性胶囊控件本批交付，见过去佛 §12.16）；第十九批次见 v287 行（E-038）。
- 2026-10-24 第十九交付批次：`EFFECTS-038`（#155 双 no-op 删除 + 全量分类、#158 实测复核旧式赋值已清零、#159 死属性 8 条删除 233→225，见过去佛 §12.15）；第十八批次见 v286 行（E-037）。
- 2026-10-24 第十八交付批次：`EFFECTS-037`（#150~#153 Zinc 阶梯/双阶阴影/内描边/桌面双列网格本批交付，#154 随布局落地待像素回执，见过去佛 §12.14）；第十七批次见 v285 行（E-035）。
- 2026-10-24 第十七交付批次：`EFFECTS-035`（#137 悬浮淡入本批交付，#138/#139/#141/#144 四点记录找回在树，#142/#143/#145 三腿在案，见过去佛 §12.13）；第十六批次见 v284 行（E-034）。
- 2026-10-24 第十六交付批次：`EFFECTS-034`（#132 导航单一真源本批交付，#135 记录找回在树，#133/#134/#136 视觉腿在案，见过去佛 §12.12）；第十五批次见 v283 行（E-033）。
- 2026-10-24 第十五交付批次：`EFFECTS-033`（#128~#130 流光 Shader 管线 + 矢量孪生本批交付，#131 类型开关在案、运行时腿待实机，见过去佛 §12.11）；第十四批次见 v282 行（E-032）。
- 2026-10-24 第十四交付批次：`EFFECTS-032`（#126 贝塞尔+渐变填充本批交付，#127 脉冲粒子记录找回在树，见过去佛 §12.10）；第十三批次见 v281 行（E-031）。
- 2026-10-24 第十三交付批次：`EFFECTS-031`（#122~#125 三层缓存架构 + 主交通图表迁移，见过去佛 §12.9）；第十二批次见 v280 行（E-030）。
- 2026-10-24 第十二交付批次：`EFFECTS-030`（#119~#121 语义 Token/双 stop 渐变/分阶投影，见过去佛 §12.8）；第十一批次见 v279 行（E-029）。
- 2026-10-24 第十一交付批次：`EFFECTS-029`（#116~#118 官方 Animation 协调器 + 按需帧门 + 抽屉退场保留，见过去佛 §12.7）；第十批次见 v278 行（E-027）。
- 2026-10-24 第十交付批次：`EFFECTS-027`（#99 视口点击→球面逆映射拾取管线本批交付，#98 纹理腿分腿既有 + 架构裁决，见过去佛 §12.6）；第九批次见 v277 行（E-024）。
- 2026-10-24 第九交付批次：`EFFECTS-024`（#86~#88 离屏降采样双 pass 高斯模糊管线本批交付，#89 前景合成为集成边界在案，见过去佛 §12.5）；第八批次见 v276 行（E-050）。
- 2026-10-24 第八交付批次：`EFFECTS-050`（#197 Niri 离屏管线本批交付，#198/#199/#200 分腿既有，见过去佛 §12.4）；第七批次见 v275 行（E-048/049）。
- 2026-10-24 迁移损伤修复（依 168b83c pristine 复位；证据 = e680ed8/1e00aa3 两次波次提交信息 + 代码实测）：e680ed8/3887ef6/1e00aa3 三次看板迁移的行拼接乱码逐行修复；`EFFECTS-003`（残形「TS-003」）与 `EFFECTS-008`（被截断并入 007 尾部且曾误回登本板）复位重建；`EFFECTS-013` 与 `EFFECTS-015`（`ButtonHierarchy`/`ButtonPressScale` 已在树）由误标的 Planned 复位 Integrated；Layer 2 的 `EFFECTS-016`~`020` 错位 ID 全部复位；`EFFECTS-006`/`013`/`020` 的 points 依 pristine 复位（3→2）；本板误留的 `EFFECTS-008` 与 `EFFECTS-020`（曾误标 019）重复项删除，`EFFECTS-012`/`014` 归位本板；空壳节（§四/§五/§六/5.1）与 EFFECTS-007 孤儿残句清除；§7.1 过期事实按强制化台账刷新；`TODO.md` 波次 A 日志的羽化条目 ID 复位为 018。修复前实测两板为 44 条/122 pt 与 68 条/180 pt（v268 所记过去佛 69 条/183 pt 与当时实际不符），修复后以本行实测为准。
- Slint 九条来自 2026-10-19 的表面审计实测：缺 7 类页面导致 19 条业务无载体、只派发 42 种 `CommandIntent`、59 条业务里 2 条完整实现 / 18 条部分实现 / 39 条未实现且今天零锚点、4 个测试文件重实现 Slint 逻辑构成第二业务真源、2 个已声明回调无调用点。
- Bevy UI 能力四条源自 2026-10-20 的强制化实测审计；执行状态（机器强制还是人工评审、存量欠账多少）以 Bevy UI 能力规范文档「强制化台账」为单一事实源，本板不另记第二份。
<!-- POINT_LEDGER:END -->

- **执行次序（2026-10-24 第二十一批次后复核）**：本板条目按 **Core-first** 排成一条可推的链——① Slint 表面（`EFFECTS-037`~`041`）→ ③ Slint 表面（`EFFECTS-037`~`041`）→ ④ Slint 表面专项（`SLINTUI-002`~`007`）→ ⑤ Bevy 捕获证据（`BEVYCAP-002`）→ ⑥ 工程约束门禁（`ENG-002.2`）→ ⑦ 像素认证收口（`PARITY-001.5`）。上层条目在前置落地前不得开工；`ENG-003.2`/`003.3` 等统一 cargo 批次解锁。
- **cargo 纪律（减少被 cargo 阻塞）**：同一时刻只有一条流允许跑 cargo；并行期只写代码，编译与像素取证由收口流**统一跑一次**（全仓 nextest → 守卫 → fmt → clippy → 三表面捕获 → 重建回执账本 → 全局验收）。**只有真正因缺少实际环境而无法在本机取回执的验收项**才进[未来佛](CODE_TODO_FUTURE.md)；排队等 cargo lane 不构成阻塞，那些条目留在本板按上面的次序推进。

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

## 七、C 主线：质量红线与安装包

### 7.1 Bevy UI 能力规范强制化（2026-10-20 实测立项：bsn! 已是机器红线，rem 部分机器强制）

立项与现状（2026-10-20 实测立项，2026-10-24 复核刷新）：`bsn!` 场景律法与 rem/em 尺寸通道均已机器强制——产品守卫 `scripts/quality/bevy_rem_guard.py` 是唯一阻塞执行器（命名/派生 px 值、硬编码 px 字面量、非 rem 字号、字面量 `em` 全部拦截，存量台账 `scripts/quality/bevy_rem_inventory.json` 已归零，180 处存量迁移由 `BEVYCAP-001.2` 收口），共享包场景门 `scripts/quality/bevy_scene_gate.py` 为上游 advisory 镜像；⑦ 小件收敛（`bsn_list!`/`Ready`/`despawn_all`/胶囊圆角）已由 `BEVYCAP-002.1` 机器化。执行状态的单一事实源是 Bevy UI 能力规范文档的「强制化台账」一节，本节不另记第二份。剩余未闭环：字号档位的像素回执（唯一证据项）与 `BEVYCAP-002` 的 17 格捕获。

- [ ] BEVYCAP-002 [kind: evidence] [priority: P1] [points: 3] [owner: infiltrator-bevy-ui] [status: Planned] Bevy 九个业务仍缺捕获模块（17 格）
  - 产物：为 `core-version-rollback-confirm`、`editor-snippet-insertion`、`editor-yaml-error-navigation`、`profiles-aggregator-preview`、`profiles-import-modal`、`rules-provider-refresh-error`、`settings-lan-security`、`sync-conflict-resolver`（各 standard+compact）与 `proxies-group-expanded` 补捕获模块（owner 装配 + typed 命令 + 能力闸 + 观测）；`proxies-group-expanded` 的观测要量「折叠→展开」的**可见差异**，而不是量展开节点自身的框。
  - 依赖：表面控件与生产回调都已在树里，只差 capture 侧接线。
  - 验收：三表面捕获矩阵全绿；这 17 格拿到像素回执并被验收认证为 `ready`。
  - 事实依据：2026-10-22 Bevy 捕获流逐格分类的结论（缺 capture 模块，非环境阻塞，故留本板）。

## 十、跨平台 GUI/CLI 表现深化 200 点自底向上攻坚（报告实施；现存 22 条 / 65 pt，已交付 36 条见[过去佛](CODE_TODO_PAST.md)）

### 10.4 Layer 3: Bevy UI 0.20 渲染管线与 WESL 材质深化（0条，已清空，涵盖 #76~#115）

### 10.5 Layer 4: Iced 0.14 原生动画、Canvas 三层缓存与 Shader（0条，已清空，涵盖 #116~#145）

### 10.6 Layer 5: Slint 1.18+ Skia 硬件加速、桌面多列网格与原生状态机（1条 / 2pt，涵盖 #146~#170）

- [ ] EFFECTS-041 [kind: slint] [priority: P1] [points: 2] [owner: infiltrator-slint] [status: Planned] 一体化无边框窗口、Mica/Vibrancy 与软键盘适配（#167~#170）
  - 产物：实现 #167 no-frame 自绘现代标题栏与拖拽区、#168 Windows 11 Mica 亚克力底衬、#169 macOS Vibrancy 毛玻璃底色、#170 移动软键盘避让与安全区。
  - 依赖：EFFECTS-036。
  - 验收：桌面窗口呈现现代操作系统原生材质，移动端输入框自动顶起避让键盘。
  - 计划：更新 desktop.rs 与 app_chrome.slint。

### 10.9 Slint 质感与架构攻坚（6条 / 17pt，涵盖 #1~#34）

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
  - 验收：已做——无：#16~#20（原生 ListView 视口回收、FilterModel/SortModel 零拷贝管道、日志流零拷贝 Model）均未落地。 未做——#16/#17 未改用原生 ListView（ui/ 内 ListView 引用仅 1 处且非虚拟化用法）；#18 仍走 Rust 侧每轮 `slint::VecModel::from(items)` 全量重建；#19/#20 未接入官方 FilterModel/SortModel，也未实现自定义零拷贝流式 Model。
  - 计划：重构 page_proxies.slint 与 lumen_projection/collections，接入 ListView 与 FilterModel。
- [ ] SLINTUI-005 [kind: refactor] [priority: P1] [points: 3] [owner: infiltrator-slint] [status: Planned] 现代 Slint 语法与工程清理 —— 消除占位 Hack 与强类型枚举（#21~#25）
  - 产物：解决 #21 全面消除透明 Rectangle 占位 hack 改用原生 spacing/padding、#22 清理全仓散落的 * LumenUiScale.factor 手动乘法改走系统原生 DPI、#23 在 .slint 声明领域强类型 export enum 替代弱类型字符串、#24 全面改用现代 <=> 双向属性绑定、#25 收敛规范 lumen-app.slint 中堆积的全局属性。
  - 依赖：无。
  - 验收：已做——#22 与 #156 同源，已随 EFFECTS-038 一并收口；#23 与 #157 同源，已落地。 未做——#21 透明占位 hack 未消除；#24 双向绑定未迁移；#25 全局属性收敛只完成一部分（206 → 189）。
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

### 10.10 像素认证收口（1条 / 5pt）

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


## 十一、工程约束门禁补齐（错误类型纪律与合规收尾）

### 11.1 错误类型纪律与可选审计工具（2026-10-22 实测登记；manifest 批次已交付）

- [ ] ENG-002.2 [kind: refactor] [priority: P1] [points: 3] [owner: infiltrator-domain + infiltrator-core] [status: Planned] domain / core 层的 `anyhow` 收敛到 typed 错误
  - 产物：`infiltrator-domain`（31 文件 / 208 处引用）与 `infiltrator-core`（22 文件 / 167 处引用）的错误类型从 `anyhow::Result` 收敛为每层一个 typed 错误枚举（可复用的失败类别 + 上下文），边界处保留一处 `From<anyhow::Error>` 收敛（`lurker-core-contract` 已有该范式）。
  - 依赖：`infiltrator-domain` 与 `infiltrator-core` 的独占写窗口；需按 owner 分层逐个 crate 推进，不跨层并行改同一批文件。
  - 验收：两个 crate 内 `anyhow` 引用归零或只剩边界收敛处；工作区 `cargo nextest` 全绿；契约层错误类型零变化（对外错误面不得因本条漂移）。
  - 事实依据：2026-10-22 实测——`infiltrator-contract`/`infiltrator-ports`/`infiltrator-application`/`infiltrator-presentation` 的 anyhow 引用均为 0，只有 domain 与 core 两层大面积使用，错误类型纪律目前只在两层失守。
  - 计划：先 domain 后 core，每层一个纵切批次；每批补齐失败类别的行为测试再迁调用点。

### 11.2 依赖政策落地后的合规缺口与收尾（2026-10-22 实测登记）

- [ ] ENG-003.2 [kind: evidence] [priority: P1] [points: 1] [owner: workspace] [status: Planned] 工程约束门禁的统一验证收口：全量结构门复跑 + cargo 专责批次
  - 产物：`bash scripts/quality/check-structure.sh` 在合并后的树上 exit 0（上一轮全量复跑撞上本批自改进行中的中间态，故未取得终态回执）；cargo 专责批次补一次 `cargo fmt --all --check`、`cargo clippy --workspace --all-targets -- -D warnings`、`cargo nextest run --workspace --no-fail-fast` 与 `cargo deny check advisories bans licenses sources`。
  - 依赖：结构门复跑可推；cargo 批次由 cargo 专责流执行（共享 lane，按并行纪律错峰）。
  - 验收：结构门 exit 0 的回执 + cargo 四项命令的 run 标识；本批七道守卫与 `ENG-001.1`～`ENG-001.7` 的「待验证」逐条销账。
  - 事实依据：2026-10-22 实测——七道新守卫的 `--self-test` 与 `--mode enforce` 各自全绿、新增 21 项单测全绿、`doc-governance`/`closure-questions`/`ci-trigger`/`license`/`check-test-policy` 全绿、2735 个 Rust 文件的 rustfmt 形状零违例；但全量结构门在最后一次复跑时读到的是本批自改的中间态，缺一条终态回执。
  - 计划：结构门与 cargo 四项各跑一次，证据写进本条与过去佛对应条目。

- [ ] ENG-003.3 [kind: release] [priority: P1] [points: 1] [owner: workspace] [status: Planned] 把当前工作树一体提交并推送 `main`
  - 产物：工作树 320+ 个在制品文件（含本批七道门禁与台账、Slint/Iced/Bevy 表面改动、Android host 改动、版本 pin 与锁文件）一次提交并推送到 `main`；提交前跑一次 `git diff --check` 与结构门。
  - 依赖：无（可推）。提交会一并纳入其它会话遗留的在制品，故必须在终态结构门通过后执行。
  - 验收：远端 `main` 收到该提交；`main-gate`（格式化 + 结构守卫）绿。
  - 事实依据：2026-10-22 实测——工作树含 320 个已改文件与 130 个新增文件（含 24 张截图与 18 个 Rust/Kotlin 新模块），另有两处守卫回归已在本批修掉（parity 覆盖测试的 354→360 格、退役表面守卫的“已迁移符号”误判）。
  - 计划：终态门禁通过后一次性 `git add -A` + 单条提交 + 推送，不再拆批。
