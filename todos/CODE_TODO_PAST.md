# MusicFrog 工程 TODO · 过去佛（已实现待验证与收口）

> 层级：L4

## 0. 编写规则

- 本文件里的工作仍未办结：已有实现包等待验证批次、正式产品采用、最终证据复验、模拟器或真机取证。已有局部 PASS 不自动变成完整验收。
- **纯粹三佛体系（无体外看板）：**模拟器（`-M`）与 ARM64 真机（`-D`）实机验收条件直接内聚在对应条目的“待验证”中；不设外部 `MACHINE_TODO` 孤岛看板，一处看清代码实现、单元测试与实机验收全貌。
- 每次先读 task 的验收、依赖与当前证据身份，执行最低充分验证；前置未满足保持 Blocked，验证发现实现缺口则把修复任务回流[现在佛](CODE_TODO.md)或[未来佛](CODE_TODO_FUTURE.md)。凡发现底层架构未决或在上层修补规避底层问题的条目，不得标为 Integrated 或通过验证，必须将修复与底层裁决任务回流现在佛。
- 验证收口通过后，把条目从看板删除并在 [TODO.md](../TODO.md) 登记归档；不在本文件长期驻留已验证成果。
- `Integrated` 表示实现包已进入工作区、只剩验证/收口；位置不授予验证结论。若仅缺设备/真机证据，在待验证字段中明确标注，本条保持 Integrated 并注明设备依赖；只有实现依赖任务尚未完成时才标 Blocked。外部不可解条件（无真机、等 tag CI、等用户 secrets）不阻塞统一验证批次对其它条目的批量解锁。
- 迁移必须同次移出/移入，保持稳定 task ID、owner、依赖、验收与 points。
- 当前证据行只保留 run ID 与计数；完整证据文本见 `.evidence/CODE_TODO_PAST_EVIDENCE.md`（本地，不入库）。

## 一、Engine-Neutral Core、契约与呈现投影迁移（底层契约与 Presentation 解耦，3条 / 9pt）

- [ ] BANDROID-041.3.2R.15 [kind: mechanism] [priority: P1] [points: 5] [owner: protocol contract + presentation + Application] [status: Integrated] Protocol editor model split from persistence use case
  - 产物：Protocol form/draft/preview projection 归 presentation；Profile 写入继续由 Application use case 接收 typed intent，UI 不直接调用持久化方法。
  - 依赖：BANDROID-041.3.2R.13、.14.1～.14.7；ProfileDocument codec contract；Core-first 阶段门。
  - 验收：旧 Application protocol-form presentation exports 与 UI imports 清零；编码 round-trip、拒绝/取消/保存终态由 fake/nextest 覆盖。
  - 当前证据：`5a7db497-6576-436c-a437-b73695098f8b`；790 passed；5116/5116。
  - 待验证：[ARM64真机] 实际设备运行取证（当前暂无 ARM64 设备，保持开放）。
- [ ] CORE-007.2 [kind: mechanism] [priority: P0] [points: 2] [owner: infiltrator-android IPC adapter + :vpn owner] [status: Integrated] Android 消费者 scope fence 与 Binder readback/adopt
  - 产物：Binder death/rebind 后先 readback/adopt Core 与 VPN 双状态再接命令；过期 generation/session 回执不改写新 owner 状态。
  - 依赖：BANDROID-040.2/040.4 typed IPC；BANDROID-040.5 同窗回归。
  - 验收：Rust fake bridge 与 Kotlin fake transport 覆盖双 readback 顺序、Unavailable/stale 无副作用；nextest 与 Core host JVM 批次通过。
  - 当前证据：`f9d6be76-6996-495b-9b78-b5181ef1046a`；3100 passed；5116 passed；13/13。
  - 待验证：[ARM64真机] 真实 Binder death/rebind 设备取证（当前用户确认暂无 ARM64 设备，保持未验证）。
- [ ] BANDROID-041.3.3 [kind: mechanism] [priority: P1] [points: 2] [owner: android/bevy-host driver] [status: Integrated] Android Bevy product dependency closure
  - 产物：driver 仅装配 NativeActivity、typed `AndroidOwnerClient`、Bevy UI/widgets/contract 与 system-host ports/Insets/IME adapter；capture-only Application 路径排除。
  - 依赖：BANDROID-041.3.2A、.3.2R.13、.14.1～.14.7、.15。
  - 验收：移除 `ui-application-projections`；locked Android target tree 无 application/composition/mihomo-api/platform；UI package nextest 留 CORE-UI-002。
  - 当前证据：`d357a7f7-c103-4a0f-bec1-01ccb9a9a518`；795 passed；5116/5116；6/6。
  - 待验证：[ARM64真机] 最终 driver `.so`/APK ELF 设备验证（当前暂无 ARM64 设备）。

## 二、Android Owner、VPN 生命周期与核心事务（底层内核与系统服务，8条 / 26pt）

- [ ] BANDROID-002.1 [kind: mechanism] [priority: P0] [points: 3] [owner: Android VpnService + :vpn owner + CoreApplication] [status: Integrated] system-managed VPN startup and active-profile restore
  - 产物：显式 Start/Stop 与无命令启动由 sealed policy 区分；Android always-on 恢复唯一 `:vpn` owner 后复用 `StartProxy` 和当前 active profile；系统托管通知不提供本地 Stop；普通 ControlService sticky 重建不启动 Core/proxy。
  - 依赖：BANDROID-001.2、BANDROID-040.3/.7、CORE-015.7/.8.3。
  - 验收：API 24–28 legacy unflagged policy、API 29+ `isAlwaysOn`、malformed intent、always-on disabled、owner init failure/timeout 有 typed JVM 测试；StartProxy 共享行为测试通过。
  - 当前证据：`3162c1d0-88e1-491e-9764-b53f260232a9`；666 passed；52 passed。
  - 待验证：
    1. [模拟器 M1] 在 Compose/Bevy/Slint 三 flavor 各自安装同一候选源码 APK，启用 always-on，重启或 kill `:vpn` 确认系统恢复、active profile 配置与托管通知无本地 Stop；仅重建 ControlService 确认 Core/proxy 保持暂停。关闭条件：绑定 APK SHA、镜像、service/core PID、owner/session/profile readback 与 logcat。
    2. [ARM64真机 D] 在 ARM64 Android 29+ 验证上述同等场景与真实连通性。当前暂无 ARM64 设备，保持开放。
- [ ] BANDROID-002.2 [kind: validation] [priority: P0] [points: 3] [owner: :vpn host + NetworkRoamingApplication] [status: Integrated] 网络切换与服务终态回归
  - 产物：NetworkRoaming 初次/已观测基线和网关/地址变化以显式 enum 表达；`reconcile` 操作用 typed intent；TUN 未报告与已关闭分开处理，修复失败或关键观测缺失时保留旧基线以便重试。系统默认 underlay 交给 OS；VPN owner UID 从自身 TUN 排除；Stop/Revoke 清理 worker/FD/WakeLock。
  - 依赖：BANDROID-002.4；BANDROID-040.7 owner snapshot。
  - 验收：fake 覆盖初次观察、稳定/地址变化/网关迁移、自动修复失败与未报告 TUN 的重试、owner UID 排除、Stop/Revoke、WakeLock 与迟到终态。
  - 当前证据：`f9d6be76-6996-495b-9b78-b5181ef1046a`；`60d3f0c2-1571-4f55-8b61-37bc62b03c34`；3100 passed；11/11；183/183。
  - 待验证：
    1. [模拟器 M1] 安装含 002.4 的候选 APK，在流量中切换 Wi-Fi/断网/恢复，执行 Stop/Revoke，记录默认网络、VPN/Core snapshot、TUN/worker/FD 终态及 WakeLock 事件。关闭条件：APK SHA、切换前后流量与资源清理回执。
    2. [ARM64真机 D] ARM64 实机真实流量、Wi-Fi/蜂窝/断网/恢复、锁屏超时 WakeLock 释放。当前暂无 ARM64 设备，保持开放。
- [ ] BANDROID-002.4 [kind: mechanism] [priority: P0] [points: 2] [owner: Android MihomoVpnService] [status: Integrated] VPN lifecycle bounded WakeLock
  - 产物：仅在 Start/Cleanup transaction 持有有时限 partial WakeLock；typed lease 绑定 generation/deadline，终态及时释放，系统 timeout 作兜底，空闲期不持有。
  - 依赖：BANDROID-002.1 VPN lifecycle admission。
  - 验收：fake handle 覆盖 acquire/join/expiry/release failure/stale generation；host integration 确认 Start 与 Stop/Revoke task 的 finally 收口；Manifest 声明由 contract guard 覆盖。
  - 当前证据：178/178；11/11。
  - 待验证：[ARM64真机] 最终 APK 安装与真机实际功耗样本（当前暂无 ARM64 设备）。
- [ ] BANDROID-005.1 [kind: mechanism] [priority: P0] [points: 3] [owner: Android 原生窗口 adapter] [status: Integrated] AppLifecycle/Surface 重建 typed 状态
  - 产物：接入真实 Activity phase、Surface 暂停/恢复、窗口焦点及 generation fence；Compose sealed route 跨 Activity 重建保存；UI 销毁只释放观察，导入 URL 继续以 Android Intent 为源。
  - 依赖：BANDROID-003、004。
  - 验收：Rust/Kotlin 生命周期迁移与 wire-code 合成测试通过；AppRoute 全状态保存/恢复测试通过。
  - 当前证据：192 passed；194/194；3/3。
  - 待验证：[ARM64真机] 锁屏、后台、旋转、分屏 ≥100 次循环（无 surface panic、无重复 VPN 命令、内存曲线平稳）。当前暂无 ARM64 设备，保持开放。
- [ ] BANDROID-040.5 [kind: mechanism] [priority: P0] [points: 2] [owner: android IPC adapter] [status: Integrated] Binder death/rebind 与双状态 readback（含 BANDROID-001.2）
  - 产物：新 Binder session 放行命令前必须读回 Core 与 VPN state/generation；旧 request、session、Core/VPN generation 与乱序终态不能改写当前 owner 快照。
  - 依赖：BANDROID-040.2/040.4 typed IPC；BANDROID-042.7 typed stop IPC。
  - 验收：Rust fake bridge 与 Kotlin fake transport 覆盖 death/restart/乱序/Unavailable/stale/no-side-effect；nextest 与 Core host JVM 批次通过。
  - 当前证据：`f9d6be76-6996-495b-9b78-b5181ef1046a`；`cc3c0959-7e30-467e-a1a5-15eca68fca48`；`bd01251d-c13c-4bf7-a8d5-f316a852c7f4`；3100 passed；2673/6；13/13。
  - 待验证：[ARM64真机] 实际 APK/设备核对 Compose/Bevy/service 进程与 session，两套 `.so` 链接边界（当前暂无 ARM64 设备）。
- [ ] BANDROID-040.7 [kind: mechanism] [priority: P0] [points: 5] [owner: :vpn owner + contract + Android surface adapters] [status: Integrated] owner-authored VPN snapshot
  - 产物：`:vpn` 单次发布 typed `VpnOwnerSnapshot`，含完整 `CoreLifecycleSnapshot` 与 typed failure；Android `SurfaceSnapshot.vpn` 原样承载 owner observation；Compose/Bevy/Slint 读取同一 owner snapshot；旧 integer reply、Android 合成 UI model 与进程本地 revision/cache 入口已退役。
  - 依赖：BANDROID-040.5；BANDROID-042.7 owner-first stop。
  - 验收：Rust/Kotlin 负测与 roundtrip 通过。
  - 当前证据：`f9d6be76-6996-495b-9b78-b5181ef1046a`；`cc3c0959-7e30-467e-a1a5-15eca68fca48`；`bd01251d-c13c-4bf7-a8d5-f316a852c7f4`；3100 passed；2673/6；13/13。
  - 待验证：[ARM64真机] Compose/Bevy/Slint 三端同 revision 设备记录；owner 重绑先 readback/adopt 的设备观察（当前暂无 ARM64 设备）。
- [ ] BANDROID-040.8 [kind: mechanism] [priority: P0] [points: 5] [owner: Android :vpn typed IPC + Rust credential owner] [status: Integrated] Age credential owner operation with sensitive transport
  - 产物：新增 scope/selection/revision-fenced Age credential read/set/clear owner operations；密钥经一次性 typed sensitive payload 传输，不进入 JSON command, snapshot, reply, log or profile state；Rust 在 owner 侧构造并清理 `AgePrivateKey`，仅返回非敏感状态/revision readback。
  - 依赖：BANDROID-040.2/.3/.5/.7；CORE-020.1/.2；SLINT-003。
  - 验收：fake/nextest 与 Kotlin host tests 覆盖 wrong scope、stale selection/revision、owner death/cancel、无效/超长 key、readback 与零 secret 泄漏；旧 Age unsupported consumer path 由 SLINT-014 迁移，package verification 留 CORE-UI-002。
  - 当前证据：`254253dc-4c7a-4a20-bccc-171b0a30664b`；`d4df4dfc-df6f-4fe4-a059-dcb58ed06b78`；`29bd657b-9053-426d-b976-267835be958f`；1271 passed；191 passed；1226 passed。
  - 待验证：[ARM64真机] Compose APK 提交 Age 凭据 read/set/clear，确认双进程经 Binder 获得匹配 readback，无 secret 泄露；Binder timeout 与重绑测试（当前暂无 ARM64 设备）。
- [ ] BANDROID-042.5c [kind: mechanism] [priority: P0] [points: 3] [owner: :vpn owner + Android bridge + Mihomo validator] [status: Integrated] 暂停 Core 时的原生配置校验
  - 产物：`:vpn` owner 对临时候选调用同 ABI Mihomo `-t`；请求绑定 workspace/launch identity、候选 digest、request ID 与有界 budget；owner 收到请求后建立 elapsedRealtime deadline；不启动 Core、不写 active workspace。
  - 依赖：CORE-013.4、BANDROID-040.3/.7。
  - 验收：Stopped/Failed/Running 的 foreign/stale/native-invalid 均零写入；receipt identity/digest 精确匹配；超时清理子进程和候选；缺 binary/owner IPC typed fail closed。
  - 当前证据：`f4e0ff30-a311-4ac5-90fa-221a77e35d8d`；`a62d1717-5d4d-4c2a-80cc-55fc803b6dad`；15/15；234 passed；183/183。
  - 待验证：[ARM64真机] 安装同 ABI 候选 APK，在 Core Stopped 提交有效/无效配置，确认 `-t` 子进程与回执精确归档（当前暂无 ARM64 设备）。

## 三、Android IPC 与 Kotlin Host 边界（跨进程通信与适配层，4条 / 9pt）

- [ ] BANDROID-040.4 [kind: validation] [priority: P0] [points: 2] [owner: android/bevy-host driver] [status: Integrated] Bevy 产品 IPC 源码路径
  - 产物：driver 注入 AndroidOwnerClient request/snapshot source；JNI→Activity→UniFFI→MihomoHost→Messenger→`:vpn` 路径接通；移除 driver 对 infiltrator-android 静态链接；双 ABI 与 universal Debug APK 构建通过 identity/aapt（2026-10-11 记录）。
  - 依赖：BANDROID-040.2。
  - 验收：fake transport scope/stale/timeout 批次回执记录。
  - 当前证据：`bd01251d-c13c-4bf7-a8d5-f316a852c7f4`；13/13。
  - 待验证：[ARM64真机] 实际设备运行与 IPC 记录（当前暂无 ARM64 设备）。
- [ ] BANDROID-043.3 [kind: mechanism] [priority: P1] [points: 3] [owner: MihomoControlClient] [status: Integrated] Binder client 有界复用
  - 产物：Application 级复用单 binding/reply Handler/有界 pending 表；disconnect/death typed 失败清 timeout；caller cancellation 撤销未开始请求，不可取消副作用以同 request ID/generation 幂等终态回读；禁每请求 `HandlerThread`、禁 owner `runBlocking`。
  - 依赖：已交付的 sealed bind migration（bind callback 结果不再用 Boolean）。
  - 验收：正向/非法迁移测试与排队/取消/死亡重绑 fake 已添加；统一 Gradle 与设备证据待验证。
  - 待验证：[ARM64真机] 实际设备运行与 Binder client 复用取证（当前暂无 ARM64 设备）。
- [ ] BANDROID-043.4 [kind: mechanism] [priority: P1] [points: 2] [owner: Rust AndroidBridge seam] [status: Integrated] 同步 JNI host 等待改为 request-ID 异步回调
  - 产物：Rust application await typed owner reply；Kotlin 用单一有界 executor、Application 持有的 Binder client 与同一绝对 deadline；timeout/cancel 按 request ID 撤销并丢弃迟到回执，不得从缓存伪造成功；凭据写删结果为 `Applied/Rejected` enum。
  - 依赖：BANDROID-043.3。
  - 验收：乱序、容量、超时、取消与迟到回执有行为覆盖。
  - 当前证据：`cd4c91fa-ba43-46b8-a94c-6e1a92a9bca7`；6/6；183/183。
  - 待验证：[ARM64真机] 实际设备运行证据（当前暂无 ARM64 设备）。
- [ ] BANDROID-043.7 [kind: validation] [priority: P1] [points: 2] [owner: NativeHostAdapter] [status: Integrated] attach/detach 事务化
  - 产物：sealed attachment state；attach 失败逆序回滚；detach 逐项清理保留 `CleanupNeeded`；Rust bridge registration 按 generation 串行；结果不确定 fail closed。
  - 依赖：无。
  - 验收：注入式 Kotlin unit tests 统一批次通过。
  - 待验证：[ARM64真机] 真实 JNI 与设备生命周期证据（当前暂无 ARM64 设备）。

## 四、UI 表面、组件与第三方界面（消费端表面实现，1条 / 1pt）

- [ ] BANDROID-009.1 [kind: validation] [priority: P1] [points: 1] [owner: bevy-widgets::list + zhuge-ui-core] [status: Integrated] 虚拟列表变高共享几何回归
  - 产物：Proxies/Connections/Logs 虚拟窗口/行槽回收接入；变高 extent 前缀索引与窗口由 `zhuge-ui-core` 计算；旧 list-local fling/physics 已退役。
  - 依赖：无。
  - 验收：变高窗口共享几何回归统一 nextest 批次通过。
  - 待验证：[ARM64真机] 500/2000 节点滚到尾项的帧耗时/内存基准（当前暂无 ARM64 设备）。

## 五、工程基线、编译配置与质量守卫（基础设施支持，2条 / 4pt）

- [ ] BANDROID-020.1 [kind: infra] [priority: P0] [points: 2] [owner: toolchain baseline] [status: Integrated] Android SDK/NDK/Gradle/JDK 基线与核对
  - 产物：SDK/NDK/JDK 值统一由 `android/toolchain.properties` 提供给 Gradle、CI、模拟器与 Bevy 诊断构建；wrapper 和 Rust 使用各自唯一版本文件；本机/CI 核对由 `scripts/android-toolchain.py check` 执行。
  - 依赖：无。
  - 验收：本机 JDK/Gradle/SDK/NDK/Rust 与登记值匹配；Tag-only CI 在 SDK 安装后运行同一核对；配置漂移由检查拒绝。
  - 当前证据：本地定向批次通过。
  - 待验证：未派发 tag workflow，因此 CI 实际 runner 回执仍待 BANDROID-018.2；本条不代表 APK/设备验证。
- [ ] BANDROID-024.1 [kind: validation] [priority: P0] [points: 2] [owner: android gradle] [status: Integrated] AGP/Kotlin/Gradle 兼容复核
  - 产物：AGP 9.3.1、KGP 2.4.21、Gradle 9.7.0、JDK 17 的组合落在官方 fully-supported 矩阵；API 37 与 AGP 9.3 上限一致；SDK/NDK/JDK pins 由 BANDROID-020.1 唯一登记。
  - 依赖：BANDROID-020.1（Integrated）。
  - 验收：官方兼容矩阵复核；Gradle 配置与 Core host JVM 测试通过；版本漂移由 toolchain checker 拒绝。
  - 当前证据：183/183。
  - 待验证：未运行 Android app/APK/设备；AGP9.3.1 的 JDK17 lint crash 已登记到未来 BANDROID-044.1，Lint 暂不启用；真实 tag CI 证据留在 BANDROID-018.2。

## 七、中性内核（PUB）发布验收（8条 / 20pt）

- [ ] PUB-001.3 [kind: mechanism] [priority: P1] [points: 3] [owner: profile-store] [status: Integrated] 抽象通用 lurker-profile-store 与租约并发接口
  - 产物：基于 CAS 与版本租约锁的通用存储/事务接口；剥离产品私有 Age 凭据、平台依赖与硬编码路径；CAS/租约/owner-switch/reset 完整下沉 `lurker-profile-store::owner_switch`。
  - 依赖：PUB-001.1、PUB-001.2。
  - 验收：单测覆盖活动租约锁竞争、并发 CAS 冲突、安全原子替换；零业务私有路径；`cargo package` 通过（dry-run 待 contract 上架）。
  - 当前证据（L）：`2a2726ba-f211-44e6-9986-9b28f6bd1977`；5205 passed；99/99 store tests。
  - 待验证：1.0-M1 crates.io 发布顺序门。
- [ ] PUB-003.2 [kind: mechanism] [priority: P1] [points: 3] [owner: lurker-engine-runtime::switch] [status: Integrated] 双引擎原子切换抽象为 EngineSwitchCoordinator
  - 产物：中立契约/端口/协调器 + 内存 journal 与产品迁移；基于 CAS 租约与 Generation/Revision 围栏的无缝切换组件；失败时原主自动恢复。
  - 依赖：PUB-003.1。
  - 验收：nextest 覆盖正常切换与注入故障回滚，证明新旧进程生命周期严格互斥且终态可查。
  - 当前证据（L）：`2a2726ba-f211-44e6-9986-9b28f6bd1977`；5205 passed；29/29 composition tests。
  - 待验证：统一发布批次。
- [ ] PUB-003.3 [kind: mechanism] [priority: P1] [points: 3] [owner: runtime facade] [status: Integrated] 高层 Facade SDK lurker-engine-runtime
  - 产物：`LurkerRuntimeBuilder` / `LurkerRuntime`，封装 Token/Generation 细节与单并发控制租约；提供 start/stop/switch_engine/wait_ready 与状态流。
  - 依赖：PUB-003.2。
  - 验收：第三方 10 行内完成初始化并监听状态流；单测与文档示例验证；`cargo publish --dry-run` 通过。
  - 当前证据（L）：`2a2726ba-f211-44e6-9986-9b28f6bd1977`；28/28 runtime tests；5205 passed。
  - 待验证：1.0-M1 crates.io 发布顺序门。
- [ ] PUB-002.1 [kind: mechanism] [priority: P1] [points: 3] [owner: mihomo-driver] [status: Integrated] Mihomo 驱动整合为独立 `mihomo-driver`
  - 产物：控制器 REST/WS、outbound/routing port、进程监督与精确终态、版本/readiness，均以 `lurker-core-ports` trait 暴露。
  - 依赖：PUB-001.1、PUB-001.2。
  - 当前证据（L）：driver 152 本地 nextest（mock/fake）；四目标 `cargo check` 通过；clippy `-D warnings` 0。
  - 待验证：真实 mihomo 二进制起停/readiness/selector 切换（driver 内无真进程测试，产品侧真进程 smoke 仍 `#[ignore]`）；`cargo publish --dry-run`（需 contract 先上架）；统一验证批次。
- [ ] PUB-002.2 [kind: mechanism] [priority: P1] [points: 2] [owner: mihomo-driver::codec] [status: Integrated] Mihomo YAML 配置 Codec 独立化
  - 产物：中性 YAML 解析/校验/`ProfileDocumentCodecPort`、字节保真 snippet 拼接（正文 `SnippetBodyLookup` 注入）、原生 `mihomo -t` dry-run。
  - 依赖：PUB-001.1、PUB-001.2。
  - 当前证据（L）：driver 130 + config 158 本地 nextest；parity 锚点已迁到 driver。
  - 待验证：`cargo publish --dry-run`；统一验证批次。
- [ ] PUB-002.3 [kind: mechanism] [priority: P1] [points: 3] [owner: singbox-driver] [status: Integrated] sing-box 驱动整合为独立 `singbox-driver`
  - 产物：Clash API/selector、进程监督、readiness/outbound、版本探测，均以中性 trait 暴露。
  - 依赖：PUB-001.1、PUB-001.2。
  - 当前证据（L）：driver 13 + adapter 18 本地 nextest；四目标 `cargo check` 通过。
  - 待验证：真实 sing-box 二进制拉起/readiness/selector 安全退出（driver 内无真进程测试）；`cargo publish --dry-run`；统一验证批次。
- [ ] PUB-002.4 [kind: mechanism] [priority: P1] [points: 2] [owner: singbox-driver::codec] [status: Integrated] sing-box JSON 配置 Codec 独立化
  - 产物：原生 JSON 模型、schema 校验、native `check` 事务与中性 `ProfileDocumentCodecPort`。
  - 依赖：PUB-001.1、PUB-001.2。
  - 当前证据（L）：driver codec 单测；产品 `SingBoxProfileConfigurationCodec` 只委托、typed 操作 fail-closed。
  - 待验证：`cargo publish --dry-run`；统一验证批次。
- [ ] PUB-002.5 [kind: evidence] [priority: P1] [points: 1] [owner: mihomo-driver + singbox-driver] [status: Integrated] 双引擎独立 example 与 nextest
  - 产物：两个 driver 各含 `examples/lifecycle.rs` 与 CI 安全 smoke。
  - 依赖：PUB-002.1～PUB-002.4。
  - 当前证据（L）：未配置时打印计划并 0 退出；配置路径有代码但未跑真二进制；driver 148 本地 nextest。
  - 待验证：**脱离本仓其他包的独立工程内 `cargo run --example` 真实生命周期**（当前是在仓内示例，且无引擎时只打印计划）；统一验证批次。

## 八、v0.42.5 发布管道与构建产物 CI 留痕（受控环境阻塞待验，3条 / 3pt）

- [ ] BANDROID-018.2 [kind: evidence] [priority: P0] [points: 1] [owner: CI] [status: Integrated] tag-only CI 实际全绿回执
  - 产物：受控 `v*` tag release 与 Android workflow 的完整 run 链接和分项证据；普通 push/PR 不运行。
  - 依赖：高成本 tag-only 工作流；CI-QUALITY-001～005；CORE-UI-002（全仓绿）。
  - 验收：tag run 中 nextest、构建、Clippy、APK identity 与 Android tests 全绿并归档。
  - 当前证据：本地 5200+ nextest 与 Clippy 0 警告全绿；受控 tag 流水线定义在 `.github/workflows/` 已对齐。
  - 待验证：[CI/Tag] 受控 tag release 工作流回执待留存（用户选择不为凑 CI 盲目打 tag，在 v0.42.5 档保持环境阻塞待验，不阻塞后续版本）。
- [ ] BANDROID-019.2 [kind: evidence] [priority: P0] [points: 1] [owner: CI] [status: Integrated] manifest/aapt 守卫 CI 回执
  - 产物：`android-manifest-contract-guard.py` 与 aapt identity 检查在 CI 实际执行并留存回执。
  - 依赖：BANDROID-018.2。
  - 验收：回执绑定 run 与 APK 哈希。
  - 当前证据：本地 manifest guard 11/11 全通。
  - 待验证：[CI/Tag] 伴随 tag release 工作流实际 runner 留存回执（v0.42.5 档环境阻塞待验）。
- [ ] BANDROID-038.2 [kind: evidence] [priority: P0] [points: 1] [owner: CI + release workflow] [status: Integrated] 每个 APK 的 aapt identity CI 检查
  - 产物：tag release 对每个最终 APK 的 aapt identity 检查实际接入并留存回执；普通 push/PR 不触发构建。
  - 依赖：已交付的 identity registry 与 guard（BANDROID-038.1）。
  - 验收：负例（drift APK）在 CI 被拒；回执绑定 APK SHA。
  - 当前证据：本地 identity registry 与 guard 已交付闭环。
  - 待验证：[CI/Tag] 伴随 tag release 工作流对各 APK 实际输出留存 SHA 校验回执（v0.42.5 档环境阻塞待验）。

## 九、登记说明

- 2026-10-11 之前的历史切片回执（run ID、APK SHA、模拟器记录）已在 [TODO.md](../TODO.md) 归档表与 `.evidence/` 永久留存；本文件只持未办结验证项。
- 已验证退出项不在本文件重复登记。
