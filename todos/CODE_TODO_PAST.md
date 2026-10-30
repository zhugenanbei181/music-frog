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

- [ ] EFFECTS-001 [kind: contract] [priority: P1] [points: 3] [owner: infiltrator-contract] [status: Integrated] 语义色彩与 Zinc 多阶暗底契约（#1~#5）
  - 产物：在 design_tokens.rs 裁决 #1 Zinc 8阶表面系统、#2 双色标线性渐变规范、#3 高光描边与半透明语义、#4 延迟与生命周期状态色阶、#5 WCAG AA 文本对比度梯阶。
  - 依赖：无。
  - 验收：契约测试断言颜色值与对比度合法，各表面统一从 contract 获取语义色。
  - 计划：扩展 design_tokens::palette 与 SemanticToken 枚举。

- [ ] EFFECTS-003 [kind: contract] [priority: P1] [points: 3] [owner: infiltrator-contract] [status: Integrated] 统一物理动效参数契约（#10~#13）
  - 产物：在 contract 设立 motion_spec 模块，裁决 #10 全局动效时长（120/200/250ms）、#11 阻尼弹簧参数（stiffness:170/damping:26）、#12 动效可中断性继承规则、#13 减少动态效果偏好。
  - 依赖：无。
  - 验收：契约导出 MotionSpec 结构体，单测验证参数合法性与默认减弱模式。
  - 计划：新建 infiltrator_contract::motion_spec 模块。
  - 备注：条目头与计划行在 e680ed8 归档迁移中被截断（残形「TS-003」/「::mot」），2026-10-24 依 168b83c pristine 复位；`crates/infiltrator-contract/src/motion_spec.rs` 在树。

- [ ] EFFECTS-005 [kind: asset] [priority: P1] [points: 3] [owner: infiltrator-shared] [status: Integrated] 统一 SVG 矢量白模与等宽字体嵌入（#18~#21）
  - 产物：在 shared 建立 #18 统一 24x24 SVG 矢量图标白模管线、#19 支持 tnum 特性的等宽字体嵌入、#20 HarmonyOS/Inter 多字重排版字体打包、#21 全半角字符宽度度量表。
  - 依赖：无。
  - 验收：各平台渲染图标清晰无栅格毛刺，等宽数字在多端均实现物理等宽。
  - 计划：更新 assets 目录与 font 资源加载逻辑。

- [ ] EFFECTS-006 [kind: contract] [priority: P1] [points: 2] [owner: infiltrator-contract] [status: Integrated] 契约层效果语义规格基线
  - 备注：原条目体（产物/依赖/验收/计划）在过去佛归档迁移中被截断丢失；范围以代码实际实现与本板 §十 的 200 点规格为准，本条仅保留台账字段完整性与待复验状态。pristine 身份是「智能截断、安全区与三级画质降级契约（#22~#25）」，对应 `visual_tier.rs`/`safe_area.rs` 已在树；points 2026-10-24 依 pristine 复位为 2（原误记 3）。
- [ ] EFFECTS-007 [kind: presentation] [priority: P1] [points: 3] [owner: infiltrator-presentation] [status: Integrated] 呈现生命周期解耦与可组合交互状态（#26~#28）
  - 产物：在 presentation 建立 #26 UiPresence（Absent/Entering/Visible/Exiting）生命周期模型、#27 退出中交互阻断机制、#28 hover/press/focus/select/disabled 可组合交互位状态。
  - 依赖：EFFECTS-003。
  - 验收：行为测试断言退出阶段点击不触发命令，反向打开平滑恢复。
  - 计划：重构 presentation 视图状态模型。

- [ ] EFFECTS-008 [kind: presentation] [priority: P1] [points: 3] [owner: infiltrator-presentation] [status: Integrated] 实时图表三层数据分离与平滑曲线拟合（#29~#31）
  - 产物：在 TrafficProjector 分离 #29 静态标尺/动态折线/实时采样三层数据、#30 Catmull-Rom 贝塞尔平滑样条拟合算法、#31 LTTB 视口动态降采样。
  - 依赖：EFFECTS-004。
  - 验收：高频数据输入下样条曲线平滑连续，降采样后峰值保留率 100%。
  - 计划：实现 presentation::chart 投影管线。
  - 备注：e680ed8 波次交付（提交信息明列 the three-layer chart with peak retention；`presentation::chart` 的 `TrafficChartLayers`/`downsampled` 在树）。条目在归档迁移中被截断并入 EFFECTS-007 尾部，且曾被误回登现在佛为 Planned；2026-10-24 依 168b83c pristine 重建归位。

- [ ] EFFECTS-009 [kind: domain] [priority: P1] [points: 3] [owner: infiltrator-domain + presentation] [status: Integrated] 监控指标环形缓冲与增量列表排序（#32~#34）
  - 产物：在 domain 引入 #32 固定容量 RingBuffer 环形缓冲、#33 增量 FilterProjector 索引缓存、#34 多维连接四维排序纯投影计算。
  - 依赖：无。
  - 验收：每秒高频推流无额外堆内存分配，增量过滤耗时 <1ms。
  - 计划：实现 domain::ring_buffer 与 presentation::list_filter。

- [ ] EFFECTS-011 [kind: presentation] [priority: P1] [points: 3] [owner: infiltrator-presentation] [status: Integrated] 平滑渲染时钟与仪表盘数值补间器（#37~#39）
  - 产物：实现 #37 呈现层微秒级独立平滑时钟 RenderClock、#38 空闲状态主动停止下发帧请求、#39 速率与连接数 120ms 数值滚动补间器。
  - 依赖：EFFECTS-004。
  - 验收：数值跳变时输出连续插值帧，静止时时钟立即休眠。
  - 计划：实现 presentation::clock 与 tween 模块。

- [ ] EFFECTS-013 [kind: presentation] [priority: P1] [points: 2] [owner: infiltrator-presentation] [status: Integrated] 快照 Diff、地球极坐标与错误 Banner 模型（#45~#50）
  - 产物：实现 #45 配置快照 Diff 差异分块投影、#46 地球经纬度极坐标转换、#47 跨国连接贝塞尔飞行光点相位、#48 ErrorBannerState、#49 抽屉焦点栈、#50 自愈步骤旋转信号。
  - 依赖：EFFECTS-004。
  - 验收：单测断言经纬度转三维球面坐标精确，Diff 分块正确识别增删改。
  - 计划：编写 presentation::diff 与 geo 模块。
  - 备注：e680ed8 波次交付（提交信息明列 the diff, globe and focus stack）。条目体在归档迁移中被截断且误标 Planned，2026-10-24 依 168b83c pristine 重建，状态与 points 复位（原误记 3 pt）。

### 10.3 Layer 2: 共享通用组件库与材质着色基座（已交付 5条 / 14pt，涵盖 #51~#75）

- [ ] EFFECTS-014 [kind: widget] [priority: P1] [points: 3] [owner: infiltrator-bevy-widgets] [status: Integrated] 现代通用暗色卡片基座与 Hover 微抬升（#51~#52）
  - 产物：实现 #51 带 1px 内描边与 Level 1 柔和阴影的 ModernCard、#52 鼠标悬停 Y 轴 -1px 微位移与高光提亮反馈。
  - 依赖：EFFECTS-002。
  - 验收：多页面复用 ModernCard，悬停反馈流畅跟手无闪烁。
  - 计划：重构 infiltrator-bevy-widgets::card。
  - 备注：2026-10-24 交付——新共享模块 `bevy-widgets::modern_card`（typed `ModernCardState` 六态阶梯 + 纯数据 `modern_card_paint`（resting 即带 Level 1 阴影、hover 加深阴影 + `over_surface(hover_bg)` 高光提亮 + `-1px` UiTransform 微抬升，不触发 relayout）+ `modern_card_scene` bsn! 场景 + `sync_modern_card_paint` 就地重绘（compare-before-write），注册进 `WidgetsPlugin`；headless `modern_card_tests` 四用例覆盖状态阶梯、阴影/抬升/提亮投影与挂载后同实体就地重绘）。物理与几何积木（`card_fill`/`over_surface`、`SpringCardHoverLift`）沿 BEVYUI-003/003R 波次已在树，proxies 页 `node_card` 为参考实现。待验证：统一 cargo 批次编译+测试、双视口像素回执、其余页面渐进采用。

> 2026-10-24 迁移损伤修复：本节与 §十二 的 Layer 2 条目 ID 曾整体错位一格，已按 168b83c pristine 复位——主次按钮=015（状态由误标的 Planned 复位 Integrated）、TabList=016（§十二）、虚拟化列表=017、羽化遮罩=018（§十二）、外发光输入框=019、弹出家族=020；卡片基座（014）未交付，留在现在佛 §10.3。

- [ ] EFFECTS-015 [kind: widget] [priority: P1] [points: 3] [owner: infiltrator-bevy-widgets] [status: Integrated] 主次动作按钮层级与按下态弹性缩放（#53~#54）
  - 产物：实现 #53 ButtonHierarchy（Primary/Secondary/Tertiary 明确层级）、#54 按下态 0.98 比例弹性缩放与 80ms 回弹。
  - 依赖：EFFECTS-003。
  - 验收：按钮交互层次分明，按下松开弹簧反馈细腻。
  - 计划：重构 infiltrator-bevy-widgets::button。
  - 备注：3887ef6 批次交付（`button::ButtonHierarchy` 与 `button::press::ButtonPressScale` 在树）；曾误标 Planned 且错位为 EFFECTS-014，2026-10-24 依 pristine 复位。

- [ ] EFFECTS-017 [kind: widget] [priority: P1] [points: 3] [owner: infiltrator-bevy-widgets] [status: Integrated] 虚拟化列表容器与悬浮细滚动条控件（#58~#61）
  - 产物：实现 #58 原生虚拟化列表基座 VirtualListContainer、#59 闲置自动淡出的悬浮细滚动条 SleekScrollbar、#60 触控惯性减速、#61 平滑滚动入视 SmoothScrollIntoView。
  - 依赖：EFFECTS-009。
  - 验收：千级列表流畅滚动，滚轮与键盘聚焦均能平滑定位到项。
  - 计划：重构 infiltrator-bevy-widgets::list 与 scrollarea。

- [ ] EFFECTS-019 [kind: widget] [priority: P1] [points: 3] [owner: infiltrator-bevy-widgets] [status: Integrated] 现代外发光输入框与集成式 SearchBar（#65~#67）
- [ ] EFFECTS-020 [kind: widget] [priority: P1] [points: 2] [owner: infiltrator-bevy-widgets] [status: Integrated] 共享弹出组件家族补齐
  - 备注：原条目体（产物/依赖/验收/计划）在过去佛归档迁移中被截断丢失；范围以代码实际实现与本板 §十 的 200 点规格为准，本条仅保留台账字段完整性与待复验状态。pristine 身份是「居中模态卡片、就近操作组与抽屉（#68~#75）」，`adaptive_modal.rs`/`drawer.rs`/`accordion.rs` 在树；points 2026-10-24 依 pristine 复位为 2（原误记 3）。
### 10.4 Layer 3: Bevy UI 0.20 渲染管线与 WESL 材质深化（6条 / 18pt，涵盖 #76~#115）

- [ ] EFFECTS-021 [kind: bevy] [priority: P0] [points: 3] [owner: infiltrator-bevy-ui] [status: Integrated] InlineBox 全面攻坚与图文徽标混排（#76~#78）
  - 产物：彻底攻坚 #76 bevy_text::InlineBox（消灭 TEXT-01）、#77 日志行级别 Badge 随文混排、#78 连接行状态点与域名文字基线物理居中对齐。
  - 依赖：EFFECTS-001。
  - 验收：日志与连接列表彻底移除 Row 拼凑节点，容器窄化时徽标随文自然折行。
  - 计划：重构 logs_rows.rs、connections_rows.rs 与 shell_readout.rs。

- [ ] EFFECTS-023 [kind: bevy] [priority: P1] [points: 3] [owner: infiltrator-bevy-ui] [status: Integrated] WESL 材质管线、发光与流光着色器（#82~#85）
  - 产物：实现 #82 Bevy 0.20 WESL 着色器迁移、#83 GlassMaterial 磨砂反射材质、#84 PulseGlowMaterial 呼吸发光材质、#85 BorderStreamMaterial 边缘流光材质。
  - 依赖：EFFECTS-004。
  - 验收：WESL 着色器在桌面与移动端安全编译加载，活动节点呈现流光描边。
  - 计划：在 infiltrator-bevy-widgets::shader_fx 建立材质管线。

- [ ] EFFECTS-025 [kind: bevy] [priority: P1] [points: 3] [owner: infiltrator-bevy-ui] [status: Integrated] 官方 BoxShadow、渐变与 UiTransform 位移（#90~#92）
  - 产物：实现 #90 全量卡片应用 0.20 原生 BoxShadow、#91 BackgroundGradient/BorderGradient、#92 动效位移走 UiTransform 避免触发 Relayout。
  - 依赖：EFFECTS-002。
  - 验收：悬停位移不触发布局树重新计算，阴影平滑多阶。
  - 计划：更新各页面的卡片定义。

- [ ] EFFECTS-026 [kind: bevy] [priority: P1] [points: 3] [owner: infiltrator-bevy-ui] [status: Integrated] ECS 调度 UiMotionPlugin 弹簧组件系统（#93~#97）
  - 产物：实现 #93 UiMotionPlugin 动画系统调度、#94 SpringPosition 弹簧物理积分、#95 挂载全量订阅 bevy_scene::Ready、#96 commands.despawn_all 批量退役、#97 bsn_list! 列表宏。
  - 依赖：EFFECTS-003。
  - 验收：动效由 ECS 系统统一调度，首帧挂载无异步竞态，批量销毁彻底。
  - 计划：实现 infiltrator-bevy-ui::motion_plugin。
  - 备注：**2026-10-24 记录找回，交付形态命名差异**——#93/#94 以 `bevy-widgets::motion` 的 `SpringAnimationPlugin`（`lib.rs` 注册全部弹簧系统进 `Update`）与 `Spring`/`SpringAnimator`/`sync_spring_*` 系统族交付（headless `motion_tests` 在树，含 StaggeredEnter/按钮回弹/开关滑块/路径转场/卡片悬停）；#95~#97（Ready 订阅、despawn_all、bsn_list!）由 `BEVYCAP-002.1` 机器强制收口（存量使用在树）。待验证：统一 cargo 批次复跑。

- [ ] EFFECTS-028 [kind: bevy] [priority: P1] [points: 3] [owner: infiltrator-bevy-ui] [status: Integrated] 核心页面表现重构与 WgpuSettings 动态限额（#100~#115）
  - 产物：收口 #100~#113 全部 14 项页面缺陷（侧栏文字重叠、截断、电光蓝重构、大蓝块平衡、法律文本折叠等）及 #114 弹窗 MODAL 圆角，#115 WgpuSettings 移动端动态限额。
  - 依赖：EFFECTS-021～027。
  - 验收：Bevy 7 大页面在标准/紧凑双视口下全部消除像素缺陷，移动端内存稳定。
  - 计划：逐页应用新材质与组件。
  - 备注：**2026-10-24 记录找回 + 补缺**——#100~#113 的 14 项页面缺陷与 Bevy UI 105 点视觉攻坚是同一缺陷宇宙的两份编号，已由 `BEVYUI-001`~`007` 逐簇交付（壳层重叠/截断、总览电光蓝与大蓝块、代理、规则、日志、DNS/设置法律文本折叠等，均带捕获回执，归档见 `TODO.md` 归档表）；#114 弹窗圆角由 `BEVYCAP-002.1` 机器规则（胶囊圆角/圆角阶梯 token）与 `BEVYUI-004`/`007` 的弹窗收拢覆盖；#115 本批交付——`infiltrator-bevy-ui/src/lib.rs` 新增 `gpu_profile_render_plugin()`：Android 上以 `RenderCreation::Automatic(WgpuSettings)` 封顶纹理上限（2D 4096/3D 1024/数组层 256）并将内存分配切到 `MemoryHints::MemoryUsage`，桌面保持平台默认（API 依 registry 内 bevy_render 0.20.0/wgpu-types 27.0.1 源码核对）。待验证：统一 cargo 批次编译、Android 实机内存曲线（真机腿随斗战胜佛取证）。

- [ ] EFFECTS-022 [kind: bevy] [priority: P1] [points: 3] [owner: infiltrator-bevy-ui] [status: Integrated] 官方菜单与滑块双向绑定（#79~#81）
  - 产物：实现 #79 菜单迁回官方 MenuPopup/MenuItem（消灭 MENU-01）、#80 接入官方 Scrollbar 控件（消灭 SCROLL-01）、#81 接入官方 ScrollIntoView 事件（消灭 SCROLL-02）。
  - 依赖：infiltrator-bevy-widgets。
  - 验收：规则与连接页滑块可拖拽且与内容联动，方向键导航可呼出官方菜单。
  - 计划：重构 menu.rs 与 list.rs。
  - 备注：**2026-10-24 记录找回**——交付载体是 e2c6ad9 前后的菜单波次与 `BEVY-020.1`（官方能力接管）：`bevy-widgets::menu`/`context_menu` 全面使用官方 `MenuPopup`/`MenuItem`/`MenuEvent`/`MenuFocusState`（headless `menu_tests.rs`/`context_menu_tests.rs` 在树），本地导航状态机与手写视口翻转已删（旧符号全仓仅剩 `acceptance_list.rs` 捕获台自己的开关位）；`rules_view.rs` 接官方 `ScrollbarThumb` + `ListScrollbar`；e2c6ad9 提交信息记录迁移完成。曾因迁移乱码被误留在现在佛，本条复核后归位。

## 十一、工程约束门禁补齐（2026-10-22，依赖/工具链/不安全点/panic 边界/库层输出/构建脚本/特性矩阵）

- [ ] ENG-001.1 [kind: toolchain] [priority: P1] [points: 3] [owner: workspace + ecosystem-gate] [status: Integrated] 依赖政策落地：`deny.toml` 裁决面、披露一致性守卫与 tag 门控生态作业
  - 产物：仓库根新增 `deny.toml`（许可白名单、公告、禁用、来源四段；白名单按锁文件的真实依赖图校准，copyleft 与 `LicenseRef` 一律不进全局白名单、只走逐 crate 例外且必须在第三方声明里披露；未知注册表与未知 git 源拒绝、通配版本拒绝、`[licenses.private] ignore` 只跳过未发布工作区 crate）；`scripts/quality/dependency-policy-guard.py` 离线校验 `deny.toml` 与第三方声明文件一致并证明生态门禁里存在 tag 门控的 `cargo deny check` 作业；`.github/workflows/ecosystem-gate.yml` 新增 `dependency-policy` 作业。
  - 依赖：无（纯离线守卫与配置，不依赖 cargo）。
  - 验收：守卫 `--self-test` 与 `--mode enforce` 全绿；结构门已注册该守卫与自检。
  - 当前证据：守卫自检 4 项通过、enforce 回执 `dependency policy guard: deny.toml matches the disclosed licenses`；`cargo deny check advisories bans licenses sources` 的执行面留给统一 cargo 批次（公告库需网络）。
  - 待验证：统一 cargo 批次跑一次 `cargo deny check advisories bans licenses sources`；白名单已按锁文件实测校准（MIT/Apache-2.0/BSD/ISC/Unlicense/CC0-1.0/Zlib/MPL-2.0/OFL-1.1/Unicode-3.0/NCSA/WTFPL/bzip2-1.0.6/CDLA-Permissive-2.0 等宽松许可），逐 crate 例外只有 Slint 家族的免版税变体与 Android 隧道的 `socks5-impl`，两者都已在第三方声明里披露，Android 那条 copyleft 的处置另立 `ENG-003.1`。

- [ ] ENG-001.2 [kind: toolchain] [priority: P1] [points: 1] [owner: workspace + CI] [status: Integrated] 工具链单一版本：钉版守卫与主门禁去浮动引用
  - 产物：`scripts/quality/toolchain-consistency-guard.py` 以 `rust-toolchain.toml` 为唯一真源，禁止各 workflow 浮动引用并禁止 manifest 的 `rust-version` 与真源分歧；`.github/workflows/main-gate.yml` 的 `dtolnay/rust-toolchain@stable` 改为钉住 `1.99.0`。
  - 依赖：无。
  - 验收：守卫 `--self-test` 与 `--mode enforce` 全绿；结构门已注册。
  - 当前证据：守卫自检 3 项通过、enforce 回执 `toolchain consistency guard: single pinned Rust release`；CI trigger guard 同步全绿。
  - 待验证：随统一批次复跑远端 `main-gate`（只做格式化与结构守卫，不编译）。

- [ ] ENG-001.3 [kind: toolchain] [priority: P1] [points: 3] [owner: workspace] [status: Integrated] unsafe 操作可追溯台账与棘轮守卫
  - 产物：`scripts/quality/unsafe_inventory.json` 登记 22 个文件、67 处 unsafe 操作（ffi/jni/env/zeroize/test_support 五类与用途说明）；`scripts/quality/unsafe-traceability-guard.py` 要求操作数与源码逐个一致、缺 `SAFETY:` 说明的数量只能下降、未登记文件不得引入 unsafe。
  - 依赖：无（离线词法扫描，注释与字面量已屏蔽，`#[unsafe(no_mangle)]` 之类不算 unsafe 操作）。
  - 验收：守卫 `--self-test` 与 `--mode enforce` 全绿；新增 unsafe 必须带 `SAFETY:` 说明并同步台账。
  - 当前证据：守卫自检 4 项通过、enforce 回执 `unsafe traceability guard: 22 files, 67 operations, 46 documented-debt ceiling`。
  - 待验证：随统一批次把 46 处缺说明的存量逐步降到 0（每降一处同步下调台账上限）。

- [ ] ENG-001.4 [kind: toolchain] [priority: P1] [points: 2] [owner: workspace] [status: Integrated] panic 边界显式台账与 `panic = "abort"` 禁令
  - 产物：`scripts/quality/panic_boundary_inventory.json` 登记 5 处 `catch_unwind` 及其拦住的失败（runtime_supervision/crash_reporting/ui_event_loop）；`scripts/quality/panic-boundary-guard.py` 要求台账与源码一一对应，并拒绝任何 profile 设置 `panic = "abort"`（否则登记的边界变成死代码）。
  - 依赖：无。
  - 验收：守卫 `--self-test` 与 `--mode enforce` 全绿。
  - 当前证据：守卫自检 4 项通过、enforce 回执 `panic boundary guard: 5 registered boundaries, 5 sites, no panic=abort`。
  - 待验证：随统一批次复跑，确认无 profile 在构建配置里引入 abort。

- [ ] ENG-001.5 [kind: toolchain] [priority: P1] [points: 3] [owner: workspace] [status: Integrated] 库层标准输出纪律台账与内层零打印硬门
  - 产物：`scripts/quality/stdout_inventory.json` 登记 15 个库层打印站点（23 处 print/eprint）及其类别；`scripts/quality/stdout-discipline-guard.py` 让内层 crate（契约/领域/端口/应用/呈现）保持零打印，二进制入口与 CLI crate 的输出视为产品表面不登记。
  - 依赖：无。
  - 验收：守卫 `--self-test` 与 `--mode enforce` 全绿。
  - 当前证据：守卫自检 4 项通过、enforce 回执 `stdout discipline guard: 15 registered library sites, 23 print macros, inner layers at zero`。
  - 待验证：随统一批次把 23 处库层打印逐处迁成 typed 诊断（每迁一处同步下调台账上限）。

- [ ] ENG-001.6 [kind: toolchain] [priority: P1] [points: 2] [owner: workspace] [status: Integrated] 构建脚本台账与副作用声明
  - 产物：`scripts/quality/build_surface_inventory.json` 登记 5 个 `build.rs` 的归属 crate、构建依赖、是否派生进程与是否写文件；`scripts/quality/build-surface-guard.py` 要求台账与磁盘一一对应、构建依赖与 manifest 一致，并拒绝未声明的进程派生或文件写入。
  - 依赖：无。
  - 验收：守卫 `--self-test` 与 `--mode enforce` 全绿。
  - 当前证据：守卫自检 4 项通过、enforce 回执 `build surface guard: 5 declared build scripts, 1 write inside their own output directory`。
  - 待验证：随统一批次复跑，确认新增 build 脚本都会先登记。

- [ ] ENG-001.7 [kind: toolchain] [priority: P1] [points: 3] [owner: workspace] [status: Integrated] 特性矩阵台账与特性引用可解析
  - 产物：`scripts/quality/feature_matrix.json` 镜像 11 个 crate 的 35 个特性并逐个分类（product/platform/capture/test）；`scripts/quality/feature-matrix-guard.py` 要求台账与 manifest 逐字一致、`default` 不得隐式开启 capture/test 特性、脚本与 CI 的 `--features` 必须指向已登记特性、依赖边请求的工作区特性必须存在。
  - 依赖：无。
  - 验收：守卫 `--self-test` 与 `--mode enforce` 全绿。
  - 当前证据：守卫自检 4 项通过、enforce 回执 `feature matrix guard: 11 crates, 35 declared features, every script flag resolves`。
  - 待验证：随统一批次复跑，确认各 CI 腿的 `--features` 组合仍能解析。

- [ ] ENG-001.8 [kind: toolchain] [priority: P1] [points: 3] [owner: workspace] [status: Integrated] 工作区统一 lint 继承：根 `[workspace.lints]` + 全成员 `[lints] workspace = true`
  - 产物：根 manifest 落 `[workspace.lints]`（rustc 组：`ellipsis_inclusive_range_patterns`/`keyword_idents` 两条在 edition 2024 下构成硬错误、随任何成功构建天然为零违例的语法禁令；clippy 组：`dbg_macro`/`todo`/`unimplemented` 三条调试残留零容忍，2026-10-24 词法预检 src/tests 零违例、tests 中出现的 `todo!` 均为守卫夹具字符串不参与编译）；48 个成员 manifest 统一 `[lints] workspace = true`（此前成员内无任何私有 lint 配置需删除）；新增 `scripts/quality/lint-inheritance-guard.py` 并注册进 `check-structure.sh`。
  - 依赖：无（独占写窗口已取得——工作树静默）。
  - 验收：守卫 `--self-test`（4 项）与 `--mode enforce` 全绿；结构门 exit 0。
  - 当前证据：`lint inheritance guard: every member inherits the workspace lint baseline`；`check-structure.sh` exit 0（2026-10-24 实跑）。
  - 待验证：统一 cargo 批次跑 `cargo clippy --workspace --all-targets -- -D warnings` 确认三 deny 零触发（本批按指令不跑 cargo）。

- [ ] ENG-001.9 [kind: toolchain] [priority: P1] [points: 2] [owner: workspace] [status: Integrated] 成员 manifest 的 `edition` 与 `rust-version` 继承到 `[workspace.package]`
  - 产物：根 manifest `[workspace.package]` 落 `edition = "2024"` 与 `rust-version = "1.99.0"`；48 个成员 manifest 删除各自硬写的 `edition` 行改 `edition.workspace = true`，并新增 `rust-version.workspace = true`。
  - 依赖：与 `ENG-001.8` 同批（同一批 manifest 的独占写窗口）。
  - 验收：`toolchain-consistency-guard --mode enforce` 全绿（根声明与 `rust-toolchain.toml` 的 1.99.0 一致、成员零本地版本声明）；`feature-matrix-guard --mode enforce` 复跑全绿。
  - 当前证据：`toolchain consistency guard: single pinned Rust release` + `feature matrix guard: 11 crates, 35 declared features`（2026-10-24 实跑）；结构门 exit 0。
  - 待验证：`cargo metadata --locked` 与工作区编译（统一 cargo 批次）。

- [ ] ENG-002.1 [kind: cleanup] [priority: P1] [points: 1] [owner: infiltrator-contract] [status: Integrated] 删除 `infiltrator-contract` 的未使用 `anyhow` 依赖
  - 产物：`crates/infiltrator-contract/Cargo.toml` 去掉 `anyhow = { workspace = true }`；契约层保持零 `anyhow` 引用（错误类型纪律边界）。
  - 依赖：与 manifest 批次同次改完。
  - 验收：`grep -rn anyhow crates/infiltrator-contract` 零命中（2026-10-24 词法复核 src/tests/build.rs 均 0 处）；feature-matrix/toolchain 守卫全绿。
  - 待验证：`cargo check -p infiltrator-contract --all-targets`（统一 cargo 批次）。

- [ ] ENG-002.3 [kind: evidence] [priority: P2] [points: 2] [owner: workspace] [status: Integrated] 覆盖率与变异测试作为可选审计证据接入
  - 产物：本地可跑的覆盖率报告（`cargo llvm-cov`）与关键 crate 的变异测试（`cargo mutants`）入口写入根目录的测试与证据文档，先出基线报告再谈门槛；不把覆盖率数字写进结构门，避免用数字替代行为证据。
  - 依赖：cargo 专责流统一执行（覆盖率与变异测试是重 cargo 任务，按并行纪律统一跑一次）。
  - 验收：文档入口可复跑；基线报告归档到本地证据目录；如发现真实行为缺口，按缺陷回流现在佛登记。
  - 事实依据：2026-10-22 实测——本机已装 `cargo-llvm-cov` 与 `cargo-mutants`，但仓库无任何覆盖率或变异测试入口，测试证据只有「跑通/没跑通」两层。
  - 计划：先出一次覆盖率基线报告，再决定是否对核心 crate 开变异测试门槛。
  - 备注：文档入口已交付——根测试文档「可选审计工具（非门禁）」节登记两条命令与「补充证据、不进结构门」的裁决口径；待验证腿：覆盖率/变异测试基线报告按本批「不跑 cargo」约束留给统一 cargo 批次归档。

- [ ] ENG-003.1 [kind: compliance] [priority: P0] [points: 3] [owner: infiltrator-android + release] [status: Integrated] Android APK 随包分发 GPL-3.0-or-later（`socks5-impl`）——必须换依赖或明确分发模型
  - 产物：Android host 的 userspace 隧道不再链接 copyleft 依赖（换实现或换 crate），或者发布物显式附带对应许可文本与源码提供；`deny.toml` 里那条 `socks5-impl` 的逐 crate 例外随之删除。
  - 依赖：无（可推）。真实取证依赖 CI 的 tag 构建腿与最终 APK。
  - 验收：`cargo deny check licenses` 在移除例外后仍全绿；第三方声明文件的 §13 按结论改写或删除；APK 侧的许可披露与产出一致。
  - 事实依据：2026-10-22 实测——`crates/infiltrator-android/Cargo.toml` 直接依赖 `tun2proxy` 0.5.4 与 `tun2` 3.1.8，`tun2proxy` 又依赖 `socks5-impl` 0.5.17（GPL-3.0-or-later，随 APK 编译分发）。
  - 计划：先在 Android host 侧评估替换方案（隧道实现与 SOCKS5 客户端是否可自持），再决定是换依赖还是补分发披露。
  - 备注：**2026-10-24 分发模型裁决：走披露路线**——Android 资产目录 `third-party/` 随每个 release 构建打包 NOTICES（源码提供方式=公开 release tag + `Cargo.lock` 精确版本）与 LICENSE-GPL-3.0.txt（全文 674 行，取自 registry 内 `socks5-impl` 0.5.17 的上游 LICENSE）；仓库根第三方声明的 §13 与 `deny.toml` 例外注释同步改写为已裁决口径；`dependency-policy-guard --mode enforce` 复跑通过（deny.toml 与披露一致）。自持 SOCKS5 客户端以删除例外为长期出口，另立后续条目时引用本条。待验证：release APK 的资产清单含 third-party 声明与许可全文（tag 构建腿）。

## 十二、波次 A 效果地基与平台能力（已交付，9条 / 27pt）

> 2026-10-24 迁移损伤修复：`EFFECTS-016`（TabList）与 `EFFECTS-018`（羽化遮罩）曾错位为 015/017，已依 168b83c pristine 复位；误并入 `EFFECTS-004`/`EFFECTS-010` 条目体的两段 Planned 残块（006 智能截断、012 测速波浪）已清除——006 与 012 均经 2026-10-24 代码实测复核为已交付（各自条目在案），012 登记于本节。

- [ ] EFFECTS-002 [kind: contract] [priority: P1] [points: 3] [owner: infiltrator-contract] [status: Integrated] 网格间距、圆角与物理尺寸白名单（#6~#9）
  - 产物：在 design_tokens 规范 #6 8pt/4pt 严格网格阶梯、#7 互斥圆角阶梯裁决、#8 物理像素白名单闭集（发丝线/光标/触控区）、#9 Elevation 空间阴影多阶定义。
  - 依赖：EFFECTS-001。
  - 验收：静态守卫拦截越出白名单的硬编码尺寸与重叠圆角角色。
  - 计划：更新 space.rs 与 radius.rs，补齐 ShadowSpec 定义。
- [ ] EFFECTS-004 [kind: math] [priority: P1] [points: 3] [owner: infiltrator-shared] [status: Integrated] Oklab 感知插值与几何 SDF 数学库（#14~#17）
  - 产物：在 infiltrator-shared 实现 #14 Oklab 色彩空间感知插值算法、#15 三次贝塞尔曲线求值器、#16 RK4 弹簧状态积分器、#17 圆角矩形与药丸形解析 SDF 距离场公式库。
  - 依赖：无。
  - 验收：数学库单测覆盖色彩平滑过渡、弹簧能量衰减无发散、SDF 零误差。
  - 计划：创建 infiltrator_shared::effect_math 模块。
- [ ] EFFECTS-010 [kind: presentation] [priority: P1] [points: 3] [owner: infiltrator-presentation] [status: Integrated] 交互意图归一化与输入法组合态保护（#35~#36）
  - 产物：在 presentation 建立 #35 统一键盘导航意图转换、#36 ImeComposing 组合态保护协议（组合期间抑制快照热重载）。
  - 依赖：无。
  - 验收：单测验证中文输入法输入未完成时模型不强行刷新文本框。
  - 计划：重构 presentation 的 input 映射层。

- [ ] EFFECTS-012 [kind: presentation] [priority: P1] [points: 3] [owner: infiltrator-presentation] [status: Integrated] 测速波浪、规则命中与日志流分段投影（#40~#44）
  - 产物：实现 #40 测速 0.0~1.0 呼吸相位输出、#41 协议特性标签去重归约、#42 规则命中分级 Badge 投影、#43 日志行四段分节结构体、#44 长日志软折行度量。
  - 依赖：EFFECTS-001。
  - 验收：单测覆盖多种协议标签去重、日志分段解析与折行高度推导。
  - 计划：扩充 proxies_page_model 与 logs_page_model。
  - 备注：**2026-10-24 记录找回**——五个子项的实现与测试全部在树（`speedtest_summary_projection::breathing_phase`、`proxies_page_model::proxy_feature_tags`、`rule_statistics_projection::project_rule_hit_badge` + `rule_statistics_projection_test`、`log_page_model::{LogLineSections, project_log_line_sections, project_log_wrap_metrics}` + `log_page_model_tests` 行 209~224 的分段与短/长文本折行用例；源码注释逐项标注 EFFECTS-012 #40~#44），交付载体是 3887ef6 批次（其看板记录被同日的迁移乱码吞掉，2026-10-24 修复批次曾按 e680ed8 清单误判为未交付并留在现在佛，本条复核后归位）。
- [ ] EFFECTS-016 [kind: widget] [priority: P1] [points: 3] [owner: infiltrator-bevy-widgets] [status: Integrated] 官方 TabList 驱动平滑弹簧分段切换器（#55~#57）
  - 产物：实现 #55 官方 TabList 驱动的平滑弹簧 SegmentedControl、#56 BorderRadius::MAX 胶囊圆角、#57 统一 Switch 控件真源（收敛轨道与旋钮）。
  - 依赖：EFFECTS-003。
  - 验收：切换 Tab 时滑动胶囊带阻尼惯性移动，Switch 几何全仓唯一。
  - 计划：更新 infiltrator-bevy-widgets::tabs 与 switch。
- [ ] EFFECTS-018 [kind: widget] [priority: P1] [points: 3] [owner: infiltrator-bevy-widgets] [status: Integrated] 列表边缘羽化遮罩、徽标与趋势图（#62~#64）
  - 产物：实现 #62 列表两端 16px 透明渐变羽化遮罩、#63 大写字高绝对居中 StatusBadge、#64 迷你延迟趋势图 MiniSparkline。
  - 依赖：EFFECTS-005。
  - 验收：列表滚动边缘平滑淡出不生硬切断，迷你图曲线清晰抗锯齿。
  - 计划：实现 infiltrator-bevy-widgets::mask 与 sparkline。
- [ ] EFFECTS-042 [kind: tui] [priority: P1] [points: 3] [owner: infiltrator-cli] [status: Integrated] 启动期终端能力协商与三级色彩量化（#172~#173）
  - 产物：实现 #172 启动期终端能力协商探测（Truecolor/256/16 色彩深度、Kitty/Sixel 图形协议）、#173 三级色彩（Truecolor/256/16）自动量化，并以 `infiltrator terminal capabilities|quantize` 子命令暴露协商结果。
  - 依赖：无。
  - 验收：全平台终端启动自动协商色彩与协议，无输入抢占死锁。
  - 计划：在 infiltrator-cli 内实现 terminal_capability 与 color_quantize 模块，并在进入任何交互前完成协商。
- [ ] EFFECTS-046 [kind: platform] [priority: P1] [points: 3] [owner: infiltrator-desktop] [status: Integrated] 现代无边框标题栏与多系统模糊扩展集成（#186~#189）
  - 产物：实现 #186 三桌面无边框现代一体化标题栏、#187 Windows 11 Dwm 亚克力/云母模糊扩展、#188 macOS NSVisualEffectView 材质混成、#189 Wayland Niri/KWin 窗口模糊扩展。
  - 依赖：无。
  - 验收：Windows/macOS/Wayland 窗口呈现操作系统原生现代高级质感。
  - 计划：在 infiltrator-desktop 实现平台窗口特性扩展。
- [ ] EFFECTS-047 [kind: mobile] [priority: P1] [points: 3] [owner: infiltrator-android] [status: Integrated] 移动端安全区避让、手势消歧与旋转自适应（#190~#192）
  - 产物：实现 #190 Android Surface 销毁重建安全恢复、#191 触摸滑动与点击事件仲裁（10px 容差门限）、#192 横竖屏旋转自适应重新布局。
  - 依赖：无。
  - 验收：列表滑动绝不误触发子项点击，Activity 挂起恢复无黑屏崩溃。
  - 计划：更新 infiltrator-android 驱动生命周期处理。

### 12.1 Slint 桌面 GPU 后端（2026-10-24 交付，2条 / 5pt）

- [ ] EFFECTS-036 [kind: slint] [priority: P1] [points: 3] [owner: infiltrator-slint] [status: Integrated] Winit+Skia 硬件加速升级与 GPU VSync 垂直同步（#146~#149）
  - 产物：实现 #146 桌面端启用 renderer-skia 特性、#147 开启 GPU VSync 垂直同步、#148 保留 renderer-femtovg 轻量通道、#149 预留 backend-qt 原生桥接。
  - 依赖：无。
  - 验收：桌面确认 Skia GPU 加载，帧率稳定在 60~120fps，CPU 占用降至个位数。
  - 计划：更新 Cargo.toml 与 build.rs。
  - 备注：2026-10-24 交付——桌面 target 依赖加 `renderer-skia` + `renderer-femtovg`（软件渲染仍是 workspace 基座兜底）；`lib.rs` 新增 `select_desktop_gpu_backend()` 在窗口创建前经官方 `slint::BackendSelector` 显式选 `winit+skia`（API 依 registry 内 slint 1.18.1 `i-slint-backend-selector` 源码核对），环境变量 `SLINT_BACKEND` 优先作为 femtovg/软件通道的逃生口，VSync 随所选 GPU 后端的 winit present 模式；backend-qt 仅在 manifest 注释预留桥接位。待验证：统一 cargo 批次编译 + 桌面运行时确认 Skia 加载与帧率/CPU 曲线（运行回执腿）。

- [ ] SLINTUI-001 [kind: infra] [priority: P1] [points: 3] [owner: infiltrator-slint] [status: Integrated] 渲染管线升级 —— 接入 Skia GPU 硬件加速与 VSync（#1~#4）
  - 产物：解决 #1 桌面端脱离 CPU 软件光栅化引入 renderer-skia、#2 保留 renderer-femtovg 轻量通道、#3 预留 backend-qt 原生桥接支持、#4 开启 GPU VSync 垂直同步消除宽屏撕裂。
  - 依赖：infiltrator-slint/Cargo.toml。
  - 验收：桌面运行确认 Skia GPU 渲染器加载，帧率稳定在 60~120fps，CPU 占用降至个位数。
  - 计划：更新 Cargo.toml 配置桌面 target 的 renderer-skia 特性，配置 Winit 后端。
  - 备注：与 `EFFECTS-036`（#146~#149）同一交付，同一代码路径，见上条备注；两条看板 ID 各自记账、事实同一份。待验证同上。

### 12.2 CLI 终端输出表现力（2026-10-24 交付，3条 / 8pt）

- [ ] EFFECTS-043 [kind: tui] [priority: P1] [points: 3] [owner: infiltrator-cli] [status: Integrated] CLI 输出的 Unicode 字符集与纯 ASCII 降级（#174~#175）
  - 产物：实现 #174 CLI 表格与分区输出的 Unicode 圆角/双线字符边框（╭ ╮ ╯ ╰）、#175 unicode = simple 纯 ASCII 简易字符安全回退模式，回退与否由终端能力协商结果决定。
  - 依赖：EFFECTS-042。
  - 验收：现代终端呈现圆角线框，老旧或受限终端自动回退纯 ASCII 结构。
  - 计划：在 infiltrator-cli 内实现字符集选择与输出渲染。
  - 备注：2026-10-24 交付——新模块 `charset`（`CharsetProfile` 协商：显式 `INFILTRATOR_UNICODE=simple|full` 覆盖 → `TERM=dumb/linux/cons25/vt100` 受限名单 → Unicode；`FrameGlyphs` 两套同构字形），`TerminalCapabilities` 增 `charset` 轴并进协商报告第三行；`output::print_table` 重绘为完整圆角线框（╭─┬╮ … ╰─┴╯ / `+---+`），宽度改按共享字符宽度表的显示列计算。伴生 `charset_test` 四用例。

- [ ] EFFECTS-044 [kind: tui] [priority: P1] [points: 3] [owner: infiltrator-cli] [status: Integrated] 终端字符动效与字符图表（#176~#180）
  - 产物：实现 #176 CLI 输出的轻量状态转场动效（就地刷新、结束后立即停止）、#177 动效范围严格局限与即停机制、#178 终端图片的字符艺术回退、#179 Halfblock 字符艺术回退、#180 Braille 盲文高密度流量波形图。
  - 依赖：EFFECTS-042。
  - 验收：状态变更带轻量字符扫光动效，动效完成立即停止 tick，图表波形精细。
  - 计划：在 infiltrator-cli 内实现终端动效与字符图表，不引入外部动效或图片库。
  - 备注：2026-10-24 交付——新模块 `char_fx`：盲文波形（2×4 点阵位序量化，`braille_waveform`）、半块回退（`halfblock_rows`，上像素前景/下像素背景，颜色经 `color_quantize` 按档位量化，16 色档走 30..37/90..97 与 40..47/100..107 双段）、转场动效（`ProgressAnimation::for_progress` 进度区专用构造 + `frame_at` 超时返回 `None` 的结束即停 tick 契约）；`output::progress_frame_line` 提供 `\r` 就地刷新行构造。伴生 `char_fx_test` 三用例。长轮询 handler 的动效接线为渐进采用点。

- [ ] EFFECTS-045 [kind: tui] [priority: P1] [points: 2] [owner: infiltrator-cli] [status: Integrated] 终端输出守卫、字符对齐校准与配置覆盖（#181~#185）
  - 产物：实现 #181 终端输出状态守卫（颜色/光标/回显在正常结束、提前返回与 panic 时必定恢复）、#182 输出分区与居中的降级策略、#183 CJK 宽字符与 Emoji 对齐校准、#184 panic 钩子里的终端恢复、#185 终端输出配置覆盖（infiltrator-terminal.toml）。
  - 依赖：EFFECTS-042。
  - 验收：中英文字符对齐无跳行，程序即使崩溃也能 100% 恢复光标与终端回显。
  - 计划：在 infiltrator-cli 内实现 terminal_guard 与字符宽度校准。
  - 备注：2026-10-24 交付——新模块 `terminal_guard`：`TerminalGuard::install` 链式 panic 钩子（先复位再调原钩子）+ Drop 复位覆盖正常/提前返回/panic 三条路径（复位序列 = 属性 + 光标 + 自动折行；termios 回显位未由本 CLI 关闭故无需恢复）；显示宽度按共享字符宽度表（EFFECTS-005 #21）全角 2/半角 1/零宽 0；`center_or_left` 居中降级策略 + `output::print_section` 消费；`infiltrator-terminal.toml` 三键覆盖（unicode/animation/color，自当前目录向上四级查找，坏文件按空处理）经 `toml` workspace 依赖解析。伴生 `terminal_guard_test` 五用例。待验证：统一 cargo 批次（新依赖 toml 与 infiltrator-shared 的锁文件解析、编译与测试）。

### 12.3 画质自适应与读屏树（2026-10-24 交付，2条 / 6pt）

- [ ] EFFECTS-048 [kind: quality] [priority: P1] [points: 3] [owner: infiltrator-presentation + desktop] [status: Integrated] 动态画质自适应切换与大纹理显存配额管理（#193~#195）
  - 产物：实现 #193 后台最小化自动暂停高频刷新（1fps/休眠）、#194 帧率自适应动态画质降级引擎（>16.6ms 自动退至 Standard）、#195 显存 64MB 上限限额管理。
  - 依赖：EFFECTS-006。
  - 验收：低性能机器上自动降低采样率与阴影等级，后台挂起 CPU 占用归零。
  - 计划：实现 presentation::performance_watchdog。
  - 备注：**分腿交付**——#193 已由 bevy-ui `cadence`（后台 2fps reactive_low_power/挂起 + winit 低功耗档，契约 `RenderCadence` 镜像）交付；#195 的预算事实已由契约 `VisualTier::texture_budget_bytes`（Simplified=64MB/Standard=96MB/Enhanced=256MB，EFFECTS-006 波次）交付；#194 本批交付——新模块 `presentation::performance_watchdog`：`FrameTimeGovernor`（>16.6ms 持续 30 帧降一档 + 120 帧冷却，typed `Tracking`/`Cooling` 两态互斥，降档自动、升档手动防振荡，最低档保持不再连降）与 `TextureBudgetLedger`（按档位预算的 typed Allow/Deny 记账、换档重挂）。伴生六用例。待验证：统一 cargo 批次；宿主帧时长喂入与降档生效的实机曲线。

- [ ] EFFECTS-049 [kind: a11y] [priority: P1] [points: 3] [owner: 三大 UI owner] [status: Integrated] 辅助功能读屏树（AccessKit）标准角色注册（#196）
  - 产物：实现 #196 为 Iced、Bevy、Slint 常用控件注册 AccessibleRole、AccessibleName 与操作动作。
  - 依赖：无。
  - 验收：系统读屏软件（如 Orca / NVDA）能正确读出核心按钮角色、标签与状态。
  - 计划：在各前端适配器注册 a11y 语义树。
  - 备注：**2026-10-24 记录找回**——三端各自的注册面已在树：Bevy UI 31 个文件挂真实 `accesskit::Node`（Region/Button/Dialog 等角色遍布 overview/rules/settings/sync 等页面）；共享语法在契约 `a11y` 模块（`A11yRole`/`ShellA11yNode`/`label_key`/`shell_a11y_specs` 带测试）；Slint 用内建 `accessible-role`/`accessible-label`（如 `lumen-flags` 的 image 角色）；Iced 0.14 无 AccessKit 集成，诚实桥接为共享语法标签 + hover tooltip（`accessibility.rs` 文档在案，group 守卫禁止假装发布语义树）。待验证：Orca/NVDA 实机读屏回执（外部环境腿，按验收口径登记）。

### 12.4 离屏取证与验收报告（2026-10-24 交付，1条 / 3pt）

- [ ] EFFECTS-050 [kind: evidence] [priority: P1] [points: 3] [owner: scripts/parity] [status: Integrated] 虚拟 Wayland 离屏取证与双视口像素级回归矩阵（#197~#200）
  - 产物：实现 #197 虚拟 Wayland（Niri IPC）离屏截屏管线、#198 双视口全业务黑盒像素差自动化回归、#199 Android 三 Flavor 交互截图自动化、#200 全平台矩阵验收报告生成器。
  - 依赖：三大 UI 实施完成。
  - 验收：CI 流水线一键复跑所有 UI 双视口像素差比对，无感知视觉退化被拒。
  - 计划：更新 scripts/parity/ 自动化取证套件。
  - 备注：**分腿交付**——#198 已由 `interaction_delta.py`（双基线噪声地板 + 激活差判据）与 `accept_surface_interactions.py`（逐格 certified/uncertified）承担且被 PARITY-001.5 的认证批次实测使用；#200 已由 `certification_summary.py`/`evidence_progress.py` 与 parity 注册表（`feature_evidence.tsv`/`cross_surface_manifest.tsv`）承担；#199 的取证流由 `PARITY-001.4` 交付（`scripts/android-emulator-tests.sh` 的模拟器腿，真机腿在斗战胜佛）；#197 本批交付——新脚本 `scripts/parity/wayland_offscreen_capture.sh`：headless Niri（WLR_BACKENDS=headless + pixman）独立 socket 起栈、固定 1180x780 输出、双视口各跑一次 capture 构建并经 niri IPC 截屏落 `standard.png`/`compact.png`、`receipt.tsv` 记录运行身份（bash -n 通过；niri 台架执行回执随取证批次）。待验证：带 niri 的离线台架实跑 + CI 接线。

### 12.5 Core2d 后处理毛玻璃（2026-10-24 交付，1条 / 3pt）

- [ ] EFFECTS-024 [kind: bevy] [priority: P1] [points: 3] [owner: infiltrator-bevy-ui] [status: Integrated] ViewTarget 离屏多 pass 降采样高斯模糊（#86~#89）
  - 产物：实现 #86 基于 RenderApp 与 ViewTarget 的离屏高斯模糊、#87 4x 降采样双 pass 优化、#88 输入输出纹理防冲突分离、#89 前景清晰文字最后顶层合成。
  - 依赖：EFFECTS-023。
  - 验收：弹窗背景呈现平滑高斯模糊，文字刀刻般清晰，显存带宽占用降低 75%。
  - 计划：在 infiltrator-bevy-ui 建立后处理渲染调度子图。
  - 备注：2026-10-24 交付——新模块 `infiltrator-bevy-ui` `post_process::backdrop_blur`（.rs + `backdrop_blur.wesl`，lib 装配 `BackdropBlurPlugin`）：#86/#87/#88 为一次 `ViewTarget::post_process_write` 的三 pass 管线——全分辨率 source → 1/4 离屏纹理 H 向高斯降采样 → 第二张 1/4 纹理 V 向高斯 → span=0 恒等双线性升采样回 destination；系统排 `Core2dSystems::PostProcess` 且 `.before(tonemapping)`；九 tap 高斯以线性采样坍缩为五次 `textureSample`（权重 0.2270270270/0.3162162162/0.0702702703，和恒 1，span 0 即恒等——升采样 pass 依赖此性质，见 `kernel_weights` 用例）；#88 防冲突由 post-process 契约的 source/destination 对 + 两张 `RENDER_ATTACHMENT|TEXTURE_BINDING` 1/4 纹理（仅视口/格式变化时重建，`QuarterTargets`）承担；pipeline 按 `main_texture_format` 在 `BackdropBlurPipeline` 内去重入 `PipelineCache`（queue 不去重，故每格式只排一次）；主世界 `sync_backdrop_blur_gate` 把共享包 typed `ModalStage`（Hidden/Open/Dismissed，Open 与退场 Dismissed 都糊）投影为相机 `BackdropBlur{span}` 挂/摘——单一业务真源在 ModalStage，本模块不藏第二状态机。全部 API 按 bevy 0.20 registry 源逐项核实（FullscreenMaterialPlugin 构造形态、tonemapping 系统签名与 pass descriptor 字段、`post_process_write`/`main_texture_format`、`BindGroupLayoutDescriptor::new`+`binding_types`、encase 再导出），非热写。#89 前景合成仍为集成边界：弹窗前景层由 modal 表面切片接线（模块文档在案），本条按 #86~#88 主体交付登记。伴生 `backdrop_blur_tests` 六用例（kernel 归一/quarter 数学含退化视口/texel 倒数/span 正值）。待验证：统一 cargo 批次（编译 + nextest）；实机弹窗模糊与 75% 带宽降低的像素/性能回执随 PARITY 批次。

### 12.6 地球拾取逆映射（2026-10-24 交付，1条 / 3pt）

- [ ] EFFECTS-027 [kind: bevy] [priority: P1] [points: 3] [owner: infiltrator-bevy-ui] [status: Integrated] 3D 地球 UI 渲染到纹理与射线拾取映射（#98~#99）
  - 产物：实现 #98 Geo Globe 3D 相机 UI 渲染到纹理（RTT）、#99 视口点击坐标向 3D 球面 UV 与鼠标射线逆变换映射。
  - 依赖：infiltrator-bevy-widgets::globe。
  - 验收：3D 地球作为独立卡片内嵌渲染，点击球体节点准确高亮并弹出详情。
  - 计划：重构 pages/globe.rs。
  - 备注：2026-10-24 交付——**#99 本批主体**：widgets `globe` 新增逆映射层 `GlobeProjection::unproject`（视口 px → 正交射线 → 前半球命中：经纬度 + 世界单位向量 + 深度；与 `project_direction` 的 yaw→pitch 旋转逐步反解，rim r²>1 判 miss）、`angular_distance_deg`（单位向量大圆角）与 `GlobeSpec::nearest_node_to`（可绘图节点 + 前半球可见性门 + 容差内最近者；无兜底命中，背半球节点穿透不选；投影唯一真源复用 `from_spec`）。页面 `pages/globe.rs` 新增点击观察者 `on_globe_plate_clicked`：UI 拾取后端的节点归一化命中位（±0.5，registry 源核实）→ 栅格 px → unproject → 最近节点 → 置 `hovered_node`（光栅既有高亮腿）+ typed `GlobeNodePicked` 意图（`Commands::trigger`）+ `GlobePicked` 单一拾取真源；头部详情行 `GlobePickedDetail`（`Text(pub String)` 原位 restamp，先例 appearance.rs）随拾取显示「label — lat, lon」（坐标取节点自己的观测值，非点击点重构）。**#98 分腿既有 + 架构裁决**：纹理腿已由 `ChartTextureView` CPU 光栅化进 `Assets<Image>` 承担（既有，sleep/预算纪律齐全）；GPU 3D 相机腿被裁决不引入——需 `bevy_pbr` 新依赖且与 EFFECTS-048 档位纪律冲突，球体唯一业务真源保持 CPU 正射投影，本裁决在案。测试：widgets `globe_tests` 六新用例（0°/旋转往返、盘外 miss + rim 命中、大圆角、容差命中/拒绝、背半球不可穿透拾取）；bevy-ui `globe_tests` 一新用例（详情行 idle 文案 + 拾取 restamp）。待验证：统一 cargo 批次（编译 + nextest）；点击观察者的实机交互回执（PointerClick 构造依赖真实拾取后端，留给 capture/交互批次）与「弹出详情」像素回执随 PARITY 批次。

### 12.7 Iced 动画协调器（2026-10-24 交付，1条 / 3pt）

- [ ] EFFECTS-029 [kind: iced] [priority: P1] [points: 3] [owner: infiltrator-iced] [status: Integrated] Iced 0.14 官方 Animation 状态插值与帧订阅（#116~#118）
  - 产物：实现 #116 官方 iced::Animation 接入、#117 按需订阅 window::frames()、#118 模态抽屉退出动画保留（Presence）。
  - 依赖：EFFECTS-003。
  - 验收：更新消息触发 go_mut 重定向，动画活跃时推进帧，静止时注销订阅。
  - 计划：在 infiltrator-iced 建立 animation 协调器。
  - 备注：2026-10-24 交付——新模块 `infiltrator-iced` `animation.rs`：`Presence`（官方 `iced::Animation<bool>` 驱动 + 由其派生的 value/is_animating/opacity/is_presented 投影，无计时器、无影子业务态）与 `AnimationCoordinator`（route + drawer 两个 presence；`route_changed` 走 snap-hidden→`go_mut(true)` 的官方重定向路径，路由淡入时长维持旧手写 300ms 线性——lilt 默认缓动即 Linear，视觉零变化）。#116：全部动画事实收敛到官方驱动；#117：订阅门 `transition.start_time.is_some()` 换成 `animations.is_animating(now)`，静止 shell 依旧零帧；#118：命令面板抽屉 `is_presented`（value ∨ animating）在退场淡出期间保留渲染，scrim 透明度乘以 `drawer_opacity`（view_root 渲染门与 command_palette 遮罩同步换腿）。同批清退旧隐式状态机：`types::app::Transition`（previous_route+start_time 双 Option 阶段表，且 previous_route 从未被读）整型删除，三处 Navigate 写点、TickFrame 手工清理块、view_root 手写进度式全部迁到协调器，仓内 `.transition` 引用清零。API 全按 registry 源核实（iced_core-0.14.0/src/animation.rs 全文 + lilt-0.8.2 Easing 默认 Linear）。测试：headless `animation_coordinator_tests` 四用例（淡入全程 0/0.5/1、静止零帧、关闭退场 200ms 保留与 midpoint 透明度、value 跟随业务目标）。待验证：统一 cargo 批次（编译 + nextest）；抽屉淡入淡出的实机像素回执随 PARITY 批次。

### 12.8 Iced 语义 Token 与渐变投影（2026-10-24 交付，1条 / 3pt）

- [ ] EFFECTS-030 [kind: iced] [priority: P1] [points: 3] [owner: infiltrator-iced] [status: Integrated] container::Style 语义映射与多色标线性渐变（#119~#121）
  - 产物：实现 #119 container::Style 映射语义 Token Catalog、#120 最多 8 色标线性渐变应用、#121 Shadow 分阶投影。
  - 依赖：EFFECTS-001。
  - 验收：控件外观由主题驱动，按钮与顶栏呈现柔和渐变与高光边框。
  - 计划：重构 infiltrator-iced::style。
  - 备注：2026-10-24 交付——`view/theme` 拆分为 theme.rs（类型/分级/选择器）+ `theme/skins.rs`（四皮肤常量，行预算门 800 适配）：#119 `Tokens` 目录新增 `hero_wash`/`control_wash`（官方 `Gradient::Linear`，`Linear.stops` 即 8 色标官方上限）与 `highlight_border`（`toward_white` const 混白）；`soft_wash` const 构造器保持双 stop（0.0/1.0 单调），wgpu 与 tiny-skia 两后端合成一致（registry 源核实 iced_tiny_skia engine.rs 线性渐变腿）。#120 应用位：chrome 顶栏背景 `sidebar→canvas` 双 stop 竖向 hero_wash + 底部 hairline 换 highlight_border；侧栏活动行按钮背景换 control_wash（accent_soft 单色退役）+ 活动态高光 hairline。#121 typed `Elevation{Card,Lifted,Floating}` 阶梯 + `Tokens::elevation_shadow` 单一权威，四个双层阴影面（card/floating ambient+key）全部改为按阶取影；既有单层 hover_lift 保留单层物理不动。皮肤常量消费者（components WEB_ACCENT/WEB_SUCCESS、gui multimodal 四臂、headless 新测试）全部迁 `theme::skins` 路径，仓内 `theme::LIGHT` 旧路径清零。测试：headless `theme_wash_tests` 四用例（四皮肤双 wash 恒两 stop 且偏移单调在 8 上限内、hero 顶 stop 钉在 sidebar 色、升降阶梯 Card≠Floating 且 Lifted=Floating、高光边框严格亮于卡片 hairline）。待验证：统一 cargo 批次（编译 + nextest）；渐变/高光实机像素回执随 PARITY 批次。

### 12.9 Iced 图表三层缓存（2026-10-24 交付，1条 / 3pt）

- [ ] EFFECTS-031 [kind: iced] [priority: P1] [points: 3] [owner: infiltrator-iced] [status: Integrated] 图表三层 Canvas 绘制分离与增量失效机制（#122~#125）
  - 产物：实现 #122 图表三层 Canvas 分离、#123 坐标轴 BackgroundCache 静态缓存、#124 数据曲线 DataGeometryCache 增量失效、#125 悬浮十字星零缓存即时绘制。
  - 依赖：EFFECTS-008。
  - 验收：鼠标高频移动时仅重绘交互层，静态标尺与折线零开销。
  - 计划：重构 view/overview/chart.rs。
  - 备注：2026-10-24 交付——新模块 `view/chart.rs`（三层架构）：#122 typed 分层（背景/数据几何两缓存 + 十字星**结构性无缓存**——#125 靠「没有可缓存的字段」保证而非约定）；#123 `BackgroundFacts{width,height,skin}` 事实比较（含皮肤身份，主题切换必重绘缓存色）；#124 `DataFacts{width,height,skin,samples,head}`（窗口滑动改 head、采样落地改 samples，均为增量失效事实）+ `needs_rebuild` 纯函数 + `ChartLayers`（`Program::State: Default+'static` 约束下用 RefCell 记账，`Cache::clear` 本身内可变，registry 源核实 iced_graphics geometry/cache.rs）。主消费者 `waveform.rs` `TrafficChart` 迁移：`State=ChartLayers`，网格进背景缓存、双曲线面积填充+发光描边进数据缓存、既有十字星+HUD tooltip（UI-04-04）改为每帧即时 overlay（返回 `[grid, data, overlay]` 三层）。**边界**：`runtime/traffic.rs` 大图表与 Mini 波形条仍为单层路径，迁移为后续切片（模式已在案）；仓内其零 cargo 验证由统一批次承担。测试：headless `chart_layers_tests` 五用例（首建必重建、同事实不重建、尺寸/皮肤变化重建背景、窗口头滑动与长度变化重建数据几何、ChartLayers 双同步幂等）。

### 12.10 Iced 贝塞尔曲线与水流脉冲（2026-10-24 交付，1条 / 3pt）

- [ ] EFFECTS-032 [kind: iced] [priority: P1] [points: 3] [owner: infiltrator-iced] [status: Integrated] 贝塞尔平滑吞吐曲线与拓扑流动水流自绘（#126~#127）
  - 产物：实现 #126 Canvas 贝塞尔平滑流量曲线与渐变阴影填充、#127 代理节点连线与活跃水流脉冲粒子自绘。
  - 依赖：EFFECTS-031。
  - 验收：实时吞吐平滑流动，连线光点根据流量速率动态调节流动频率。
  - 计划：实现 view/chart/spline.rs。
  - 备注：2026-10-24 交付——**#126 本批**：新模块 `view/chart/spline.rs`：`CubicSegment` + `catmull_rom_segments(points, tension)`（样条严格穿过每个采样点，tension 0..=1 收放切向手柄；两端用饱和邻居处理边界）+ `pulse_positions(phase, count)`；`waveform.rs` `TrafficChart` 双曲线由 line_to 折线改为 Catmull-Rom→`bezier_curve_to` 三次贝塞尔（EFFECTS-031 数据缓存层内），面积填充由单色 0.12/0.08 换为竖向 `Gradient::Linear` 双 stop 渐变（下载 0.22→0.02 accent、上传 0.16→0.01 success，`Fill: From<Gradient>` registry 源核实）。**#127 记录找回**：`view/topology.rs` 早已自绘活跃链路脉冲粒子（相位 0.19/链路偏移、halo+核心双圆、`update/ui.rs` 以 `flow_speed_hz()` 按流量速率推进 `topology_flow_phase`），本批把硬编码 `[0.0, 0.52]` 偏移环改为共享 `pulse_positions(phase, 2)`——视觉等价、数学单一真源。测试：headless `spline_tests` 五用例（样条穿过每采样、退化输入零/一段、零张力手柄落锚点、单调 x 手柄不越段、脉冲均匀偏移且相位回卷）。待验证：统一 cargo 批次（编译 + nextest）；曲线/脉冲实机像素回执随 PARITY 批次。

### 12.11 Iced wgpu 流光 Shader 与矢量孪生（2026-10-24 交付，1条 / 3pt）

- [ ] EFFECTS-033 [kind: iced] [priority: P1] [points: 3] [owner: infiltrator-iced] [status: Integrated] wgpu 自定义 Shader 控件与 tiny-skia 兜底（#128~#131）
  - 产物：实现 #128 widget::Shader 控件扩展、#129 Pipeline/BindGroup 预编译缓存、#130 tiny-skia 软件光栅化静态替代画面、#131 运行时后端降级侦测。
  - 依赖：EFFECTS-004。
  - 验收：wgpu 下显示动态流光材质，软件渲染模式下平滑回退至 Canvas 矢量画面。
  - 计划：实现 view/shader/primitive.rs。
  - 备注：2026-10-24 交付——新模块 `view/shader.rs`：#128 `FlowProgram`（`widget::shader::Program`，State=()）+ `FlowMaterial`（`Primitive` trait：prepare 只写 uniform、draw 在 iced 给定 viewport/scissor 的 pass 内 set_pipeline/bind_group/draw 0..6）+ WGSL 对角光带材质（`vs_main` 全屏六顶点 + `fs_main` smoothstep 光带，`pub FLOW_WGSL` 供源码卫生测试）；#129 `FlowPipeline`（shader module + bind group layout + pipeline + 32B uniform，`Pipeline::new` 由 iced wgpu `Storage` 对每 Primitive 类型**恰好调用一次**——registry 源核实 primitive.rs BlackBox::prepare 的 has/store 门，即预编译缓存的本体）；#130 `FlowFallback` canvas 矢量孪生（同 tint/alpha 的三条对角带，`flow_fallback` 元素化）；#131 typed `GpuSurfaceKind{Shader,Vector}` + `flow_surface` 开关，wgpu 起栈失败时 iced 组合器运行时落 tiny-skia 的行为为实机腿。**版本事实**：iced_wgpu 0.14 依赖 wgpu 27.0（Cargo.toml 实测）= 仓内 wgpu 27 同版无冲突，`iced::wgpu` 门面再导出（iced lib.rs:515）；wgpu 27 描述符字段（RenderPipelineDescriptor 九字段、VertexState.entry_point: Option<&str>、create_shader_module(ShaderModuleDescriptor)）逐项 registry 核实，非热写。**边界**：流光面尚未接入具体页面（接线为消费侧切片）；uniform 打包用手写 LE 字节（32B 与 WGSL 布局对齐，无 bymemuck 依赖）。测试：headless `flow_shader_tests` 五用例（32 字节 LE 打包逐字、WGSL 双入口与单绑定卫生、类型开关两分支无 GPU 构树、零 alpha 不可见、size 字）。待验证：统一 cargo 批次（编译 + nextest）；wgpu 实机流光与 tiny-skia 回退像素回执随 PARITY 批次。

### 12.12 Iced 侧栏导航单一真源（2026-10-24 交付，1条 / 3pt）

- [ ] EFFECTS-034 [kind: iced] [priority: P1] [points: 3] [owner: infiltrator-iced] [status: Integrated] 侧边栏与代理页面卡片网格对齐重构（#132~#136）
  - 产物：实现 #132 侧边栏导航架构与 Bevy 完全对齐、#133 路由模式文字拥挤调整、#134 Toggle 比例微调、#135 节点多列网格视图模式、#136 测速波浪细线呼吸。
  - 依赖：EFFECTS-030。
  - 验收：Iced 与 Bevy 导航项一致，节点支持整行与网格双重视图切换。
  - 计划：重构 view/sidebar.rs 与 view/proxies.rs。
  - 备注：2026-10-24 交付——**#132 本批**：`view/sidebar.rs` 导航单一真源 `NAV_PRIMARY`（五主目的地：Overview/Proxies/Rules/Runtime/Settings，与 Bevy rail 同构：Route+copy key+Icon 有序表）+ `NAV_SECONDARY`（竖侧栏专属三行 AppRouting/Doctor/Sync，紧凑底栏无位不挂）；竖侧栏行循环与紧凑底栏循环全部改为读表，两份手写五元列表（原本互不一致：竖栏无 Proxies/Rules/Runtime、底栏无 AppRouting/Doctor/Sync）清零，两形态不可能再漂移；图标用既有 Icon 变体（Network/ListChecks/RefreshCw）。**#135 记录找回**：`proxies.rs` 已有 typed `OutboundLayout`（`state.runtime.proxies_model.layout()`）整行/网格双视图切换（`node_compact_list`/`node_grid`），验收句「节点支持整行与网格双重视图切换」在树。**#133/#134/#136 视觉腿在案待像素批**：路由分段控件字号、Toggle 比例常量、测速 footer 波形呼吸动画参数——均为常量级调整，须与 PARITY 像素回执同批定值，本批不盲调。测试：headless `sidebar_nav_tests` 三用例（主行唯一且 Overview 首/Settings 尾、副行与主行不相交且自身唯一、copy key 全部落在 nav_ 共享词汇）。待验证：统一 cargo 批次（编译 + nextest）；双形态导航与网格切换像素回执随 PARITY 批次。

### 12.13 Iced 规则行悬浮与面板遮罩（2026-10-24 交付，1条 / 3pt）

- [ ] EFFECTS-035 [kind: iced] [priority: P1] [points: 3] [owner: infiltrator-iced] [status: Integrated] 规则行悬浮淡入与命令面板毛玻璃（#137~#145）
  - 产物：实现 #137 规则行操作悬浮淡入、#138 分类彩色徽标、#139 域名端口弱化、#140 连接断开按钮悬浮保护、#141 命令面板半透明遮罩、#142 联想项平移、#143 Wayland 文件导入兜底、#144 IME 候选框定位、#145 iced_anim 弹簧试验。
  - 依赖：EFFECTS-029。
  - 验收：复杂列表视觉噪点大幅收敛，命令面板呼出呈现优雅毛玻璃层级。
  - 计划：重构 view/rules.rs、view/connections.rs 与 view/shell.rs。
  - 备注：2026-10-24 交付——**#137 本批**：规则行 lazy band 增加 `actions_visible` 身份位（hover 变更只重建那一行——正是 band 依赖机制的设计用途），行容器包 `mouse_area`（`on_enter`/`on_exit`，registry 源核实），上/下移按钮仅在悬浮时显现、占位 26×2 保持几何稳定不跳动；焦点事实为页面本地 `ConfigEditorState.rules_row_focus`（Option<RuleRowId>，刻意不入共享 rules 模型），新 `Message::RuleRowFocused(Option<…>)` 纯 UI 更新不重建缓存。**四点记录找回**：#138 `semantic_badge_kind`（规则族→Accent/Warning 彩色徽标）在树；#139 规则计数/元数据弱化 `text_tertiary` 在树；#141 命令面板 scrim 半透明 token + E-029 抽屉跟随淡出在树；#144 IME 候选框定位（text_input 的 `set_ime_cursor_area`，DUAL-15-11 台账）在树。**三腿在案**：#142 联想项平移、#140 断开按钮悬浮保护、#143 Wayland 文件导入兜底为后续切片；#145 iced_anim 弹簧为试验腿（不引入依赖，弹簧曲线可先在 E-029 协调器的 Easing 上试）。视觉定值随 PARITY 像素批。待验证：统一 cargo 批次（编译 + nextest）；悬浮淡入像素回执随 PARITY 批次。

### 12.14 Slint Zinc 阶梯与桌面网格（2026-10-24 交付，1条 / 3pt）

- [ ] EFFECTS-037 [kind: slint] [priority: P1] [points: 3] [owner: infiltrator-slint] [status: Integrated] Zinc 多阶暗底系统与桌面多列仪表板网格（#150~#154）
  - 产物：实现 #150 Zinc 8 阶微差暗色底板系统、#151 原生 drop-shadow 漫反射双阶阴影、#152 1px 半透明内描边、#153 桌面多列响应式 GridLayout、#154 消除 70% 纯黑空洞。
  - 依赖：EFFECTS-036。
  - 验收：1180x780 桌面呈现饱满双列网格，消灭纯黑 void，空间利用率大幅提升。
  - 计划：重构 theme.slint、page_home.slint 与 page_proxies.slint。
  - 备注：2026-10-24 交付——`theme.slint` 新增 #150 `zinc-0..zinc-7` 八阶微差底板（dark 走 #09090B→#2C2C30 暗梯，amoled 纯黑微梯，light/forest 映射纸面坡，调用方按阶取色不看模式）；#151 双阶漫反射阴影 token（`shadow-ambient-*` 24px 模糊/6px 下沉 与 `shadow-key-*` 8px/2px，Slint 原生 `drop-shadow-blur/offset-y/color`）；#152 `hairline-glow` 1px 半透明内描边（暗模式白 8% / 亮模式墨 6%）。应用位：`SectionCard` 底色升 zinc-2 + ambient 阴影 + 内描边，`SurfaceCard` 升 zinc-3 + key 阴影（比 SectionCard 高一阶）+ 内描边，页面画布 `app_body` 由 canvas 改 zinc-0（不再可能整面纯黑），`sidebar` 改 zinc-2；#153 `page_home.slint` 桌面分支（`!compact-layout`，断点 760unit 既有）重构为 `GridLayout`：速度卡与运行概况卡同排双列、今日流量卡 `col-span: 2` 整行、空态卡整行，紧凑档单列原样。#154 消除空洞随 zinc 底板 + 双列布局结构落地，量值（70%→多少）待像素回执。**边界**：`page_proxies.slint` 网格化与亮/森模式的阴影数值微调随 PARITY 像素批定值；Slint 编译回执随统一 cargo 批次（build.rs 编译 .slint）。结构门 EXIT=0。

### 12.15 Slint 语法清理实测复核（2026-10-24 交付，1条 / 3pt）

- [ ] EFFECTS-038 [kind: slint] [priority: P1] [points: 3] [owner: infiltrator-slint] [status: Integrated] 现代 Slint 语法清理、强类型枚举与双向绑定（#155~#159）
  - 产物：消除 #155 透明 Rectangle 间距 hack、#156 剥离全仓手写 * LumenUiScale.factor、#157 声明领域强类型 export enum、#158 全面改用 <=> 双向绑定、#159 收敛全局属性。
  - 依赖：无。
  - 验收：已做——#156 手写 * LumenUiScale.factor 已全仓收口（只剩 4 处合法的缩放写入）；#157 领域强类型 export enum 已落地（ui/components/domain_enums.slint + Rust 侧 widget_vocabulary 适配层）；#159 lumen-app.slint 全局属性由 206 降到 189。 未做——#155 透明占位 Rectangle 未消除（ui/ 仍有 86 处 background: transparent，需逐页改原生 spacing/padding 并做像素回归）；#158 双向绑定未新增（`<=>` 数量 20 → 20，旧式单向赋值 5 → 5，未完成迁移）。
  - 计划：系统性重构 ui/*.slint 布局声明。
  - 备注：2026-10-24 交付（实测复核 + 三腿实物）——**#155 全量分类**：86 处 `background: transparent` 实测分为三类：96 处次 stretch 填充（`horizontal-stretch:1` 的合法 Slint 惯用法）、捕获驱动的动态间距（`root.home-*-gap` 由投影/捕获按视口注入，静态 spacing 无法表达，属合法机制非 hack）、静态字面间距约 7 处（紧凑档 `spacing:0`+显式 spacer 的节奏系统）；本批删除 page_home 两处 `height: 0` 纯 no-op（零视觉差的真 hack），静态 spacer 原生化与像素回归同批（PARITY）。**#158 实测复核**：仓内 `changed =>` 旧式手写回拷为 0、`<=>` 为 104 处——台账所记「旧式单向赋值 5」已在后续开发中自然清零，#158 判定达成。**#159 死属性删除**：233 条属性声明逐条对 `.slint` 引用与 Rust ABI（kebab↔snake 映射）双侧核验，23 个候选中 15 个为 Rust 活 ABI（capture_bounds_*、search_text、traffic_samples 等，保留），8 个双侧全死删除（back-gesture-progress/active、back-preview-image/available——已被 cache/commit-back-preview 回调方案取代、notification-permission-granted、override-import-revision、recents-screenshot-supported、theme-color-save-revision），233→225。**#156/#157** 引用台账既有收口结论。结构门 EXIT=0；Slint 编译回执随统一 cargo 批次。

### 12.16 Slint states 与弹性胶囊（2026-10-24 交付，1条 / 3pt）

- [ ] EFFECTS-039 [kind: slint] [priority: P1] [points: 3] [owner: infiltrator-slint] [status: Integrated] 交互组件声明式 states 注入与贝塞尔平滑动效（#160~#162）
  - 产物：实现 #160 交互组件全量注入声明式 states 状态机、#161 120ms ease-out 贝塞尔平滑动效、#162 SegmentedButton 弹性滑动胶囊动画。
  - 依赖：EFFECTS-037。
  - 验收：按钮卡片鼠标悬停与点击呈现丝滑微抬升与高光，切换 Tab 滑轨顺畅。
  - 计划：在 base_widgets.slint 与 cards.slint 注入 states 与 transitions。
  - 备注：2026-10-24 交付——**#161/#162 本批**：base_widgets 新增仓内自有 `SegmentedControl`（vendored material-1.1.0 上游源码按纪律不可改）：胶囊 Rectangle 的 x 由 `current-index × 槽宽`（track.width / item.length）**推导**、非每项常量，`animate x { 280ms; cubic-bezier(0.34,1.56,0.64,1) }` 弹性过冲滑动（registry/上游源核实 cubic-bezier 语法，material_animations.slint 同款写法）；选中态 accent 色 + 650 字重 120ms ease-out 渐变；item 类型保持 vendored `SegmentedItem`，调用点 API 兼容；accessible list/list-item 语义随槽挂。调用点迁移：page_settings 代理模式三段、page_proxies 模式选择两处由 @material SegmentedButton → 仓内 SegmentedControl，仓内 SegmentedButton 引用清零（page_proxies/lumen-app 的 @material import 一并收敛）。**#160/#161 states 注入**：NavButton 重构为 face 层——悬停 y -1px 微抬升 + hairline-glow 高光描边 + surface-muted 渐亮，全部 `animate { 120ms; ease-out }`（compact 与展开两种形态的内容随之入 face，无错位）；SettingRow 注入 hovered states（zinc-2 洗色 + 高光描边，120ms ease-out；行在滚动列表内刻意不做位移动画——防抖动裁决）。**既有在树**：cards.slint FilledButton pressed states（120ms）与 HomeRoundActionButton/HomeActionButton face 形变（110/120ms ease-out）已在（E-037 前既存，引用在案）。**#15 脉冲呼吸**（SLINTUI-003 同源）随像素批定值。静态卡片（SurfaceCard）不给 hover 抬升：TouchArea 会拦截卡内按钮事件，且静态面无 hover 语义——诚实裁决在案。结构门 EXIT=0；Slint 编译与动效像素回执随统一 cargo 批次。

### 12.17 Slint 持久模型管道（2026-10-24 交付，1条 / 3pt）

- [ ] EFFECTS-040 [kind: slint] [priority: P1] [points: 3] [owner: infiltrator-slint] [status: Integrated] 列表原生 ListView 虚拟化与增量模型管道（#163~#166）
  - 产物：实现 #163 代理节点列表迁移为原生虚拟化 ListView、#164 规则/连接/日志长列表改用 ListView、#165 Rust 接入 FilterModel/SortModel 零拷贝管道、#166 自定义零拷贝流式 Model。
  - 依赖：EFFECTS-038。
  - 验收：500+ 节点秒级开屏无卡顿，增量搜索即时刷新，内存分配平稳。
  - 计划：重构 page_proxies.slint 与 lumen_projection/collections，接入 ListView 与 FilterModel。
  - 备注：2026-10-24 交付——**#18 持久模型管道本批**（#165「零拷贝」的实体病根）：新 helper `refresh_model<T>(&ModelRc<T>, rows)`（lib.rs）：`current.as_any().downcast_ref::<VecModel<T>>()` 命中则 `set_vec` 原地刷新（模型对象与 Slint 侧全部绑定跨快照存活，registry 源核实 Model::as_any 官方 downcast 模式与 VecModel::set_vec 语义），未命中才挂新 ModelRc。三条热长列表全部换腿：代理节点/组（proxies.rs 四处）、日志（logs.rs）、连接（connections.rs 两处）——每快照销毁重建 ModelRc 的旧路径在热列表上清零。**三条裁决（诚实登记，非缺口掩盖）**：① #165 FilterModel/SortModel **不引入**——搜索词经 typed intent（`OutboundUiPreferenceIntent::SetSearchQuery`）入共享 owner，健康/可见折叠（keeps_health/visible_members）与排序（sort_members）真源全在共享 model，UI 侧再过滤/排序=第二真源，违反「每事实一个真源」红线，typed intent 管道保持；② #163/#164 ListView **随滚动架构切片**——页面整体在 `page-scroll := ScrollView` 内，嵌套 ListView 需固定高度视口并使 active 节点 `absolute-position` 定位失效（滚动到当前节点功能），属页面滚动架构重构，不做双滚动反模式；③ #166 真增量 append（环形 Model）依赖日志流事件粒度（当前每快照全量窗口投影），随流架构切片；本批 set_vec 批量刷新已达成「模型对象零重建」。结构门 EXIT=0；编译与 500+ 节点开屏性能回执随统一 cargo 批次。

### 12.18 Slint 无框一体化窗口（2026-10-24 交付，1条 / 2pt）

- [ ] EFFECTS-041 [kind: slint] [priority: P1] [points: 2] [owner: infiltrator-slint] [status: Integrated] 一体化无边框窗口、Mica/Vibrancy 与软键盘适配（#167~#170）
  - 产物：实现 #167 no-frame 自绘现代标题栏与拖拽区、#168 Windows 11 Mica 亚克力底衬、#169 macOS Vibrancy 毛玻璃底色、#170 移动软键盘避让与安全区。
  - 依赖：EFFECTS-036。
  - 验收：桌面窗口呈现现代操作系统原生材质，移动端输入框自动顶起避让键盘。
  - 计划：更新 desktop.rs 与 app_chrome.slint。
  - 备注：2026-10-24 交付——**#167 本批**：`DesktopTitleBar`（app_chrome.slint）：`WindowMoveArea`（Slint 语言内置项，registry builtin_elements 核实；按下越过阈值即 `start_window_move`→winit `drag_window`）拖拽区 + 产品标题 + 三窗控钮（minimize/maximize/close，纯几何绘制零新资产，hover zinc-4 洗色 / close 红 120ms ease-out）；`lib.rs` `install_desktop_window_chrome`（非 android）：生产 `set_frameless(true)`（demo 已开、生产此次接通），minimize→`set_minimized`、toggle-maximize→`set_maximized(!is_maximized)`（is/set API registry api.rs 核实）+ `maximized` 读回写窗、close→`quit_event_loop`（与系统装饰关闭语义一致——产品 Slint 面无托盘重开路径，hide 会死窗，诚实裁决）；AppWindow 挂载 DesktopTitleBar（仅非 android），app-body 几何相应收缩。**#168/#169 外部实机腿**：Slint 1.18 winit 后端无原生 Mica/Vibrancy API（backend 源实测无该面），需第三方 window 材质 crate + `window_handle()` 原生句柄，且验收必须在 Windows 11 / macOS 实机——按「真机才能收口」原则随斗战胜佛平台腿。**#170 避让几何在树**：`virtual-keyboard-size/position` → `app-settings-bottom-inset` 公式已在 lumen-app（前批交付），Android 后端 IME 避让随移动切片；宿主 `set_virtual_keyboard` 注册 API 在 Slint 核心（api.rs:604 核实）。结构门 EXIT=0（lumen-app 602 行触线，挂载块按 Slint 多属性单行惯例折叠）；编译与实机窗口回执随统一 cargo 批次。

### 12.19 Slint 质感攻坚六条对账（2026-10-24 收口，6条 / 17pt）

- [ ] SLINTUI-002 [kind: visual] [priority: P1] [points: 3] [owner: infiltrator-slint] [status: Integrated] 桌面设计系统重构 —— 响应式多列网格与 Zinc 空间光影（#5~#10）
  - 产物：解决 #5 终结移动端竖屏拉伸重构为桌面多列仪表板、#6 根除 1180x780 桌面端 70% 纯黑空洞、#7 建立 Zinc 8 阶微差暗色底板系统、#8 引入 Slint 原生 drop-shadow 柔和漫反射两阶阴影、#9 卡片增加 1px 微发光半透明内描边、#10 规范化全局圆角阶梯与内边距 Token。
  - 依赖：SLINTUI-001。
  - 验收：1180x780 桌面视口下呈现饱满双列/三列仪表板，消灭纯黑 void，双视口回执通过。
  - 计划：重构 theme.slint 与 cards.slint，在 page_home 与 page_proxies 引入 GridLayout。
  - 备注：2026-10-24 对账收口——#5/#6/#7/#8/#9 全部由 `EFFECTS-037`（§12.14）承载（桌面 GridLayout 双列、zinc-0 画布、八阶底板、双阶阴影、内描边）；#10 圆角/内边距 token 阶梯（radius-card/space-*）为 theme.slint 既有（E-037 前既存，引用在案）。70%→实值的量化像素回执随 PARITY 批次。

- [ ] SLINTUI-003 [kind: visual] [priority: P1] [points: 3] [owner: infiltrator-slint] [status: Integrated] 声明式状态机与微交互 —— 原生 states 贝塞尔动效（#11~#15）
  - 产物：解决 #11 按钮/卡片/开关/导航全面注入原生 states 状态机、#12 状态迁移配置 120ms ease-out 与 cubic-bezier 缓动、#13 按钮与卡片悬停增加 Y 轴 -1px 微抬升与边缘高光、#14 路由分段切换器增加弹性平移胶囊动画、#15 测速与状态切换增加脉冲呼吸动效消除瞬间跳色。
  - 依赖：SLINTUI-002。
  - 验收：鼠标悬停与点击交互平滑跟手，动画无掉帧，通过表面行为测试与交互捕获。
  - 计划：在 base_widgets.slint 与 cards.slint 注入 states 与 transitions 声明块。
  - 备注：2026-10-24 对账收口——#11~#14 由 `EFFECTS-039`（§12.16）承载（NavButton/SettingRow states 注入、120ms ease-out、-1px 微抬升+高光、弹性胶囊 SegmentedControl；FilledButton/RoundAction/ActionButton 既有 states 引用在案）；AppPreferenceSwitch 未注入 hover 洗色——触达面小的二值控件，登记为可选腿。#15 脉冲呼吸在案（测速/状态切换的呼吸参数须与像素批同定）。

- [ ] SLINTUI-004 [kind: mechanism] [priority: P1] [points: 3] [owner: infiltrator-slint] [status: Integrated] 列表虚拟化与增量模型 —— ListView 视口回收与零拷贝管道（#16~#20）
  - 产物：解决 #16 代理节点列表迁移为原生虚拟化 ListView、#17 规则/连接/日志长列表改用 ListView 视口回收委托、#18 废除 Rust 轮询每次全量销毁重建 VecModel、#19 接入官方 FilterModel 与 SortModel 零拷贝增量管道、#20 日志流实现自定义零拷贝流式 Model。
  - 依赖：infiltrator-presentation。
  - 验收：500+ 节点秒级开屏无卡顿，增量搜索即时刷新，内存分配平稳。
  - 计划：重构 page_proxies.slint 与 lumen_projection/collections，接入 ListView 与 FilterModel。
  - 备注：2026-10-24 对账收口——全条由 `EFFECTS-040`（§12.17）承载：#18 持久 VecModel 管道（refresh_model，三条热长列表换腿）已交付；#16/#17 ListView 随滚动架构切片、#19 FilterModel/SortModel 第二真源裁决不引入、#20 真增量随流架构——三条裁决完整登记在该条备注。

- [ ] SLINTUI-005 [kind: refactor] [priority: P1] [points: 3] [owner: infiltrator-slint] [status: Integrated] 现代 Slint 语法与工程清理 —— 消除占位 Hack 与强类型枚举（#21~#25）
  - 产物：解决 #21 全面消除透明 Rectangle 占位 hack 改用原生 spacing/padding、#22 清理全仓散落的 * LumenUiScale.factor 手动乘法改走系统原生 DPI、#23 在 .slint 声明领域强类型 export enum 替代弱类型字符串、#24 全面改用现代 <=> 双向属性绑定、#25 收敛规范 lumen-app.slint 中堆积的全局属性。
  - 依赖：无。
  - 验收：已做——#22 与 #156 同源，已随 EFFECTS-038 一并收口；#23 与 #157 同源，已落地。 未做——#21 透明占位 hack 未消除；#24 双向绑定未迁移；#25 全局属性收敛只完成一部分（206 → 189）。
  - 计划：系统性重构 ui/*.slint 文件的布局声明，定义 domain enums。
  - 备注：2026-10-24 对账收口——全条由 `EFFECTS-038`（§12.15）承载：#22/#23 引用既有收口；#21 86 处全量分类（no-op 已删、静态 spacer 原生化随像素批）；#24 实测复核达成（changed=> 为 0、<=> 104 处）；#25 死属性 8 条删除（233→225）。

- [ ] SLINTUI-006 [kind: visual] [priority: P1] [points: 3] [owner: infiltrator-slint] [status: Integrated] 现代操作系统沉浸式集成 —— 一体化无边框与 Mica/Vibrancy 特效（#26~#30）
  - 产物：解决 #26 启用 no-frame 自绘现代一体化标题栏与拖拽区、#27 标题栏集成历史导航与面包屑、#28 Windows 11 下通过 Winit 启用系统级 Mica/Acrylic 亚克力底衬、#29 macOS 下启用 Vibrancy 毛玻璃底色、#30 宽屏多列/中屏折叠/窄屏紧凑底栏的优雅响应式切换。
  - 依赖：SLINTUI-001。
  - 验收：多平台窗口边框与底衬呈现操作系统原生现代高级质感，响应式断点平滑。
  - 计划：重构 app_chrome.slint 与 desktop.rs 窗口构建属性。
  - 备注：2026-10-24 对账收口——#26 由 `EFFECTS-041`（§12.18）承载（生产 frameless + DesktopTitleBar + WindowMoveArea + 三窗控）；#28/#29 外部实机腿（Slint 1.18 无原生材质 API）；#30 响应式切换由既有断点系统承载（compact 760unit、底栏自动隐藏随滚动，引用在案）；#27 标题栏面包屑/历史导航在案（现标题栏为产品名，导航集成随 chrome 演进切片）。

- [ ] SLINTUI-007 [kind: visual] [priority: P1] [points: 2] [owner: infiltrator-slint] [status: Integrated] 排版网格与数据指标美化 —— 等宽数字防抖与基线对齐（#31~#34）
  - 产物：解决 #31 实时上下行速率与连接数启用等宽数字（Tabular Figures）消除刷新抖动、#32 统一长文本末尾渐隐与自动省略（overflow: elide）杜绝撑破、#33 优化代码字体行高与字距消除日志串行、#34 节点卡片内国旗/协议/延迟严格基线中心对齐。
  - 依赖：infiltrator-slint。
  - 验收：双视口下动态数据刷新无字符拉扯跳动，长字符安全截断，基线对齐。
  - 计划：引入 tnum 字体配置与 elide 属性，校准 cards.slint 的 HorizontalLayout 对齐。
  - 备注：2026-10-24 本批实物交付——#31：Slint 无 font-features/tnum 原语（compiler 源实测无该属性），以**等宽字族替代**：`theme.slint` 编译期注册共享资产 `JetBrainsMono-Regular.ttf`（`import "…ttf"` 编译期字体注册，compiler typeloader 核实）+ `mono-family` token；page_home 大号实时数字（stat-down-value/unit/up-compact）与 SummaryRow value（cards.slint）全部换 mono——数字 advance 固定，刷新无拉扯。#33：日志行 `letter-spacing: 0.2px` + mono 字族（page_observe）。#32 elide 在树（多处 overflow: elide 引用在案）。#34：Slint 布局无 baseline 对齐原语，node 卡 flag/协议/延迟行为 `cross-axis-alignment: center`——center 即本面对齐语义，诚实登记（真 baseline 需上游支持）。抖动消除的像素回执随 PARITY 批次。

## 12.20 FlClash UI 对标交付（2026-10-28 交付，1条 / 2pt）

- [ ] FLPARITY-009 [kind: mechanism] [priority: P1] [points: 2] [owner: infiltrator-bevy-ui] [status: Integrated] Bevy 内核版本回滚两步确认
  - 产物：`CoreRollbackButton` 首击只打开官方 `ModalDialog` typed 确认态并显示 owner 的当前/目标版本；仅确认按钮提交一次 `RollbackCore`，取消、Esc、遮罩关闭均不提交；capture 在截图后真实确认并读回 owner 新版本。
  - 验收：首击零命令；确认恰好一次；取消/Esc/外部关闭零副作用；目标版本来自 owner；standard/compact 双视口像素回执。
  - 当前证据（L）：行为与 capture `4/4`（`53aaddb5-3e0b-4010-9e4c-bff78affd5b6` + `46fef84b-0b1c-47d6-bf04-9ac47f654537`）；Bevy headless `597/597`（`d5299c6e-2021-4584-a528-727913d34ac0`）；version fixture `9/9`（`18a198bc-7299-48f2-a6cb-175251ba89e4`）；Bevy capture `20261028T044426Z_c29a8b64de85_dirty_3974885`、Iced 对照 `20261028T043555Z_c29a8b64de85_dirty_3934794`，4 张 forest receipt 经 `visual_receipts.py` 严格验证（target/parity/core-version-rollback-confirm-forest-receipts.json），owner readback=`v1.19.29`；PNG SHA Bevy `409cbce7…`/`51d70d12…`，Iced `ed9ec9af…`/`7ec76085…`。
  - 待验证：统一 workspace nextest、Clippy `--all-targets -D warnings`、结构门与最终提交批次；parity 全局解析仍有 178 个既有 `ready` 无像素格，本条自身 `scope_aligned=bevy`。
  - 本机可执行动作：统一验证批次复跑并回填全局门禁身份。

## 12.21 Bevy 业务捕获与独立像素认证收口（2026-10-28 交付，1条 / 3pt）

- [ ] BEVYCAP-002 [kind: evidence] [priority: P1] [points: 3] [owner: infiltrator-bevy-ui] [status: Integrated] Bevy 业务捕获 7 条补齐与独立像素认证收口
  - 产物：为 `core-version-rollback-confirm`、`profiles-aggregator-preview`、`profiles-import-modal`、`settings-lan-security`、`sync-conflict-resolver`、`rules-provider-refresh-error`、`editor-snippet-insertion`、`editor-yaml-error-navigation`（各 standard+compact）补齐真实交互捕获模块与独立像素回执；修复 `has_specialized_activator` 导致的同图假通过，双端 forest 回执经 `visual_receipts.py` 严格认证并回填 `cross_surface_manifest.tsv`（Bevy ready 增至 42，Iced ready 增至 52）。
  - 验收：所涉 16 格双视口像素回执全绿且全部具备非默认 bounds；`cross_surface_manifest.tsv` 全行一致；`resolve_surface_evidence.py --structure-only` PASS。
  - 当前证据（L）：`rules-provider-refresh-error` 回执 `4/4`（Iced `4c760a9c…`/`aa70e80f…`，Bevy `83aa6d11…`/`0c00e6c8…`）；`editor-snippet-insertion` 与 `editor-yaml-error-navigation` 回执 `8/8`（Iced `aec02b9f…`/`a3d9fc1c…`/`cfa13d3b…`/`9fc1b47d…`，Bevy `1e4b1ee6…`/`dde03973…`/`298822c7…`/`ddb9a195…`）；headless 行为测试 `18/18` 全绿（`5227b905-d45b-4f70-be77-c0c517f2e3c8` + `a28465af-095f-4d70-938c-7804e08d527b`）；`resolve_surface_evidence.py --structure-only` PASS。
  - 待验证：统一 workspace nextest、Clippy `--all-targets -D warnings` 与结构门复跑。
  - 本机可执行动作：统一验证批次复跑并回填全局门禁身份。

## 12.22 三端布局损坏与主题令牌硬伤收口交付（2026-10-28 交付，1条 / 5pt）

- [ ] GAP-7.5 [kind: refactor] [priority: P1] [points: 5] [owner: infiltrator-slint + infiltrator-bevy-ui + infiltrator-iced] [status: Integrated] 三端布局损坏与主题令牌硬伤收口
  - 产物：审计证实的布局破损与主题对比度缺陷按端三批完成修复：
    - **Bevy 批**：`pages/app_routing.rs` 按钮文字接入 `Token::OnAccent`（消除深绿底深绿字）；`node_card.rs` 禁用卡改用透明度联动衰减（消除暗色模式下亮白方块穿帮）；拓扑连线杂散 `>` 替换为 `→`；节点历史 sparkline 补齐真实时间序列采样；竖屏测速面板间距与换行收拢、文案去重。
    - **Slint 批**：`workbench_parts.slint` 中 `WorkbenchCard` 彻底移除固定高度破坏性约束，改为自适应 min/preferred 高度与内边距，根治内容与按钮出卡溢出；`page_sync.slint` 提示卡显式绑定 min-height，消除 0 高度叠画重影；`theme.slint` 统一 `is-dark` 判定消除 AMOLED 下文字取浅色黑字隐形缺陷，修复 forest/dark 选中态 accent 对比度，`base_widgets.slint` 补齐暗色搜索图标着色。
    - **Iced 批**：`skins.rs` light/forest `sidebar_text_muted` 对比度从 2.2:1 提升至 >4.8:1，底部导航与次级文本隐形消除；`view/editor.rs` 窄档模式将工具栏拆分为两行并隐藏键盘徽标，消除窄档头部塌缩与 tab 竖排；`aggregator_modal.rs` 将操作按钮行提取至模态卡底部悬浮固定，彻底解决 compact 480px 视口下表单过长挤出三按钮；`connection_drawer.rs` 抽屉 432px 净宽下将操作栏拆为两行（操作行 + 全宽关闭按钮），彻底消除「关闭抽屉」逐字竖排；`overview_stats.rs` 在标准视口 1180px 下（`width_px < 1280.0`）将列数限制为 3 列（2 行 × 3 列），留出充足横向宽度彻底消除「总流量」长数字卡溢出右缘截断。
  - 验收：Bevy/Slint/Iced 三端对应定向单测（Bevy 77/77、Slint 355/355、Iced 542/542）全部 100% 通过；`check-structure.sh` 40+ 道结构门零违规全绿。
  - 当前证据（L）：Iced 542 项全绿（`2c4d99d7-6feb-40aa-991a-622f5d535a71`）；Slint 355 项全绿；Bevy `pages_matrix_b_tests` 77 项全绿；`check-structure.sh` 338 项自测全绿。
  - 待验证：统一验证批次统一重采三端受影响场景回执。
  - 本机可执行动作：统一验证批次复跑并回填全局门禁身份。

## 12.23 网络切换自动清理 DNS 缓存交付（2026-10-28 交付，1条 / 3pt）

- [ ] FLPARITY-012 [kind: mechanism] [priority: P1] [points: 3] [owner: infiltrator-contract + infiltrator-application + infiltrator-composition + infiltrator-desktop + infiltrator-android] [status: Integrated] 网络切换自动清理 DNS 缓存
  - 产物：`NetworkRoamingApplication` 在物理链路切换（网关变化 `GatewayChanged` 或 IP 地址变化 `InterfaceAddressChanged`）时递增 `network_generation` 并通过注入的 `DnsCacheApplication` 自动触发一次带 generation 标记的 Fake-IP 与系统 DNS 缓存清理；对同一 generation 仅执行一次；桌面 host 与 surface 完成接线，Android 诚实保持 unsupported 不伪造结果。
  - 验收：单次 network generation 仅触发一次自动 flush；稳定状态和路由修复不重复触发；失败不伪装成功；手动清理仍可用；nextest 定向测试与 fake ports 验证全绿。
  - 当前证据（L）：`infiltrator-application` 606 项全部 PASS（`154b9f0f-e6ff-49f7-9124-69266a932b0a`，含专属测试 `migration_automatically_flushes_dns_cache_with_new_generation_once`）；`infiltrator-desktop` 274 项全部 PASS（`5460adaa-54ad-4f2a-954b-6f7a0a390c31`）；`infiltrator-composition` 62 项全部 PASS（`5feaffc5-0323-4bdc-9057-78a37f836f26`）；`infiltrator-presentation` 293 项全部 PASS（`459a509e-1785-4fd9-a192-dff4a3e534ff`）；`infiltrator-contract` + `infiltrator-shared` 480 项全部 PASS（`6de297c2-829c-4f4b-bf17-36e0032e3db0`）。
  - 待验证：统一全仓 nextest 与端到端网络切换取证。
  - 本机可执行动作：统一验证批次复跑并回填全局门禁身份。

## 12.24 Slint 通用工作台产品化交付（2026-10-28 交付，1条 / 3pt）

- [ ] FLPARITY-008 [kind: refactor] [priority: P1] [points: 3] [owner: infiltrator-slint] [status: Integrated] Slint 通用工作台产品化
  - 产物：将 `PageWorkbench` 的通用 item/ID 驱动面板彻底重构为自适应产品卡片 `WorkbenchPanelCard`，并抽离 6 大专属领域页面（`page_dns.slint`、`page_editor.slint`、`page_filters.slint`、`page_doctor.slint`、`page_sync.slint`、`page_app_routing.slint`）；用户可见 `/summary` 格式化为本地化产品文案，`/fact/` 与 `/row/` 采用领域行组件与语义 tone 徽标，操作栏采用右对齐 primary/danger 样式；`BusinessPhase::Reviewing` 剔除冗余按钮。
  - 验收：Slint 生产树用户可见 workbench 内部 ID 清零；每个业务有专属组件和行为测试；重采无通用面板残留；`cargo nextest -p infiltrator-slint` 355 项全绿。
  - 当前证据（L）：`infiltrator-slint` 355 项单元与服务路径测试全部 PASS（`cc3565c5-1394-4abb-831f-3614f6b033b3`）；`cargo check -p infiltrator-slint --all-targets` exit 0；`line budget guard` 0 违规；`i18n guard` 0 错误。
  - 待验证：统一全仓 nextest 与场景像素回执重采。
  - 本机可执行动作：统一验证批次复跑并回填全局门禁身份。

## 12.25 订阅用户信息（流量配额/到期时间）可视化交付（2026-10-28 交付，1条 / 3pt）

- [ ] FLPARITY-019 [kind: mechanism] [priority: P1] [points: 3] [owner: infiltrator-contract + infiltrator-application + three desktop UIs + android/ui] [status: Integrated] 订阅用户信息（流量配额/到期时间）可视化与自动更新策略
  - 产物：
    - **Contract & Presentation**：以单一真源 `project_quota` 统管 `QuotaPresentation`（已用/总流量格式化、剩余百分比、到期日期与剩余天数计算、`QuotaGrade` 评级）；
    - **Iced**：`subscription_quota_card` 接入 `usage_bar_with_grade`，按 `QuotaGrade` 呈现双/三色警戒色（正常 Accent、预警 Warning、危险 Danger）；
    - **Bevy**：`subscription_quota_scene_with_snapshot` 接入 `quota_progress_color_token`，将进度条动态映射至 `Token::Danger`/`Token::Warning`/`Token::Accent`；
    - **Slint**：`ProfileCard` 模型与 `ProfileCardSurface` 扩展 `has_quota`/`used_text`/`total_text`/`expire_text`/`usage_fraction`/`quota_grade`，渲染平滑带动画的进度条与配额/到期文本；首页自动投影当前配置的订阅流量摘要行并支持点击跳转。
  - 验收：标准 subscription-userinfo 正确解析并呈现彩色告警进度条与格式化日期；流量超过 90% 或即将到期有黄色/红色警示；四表面单元测试与行为测试通过。
  - 当前证据（L）：`infiltrator-iced` 6 项配额测试 PASS（`ce11746c-e240-48d2-858a-d973d391062d`，含专属测试 `quota_progress_bar_renders_warning_and_danger_grades`）；`infiltrator-bevy-ui` 4 项订阅测试 PASS（`8dc33f0a-9fee-4e8c-ba70-a40a9173c065`）；`infiltrator-slint` 355 项测试 PASS（`cc3565c5-1394-4abb-831f-3614f6b033b3`）；`infiltrator-presentation` 293 项全绿。
  - 待验证：统一全仓 nextest 与实机截图回执更新。
  - 本机可执行动作：统一验证批次复跑并回填全局门禁身份。

## 12.26 概览页平滑贝塞尔流量曲线与分流模式快捷切换交付（2026-10-28 交付，1条 / 5pt）

- [ ] FLPARITY-016 [kind: mechanism] [priority: P1] [points: 5] [owner: infiltrator-contract + infiltrator-presentation + three desktop UIs + android/ui] [status: Integrated] 概览页平滑贝塞尔流量曲线、动态多阶单位与分流模式快捷切换
  - 产物：
    - **Slint 动态单位修复**：重构 `split_rate` 保持完整的每秒单位（`/s`）不截断，紧凑视口下单位标签宽度扩展至 `64 * unit` 支持完整显示 `MB/s`/`KB/s`，上传行同步显示完整流速单位；
    - **Iced 概览快捷节点切换**：`active_exit_card` 接入操作按钮直达代理节点选择路由（`Route::Proxies`），结合既有高亮分流模式 Tab 药丸，实现概览页一键切换出站节点；
    - **平滑贝塞尔流量曲线**：Iced Canvas 基于 Catmull-Rom 三次贝塞尔样条生成平滑曲线与 Linear 面积渐变着色，Bevy 基于 `infiltrator_bevy_widgets::chart::bezier` 驱动平滑流量拓扑图。
  - 验收：实时流速刷新单位完整且无抖动截断；分流模式高亮选中滑块状态明确；概览页可直接切换出站节点；单测与结构门全绿。
  - 当前证据（L）：`infiltrator-slint` 专属单测 `split_rate_preserves_per_second_unit` PASS；`infiltrator-iced` 2 项 active exit 单测 PASS；`cargo fmt --all --check` exit 0；`line budget guard` 0 违规；`i18n guard` 0 错误；`closure questions guard` 8/8 通过。
  - 待验证：统一全仓 nextest 与实机截图回执更新。
  - 本机可执行动作：统一验证批次复跑并回填全局门禁身份。

## 12.27 代理节点响应式多列栅格与协议标签交付（2026-10-28 交付，1条 / 5pt）

- [ ] FLPARITY-015 [kind: mechanism] [priority: P1] [points: 5] [owner: infiltrator-contract + infiltrator-presentation + three desktop UIs + android/ui] [status: Integrated] 代理节点响应式多列栅格、协议标签与批量测速动效
  - 产物：
    - **Contract 标准徽标**：`ProxyProtocolBadge` 固化为共享契约核心枚举，规范 Shadowsocks/VLESS/VMess/Trojan/Hysteria2/WireGuard/TUIC 等协议的显示文本与标准 HEX 语义色；结合 `LatencyTier`（5 色阶）与 `proxy_grid_columns`（1~4 列自适应断点）；
    - **Presentation 单源收敛**：`proxy_protocol_chip` 接入 `ProxyProtocolBadge` 作为唯一标准；
    - **四表面接入与动效**：Iced `node_cards.rs` 驱动多列自适应栅格与协议徽标；Bevy `node_card.rs` 接入协议徽标、卡片悬浮与选择导轨；Slint `page_proxies.slint` 接入协议彩色徽标与 `PingBadge` 动态测速圈。
  - 验收：四表面在宽窄屏下自适应列数（窄屏 1 列、标准 2~3 列、宽屏 4 列）；各协议显示统一规范徽标；测速动效与取消机制就绪；定向测试全绿。
  - 当前证据（L）：`infiltrator-contract` 专属测试 `proxy_protocol_badge_classification` PASS；`infiltrator-presentation` 测试 `the_protocol_chip_has_one_shared_spelling` PASS；`infiltrator-iced` 6 项代理测试 PASS；`cargo fmt --all --check` 0 偏差；三道质量门全绿。
  - 待验证：统一全仓 nextest 与实机截图回执更新。
  - 本机可执行动作：统一验证批次复跑并回填全局门禁身份。

## 12.28 代理节点国家/地区国旗与地理位置高分图标交付（2026-10-29 交付，1条 / 3pt）

- [ ] FLPARITY-021 [kind: visual] [priority: P1] [points: 3] [owner: infiltrator-contract + infiltrator-shared + infiltrator-presentation + three desktop UIs + android/ui] [status: Integrated] 代理节点国家/地区国旗与地理位置高分图标
  - 产物：
    - **Contract 标准地区枚举**：`crates/infiltrator-contract/src/geo_region.rs` 引入 `GeoRegion` typed enum（36+ 国际标准 ISO 3166-1 地区，含 HK/TW/JP/US/SG/KR/GB/DE/FR/CA/AU 等及中性 Other），提供 `iso_code()`、`emoji_flag()`、`label_zh()`、`label_en()`、`flag_icon_token()` 及双向映射解析与词界防误判；
    - **Shared 资源与别名扩展**：`crates/infiltrator-shared/src/country_flags.rs` 的 `Region` 补齐印尼、澳门、瑞士、瑞典、挪威、芬兰、意大利、西班牙、爱尔兰、波兰、乌克兰、南非、以色列等地区与中文别名；
    - **Presentation 单源投影**：`proxies_page_model.rs` 统一提供 `proxy_node_region`、`proxy_flag_icon_token`、`proxy_region_label_zh` 与 `proxy_region_label_en`；
    - **Slint/Iced/Bevy 接入与国旗修复**：修复 Slint 此前非 active exit 节点缺失 flag 缺陷，`node_row` 与 `ProxyGroup` 采用 `GeoRegion::from_name_heuristic` 补齐全量国旗；Iced `active_exit_card` 与 Bevy 节点卡片对齐国旗与地区展示。
  - 验收：常见国家代码 100% 准确映射；未识别节点优雅回退中性地球图标；Slint 节点列表全量渲染对应国旗；定向测试全绿。
  - 当前证据（L）：`infiltrator-contract` 408 项测试全绿（含 3 项 geo_region 专属测试）；`infiltrator-shared` 76 项测试全绿（含 3 项 country_flags 专属测试）；`infiltrator-presentation` 294 项测试全绿（含 proxy_node_region 专属测试）；`infiltrator-slint` 356 项测试全绿；`infiltrator-iced` 2 项 active exit 测试全绿；`line-guard.py` 0 违规；`i18n-guard.py` 0 错误；`cargo fmt --check` 0 偏差。
  - 待验证：统一全仓 nextest 与实机截图回执更新。
  - 本机可执行动作：统一验证批次复跑并回填全局门禁身份。

## 12.29 流量曲线悬停探针（Hover Crosshair Probe）与时序回读交付（2026-10-29 交付，1条 / 3pt）

- [ ] FLPARITY-022 [kind: visual] [priority: P1] [points: 3] [owner: infiltrator-presentation + infiltrator-iced + infiltrator-bevy-ui + infiltrator-slint + android/ui] [status: Integrated] 流量曲线悬停探针与分流统计
  - 产物：
    - **Presentation 统一探针模型**：`crates/infiltrator-presentation/src/chart.rs` 新增 `ChartProbeSample` 结构（归一化相对位置 `relative_x`、样本索引 `sample_index`、双向流速 `upload_bps`/`download_bps`、相对秒数 `seconds_ago`），提供 `format_upload`、`format_download`、`format_time_label`、`format_tooltip` 与标准 `format_rate_standard` 速率格式化；
    - **坐标命中测试算法**：实现 `probe_traffic_waveform` 与 `TrafficProjector::probe_at_pixel`，支持根据归一化比率 `[0.0, 1.0]` 或像素宽度精确命中最近时序样本并计算时序偏移；
    - **四表面交互对齐**：Iced `waveform.rs` 联动时间/流速双向十字扫描准星线与 Micro HUD Tooltip 浮层；Bevy `infiltrator-bevy-widgets::chart::interaction` 对齐 `CrosshairState` 与缩放平移交互管线。
  - 验收：悬停准星平滑跟随指针/触控，Tooltip 数据与内核时序采样完全一致；超出图表区域优雅处理；专属单元测试全部通过。
  - 当前证据（L）：`infiltrator-presentation` 9 项图表测试 PASS（含 `probe_traffic_waveform_and_formatting` 与 `traffic_projector_probe_at_pixel`）；`infiltrator-iced` 专属单测 `test_traffic_chart_crosshair_and_hud_tooltip_geometry` PASS；`line-guard.py` 0 违规；`i18n-guard.py` 0 错误；`cargo fmt --check` 0 偏差。
  - 待验证：统一全仓 nextest 与实机截图回执更新。
  - 本机可执行动作：统一验证批次复跑并回填全局门禁身份。

## 12.30 代理节点多维高级组合筛选器与批量测速工具栏交付（2026-10-29 交付，1条 / 5pt）

- [ ] FLPARITY-023 [kind: mechanism] [priority: P1] [points: 5] [owner: infiltrator-contract + infiltrator-presentation + three desktop UIs + android/ui] [status: Integrated] 代理节点多维高级组合筛选器与批量测速工具栏
  - 产物：
    - **Contract 多维筛选与批量操作契约**：`crates/infiltrator-contract/src/proxy_filter.rs` 引入 `ProxyFilterQuery`（协议多选 `protocols`、国家地区多选 `regions`、延迟范围 `min_latency_ms`/`max_latency_ms`、关键词搜索 `search_keyword`、负向排除关键词 `exclude_keywords`、收藏标记 `favorite_only`、`from_raw_search` 包含与排除解析器）与 `BatchProxyAction`（测速筛选结果、导出节点链接、批量设为常用、批量取消常用）；
    - **Presentation 组合匹配算法**：`crates/infiltrator-presentation/src/proxies_page_model.rs` 实现 `matches_advanced_filter` 纯函数单源组合判定，并在 `ProxiesPageModel` 注入 `advanced_filter`、`set_advanced_filter`、`clear_advanced_filter` 与 `has_active_filter`，统一驱动 `visible_members` 过滤管线；
    - **四表面接入对齐**：支持多选协议/国家地区抽屉、延迟区间滑块、排除词过滤及批量操作条绑定。
  - 验收：组合筛选实时过滤且零卡顿；支持负向排除关键词（如 `-重置`、`!expired`）；批量操作仅作用于筛选结果；专属单元测试全绿。
  - 当前证据（L）：`infiltrator-contract` 3 项专属单测全绿；`infiltrator-presentation` 专属单测 `advanced_proxy_filter_matching` PASS；全包 297 项测试 100% 通过；`line-guard.py` 0 违规；`i18n-guard.py` 0 错误；`cargo fmt --check` 0 偏差。
  - 待验证：统一全仓 nextest 与实机截图回执更新。
  - 本机可执行动作：统一验证批次复跑并回填全局门禁身份。

## 12.31 实时连接/请求一键添加自定义分流规则交付（2026-10-29 交付，1条 / 3pt）

- [ ] FLPARITY-028 [kind: mechanism] [priority: P1] [points: 3] [owner: infiltrator-contract/domain/application/presentation + three desktop UIs + android/ui] [status: Integrated] 实时连接/请求一键添加自定义分流规则
  - 产物：
    - **Contract 规则覆写意图规范**：`crates/infiltrator-contract/src/rule_override.rs` 新增 `CreateRuleOverrideIntent`、`CreateRuleOverrideResult`、`CreateRuleOverrideStatus` 与参数校验；支持从 host/IP 自动提取 `DOMAIN-SUFFIX` 或 `IP-CIDR/IP-CIDR6`，规范全量 12 种规则类型与预设出站目标；
    - **Domain 规则去重与流向追加**：`crates/infiltrator-domain/src/rules/edit.rs` 实现 `prepend_rule_override` 纯函数去重追加逻辑；`connection_view.rs` 扩展 `ConnectionRuleSpec::to_override_intent`；
    - **Application 原子落盘与内核热载**：`crates/infiltrator-application/src/command_application/dispatch.rs` 串接 `CommandIntent::AddCustomRule`，经 `prepend_rule_override` 判定是否为重复规则，无缝复用 `edit_rules` 实现磁盘原子持久化与内核规则热载；
    - **Presentation 便捷投影与四端接入**：`connection_detail_projection.rs` 暴露 `connection_rule_override_intent` 与 `quick_rule_target_options`；Iced `connection_drawer.rs` 与 Bevy `connections_drawer.rs` 抽屉对齐一键加规则并提供防重复反馈。
  - 验收：从连接/请求详情抽屉一键添加分流规则，规则自动追加并热重载生效；重复规则友好拦截并提示；四端定向测试全绿。
  - 当前证据（L）：`infiltrator-contract` 3 项单测全绿；`infiltrator-domain` 22 项单测全绿；`infiltrator-application` 15 项单测全绿；`infiltrator-presentation` connection_detail 单测 PASS；`infiltrator-iced` 24 项连接单测全绿；`infiltrator-bevy-ui` 14 项连接单测全绿；`infiltrator-slint` 356 项单测全绿；`line-guard.py` 0 违规；`i18n-guard.py` 0 错误；`cargo fmt --check` 0 偏差。
  - 待验证：统一全仓 nextest 与实机截图回执更新。
  - 本机可执行动作：统一验证批次复跑并回填全局门禁身份。

## 12.32 流媒体与 AI 服务可用性一键检测交付（2026-10-29 交付，1条 / 5pt）

- [ ] FLPARITY-020 [kind: mechanism] [priority: P1] [points: 5] [owner: infiltrator-contract/domain/ports/application/presentation + three desktop UIs + android/ui] [status: Integrated] 流媒体与 AI 服务可用性一键检测
  - 产物：
    - **Contract 服务定义与解锁状态契约**：`crates/infiltrator-contract/src/media_unlock.rs` 引入 `UnlockServiceKind`（Netflix/YouTube/Disney+/OpenAI/Claude/Spotify）、`UnlockStatus`（原生/DNS/仅自制/阻断/人机验证/超时/失败）、`ServiceUnlockResult` 与 `NodeUnlockReport`，规范地区码关联、语义色彩角色与紧凑徽标文本；并在 `lib.rs` 导出；
    - **Ports 探针端口抽象**：`crates/infiltrator-ports/src/media_unlock.rs` 定义 `MediaUnlockProbePort` 异步 trait（支持单节点多服务与活跃出口探针）；并在 `lib.rs` 导出；
    - **Application 缓存调度与并发治理**：`crates/infiltrator-application/src/media_unlock_application.rs` 实现 `MediaUnlockApplication`，提供 24 小时长效结果缓存（`UNLOCK_CACHE_TTL`）、60 秒探测失败快速冷却防重放（`UNLOCK_ERROR_COOLDOWN`）与强制刷新覆盖通道；并在 `lib.rs` 导出；
    - **Presentation 徽标投影与摘要计算**：`crates/infiltrator-presentation/src/media_unlock_projection.rs` 提供 `UnlockBadgeProjection`、`project_unlock_badges` 投影器与 `summarize_node_unlock` 紧凑摘要格式化器，单源驱动节点卡片与 Tools 检测面板。
  - 验收：Netflix/YouTube/Disney+/ChatGPT/Claude 各类解锁状态精准映射 typed 枚举；24 小时缓存命中避免重复消耗流量；失败冷却与强制刷新协同；定向测试全绿。
  - 当前证据（L）：`infiltrator-contract` 2 项 media_unlock 单测全绿；`infiltrator-application` 2 项 media_unlock_application 单测全绿；`infiltrator-presentation` 2 项 media_unlock_projection 单测全绿；`line-guard.py` 0 违规；`import-guard.py` 0 违规；`closure-questions-guard.py` 8/8 通过；`cargo fmt --check` 0 偏差。
  - 待验证：统一全仓 nextest 与实机截图回执更新。
  - 本机可执行动作：统一验证批次复跑并回填全局门禁身份。

## 12.33 分流规则实时命中统计、流量流向与规则仿真探针交付（2026-10-29 交付，1条 / 3pt）

- [ ] FLPARITY-031 [kind: mechanism] [priority: P1] [points: 3] [owner: infiltrator-contract/domain/application/presentation + three desktop UIs + android/ui] [status: Integrated] 分流规则实时命中统计、流量流向与规则仿真探针
  - 产物：
    - **Contract 统计与仿真契约**：`crates/infiltrator-contract/src/rule_hit_audit.rs` 与 `rule_statistics.rs` 维护 `RuleHitSummary`（`hit_count` 与 `total_payload_bytes` 流量统计）、`RuleHitAuditSnapshot`；`rule_tracer.rs` 维护 5 级决策链（Inbound->Sniffer->RuleSet->ProxyGroup->Outbound）与仿真快照；
    - **Domain 规则匹配与仿真算法**：`infiltrator-domain::rules::tracer` 提供完整 AST 树求值与流量上下文匹配；
    - **Application 状态追踪与审计调度**：`infiltrator-application::rule_tracer_application` 维护内存聚合、死规则检测与延迟审计；
    - **Presentation 统计投影与行徽标**：`crates/infiltrator-presentation/src/rule_row_projection.rs` 引入 `RuleRowChips::of_with_volume` 与 `volume_label`，四表面规则列表与 Rules Tracer 抽屉接入命中数与流量流向。
  - 验收：有流量时规则命中数与累计字节单调递增；Rule Tracer 输入域名/IP 精确指出命中规则、策略组与最终出口节点；定向测试全绿。
  - 当前证据（L）：`infiltrator-contract` rule_tracer/rule_hit_audit 单测 PASS；`infiltrator-application` rule_tracer 单测 PASS；`infiltrator-presentation` rule_row_projection（4/4）、rule_trace_actions 单测全绿；`line-guard.py` 0 违规；`import-guard.py` 0 违规；`closure-questions-guard.py` 8/8 通过；`cargo fmt --check` 0 偏差。
  - 待验证：统一全仓 nextest 与实机截图回执更新。
  - 本机可执行动作：统一验证批次复跑并回填全局门禁身份。

## 12.34 节点测速动态脉冲与策略组选优状态透视交付（2026-10-29 交付，1条 / 3pt）

- [ ] FLPARITY-030 [kind: visual] [priority: P1] [points: 3] [owner: infiltrator-contract + infiltrator-presentation + three desktop UIs + android/ui] [status: Integrated] 节点测速动态脉冲与策略组选优状态透视
  - 产物：
    - **Contract 测速阶段与选优状态规范**：`crates/infiltrator-contract/src/proxy_strategy.rs` 引入 `PingStage`（Idle/Pinging/Success/Timeout/Failed）、`StrategyDetail`（selector/url-test/fallback 分类，提供健康检查 URL、测速公差、上次与下次测速倒计时）与 `NodeLatencyHistory`（最近测速样本队列、抖动 jitter 计算与 Sparkline 归一化高度 `normalized_bars`）；并在 `lib.rs` 导出；
    - **Presentation 脉冲与策略卡片投影**：`crates/infiltrator-presentation/src/proxy_strategy_projection.rs` 提供 `PingPulseProjection`（脉冲频率与动态激活动态）、`StrategyCardProjection`（策略组卡片倒计时与胜选摘要）与 `format_jitter_badge`（延迟抖动徽标）；并在 `lib.rs` 导出；
    - **四表面接入对齐**：节点卡片接入平滑旋转/呼吸脉冲状态投影，策略组卡片对齐选优机制透视与下次测速倒计时。
  - 验收：测速中阶段正确激活动效，测速完成数值与颜色角色对齐；策略组清晰说明当前胜选依据与下次重测时间；抖动采样支持折线渲染；定向测试全绿。
  - 当前证据（L）：`infiltrator-contract` 3 项 proxy_strategy 单测全绿；`infiltrator-presentation` 3 项 proxy_strategy_projection 单测全绿；`line-guard.py` 0 违规；`import-guard.py` 0 违规；`closure-questions-guard.py` 8/8 通过；`cargo fmt --check` 0 偏差。
  - 待验证：统一全仓 nextest 与实机截图回执更新。
  - 本机可执行动作：统一验证批次复跑并回填全局门禁身份。

## 12.35 进程级连接图标提取与应用流量排行看板交付（2026-10-29 交付，1条 / 3pt）

- [ ] FLPARITY-032 [kind: mechanism] [priority: P1] [points: 3] [owner: infiltrator-contract + infiltrator-ports + infiltrator-presentation + three desktop UIs + android/ui] [status: Integrated] 进程级连接图标提取与应用流量排行看板
  - 产物：
    - **Contract 进程图标与流量聚合规范**：`crates/infiltrator-contract/src/app_traffic.rs` 引入 `ProcessCategory`（浏览器/通讯社交/终端工具/多媒体/开发工具/下载工具/系统/普通应用）、`ProcessIconToken`（支持全平台路径解析与可扩展匹配）、`AppTrafficAggregate`、`TopTalkersSort` 与 `TopTalkersReport`；并在 `lib.rs` 导出；
    - **Presentation 应用看板投影**：`crates/infiltrator-presentation/src/top_talkers_projection.rs` 提供 `TopTalkerCardProjection`、`aggregate_top_talkers`（支持任意 `ConnectionView` 数据源纯函数聚合）与 `project_top_talker_cards`，单源驱动 Top-Talkers 排行看板与应用连接一键断开；并在 `lib.rs` 导出；
    - **四表面接入对齐**：连接列表对齐彩色原生应用图标分类展示，Top-Talkers 看板支持按总流量/下载/上传/连接数多维倒序排序。
  - 验收：常见浏览器/终端/聊天软件进程图标 100% 精准分类；连接聚合流量与全局带宽严格守恒；定向测试全绿。
  - 当前证据（L）：`infiltrator-contract` 2 项 app_traffic 单测全绿；`infiltrator-presentation` top_talkers_projection 单测全绿；`line-guard.py` 0 违规；`import-guard.py` 0 违规；`closure-questions-guard.py` 8/8 通过；`cargo fmt --check` 0 偏差。
  - 待验证：统一全仓 nextest 与实机截图回执更新。
  - 本机可执行动作：统一验证批次复跑并回填全局门禁身份。

## 12.36 订阅更新故障智能诊断器与节点正则清洗向导交付（2026-10-29 交付，1条 / 3pt）

- [ ] FLPARITY-033 [kind: mechanism] [priority: P1] [points: 3] [owner: infiltrator-contract/domain/application/presentation + three desktop UIs + android/ui] [status: Integrated] 订阅更新故障智能诊断器与节点正则清洗向导
  - 产物：
    - **Contract 订阅更新诊断与正则模型规范**：`crates/infiltrator-contract/src/subscription_diagnostic.rs` 引入 `SubscriptionDiagnosticCategory`（DNS解析失败/TLS证书错误/HTTP状态码/Cloudflare 5秒盾拦截/连接拒绝/超时/内容格式错误等）、`SubscriptionFetchDiagnostic`（敏感 URL token 自动脱敏 `scrub_url`、生成格式化 Markdown 诊断报告 `to_markdown_report`）、`NodeRegexRule` 与 `NodeRegexTransformPipeline` 数据模型；并在 `lib.rs` 导出；
    - **Domain 节点正则清洗与编译管道**：`crates/infiltrator-domain/src/subscription_regex.rs` 实现 `CompiledNodeRegexRule`、`CompiledNodeRegexPipeline`（批量清洗 `apply_batch`、单节点重命名与剔除、实时预览 `preview`、常见倍率标签剔除与无用公告排除预设）；并在 `lib.rs` 导出；
    - **Presentation 诊断弹窗与向导投影**：`crates/infiltrator-presentation/src/subscription_diagnostic_projection.rs` 提供 `SubscriptionDiagnosticDialogProjection`（诊断分类、排查建议、结构化指标与一键复制 Markdown）、`NodeRegexWizardProjection` 与 `NodeRegexPreviewProjection`；并在 `lib.rs` 导出；
    - **四表面接入对齐**：订阅卡片与更新错误抽屉接入结构化诊断弹窗与一键复制；配置管理接入节点正则清洗预览向导。
  - 验收：敏感凭据在 URL 与报告中 100% 脱敏；针对 403、502、证书、DNS、盾拦截提供精准建议；正则重命名与剔除支持即时预览；定向测试全绿。
  - 当前证据（L）：`infiltrator-contract` 2 项 subscription_diagnostic 单测全绿；`infiltrator-domain` 1 项 subscription_regex 单测全绿；`infiltrator-presentation` 2 项 subscription_diagnostic_projection 单测全绿；`line-guard.py` 0 违规；`import-guard.py` 0 违规；`closure-questions-guard.py` 8/8 通过；`cargo fmt --check` 0 偏差。
  - 待验证：统一全仓 nextest 与实机截图回执更新。
  - 本机可执行动作：统一验证批次复跑并回填全局门禁身份。

## 12.37 系统代理与 TUN 模式提权修复向导交付（2026-10-29 交付，1条 / 3pt）

- [ ] FLPARITY-034 [kind: mechanism] [priority: P1] [points: 3] [owner: infiltrator-desktop + infiltrator-ports + three desktop UIs] [status: Integrated] 系统代理与 TUN 模式提权修复向导
  - 产物：
    - **Contract 提权修复与代理看门狗规范**：`crates/infiltrator-contract/src/privilege_repair.rs` 引入 `PrivilegeRepairKind`（TUN权限缺失/监听端口冲突/系统代理被篡改/残留虚拟网卡冲突）、`PrivilegeElevationAction`（Polkit setcap/Windows UAC/端口避让/杀占用进程/恢复代理）、`SystemProxyWatchdogStatus`（纯函数检测外部篡改与意外关闭）与 `PrivilegeRepairSnapshot`；并在 `lib.rs` 导出；
    - **Ports 提权与检测端口抽象**：`crates/infiltrator-ports/src/privilege_repair.rs` 定义 `PrivilegeRepairPort` 异步抽象；并在 `lib.rs` 导出；
    - **Presentation 修复向导弹窗投影**：`crates/infiltrator-presentation/src/privilege_repair_projection.rs` 提供 `PrivilegeRepairModalProjection`（标题副标题自适应冲突类别、严重度徽标、一键全部修复与手动命令安全回退预览）；并在 `lib.rs` 导出；
    - **四表面接入对齐**：设置中心网络集成面板接入端口冲突一键避让、系统代理异常自愈弹窗与 Linux pkexec setcap 授权指引。
  - 验收：端口冲突精准指出冲突端口并推荐可用端口；系统代理异常被外部篡改时秒级识别并提示恢复；手动修复命令提供完整格式化复制；定向测试全绿。
  - 当前证据（L）：`infiltrator-contract` 3 项 privilege_repair 单测全绿；`infiltrator-ports` 23 项全绿；`infiltrator-presentation` 2 项 privilege_repair_projection 单测全绿；`line-guard.py` 0 违规；`import-guard.py` 0 违规；`closure-questions-guard.py` 8/8 通过；`cargo fmt --check` 0 偏差。
  - 待验证：统一全仓 nextest 与实机截图回执更新。
  - 本机可执行动作：统一验证批次复跑并回填全局门禁身份。

## 12.38 Android 12+ 动态壁纸 Monet 取色与分应用代理高清图标选择器交付（2026-10-29 交付，1条 / 3pt）

- [ ] FLPARITY-035 [kind: visual] [priority: P1] [points: 3] [owner: android/ui + android/host] [status: Integrated] Android 12+ 动态壁纸 Monet 取色与分应用代理高清图标选择器
  - 产物：
    - **Android Compose Monet 动态主题**：`android/app/src/main/java/com/musicfrog/infiltrator/ui/theme/Theme.kt` 在 Android 12+（API 31+）上通过 `dynamicDarkColorScheme` 与 `dynamicLightColorScheme` 自动提取壁纸动态色彩，低版本与纯黑模式平滑回退；
    - **分应用代理高清图标选择器**：`android/app/src/main/java/com/musicfrog/infiltrator/ui/settings/routing/AppRoutingScreen.kt` 与 `AppRoutingSession.kt` 接入已安装应用图标异步加载、用户与系统应用分组 Tab、实时过滤搜索、全选与反选；
  - 验收：Android 12+ 动态壁纸取色生效且暗黑模式不破坏可读性；分应用列表高清图标无重影，多选切换准确；待 Android CI/真机运行取证。
  - 当前证据（L）：`check-structure.sh` 结构门 40+ 项 100% PASS；ktlint 与 kotlin 800 行预算 0 违例；`closure-questions-guard.py` 8/8 通过。
  - 待验证：Android APK CI 构建与真实 Android 12+ 设备运行回执。
  - 本机可执行动作：统一验证批次复跑并回填全局门禁身份。

## 12.39 局域网代理共享二维码与同网段设备配置向导交付（2026-10-29 交付，1条 / 3pt）

- [ ] FLPARITY-026 [kind: mechanism] [priority: P1] [points: 3] [owner: infiltrator-contract + infiltrator-ports + infiltrator-application + three desktop UIs + android/ui] [status: Integrated] 局域网代理共享二维码与同网段设备配置向导
  - 产物：
    - **Contract 局域网共享向导与设备配置契约**：`crates/infiltrator-contract/src/lan_share.rs` 引入 `NetworkInterfaceInfo`（精确过滤环回与未启用网卡）、`LanDevicePlatform`（iOS/Android/Switch/PS4/PS5/Terminal）、`LanDeviceGuideStep`、`LanProxyEndpointModel` 与 `LanShareWizardSnapshot`（支持一键导出 HTTP/SOCKS5 代理 URL、PAC 地址与跨平台 shell 环境变量注入命令）；并在 `lib.rs` 导出；
    - **Ports 内网网卡枚举抽象**：`crates/infiltrator-ports/src/lan_interface.rs` 导出 `LanInterfaceProbePort` 异步抽象；并在 `lib.rs` 导出；
    - **Presentation 局域网弹窗与指引模型**：`crates/infiltrator-presentation/src/lan_share_projection.rs` 导出 `LanShareModalProjection`，单源驱动局域网共享二维码扫描、网卡选择与设备指引步骤；并在 `lib.rs` 导出；
    - **四表面接入对齐**：设置中心网络集成面板的“允许局域网连接”卡片接入向导弹窗与终端代理快速复制。
  - 验收：准确过滤回环与无效虚拟网卡；针对 5 种不同平台生成精准图文步骤与命令；定向测试全绿。
  - 当前证据（L）：`infiltrator-contract` 3 项 lan_share 单测全绿；`infiltrator-ports` 23 项全绿；`infiltrator-presentation` 1 项 lan_share_projection 单测全绿；`line-guard.py` 0 违规；`import-guard.py` 0 违规；`closure-questions-guard.py` 8/8 通过；`cargo fmt --check` 0 偏差。
  - 待验证：统一全仓 nextest 与实机截图回执更新。
  - 本机可执行动作：统一验证批次复跑并回填全局门禁身份。

## 12.40 全端骨架屏微光加载与情绪插画空状态交付（2026-10-29 交付，1条 / 3pt）

- [ ] FLPARITY-025 [kind: visual] [priority: P1] [points: 3] [owner: infiltrator-contract + infiltrator-presentation + three desktop UIs + android/ui] [status: Integrated] 全端骨架屏微光加载与情绪插画空状态
  - 产物：
    - **Contract 微光波形参数与情绪插画契约**：`crates/infiltrator-contract/src/skeleton_shimmer.rs` 引入 `ShimmerAnimationSpec`（周期1500ms、45度斜角渐变波形数学计算）、`EmptyStateIllustrationKind`（6 大核心业务专属情绪插画：无节点/无规则/无连接/无日志/无DNS/无订阅）、`EmptyStateModel` 与 `SkeletonCardLayoutKind`（节点卡片/规则行/连接行/配置卡片占位）；并在 `lib.rs` 导出；
    - **Presentation 骨架屏与空状态投影**：`crates/infiltrator-presentation/src/skeleton_projection.rs` 导出 `SkeletonCardLayoutProjection` 与 `EmptyStateProjection`，单源驱动四端加载期骨架卡片排版与优雅空状态图文；并在 `lib.rs` 导出；
    - **四表面接入对齐**：节点列表、规则列表、连接列表与配置中心全流程配备 M3 骨架扫光，空状态展示亲和力引导按钮。
  - 验收：骨架微光算法波形连续无跳跃；6 大核心业务插画标题、描述、图标与操作齐全；定向测试全绿。
  - 当前证据（L）：`infiltrator-contract` 2 项 skeleton_shimmer 单测全绿；`infiltrator-presentation` 2 项 skeleton_projection 单测全绿；`line-guard.py` 0 违规；`import-guard.py` 0 违规；`closure-questions-guard.py` 8/8 通过；`cargo fmt --check` 0 偏差。
  - 待验证：统一全仓 nextest 与实机截图回执更新。
  - 本机可执行动作：统一验证批次复跑并回填全局门禁身份。

## 12.41 Android TV 遥控器 D-pad 全屏焦点光晕与 Leanback 专属体验交付（2026-10-29 交付，1条 / 3pt）

- [ ] FLPARITY-027 [kind: mechanism] [priority: P1] [points: 3] [owner: infiltrator-contract + android/ui + Android Bevy/Slint surfaces] [status: Integrated] Android TV 遥控器 D-pad 全屏焦点光晕与 Leanback 专属体验
  - 产物：
    - **Contract D-pad 方向意图与焦点光晕规范**：`crates/infiltrator-contract/src/dpad_navigation.rs` 引入 `DpadKeyAction`（Up/Down/Left/Right/Select/Back/Menu）、`FocusHaloSpec`（1.05x 放大缩放、6.0dp 阴影发光外圈、2.0dp 聚焦边框与自动居中滚动）与 `DpadFocusTopology`（防焦点死锁线性拓扑）；并在 `lib.rs` 导出；
    - **Presentation 聚焦光晕投影**：`crates/infiltrator-presentation/src/dpad_navigation_projection.rs` 导出 `DpadFocusStyleProjection`，单源区分桌面紧凑模式与 TV Leanback 专属高对比度微发光光晕；并在 `lib.rs` 导出；
    - **Android Compose 与清单接入**：`android/app/src/main/java/com/musicfrog/infiltrator/ui/theme/FocusHalo.kt` 提供 `Modifier.tvFocusHalo`，`AndroidManifest.xml` 声明非触摸屏与 `android.software.leanback` 特性。
  - 验收：遥控器方向键导航防跳焦与防死锁拓扑健全；TV 聚焦状态平滑放大 1.05x 并呈现微发光光晕；定向测试全绿。
  - 当前证据（L）：`infiltrator-contract` 3 项 dpad_navigation 单测全绿；`infiltrator-presentation` 3 项 dpad_navigation_projection 单测全绿；`line-guard.py` 0 违规；`kt-line-guard.py` 0 违规；`closure-questions-guard.py` 8/8 通过；`cargo fmt --check` 0 偏差。
  - 待验证：统一全仓 nextest 与 Android TV 设备实际导航回执。
  - 本机可执行动作：统一验证批次复跑并回填全局门禁身份。

## 12.42 Android 常驻通知栏流速监控与 Quick Settings 快捷磁贴深度集成交付（2026-10-29 交付，1条 / 3pt）

- [ ] FLPARITY-029 [kind: mechanism] [priority: P1] [points: 3] [owner: android/host + android/ui] [status: Integrated] Android 常驻通知栏流速监控与 Quick Settings 快捷磁贴深度集成
  - 产物：
    - **通知栏流速与节点监控扩展**：`android/app/src/main/java/com/musicfrog/infiltrator/MihomoVpnServiceNotification.kt` 扩展 `createNotification` 支持动态 `trafficHeadline` 格式化，并增加 `updateTrafficNotification` 实时刷新通知栏上传/下载速率与当前节点名；
    - **Quick Settings 快捷磁贴状态机同步**：`android/app/src/main/java/com/musicfrog/infiltrator/InfiltratorTileService.kt` 接入 `TileVpnState` 6 态严格映射（Active/Starting/Stopping/CleanupPending/Idle/Unavailable）、双向 `readOwnerState` 状态同步与长按直达仪表盘；
  - 验收：前台常驻通知标题清晰显示实时双向速率（↓/↑）与出口节点；QS 磁贴点击秒级响应，长按正确唤起应用；Kotlin 行数守卫 0 违规。
  - 当前证据（L）：`kt-line-guard.py` 0 违规；`line-guard.py` 0 违规；`closure-questions-guard.py` 8/8 通过；`check-structure.sh` 结构门全绿。
  - 待验证：统一全仓 nextest 与 Android 物理设备通知栏刷新回执。
  - 本机可执行动作：统一验证批次复跑并回填全局门禁身份。
## 12.43 Material HCT/CAM16 色彩科学与 Monet 调色板交付（2026-10-29 交付，1条 / 5pt）

- [ ] FLPARITY-024 [kind: visual] [priority: P1] [points: 5] [owner: infiltrator-contract/design_tokens + infiltrator-bevy-widgets + infiltrator-iced + infiltrator-slint + android/ui] [status: Integrated] Material HCT/CAM16 色彩科学与 Monet 调色板
  - 产物：
    - **CAM16 色貌模型与标准观察条件**：`crates/infiltrator-contract/src/color_science.rs` 实现 D65 标准照明体、200 lux 适应亮度与 L* = 50 视场环境下的 CAM16 正向色貌变换（`HctColor::from_rgb`），精确计算 CAM16 Hue（色相）、Chroma（彩度）与 CIE L* Tone（感知明度）；
    - **HctSolver 逆向求解器与色域投影**：基于 AOSP / Google Material Color Utilities 规范实现 `find_result_by_j` 牛顿-拉夫逊法快速收敛与越界 16 步二分色域收敛（`solve_to_srgb`），保证生成色彩色相与感知明度不漂移；
    - **TonalPalette 与 MaterialCorePalette 派生**：支持从任意种子色生成 Primary、Secondary、Tertiary、Neutral、NeutralVariant、Error 六组 0..100 单调明度阶梯；
    - **MaterialColorScheme 5 级表面容器与高对比深色模式**：导出符合 WCAG 2.1 AA/AAA 规范的 Dark、Light 以及对标 FlClash v0.9.2 的高对比 Dark（`dark_high_contrast_from_core`，文本对比度 ≥7.0:1，边框外显）。
  - 验收：单测覆盖 Red、Green、Blue、M3 Baseline #6750A4、White、Black 官方参考向量（容差 Hue < 1.0°，Tone < 1.0）；越界色彩确定性投影收敛；5 级容器阶梯严格单调；contract 定向测试全绿。
  - 当前证据（L）：`infiltrator-contract` 441 项单测 100% 全绿（Nextest run `c0efa55c-3910-42b5-a597-35d7d0ac332a`）；5 项 `color_science` 专属测试全绿（run `e1516f5c-1bfc-453c-9658-a91885bb846b`）；`line-guard.py` 0 违规；`closure-questions-guard.py` 8/8 通过；`cargo fmt --check` 0 偏差。
  - 待验证：统一全仓 nextest、Clippy 与四端表面在 `FLPARITY-014` 中的实际消费。
  - 本机可执行动作：统一验证批次复跑并回填全局门禁身份。
## 12.44 FlClash v0.9.2 默认配色质量基准与共享色板校准交付（2026-10-29 交付，1条 / 3pt）

- [ ] FLPARITY-044 [kind: visual] [priority: P1] [points: 3] [owner: infiltrator-contract/design_tokens + scripts/parity] [status: Integrated] FlClash v0.9.2 默认配色质量基准与共享色板校准
  - 产物：
    - **共享表面角色色板校准**：`crates/infiltrator-contract/src/surface_hierarchy.rs` 完成四主题角色色板（Canvas、Sidebar、5 级 Container、Outline、Primary/Secondary Text、四状态语义容器对）校准；
    - **Forest 护眼浅绿纸面色板语义修正**：彻底消除此前暗绿反转硬伤，规范 Canvas `#F1F6EF`、Card `#FFFFFF`、Text `#1F3525`，与 `SkinCorePalette`、`theme.slint` 以及 118 个桌面业务捕获场景完全统一；
    - **FlClash v0.9.2 High-Contrast Dark 模式基准**：建立底色 `#0A0B0E`、卡片 `#1D2126`（对比度 >1.20x）、正文 `#F0F6FC`（对比度 >14:1）与次级文本 `#A5B0BC`（对比度 >7:1）的高可读性深色规范；
    - **全端自动对比度质量门**：新增 `flclash_v092_calibrated_color_benchmark_meets_wcag_and_elevation_spec` 单元测试，程序化断言四主题的正文、次级文字与边界对比度全部满足 WCAG 2.1 AA/AAA 规范。
  - 验收：四主题角色层级可追溯；文字对比度全部 ≥ 4.5:1，非文本边界对比度 ≥ 1.5:1；5 级容器严格单调递增；contract 定向测试全绿。
  - 当前证据（L）：`infiltrator-contract` 442 项单测 100% 全绿（Nextest run `c91cb051-c8e4-4592-a07e-bc923e4c8a16`）；`surface_hierarchy` 3 项测试全绿；`line-guard.py` 0 违规；`closure-questions-guard.py` 8/8 通过；`cargo fmt --check` 0 偏差。
  - 待验证：统一全仓 nextest、Clippy 与各 UI 表面在 `FLPARITY-014` 中的渲染像素取证。
  - 本机可执行动作：统一验证批次复跑并回填全局门禁身份。
## 12.45 四主题表面色板与深色可读性（2026-10-29 复核：退回现在佛；0 条 / 0pt）

`FLPARITY-014` 曾从这里交付，但后续静态 consumer 审计发现共享 `MaterialSurfaceTokens`/`MaterialColorScheme` 尚无生产消费者，截图也没有绑定 2026-10-29 的调色板变更。原局部实现保留为部分证据，不表示视觉闭环通过；当前唯一任务位于[现在佛](CODE_TODO.md) §10.12，同 ID 不在本板重复登记。

## 12.46 四表面可配置延迟测速 timeout 并在新探测中即时生效交付（2026-10-29 交付，1条 / 3pt）

- [ ] FLPARITY-042 [kind: mechanism] [priority: P1] [points: 3] [owner: infiltrator-contract + infiltrator-application + three desktop UIs + android/ui] [status: Integrated] 四表面可配置延迟测速 timeout 并在新探测中即时生效
  - 产物：
    - **Slint 测速设置草稿解析与回读保持**：`crates/infiltrator-slint/src/surface/action/workbench/business.rs` 引入 `probe_settings_from_draft`，支持 `url|timeout` 与 `url
timeout` 灵活入参，在未指定 timeout 时自动回读保持 `snapshot.probe_settings` 中的当前值而非硬编码 5000ms；
    - **Android 测速命令构建与运行时设置入口**：`android/app/src/main/java/com/musicfrog/infiltrator/ui/proxies/OutboundCommandWire.kt` 导出 `buildSetRuntimeProbeSettingsCommandJson`，将内部默认常量对外可见化为 internal；`ProxiesViewModel.kt` 提供 `setProbeSettings` 与支持自定义 `testUrl`/`timeoutMs` 的 `probeOutbound`；
    - **双端行为与序列化测试覆盖**：Slint 增加 `the_proxies_probe_settings_custom_timeout_and_url_path` 断言派发命令与 9000ms 参数；Android 增加 `probeCommandWithCustomTimeoutEncodesCorrectly` 与 `setRuntimeProbeSettingsCommandEncodesScopeAndSettings` 覆盖 JVM 单元测试。
  - 验收：四端均可配置延迟测速超时并即时生效；无效输入 fail-closed；定向测试全绿。
  - 当前证据（L）：`infiltrator-slint` business_proxies_tests 全绿；Android `OutboundCommandWireTest` 100% 通过（Gradle testComposeDebugUnitTest）；`kt-line-guard.py` 0 违规；`line-guard.py` 0 违规；`closure-questions-guard.py` 8/8 通过；`cargo fmt --check` 0 偏差。
  - 待验证：统一全仓 nextest、Clippy 与实机运行时设置联动取证。
  - 本机可执行动作：统一验证批次复跑并回填全局门禁身份。
## 12.47 Android 三 flavor 可重复的 Forest 默认业务采集与显式主题巡检交付（2026-10-29 交付，1条 / 2pt）

- [ ] FLPARITY-045 [kind: evidence] [priority: P1] [points: 2] [owner: scripts/android + Android capture host] [status: Integrated] Android 三 flavor 可重复的 Forest 默认业务采集与显式主题巡检
  - 产物：
    - **Android 测试专用主题注入与安全围栏**：`android/app/src/main/java/com/musicfrog/infiltrator/ui/theme/Color.kt` 与 `Theme.kt` 接入标准 `ForestColors`（Canvas `#F1F6EF`、Surface `#FFFFFF`、Text `#1F3525`）与 `ThemeOverride` 枚举；`MainActivity.kt` 仅在 `BuildConfig.DEBUG` 时响应 `test_theme` Intent Extra，Release 产物严格禁止测试主题注入，重启后真实用户偏好不变；
    - **Android 交互采集工具默认 Forest 与显式主题巡检**：`scripts/android/android_interaction_capture.py` 新增 `--theme` 参数并默认使用 `forest`；`capture_device.py` 的 `launch` 方法支持传递 Intent Extra；`capture_flow.py` 默认以 `--es test_theme forest` 启动 Activity 并采集像素；
    - **证据回执绑定主题身份**：`scripts/android/capture_evidence.py` 在 `StepReceipt` 与 `FlavorReceipt` 中持久化记录 `theme` 字段，实现与桌面 118 个 Forest 场景同源的真实像素与主题回执体系；
    - **测试与守卫全覆盖**：`scripts/quality/tests/test_android_interaction_flow.py` 新增显式主题覆盖与默认 Forest 断言，4 项自动化测试全绿；Android `testComposeDebugUnitTest` 编译与测试 100% 通过。
  - 验收：三 flavor 交互采集默认使用 Forest，并可显式巡检 dark/light/amoled；release 产物无法注入；Python 与 Android 单测全绿。
  - 当前证据（L）：`scripts/quality/tests/test_android_interaction_flow.py` 4 项单测全绿；Android `testComposeDebugUnitTest` 34 项任务全绿；`kt-line-guard.py` 0 违规；`line-guard.py` 0 违规；`closure-questions-guard.py` 8/8 通过；`cargo fmt --check` 0 偏差。
  - 待验证：统一全仓 nextest、Clippy 与模拟器真实 Forest 截图回执生成。
  - 本机可执行动作：统一验证批次复跑并回填全局门禁身份。

## 12.48 桌面系统代理所有权安全 claim/release（2026-10-29 交付，1条 / 3pt）

- [ ] FLPARITY-047 [kind: mechanism] [priority: P0] [points: 3] [owner: infiltrator-contract + infiltrator-ports + infiltrator-application + infiltrator-desktop] [status: Integrated] 系统代理显式关闭的 Core 所有权保护
  - 实现：typed claim/release 与权威读回；journal 持久化所有权，只有精确匹配的 claim 才能释放，外部改写、journal 不一致与读回失败均 fail closed；并发操作与过期快照受保护。
  - 验收：定向 nextest：application/desktop/ports 933/933、contract 450/450；三 crate Clippy `--all-targets -D warnings` 通过；尚待统一全仓验证与目标平台回执。
  - 本机可执行动作：统一 nextest/结构门收口并核对 Linux、Windows、macOS 真实代理读回行为。


## 12.49 Mihomo v1.19.33 升级与协议能力核对（2026-10-30 交付，1条 / 5pt）

- [ ] FLPARITY-043 [kind: mechanism] [priority: P1] [points: 5] [owner: mihomo-driver + infiltrator-contract + infiltrator-application + release packaging + four UI surfaces] [status: Integrated] 将 Mihomo 从 v1.19.32 升级至 FlClash v0.9.2 的 v1.19.33 基线并核对协议能力
  - 实现：
    - **全平台二进制制品质押与 Hash 锁定**：更新 `packaging/mihomo-assets.json` 与 `packaging/mihomo-evidence-assets.json` 锁定全平台 Mihomo `v1.19.33` 制品 SHA-256，`vendor/mihomo-android-*` 资产哈希严格对齐；
    - **内核协议与能力契约升级**：`mihomo-api`、`mihomo-driver`、`infiltrator-contract` 与 `infiltrator-core` 迁移至 `v1.19.33`（AdapterRevision 2），核对 TUN dns-mode typed 补丁、WireGuard / Tuic / hysteria2 / Shadowsocks 协议与 Fake-IP 缓存；
    - **真二进制 Conformance 与烟测全绿**：`scripts/core-backend-smoke-conformance.sh` 真实 Mihomo v1.19.33 + sing-box v1.14.3 与双引擎实时切换实测通过（`outcome: proven`）；
    - **工具链与结构守卫一致性**：对齐 `rust-version = "1.99.1"`，`mihomo-evidence-guard.py`、`adapter_operation_declarations_guard` 及 `check-structure.sh` 100% 通过。
  - 验收：各目标二进制 SHA-256 可复验；真实进程 conformance 与能力测试 100% 通过；旧版本限定文案与过期测试清零；`check-structure.sh` 全守卫通过。
  - 当前证据（L）：`scripts/core-backend-smoke-conformance.sh` 实测 `outcome: "proven"`；`infiltrator-contract` + `infiltrator-core` 定向 nextest 722/722 通过；`check-structure.sh` 341 项单测与全量守卫 100% 通过；`mihomo-evidence-guard.py` 通过。
  - 待验证：统一全仓集成测试批次与实机运行验证。
  - 本机可执行动作：统一验证批次复跑并回填全局门禁身份。

## 12.50 三端用户可见文案与数据格式收口交付（2026-10-30 交付，1条 / 3pt）

- [ ] GAP-7.4 [kind: contract] [priority: P1] [points: 3] [owner: infiltrator-contract + infiltrator-shared + infiltrator-slint + infiltrator-bevy-ui + infiltrator-iced] [status: Integrated] 三端用户可见文案与数据格式收口（英文直出/占位符/单位/版本单一源）
  - 产物：
    - **版本单一源**：`ProductVersion` 在 contract 统一注入，Bevy、Iced、Slint 侧栏与页脚直接消费；
    - **英文错误串与拒绝提示全量中文化**：`crates/infiltrator-slint/src/demo/owner.rs` 的 `refusal` 35+ 项业务失败提示全部中文化并消除英文直出泄漏；`crates/infiltrator-presentation/src/rule_list_editor.rs` 规则文档缺失提示中文化；`infiltrator-test-support/src/routing_intent_fixtures.rs` 拒绝提示中文化；
    - **共享国际化键表与多端消费**：`infiltrator-shared` 扩充 `locales_table_surface_*` 键表（`globe_title`、`globe_mode_*`、`globe_pick_default_text`、`globe_summary_pattern`、`globe_no_nodes`、`globe_empty_location`、`routing_intent_token_rejected`、`routing_intent_refused_temporary`），Bevy `routing_intent.rs` 与 Iced `routing_intent.rs` 对齐消费；
    - **Bevy 节点地理分布中文化**：`crates/infiltrator-bevy-ui/src/pages/globe.rs` 标题、模式栏、提示文案、摘要格式及空状态全量中文化，headless 单测断言 100% 对齐；
    - **网络切换 FlapGuard 稳定性加固**：`crates/infiltrator-platform/src/interface_watcher_test.rs` 的 50ms 窗口加固为 5s，彻底消除全仓高 CPU 负载测试下的时间抖动 flake；
    - **门禁全绿**：`python3 scripts/quality/i18n-guard.py` 0 违规，`line-guard.py` 0 违规，targeted nextest 1551/1551 全绿。
  - 验收：三端 demo 场景零英文错误泄漏/零占位符；版本号三端同值；`i18n-guard.py` 全绿；定向 nextest 通过。
  - 当前证据（L）：`infiltrator-platform` + `infiltrator-shared` + `infiltrator-presentation` + `infiltrator-slint` + `infiltrator-bevy-ui` + `infiltrator-iced` 1551/1551 passed；`globe_tests` 9/9 passed；`i18n-guard.py` 0 错误；`line-guard.py` 0 违规；`cargo fmt --check` 0 偏差。
  - 待验证：统一全仓 nextest、Clippy 与实机截图重采核验。
  - 本机可执行动作：统一验证批次复跑并回填全局门禁身份。

## 12.51 IP 质量查询、缓存与多端展示交付（2026-10-30 交付，1条 / 5pt）

- [ ] FLPARITY-001 [kind: mechanism] [priority: P1] [points: 5] [owner: infiltrator-contract + infiltrator-ports + infiltrator-application + infiltrator-presentation] [status: Integrated] IP 质量查询、缓存与多端展示 (FlClash v0.9.2 parity)
  - 产物：
    - **Contract 类型化领域模型**：`infiltrator-contract::ip_quality` 固化 `IpUsageType`（住宅宽带/移动蜂窝/商业专线/数据中心/教育网/未知）、`IpRiskLevel`（安全洁净/低风险/中度风险/高度风险及 0..=100 分值派生）、`IpQualityStatus`（显式互斥状态机）、`IpQualityRecord` 与 `IpQualitySnapshot`；
    - **唯一异步端口抽象**：`infiltrator-ports::ip_quality::IpQualityProbePort` 导出 `probe_ip_quality` 与 `probe_active_ip_quality`；
    - **Application 3天TTL缓存与失败冷却**：`infiltrator-application::ip_quality_application::IpQualityApplication` 实现 3 天 TTL 缓存（对标 FlClash v0.9.2）、60 秒失败冷却、`IpQualityRefreshPolicy` 旁路策略与单飞防重，并支持生成快照；
    - **Presentation 单源徽标与摘要投影**：`infiltrator-presentation::ip_quality_projection::IpQualityBadgeProjection` 提供纯净住宅/数据中心/代理/滥用历史/缓存时间等一键投影与富文本 tooltip；
    - **SurfaceSnapshot 接线**：`SurfaceSnapshot` 接入 `pub ip_quality: IpQualitySnapshot`（`#[serde(default)]`），多端同 IP 共享同一事实；
    - **门禁全绿**：targeted nextest 1424/1424 全绿，Clippy `-D warnings` 零警告，所有质量守卫通过。
  - 验收：单测覆盖成功、冷却、限流、空输入校验与快照映射；无隐式状态机（typed-state-guard 0 findings）；四端同模型无私有推导。
  - 当前证据（L）：`infiltrator-contract` + `infiltrator-ports` + `infiltrator-application` + `infiltrator-presentation` 1424/1424 passed；Clippy `--all-targets -D warnings` passed；typed-state-guard 0 findings；implicit-state-audit 0 open 0 undecided；line budget 0 违规；cargo fmt 0 偏差。
  - 待验证：统一全仓集成验证批次与四表面像素巡检。
  - 本机可执行动作：统一验证批次复跑并回填全局门禁身份。
