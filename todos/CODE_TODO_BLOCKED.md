# MusicFrog 工程 TODO · 斗战胜佛（环境阻塞待解锁）

> 层级：L4

## 0. 编写规则

- 本板只放**当前环境无法推进**的未办结条目：剩余闭环动作必须依赖仓库之外、且此刻不存在的外部环境（ARM64 物理设备、受控 tag CI runner、用户 secrets/持久 keystore、crates.io 上架凭证、异构目标机与被禁平台 target）。
- **执行标准（2026-10-20 起强制）：** 底层结构不够就扩 owner 结构，禁止再塞 `bool`/`Option`；组合优于继承；field 归 field、表现归表现、事务归 owner；每个事实只有一个真源，不留第二份缓存；非身份改动不碰 wire/快照/语料，身份改动集中一次做完。
- **可推条目禁止留在本板**：只要条目还剩至少一个可在本机执行的动作（统一 nextest/clippy/guard 批次、本地 Gradle 与模拟器 M1、本地真实引擎二进制 smoke、`cargo package`/dry-run、纯代码实现），它就属于[现在佛](CODE_TODO.md)或[过去佛](CODE_TODO_PAST.md)，本板不得预占。判定以「条目的下一个可执行动作」为准，不以条目是否已实现为准。
- 每条必须写明三件事：**阻塞方**（谁/什么提供环境）、**解锁条件**（拿到什么证据即可办结）、**本板停留期间的可执行动作**（若有任何本机动作，说明它为何不构成办结）。
- 状态固定 `Blocked`。本板条目不计入执行态进度，不阻塞其它板条的选取与统一验证批次。
- 环境到位后，同次把条目迁回[过去佛](CODE_TODO_PAST.md)（已交付待验证）或[现在佛](CODE_TODO.md)（仍需实现），并在 §六 登记迁出记录；办结后按[台账](../TODO.md)登记 archived points。
- 外部环境只是条目的**子条件**时不必拆条：条目本体留在可推的看板，其阻塞腿在本板 §五 交叉索引里登记，避免同一事实两处登记。
- 本板不重复记录已经在过去佛/未来佛登记的条目正文；迁入时必须整条迁入（含 ID、owner、points、依赖、验收、当前证据），不得只留摘要。
- **并行工流与验证纪律（2026-10-20 起强制）：** 并行开发按文件所有权切分，编译不通过的半成品不得放进会被 `cargo nextest --workspace`、结构门或 capture 构建扫到的路径（WIP 先放草稿或 `.evidence/wip/`）；开发期只跑受影响包，全仓 `cargo nextest --workspace` 与全量结构门统一放到收尾批次只跑一次；共享 `/tmp` 配额、cargo lane 争用与合成器起栈失败等外部故障先判定归属，只修本切片能修的那一类。

## 一、ARM64 真机取证（无物理设备，11 条 / 33 pt）

- [ ] BANDROID-041.3.2R.15 [kind: mechanism] [priority: P1] [points: 5] [owner: protocol contract + presentation + Application] [status: Blocked] Protocol editor model split from persistence use case
  - 产物：Protocol form/draft/preview projection 归 presentation；Profile 写入继续由 Application use case 接收 typed intent，UI 不直接调用持久化方法。
  - 依赖：BANDROID-041.3.2R.13、.14.1～.14.7；ProfileDocument codec contract；Core-first 阶段门。
  - 验收：旧 Application protocol-form presentation exports 与 UI imports 清零；编码 round-trip、拒绝/取消/保存终态由 fake/nextest 覆盖。
  - 当前证据：`5a7db497-6576-436c-a437-b73695098f8b`；790 passed；5116/5116。
  - 阻塞方：ARM64 Android 物理设备（当前无设备；x86_64 模拟器不能替代 ARM64 腿）。
  - 解锁条件：真机上完成协议编辑表单实际运行取证并回填证据身份；取得后同次迁回过去佛走统一批次办结。
  - 本板停留期间：实现与本地证据已闭环，无剩余本机可执行动作。
- [ ] CORE-007.2 [kind: mechanism] [priority: P0] [points: 2] [owner: infiltrator-android IPC adapter + :vpn owner] [status: Blocked] Android 消费者 scope fence 与 Binder readback/adopt
  - 产物：Binder death/rebind 后先 readback/adopt Core 与 VPN 双状态再接命令；过期 generation/session 回执不改写新 owner 状态。
  - 依赖：BANDROID-040.2/040.4 typed IPC；BANDROID-040.5 同窗回归。
  - 验收：Rust fake bridge 与 Kotlin fake transport 覆盖双 readback 顺序、Unavailable/stale 无副作用；nextest 与 Core host JVM 批次通过。
  - 当前证据：`f9d6be76-6996-495b-9b78-b5181ef1046a`；3100 passed；5116 passed；13/13。
  - 阻塞方：ARM64 Android 物理设备。
  - 解锁条件：真机 Binder death/rebind 设备取证回执；取得后同次迁回过去佛。
  - 本板停留期间：fake/nextest/JVM 批次已闭环，无剩余本机可执行动作。
- [ ] BANDROID-041.3.3 [kind: mechanism] [priority: P1] [points: 2] [owner: android/bevy-host driver] [status: Blocked] Android Bevy product dependency closure
  - 产物：driver 仅装配 NativeActivity、typed `AndroidOwnerClient`、Bevy UI/widgets/contract 与 system-host ports/Insets/IME adapter；capture-only Application 路径排除。
  - 依赖：BANDROID-041.3.2A、.3.2R.13、.14.1～.14.7、.15。
  - 验收：移除 `ui-application-projections`；locked Android target tree 无 application/composition/mihomo-api/platform；UI package nextest 留 CORE-UI-002。
  - 当前证据：`d357a7f7-c103-4a0f-bec1-01ccb9a9a518`；795 passed；5116/5116；6/6。
  - 阻塞方：ARM64 Android 物理设备。
  - 解锁条件：最终 driver `.so`/APK 的 ELF 设备验证回执；取得后同次迁回过去佛。
  - 本板停留期间：依赖闭包与本地批次已闭环，无剩余本机可执行动作。
- [ ] BANDROID-002.4 [kind: mechanism] [priority: P0] [points: 2] [owner: Android MihomoVpnService] [status: Blocked] VPN lifecycle bounded WakeLock
  - 产物：仅在 Start/Cleanup transaction 持有有时限 partial WakeLock；typed lease 绑定 generation/deadline，终态及时释放，系统 timeout 作兜底，空闲期不持有。
  - 依赖：BANDROID-002.1 VPN lifecycle admission。
  - 验收：fake handle 覆盖 acquire/join/expiry/release failure/stale generation；host integration 确认 Start 与 Stop/Revoke task 的 finally 收口；Manifest 声明由 contract guard 覆盖。
  - 当前证据：178/178；11/11。
  - 阻塞方：ARM64 Android 物理设备。
  - 解锁条件：最终 APK 安装与真机功耗样本回执；取得后同次迁回过去佛。
  - 本板停留期间：fake 与 host integration 批次已闭环，无剩余本机可执行动作。
- [ ] BANDROID-005.1 [kind: mechanism] [priority: P0] [points: 3] [owner: Android 原生窗口 adapter] [status: Blocked] AppLifecycle/Surface 重建 typed 状态
  - 产物：接入真实 Activity phase、Surface 暂停/恢复、窗口焦点及 generation fence；Compose sealed route 跨 Activity 重建保存；UI 销毁只释放观察，导入 URL 继续以 Android Intent 为源。
  - 依赖：BANDROID-003、004。
  - 验收：Rust/Kotlin 生命周期迁移与 wire-code 合成测试通过；AppRoute 全状态保存/恢复测试通过。
  - 当前证据：192 passed；194/194；3/3。
  - 阻塞方：ARM64 Android 物理设备。
  - 解锁条件：锁屏、后台、旋转、分屏 ≥100 次循环真机回执（无 surface panic、无重复 VPN 命令、内存曲线平稳）；取得后同次迁回过去佛。
  - 本板停留期间：合成测试与状态恢复测试已闭环，无剩余本机可执行动作。
- [ ] BANDROID-040.5 [kind: mechanism] [priority: P0] [points: 2] [owner: android IPC adapter] [status: Blocked] Binder death/rebind 与双状态 readback（含 BANDROID-001.2）
  - 产物：新 Binder session 放行命令前必须读回 Core 与 VPN state/generation；旧 request、session、Core/VPN generation 与乱序终态不能改写当前 owner 快照。
  - 依赖：BANDROID-040.2/040.4 typed IPC；BANDROID-042.7 typed stop IPC。
  - 验收：Rust fake bridge 与 Kotlin fake transport 覆盖 death/restart/乱序/Unavailable/stale/no-side-effect；nextest 与 Core host JVM 批次通过。
  - 当前证据：`f9d6be76-6996-495b-9b78-b5181ef1046a`；`cc3c0959-7e30-467e-a1a5-15eca68fca48`；`bd01251d-c13c-4bf7-a8d5-f316a852c7f4`；3100 passed；2673/6；13/13。
  - 阻塞方：ARM64 Android 物理设备。
  - 解锁条件：真机 APK/设备核对 Compose/Bevy/service 进程与 session、两套 `.so` 链接边界的设备回执；取得后同次迁回过去佛。
  - 本板停留期间：fake bridge 与 JVM 批次已闭环，无剩余本机可执行动作。
- [ ] BANDROID-040.7 [kind: mechanism] [priority: P0] [points: 5] [owner: :vpn owner + contract + Android surface adapters] [status: Blocked] owner-authored VPN snapshot
  - 产物：`:vpn` 单次发布 typed `VpnOwnerSnapshot`，含完整 `CoreLifecycleSnapshot` 与 typed failure；Android `SurfaceSnapshot.vpn` 原样承载 owner observation；Compose/Bevy/Slint 读取同一 owner snapshot；旧 integer reply、Android 合成 UI model 与进程本地 revision/cache 入口已退役。
  - 依赖：BANDROID-040.5；BANDROID-042.7 owner-first stop。
  - 验收：Rust/Kotlin 负测与 roundtrip 通过。
  - 当前证据：`f9d6be76-6996-495b-9b78-b5181ef1046a`；`cc3c0959-7e30-467e-a1a5-15eca68fca48`；`bd01251d-c13c-4bf7-a8d5-f316a852c7f4`；3100 passed；2673/6；13/13。
  - 阻塞方：ARM64 Android 物理设备。
  - 解锁条件：Compose/Bevy/Slint 三端同 revision 设备记录与 owner 重绑 readback/adopt 设备观察；取得后同次迁回过去佛。
  - 本板停留期间：负测与 roundtrip 已闭环，无剩余本机可执行动作。
- [ ] BANDROID-040.8 [kind: mechanism] [priority: P0] [points: 5] [owner: Android :vpn typed IPC + Rust credential owner] [status: Blocked] Age credential owner operation with sensitive transport
  - 产物：新增 scope/selection/revision-fenced Age credential read/set/clear owner operations；密钥经一次性 typed sensitive payload 传输，不进入 JSON command, snapshot, reply, log or profile state；Rust 在 owner 侧构造并清理 `AgePrivateKey`，仅返回非敏感状态/revision readback。
  - 依赖：BANDROID-040.2/.3/.5/.7；CORE-020.1/.2；SLINT-014。
  - 验收：fake/nextest 与 Kotlin host tests 覆盖 wrong scope、stale selection/revision、owner death/cancel、无效/超长 key、readback 与零 secret 泄漏；旧 Age unsupported consumer path 由 SLINT-014 迁移，package verification 留 CORE-UI-002。
  - 当前证据：`254253dc-4c7a-4a20-bccc-171b0a30664b`；`d4df4dfc-df6f-4fe4-a059-dcb58ed06b78`；`29bd657b-9053-426d-b976-267835be958f`；1271 passed；191 passed；1226 passed。
  - 阻塞方：ARM64 Android 物理设备。
  - 解锁条件：Compose APK 提交 Age 凭据 read/set/clear 的双进程 Binder readback 记录与无 secret 泄漏核验；取得后同次迁回过去佛。
  - 本板停留期间：fake/nextest 与 Kotlin host 批次已闭环，无剩余本机可执行动作。
- [ ] BANDROID-042.5c [kind: mechanism] [priority: P0] [points: 3] [owner: :vpn owner + Android bridge + Mihomo validator] [status: Blocked] 暂停 Core 时的原生配置校验
  - 产物：`:vpn` owner 对临时候选调用同 ABI Mihomo `-t`；请求绑定 workspace/launch identity、候选 digest、request ID 与有界 budget；owner 收到请求后建立 elapsedRealtime deadline；不启动 Core、不写 active workspace。
  - 依赖：CORE-013.4、BANDROID-040.3/.7。
  - 验收：Stopped/Failed/Running 的 foreign/stale/native-invalid 均零写入；receipt identity/digest 精确匹配；超时清理子进程和候选；缺 binary/owner IPC typed fail closed。
  - 当前证据：`f4e0ff30-a311-4ac5-90fa-221a77e35d8d`；`a62d1717-5d4d-4c2a-80cc-55fc803b6dad`；15/15；234 passed；183/183。
  - 阻塞方：ARM64 Android 物理设备。
  - 解锁条件：同 ABI 候选 APK 在 Core Stopped 下的 `-t` 子进程与回执归档设备记录；取得后同次迁回过去佛。
  - 本板停留期间：本地批次已闭环，无剩余本机可执行动作。
- [ ] BANDROID-040.4 [kind: validation] [priority: P0] [points: 2] [owner: android/bevy-host driver] [status: Blocked] Bevy 产品 IPC 源码路径
  - 产物：driver 注入 AndroidOwnerClient request/snapshot source；JNI→Activity→UniFFI→MihomoHost→Messenger→`:vpn` 路径接通；移除 driver 对 infiltrator-android 静态链接；双 ABI 与 universal Debug APK 构建通过 identity/aapt（2026-10-11 记录）。
  - 依赖：BANDROID-040.2。
  - 验收：fake transport scope/stale/timeout 批次回执记录。
  - 当前证据：`bd01251d-c13c-4bf7-a8d5-f316a852c7f4`；13/13。
  - 阻塞方：ARM64 Android 物理设备。
  - 解锁条件：真机运行与 IPC 记录回执；取得后同次迁回过去佛。
  - 本板停留期间：源码路径与 fake 批次已闭环，无剩余本机可执行动作。
- [ ] BANDROID-043.4 [kind: mechanism] [priority: P1] [points: 2] [owner: Rust AndroidBridge seam] [status: Blocked] 同步 JNI host 等待改为 request-ID 异步回调
  - 产物：Rust application await typed owner reply；Kotlin 用单一有界 executor、Application 持有的 Binder client 与同一绝对 deadline；timeout/cancel 按 request ID 撤销并丢弃迟到回执，不得从缓存伪造成功；凭据写删结果为 `Applied/Rejected` enum。
  - 依赖：BANDROID-043.3。
  - 验收：乱序、容量、超时、取消与迟到回执有行为覆盖。
  - 当前证据：`cd4c91fa-ba43-46b8-a94c-6e1a92a9bca7`；6/6；183/183。
  - 阻塞方：ARM64 Android 物理设备。
  - 解锁条件：真机实际运行证据；取得后同次迁回过去佛。
  - 本板停留期间：行为覆盖已闭环，无剩余本机可执行动作。

## 二、受控 tag CI 回执（用户选择不打 tag，5 条 / 7 pt）

> 本节的共同前置：远端默认分支仍停在 `58158da6`（即既有 `v0.50.0` tag 指向的提交），本地 `main` 已领先 43 个提交。因此任何绑定当前代码的 tag 回执都以「先把 `main` 推到远端、再打一个新的受控 `v*` tag」为前提；`release-neutral-crates.yml` 也随这次推送才在默认分支可见。用户已明确选择不为凑 CI 证据打 tag，故本板条目在环境到位前保持阻塞。

- [ ] BANDROID-018.2 [kind: evidence] [priority: P0] [points: 1] [owner: CI] [status: Blocked] tag-only CI 实际全绿回执
  - 产物：受控 `v*` tag release 与 Android workflow 的完整 run 链接和分项证据；普通 push/PR 不运行。
  - 依赖：高成本 tag-only 工作流；CI-QUALITY-001～005；CORE-UI-002（全仓绿）。
  - 验收：tag run 中 nextest、构建、Clippy、APK identity 与 Android tests 全绿并归档。
  - 当前证据：本地 5200+ nextest 与 Clippy 0 警告全绿；受控 tag 流水线定义在 `.github/workflows/` 已对齐。
  - 阻塞方：受控 tag release runner（用户明确选择不为凑 CI 证据打 tag）。
  - 解锁条件：用户授权打受控 `v*` tag 后取得的 workflow run 回执；取得后同次迁回过去佛办结。
  - 本板停留期间：本地全量门禁已闭环，无剩余本机可执行动作。
- [ ] BANDROID-019.2 [kind: evidence] [priority: P0] [points: 1] [owner: CI] [status: Blocked] manifest/aapt 守卫 CI 回执
  - 产物：`android-manifest-contract-guard.py` 与 aapt identity 检查在 CI 实际执行并留存回执。
  - 依赖：BANDROID-018.2。
  - 验收：回执绑定 run 与 APK 哈希。
  - 当前证据：本地 manifest guard 11/11 全通。
  - 阻塞方：受控 tag release runner。
  - 解锁条件：伴随 tag release 工作流的实际 runner 回执；取得后同次迁回过去佛办结。
  - 本板停留期间：本地 guard 全通，无剩余本机可执行动作。
- [ ] BANDROID-038.2 [kind: evidence] [priority: P0] [points: 1] [owner: CI + release workflow] [status: Blocked] 每个 APK 的 aapt identity CI 检查
  - 产物：tag release 对每个最终 APK 的 aapt identity 检查实际接入并留存回执；普通 push/PR 不触发构建。
  - 依赖：已交付的 identity registry 与 guard（BANDROID-038.1）。
  - 验收：负例（drift APK）在 CI 被拒；回执绑定 APK SHA。
  - 当前证据：本地 identity registry 与 guard 已交付闭环。
  - 阻塞方：受控 tag release runner。
  - 解锁条件：tag release 工作流对各 APK 的 SHA 校验回执；取得后同次迁回过去佛办结。
  - 本板停留期间：本地 identity registry 与 guard 已闭环，无剩余本机可执行动作。
- [ ] BANDROID-020.1 [kind: infra] [priority: P0] [points: 2] [owner: toolchain baseline] [status: Blocked] Android SDK/NDK/Gradle/JDK 基线与核对
  - 产物：SDK/NDK/JDK 值统一由 `android/toolchain.properties` 提供给 Gradle、CI、模拟器与 Bevy 诊断构建；wrapper 和 Rust 使用各自唯一版本文件；本机/CI 核对由 `scripts/android-toolchain.py check` 执行。
  - 依赖：无。
  - 验收：本机 JDK/Gradle/SDK/NDK/Rust 与登记值匹配；Tag-only CI 在 SDK 安装后运行同一核对；配置漂移由检查拒绝。
  - 当前证据：`scripts/android-toolchain.py check` 本机通过（JDK 17、Gradle 9.7.0、AGP 9.3.1、KGP 2.4.21、platform 37.0、Build Tools 36.0.0、cmdline-tools 12.0、NDK 29.0.14206865、Rust 1.99.0）。
  - 阻塞方：受控 tag release runner（CI 侧同一核对尚未执行）。
  - 解锁条件：tag run 中 toolchain 核对步骤的回执；取得后同次迁回过去佛办结。
  - 本板停留期间：本机核对已闭环，无剩余本机可执行动作。
- [ ] BANDROID-024.1 [kind: validation] [priority: P0] [points: 2] [owner: android gradle] [status: Blocked] AGP/Kotlin/Gradle 兼容复核
  - 产物：AGP 9.3.1、KGP 2.4.21、Gradle 9.7.0、JDK 17 的组合落在官方 fully-supported 矩阵；API 37 与 AGP 9.3 上限一致；SDK/NDK/JDK pins 由 BANDROID-020.1 唯一登记。
  - 依赖：BANDROID-020.1（本条同板阻塞）。
  - 验收：官方兼容矩阵复核；Gradle 配置与 Core host JVM 测试通过；版本漂移由 toolchain checker 拒绝。
  - 当前证据：183/183；本机 toolchain checker 通过。
  - 阻塞方：受控 tag release runner（AGP 9.3.1 + JDK 17 的 Lint crash 另行登记在 BANDROID-044.1）。
  - 解锁条件：真实 tag CI 证据回执；取得后同次迁回过去佛办结。
  - 本板停留期间：本机矩阵复核与 JVM 批次已闭环，无剩余本机可执行动作。

## 三、用户 secrets 与持久 keystore（3 条 / 6 pt）

- [ ] BANDROID-039.1 [kind: infra] [priority: P0] [points: 3] [owner: release.yml + build.gradle.kts] [status: Blocked] 持久 release keystore 接入
  - 产物：用户/组织持有的 release keystore 与受保护 CI secrets；无 secret 时发布 job 清晰失败，禁止静默退回 debug key。
  - 依赖：用户提供并安全配置 keystore/alias/password。
  - 验收：签名产物可复现；私钥不入仓库/日志/APK 输出。
  - 当前证据：release 变体已用专用签名配置读取 keystore 路径/store 密码/key alias/key 密码（typed sum：完整 keystore 或缺失清单）；无 secret 时发布 job 明确失败、Gradle 配置期即报缺失清单，debug key 不再用于任何 release 产物（提交 `0031973`）。本机验证：缺 secret 时 `:app:assembleComposeRelease` 配置失败并列出缺失项，材料齐备时配置通过。
  - 阻塞方：用户提供的持久 release keystore 与 CI secrets。
  - 解锁条件：用户下发 keystore/alias/password 并完成 secret 配置后的发布 job 回执；取得后同次迁回过去佛办结。
  - 本板停留期间：仓库侧 fail-closed 路径已闭环并实测，剩余闭环只依赖用户 secrets。
- [ ] BANDROID-015.2 [kind: evidence] [priority: P0] [points: 2] [owner: android/app Gradle + release] [status: Blocked] release universal APK 签名/identity 校验
  - 产物：Compose/Bevy/Slint release universal APK 通过 identity/aapt 与签名校验。
  - 依赖：BANDROID-039 持久 keystore。
  - 验收：签名指纹稳定；identity/aapt 全部匹配。
  - 当前证据：Debug 变体 identity/aapt 本地通过；release 签名未验证。
  - 阻塞方：持久 release keystore（同板 BANDROID-039.1）。
  - 解锁条件：keystore 就绪后的 release universal APK 签名与 identity 回执；取得后同次迁回过去佛办结。
  - 本板停留期间：Debug 变体本地核验已闭环，无剩余本机可执行动作。
- [ ] BANDROID-039.2 [kind: evidence] [priority: P0] [points: 1] [owner: release workflow] [status: Blocked] 连续两候选签名一致
  - 产物：连续两个候选 release 签名证书指纹一致；前版安装后 `adb install -r` 升级可用。
  - 依赖：BANDROID-039.1。
  - 验收：升级验证记录（设备部分按对应任务实机条件记录）。
  - 当前证据：无。
  - 阻塞方：持久 release keystore；指纹比对可在 keystore 就绪后由 CI 产出，设备升级腿另受 ARM64 真机限制。
  - 解锁条件：两候选版本的签名指纹一致回执与升级验证记录；取得后同次迁回过去佛办结。
  - 本板停留期间：无独立本机动作（前置整条被 keystore 阻塞）。
  - 交叉索引：设备升级腿见 §一 ARM64 真机取证组。

## 四、crates.io 上架与发布顺序门（5 条 / 11 pt）

- [ ] PUB-001.3 [kind: mechanism] [priority: P1] [points: 3] [owner: profile-store] [status: Blocked] 抽象通用 lurker-profile-store 与租约并发接口
  - 产物：基于 CAS 与版本租约锁的通用存储/事务接口；剥离产品私有 Age 凭据、平台依赖与硬编码路径；CAS/租约/owner-switch/reset 完整下沉 `lurker-profile-store::owner_switch`。
  - 依赖：PUB-001.1、PUB-001.2。
  - 验收：单测覆盖活动租约锁竞争、并发 CAS 冲突、安全原子替换；零业务私有路径；`cargo package` 通过（dry-run 待 contract 上架）。
  - 当前证据（L）：`2a2726ba-f211-44e6-9986-9b28f6bd1977`；5205 passed；99/99 store tests。
  - 阻塞方：crates.io 上架（`lurker-core-contract` 尚未实际发布，dry-run 需 registry 解析中性依赖）。
  - 解锁条件：contract 上架后 `cargo publish --dry-run -p lurker-profile-store` 通过的回执；取得后同次迁回过去佛办结。
  - 本板停留期间：`cargo package` 与本地单测已闭环，无剩余本机可执行动作。
- [ ] PUB-003.2 [kind: mechanism] [priority: P1] [points: 3] [owner: lurker-engine-runtime::switch] [status: Blocked] 双引擎原子切换抽象为 EngineSwitchCoordinator
  - 产物：中立契约/端口/协调器 + 内存 journal 与产品迁移；基于 CAS 租约与 Generation/Revision 围栏的无缝切换组件；失败时原主自动恢复。
  - 依赖：PUB-003.1。
  - 验收：nextest 覆盖正常切换与注入故障回滚，证明新旧进程生命周期严格互斥且终态可查。
  - 当前证据（L）：`2a2726ba-f211-44e6-9986-9b28f6bd1977`；5205 passed；29/29 composition tests。
  - 阻塞方：统一发布批次（crates.io 顺序上架门）。
  - 解锁条件：中性 crate 顺序发布批次回执；取得后同次迁回过去佛办结。
  - 本板停留期间：本地切换与回滚批次已闭环，无剩余本机可执行动作。
- [ ] PUB-003.3 [kind: mechanism] [priority: P1] [points: 3] [owner: runtime facade] [status: Blocked] 高层 Facade SDK lurker-engine-runtime
  - 产物：`LurkerRuntimeBuilder` / `LurkerRuntime`，封装 Token/Generation 细节与单并发控制租约；提供 start/stop/switch_engine/wait_ready 与状态流。
  - 依赖：PUB-003.2。
  - 验收：第三方 10 行内完成初始化并监听状态流；单测与文档示例验证；`cargo publish --dry-run` 通过。
  - 当前证据（L）：`2a2726ba-f211-44e6-9986-9b28f6bd1977`；28/28 runtime tests；5205 passed。
  - 阻塞方：1.0-M1 crates.io 发布顺序门。
  - 解锁条件：contract 上架后 `cargo publish --dry-run -p lurker-engine-runtime` 通过的回执；取得后同次迁回过去佛办结。
  - 本板停留期间：本地单测与示例已闭环，无剩余本机可执行动作。
- [ ] PUB-006.12 [kind: release] [priority: P1] [points: 1] [owner: release] [status: Blocked] 1.0-RC 候选版本基线冻结与预发布
  - 产物：锁定七大中性 crate 版本为 `1.0.0-rc.1`；生成首个 release candidate 校验和清单、GPG 签名草案与预发布发布说明（Release Notes）。
  - 依赖：PUB-006.1～PUB-006.11。
  - 验收：七个中性 crate API 签名完全冻结；预发布打包 dry-run 100% 成功；0.6x 开源发布与生态体系完备收官，正式进入 0.7x 质量与安装包阶段。
  - 当前证据：无。
  - 阻塞方：crates.io 上架凭证与 GPG 签名材料（依赖链 PUB-006.1～006.11 仍在本板外推进中）。
  - 解锁条件：RC 版本基线冻结、校验和清单与签名草案归档回执；取得后同次迁回过去佛办结。
  - 本板停留期间：依赖链尚未完成，本条无独立本机动作。
- [ ] PUB-006.5 [kind: evidence] [priority: P1] [points: 1] [owner: release] [status: Blocked] 1.0.0 首发布回执
  - 产物：`v1.0.0` tag、release notes、crates.io 七个中性 crate（contract/ports/profile-store/mihomo-driver/singbox-driver/engine-runtime/ui-presentation）顺序发布回执与 docs.rs 构建链接。
  - 依赖：PUB-006.1、PUB-006.3、PUB-006.4、PUB-006.12。
  - 验收：crates.io 七 crate 版本严格对齐；docs.rs 七页构建成功；回执绑定 tag 与 commit。
  - 当前证据：无。
  - 阻塞方：crates.io 上架凭证、受控 tag 与 PUB-006.12 基线冻结。
  - 解锁条件：`v1.0.0` tag 发布回执与 docs.rs 构建链接；取得后同次迁回过去佛办结。
  - 本板停留期间：本条无独立本机动作。

## 五、异构目标机与被禁平台 target（5 条 / 15 pt）

- [ ] APPID-001.4 [kind: evidence] [priority: P0] [points: 2] [owner: packaging/windows] [status: Blocked] Windows NSIS/Wix 产物
  - 产物：Windows NSIS/Wix 最终产物构建与验收。
  - 依赖：APPID-001.2。
  - 验收：版本/名称/图标来自登记表；安装冒烟通过。
  - 当前证据：登记表与 builder 脚本已交付；Windows 产物未构建。
  - 阻塞方：Windows 目标机（安装与卸载冒烟必须在真实 Windows 环境执行）。
  - 解锁条件：Windows CI 或本机产物与安装冒烟回执；取得后同次迁回未来佛办结。
  - 本板停留期间：builder 与登记表已闭环，无剩余本机可执行动作。
- [ ] GAP-2.1 [kind: evidence] [priority: P2] [points: 2] [owner: release] [status: Blocked] Windows/Linux 安装包与系统集成验证
  - 产物：NSIS/AppImage/deb 产物与安装、托盘、TUN 回执。
  - 依赖：`GAP-3.1` 之外的发布流水线；需要 CI 或本机打包环境。
  - 验收：每个安装包有可复跑构建与安装回执；不达标项显式 Unsupported。
  - 当前证据：无。
  - 阻塞方：Windows 目标机（NSIS 腿）；Linux 腿由 APPID-001.3 在未来佛推进。
  - 解锁条件：Windows 安装、托盘与 TUN 回执；取得后同次迁回未来佛办结。
  - 本板停留期间：Linux 侧由 APPID-001.3 承担，本条无独立本机动作。
- [ ] GAP-2.2 [kind: evidence] [priority: P2] [points: 3] [owner: release] [status: Blocked] macOS 产物与签名
  - 产物：App bundle、签名、权限与安装运行回执。
  - 依赖：Apple target 解禁（当前由 `apple-ci-deferral-guard.py` 显式禁止）。
  - 验收：更新平台矩阵并取得目标产物证据。
  - 当前证据：无。
  - 阻塞方：macOS 目标机与 Apple target 解禁决策。
  - 解锁条件：macOS bundle、签名与安装运行回执；取得后同次迁回未来佛办结。
  - 本板停留期间：被 deferral guard 显式禁止，本条无本机动作。
- [ ] GAP-3.1 [kind: evidence] [priority: P1] [points: 3] [owner: android/release] [status: Blocked] Android APK 构建修复取证
  - 产物：cmdline-tools 重链修复后的 APK 与模拟器/真机回执。
  - 依赖：CI tag 或本机完整 SDK/Gradle。
  - 验收：APK 产物 + 启动/触摸/权限回执；否则保持 Unsupported。
  - 当前证据：本机 SDK 已含 cmdline-tools 12.0、NDK 29.0.14206865 与 platform 37.0；修复后的 APK 与模拟器回执尚未产出。
  - 阻塞方：受控 tag CI runner（Android 变体构建与 aapt 身份回执只在 tag 工作流产出）。
  - 解锁条件：tag 工作流的 APK 构建与身份回执；取得后同次迁回未来佛办结。
  - 本板停留期间：本机 toolchain 齐备但缺少 CI 回执，无剩余本机可执行动作。
- [ ] GAP-3.2 [kind: mechanism] [priority: P2] [points: 5] [owner: ios] [status: Blocked] iOS target 与应用骨架
  - 产物：`aarch64-apple-ios` 构建、应用包与 NetworkExtension 骨架。
  - 依赖：Apple target 解禁。
  - 验收：真实 target 构建 + 包结构检查；当前只有 626 行 host stub。
  - 当前证据：无（当前只有 626 行 host stub）。
  - 阻塞方：Apple target 解禁与 macOS 工具链。
  - 解锁条件：`aarch64-apple-ios` 真实构建与包结构检查回执；取得后同次迁回未来佛办结。
  - 本板停留期间：被 deferral guard 显式禁止，本条无本机动作。

## 六、跨板交叉索引（阻塞腿随条目本体推进）

| 条目 | 所在看板 | 阻塞腿 | 阻塞方 | 本板状态 |
| --- | --- | --- | --- | --- |
| BANDROID-002.1 | [过去佛](CODE_TODO_PAST.md) | `[ARM64真机 D]` always-on 恢复与真实连通性 | ARM64 物理设备 | 随条目本体推进，条目本体不因该腿阻塞 |
| BANDROID-002.2 | [过去佛](CODE_TODO_PAST.md) | `[ARM64真机 D]` 真实流量、Wi-Fi/蜂窝切换、WakeLock 释放 | ARM64 物理设备 | 随条目本体推进，条目本体不因该腿阻塞 |
| BANDROID-039.2 | 本板 §三 | 设备升级腿 | ARM64 物理设备 | 随条目本体推进 |

## 七、迁出记录

| 迁出日期 | 条目 | 原阻塞方 | 迁回看板 | 证据身份 |
| --- | --- | --- | --- | --- |
| — | 暂无 | — | — | — |

## 八、登记说明

- 本板是三佛体系的环境阻塞层，与[现在佛](CODE_TODO.md)、[过去佛](CODE_TODO_PAST.md)互斥：同一条目只能在一块板上登记正文。
- 阻塞原因消失后必须同次迁出并登记，不得在本板长期停放可推条目。
- 模拟器（`-M`）与真机（`-D`）证据要求仍内聚在条目本体；本板只承载「整体不可推进」的环境门槛。
