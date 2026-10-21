# MusicFrog 工程 TODO · 过去佛（已实现待验证与收口）

> 层级：L4

## 0. 编写规则

- 本文件里的工作仍未办结：已有实现包等待验证批次、正式产品采用或最终证据复验。已有局部 PASS 不自动变成完整验收。
- **四佛体系（无体外孤岛看板）：**模拟器（`-M`）与 ARM64 真机（`-D`）实机验收条件直接内聚在对应条目的“待验证”中；不设外部 `MACHINE_TODO` 孤岛看板，一处看清代码实现、单元测试与实机验收全貌。
- **执行标准（2026-10-20 起强制）：** 底层结构不够就扩 owner 结构，禁止再塞 `bool`/`Option`；组合优于继承；field 归 field、表现归表现、事务归 owner；每个事实只有一个真源，不留第二份缓存；非身份改动不碰 wire/快照/语料，身份改动集中一次做完。
- **本板只放可推条目：**条目的下一个可执行动作必须能在当前环境完成（统一 nextest/clippy/guard 批次、本地 Gradle 与模拟器 M1、本地真实引擎二进制 smoke、`cargo package`/dry-run）。剩余闭环动作全部依赖外部环境（ARM64 真机、受控 tag CI、用户 secrets、crates.io 上架凭证、异构目标机）的条目，整条迁入[斗战胜佛](CODE_TODO_BLOCKED.md)，本板不得预占。
- 每次先读 task 的验收、依赖与当前证据身份，执行最低充分验证；前置未满足保持 Blocked，验证发现实现缺口则把修复任务回流[现在佛](CODE_TODO.md)。凡发现底层架构未决或在上层修补规避底层问题的条目，不得标为 Integrated 或通过验证，必须将修复与底层裁决任务回流现在佛。
- 验证收口通过后，把条目从看板删除并在 [TODO.md](../TODO.md) 登记归档；不在本文件长期驻留已验证成果。
- `Integrated` 表示实现包已进入工作区、只剩验证/收口；位置不授予验证结论。若仅缺设备/真机证据，在待验证字段中明确标注，本条保持 Integrated 并注明设备依赖；只有实现依赖任务尚未完成时才标 Blocked。外部不可解条件（无真机、等 tag CI、等用户 secrets）不阻塞统一验证批次对其它条目的批量解锁。
- 迁移必须同次移出/移入（含整条正文），保持稳定 task ID、owner、依赖、验收与 points。
- **并行工流与验证纪律（2026-10-20 起强制）：** 并行开发按文件所有权切分，编译不通过的半成品不得放进会被 `cargo nextest --workspace`、结构门或 capture 构建扫到的路径（WIP 先放草稿或 `.evidence/wip/`）；开发期只跑受影响包，全仓 `cargo nextest --workspace` 与全量结构门统一放到收尾批次只跑一次；共享 `/tmp` 配额、cargo lane 争用与合成器起栈失败等外部故障先判定归属，只修本切片能修的那一类。
- 当前证据行只保留 run ID 与计数；完整证据文本见 `.evidence/CODE_TODO_PAST_EVIDENCE.md`（本地，不入库）。

## 一、Android VPN 生命周期与系统服务（本地 M1 可推，2条 / 6pt）

- [ ] BANDROID-002.1 [kind: mechanism] [priority: P0] [points: 3] [owner: Android VpnService + :vpn owner + CoreApplication] [status: Integrated] system-managed VPN startup and active-profile restore
  - 产物：显式 Start/Stop 与无命令启动由 sealed policy 区分；Android always-on 恢复唯一 `:vpn` owner 后复用 `StartProxy` 和当前 active profile；系统托管通知不提供本地 Stop；普通 ControlService sticky 重建不启动 Core/proxy。
  - 依赖：BANDROID-001.2、BANDROID-040.3/.7、CORE-015.7/.8.3。
  - 验收：API 24–28 legacy unflagged policy、API 29+ `isAlwaysOn`、malformed intent、always-on disabled、owner init failure/timeout 有 typed JVM 测试；StartProxy 共享行为测试通过。
  - 当前证据：`3162c1d0-88e1-491e-9764-b53f260232a9`；666 passed；52 passed。
  - 待验证：
    1. [模拟器 M1] 在 Compose/Bevy/Slint 三 flavor 各自安装同一候选源码 APK，启用 always-on，重启或 kill `:vpn` 确认系统恢复、active profile 配置与托管通知无本地 Stop；仅重建 ControlService 确认 Core/proxy 保持暂停。关闭条件：绑定 APK SHA、镜像、service/core PID、owner/session/profile readback 与 logcat。
    2. [ARM64真机 D] 在 ARM64 Android 29+ 验证上述同等场景与真实连通性。当前暂无 ARM64 设备，保持开放（设备腿登记在 [斗战胜佛](CODE_TODO_BLOCKED.md) §六 交叉索引，不阻塞本条的 M1 腿办结）。
  - 本机可执行动作：M1 腿（`scripts/android-emulator-tests.sh` + 本机 SDK/AVD 齐备），先做 M1 统一批次。
- [ ] BANDROID-002.2 [kind: validation] [priority: P0] [points: 3] [owner: :vpn host + NetworkRoamingApplication] [status: Integrated] 网络切换与服务终态回归
  - 产物：NetworkRoaming 初次/已观测基线和网关/地址变化以显式 enum 表达；`reconcile` 操作用 typed intent；TUN 未报告与已关闭分开处理，修复失败或关键观测缺失时保留旧基线以便重试。系统默认 underlay 交给 OS；VPN owner UID 从自身 TUN 排除；Stop/Revoke 清理 worker/FD/WakeLock。
  - 依赖：BANDROID-002.4；BANDROID-040.7 owner snapshot。
  - 验收：fake 覆盖初次观察、稳定/地址变化/网关迁移、自动修复失败与未报告 TUN 的重试、owner UID 排除、Stop/Revoke、WakeLock 与迟到终态。
  - 当前证据：`f9d6be76-6996-495b-9b78-b5181ef1046a`；`60d3f0c2-1571-4f55-8b61-37bc62b03c34`；3100 passed；11/11；183/183。
  - 待验证：
    1. [模拟器 M1] 安装含 002.4 的候选 APK，在流量中切换 Wi-Fi/断网/恢复，执行 Stop/Revoke，记录默认网络、VPN/Core snapshot、TUN/worker/FD 终态及 WakeLock 事件。关闭条件：APK SHA、切换前后流量与资源清理回执。
    2. [ARM64真机 D] ARM64 实机真实流量、Wi-Fi/蜂窝/断网/恢复、锁屏超时 WakeLock 释放。当前暂无 ARM64 设备，保持开放（设备腿登记在 [斗战胜佛](CODE_TODO_BLOCKED.md) §六 交叉索引）。
  - 本机可执行动作：M1 腿，先做 M1 统一批次。

## 二、Android IPC 与 Kotlin Host 边界（本地批次可推，2条 / 5pt）

- [ ] BANDROID-043.3 [kind: mechanism] [priority: P1] [points: 3] [owner: MihomoControlClient] [status: Integrated] Binder client 有界复用
  - 产物：Application 级复用单 binding/reply Handler/有界 pending 表；disconnect/death typed 失败清 timeout；caller cancellation 撤销未开始请求，不可取消副作用以同 request ID/generation 幂等终态回读；禁每请求 `HandlerThread`、禁 owner `runBlocking`。
  - 依赖：已交付的 sealed bind migration（bind callback 结果不再用 Boolean）。
  - 验收：正向/非法迁移测试与排队/取消/死亡重绑 fake 已添加；统一 Gradle 与设备证据待验证。
  - 当前证据：统一 Gradle 批次尚未回填。
  - 待验证：本地统一 Gradle 批次（`scripts/android-build.sh` + `:app:test*UnitTest`）回执；[ARM64真机] 实际设备运行与 Binder client 复用取证。
  - 本机可执行动作：先跑本地 Gradle 统一批次并回填证据身份。
- [ ] BANDROID-043.7 [kind: validation] [priority: P1] [points: 2] [owner: NativeHostAdapter] [status: Integrated] attach/detach 事务化
  - 产物：sealed attachment state；attach 失败逆序回滚；detach 逐项清理保留 `CleanupNeeded`；Rust bridge registration 按 generation 串行；结果不确定 fail closed。
  - 依赖：无。
  - 验收：注入式 Kotlin unit tests 统一批次通过。
  - 当前证据：注入式 Kotlin unit tests 批次尚未回填。
  - 待验证：本地注入式 Kotlin unit tests 统一批次回执；[ARM64真机] 真实 JNI 与设备生命周期证据。
  - 本机可执行动作：先跑本地 Kotlin unit tests 批次并回填证据身份。

## 三、UI 表面与组件（本地回归可推，1条 / 1pt）

- [ ] BANDROID-009.1 [kind: validation] [priority: P1] [points: 1] [owner: bevy-widgets::list + zhuge-ui-core] [status: Integrated] 虚拟列表变高共享几何回归
  - 产物：Proxies/Connections/Logs 虚拟窗口/行槽回收接入；变高 extent 前缀索引与窗口由 `zhuge-ui-core` 计算；旧 list-local fling/physics 已退役。
  - 依赖：无。
  - 验收：变高窗口共享几何回归统一 nextest 批次通过。
  - 当前证据：统一 nextest 批次尚未回填。
  - 待验证：本地统一 nextest 批次回执；[ARM64真机] 500/2000 节点滚到尾项的帧耗时/内存基准。
  - 本机可执行动作：先跑变高窗口共享几何回归批次并回填证据身份。

## 四、中性内核双引擎真实二进制与统一批次（可推，5条 / 11pt）
- [ ] PUB-002.1 [kind: mechanism] [priority: P1] [points: 3] [owner: mihomo-driver] [status: Integrated] Mihomo 驱动整合为独立 `mihomo-driver`
  - 产物：控制器 REST/WS、outbound/routing port、进程监督与精确终态、版本/readiness，均以 `lurker-core-ports` trait 暴露。
  - 依赖：PUB-001.1、PUB-001.2。
  - 当前证据（L）：driver 152 本地 nextest（mock/fake）；四目标 `cargo check` 通过；clippy `-D warnings` 0。
  - 待验证：真实 mihomo 二进制起停/readiness/selector 切换（driver 内无真进程测试，产品侧真进程 smoke 仍 `#[ignore]`）；`cargo publish --dry-run`（需 contract 先上架）；统一验证批次。
  - 本机可执行动作：跑 `scripts/core-backend-smoke-mihomo.sh` 真实二进制 smoke 并回填证据身份；`cargo publish --dry-run` 腿随 contract 上架在 [斗战胜佛](CODE_TODO_BLOCKED.md) §四 登记。
- [ ] PUB-002.2 [kind: mechanism] [priority: P1] [points: 2] [owner: mihomo-driver::codec] [status: Integrated] Mihomo YAML 配置 Codec 独立化
  - 产物：中性 YAML 解析/校验/`ProfileDocumentCodecPort`、字节保真 snippet 拼接（正文 `SnippetBodyLookup` 注入）、原生 `mihomo -t` dry-run。
  - 依赖：PUB-001.1、PUB-001.2。
  - 当前证据（L）：driver 130 + config 158 本地 nextest；parity 锚点已迁到 driver。
  - 待验证：`cargo publish --dry-run`；统一验证批次。
  - 本机可执行动作：跑统一验证批次（结构门 + parity + nextest + Clippy）并回填证据身份；dry-run 腿受 contract 上架限制。
- [ ] PUB-002.3 [kind: mechanism] [priority: P1] [points: 3] [owner: singbox-driver] [status: Integrated] sing-box 驱动整合为独立 `singbox-driver`
  - 产物：Clash API/selector、进程监督、readiness/outbound、版本探测，均以中性 trait 暴露。
  - 依赖：PUB-001.1、PUB-001.2。
  - 当前证据（L）：driver 13 + adapter 18 本地 nextest；四目标 `cargo check` 通过。
  - 待验证：真实 sing-box 二进制拉起/readiness/selector 安全退出（driver 内无真进程测试）；`cargo publish --dry-run`；统一验证批次。
  - 本机可执行动作：跑 `scripts/core-backend-smoke-singbox.sh` 真实二进制 smoke 并回填证据身份。
- [ ] PUB-002.4 [kind: mechanism] [priority: P1] [points: 2] [owner: singbox-driver::codec] [status: Integrated] sing-box JSON 配置 Codec 独立化
  - 产物：原生 JSON 模型、schema 校验、native `check` 事务与中性 `ProfileDocumentCodecPort`。
  - 依赖：PUB-001.1、PUB-001.2。
  - 当前证据（L）：driver codec 单测；产品 `SingBoxProfileConfigurationCodec` 只委托、typed 操作 fail-closed。
  - 待验证：`cargo publish --dry-run`；统一验证批次。
  - 本机可执行动作：跑统一验证批次并回填证据身份。
- [ ] PUB-002.5 [kind: evidence] [priority: P1] [points: 1] [owner: mihomo-driver + singbox-driver] [status: Integrated] 双引擎独立 example 与 nextest
  - 产物：两个 driver 各含 `examples/lifecycle.rs` 与 CI 安全 smoke。
  - 依赖：PUB-002.1～PUB-002.4。
  - 当前证据（L）：未配置时打印计划并 0 退出；配置路径有代码但未跑真二进制；driver 148 本地 nextest。
  - 待验证：**脱离本仓其他包的独立工程内 `cargo run --example` 真实生命周期**（当前是在仓内示例，且无引擎时只打印计划）；统一验证批次。
  - 本机可执行动作：在仓外临时工程跑 `cargo run --example` 并绑定真实二进制回执。

## 五、发布流水线与 parity 管线（可推，3条 / 6pt）

- [ ] PLATFORM-001.2 [kind: mechanism] [priority: P0] [points: 2] [owner: scripts/quality] [status: Integrated] 五大平台 × 三大 UI 业务覆盖守卫
  - 产物：`scripts/quality/parity-coverage-guard.py` 离线解析契约目录得出「业务 × 表面 × 层级」完整期望集，双向核对证据表与清单表；打印每表面 anchored/pending/missing 覆盖表与五大平台腿；14 类失败模式各有负例测试；接入 `scripts/quality/check-structure.sh`。
  - 依赖：PARITY-001.1、PLATFORM-001.1。
  - 验收：正例通过、缺格/重复/无锚/无原因/无载体等负例被拒；矩阵变更必须同时改守卫期望。
  - 当前证据（L）：`--self-test` PASS；`--mode enforce` 输出「59 业务 × 3 表面 × 2 层级 → 354 证据格、177 清单格；anchored 292、pending 62、missing 0」，并列出 iced/scenario 1、slint/contract 2、slint/scenario 59 个已声明缺口；新增 16 条单测全绿。
  - 待验证：与平台矩阵同批复核时确认五个平台腿登记（`PLATFORM-001.1`）。
  - 本机可执行动作：跑统一验证批次；矩阵变更时同步改守卫期望。

- [ ] PUB-006.1 [kind: infra] [priority: P0] [points: 2] [owner: CI] [status: Integrated] Crates.io 自动化发布 CI 工作流
  - 产物：`.github/workflows/release-neutral-crates.yml`（validate-topology → dry-run → publish 三段，tag-only，dispatch 仅演练）；`scripts/quality/neutral-release-guard.py`（拓扑审计、逐 crate 表格、dry-run 分类、顺序发布、索引等待、缺 token 硬拒绝）；`scripts/neutral-crate-release.sh` 本地可复跑入口与同源说明文档。
  - 依赖：PUB-001.1～PUB-003.3（七个中性 crate 已归档）。
  - 验收：本地与 CI 环境下 dry-run 顺序打包成功，依赖图谱无环且版本严格对齐。
  - 当前证据（L）：`bash scripts/neutral-crate-release.sh structure` 七行表格全 `ready`；`neutral-release-guard.py --self-test` PASS；新增 24 条单测全绿；`ci-trigger-guard`、`release-build-target-guard`、`apple-ci-deferral-guard`、`publish-order-guard` 四个守卫 enforce 全绿；无 `CARGO_REGISTRY_TOKEN` 时 publish 模式明确拒绝（exit 1，无匿名/调试回退）。
  - 待验证：受控 `v*` tag 的真实 workflow run 与 crates.io 上架回执（顺序上架门在[斗战胜佛](CODE_TODO_BLOCKED.md) §四）。
  - 本机可执行动作：跑统一验证批次与 `cargo publish --dry-run`（依赖未上架时记为顺序门，不算缺陷）。
- [ ] PARITY-001.1 [kind: mechanism] [priority: P0] [points: 2] [owner: scripts/parity + infiltrator-contract] [status: Integrated] Slint 纳入 parity 目录、注册表与视觉回执管线
  - 产物：契约目录 `FeatureSpec` 增 `slint_page` 并回填 59 条载体归属；注册表补 59×2 个 Slint 单元格（57 `pending` + 逐条缺失能力原因、2 `implemented`）；`resolve_surface_evidence.py` 的 `SURFACES` 增 slint 并新增「无载体页面不得锚定」规则；`visual_receipts.py` 接受 `--slint-run`；`evidence_progress.py` 与验收脚本按三表面统计；契约测试同步校验三表面与载体规则。
  - 依赖：SLINT-001.1、GAP-4.1 场景表。
  - 验收：177 单元格全部可解析且无 dangling/invalid；`--require-complete` 在 Slint 补齐前必须仍失败。
  - 当前证据（L）：`resolve_surface_evidence.py --structure-only` status=pass、177 cells、293 anchors；契约 nextest `be57f290-e562-48fb-a817-c13093f1bfce` 1/1。
  - 待验证：Slint 真实 capture run 已产出首批像素回执——`profiles-import-modal` 与 `profiles-delete-confirm` 各有标准/紧凑双视口共 4 张通过校验（PNG SHA-256、原生 bounds、进程与窗口 ID、构建清单摘要全部绑定），注册表对应两格已从 `implemented` 转 `ready`；其余 57 条业务仍 pending，随 `SLINT-001.2`～`001.7` 逐页补齐。
  - 本机可执行动作：跑统一验证批次；把仍为 pending 的 57 格按 SLINT 条目逐项转 anchored。

## 六、Slint 第三 UI 表面（已交付，2条 / 4pt）

- [ ] SLINT-001.1 [kind: mechanism] [priority: P0] [points: 2] [owner: infiltrator-slint] [status: Integrated] Slint 表面行为测试台与 scenario 锚点
  - 产物：`slint::surface::harness` 无窗口行为台（软件渲染器 + 排队事件循环），在真实 `AppWindow` 上注册真实回调、派发真实手势并回报派发的 `SlintAction`、typed `Failure` 与 owner 可见投影；`slint::surface::action` 收拢全部产生 typed owner 命令的表面规则。
  - 依赖：SLINT-001.8（同一批交付）。
  - 验收：Slint 行为测试必须驱动表面并断言派发意图与投影结果；纯意图形状断言不得计入 scenario 锚点。
  - 当前证据（L）：`cargo nextest run -p infiltrator-slint` 72/72 全绿；clippy `-D warnings` 0。
  - 待验证：把已实现的 21 条业务逐条接成 parity scenario 锚点（随 `PARITY-001.2`）。
  - 本机可执行动作：用该行为台补 Slint 的 scenario 锚点。
- [ ] SLINT-001.8 [kind: mechanism] [priority: P0] [points: 2] [owner: infiltrator-slint] [status: Integrated] 形状级测试文件改为驱动真实表面（消除第二业务真源）
  - 产物：`connections_and_logs_tests`、`core_and_network_mode_tests`、`outbound_and_probe_tests`、`rules_and_settings_tests` 四个文件不再在用例里重实现路由模式映射、网络模式序列、主题白名单、十六进制解析与连接排序，改为调用 `surface::action` 并驱动真实表面。
  - 依赖：SLINT-001.1。
  - 验收：四个文件的用例全部经 Slint 表面断言派发 typed 意图与投影结果；重实现代码清零。
  - 当前证据（L）：41 条表面行为用例（路由/网络模式 8、代理与测速 12、规则与设置 14、连接与日志 7）全绿；测试树内 `route_mode`/`theme_whitelist`/`parse_color`/`sort_connections` 本地副本搜索为空；`cargo nextest run -p infiltrator-slint` 72/72；clippy 0。
  - 待验证：Android 分支的网络模式序列需在 Android target 跑；日志等级过滤存在「第二次发布才过滤」的顺序瑕疵（已记录，未改产品行为）。
  - 本机可执行动作：Android target 的网络模式用例补测。

## 七、Bevy UI 编写律法强制化（已交付，4条 / 10pt）

- [ ] BEVYCAP-001.1 [kind: mechanism] [priority: P0] [points: 2] [owner: scripts/quality + bevy-widgets] [status: Integrated] rem/em 尺寸通道机器守卫
  - 产物：产品守卫 `scripts/quality/bevy_rem_guard.py`（`BEVY-REM-001`～`005`：命名/派生 px 越出物理白名单、硬编码 px 字面量、非 rem 字号、字面量 `em`、未命名 rem 尺寸、债务台账完整性）＋ 只允许缩小的存量台账 `scripts/quality/bevy_rem_inventory.json` ＋ 规则回归 `scripts/quality/tests/test_bevy_rem_guard.py` ＋ 结构门注册；守卫覆盖共享包场景门禁的全部尺寸规则，是本产品唯一阻塞执行器（上游律法为超集下限）。
  - 依赖：无（能力规范①的机器红线）。
  - 验收：守卫自测正例放行/负例拒绝；生产树零违例；`check-structure.sh` 全绿。
  - 当前证据（L）：`bevy_rem_guard.py --self-test` PASS（含 10 条负例）；`--mode enforce` 生产树 356 文件 0 违例 0 漂移；单测 14 例全绿；结构门已注册。
  - 待验证：统一结构门批次（随本批收口）。
  - 本机可执行动作：跑统一结构门批次。
- [ ] BEVYCAP-001.2 [kind: mechanism] [priority: P0] [points: 3] [owner: infiltrator-bevy-ui + infiltrator-bevy-widgets] [status: Integrated] 存量 px 尺寸迁移到 rem 通道
  - 产物：两个 crate 生产树 **180 处非白名单 px 全量迁到 rem 通道**（字面量 132 + 命名 48），尺寸 token 化（`space::S*`/`radius::*`/`metrics::*`），胶囊圆角改 `BorderRadius::MAX`（29 处），一次性布局尺寸改角色命名的文件内常量（`PALETTE_PANEL_WIDTH_PX` 等 70 余处），存量台账归零（`{"debt": {}}`）。
  - 依赖：BEVYCAP-001.1 的守卫与白名单口径。
  - 验收：两份台账归零且守卫零违例；调用户字号档位后全 UI 跟随缩放（像素回执）；非白名单 px 站点清零搜索。
  - 当前证据（L）：`bevy_rem_guard.py --mode enforce` 0 违例；`bevy_scene_gate.py` 棘轮 0 new / 0 stale；`bevy_bsn_guard.py` 356 文件 0 违例；widgets nextest 410/410 全绿；清零搜索 `rem([0-9]` 与 `Val::Px([0-9]` 在两个 crate 生产树均无残留。
  - 当前证据（像素回执，D 级本地 compositor）：`INFILTRATOR_BEVY_FONT_SCALE=1.0` 与 `1.5` 同场景 `overview-dark` 各一张真实截图（虚拟 KWin + 嵌套 niri，窗口 1180×760，binary sha256 `bfe09f18ac265026d74d1074b84a551503533857a935cc1044c87449a5e24038`）：PNG 93108 B / sha `4551075635303041…` 与 106326 B / sha `7251d85f4e5178f5…`，**逐像素比对 46.2% 像素变化、差异框覆盖整帧**（`.evidence/captures/bevy/20261020T052742Z_…_1832886` 与 `…T052942Z_…_1843475`）——用户字号偏好真实缩放全 UI，rem 通道的「唯一缩放入口」成立（回执取自 dirty 工作树，binary sha 已记录）。
  - 待验证：无（本条验收已由像素回执闭环）。
  - 本机可执行动作：跑 `bash scripts/capture-bevy-matrix.sh <scenario>` 两档字号各一张并比对。
- [ ] BEVYCAP-003.1 [kind: mechanism] [priority: P1] [points: 2] [owner: scripts/quality + 共享包 owner] [status: Integrated] Bevy 场景律法唯一 owner 收敛
  - 产物：裁决**产品自有守卫 = 唯一阻塞执行器且是共享包律法的严格超集**——`bevy_bsn_guard.py`（BSN-001～006 + ECS-007～009 + 新增 BSN-007 `template_value` 硬零 + 007～010 小件收敛）与 `bevy_rem_guard.py`（REM-001～005）两域分治；共享包场景门禁降为上游镜像（advisory，不阻塞、不作放行依据）；权威台账唯一（产品库存只允许缩小，上游棘轮为其 owner 的只读镜像）；覆盖差表写入能力规范「强制化台账」。
  - 依赖：BEVYCAP-001.1／001.2。
  - 验收：产品守卫覆盖上游全部规则且零违例；每个事实一个阻塞执行器 + 一份权威台账。
  - 当前证据（L）：`bevy_bsn_guard.py --mode enforce` 356 文件 0 违例；`bevy_rem_guard.py --mode enforce` 0 违例；上游门禁 `new=0 stale=0`；能力规范覆盖差表已登记 17 条事实的归属。
  - 待验证：统一结构门批次。
  - 本机可执行动作：跑统一结构门批次。
- [ ] TOKEN-001.1 [kind: mechanism] [priority: P1] [points: 3] [owner: infiltrator-contract::design_tokens + bevy-widgets::theme] [status: Integrated] 圆角阶梯缺档导致 rem 迁移留下字面量
  - 产物：裁决**契约阶梯无需改动**——三处待定角色全部落在既有语义档（命令面板行 → `radius::SM`、命令面板浮层 → `radius::MODAL`、编辑器工具条 → `radius::CONTROL`、状态点 → `radius::XS` 夹取后不变）；契约 `design_tokens.rs` 与 widgets `theme.rs` 镜像字节不变；其余一次性尺寸改角色常量并由 `BEVY-REM-005` 机械禁止 `rem(<裸数字>)`。
  - 依赖：BEVYCAP-001.2 的存量迁移。
  - 验收：契约与镜像一致且 `design_token_tests` 全绿；Bevy 侧 `rem(<裸数字>)` 清零。
  - 当前证据（L）：`design_token_tests` 5/5、contract 344/344、widgets 主题 27/27 全绿；`bevy_rem_guard.py` 0 违例（`BEVY-REM-005` 生效）。
  - 待验证：Iced 侧命令面板浮层仍用 `R_CARD`(16) / 子面板 `R_CARD`，与契约登记的 `MODAL`(24) 角色不一致——归 Iced owner 收敛（跨表面对齐，见 `PARITY-001.2`）。
  - 本机可执行动作：跑统一结构门批次；Iced 侧圆角收敛归其 owner。

- [ ] BEVYCAP-002.1 [kind: mechanism] [priority: P1] [points: 3] [owner: infiltrator-bevy-ui + infiltrator-bevy-widgets] [status: Integrated] ⑦ 小件收敛强制化（bsn_list!/Ready/despawn_all/胶囊）
  - 产物：能力规范⑦从推荐变红线并机器化——守卫新增 `BEVY-BSN-008`（列表场景必须 `bsn_list!`）、`BEVY-BSN-009`（批量退役必须 `despawn_all`）、`BEVY-BSN-010`（胶囊必须 `BorderRadius::MAX`）+ 既有 `BEVY-BSN-007`（`template_value` 硬零）；存量迁移：6 处真实列表场景改 `bsn_list!` + `SceneList`（`proxies_form`／`proxies_custom`／`settings_language`／`proxies_virtual`／`profiles_diff`／`rules_json`），1 处挂载后初始化改订阅 `bevy_scene::Ready`（`rules_json` 编辑器体：挂载帧丢投影的根因修复），1 处批量退役改 `despawn_all`（`proxies_virtual` 槽位池），胶囊圆角全量 `BorderRadius::MAX`（29 处）；固定 2–3 个手写子项的静态 `Children [ ... ]` 明确保留（非列表场景）。
  - 依赖：BEVYCAP-001.1 的守卫骨架与遮罩工具。
  - 验收：守卫零违例；列表场景清零搜索；焦点落点与首帧投影回归通过。
  - 当前证据（L）：`bevy_bsn_guard.py --mode enforce` 356 文件 0 违例（含 4 条新规则负例自测）；`bevy_rem_guard.py` 0 违例；`cargo nextest run -p infiltrator-bevy-ui --features capture-tools --test headless` 552/552；`--lib` 153/153；`line-guard` 0 违例。
  - 待验证：真实交互与像素验收（焦点落点、首帧投影的用户可见证据）随本批截图批次。
  - 本机可执行动作：跑统一截图批次。
- [ ] BEVYCAP-004.1 [kind: mechanism] [priority: P1] [points: 3] [owner: infiltrator-bevy-widgets + infiltrator-bevy-ui] [status: Integrated] 浮层根补 `ThemeContext(SurfaceLevel::Floating)` 与 `FixedNode` 并机器化
  - 产物：能力规范②④的存量缺口收口——九个浮层 owner（toast 栈、命令面板、modal、adaptive modal、菜单、popover、combobox、上下文菜单、mini HUD）的浮层根全部声明官方 `ThemeContext(SurfaceLevel::Floating)` 并挂载官方 `FixedNode`；新增守卫规则 `BEVY-BSN-011`（声明浮层根标记的模块必须声明 floating 层级，owner 由标记定义自动推导，不写死文件表），能力规范②④从「评审口径 + 存量缺口」变为机器红线。
  - 依赖：无（官方 `bevy::feathers::theme` 与 `bevy::ui::FixedNode` 已在依赖里）。
  - 验收：八个浮层根全部声明并挂载（清零搜索）；守卫规则生效且生产树零违例；浮层行为回归全绿。
  - 当前证据（L）：`bevy_bsn_guard.py --mode enforce` 359 文件 0 违例（含 `BEVY-BSN-011` 的正反例自测，owner 由标记定义自动推导，覆盖 adaptive modal）；`bevy-widgets` nextest 414/414 全绿；能力规范②④行已改为「机器强制」并写明实测。
  - 待验证：浮层换肤的真实像素回执（浮层层级色）随本批截图批次。
  - 本机可执行动作：跑统一截图批次。
- [ ] WIDGET-001.1 [kind: mechanism] [priority: P1] [points: 3] [owner: infiltrator-bevy-widgets] [status: Integrated] 开关（switch）轨道与旋钮几何收敛为共享控件
  - 产物：rem 迁移实测发现的重复事实（同一开关几何在四个页面各写一份常量）收敛为**唯一 owner**：`infiltrator_bevy_widgets::switch` 拥有 `SWITCH_TRACK_WIDTH_PX`/`SWITCH_TRACK_HEIGHT_PX`/`SWITCH_KNOB_SIZE_PX`/`SWITCH_KNOB_ON_OFFSET_PX`/`SWITCH_KNOB_OFF_OFFSET_PX`、typed `SwitchPosition{Off,On}`（由二值事实 `from_enabled` 派生）与两个场景构造器（轨道带发丝线边框 + `BorderRadius::MAX` 胶囊，旋钮按 typed 偏移绝对定位）；四个页面（`settings_preferences`／`dns_form`／`profiles_import`／`dns_edit`）改为消费共享场景并删除本地常量。
  - 依赖：无（纯 widgets owner 内的补结构）。
  - 验收：四个页面不再声明开关几何常量（清零搜索）；共享开关有 headless 行为测试（开/关两态的轨道、旋钮位置与胶囊圆角）；页面行为与视觉不变。
  - 当前证据（L）：清零搜索 `TRACK_WIDTH_PX|TRACK_HEIGHT_PX|KNOB_SIZE_PX|KNOB_ON_OFFSET|KNOB_OFF_OFFSET` 在 `infiltrator-bevy-ui/src` 无匹配；`knob_offset_px` 只剩控件自身与断言——**页面已不再推导任何偏移**：`settings_preferences` 改为翻转 `SwitchKnob` 的 typed 位并由控件重投影；控件现在是一个整体（`SwitchColors` + `SwitchTrack` + `SwitchKnob(SwitchPosition)` + `switch_control_scene` + `sync_switch_knobs`），而不是让页面自行拼装轨道与旋钮；`switch_tests.rs` 6 条用例（轨道胶囊几何、typed 开/关偏移、typed 决策、父子组合、两个消费者渲染同一几何、旋钮只随 typed 位移动）；`bevy-widgets` nextest 431/431 全绿；rem/bsn/line 守卫 0 违例。
  - 待验证：真实像素回执（开关两态的轨道与旋钮）随本批截图批次。
  - 本机可执行动作：跑统一截图批次。
- [ ] BEVYCAP-004.2 [kind: mechanism] [priority: P1] [points: 3] [owner: infiltrator-bevy-widgets::modal + infiltrator-bevy-ui] [status: Integrated] modal 层迁到官方 `Dialog`/`ModalDialog`/`RequestClose` 并把 bool 对改 typed stage
  - 产物：能力规范⑥收口——**全部模态面**（共享 modal、命令面板、13 个产品模态页、adaptive modal）挂载官方 `ModalDialog`/`ModalDialogBarrier`/`RequestClose` 并注册 `ModalWidgetsPlugin`（`DialogPlugin`+`ModalDialogPlugin`），barrier 点击外关闭、焦点圈闭（`Dialog` → `TabGroup{modal:true}`）、`DialogStack` 与 Esc 拦截全部由 Bevy 提供；打开时把输入焦点交接进对话框（`TabIndex` + `AcquireFocus`）、关闭时释放，手写 `KeyCode::Escape` 分支已删除（命令面板这类键盘控制台与子菜单阶段除外，已注明）；业务状态保留在产品侧但改为**一个 typed stage** `ModalStage{Hidden, Open, Dismissed}`（`is_open()` 为派生方法，旧的 `is_open`/`dismiss_on_scrim` bool 对消失，不再能表达非法组合）；新增 `button_action_scene` 让确认/取消标记与官方 `Button` 同节点，官方 `RequestClose` 的原始触发者（`original_event_target`）决定 `Confirmed`/`Cancelled`/`Dismissed` 结果。
  - 依赖：Bevy 0.20 `bevy::ui_widgets` 官方 `Dialog`/`ModalDialog`/`RequestClose`/`DialogStack`；`menu` 层为已评审的本地例外，不在本条范围。
  - 验收：弹窗行为 headless 回归（开/关/取消/确认/焦点圈闭）全绿；`implicit-state-audit` 判决台账同步（旧的 `ModalState::multi-bool` 判决已撤销）。
  - 当前证据（L）：`bevy-widgets` nextest 419/419、`bevy-ui` nextest 716/716 全绿；`implicit-state-audit` 报 22 候选 / 0 未判决 / 0 open / 0 stale（撤销旧判决后）。
  - 待验证：真实交互与像素回执（官方 barrier/Esc/焦点圈闭的用户可见证据）随本批截图批次。
  - 本机可执行动作：跑统一截图批次。

## 八、Android 静态质量门（已交付，4条 / 9pt）





## 九、文档与状态表达红线（已交付，2条 / 5pt）



## 十、开源生态与发布门禁（已交付，4条 / 7pt）





## 十一、平台证据与安装包（已交付，2条 / 5pt）



## 十二、地球增强与其帧/内存报告（已交付，2条 / 4pt）



## 十三、行为锚点补齐（已交付，1条 / 2pt）

- [ ] PARITY-001.3 [kind: mechanism] [priority: P0] [points: 2] [owner: infiltrator-iced + infiltrator-bevy-ui] [status: Integrated] 四个 pending 行为锚点补齐
  - 产物：四个 pending 行为锚点补齐（`core-version-rollback-confirm`、`proxies-search-highlight`、`speedtest-progress-cancel` 的 contract 与 scenario 锚点）。
  - 依赖：共享 typed 状态与 command 契约。
  - 验收：四格状态由 pending 转 anchored，且测试覆盖失败恢复、拒绝与取消终态。
  - 计划：① 逐格补行为测试（不复用 demo 快照冒充）；② 更新注册表 `test_id`/`reason`；③ 跑 resolver 确认零 pending。
  - 当前证据（L）：四格全部 anchored——`core-version-rollback-confirm` 的 iced/bevy/slint contract 由 `infiltrator-application::version_application::tests` 的三条回滚用例（转发引擎身份并回报激活版本 / 失败不伪造 / 外来引擎 fail-closed）承担；`proxies-search-highlight` 的 iced scenario 由 `infiltrator_iced::headless::proxy_search_behavior_tests`（typed 偏好提交、命中高亮、诚实空态、清空恢复、owner 拒绝时保留已确认偏好与可重试草稿、刷新后重放查询）承担；**`speedtest-progress-cancel` 的 iced scenario 由 `infiltrator_iced::headless::speedtest_cancel_tests::the_running_speedtest_control_cancels_the_observed_run_and_lands_the_owner_outcome` 承担**——运行中的测速控件从观测到的 owner 快照取动作，只为那一轮 run 提交共享 typed 取消命令，owner 的 typed 回答落到表面状态而不是本地自造的��功；该用例同时覆盖失败恢复与拒绝终态。
  - 待验证：统一验证批次复跑 resolver，确认四格在当前代码上仍 anchored 且没有 dangling 锚点。
  - 本机可执行动作：跑统一验证批次。

## 十四、真实二进制与 Android 取证（已交付，2条 / 5pt）

- [ ] PUB-006.8 [kind: evidence] [priority: P1] [points: 2] [owner: core/drivers] [status: Integrated] 双引擎驱动真实二进制本地 Smoke 取证
  - 产物：激活 `mihomo-driver`/`singbox-driver` 的真实二进制 smoke 套件并固化本地可复跑入口。
  - 依赖：PUB-002.1、PUB-002.3、PUB-006.1。
  - 验收：真实进程下完成 API 握手、配置写入、节点测速与流量分发冒烟验证。
  - 当前证据（L）：两个驱动各有一个具名入口 `real_process_evidence` 与一个 **typed 环境契约**（`Absent{missing}` / `Present(target)` / `Invalid{reason}` 三种互斥结局，绝不是 flag 组合）：二进制缺失时产出 `environment_absent` 回执**并且让测试失败**，绝不静默变绿；四项证明都是 typed——API 握手（`CoreReadiness::probe` + `active_core_version`）、配置写入与回读、节点测速（typed `OutboundPort::probe` → `OutboundProbeState`）、经引擎自带 SOCKS5 监听器的真实流量分发（回环夹具回显载荷）；回执的总体结论**由证明记录推导**（`complete` / `unsupported_proof` / `defect`），回执不可能声称比它的证明更多。
  - 当前证据：三个 smoke 脚本打印一行机器可读摘要 `MUSIC_FROG_SMOKE_SUMMARY <json>`（schema `music-frog.core-backend-smoke/1`，含 binary 路径/sha256/版本/来源、proven/not_proven/skipped（各带原因）、defects、driver_receipts），并从 **Rust 契约**推导环境变量名（不是再抄一份）；退出码 0 = proven/incomplete/environment_absent，1 = 真缺陷，2 = 用法错误。
  - 当前证据：新增守卫 `real-process-evidence-guard.py`（入口必须保持 `#[ignore]` 且可用 `--run-ignored` 触达、契约必须保持三态枚举、回执必须保留四项证明与 `environment_absent` 结局、脚本必须打印摘要且不得硬编码契约键），自测 8 条负例 + 11 条单测全绿并接入结构门。
  - 待验证：**sing-box 的节点测速仍未证明**——sing-box 驱动的 typed `OutboundPort::probe` 返回 `OutboundProbeState::Unsupported`，回执如实记为 `unsupported_proof`（引擎本身不是瓶颈：真实二进制上 sing-box 的 delay 接口确实返回 `{"delay": N}`，缺口在驱动的 typed port，已回流为现在佛 `PUB-006.13`）；mihomo 的测速只能经 HTTP-proxy 出站完成（`DIRECT` 出站的 delay 测试返回 503），脚本的探针出站指向本机 HTTP 代理夹具。
  - 待验证：统一验证批次跑一次真实二进制回执（`bash scripts/core-backend-smoke-mihomo.sh` / `-singbox.sh` / `-conformance.sh`，可用 `MIHOMO_REAL_BINARY` / `SINGBOX_REAL_BINARY` 指向本机二进制）；Windows 腿在斗战胜佛。
  - 本机可执行动作：跑统一验证批次与真实二进制 smoke。

- [ ] PARITY-001.4 [kind: evidence] [priority: P0] [points: 3] [owner: android 三 flavor] [status: Integrated] Android 三 flavor 实际交互截图与行为认证
  - 产物：Compose / Bevy / Slint 三个 flavor 在模拟器上的真实交互截图与行为回执（启动、导航、列表滚动、权限与 VPN consent 终态）。
  - 依赖：模拟器 M1 腿（BANDROID-002.1/002.2 在过去佛）、`scripts/android-emulator-tests.sh`、APK identity 守卫。
  - 验收：三 flavor 各绑定同一候选源码 APK 的 SHA-256 与截图清单；安全区、键盘与滚动命中区取证完整。
  - 当前证据（L）：新增 `scripts/android/` 取证流（13 个文件）：按 flavor 驱动一次模拟器取证并写回执与报告；`capture_device.py` **只接受 `emulator-<port>` 序列**，永不选中物理设备；`capture_actions.py` 是 typed 动作词汇表（launch / tap_text / tap_nav / tap_anchor / swipe_up / wait_text / tap_dialog / revoke_permission）；`capture_pixels.py` **复用** `scripts/parity/interaction_delta.py` 作为唯一像素判决来源，桌面与 Android 对「交互是否真的发生」用同一把尺子。
  - 当前证据：每个步骤的回执绑定 APK 摘要、所属业务 id、截图与其摘要、证明该步发生过的 logcat 片段，以及相对噪声地板的实测像素差；**无法观测的步骤不产回执**，如实记为 unproven 并写明原因，依赖它的步骤记为 blocked（绝不伪造回执）；守卫 `android-interaction-receipt-guard.py` 拒绝：缺 APK 摘要 / 缺业务 id / 业务 id 不在 `feature_evidence.tsv` / 缺 logcat 证据 / 标记从未出现过 / 像素差未过噪声地板 / 未解析的动作被报成 proven / 空回执集 / 截图全同（什么都没证明）；守卫与取样表的负例用例全绿并接入结构门。
  - 待验证：真实回执需要一次三 flavor 候选 APK 构建 + 一次性模拟器 + 三个 flavor 的取证运行（收口批次执行）；**Bevy 与 Slint flavor 在 Android 上不发布无障碍树**，所以 Overview 电源控件没有标签，这两条 flavor 的 VPN consent / 通知授权 / 终态步骤如实报 unproven（锚点表补标后即可自动化，Compose 已原生证明这三步）；模拟器上没有可用内核时 VPN 终态回读为 `not-established`，回执如实记录而不谎称 running；真机腿在斗战胜佛 §一。
  - 本机可执行动作：跑统一验证批次：构建三 flavor Debug APK → identity 守卫 → 起一次性模拟器 → 三个 flavor 取证 → 合并与守卫校验。

## 十五、Slint 第三表面补齐（已交付，7条 / 15pt）

- [ ] SLINT-001.2 [kind: mechanism] [priority: P1] [points: 3] [owner: infiltrator-slint] [status: Integrated] Slint DNS 页（4 业务）
  - 产物：Slint DNS 页（4 业务）的载体在 Slint 表面落地，并接入共享 owner 快照与共享 typed 命令。
  - 依赖：SLINT-001.1 行为台；共享 presentation 投影。
  - 验收：该业务在 Slint 有表面行为测试与双视口像素回执。
  - 当前证据（L）：四个 DNS 业务在 `dns` 页有载体：泄漏告警派发共享探测命令并投影共享结论、查询面板派发共享查询命令并拒绝非法名称、Fake-IP 缓存刷新派发共享刷新命令并可取消、Hosts 编辑器提交共享受保护的写入并拒绝非法行；表面不实现任何自己的状态机（查询面板、缓存刷新、Hosts 编辑器都是共享 presentation reducer）。
  - 当前证据：Slint 包 nextest 117/117 全绿（其中 33 条 workbench 表面行为用例 + 2 条回调用例）；契约已登记该业务的 `slint_page` 载体页，parity 注册表逐条记录**已证明的交互路径**与**仍缺的路径**（缺路径的业务不计入已认证 scenario）。
  - 待验证：标准/紧凑双视口真实像素回执随统一截图批次产出；缺交互路径的业务在补齐前保持 `pending`。
  - 本机可执行动作：跑统一验证批次与统一截图批次。

- [ ] SLINT-001.3 [kind: mechanism] [priority: P1] [points: 3] [owner: infiltrator-slint] [status: Integrated] Slint 编辑器族页面（4 业务：脚本沙箱/导出评审/差异视图/快照恢复）
  - 产物：Slint 编辑器族页面（4 业务：脚本沙箱/导出评审/差异视图/快照恢复）的载体在 Slint 表面落地，并接入共享 owner 快照与共享 typed 命令。
  - 依赖：SLINT-001.1 行为台；共享 presentation 投影。
  - 验收：该业务在 Slint 有表面行为测试与双视口像素回执。
  - 当前证据（L）：四个编辑器业务在 `editor` 页有载体：脚本沙箱派发共享运行命令并可清空、导出评审准备共享产物并拒绝二次准备、配置差异加载共享 diff、快照恢复准备共享评审。
  - 当前证据：Slint 包 nextest 117/117 全绿（其中 33 条 workbench 表面行为用例 + 2 条回调用例）；契约已登记该业务的 `slint_page` 载体页，parity 注册表逐条记录**已证明的交互路径**与**仍缺的路径**（缺路径的业务不计入已认证 scenario）。
  - 待验证：标准/紧凑双视口真实像素回执随统一截图批次产出；缺交互路径的业务在补齐前保持 `pending`。
  - 本机可执行动作：跑统一验证批次与统一截图批次。

- [ ] SLINT-001.4 [kind: mechanism] [priority: P1] [points: 2] [owner: infiltrator-slint] [status: Integrated] Slint 配置过滤与聚合器（3 业务）
  - 产物：Slint 配置过滤与聚合器（3 业务）的载体在 Slint 表面落地，并接入共享 owner 快照与共享 typed 命令。
  - 依赖：SLINT-001.1 行为台；共享 presentation 投影。
  - 验收：该业务在 Slint 有表面行为测试与双视口像素回执。
  - 当前证据（L）：过滤编辑器提交共享过滤流水线、聚合器向导预览共享报告并创建共享配置且拒绝空目标、聚合预览投影共享 `AggregationReport`，三个业务在 `filters` 页有载体。
  - 当前证据：Slint 包 nextest 117/117 全绿（其中 33 条 workbench 表面行为用例 + 2 条回调用例）；契约已登记该业务的 `slint_page` 载体页，parity 注册表逐条记录**已证明的交互路径**与**仍缺的路径**（缺路径的业务不计入已认证 scenario）。
  - 待验证：标准/紧凑双视口真实像素回执随统一截图批次产出；缺交互路径的业务在补齐前保持 `pending`。
  - 本机可执行动作：跑统一验证批次与统一截图批次。

- [ ] SLINT-001.5 [kind: mechanism] [priority: P1] [points: 2] [owner: infiltrator-slint] [status: Integrated] Slint 诊断与生命周期恢复页（2 业务）
  - 产物：Slint 诊断与生命周期恢复页（2 业务）的载体在 Slint 表面落地，并接入共享 owner 快照与共享 typed 命令。
  - 依赖：SLINT-001.1 行为台；共享 presentation 投影。
  - 验收：该业务在 Slint 有表面行为测试与双视口像素回执。
  - 当前证据（L）：诊断页派发共享诊断/修复单条/修复全部命令并投影共享报告，生命周期恢复面板派发共享引导命令，两条业务在 `doctor` 页有载体。
  - 当前证据：Slint 包 nextest 117/117 全绿（其中 33 条 workbench 表面行为用例 + 2 条回调用例）；契约已登记该业务的 `slint_page` 载体页，parity 注册表逐条记录**已证明的交互路径**与**仍缺的路径**（缺路径的业务不计入已认证 scenario）。
  - 待验证：标准/紧凑双视口真实像素回执随统一截图批次产出；缺交互路径的业务在补齐前保持 `pending`。
  - 本机可执行动作：跑统一验证批次与统一截图批次。

- [ ] SLINT-001.6 [kind: mechanism] [priority: P1] [points: 2] [owner: infiltrator-slint] [status: Integrated] Slint 订阅同步冲突页（2 业务）
  - 产物：Slint 订阅同步冲突页（2 业务）的载体在 Slint 表面落地，并接入共享 owner 快照与共享 typed 命令。
  - 依赖：SLINT-001.1 行为台；共享 presentation 投影。
  - 验收：该业务在 Slint 有表面行为测试与双视口像素回执。
  - 当前证据（L）：同步冲突页派发共享同步命令、保留本地副本、接受远端副本并投影共享冲突，恢复确认派发共享备份命令，两条业务在 `sync` 页有载体。
  - 当前证据：Slint 包 nextest 117/117 全绿（其中 33 条 workbench 表面行为用例 + 2 条回调用例）；契约已登记该业务的 `slint_page` 载体页，parity 注册表逐条记录**已证明的交互路径**与**仍缺的路径**（缺路径的业务不计入已认证 scenario）。
  - 待验证：标准/紧凑双视口真实像素回执随统一截图批次产出；缺交互路径的业务在补齐前保持 `pending`。
  - 本机可执行动作：跑统一验证批次与统一截图批次。

- [ ] SLINT-001.7 [kind: mechanism] [priority: P1] [points: 2] [owner: infiltrator-slint] [status: Integrated] Slint 应用路由策略编辑页（1 业务）
  - 产物：Slint 应用路由策略编辑页（1 业务）的载体在 Slint 表面落地，并接入共享 owner 快照与共享 typed 命令。
  - 依赖：SLINT-001.1 行为台；共享 presentation 投影。
  - 验收：该业务在 Slint 有表面行为测试与双视口像素回执。
  - 当前证据（L）：应用路由策略编辑器派发共享模式变更与共享规则变更命令、拒绝未知规则并投影共享策略，该业务在 `app_routing` 页有载体。
  - 当前证据：Slint 包 nextest 117/117 全绿（其中 33 条 workbench 表面行为用例 + 2 条回调用例）；契约已登记该业务的 `slint_page` 载体页，parity 注册表逐条记录**已证明的交互路径**与**仍缺的路径**（缺路径的业务不计入已认证 scenario）。
  - 待验证：标准/紧凑双视口真实像素回执随统一截图批次产出；缺交互路径的业务在补齐前保持 `pending`。
  - 本机可执行动作：跑统一验证批次与统一截图批次。

- [ ] SLINT-001.9 [kind: mechanism] [priority: P1] [points: 1] [owner: infiltrator-slint] [status: Integrated] 两个未接线的 Slint 回调接线
  - 产物：两个未接线的 Slint 回调接线的载体在 Slint 表面落地，并接入共享 owner 快照与共享 typed 命令。
  - 依赖：SLINT-001.1 行为台；共享 presentation 投影。
  - 验收：该业务在 Slint 有表面行为测试与双视口像素回执。
  - 当前证据（L）：`refresh-profile(string)` 与 `refresh-home-egress()` 都有了真实控件：订阅动作面板的「刷新」按钮派发共享 `UpdateProfile` 命令，Home 出口 IP 旁的「刷新」按钮（紧凑与桌面两种形态）派发共享 `RefreshPublicIpProbe` 命令——两个回调不再是「声明了但无调用点」。
  - 当前证据：Slint 包 nextest 117/117 全绿（其中 33 条 workbench 表面行为用例 + 2 条回调用例）；契约已登记该业务的 `slint_page` 载体页，parity 注册表逐条记录**已证明的交互路径**与**仍缺的路径**（缺路径的业务不计入已认证 scenario）。
  - 待验证：标准/紧凑双视口真实像素回执随统一截图批次产出；缺交互路径的业务在补齐前保持 `pending`。
  - 本机可执行动作：跑统一验证批次与统一截图批次。
- [ ] TOKEN-001.2 [kind: mechanism] [priority: P2] [points: 2] [owner: infiltrator-contract::design_tokens + 三大 UI owner] [status: Integrated] 圆角阶梯「居中对话框」角色重叠需契约裁决
  - 产物：契约圆角阶梯里 `MODAL`(24) 与 `DIALOG`(28) 的角色重叠由 owner 裁决为互斥可判定，每档只占一个角色。
  - 依赖：TOKEN-001.1 的阶梯裁决（既有档位足够，角色重叠需澄清）。
  - 验收：契约 `design_tokens::radius` 的每档角色注释互斥且可判定；三个表面按契约改齐；清零搜索确认无「居中对话框用 CARD」的残留。
  - 当前证据（L）：契约裁决三个互斥角色并写进阶梯文档的表格——**居中模态与居中命令面板 = `MODAL`(24)、面板式对话框卡片 = `CARD`(16)、移动端底部抽屉上沿 = `BOTTOM_SHEET`(28)**；与 `BOTTOM_SHEET` 同值且角色重叠的 `DIALOG`(28) 档退役，契约测试新增「每个角色只占一档」断言；widgets 的镜像阶梯同步退役 `DIALOG` 与与契约冲突的表面私有 `SHEET_TOP`(16)，镜像测试改为**逐档对照两套阶梯**（原来含 `assert_eq!(radius::DIALOG, radius::DIALOG)` 这类同侧自证）。
  - 当前证据：调色板新增 `modal_radius_px`（绑定 `radius::MODAL`）供居中对话框取用，面板式表面保留 `card_radius_px`；共享 modal 卡片改取 `radius::MODAL`（它是居中模态），Iced 侧共享弹窗 chrome、聚合器弹窗与快照差异弹窗改用 `R_MODAL`，`adaptive_modal` 的居中形态取 `MODAL`、抽屉形态取 `BOTTOM_SHEET`；清零搜索确认 Iced 居中弹窗已无 `R_CARD` 残留。
  - 待验证：Bevy 产品页的居中弹窗卡片（`proxy_inspection`／`proxy_probe_settings`／`proxy_group_order`／`rules_tracer_confirm`／`snapshot_restore_scene`／`connections_confirm`／`dns_*`／`logs_export_scene`／`profiles_script_scene`／`rules_statistics_scene`／`overview_speedtest` 等）仍在用 `palette.card_radius_px`(16)，需改为 `palette.modal_radius_px`——这些文件由 Bevy 模态迁移流在改，改齐后由该流补断言。
  - 本机可执行动作：跑统一验证批次与统一截图批次。

## 十六、真实引擎测速（已交付，1条 / 2pt）

- [ ] PUB-006.13 [kind: mechanism] [priority: P1] [points: 2] [owner: singbox-driver] [status: Integrated] sing-box 驱动的节点测速 typed port 未实现
  - 产物：`singbox-driver` 的 typed `OutboundPort::probe` 真正实现节点测速并回报 `OutboundProbeState` 的实测值，而不是 `Unsupported`。
  - 依赖：PUB-002.3、PUB-006.8（真实二进制取证暴露的缺口）。
  - 验收：真实 sing-box 二进制下测速返回实测延迟（typed 状态而非字符串），失败/不支持/超时各有自己的 typed 结局，驱动与真实引擎的语义一致。
  - 当前证据（L）：驱动向引擎的 delay 端点要一次真实测速并回报实测延迟，每种回答各有自己的 typed 结局：实测值、引擎自己的超时、按状态码的拒绝、端点不提供该能力（不支持）、不是测量的响应体、驱动自己的截止时间；端口级拒绝（陈旧 revision、外来引擎、不在已观测清单里的出站、端点承载不了的超时毫秒数）仍在发请求之前就拒绝。预算语义与 mihomo 驱动一致（引擎执行测速预算，驱动的截止时间是迟到的唯一权威，驱动从不重试）。
  - 当前证据：新增 11 条驱动行为用例（请求串与 URL 编码、引擎超时、失败不是测量、未知出站、端点不提供该能力、响应体不是测量、控制器不答复即超时（暂停时钟）、传输失败、端点承载不了的超时、陈旧 revision 与清单外的出站在请求前被拒）与 7 条分类单测；**引擎语义是对着真实二进制复现的**（`200 {"delay": N}`、`400`、`404` 带引擎错误体、纯文本 `404 page not found`、`503`、`504`、`405`、百分号编码的 tag）。
  - 当前证据：sing-box 的 conformance 用例改为期望**实测**结果并改用 HTTPS 目标（引擎只测 HTTPS：http 目标会被引擎悄悄换成它自己的 URL 测试目标），驱动侧套件 42/42、mihomo 侧 136/136 全绿。
  - 待验证：真实二进制回执（`bash scripts/core-backend-smoke-singbox.sh --driver-evidence-only`）需在收口批次跑一次，回执的 `node_speed_test` 记录应从 `unsupported_proof` 变为已证明；本机可达该 HTTPS 目标时才能拿到实测值。
  - 本机可执行动作：跑统一验证批次与真实二进制 smoke。

## 十七、登记说明

- 2026-10-11 之前的历史切片回执（run ID、APK SHA、模拟器记录）已在 [TODO.md](../TODO.md) 归档表与 `.evidence/` 永久留存；本文件只持未办结验证项。
- 已验证退出项不在本文件重复登记。
- 环境阻塞条目统一在[斗战胜佛](CODE_TODO_BLOCKED.md)；本板不再持有「下一个动作无法在本机执行」的条目。

## 十、跨表面页面模型与像素认证（2026-10-21 批次交付）

- [ ] PARITY-001.2 [kind: evidence] [priority: P0] [points: 5] [owner: scripts/parity + 三大 UI] [status: Integrated] 三大桌面 UI 全业务双视口像素认证闭环
  - 产物：59 业务 × 3 表面 × standard/compact 双视口的真实像素回执全量归档，覆盖率 100%。
  - 依赖：PARITY-001.1。
  - 验收：`visual_receipts.py` 输出的待回执单元格全部转绿，无「以静态页面截图冒充平权通过」。
  - 当前证据：黑盒像素差判据进 `scripts/parity/interaction_delta.py`（未激活基线与激活截图在交互区内做像素差，噪声地板由两次基线差测得）；批量验收 `scripts/parity/accept_surface_interactions.py` 逐格给出 certified/uncertified 与原因，编译失败的包不再被记成「通过」；Iced 与 Bevy 的 demo/捕获层已按业务补齐真实交互（Iced 的 5 个 panic 根因是共享能力门未发布能力矩阵，Bevy 的 4 个 panic 根因是捕获应用无能力矩阵，另有代理组折叠、代理搜索高亮、规则搜索、聚合向导、删除确认、概览卡片重排、路由意图 control 与 authentication 等真实交互）。
  - 待验证：① 全量重跑 `python3 scripts/parity/accept_surface_interactions.py --all --surfaces iced,bevy,slint`，把 certified 格按 PNG 摘要与原生 bounds 登记为 `ready`；② 仍无载体或缺可滚动身份的业务逐条登记（缺的是业务自己的交互区域或 owner 端口，不是「路由到页面」）；③ Android 三 flavor 的实际交互截图回执随取证批次产出。

- [ ] GAP-4.1 [kind: evidence] [priority: P1] [points: 3] [owner: slint] [status: Integrated] Slint 功能验收与 parity
  - 产物：Slint 承载业务的交互路径覆盖与 parity cell 证据（含 `capture-slint-matrix.sh` 场景结果）。
  - 依赖：SLINT-001.1、PARITY-001.1。
  - 验收：每条承载业务的目录路径全部有行为测试证明，scenario 级锚点与双视口像素回执齐备；只派发命令不算已认证 scenario。
  - 当前证据：18 条承载业务的交互路径全部补齐（42 个新行为测试：DNS 泄漏/查询/缓存冲刷/hosts 编辑、脚本沙箱与导出评审、差异视图、快照恢复、过滤编辑、聚合向导与预览、诊断失败恢复、生命周期恢复、同步冲突与恢复、应用路由策略）；demo fixture 现已发布全部 workbench 读模型（此前每个工作台截图都是空面板）；共享 workbench 补上在途台账与 typed 终态（`Workbenches::settle`），owner 的失败/重试/取消路径第一次真正可达。
  - 待验证：① 按 18 条业务重跑 Slint 场景矩阵并把像素回执登记进 `scripts/parity/feature_evidence.tsv` 的 scenario 级锚点；② Android host 侧迁移到 `execute_typed`（当前 Android 仍只发 `CommandOutput::Unit`，带 identity 的评审类业务在 Android 上只能到「已派发」）；③ 四条被收窄含义的路径（泄漏探测的真实 echo 源、冲突备份文件的删除、读取应用路由策略的命令、过滤器的重命名/高级策略字段）要么补 owner 能力，要么在目录里显式写明「该表面不具备」。

- [ ] GAP-7.1 [kind: mechanism] [priority: P1] [points: 5] [owner: presentation] [status: Integrated] 跨 UI 页面模型去重
  - 产物：Iced/Bevy/Slint 共享的页面模型层，消灭各自实现的同义状态机。
  - 依赖：GAP-5.2 完成后评估。
  - 验收：同一页面在三个表面只保留一份状态推导；UI 只做渲染与事件投递；旧符号清零搜索为空。
  - 当前证据：共享层新增九个页面模型（代理/概览/设置/规则/订阅/诊断/DNS/应用路由/同步）与两个共用基座（`OperationPhase` 带提交令牌，被取代的答复不移动任何状态；`EditableDraft` 的「已修改」由草稿与基线推导，不再是 flag）。三端删除各自的私有推导：Iced 的 `member_matches`/`sort_members`/`highlight_runs`/`format_protocol_chip`、四份契约快照副本与两个 `*_dirty` bool、八个规则阶段机、十个订阅阶段机、`DoctorPanelOperation`、`DnsTab`/`AdvancedEditMode`、`UwpLoopbackState`；Bevy 的 `matches_proxy_filter`/`name_runs`/`format_protocol_chip`/`sort_outbound_nodes` 与私有 `OverviewProjection`；Slint 的私有过滤与排序、`route_mode`/`network_mode`/`network_mode_status`/`network_mode_warning`。共享层裁决的差异一次写定：过滤规则取三端并集（名称/身份/协议与别名/feature 标签/UDP/延迟/拼音）、收藏恒排最前、未知延迟恒排最后。批次证据：全仓 nextest **5534/5534 通过（15 skipped）**、`check-structure.sh` **exit 0**、`cargo fmt --all --check` 通过。
  - 待验证：① 共享模型仍缺的契约事实（LAN 共享快照、PAC `bypass_lan`/`minify` 读回、规则页过滤 API、应用路由「未观测」文案键、闭合连接 `ConnectionView`）按现在佛 `GAP-7.2` 收口；② 三端各自的「无消费者」字段（Slint 的设置与订阅页无载体、Bevy 的应用路由无过滤控件、Iced 的设置文本缓冲仍在 widget）随 `GAP-7.2` 逐条清零；③ 共享模型的阶段机与草稿在三端的失败/取消/恢复路径上各补一条端到端行为测试。
- [x] GAP-7.2 [kind: mechanism] [priority: P1] [points: 3] [owner: infiltrator-contract + infiltrator-presentation] 页面模型最后一公里的契约缺口
  - 产物：共享页面模型需要的契约事实由契约层补齐，三端不再各留一份降级事实或私有过滤。
  - 验收：契约有字段或读回，共享模型有消费面，三端的降级分支清零搜索为空。
  - 当前证据：① **LAN 共享读回成为一份事实**——契约的 `RuntimeControlSnapshot` 与 `SettingsPageSnapshot` 用 `lan_sharing: Option<LanSharingSnapshot>` 取代三个必须互相吻合的独立 `Option`（`None` = 未观测，不是「关闭」），共享层新增 typed `LanSharingObservation` 与 `observe_settings_page` 折叠；Iced、Bevy、Slint 三端的私有合成（`revision: 0`）全部删除。② **PAC 读回能回环请求**——`PacSnapshot` 增 `bypass_lan`/`minify`（`#[serde(default)]`），共享草稿读 owner 的值，Iced 写路径不再硬编码 `bypass_lan: true, minify: false`。③ **规则列表过滤归共享层**——`FilterQuery` 成为每个页面共用的 typed 查询，规则页过滤进 `RulesPageModel`，三端的私有字符串过滤删除。④ **应用路由「未观测」有文案**——`app_routing_mode_unobserved` / `app_routing_unobserved_summary` 两种语言齐备，模型报 `Option<AppRoutingMode>`，三端不再回落到 `ProxyAll`。⑤ **闭合连接并入共享模型**——`infiltrator-domain` 补 `ConnectionView` 实现，闭合历史按共享模型过滤、排序与分组。
  - 批次证据：全仓 `cargo nextest run --workspace --no-fail-fast --features infiltrator-iced/capture-tools,infiltrator-bevy-ui/capture-tools` **全绿**、`bash scripts/quality/check-structure.sh` **exit 0**、`cargo fmt --all --check` 通过；三端的清零搜索（`allow_lan`/`lan_bind_address`/`observed_lan_sharing`/`filter_rule_indices`/`ProxyFilterQuery`/`revision: 0`）全部为空。


- [ ] BEVYUI-007 [kind: visual] [priority: P1] [points: 3] [owner: infiltrator-bevy-ui] [status: Integrated] DNS、自愈诊断与设置页交互收口（#95~#105）
  - 产物：解决 #95 DNS 页面顶部 Query DNS 灰色长条二义、#96 跨源探测报警红字粗暴裸奔缺乏容器、#97 DNS 解析调试格式简陋、#98 DNS 配置行 Enabled 文本与开关双重冗余、#99 配置项无缝隙斑马条表格感、#100 设置页字体版权数百行全大写法律英文平铺倾倒、#101 语言设置报错单薄缺乏 Banner、#102 语言选择巨大深灰方块死板、#103 Doctor 修复步骤缺少旋转 Spinner、#104 审阅快照恢复弹窗缺乏 Diff 比较器、#105 弹窗取消与恢复按钮分居左右两极视距过远。
  - 依赖：BEVY-020.1，infiltrator-bevy-widgets。
  - 验收：DNS 与设置页双视口像素回执消除法律文本倾倒与红字裸奔，弹窗操作收拢。
  - 计划：将字体许可改为折叠或弹窗展示，重构 DNS 状态指示与弹窗操作按钮组。
  - 待验证范围：统一验证批次（全仓 nextest + 结构门 + fmt + clippy）+ DNS/诊断/设置三页双视口像素回执复核（1180x780 / 720x480）+ #103 转圈指示器的像素回执（缺一个能停在 `DoctorActionPhase::Pending` 的捕获场景，需新增 parity 特性 `DoctorRepairProgress`、fixture 的 pending 开关、捕获 activator 与两个场景行）+ 官方捕获矩阵是否登记 dns/doctor/settings 的紧凑整页场景待定。
  - 证据身份：改后整页捕获 6/6 ok（`.evidence/captures/bevy/20261021T123324Z_..._1880922`）、改前整页 6/6 ok（`20261021T122645Z_..._1854539`）、交互场景 12 中 9 ok（`20261021T121710Z_..._1831009`）；改前/改后像素副本在 `/tmp/opencode/bevyui007/`。
  - 已证明层：#95~#105 共 11 条逐条处置（法律英文整段移出设置页进官方模态、告警进带框告警井、开关行去重复文字、配置行分离成圆角行卡、审阅恢复接 owner 自己的 Myers diff 双栏对照、弹窗操作收拢、语言错误进 typed 相位告警条、诊断转圈按共享 typed 相位显形）。
  - 未证明层：#103 仅有代码无像素回执（pending 相位是瞬态，缺可停在该相位的捕获场景）；`dns-hosts-editor-standard/compact` 与 `doctor-failure-recovery-compact` 三个 failed-marker 为 2026-10-20 基线既有失败，非本条引入；披露按钮文案复用 `common_details`，更贴切的许可协议文案需共享 locale 表新增键（未擅改共享表）。

- [ ] BEVYUI-002 [kind: visual] [priority: P1] [points: 3] [owner: infiltrator-bevy-ui] [status: Integrated] 总览页与地球组件质感提升（#19~#32）
  - 产物：解决 #19 空状态卡片暗哑死板、#20 地球模式切换未走 TabList、#21 副标题字重过轻、#22 贯穿横向分割线生硬、#23 底部 Demo Mode 蓝块失衡、#24 状态圆点缺乏呼吸动画、#25 Follow rules 徽标与按钮重名混淆、#26 提示小框圆角过大、#27 Start probes 次级操作层级过弱、#28 拓扑空状态插画缺失、#29 紧凑模式下横幅被挤出视口、#30 地球未保留骨架屏、#31 卡片内外留白比例失调、#32 卡片圆角规格不统一。
  - 依赖：BEVY-020.1。
  - 验收：总览页双视口像素回执消除蓝块失衡与内容溢出，TabList 接入官方控件。
  - 计划：接入 TabList 替换普通按钮组，重构 Demo Mode 横幅与 Geo Globe 空态卡片。
  - 待验证范围：统一验证批次（全仓 nextest + 结构门 + fmt + clippy）+ 总览/地球双视口像素回执复核（1180x780 dark/light、720x480 dark）+ 官方捕获矩阵是否登记 `overview-page-1180`/`overview-page-720`/`overview-page-light-1180` 三个场景名待定。
  - 证据身份：改后捕获 3/3 ok（`.evidence/captures/bevy/20261021T123803Z_c11e55a3be03_dirty_1904104/`）；改前对照取自 2026-10-20 基线跑批的 `baseline-overview-720x480`（紧凑视口里地球卡占满全屏、hero 零像素可见）与 `baseline-r2-overview-1180x780`。
  - 已证明层：#19~#32 共 14 条逐条处置（空态改线框地球骨架 + typed 原因；地球模式改走共享 owner 的官方 `TabList` 分段控件并只观察官方 `ValueChange`；副标题字重上调；贯穿线改短标；Demo Mode 蓝块压成两行紧凑带；状态圆点按 `MotionPreference` 呼吸并尊重 Reduced 动效；删除 hero 里重复的 Follow rules 药丸；三档圆角各归其位；拓扑空状态补插画与 typed 原因；卡片内外留白改用共享 `surface_scene`）。**#29 的根因**：地球段原被 `Add<PageRoot>` 观察者插成页根第一个子节点，把 hero 顶出视口——改为页面拥有 `GlobeSectionSlot`、地球段自己声明进槽位，页内顺序由页面决定。
  - 未证明层：本条改动的 headless 测试文件按并行协议未编译验证，须由收口流的 `cargo nextest run -p infiltrator-bevy-ui` 覆盖；共享分段控件的滑动药丸比页签宽约 2–3px（百分比定位在 padding box 上），像素级对齐需改 widgets crate，本条未擅改共享文件。

- [ ] BEVYUI-003 [kind: visual] [priority: P1] [points: 3] [owner: infiltrator-bevy-ui] [status: Integrated] 代理选择与节点卡片重构（#33~#52）
  - 产物：解决 #33 顶部操作按钮杂乱、#34 Test All 浅蓝块撕裂、#35 搜索框与 Clear/Retry 挤压、#36 Alive Only 与排序垂直错位、#37 排序标签未走 TabList、#38 Probe settings 文本粘连、#39 Custom Node Editor 栏目条缺乏交互暗示、#40 分组标题技术术语生硬外露、#41 分组折叠上移下移按钮微小、#42 激活节点整卡亮蓝底色遮蔽内容、#43 卡片内图标过密、#44 未激活卡片死灰单调、#45 协议特性标签重复啰嗦、#46 延迟方块锐角嵌套、#47 延迟折线图微小抽象、#48 卡片悬停无位移阴影、#49 卡片网格换行留白断裂、#50 两端横条/卡片形态分裂、#51 Ready to test 状态缺乏指引、#52 长节点名称缺失渐隐截断。
  - 依赖：BEVY-020.1，infiltrator-bevy-widgets::tabs。
  - 验收：代理页双视口像素回执消除高亮遮蔽与拥挤，排序走 TabList。
  - 计划：重构 proxies/scene.rs 与 node 卡片渲染，优化调色板高亮与内联间距。
  - 待验证范围：统一验证批次（全仓 nextest + 结构门 + fmt + clippy）+ 代理页双视口像素回执复核（1180x780 / 720x480）+ 本条改动的 headless 测试按并行协议未编译验证，须由收口流的 `cargo nextest run -p infiltrator-bevy-ui` 覆盖。
  - 证据身份：改后捕获 4/4 ok（`.evidence/captures/bevy/20261021T132353Z_6d39d986b658_dirty_2146509/`）；改前对照取自 `.evidence/runs/20261020T081015Z_175409/bevy-published/proxies-group-expanded-{standard,compact}.png`。量化像素：选中卡片内芯由 `rgb(30,143,245)`（accent 实心，内容被埋）变为 `rgb(83,85,86)`（与其余卡片同一阅读面），accent 收成 1px 边线 + 3px 选中轨（x=286..288）；协议芯片不再裁切，`udp`/`UDP` 只出现一次。
  - 已证明层（#33~#52 共 20 条）：14 条已修——顶部动作并入一个带框组、Test All 改 accent 描边触发、搜索与 Clear/Retry 分组且 Retry 仅在搜索被拒后出现、Alive Only 与排序同轴居中、排序标签走官方 `TabList`/`Tab`/`SelectedTab`/`TabActivation` 并由 owner 已发布顺序驱动、分组标题去技术黑话、协议芯片去重、卡片图标改为"旗标+名字在前、操作在后"独立行、延迟方块圆角分档、上区改卡片式带分组动作条与筛选行、上下移改 28px 命中区的箭头图标板、Probe settings 改带框触发+独立说明、选中卡片不再埋内容、未选中卡片不再是死灰。
  - 未证明层（诚实列出，不计为完成）：#48 悬停位移与阴影**只有代码与单测**（捕获台架无指针，取不到像素）；#52 长名截断已做 grapheme 安全省略，但**像素渐隐**需共享不透明卡片填充色（`UiPalette::card_fill`）；#50/#51 仅部分；**#47 延迟迷你折线图未做**（需 owner 的延迟采样历史进投影）；**#39 Custom Node Editor 栏目条未做**（需共享 `business_panel.rs`）；#49 判定为非页面缺陷（换行网格末行留白属共享 `FluidCardGrid` owner 的平衡问题）。
  - 遗留共享改动需求（转 `BEVYUI-003R`）：`business_panel.rs` 的 `launcher_scene` 加描边与尾部箭头；`infiltrator-shared` locale 表新增四个键（分组类型 Fallback/LoadBalance/Relay + 测速就绪提示）；`infiltrator-bevy-widgets/src/palette.rs` 新增 `card_fill()`；`infiltrator-bevy-ui/src/surface_projection.rs` 的 `ProxyNode` 增加 `latency_samples`、`ProxiesProjection` 增加 `sort_order`；`infiltrator-bevy-widgets/src/tabs.rs` 增加带自定义标记的分段控件入口。
  - 已知风险（须写进共享控件层说明）：卡片填色必须跑在 `Last` 调度——共享皮肤在 `Update` 把每个 `ControlVisual.0 == true` 的控件重涂成实心 accent，若把卡片填色移回 `Update` 会重新引入 #42（"选中控件=实心 accent 填色"与"卡片必须保留阅读面"不可组合）。

- [ ] BEVY-020.1 [kind: mechanism] [priority: P1] [points: 3] [owner: infiltrator-bevy-widgets + infiltrator-bevy-ui] [status: Integrated] Bevy 0.20 未采用能力收口（滚动条 / 滚动入视 / 官方菜单）
  - 产物：① 列表与规则页挂官方 `bevy_ui_widgets::Scrollbar` + `ScrollbarThumb`（滑块与 `ScrollArea` 的滚动位置绑定），替掉"只有一行文字页码"的现状；② 用官方 `ScrollIntoView` 事件替掉 `infiltrator-bevy-widgets/src/list.rs` 里自算偏移的 `scroll_to_item`；③ 菜单层迁回官方 `MenuPopup`/`MenuItem`/`MenuFocusState`/`MenuLayout`，删除本地 `MenuState`/`MenuNavEvent`/`MenuOutcome` 状态机与旧回归路径。
  - 依赖：0.20 官方 `bevy_ui_widgets` 的 `ScrollbarPlugin`、`ScrollIntoView` 观察者、`MenuLayout` 键盘导航已就位（已核对上游 0.20.0 源码）；本地 rem 通道与 token 体系不变（滑块颜色走 `ThemeBackgroundColor`/`ThemeBorderColor`）。
  - 验收：全仓 nextest 全绿；官方滑块可拖动且与滚动位置一致；键盘导航能滚动到聚焦项；菜单的方向键导航与焦点圈闭由官方提供；「Bevy 0.20 UI 能力规范」的「0.20 未采用能力台账」里 SCROLL-01/SCROLL-02/MENU-01 三行清零。
  - 剩余缺口（NAV-01 收口）：官方方向导航已接上并有 7 条测试，但**可见焦点真源仍是本地的**（`crates/infiltrator-bevy-ui/src/shell_focus.rs` 的 `ShellFocusState`/`GamepadFocusController`/本地空间搜索），控件也没有官方 `TabIndex`，所以官方指针焦点够不到按钮、可见焦点环不跟随。收口需要「一个焦点真源」：`shell_focus.rs` 改为从官方 `InputFocus` 派生可见焦点并退役本地空间搜索，控件补官方 `TabIndex`。
  - 推进方式：按文件所有权拆成互不相干的三条流并行推进——官方菜单（`menu.rs`/`context_menu.rs`）、滚动条与滚动入视（`list.rs`）、行内文字与方向导航（`text.rs`/`shell_readout.rs`）；Iced/Slint/Android 三个表面的同类「框架能力自研重造」由第四条流审计并给出整改清单。
  - 事实依据：2026-10-21 在两个生产树上按官方符号名实测——`Scrollbar`/`ScrollbarThumb`/`ScrollbarPlugin` 0 处、`ScrollIntoView` 0 处（官方 `ScrollAreaPlugin` 已注册 `on_scroll_into_view` 观察者，本仓从不触发）、`MenuPopup`/`MenuItem`/`MenuFocusState`/`MenuLayout` 0 处（仅头注释提及）。0.20 官方菜单已带键盘导航与焦点圈闭，本地实现"官方仅指针驱动"的旧理由已失效。
  - 交付实况（2026-10-21）：官方菜单（`MenuButton`/`MenuPopup`/`MenuItem`/`MenuEvent`/`MenuFocusState`，本地菜单状态机与手写视口翻转全删，13 条新测试）、官方滚动（`Scrollbar`+`ScrollbarThumb`+`ScrollbarPlugin`，本地偏移算术全删改官方 `ScrollIntoView`，4 条新测试；跨 crate 断口 `acceptance_list.rs` 已迁到官方事件并把"居中"诚实改成"露出"、locale 键同步改名）、官方行内文字（`InlineBox`+`InFlow`/`OutOfFlow`+rem 通道，富文本改单文本块+官方 per-run 背景芯片）、官方方向导航（`DirectionalNavigationPlugin`+仓库配置，7 条测试）。
  - 待验证范围：统一验证批次（全仓 nextest + 结构门 + fmt + clippy）+ Bevy 双视口像素回执复核 + 菜单/滚动/行内三处官方化的视觉回执重取。
  - 未证明层：NAV-01 只完成一半——官方导航动的是 `InputFocus`，而本仓可见焦点真源仍是本地的（`shell_focus.rs`），控件也没有官方 `TabIndex`，可见焦点环不跟随官方焦点；该缺口转 `BEVY-020.2`。

- [ ] BEVYUI-001 [kind: visual] [priority: P1] [points: 3] [owner: infiltrator-bevy-ui] [status: Integrated] 侧边栏与标题栏外壳表现整改（#1~#18）
  - 产物：解决 #1 路由药丸三行重叠、#2 紧凑模式底部图标裁切、#3 侧边栏电光蓝对比过载、#4 未激活项缺少 Hover 缓动、#5 窗口三金刚键一体化、#6 顶栏 1px 分割线、#7 导航箭头比例、#8 Follow rules 胶囊禁用态二义、#9 系统代理/TUN 卡片拥挤、#10 On 徽标文字垂直对齐、#11 订阅卡片折行基线、#12 数量徽标溢出、#13 流量速率排版、#14 rem 物理对齐抖动、#15 图标文字间距统一、#16 Iced/Bevy 导航架构对齐、#17 侧边栏微阶底色、#18 纯图标模式 Tooltip。
  - 依赖：BEVY-020.1，infiltrator-bevy-widgets。
  - 验收：双视口像素回执（720x480/1180x780）消除重叠与截断，通过 visual_receipts 校验。
  - 计划：重构 shell_scene.rs 与 shell_readout.rs 布局约束，添加 Tooltip 与 Hover 状态。
  - 交付实况：#1~#18 共 18 条中 **16 条已修**——路由段由「一行三丸互压」改为一列三枚自适应 chip（三行 y80–185 互不重叠）；紧凑模式轨道改官方 `ScrollArea` + 32px 行高（720×480 末位图标由切半变完整）；侧栏电光蓝过载收敛（数量徽标改中性 elevated+dim、蓝色「On」药丸换共享开关、卡片面 `(80,81,82)`→`(37,41,44)`）；三金刚键收进同一容器，关闭字形由无字形的「✕」改为有字形的「×」；顶栏补 1px 分割线；历史箭头改 28×28 方钮+人字（墨迹 6px→9px）；Follow rules 胶囊改只读 chip（不再有「可点却点不动」的二义）；订阅卡改 Iced 同构的图标+双行；数量徽标改自适应胶囊；速率改 mono 遥测字形；图标↔文字间距统一 `space::S8`；侧栏形成三级微阶底色；导航顺序与 Iced `view/sidebar.rs` 对齐（身份→路由控制→双开关卡→订阅卡→2×2 磁贴→速率页脚→分隔线→路由行）。
  - 未证明层（诚实列出）：#4 路由行 hover 缓动与 #18 纯图标模式 Tooltip **只有代码路径**——捕获台不注入指针，hover/焦点态无法成像（已按共享动效 owner `Spring` + `HostPreferences::effective_motion()` 实现，`Reduced` 时直接落位）；#14 rem 物理对齐抖动**部分**修复（壳层自身尺寸全部走 rem 通道，残余抖动源是共享图标底板仍是物理 px）；#16 导航架构**结构已对齐但图标仍重复**（共享底板只有 10 个 id）。
  - 像素证据：改后捕获 3/3 ok（`.evidence/captures/bevy/20261021T173243Z_7261c8326541_dirty_3006708/`）；改前对照取自 `20261021T133305Z_8a3c2e16724e_dirty_2187724/`；量测：路由文字带 80–139（三丸互压）→80–185（三行分离）、计数徽标 `#0D3971`+accent 字→`#535556`+dim 字、侧栏卡片面 `(80,81,82)`→`(37,41,44)`。
  - 共享改动需求（转 `BEVYUI-001R`）：S1 `crates/infiltrator-bevy-widgets/src/icon.rs` 增 `icon_scene_val(icon, size: Val, tint)` 让图标盒走 rem 通道；S2 `crates/infiltrator-shared/src/assets.rs` 的 `bevy_icon_png` 增补 `minus/square/x/chevron-left/chevron-right` 与 11 条路由的专属图标（由既有 Lucide 源栅格化），`widgets/src/icon.rs` 的 `IconId` 同步；S3 捕获台增加「指针悬停轨道行」的 capture feature，使 hover 填充与 tooltip 获得真实像素回执。

- [ ] BEVYUI-003R [kind: visual] [priority: P1] [points: 3] [owner: infiltrator-bevy-ui + infiltrator-bevy-widgets + infiltrator-shared] [status: Integrated] 代理页余项与共享改动落地
  - 产物：① #47 延迟迷你折线图（需 owner 的延迟采样历史进投影：`surface_projection.rs` 的 `ProxyNode` 增加 `latency_samples`）；② #39 Custom Node Editor 栏目条加描边与尾部箭头（`business_panel.rs` 的 `launcher_scene`）；③ #52 长名的像素渐隐（需 `palette.rs` 新增不透明 `card_fill()`，否则渐变会二次合成出色带）；④ locale 表新增四个键（分组类型 Fallback/LoadBalance/Relay、测速就绪提示）；⑤ `ProxiesProjection` 增加 `sort_order`，排序条由 owner 顺序播种而非采用备忘；⑥ `widgets/src/tabs.rs` 增加带自定义标记的分段控件入口；⑦ 把"卡片填色必须跑 `Last` 调度"写进共享控件层说明。
  - 依赖：BEVYUI-003 已交付的部分（现在佛已无此条）。
  - 验收：代理页双视口像素回执；#47/#48/#52 的像素证据到位（#48 需带指针的捕获台架，取不到就明确标注为代码级证据）。
  - 事实依据：BEVYUI-003 交付时逐条列出的共享改动需求（文件 + 符号 + 代码片段）。
  - 交付实况：共享层与代理页两处一并落地——`UiPalette::card_fill()`（`surface_elevated` over `surface` 解析成**不透明**色）与 `UiPalette::over_surface()`（在**线性空间**合成，即渲染器混合所处的空间）；`segmented_control_with_marked_labels_scene`（调用方的标记成为所属页签的子节点，且与既有实现共用同一份胶囊/边框逻辑，防止漂移）；四个 locale 键；`ProxyNode.latency_samples` 与 `ProxiesProjection.sort_order` 进入投影，排序条改由 owner 已发布顺序播种。
  - 缺陷处置：#39 栏目条加发丝描边 + `column_gap` + 尾部暗色 `›`（字形真实渲染，非豆腐）；**#47 迷你折线图**改画 owner 自己的历史（未测量的节点没有线，不造数据），📈 emoji 已删；**#52 长名像素渐隐**在卡片**自身不透明填色**上绘制，绝不二次合成；#48 hover/press 渐隐从卡片实时 `BackgroundColor` 于 `Last` 重贴（在 `sync_proxy_card_visuals` 之后），随静止/悬停/按下/禁用四态跟随；分组类型文案改用新键（英文面不再漏中文）。
  - 像素证据：改后捕获 4/4 ok（`.evidence/captures/bevy/20261021T173059Z_7261c8326541_dirty_3002393/`，dark+light × 1180×780/720×480）；改前对照 `20261021T133305Z_8a3c2e16724e_dirty_2187724/`。**渐隐无接缝的数值证明**：卡片底 `(83,85,86)`，渐隐区最暗像素 **83**（完全一致 → 无缝）；首版在 sRGB 空间合成时产生 30 级暗带（最暗 53 vs 卡片 83，见 `20261021T170433Z_..._2919115/proxies-dark/image.png`），改为线性空间合成后消除（实测 `over(7% 白, 37 sRGB) = 83` 线性 / `52` sRGB）。
  - 未证明层：#48 悬停/按下**仅代码级**（捕获台无指针）；`proxies_test_ready_hint` 已加键但**未被可见面消费**（会让页头状态在每个表面变长，属文案与布局决定，留给收口流，未擅自改页头）；带标记分段控件已作为共享能力交付并有 headless 测试，但代理排序条**仍自建**（它需要固定 440px 宽与 25% 页签宽，共享条不提供），改接会改几何与页签查找，故证明 API 而不半接线。
  - 依赖提示：`BEVYUI-001R` 的共享层需求（图标 rem 通道、专属图标资产）与本批**必须合并成一次落地**。

- [ ] BEVYUI-006 [kind: visual] [priority: P1] [points: 3] [owner: infiltrator-bevy-ui] [status: Integrated] 实时日志与控制台流式重构（#83~#94）
  - 产物：解决 #83 日志条目整行大圆角胶囊堆叠臃肿、#84 时间戳未等宽且颜色过亮、#85 日志级别标签缺乏底衬被底灰冲淡、#86 模块标签方括号间距松散、#87 级别过滤栏未走 TabList、#88 控制台顶部按钮主次不分、#89 正则输入框 Placeholder 对比度过低、#90 长日志无软折行开关撑破卡片、#91 代码字体行高偏小易串行、#92 刷新追加缺乏渐隐指示、#93 缺少行号与单行快速复制、#94 紧凑模式下日志卡片右侧冲出屏幕。
  - 依赖：BEVY-020.1，infiltrator-bevy-widgets。
  - 验收：日志页双视口像素回执呈现流式控制台质感，消除胶囊卡片堆叠。
  - 计划：重构 logs_rows.rs 与 logs_virtual.rs，采用单行平铺流与 InlineBox 排版。
  - 交付实况：#83~#94 共 12 条**全部已修**——日志行去圆角胶囊改为扁平发丝行带（行区底色=卡片面 `(37,41,44)`，旧为浅灰 `(83,85,86)`）；时间戳进 mono 段落且用暗墨 `(69,72,75)`；级别徽章是带官方 per-run `TextBackgroundColor` 的 `TextSpan`（level 色 @0.18，如 INFO `(36,73,119)`）；模块标签进入同一段落（`[TCP] 127.0.0.1:54120` 只一个空格）；级别过滤改官方 `TabList`/`Tab`/`SelectedTab`（`TabActivation::Automatic`，唯一真源=官方 `SelectedTab`）；顶部按钮分 Ghost/Secondary/Outline 三级；搜索框底色改与页面同面，placeholder 对比度 **4.34:1 → 7.91:1**（实测）；长记录在列内折行且可切单行；控制台行高统一 `LineHeight::RelativeToFont(1.45)`；新记录到达时实况条 0.85 alpha 亮起并 2.5s 渐隐回常态（typed `StreamAppendPulse{Idle,Fading}`，无 bool 组合）；每行有右对齐行号与行尾「复制」按钮（走 `ClipboardHost` 裁剪板缝 + typed toast）；紧凑视口行不再横向溢出。
  - **与计划的一处有意偏离**：日志行实现为**两个文本块**（meta rail 段落 + message 段落），而非计划里的单块。原因是共享交互捕获 `interaction_capture/logs.rs` 以 `LogMessageText` 实体的首 run 与 accent 墨色作为"高亮已生效"的观测事实；把 meta 并进同一段会让 message 移出根 `Text`，`logs-search-highlight-*` 场景会直接失败。徽章仍是 rail 段落内带官方 per-run 背景芯片的 `TextSpan`。
  - 像素证据：改后捕获 **8/8 ok**（含交互场景 `logs-search-highlight-*` 仍能观测高亮文本与 accent 墨色）；改前对照 `20261021T115155Z_..._1705499` / `20261021T133305Z_..._2187724`；量测：行底色 `(37,41,44)`、徽章 `(36,73,119)`、placeholder 对比度 4.34→7.91。
  - 未证明层：软折行开关的**可见文案**缺本地化键（当前只有无文字开关 + 英文 a11y label）；追加指示条的**亮起→渐隐过程**未进截图（捕获在 60 帧就绪后拍摄，2.5s 渐隐已结束），像素证据只有常态实况条。
  - 共享改动需求（并入 `BEVYUI-001R` 那一批）：`infiltrator-shared/src/locales_table_ext.rs` 与 `locales_table_en_ext.rs` 各加 `logs_wrap_soft`（自动换行/Soft wrap）与 `logs_wrap_single`（单行显示/Single line），随后 `pages/logs.rs` 的软折行开关旁挂 `LocalizedText::plain("logs_wrap_soft")`（i18n 守卫禁止引用不存在的键，故本切片未加）。

- [ ] BEVY-020.2 [kind: mechanism] [priority: P1] [points: 2] [owner: infiltrator-bevy-ui] [status: Integrated] 可见焦点归一：官方 `InputFocus` 成为唯一焦点真源
  - 产物：`crates/infiltrator-bevy-ui/src/shell_focus.rs` 的 `ShellFocusState`/`GamepadFocusController`/`apply_shell_focus_marker`/`apply_shell_focus_ring`/`advance_shell_focus_by_keyboard` 改为从官方 `bevy_input_focus::InputFocus` 派生可见焦点，退役本地空间搜索 `crate::focus::find_spatial_neighbor`；控件补官方 `TabIndex`（widgets 插件的必需组件）使官方指针/标签焦点可达；非按钮的 `ShellFocusable` 补 `AutoDirectionalNavigation`。
  - 依赖：BEVY-020.1 已交付的官方方向导航（手势已存在，只差焦点真源归一）。
  - 验收：方向键移动焦点时可见焦点环跟随官方焦点；本地空间搜索与其回归路径删除；全仓 nextest 全绿。
  - 事实依据：2026-10-21 官方方向导航接线时实测——官方导航确实移动 `InputFocus`，但本仓可见焦点是本地的、控件无官方 `TabIndex`，官方焦点在壳层里是空的，故可见环不跟随。
  - 交付实况：**官方 `InputFocus` 成为唯一焦点真源**。`shell_focus.rs` 重写——`ShellFocusState` 改为每帧由官方焦点**派生**（不再有手势写它），新增 `ShellFocusCause`（读官方 `FocusGained` 的 cause）、`ShellFocusOrder`/`ShellFocusTarget`（只说"下一个是谁"，不说"谁被聚焦"，并排除官方 `ModalDialog` 子树）、`land_shell_focus`（官方焦点落到首个可测控件的自动基线，绝不把焦点从打开的对话框里抢走）；方向键/D-pad/摇杆经官方 `AutoDirectionalNavigator` 移动**官方**焦点；Tab/Shift+Tab 走壳层顺序后**交还给官方焦点**；焦点环与标记由官方焦点+官方 cause 派生（用户驱动的焦点才显示环，自动基线不显示）。
  - 已退役的本地符号（删除而非别名）：`infiltrator_bevy_widgets::focus::find_spatial_neighbor`（本地空间搜索）及其词汇 `FocusDirection`/`FocusMoveEvent`；`focus::Focusable`/`FocusManager`/`sync_focus_rings`（**影子焦点账本**：手写 `tab_index`/`is_enabled` 与第二份 `focused_entity`）；`gamepad_ui::GamepadFocusController`/`GamepadFocusTarget`（第二份焦点真源）；`GamepadDirection::focus_direction()` 改用官方词汇 `compass_octant()`；被退役搜索的回归路径 `test_spatial_focus_navigation` 一并删除。
  - 控件补官方 `TabIndex`（官方指针/标签焦点可达），非按钮的 `ShellFocusable` 与按钮同样带官方 `AutoDirectionalNavigation`；焦点模式仍是 typed `ShellFocusState::{NoFocus,Focused}` 与 `ShellFocusCause`，无 bool 组合。
  - 测试：新增 `shell_tests/keyboard_focus_tests.rs`(6) 与 `gamepad_focus_tests.rs`(8) 加模块内 5 条——箭头手势移动官方焦点**且焦点环跟随**、Tab/Shift+Tab 移动官方焦点、纯路由基线保持不可见（自动 cause 无环）、控件带 `TabIndex`+`AutoDirectionalNavigation`、官方 `AcquireFocus` 请求能到达带 `TabIndex` 的控件、D-pad 经官方导航器移动焦点、摇杆尊重死区、环绕回退到行首/行尾、阅读顺序、确认/取消映射到官方焦点与返回词汇。实测 `cargo nextest run -p infiltrator-bevy-ui --features capture-tools -E 'test(focus)'` → **33/33 通过**；`cargo nextest run -p infiltrator-bevy-widgets` → **446/446 通过**。
  - 像素证据与其边界：head 与补丁版捕获差异仅限 `(273,121)-(299,140)` 的 331 个子像素（0.0123%），且**同一构建两次捕获在同一区域也差 0.0095%** —— 那是概览 hero 呼吸光晕的动画相位，不是产品改动；两个视口与模态场景除该光晕外像素一致。**焦点环跟随焦点未被像素证明**（捕获台不发指针/键盘），证据是 headless 行为测试。
  - 未证明层：官方**自动**空间搜索无法 headless 演练（`ComputedUiTargetCamera` 的相机字段 crate 私有），headless 证明走的是官方**手动**导航图 `DirectionalNavigationMap::add_edge`（官方声明边的通道），自动搜索只在真实应用里跑；D-pad 环绕保留为回退（官方导航器无环绕），且 D-pad 接缝仍无产品宿主（除测试外无人写 `ShellFocusNav`）。
  - 共享改动需求（并入 `BEVYUI-001R` 那一批）：① `crates/infiltrator-bevy-widgets/src/lib.rs` 里由**控件层**拥有按钮的官方焦点标记 `app.register_required_components::<Button, TabIndex>()`（现由壳层插件注册，owner 位置不对）；② 能力规范文档的 `NAV-01` 行仍写"0 处"（BEVY-020.1 已接上，需改口）；③ `InputDispatchPlugin` 未装进壳层应用 → 官方"键盘→聚焦实体"的派发（聚焦按钮上的 Enter/Space）到不了控件，是既有缺口但现在官方焦点真落在控件上就显眼了，是否安装属产品决定。
  - 行为变更（须知会）：指针按下现在会把官方焦点移到控件上（控件带 `TabIndex`），壳层焦点环因此会出现在被点控件上——此前官方指针焦点根本够不到控件；焦点回到基线控件时环也会出现（cause 是用户驱动），此前只要焦点停在首个控件环就隐藏；焦点环场景改在 `PostUpdate`（官方焦点变更事件之后）生成，布局落位比原来晚约一帧；壳层的焦点顺序与环排除官方 `ModalDialog` 子树（模态在自己的焦点圈闭里拥有焦点，这也让模态捕获场景像素不变）。

- [ ] BEVYUI-004 [kind: visual] [priority: P1] [points: 3] [owner: infiltrator-bevy-ui] [status: Integrated] 规则系统与分段标签收口（#53~#68）
  - 产物：解决 #53 头部堆砌技术日志长文本、#54 辅助操作按钮体积过大、#55 Rules 分段标签切换生硬、#56 Retained draft for 悬挂句号、#57 草稿保存丢弃禁用态对比度过低、#58 分页工具栏插在搜索栏右侧压缩空间、#59 长规则列表缺失滚动条退化分页、#60 规则行圆角梯级白线框杂乱、#61 规则序号未等宽对齐、#62 匹配类型徽标色彩单调、#63 目标动作 DIRECT/REJECT 缺乏语义色彩、#64 Toggle rule 样式割裂、#65 重排序箭头蓝底黑图对比度反转、#66 命中统计与状态纯文本混杂、#67 编辑抽屉无渐入动效、#68 JSON Editor 缺乏代码高亮与行号槽。
  - 依赖：BEVY-020.1，infiltrator-bevy-widgets。
  - 验收：消除长文本堆砌，规则列表接入官方 Scrollbar，双视口像素回执通过。
  - 计划：重构 rules_tabs.rs 与 rules_view.rs，将分页工具栏移出并接入滚动条。
  - 交付实况：#53~#68 共 16 条中 **12 条已修**——头部去掉重复的命中审计行与技术日志（4 行 y145..205 → 2 行 y137..165）；头部动作改共享按钮 `ButtonSize::Sm`；分段条改 `segmented_control_with_labels_scene`（官方 `TabList`/`Tab`/`SelectedTab` + 滑动胶囊，页面写官方 `SelectedTab`）；搜索框独占整行、分页行下移右对齐；规则表接**官方滚动条**并把滑块改成看得见的 token（x=1140 处旧 `(79,79,80)` → 新 `(83,88,94)`）、视口限定 320px、页码游标由视口偏移**派生**（无第二份偏移）；规则行不再是 `SurfacePanel` 卡（无圆角无框，只有发丝分隔线）、序号改 `Role::Mono` 固定 40px 右对齐列；匹配类型徽章按族取色（Host=accent/Geo=success/Address=warning/Process=danger）、目标动作取语义色（DIRECT=success `(61,199,112)`、REJECT=danger）；行开关改**共享开关**、重排序手柄改中性图标（蓝底蓝图的 accent@0.62 方块已删）；JSON 分区编辑器有行号槽 + JSON 词法着色（key=accent/string=success/number=warning/标点=dim）。
  - **额外修掉一条清单外的更严重问题**：规则行原本会溢出卡片并压在统计卡上（改前 PNG 的 y=540..740 可见），由页根 `flex_shrink: 0` 列 + 有界视口修掉——布局不再把卡片挤到内容逃逸。
  - 像素证据：官方矩阵 2/2 ok（`.evidence/captures/bevy/20261021T192331Z_12c608ff7853_dirty_3469493/`）；双视口自定矩阵 4/4 ok（`20261021T192700Z_..._3488831/`）；改前对照 `20261021T115155Z_..._1705499`。
  - 未证明层（诚实列出）：#56 悬垂句号**部分**（空档案名时共享投影的 `editor_status` 文案缺"未命名"变体）；#57 禁用态对比度**部分**（动作已分 Primary/Default/Ghost 三级，但禁用墨仍是共享 token ≈3:1）；#66 命中统计与状态**部分**（已独立成芯片，但计数与状态仍是同一串共享文案）；#67 抽屉渐入**仅本簇可开的面板**（JSON 分区编辑器有真实过渡：typed `Closed/Opening/Open/Closing` + typed `transition(request)` + 穷尽匹配，高度和填色随 0.2s 进度；工作区本无抽屉形态面，共享抽屉控件缺过渡驱动）；**#68 JSON 分区无像素回执**（契约无该 `FeatureId`，矩阵只开列表分区）；`rules-tracer-drawer` 捕获仍 failed-marker（矩阵无该场景行且 tracer 分区在折叠线下；已补 `on_rules_tab_chip_activated` 这条捕获需要的 `Activate` 接缝，但几何门在非本簇文件里，仍是阻塞）。
  - 共享改动需求（并入 `BEVYUI-001R` 那一批，共 8 项）：① `presentation/src/rule_list_projection.rs::editor_status` 空档案名时改用"未命名"文案键 + 两条 locale；② `widgets/src/theme.rs` 的 `disabled_ink` alpha 0.35→0.55（约 3:1→4.9:1）且 `button.rs` 禁用主按钮仍读作主按钮；③ **根因**：`widgets/src/modern_surface.wesl` 边框 alpha 被丢弃，10% 发丝线画成了不透明的近白框（全应用每张卡都受影响）→ 边框色须乘自己的 alpha；④ `widgets/src/list/scrollbar.rs` 滑块 7% 白在暗卡上不可见（改 `Token::DisabledInk`），且 `list_scroll_frame` 应把滚动条作为参数以便页面画自己的列表；⑤ `presentation/src/rule_row_projection.rs` 拆出 `RuleRowHits`/`RuleRowStatus` 让表面把计数与状态画成两个芯片（+ 三个 locale 键）；⑥ `widgets/src/editor.rs::code_editor_scene` 加按 token 种类的墨色（YAML 编辑器与规则 JSON 视图同等待遇）；⑦ `widgets/src/drawer.rs` 的 `DrawerState.slide_progress` 从未动画、面板静态摆放，全应用没有抽屉能渐入；⑧ `scripts/capture_bevy_scenarios.tsv` 补规则页双视口四行（**注意命名**：捕获名不得以 `-standard`/`-compact` 结尾，除非那真是 `FeatureId` id，`rules-dark-standard` 会 panic 报"unknown or unimplemented interaction capture"）。
