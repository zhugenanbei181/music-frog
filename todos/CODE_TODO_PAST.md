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
  - 当前证据（L）：清零搜索 `TRACK_WIDTH_PX|TRACK_HEIGHT_PX|KNOB_OFFSET_PX` 在 `infiltrator-bevy-ui/src` 无匹配；`switch_tests.rs` 4 条新用例（轨道胶囊几何、typed 开/关偏移、typed 决策、父子组合）；`bevy-widgets` nextest 419/419、`bevy-ui` 716/716 全绿；rem/bsn/line 守卫 0 违例。
  - 待验证：真实像素回执（开关两态的轨道与旋钮）随本批截图批次。
  - 本机可执行动作：跑统一截图批次。
- [ ] BEVYCAP-004.2 [kind: mechanism] [priority: P1] [points: 3] [owner: infiltrator-bevy-widgets::modal + infiltrator-bevy-ui] [status: Integrated] modal 层迁到官方 `Dialog`/`ModalDialog`/`RequestClose` 并把 bool 对改 typed stage
  - 产物：能力规范⑥收口——共享 modal 场景挂载官方 `ModalDialog`/`ModalDialogBarrier` 与 `DialogPlugin`/`ModalDialogPlugin`，barrier 点击外关闭、Esc 拦截、焦点圈闭与 `DialogStack` 全部由 Bevy 提供，本地不再手写；业务状态保留在产品侧但改为**一个 typed stage** `ModalStage{Hidden, Open, Dismissed}`（`is_open()` 为派生方法，旧的 `is_open`/`dismiss_on_scrim` bool 对消失，不再能表达非法组合）。
  - 依赖：Bevy 0.20 `bevy::ui_widgets` 官方 `Dialog`/`ModalDialog`/`RequestClose`/`DialogStack`；`menu` 层为已评审的本地例外，不在本条范围。
  - 验收：弹窗行为 headless 回归（开/关/取消/确认/焦点圈闭）全绿；`implicit-state-audit` 判决台账同步（旧的 `ModalState::multi-bool` 判决已撤销）。
  - 当前证据（L）：`bevy-widgets` nextest 419/419、`bevy-ui` nextest 716/716 全绿；`implicit-state-audit` 报 22 候选 / 0 未判决 / 0 open / 0 stale（撤销旧判决后）。
  - 待验证：真实交互与像素回执（官方 barrier/Esc/焦点圈闭的用户可见证据）随本批截图批次。
  - 本机可执行动作：跑统一截图批次。

## 八、Android 静态质量门（已交付，4条 / 9pt）

- [ ] BANDROID-044.1 [kind: infra] [priority: P1] [points: 2] [owner: android gradle] [status: Integrated] Android Lint 接入
  - 产物：Compose/Bevy/Slint Debug 变体注册真实 lint task（`androidLintGate`），警告即错误（`warningsAsErrors = true`、`abortOnError = true`），不读 baseline 文件、不把 assemble 当 lint 结果；lint task 跑在 PR/push workflow 与本机入口里。
  - 依赖：BANDROID-042/043 存量违规清理。
  - 验收：lint 结果分项报告，违规清零后强制生效。
  - 当前证据（L）：提交 `faa013c`；本机实测 lint 68 条（4 errors + 64 warnings）→ 0；四条有据可查的窄豁免写在 `android/app/lint.xml` 并由架构红线守卫逐条钉住；`android/app/lint.xml` 之外无任何 suppression。
  - 待验证：受控 CI runner 的真实 lint 回执（CI 腿登记在[斗战胜佛](CODE_TODO_BLOCKED.md) §二）。
  - 本机可执行动作：跑统一验证批次 `scripts/quality/android-quality-gates.sh`。

- [ ] BANDROID-044.2 [kind: infra] [priority: P1] [points: 2] [owner: android gradle + scripts/quality] [status: Integrated] ktlint/Detekt 门
  - 产物：ktlint 与 Detekt 接入并强制；无永久 baseline、文件级 suppression 或抬高阈值。
  - 依赖：BANDROID-044.1。
  - 验收：违规清零后强制生效；正反例可复现。
  - 当前证据（L）：提交 `faa013c`；ktlint 5046 → 0（以仓库自身代码风格为声明风格，作用域限定 Gradle 脚本）；Detekt 项目配置 0 条（stock profile 的 769 条不作数），每条 exclude 都写明由哪道机器门负责该关注点；无 baseline 文件、无文件级 suppression、无抬高阈值。
  - 待验证：受控 CI runner 的真实 ktlint/Detekt 回执（CI 腿登记在[斗战胜佛](CODE_TODO_BLOCKED.md) §二）。
  - 本机可执行动作：跑统一验证批次 `scripts/quality/android-quality-gates.sh`。

- [ ] BANDROID-044.4 [kind: infra] [priority: P1] [points: 3] [owner: scripts/quality] [status: Integrated] 架构红线守卫
  - 产物：禁止生产 readiness sleep、主线程阻塞、无界 GlobalScope、空 catch、吞取消、`!!` 与多状态 Boolean wire。
  - 依赖：BANDROID-044.2。
  - 验收：窄例外有局部说明与行为回归；正反例通过。
  - 当前证据（L）：提交 `faa013c`；新增 `scripts/quality/android-architecture-redline-guard.py`，扫描 137 处违规点、0 违例；ARCH001–007 七条红线各有负例（共 23 条负例）；窄例外必须同时具备局部说明与行为回归。
  - 待验证：受控 CI runner 的守卫回执（CI 腿登记在[斗战胜佛](CODE_TODO_BLOCKED.md) §二）。
  - 本机可执行动作：跑统一验证批次；守卫已接入 `scripts/quality/check-structure.sh`。

- [ ] BANDROID-044.5 [kind: infra] [priority: P1] [points: 2] [owner: CI + 本机检查入口] [status: Integrated] 质量门接线并自测
  - 产物：Android workflow 与本机入口分别运行三 flavor lint、ktlint、Detekt、文件预算与架构守卫。
  - 依赖：BANDROID-044.1～044.4。
  - 验收：正反例验证任务注册、排除、放行与拒绝；真实执行后才称质量门接通。
  - 当前证据（L）：提交 `faa013c`；本机入口 `scripts/quality/android-quality-gates.sh` 一次跑完 lint/ktlint/Detekt/架构守卫并带自测；Android 单测 192 core host + 391 app 全绿；架构守卫以 `--self-test` + `--mode enforce` 接入结构门。
  - 待验证：受控 CI runner 的真实回执（CI 腿登记在[斗战胜佛](CODE_TODO_BLOCKED.md) §二）。
  - 本机可执行动作：跑统一验证批次 `bash scripts/quality/check-structure.sh`。

## 九、文档与状态表达红线（已交付，2条 / 5pt）

- [ ] STATE-001.1 [kind: mechanism] [priority: P0] [points: 3] [owner: 全体 crate owner] [status: Integrated] 隐式状态机全量审计与逐条改写
  - 产物：语义审计器 `scripts/quality/implicit-state-audit.py`（识别 phase carrier 上的多 bool 与 Option-as-stage）+ 判决台账 `scripts/quality/implicit-state-verdicts.json`；每条候选必须有人工判决（`legitimate` 独立事实 / `fixed` 已改写 / `open` 仍待改），未判决或 `open` 即门禁失败。
  - 依赖：无；与并行工流按文件所有权切分（Bevy UI / Bevy widgets / Iced runtime 三簇）。
  - 验收：候选全部判决完毕；`--mode enforce` 通过；真实违规全部改写为 typed enum + typed transition，并有行为测试证明非法组合不再可表示。
  - 当前证据（L）：审计器 `candidates=22 undecided=0 open=0 stale_verdicts=0`（enforce 通过）；台账 22 条全部判决——16 条判为 `fixed`（真实阶段表改写为 typed enum + typed transition，含 3 处原本可表示非法组合者：Iced 供应商维护/快照差异弹窗/探针/系统代理/生命周期/运行期补丁、Bevy UI 的确认/DNS 表单/连接抽屉、Bevy widgets 的看门狗/混沌/焦点陷阱/主从视图/拖拽/虚拟槽/PIP 激活），9 条判为 `legitimate`（真实二值独立事实，逐条写明理由）；`scripts/quality/typed-state-guard.py` 0 findings。
  - 待验证：统一验证批次复跑审计器与结构门（`--mode enforce` + 结构门），确认新合入代码未引入新候选。
  - 本机可执行动作：跑统一验证批次。

- [ ] PUB-006.6 [kind: infra] [priority: P1] [points: 2] [owner: docs] [status: Integrated] Docs.rs 预览与公共 API 文档质量门
  - 产物：docs.rs 渲染模拟配置与 API 文档门禁；七个 crate 开启 `#![warn(missing_docs)]` 并提供标准 Rustdoc 与 doctest。
  - 依赖：PUB-006.1、PUB-006.2。
  - 验收：`cargo doc --all-features --no-deps` 零警告，intra-doc links 全部校验；公开 API 文档覆盖率 100% 且含可执行 doctest。
  - 当前证据（L）：`scripts/quality/rustdoc-guard.py` 是 docs.rs 等价门禁——`--mode structure` 校验七个 crate 的公开项全部带文档、每个 crate 至少一条可执行 doctest、docs.rs 元数据齐备，`--mode docs` 跑 `cargo doc --all-features --no-deps` 取零告警并跑 `cargo test --doc` 证明示例可执行；1436 个公开项已补齐真实 Rustdoc，7 条可执行 doctest 353/353 全绿；门禁已接入 `scripts/quality/check-structure.sh`（structure + docs 两种模式）。
  - 待验证：统一验证批次复跑结构门，确认门禁在当前代码上仍零告警。
  - 本机可执行动作：跑统一验证批次。

## 十、开源生态与发布门禁（已交付，4条 / 7pt）

- [ ] PUB-006.7 [kind: mechanism] [priority: P1] [points: 2] [owner: test-infra] [status: Integrated] 无头沙盒集成测试与混沌容错验证
  - 产物：针对中性运行时的无头 CI 集成沙盒与混沌套件；覆盖网络波动、内核猝死拉起、锁竞争与损坏配置热恢复。
  - 依赖：PUB-003.4、PUB-006.1。
  - 验收：异常注入下状态机 100% fail-closed、租约安全回收、无内存泄漏且能自愈。
  - 当前证据（L）：新包 `sandbox`（`lurker-sandbox`，publish=false）注入四类确定性故障——网络抖动、内核进程猝死、锁竞争、配置损坏——每类一条场景，断言全部是 typed 结果：`RuntimeError::Port(PortError::Network(_))` 与 `Failed { phase: Readiness }`（进程起停配平）、`PortError::Failed` 与 `Failed { phase: StartProcess }`（无残留 running 声明）、真实 `flock` 争用被拒且写栅栏可再次获取、`DocumentValidation::Invalid { diagnostics }` 且耐久写回逐字节一致；租约腿证明外来终态被拒而绑定仍在、精确终态释放租约且日志改绑新 owner；资源腿证明取得的每份资源都被归还；故障模型用穷尽匹配，新增故障类必须同时新增场景；混沌套件 nextest `8f00c3f7-19a1-421f-a3f8-bebada50dbf8` 7/7 全绿；守卫 `sandbox-chaos-guard.py` 拒绝套件里的随机与计时（`rand`/`sleep(`/`Instant::now` 等）。
  - 待验证：统一验证批次复跑混沌矩阵与守卫，确认新合入代码未引入新故障类。
  - 本机可执行动作：跑统一验证批次 `cargo nextest run -p lurker-sandbox`。

- [ ] PUB-006.9 [kind: mechanism] [priority: P1] [points: 2] [owner: examples] [status: Integrated] 最小化用例展厅 (Examples Gallery)
  - 产物：`examples/` 下 4 套独立极简用例（minimal-cli / minimal-slint / minimal-iced / minimal-bevy），各 ≤200 行、无多余依赖。
  - 依赖：PUB-005.1～PUB-005.3、PUB-004.2。
  - 验收：四套 example 独立 `cargo run --example` 成功并接入 CI 编译守卫。
  - 当前证据（L）：四套用例各为独立 workspace 包并只声明一个同名 `[[example]]` 目标：minimal-cli 53 行、minimal-slint 64 行（+50 行 slint 文件）、minimal-iced 103 行、minimal-bevy 160 行，全部 ≤200 行；依赖只有中性 crate 加各自展示的一套工具链，工具链需求与产品模板逐字节比对（版本与 feature 只有一个真源）；`examples-guard.py --mode compile` 四例全部编译通过（`--mode smoke` 无头 CLI 例运行通过），`--mode structure` 校验目录、目标、白名单、模板一致与行数预算；CI 编译守卫落在已登记触发策略的 `.github/workflows/ecosystem-gate.yml`。
  - 待验证：受控 tag 的生态门禁 workflow run 回执（CI 腿在[斗战胜佛](CODE_TODO_BLOCKED.md) §二）。
  - 本机可执行动作：跑统一验证批次。

- [ ] PUB-006.10 [kind: infra] [priority: P1] [points: 2] [owner: CI] [status: Integrated] 多架构交叉构建自动化与 SemVer 兼容守卫
  - 产物：Linux/Windows/Android aarch64 与 x86_64 交叉编译流水线 + `cargo-semver-checks` 门禁。
  - 依赖：GAP-1.1、PUB-006.1。
  - 验收：四大目标架构交叉编译 100% 通过；SemVer 门禁能拦截中性 crate 公开 API 的破坏性变更。
  - 当前证据（L）：`semver-compat-guard.py` 锁定七个中性 crate 的公开 API 基线（1182 项，登记在 `semver-baseline.json`）并逐项比对；`--mode check` 报 0 差异 0 新增；**负例证明门禁会失败**——删除一个公开函数后门禁拒绝（`the gate rejected the deliberate break`），负例已接入结构门，所以一个不会失败的门禁会直接让构建红；`cross-build-guard.py` 证明四条桌面 triple、`scripts/cross-check.sh` 的两条 aarch64 腿与 `scripts/android-check.sh` 的两条 Android 腿都在位且没有 Apple 目标混入；类型检查版的 `cargo-semver-checks` 分析在生态门禁 workflow 里跑（`--mode tool --require-tool`）。
  - 待验证：受控 tag 的生态门禁 workflow run 回执与真实 `cargo-semver-checks` 工具层回执（CI 腿在[斗战胜佛](CODE_TODO_BLOCKED.md) §二）。
  - 本机可执行动作：跑统一验证批次（含 `scripts/cross-check.sh` 交叉腿）。

- [ ] PUB-006.11 [kind: evidence] [priority: P1] [points: 1] [owner: docs/qa] [status: Integrated] 开发者全生命周期演练与干跑审计
  - 产物：在纯净克隆环境模拟外部开发者全旅程（克隆 → 看文档 → 脚手架生成 → 运行 → 接驱动 → 调配置）并输出审计报告。
  - 依赖：PUB-005.5、PUB-006.4、PUB-006.9。
  - 验收：旅程零阻滞，全链路 ≤5 分钟，体验毛刺清零并归档回执。
  - 当前证据（L）：`developer-journey-rehearsal.py` 走完克隆 → 读文档 → 脚手架 → 构建 → 运行 → 接驱动 → 调配置七步，逐步计时并给出 `ok`/`blocked`/`failed` 判定，总预算 300 秒；干跑实测产出完整审计报告（无占位符残留，模板 20 个占位符与 8 个审计问题全覆盖），守卫 `journey-rehearsal-guard.py` 校验步骤顺序、预算与报告完整性。
  - 待验证：纯净克隆的真实演练（需要 `cargo-generate` 与一次真实编译），随统一批次执行。
  - 本机可执行动作：跑统一验证批次 `python3 scripts/developer-journey-rehearsal.py --profile fast`。

## 十一、登记说明

- 2026-10-11 之前的历史切片回执（run ID、APK SHA、模拟器记录）已在 [TODO.md](../TODO.md) 归档表与 `.evidence/` 永久留存；本文件只持未办结验证项。
- 已验证退出项不在本文件重复登记。
- 环境阻塞条目统一在[斗战胜佛](CODE_TODO_BLOCKED.md)；本板不再持有「下一个动作无法在本机执行」的条目。
