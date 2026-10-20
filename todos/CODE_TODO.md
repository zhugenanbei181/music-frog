# MusicFrog 工程 TODO · 现在佛

> 层级：L4

## 0. 编写规则

- 唯一用途：登记全部**可推**未办结工作（待编码、待实现、待补证），覆盖 0.60 → 1.0 收口路径：crates.io 发布流水线、0.6x 生态与文档、0.7x 质量红线与安装包、0.9x 视觉与性能，以及五大平台 × 三大 UI 表现架构的 360° 覆盖与逐业务截图认证。已实现待验证的条目在[过去佛](CODE_TODO_PAST.md)；只剩外部环境可解锁的条目在[斗战胜佛](CODE_TODO_BLOCKED.md)。
- 每条仅保留唯一 owner、产物、依赖、验收、计划与证据要求；header ≤240 字符、详情 ≤6 行、整块 ≤1200 字符。证据留 nextest run ID / guard 回执；事实变化改写原条，不追加过程叙事。
- 状态词汇：`Planned / Blocked / Ready / InProgress`（本板）与 `Integrated`（过去佛，已交付待验证）。`Verified` 只是办结过渡态，不驻留任何看板。
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
- Point 基线 `musicfrog-todo-20261020-v238`：现在佛 66 pt（28 条）/ 过去佛 29 pt（13 条）/ 斗战胜佛 72 pt（29 条），联合未办结分母 167 pt；已归档交付 682 pt（263 条）；累计 849 pt，严格归档完成率 80.3%。
- 本板 28 条 = 原未来佛 16 条 + 截图认证与平台覆盖 5 条（`PARITY-001.2`～`001.4`、`PLATFORM-001.1`）+ Slint 第三 UI 补齐 9 条（`SLINT-001.1`～`001.9`）；`PUB-006.1`、`PARITY-001.1` 与 `PLATFORM-001.2` 已交付并迁入过去佛。
- Slint 九条来自 2026-10-19 的表面审计实测：缺 7 类页面导致 19 条业务无载体、只派发 42 种 `CommandIntent`、59 条业务里 2 条完整实现 / 18 条部分实现 / 39 条未实现且今天零锚点、4 个测试文件重实现 Slint 逻辑构成第二业务真源、2 个已声明回调无调用点。
<!-- POINT_LEDGER:END -->

## 一、大计划主线与推进顺序

| 主线 | 覆盖里程碑 | 条目 | 出口判据 |
| --- | --- | --- | --- |
| A 发布流水线主线 | 0.60 / 0.69 / 1.0.0 | `PUB-006.1`、`PUB-006.6`～`PUB-006.11` | 七个中性 crate 可按顺序 dry-run 发布，docs.rs 构建通过，版本拓扑无环 |
| B 表现与截图认证主线 | 0.44～0.49 / 0.92 | `PARITY-001.2`～`001.4`、`PLATFORM-001.1`～`001.2`、`SLINT-001.1`～`001.9`、`GAP-4.1` | 59 条业务 × 桌面三 UI 锚点齐全、standard/compact 双视口像素回执全绿；Android 三 flavor 交互截图全绿 |
| C 质量红线与安装包主线 | 0.70～0.73 | `BANDROID-044.1`、`044.2`、`044.4`、`044.5`、`APPID-001.3` | Lint/ktlint/Detekt/架构守卫强制生效且零违规；Linux deb/AppImage 安装冒烟通过 |
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
| **合计** | **59** | 三表面全部登记：Iced contract 59/59、scenario 58/59；Bevy 59/59 + 59/59；Slint contract 57/59、scenario 0/59 | 177 个待像素回执单元格 | 0 个 flavor 截图回执 |

缺口来源与实测结论（2026-10-19 全量审计）：`scripts/parity/feature_evidence.tsv` 与 `scripts/parity/cross_surface_manifest.tsv` 原先只登记 iced/bevy 两表面（各 118 单元格），`scripts/parity/visual_receipts.py` 只接受 `--iced-run`/`--bevy-run`。现已补齐第三表面：**177 单元格（59 业务 × 3 表面）全部登记并可解析**，Slint 侧 57 格为 `pending` + 逐条原因，2 格为 `implemented`（业务已实现、缺表面测试）。Slint 表面审计得到五条硬事实：① 页面词汇只有 Home/Proxy/Config/Settings/Network/Connections/Logs/Rules/Traffic/AppSettings/About，**dns、editor、filter、mixin、sync、app_routing、doctor 七类页面不存在**，按契约页面归属共 **19 条业务在第三 UI 无载体**（产品缺口）；② Slint 只派发 42 种 `CommandIntent`，DNS、编辑器、脚本沙箱、测速、快照、聚合器、命令面板等 30 余种意图从未派发；③ 59 条业务里 **2 条完整实现、18 条部分实现、39 条未实现**，**今天没有任何一条可以合法标记为 Slint scenario 锚点**；④ 四个测试文件在用例里**重实现 Slint 逻辑**（路由模式映射、网络模式序列、主题白名单、十六进制解析、连接排序），构成仓库禁止的「第二业务真源」，且永远无法当表面锚点；⑤ 两个已声明回调 `refresh-profile(string)`、`refresh-home-egress()` 在任何 `.slint` 文件里都没有调用点。收口由 `PARITY-001.2`～`001.4`、`PLATFORM-001.1`～`001.2` 与 `SLINT-001.1`～`SLINT-001.9` 共同承担。

## 四、A 主线：crates.io 发布流水线与开发者生态

- [ ] PUB-006.6 [kind: infra] [priority: P1] [points: 2] [owner: docs] [status: Planned] Docs.rs 预览与公共 API 文档质量门
  - 产物：docs.rs 渲染模拟配置与 API 文档门禁；七个 crate 开启 `#![warn(missing_docs)]` 并提供标准 Rustdoc 与 doctest。
  - 依赖：PUB-006.1、PUB-006.2。
  - 验收：`cargo doc --all-features --no-deps` 零警告，intra-doc links 全部校验；公开 API 文档覆盖率 100% 且含可执行 doctest。
  - 计划：① 逐 crate 跑 doc 收集零告警缺口；② 补 missing_docs 与 doctest；③ 接入 `scripts/test.sh` 结构门。
  - 证据：`cargo doc` 零告警回执 + 门禁 run ID。
- [ ] PUB-006.7 [kind: mechanism] [priority: P1] [points: 2] [owner: test-infra] [status: Planned] 无头沙盒集成测试与混沌容错验证
  - 产物：针对中性运行时的无头 CI 集成沙盒与混沌套件；覆盖网络波动、内核猝死拉起、锁竞争与损坏配置热恢复。
  - 依赖：PUB-003.4、PUB-006.1。
  - 验收：异常注入下状态机 100% fail-closed、租约安全回收、无内存泄漏且能自愈。
  - 计划：① 定义故障注入矩阵（进程猝死/锁竞争/配置损坏/网络抖动）；② 实现注入式 harness；③ 接入统一批次并固化 fail-closed 断言。
  - 证据：混沌套件 nextest run ID + 内存曲线回执。
- [ ] PUB-006.8 [kind: evidence] [priority: P1] [points: 2] [owner: core/drivers] [status: Planned] 双引擎驱动真实二进制本地 Smoke 取证
  - 产物：激活 `mihomo-driver`/`singbox-driver` 的真实二进制 smoke 套件并固化本地可复跑入口。
  - 依赖：PUB-002.1、PUB-002.3、PUB-006.1。
  - 验收：真实进程下完成 API 握手、配置写入、节点测速与流量分发冒烟验证。
  - 计划：① 跑 `scripts/core-backend-smoke-mihomo.sh` 与 `scripts/core-backend-smoke-singbox.sh` 取得真实二进制回执；② 把被 `#[ignore]` 的真进程测试改为显式环境契约入口；③ 归档 Linux x86_64 证据。
  - 证据：真实二进制执行日志 + 摘要回执；Windows 腿在斗战胜佛。
- [ ] PUB-006.9 [kind: mechanism] [priority: P1] [points: 2] [owner: examples] [status: Planned] 最小化用例展厅 (Examples Gallery)
  - 产物：`examples/` 下 4 套独立极简用例（minimal-cli / minimal-slint / minimal-iced / minimal-bevy），各 ≤200 行、无多余依赖。
  - 依赖：PUB-005.1～PUB-005.3、PUB-004.2。
  - 验收：四套 example 独立 `cargo run --example` 成功并接入 CI 编译守卫。
  - 计划：① 逐套按 ≤200 行预算实现；② 独立工程运行验证；③ 接入 CI 编译守卫与行数门禁。
  - 证据：四个 example 的运行回执 + 行数门禁回执。
- [ ] PUB-006.10 [kind: infra] [priority: P1] [points: 2] [owner: CI] [status: Planned] 多架构交叉构建自动化与 SemVer 兼容守卫
  - 产物：Linux/Windows/Android aarch64 与 x86_64 交叉编译流水线 + `cargo-semver-checks` 门禁。
  - 依赖：GAP-1.1、PUB-006.1。
  - 验收：四大目标架构交叉编译 100% 通过；SemVer 门禁能拦截中性 crate 公开 API 的破坏性变更。
  - 计划：① 固化 `scripts/cross-check.sh` 组合进 CI；② 接入 semver-checks 并登记基线；③ 用一次故意破坏验证门禁确实拒绝。
  - 证据：交叉编译 job 回执 + semver 负例回执。
- [ ] PUB-006.11 [kind: evidence] [priority: P1] [points: 1] [owner: docs/qa] [status: Planned] 开发者全生命周期演练与干跑审计
  - 产物：在纯净克隆环境模拟外部开发者全旅程（克隆 → 看文档 → 脚手架生成 → 运行 → 接驱动 → 调配置）并输出审计报告。
  - 依赖：PUB-005.5、PUB-006.4、PUB-006.9。
  - 验收：旅程零阻滞，全链路 ≤5 分钟，体验毛刺清零并归档回执。
  - 计划：① 纯净克隆跑 `cargo-generate` 生成四类工程；② 逐类构建并运行；③ 记录每步耗时与毛刺。
  - 证据：审计报告 + 各步耗时回执。

## 五、B 主线：截图认证与平台证据（每个业务都真跑过）

- [ ] PARITY-001.2 [kind: evidence] [priority: P0] [points: 5] [owner: scripts/parity + 三大 UI] [status: Planned] 三大桌面 UI 全业务双视口像素认证闭环
  - 产物：59 业务 × 3 表面 × standard/compact 双视口的真实像素回执全量归档，覆盖率 100%。
  - 依赖：PARITY-001.1。
  - 验收：`visual_receipts.py` 输出的 118 个待回执单元格全部转绿，无「以静态页面截图冒充平权通过」。
  - 计划：① 用 `scripts/capture-{iced,bevy,slint}-matrix.sh` 跑标准/紧凑两视口；② 逐业务比对几何与交互断言；③ 失败业务回流对应 UI 修缺陷再重取。
  - 证据：`.evidence/visual_receipts.json` 全绿 + 每业务截图清单。
  - 已验证能力（2026-10-20）：本机 `kwin_wayland --virtual` + 嵌套 `niri` 私有合成器可正常起栈，应用能写出 `CAPTURE_READY` 标记并落 PNG/manifest，截图取证在本机可复跑；缺的是逐业务的 demo 交互实现与「无载体页面不得锚定」之外的覆盖补齐。
  - 第一批可取证清单：Iced demo 已实现交互的 21 条业务（`RuntimeTelemetryObservation`、`LogsRedactedExport`、`LogsScrollLock`、`LogsSearchHighlight`、`ShellRoutingIntentControl`、`ShellRoutingIntentAuthentication`、`ProfilesSnapshotRestoreConfirm`、`ProfilesFilterEditor`、`RulesStatisticsInspector`、`RulesListEditor`、`RulesOverrideEditor`、`RulesTracerDrawer`、`DnsQueryDetails`、`DnsFakeIpFlushConfirm`、`DnsHostsEditor`、`DnsLeakAlert`、`DoctorFailureRecovery`、`SettingsLanguageChoice`、`ConnectionsCloseAllConfirm`、`ConnectionsSearchHighlight`、`ConnectionsGrouping`）× standard/compact 双视口 = 42 个像素回执单元格；其余 37 条业务要么缺 demo 交互、要么 Slint 无载体，按缺口逐条立项。
- [ ] PARITY-001.3 [kind: mechanism] [priority: P0] [points: 2] [owner: infiltrator-iced + infiltrator-bevy-ui] [status: InProgress] 四个 pending 行为锚点补齐
  - 产物：`core-version-rollback-confirm`（iced/bevy/slint contract）、`proxies-search-highlight`（iced scenario）、`speedtest-progress-cancel`（iced scenario）四个单元格补真实行为测试并转 anchored。
  - 依赖：共享 core rollback / proxy 搜索 / speedtest 取消 owner 契约。
  - 验收：四格状态由 pending 转 anchored，且测试覆盖失败恢复、拒绝与取消终态。
  - 计划：① 逐格补行为测试（不复用 demo 快照冒充）；② 更新注册表 `test_id`/`reason`；③ 跑 resolver 确认零 pending。
  - 进度：4 格中已闭 3 格——`core-version-rollback-confirm` 的 iced/bevy/slint contract 由 `infiltrator-application::version_application::tests` 的三条回滚用例（转发引擎身份并回报激活版本 / 失败不伪造 / 外来引擎 fail-closed）转 anchored；`proxies-search-highlight` 的 iced scenario 由 `infiltrator-iced::headless::proxy_search_behavior_tests`（typed 偏好提交、命中高亮、诚实空态、清空恢复、owner 拒绝时保留已确认偏好与可重试草稿、刷新后重放查询）转 anchored。剩余 `speedtest-progress-cancel` 的 iced scenario 一格仍 pending。
  - 证据：四格 nextest run ID + resolver 报告。
- [ ] PARITY-001.4 [kind: evidence] [priority: P0] [points: 3] [owner: android 三 flavor] [status: Planned] Android 三 flavor 实际交互截图与行为认证
  - 产物：Compose / Bevy / Slint 三个 flavor 在模拟器上的真实交互截图与行为回执（启动、导航、列表滚动、权限与 VPN consent 终态）。
  - 依赖：模拟器 M1 腿（BANDROID-002.1/002.2 在过去佛）、`scripts/android-emulator-tests.sh`、APK identity 守卫。
  - 验收：三 flavor 各绑定同一候选源码 APK 的 SHA-256 与截图清单；安全区、键盘与滚动命中区取证完整。
  - 计划：① 构建三 flavor 候选 APK 并过 identity/aapt；② 用 Gradle Managed Device 跑交互脚本取截图；③ 把回执绑定到 parity 业务域。
  - 证据：APK SHA-256 + 截图清单 + logcat 摘要；真机腿在斗战胜佛 §一。
- [ ] PLATFORM-001.1 [kind: evidence] [priority: P0] [points: 3] [owner: release + scripts] [status: Planned] 五大平台构建-安装-运行证据矩阵落地
  - 产物：按平台矩阵口径（由 L1 入口导航）逐平台登记构建腿、安装包、运行与设备证据，缺项显式 `Unsupported` 并指向斗战胜佛。
  - 依赖：GAP-1.1 交叉检查脚本、APPID-001.3、PARITY-001.4。
  - 验收：Linux/Windows/Android 有本机或 CI 回执；macOS/iOS 显式 `Unsupported` 且阻塞方登记；矩阵无「以旧 commit 回执冒充当前源码」。
  - 计划：① 固化矩阵文件与生成脚本；② 本机跑 Linux 腿与交叉检查；③ 逐平台绑定证据身份。
  - 证据：矩阵文件 + 逐平台 run ID。
- [ ] GAP-4.1 [kind: evidence] [priority: P1] [points: 3] [owner: slint] [status: Planned] Slint 功能验收与 parity
  - 产物：Slint 六个页面回调对应的功能验收与 parity cell 证据（含 `capture-slint-matrix.sh` 24 场景结果）。
  - 依赖：目标产物与交互证据。
  - 验收：parity 锚点 + 屏幕回执；否则在八问表保持「部分」。
  - 计划：① 跑 Slint 场景矩阵并绑定像素回执；② 补六页回调功能验收；③ 与 PARITY-001.1 的 slint 注册表合并取证。
  - 证据：截图清单 + parity 报告。

### 5.1 Slint 第三 UI 的 360° 补齐（7条 / 16pt）

- [ ] SLINT-001.1 [kind: mechanism] [priority: P0] [points: 2] [owner: infiltrator-slint] [status: Ready] Slint 表面行为测试台与 scenario 锚点
  - 产物：Slint headless 行为台（快照投影 + `SlintAction` 回调 + 真实 `CommandIntent` 派发），把 4 个仅断言意图形状的测试文件升级为表面行为测试，并为已实现页面产出 parity scenario 锚点。
  - 依赖：现有 `SlintProductHost`/`SlintAction` 接线；Iced/Bevy headless 模式结构。
  - 验收：每个 Slint 行为测试必须驱动 Slint 表面并断言派发意图与投影结果；纯意图形状断言不得计入 scenario 锚点。
  - 计划：① 抽 headless 装配（不建窗口）② 升级 4 个形状级测试文件 ③ 按已实现页面补锚点并接注册表。
  - 证据：Slint nextest run ID + 注册表锚点增量。
- [ ] SLINT-001.2 [kind: mechanism] [priority: P1] [points: 3] [owner: infiltrator-slint] [status: Planned] Slint DNS 页面（4 业务）
  - 产物：Slint DNS 页承载泄漏告警、查询详情、Fake-IP 刷新确认与 Hosts 编辑四条业务，并接入共享 owner 快照。
  - 依赖：SLINT-001.1 行为台；共享 DNS presentation 投影。
  - 验收：四条业务在 Slint 有表面行为测试与双视口像素回执。
  - 计划：① 复用共享投影实现页面 ② 接行为测试 ③ 接 parity 注册表与截图。
  - 证据：截图 + 锚点 run ID。
- [ ] SLINT-001.3 [kind: mechanism] [priority: P1] [points: 3] [owner: infiltrator-slint] [status: Planned] Slint 编辑器族页面（4 业务：脚本沙箱/导出评审/差异视图/快照恢复）
  - 产物：Slint 侧脚本沙箱、导出评审、配置差异与快照恢复四条业务的载体。
  - 依赖：SLINT-001.1 行为台；共享脚本与差异投影。
  - 验收：四条业务在 Slint 有表面行为测试与双视口像素回执。
  - 计划：① 评估可复用组件 ② 实现页面与回调 ③ 接行为测试与截图。
  - 证据：截图 + 锚点 run ID。
- [ ] SLINT-001.4 [kind: mechanism] [priority: P1] [points: 2] [owner: infiltrator-slint] [status: Planned] Slint 配置过滤与聚合器（3 业务）
  - 产物：Slint 侧配置过滤编辑器与聚合器向导/预览三条业务载体。
  - 依赖：SLINT-001.1 行为台；共享聚合投影。
  - 验收：三条业务在 Slint 有表面行为测试与双视口像素回执。
  - 计划：① 实现过滤与聚合器 UI ② 接共享意图 ③ 接行为测试与截图。
  - 证据：截图 + 锚点 run ID。
- [ ] SLINT-001.5 [kind: mechanism] [priority: P1] [points: 2] [owner: infiltrator-slint] [status: Planned] Slint 诊断与生命周期恢复页（2 业务）
  - 产物：Slint 侧 doctor 诊断与失败恢复、生命周期恢复面板两条业务载体。
  - 依赖：SLINT-001.1 行为台；共享 doctor 投影。
  - 验收：两条业务在 Slint 有表面行为测试与双视口像素回执。
  - 计划：① 接入共享 doctor 观察 ② 实现页面与修复/重试动作 ③ 接行为测试与截图。
  - 证据：截图 + 锚点 run ID。
- [ ] SLINT-001.6 [kind: mechanism] [priority: P1] [points: 2] [owner: infiltrator-slint] [status: Planned] Slint 订阅同步冲突页（2 业务）
  - 产物：Slint 侧同步冲突解决与恢复确认两条业务载体。
  - 依赖：SLINT-001.1 行为台；共享同步投影。
  - 验收：两条业务在 Slint 有表面行为测试与双视口像素回执。
  - 计划：① 接入同步状态投影 ② 实现冲突处置动作 ③ 接行为测试与截图。
  - 证据：截图 + 锚点 run ID。
- [ ] SLINT-001.7 [kind: mechanism] [priority: P1] [points: 2] [owner: infiltrator-slint] [status: Planned] Slint 应用路由策略编辑页（1 业务）
  - 产物：Slint 侧应用路由策略编辑器（含取消/失败/刷新/不支持路径）载体。
  - 依赖：SLINT-001.1 行为台；共享路由策略投影。
  - 验收：该业务在 Slint 有表面行为测试与双视口像素回执。
  - 计划：① 接入共享策略投影 ② 实现编辑与提交动作 ③ 接行为测试与截图。
  - 证据：截图 + 锚点 run ID。
- [ ] SLINT-001.8 [kind: mechanism] [priority: P0] [points: 2] [owner: infiltrator-slint] [status: Planned] 形状级测试文件改为驱动真实表面（消除第二业务真源）
  - 产物：`connections_and_logs_tests`、`core_and_network_mode_tests`、`outbound_and_probe_tests`、`rules_and_settings_tests` 四个文件不再在测试里重实现路由模式、网络模式序列、主题白名单、十六进制解析与连接排序，改为调用 Slint 表面。
  - 依赖：把 `network_mode_action`、`selection_action`、`probe_action`、`theme_util::parse_color` 等私有实现提升为可测边界。
  - 验收：四个文件的用例全部经 Slint 表面断言派发意图与投影结果；重实现代码清零（仓库搜索回执）。
  - 计划：① 提升可测边界 ② 逐文件替换断言 ③ 清零旧重实现并接 registry 锚点。
  - 证据：nextest run ID + 旧符号清零搜索回执。
- [ ] SLINT-001.9 [kind: mechanism] [priority: P1] [points: 1] [owner: infiltrator-slint] [status: Planned] 两个未接线的 Slint 回调接线
  - 产物：`refresh-profile(string)`（订阅刷新）与 `refresh-home-egress()`（公网 IP 复探）在 Slint 界面有真实调用点，或从声明中退役。
  - 依赖：SLINT-001.1 行为台。
  - 验收：不存在"声明了但无调用点"的回调；订阅错误与复探失败有 typed 呈现。
  - 计划：① 逐个判定接线或退役 ② 退役则同步删除声明 ③ 接线则补行为测试。
  - 证据：调用点清单 + nextest run ID。

## 六、C 主线：质量红线与安装包

- [ ] BANDROID-044.1 [kind: infra] [priority: P1] [points: 2] [owner: android gradle] [status: Planned] Android Lint 接入
  - 产物：Compose/Bevy Debug 变体 lint task 注册并接入 PR/push workflow；不把 assemble 当 lint 结果。
  - 依赖：BANDROID-042/043 存量违规清理；AGP 9.3.1/JDK 17 的 Lint crash 待官方兼容矩阵覆盖。
  - 验收：lint 结果分项报告，违规清零后强制生效。
  - 计划：① 登记当前 lint 违规基线；② 逐类清理；③ 注册 task 与 workflow 步骤并留分项报告。
  - 证据：lint 报告 + workflow run ID。
- [ ] BANDROID-044.2 [kind: infra] [priority: P1] [points: 2] [owner: android gradle + scripts/quality] [status: Planned] ktlint/Detekt 门
  - 产物：ktlint 与 Detekt 接入并强制；无永久 baseline、文件级 suppression 或抬高阈值。
  - 依赖：BANDROID-044.1。
  - 验收：违规清零后强制生效；正反例可复现。
  - 计划：① 接入工具并盘点违规；② 清理至零；③ 去掉任何豁免开关后复跑。
  - 证据：扫描报告 + 负例回执。
- [ ] BANDROID-044.4 [kind: infra] [priority: P1] [points: 3] [owner: scripts/quality] [status: Planned] 架构红线守卫
  - 产物：禁止生产 readiness sleep、主线程阻塞、无界 GlobalScope、空 catch、吞取消、`!!` 与多状态 Boolean wire。
  - 依赖：BANDROID-044.2。
  - 验收：窄例外有局部说明与行为回归；正反例通过。
  - 计划：① 复用既有 typed-state/line 守卫规则集；② 扩展到 Kotlin 与桌面 crate；③ 接入结构门。
  - 证据：守卫 run ID + 负例回执。
- [ ] BANDROID-044.5 [kind: infra] [priority: P1] [points: 2] [owner: CI + 本机检查入口] [status: Planned] 质量门接线并自测
  - 产物：Android workflow 与本机入口分别运行两 flavor lint、ktlint、Detekt、文件预算与架构守卫。
  - 依赖：BANDROID-044.1～044.4。
  - 验收：正反例验证任务注册、排除、放行与拒绝；真实执行后才称质量门接通。
  - 计划：① 汇总本机入口脚本；② 接 workflow；③ 用故意违规验证 CI 拒绝。
  - 证据：workflow run ID + 负例回执；CI 实际 runner 回执在斗战胜佛 §二。
- [ ] APPID-001.3 [kind: evidence] [priority: P0] [points: 2] [owner: packaging/linux] [status: Planned] Linux 最终 Deb/AppImage 产物
  - 产物：Linux 最终 Deb/AppImage 构建与验收。
  - 依赖：APPID-001.2。
  - 验收：包名/图标/二进制来自登记表；安装冒烟通过。
  - 计划：① 跑打包脚本出双产物；② 校验 identity 与登记表一致；③ 安装-启动-卸载冒烟。
  - 证据：产物 SHA-256 + 冒烟日志。

## 七、D 主线：视觉与页面模型统一

- [ ] GAP-7.1 [kind: mechanism] [priority: P1] [points: 5] [owner: presentation] [status: Planned] 跨 UI 页面模型去重
  - 产物：Iced/Bevy/Slint 共享的页面模型层，消灭各自实现的同义状态机。
  - 依赖：GAP-5.2 完成后评估。
  - 验收：同一页面在三个表面只保留一份状态推导；UI 只做渲染与事件投递。
  - 计划：① 盘点三端重复状态机清单；② 逐页下沉到 presentation 共享层；③ 每迁完一页即删旧实现并跑三端回归。
  - 证据：每页一次迁移的 nextest run ID + 旧符号清零搜索回执。
- [ ] BANDROID-016.2 [kind: mechanism] [priority: P1] [points: 3] [owner: bevy-widgets::chart] [status: Planned] 按需 3D/2.5D 地球与流量弧线
  - 产物：拖拽、节点地理标记、流量弧线；保留二维与 Eco 回退、低电量/热状态/减少动效降级。
  - 依赖：BANDROID-016.1；013 图形基础。
  - 验收：操作沿用真实节点身份；离页释放资源、静置休眠。
  - 计划：① 实现弧线与地理标记；② 接入降级策略；③ 录屏与性能抽样。
  - 证据：截图/录屏回执 + 资源释放断言。
- [ ] BANDROID-016.3 [kind: evidence] [priority: P1] [points: 1] [owner: bevy-widgets::chart] [status: Planned] 地球桌面帧/内存报告
  - 产物：显示/关闭模式的帧/内存报告；增强关闭后资源释放核查。
  - 依赖：BANDROID-016.2。
  - 验收：报告绑定同一源码身份；等价操作面通过。
  - 计划：① 固定测量口径；② 采两组数据；③ 与基线对比出报告。
  - 证据：报告文件 + 采样日志。

## 八、办结与登记

- 本板条目全部验收通过后，按 [TODO.md](../TODO.md) 流转规则同次移入[过去佛](CODE_TODO_PAST.md)（实现已交付、只剩统一批次）或直接办结退出并登记 archived points。
- 需要实机、tag CI、用户 secrets、上架凭证或异构目标机才能收口的条目，在迁移前先完成本板所有可推动作，再整条迁入[斗战胜佛](CODE_TODO_BLOCKED.md)。
