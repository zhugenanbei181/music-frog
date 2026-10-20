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
- Point 基线 `musicfrog-todo-20261020-v245`：现在佛 15 pt（4 条）/ 过去佛 107 pt（47 条）/ 斗战胜佛 72 pt（29 条），联合未办结分母 194 pt；已归档交付 682 pt（263 条）；累计 876 pt，严格归档完成率 77.9%。
- 本板 24 条 = 原未来佛 16 条 + 截图认证与平台覆盖 5 条（`PARITY-001.2`～`001.4`、`PLATFORM-001.1`）+ Slint 第三 UI 补齐 9 条（`SLINT-001.1`～`001.9`）+ 共享控件真源补齐 1 条（`WIDGET-001.1`，本批新增 3 pt）；Bevy UI 能力规范强制化 5 条（`BEVYCAP-001.1`／`001.2`／`002.1`／`003.1` 与 `TOKEN-001.1`）本批全部交付并迁入过去佛，`PUB-006.1`、`PARITY-001.1` 与 `PLATFORM-001.2` 同样已交付并迁入过去佛；Android 静态质量门 4 条（`BANDROID-044.1`／`044.2`／`044.4`／`044.5`，9 pt）与状态表达/文档红线 2 条（`STATE-001.1` 隐式状态机全量审计、`PUB-006.6` 公共 API 文档质量门，5 pt）与开源生态四条（`PUB-006.7`／`006.9`／`006.10`／`006.11`，7 pt）、平台证据与安装包两条（`PLATFORM-001.1`、`APPID-001.3`，5 pt）、地球增强两条（`BANDROID-016.2`、`016.3`，4 pt）、双引擎真实二进制 smoke 与 Android 取证两条（`PUB-006.8`、`PARITY-001.4`，5 pt）与 Slint 第三表面补齐七条（`SLINT-001.2`～`001.7`、`001.9`，15 pt）本批交付并迁入过去佛。
- Slint 九条来自 2026-10-19 的表面审计实测：缺 7 类页面导致 19 条业务无载体、只派发 42 种 `CommandIntent`、59 条业务里 2 条完整实现 / 18 条部分实现 / 39 条未实现且今天零锚点、4 个测试文件重实现 Slint 逻辑构成第二业务真源、2 个已声明回调无调用点。
- Bevy UI 四条来自 2026-10-20 的强制化实测审计：场景结构（`bsn!` 唯一入口 + ECS 边界）已由 `bevy_bsn_guard.py` 机器强制（349 文件零违例），rem/em 尺寸通道同批由共享包执行器 `bevy_scene_gate.py`（硬编码 px 字面量与 `FontSize::Px` 零容忍，存量 128 处棘轮登记）与产品守卫 `bevy_rem_guard.py`（命名/派生 px 值白名单边界，存量 52 处独立台账）两段互补强制，两份台账站点不重叠；`bsn_list!` 0 处 / 内联 `Children [` 974 处、`bevy_scene::Ready` 0 处、`despawn_all` 2 处（⑦ 小件收敛仍仅人工评审）；执行状态台账见 Bevy UI 能力规范文档的「强制化台账」一节。
<!-- POINT_LEDGER:END -->

## 一、大计划主线与推进顺序

| 主线 | 覆盖里程碑 | 条目 | 出口判据 |
| --- | --- | --- | --- |
| A 发布流水线主线 | 0.60 / 0.69 / 1.0.0 | `PUB-006.1`、`PUB-006.6`～`PUB-006.11` | 七个中性 crate 可按顺序 dry-run 发布，docs.rs 构建通过，版本拓扑无环 |
| B 表现与截图认证主线 | 0.44～0.49 / 0.92 | `PARITY-001.2`～`001.4`、`PLATFORM-001.1`～`001.2`、`SLINT-001.1`～`001.9`、`GAP-4.1` | 59 条业务 × 桌面三 UI 锚点齐全、standard/compact 双视口像素回执全绿；Android 三 flavor 交互截图全绿 |
| C 质量红线与安装包主线 | 0.70～0.73 | `BANDROID-044.1`、`044.2`、`044.4`、`044.5`、`APPID-001.3`、`BEVYCAP-001.1`、`001.2`、`002.1`、`003.1` | Lint/ktlint/Detekt/架构守卫强制生效且零违规；Linux deb/AppImage 安装冒烟通过；Bevy UI 的 rem 尺寸通道与场景律法成为唯一机器红线 |
| D 视觉与模型统一主线 | 0.48 / 0.90 / 0.91 | `GAP-7.1`、`BANDROID-016.2`、`BANDROID-016.3` | 三大 UI 共用一份页面模型；地球增强与帧/内存报告交付 |

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

- [ ] PARITY-001.2 [kind: evidence] [priority: P0] [points: 5] [owner: scripts/parity + 三大 UI] [status: Planned] 三大桌面 UI 全业务双视口像素认证闭环
  - 产物：59 业务 × 3 表面 × standard/compact 双视口的真实像素回执全量归档，覆盖率 100%。
  - 依赖：PARITY-001.1。
  - 验收：`visual_receipts.py` 输出的 118 个待回执单元格全部转绿，无「以静态页面截图冒充平权通过」。
  - 计划：① 用 `scripts/capture-{iced,bevy,slint}-matrix.sh` 跑标准/紧凑两视口；② 逐业务比对几何与交互断言；③ 失败业务回流对应 UI 修缺陷再重取。
  - 证据：`.evidence/visual_receipts.json` 全绿 + 每业务截图清单。
  - 已验证能力（2026-10-20）：本机 `kwin_wayland --virtual` + 嵌套 `niri` 私有合成器可正常起栈，应用能写出 `CAPTURE_READY` 标记并落 PNG/manifest，截图取证在本机可复跑；缺的是逐业务的 demo 交互实现与「无载体页面不得锚定」之外的覆盖补齐。
  - 第一批可取证清单：Iced demo 已实现交互的 21 条业务（`RuntimeTelemetryObservation`、`LogsRedactedExport`、`LogsScrollLock`、`LogsSearchHighlight`、`ShellRoutingIntentControl`、`ShellRoutingIntentAuthentication`、`ProfilesSnapshotRestoreConfirm`、`ProfilesFilterEditor`、`RulesStatisticsInspector`、`RulesListEditor`、`RulesOverrideEditor`、`RulesTracerDrawer`、`DnsQueryDetails`、`DnsFakeIpFlushConfirm`、`DnsHostsEditor`、`DnsLeakAlert`、`DoctorFailureRecovery`、`SettingsLanguageChoice`、`ConnectionsCloseAllConfirm`、`ConnectionsSearchHighlight`、`ConnectionsGrouping`）× standard/compact 双视口 = 42 个像素回执单元格；其余 37 条业务要么缺 demo 交互、要么 Slint 无载体，按缺口逐条立项。
- [ ] PUB-006.13 [kind: mechanism] [priority: P1] [points: 2] [owner: singbox-driver] [status: Planned] sing-box 驱动的节点测速 typed port 未实现
  - 产物：`singbox-driver` 的 typed `OutboundPort::probe` 真正实现节点测速并回报 `OutboundProbeState` 的实测值，而不是 `Unsupported`。
  - 依赖：PUB-002.3、PUB-006.8（真实二进制取证暴露的缺口）。
  - 验收：真实 sing-box 二进制下测速返回实测延迟（typed 状态而非字符串），失败/不支持/超时各有自己的 typed 结局，驱动与真实引擎的语义一致。
  - 计划：① 对齐 mihomo 驱动的 typed 测速实现与引擎的 delay 接口语义 ② 实现 sing-box 的 typed 测速并补单测 ③ 用 `scripts/core-backend-smoke-singbox.sh` 复跑，回执从 `unsupported_proof` 转为 `complete`。
  - 证据：驱动单测 run ID + 真实二进制回执里 sing-box 的测速证明转为已证明。

- [ ] GAP-4.1 [kind: evidence] [priority: P1] [points: 3] [owner: slint] [status: Planned] Slint 功能验收与 parity
  - 产物：Slint 六个页面回调对应的功能验收与 parity cell 证据（含 `capture-slint-matrix.sh` 24 场景结果）。
  - 依赖：目标产物与交互证据。
  - 验收：parity 锚点 + 屏幕回执；否则在八问表保持「部分」。
  - 计划：① 跑 Slint 场景矩阵并绑定像素回执；② 补六页回调功能验收；③ 与 PARITY-001.1 的 slint 注册表合并取证。
  - 证据：截图清单 + parity 报告。

### 5.1 Slint 第三 UI 的 360° 补齐（7条 / 16pt）




## 七、C 主线：质量红线与安装包


### 7.1 Bevy UI 能力规范强制化（2026-10-20 实测立项：bsn! 已是机器红线，rem 部分机器强制）

立项依据（2026-10-20 实测审计，证据等级 L）：场景结构口径已闭合——`scripts/quality/bevy_bsn_guard.py` 扫描两个 Bevy crate 生产树 349 文件零违例并接入 `scripts/quality/check-structure.sh` 与 CI；rem/em 尺寸通道同批由共享包执行器 `scripts/quality/bevy_scene_gate.py` 接上机器红线（硬编码 px 字面量与 `FontSize::Px` 零容忍，存量 128 处棘轮登记），但 token/计算值通道的白名单边界与能力规范 ⑦ 小件收敛**仍仅人工评审**，生产树另有 263 处 `Val::Px` 写入（其中初筛 183 处非白名单、字号通道已全量 `FontSize::Rem`）、`bsn_list!` 0 处 / 内联 `Children [` 974 处、`bevy_scene::Ready` 0 处、`despawn_all` 2 处。执行状态台账见 Bevy UI 能力规范文档的「强制化台账」一节。

- [ ] GAP-7.1 [kind: mechanism] [priority: P1] [points: 5] [owner: presentation] [status: Planned] 跨 UI 页面模型去重
  - 产物：Iced/Bevy/Slint 共享的页面模型层，消灭各自实现的同义状态机。
  - 依赖：GAP-5.2 完成后评估。
  - 验收：同一页面在三个表面只保留一份状态推导；UI 只做渲染与事件投递。
  - 计划：① 盘点三端重复状态机清单；② 逐页下沉到 presentation 共享层；③ 每迁完一页即删旧实现并跑三端回归。
  - 盘点（2026-10-20 实测，证据等级 L）：owner 裁决为 `crates/infiltrator-presentation`（上位架构契约已定「跨表面的纯文案、状态展示折叠与快照到视图模型映射完全由 `infiltrator-presentation` 持有」；`lurker-ui-presentation` 不得持有依赖产品契约的页面模型）。根因是**页面模型写在了 Bevy 私有层**（`infiltrator-bevy-ui/src/surface_projection.rs` 773 行，11 个页面的投影全在这里），Iced 与 Slint 只好各自重推。
  - 迁移序（高重复、低影响在前）：① 日志页（共享 owner `log_search::LogSearchState` 与 `log_projection::project_log_facts` 已完备且 Bevy 已在用，Slint 的 `project_logs`/`log_level_key`/`log_stream_status` 是私有第二实现且**没有正则与非法模式态**，属真实行为分歧）→ ② 连接页（分组/搜索已共享，仅排序键三写：Iced `String`、Bevy `ConnectionSortKey`、Slint 字符串匹配）→ ③ 代理页（三份私有过滤/排序：Bevy `matches_proxy_filter` 与 Iced `member_matches` 规则已经不一致，Bevy 匹配 `features` 标签而 Iced 匹配 `udp`；先把 Bevy 页面模型搬进 presentation 再改指 Iced/Slint）→ ④ 概览页（`OverviewProjection` 为 Bevy 私有结构，Slint 私有重推 `route_mode`/`network_mode`/`network_mode_status`/`network_mode_warning`）→ ⑤ 设置页（Iced 私存 `PacManagerConfig`/`LanSharingConfig`/`LanSecurityConfig`/`UwpLoopbackState` 四份契约快照副本，外加 `lan_sharing_dirty`/`lan_security_dirty` 两个隐式状态 bool）→ ⑥ 规则页（tab/行投影已共享，仅 8 个保存/加载阶段机两写）→ ⑦ 订阅/体检/DNS/路由/同步页（动作折已共享，逐页跟进前六页的迁移机制）。
  - 必须清零的第二真源（清零搜索回执为验收件）：Slint 的 `project_logs` 私有正则/等级过滤、`project_connections` 私有过滤与排序、`project_proxies` 私有过滤与排序、`lumen_projection.rs` 的 `route_mode`/`network_mode`/`network_mode_status`/`network_mode_warning`；Iced 的 `LogFilterState`（与共享 `LogSearchState` 并存）、`types/app.rs` 的四个契约快照私有副本与两个 `*_dirty` bool、`types/doctor.rs` 的 `DoctorPanelOperation`（与共享 `DoctorActionState` 并行跑两台阶段机）；Bevy 的 `surface_projection.rs` 与 `pages/proxies_filter.rs` 迁入共享层。
  - 不迁：About/Licenses（仅 Slint 有）、Slint 独立 `Network` 路由（与设置页同事实，应折进共享设置页模型而不是再加第三份）。
  - 证据：每页一次迁移的 nextest run ID + 旧符号清零搜索回执。

## 九、办结与登记

- 本板条目全部验收通过后，按 [TODO.md](../TODO.md) 流转规则同次移入[过去佛](CODE_TODO_PAST.md)（实现已交付、只剩统一批次）或直接办结退出并登记 archived points。
- 需要实机、tag CI、用户 secrets、上架凭证或异构目标机才能收口的条目，在迁移前先完成本板所有可推动作，再整条迁入[斗战胜佛](CODE_TODO_BLOCKED.md)。

### 7.2 共享控件真源补齐（rem 迁移暴露的重复事实）

