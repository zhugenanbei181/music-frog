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

## 一、Android VPN 生命周期与系统服务（外部环境阻塞待验证，2条 / 6pt）

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

## 二、Android IPC 与 Kotlin Host 边界（外部环境阻塞待验证，2条 / 5pt）

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

## 三、UI 表面与组件（外部环境阻塞待验证，1条 / 1pt）

- [ ] BANDROID-009.1 [kind: validation] [priority: P1] [points: 1] [owner: bevy-widgets::list + zhuge-ui-core] [status: Integrated] 虚拟列表变高共享几何回归
  - 产物：Proxies/Connections/Logs 虚拟窗口/行槽回收接入；变高 extent 前缀索引与窗口由 `zhuge-ui-core` 计算；旧 list-local fling/physics 已退役。
  - 依赖：无。
  - 验收：变高窗口共享几何回归统一 nextest 批次通过。
  - 当前证据：统一 nextest 批次尚未回填。
  - 待验证：本地统一 nextest 批次回执；[ARM64真机] 500/2000 节点滚到尾项的帧耗时/内存基准。
  - 本机可执行动作：先跑变高窗口共享几何回归批次并回填证据身份。

## 五、发布流水线与 parity 管线（外部环境阻塞待验证，1条 / 2pt）

- [ ] PUB-006.1 [kind: infra] [priority: P0] [points: 2] [owner: CI] [status: Integrated] Crates.io 自动化发布 CI 工作流
  - 产物：`.github/workflows/release-neutral-crates.yml`（validate-topology → dry-run → publish 三段，tag-only，dispatch 仅演练）；`scripts/quality/neutral-release-guard.py`（拓扑审计、逐 crate 表格、dry-run 分类、顺序发布、索引等待、缺 token 硬拒绝）；`scripts/neutral-crate-release.sh` 本地可复跑入口与同源说明文档。
  - 依赖：PUB-001.1～PUB-003.3（七个中性 crate 已归档）。
  - 验收：本地与 CI 环境下 dry-run 顺序打包成功，依赖图谱无环且版本严格对齐。
  - 当前证据（L）：`bash scripts/neutral-crate-release.sh structure` 七行表格全 `ready`；`neutral-release-guard.py --self-test` PASS；新增 24 条单测全绿；`ci-trigger-guard`、`release-build-target-guard`、`apple-ci-deferral-guard`、`publish-order-guard` 四个守卫 enforce 全绿；无 `CARGO_REGISTRY_TOKEN` 时 publish 模式明确拒绝（exit 1，无匿名/调试回退）。
  - 待验证：受控 `v*` tag 的真实 workflow run 与 crates.io 上架回执（顺序上架门在[斗战胜佛](CODE_TODO_BLOCKED.md) §四）。
  - 本机可执行动作：跑统一验证批次与 `cargo publish --dry-run`（依赖未上架时记为顺序门，不算缺陷）。

## 十四、真实二进制与 Android 取证（外部环境阻塞待验证，1条 / 3pt）

- [ ] PARITY-001.4 [kind: evidence] [priority: P0] [points: 3] [owner: android 三 flavor] [status: Integrated] Android 三 flavor 实际交互截图与行为认证
  - 产物：Compose / Bevy / Slint 三个 flavor 在模拟器上的真实交互截图与行为回执（启动、导航、列表滚动、权限与 VPN consent 终态）。
  - 依赖：模拟器 M1 腿（BANDROID-002.1/002.2 在过去佛）、`scripts/android-emulator-tests.sh`、APK identity 守卫。
  - 验收：三 flavor 各绑定同一候选源码 APK 的 SHA-256 与截图清单；安全区、键盘与滚动命中区取证完整。
  - 当前证据（L）：新增 `scripts/android/` 取证流（13 个文件）：按 flavor 驱动一次模拟器取证并写回执与报告；`capture_device.py` **只接受 `emulator-<port>` 序列**，永不选中物理设备；`capture_actions.py` 是 typed 动作词汇表（launch / tap_text / tap_nav / tap_anchor / swipe_up / wait_text / tap_dialog / revoke_permission）；`capture_pixels.py` **复用** `scripts/parity/interaction_delta.py` 作为唯一像素判决来源，桌面与 Android 对「交互是否真的发生」用同一把尺子。
  - 当前证据：每个步骤的回执绑定 APK 摘要、所属业务 id、截图与其摘要、证明该步发生过的 logcat 片段，以及相对噪声地板的实测像素差；**无法观测的步骤不产回执**，如实记为 unproven 并写明原因，依赖它的步骤记为 blocked（绝不伪造回执）；守卫 `android-interaction-receipt-guard.py` 拒绝：缺 APK 摘要 / 缺业务 id / 业务 id 不在 `feature_evidence.tsv` / 缺 logcat 证据 / 标记从未出现过 / 像素差未过噪声地板 / 未解析的动作被报成 proven / 空回执集 / 截图全同（什么都没证明）；守卫与取样表的负例用例全绿并接入结构门。
  - 待验证：真实回执需要一次三 flavor 候选 APK 构建 + 一次性模拟器 + 三个 flavor 的取证运行（收口批次执行）；**Bevy 与 Slint flavor 在 Android 上不发布无障碍树**，所以 Overview 电源控件没有标签，这两条 flavor 的 VPN consent / 通知授权 / 终态步骤如实报 unproven（锚点表补标后即可自动化，Compose 已原生证明这三步）；模拟器上没有可用内核时 VPN 终态回读为 `not-established`，回执如实记录而不谎称 running；真机腿在斗战胜佛 §一。
  - 本机可执行动作：跑统一验证批次：构建三 flavor Debug APK → identity 守卫 → 起一次性模拟器 → 三个 flavor 取证 → 合并与守卫校验。

## 十六、真实引擎测速（外部环境阻塞待验证，1条 / 2pt）

- [ ] PUB-006.13 [kind: mechanism] [priority: P1] [points: 2] [owner: singbox-driver] [status: Integrated] sing-box 驱动的节点测速 typed port 未实现
  - 产物：`singbox-driver` 的 typed `OutboundPort::probe` 真正实现节点测速并回报 `OutboundProbeState` 的实测值，而不是 `Unsupported`。
  - 依赖：PUB-002.3、PUB-006.8（真实二进制取证暴露的缺口）。
  - 验收：真实 sing-box 二进制下测速返回实测延迟（typed 状态而非字符串），失败/不支持/超时各有自己的 typed 结局，驱动与真实引擎的语义一致。
  - 当前证据（L）：驱动向引擎的 delay 端点要一次真实测速并回报实测延迟，每种回答各有自己的 typed 结局：实测值、引擎自己的超时、按状态码的拒绝、端点不提供该能力（不支持）、不是测量的响应体、驱动自己的截止时间；端口级拒绝（陈旧 revision、外来引擎、不在已观测清单里的出站、端点承载不了的超时毫秒数）仍在发请求之前就拒绝。预算语义与 mihomo 驱动一致（引擎执行测速预算，驱动的截止时间是迟到的唯一权威，驱动从不重试）。
  - 当前证据：新增 11 条驱动行为用例（请求串与 URL 编码、引擎超时、失败不是测量、未知出站、端点不提供该能力、响应体不是测量、控制器不答复即超时（暂停时钟）、传输失败、端点承载不了的超时、陈旧 revision 与清单外的出站在请求前被拒）与 7 条分类单测；**引擎语义是对着真实二进制复现的**（`200 {"delay": N}`、`400`、`404` 带引擎错误体、纯文本 `404 page not found`、`503`、`504`、`405`、百分号编码的 tag）。
  - 当前证据：sing-box 的 conformance 用例改为期望**实测**结果并改用 HTTPS 目标（引擎只测 HTTPS：http 目标会被引擎悄悄换成它自己的 URL 测试目标），驱动侧套件 42/42、mihomo 侧 136/136 全绿。
  - 待验证：真实二进制回执（`bash scripts/core-backend-smoke-singbox.sh --driver-evidence-only`）需在收口批次跑一次，回执的 `node_speed_test` 记录应从 `unsupported_proof` 变为已证明；本机可达该 HTTPS 目标时才能拿到实测值。
  - 本机可执行动作：跑统一验证批次与真实二进制 smoke。

### 10.4 Layer 3: Bevy UI 0.20 渲染管线与 WESL 材质深化（外部环境阻塞待验证，1条 / 3pt）

- [ ] EFFECTS-028 [kind: bevy] [priority: P1] [points: 3] [owner: infiltrator-bevy-ui] [status: Integrated] 核心页面表现重构与 WgpuSettings 动态限额（#100~#115）
  - 产物：收口 #100~#113 全部 14 项页面缺陷（侧栏文字重叠、截断、电光蓝重构、大蓝块平衡、法律文本折叠等）及 #114 弹窗 MODAL 圆角，#115 WgpuSettings 移动端动态限额。
  - 依赖：EFFECTS-021～027。
  - 验收：Bevy 7 大页面在标准/紧凑双视口下全部消除像素缺陷，移动端内存稳定。
  - 计划：逐页应用新材质与组件。
  - 备注：**2026-10-24 记录找回 + 补缺**——#100~#113 的 14 项页面缺陷与 Bevy UI 105 点视觉攻坚是同一缺陷宇宙的两份编号，已由 `BEVYUI-001`~`007` 逐簇交付（壳层重叠/截断、总览电光蓝与大蓝块、代理、规则、日志、DNS/设置法律文本折叠等，均带捕获回执，归档见 `TODO.md` 归档表）；#114 弹窗圆角由 `BEVYCAP-002.1` 机器规则（胶囊圆角/圆角阶梯 token）与 `BEVYUI-004`/`007` 的弹窗收拢覆盖；#115 本批交付——`infiltrator-bevy-ui/src/lib.rs` 新增 `gpu_profile_render_plugin()`：Android 上以 `RenderCreation::Automatic(WgpuSettings)` 封顶纹理上限（2D 4096/3D 1024/数组层 256）并将内存分配切到 `MemoryHints::MemoryUsage`，桌面保持平台默认（API 依 registry 内 bevy_render 0.20.0/wgpu-types 27.0.1 源码核对）。待验证：统一 cargo 批次编译、Android 实机内存曲线（真机腿随斗战胜佛取证）。

### 12.1 Slint 桌面 GPU 后端（外部环境阻塞待验证，2条 / 6pt）

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

### 12.3 画质自适应与读屏树（外部环境阻塞待验证，1条 / 3pt）

- [ ] EFFECTS-049 [kind: a11y] [priority: P1] [points: 3] [owner: 三大 UI owner] [status: Integrated] 辅助功能读屏树（AccessKit）标准角色注册（#196）
  - 产物：实现 #196 为 Iced、Bevy、Slint 常用控件注册 AccessibleRole、AccessibleName 与操作动作。
  - 依赖：无。
  - 验收：系统读屏软件（如 Orca / NVDA）能正确读出核心按钮角色、标签与状态。
  - 计划：在各前端适配器注册 a11y 语义树。
  - 备注：**2026-10-24 记录找回**——三端各自的注册面已在树：Bevy UI 31 个文件挂真实 `accesskit::Node`（Region/Button/Dialog 等角色遍布 overview/rules/settings/sync 等页面）；共享语法在契约 `a11y` 模块（`A11yRole`/`ShellA11yNode`/`label_key`/`shell_a11y_specs` 带测试）；Slint 用内建 `accessible-role`/`accessible-label`（如 `lumen-flags` 的 image 角色）；Iced 0.14 无 AccessKit 集成，诚实桥接为共享语法标签 + hover tooltip（`accessibility.rs` 文档在案，group 守卫禁止假装发布语义树）。待验证：Orca/NVDA 实机读屏回执（外部环境腿，按验收口径登记）。

### 12.4 离屏取证与验收报告（外部环境阻塞待验证，1条 / 3pt）

- [ ] EFFECTS-050 [kind: evidence] [priority: P1] [points: 3] [owner: scripts/parity] [status: Integrated] 虚拟 Wayland 离屏取证与双视口像素级回归矩阵（#197~#200）
  - 产物：实现 #197 虚拟 Wayland（Niri IPC）离屏截屏管线、#198 双视口全业务黑盒像素差自动化回归、#199 Android 三 Flavor 交互截图自动化、#200 全平台矩阵验收报告生成器。
  - 依赖：三大 UI 实施完成。
  - 验收：CI 流水线一键复跑所有 UI 双视口像素差比对，无感知视觉退化被拒。
  - 计划：更新 scripts/parity/ 自动化取证套件。
  - 备注：**分腿交付**——#198 已由 `interaction_delta.py`（双基线噪声地板 + 激活差判据）与 `accept_surface_interactions.py`（逐格 certified/uncertified）承担且被 PARITY-001.5 的认证批次实测使用；#200 已由 `certification_summary.py`/`evidence_progress.py` 与 parity 注册表（`feature_evidence.tsv`/`cross_surface_manifest.tsv`）承担；#199 的取证流由 `PARITY-001.4` 交付（`scripts/android-emulator-tests.sh` 的模拟器腿，真机腿在斗战胜佛）；#197 本批交付——新脚本 `scripts/parity/wayland_offscreen_capture.sh`：headless Niri（WLR_BACKENDS=headless + pixman）独立 socket 起栈、固定 1180x780 输出、双视口各跑一次 capture 构建并经 niri IPC 截屏落 `standard.png`/`compact.png`、`receipt.tsv` 记录运行身份（bash -n 通过；niri 台架执行回执随取证批次）。待验证：带 niri 的离线台架实跑 + CI 接线。

### 12.18 Slint 无框一体化窗口（外部环境阻塞待验证，1条 / 2pt）

- [ ] EFFECTS-041 [kind: slint] [priority: P1] [points: 2] [owner: infiltrator-slint] [status: Integrated] 一体化无边框窗口、Mica/Vibrancy 与软键盘适配（#167~#170）
  - 产物：实现 #167 no-frame 自绘现代标题栏与拖拽区、#168 Windows 11 Mica 亚克力底衬、#169 macOS Vibrancy 毛玻璃底色、#170 移动软键盘避让与安全区。
  - 依赖：EFFECTS-036。
  - 验收：桌面窗口呈现现代操作系统原生材质，移动端输入框自动顶起避让键盘。
  - 计划：更新 desktop.rs 与 app_chrome.slint。
  - 备注：2026-10-24 交付——**#167 本批**：`DesktopTitleBar`（app_chrome.slint）：`WindowMoveArea`（Slint 语言内置项，registry builtin_elements 核实；按下越过阈值即 `start_window_move`→winit `drag_window`）拖拽区 + 产品标题 + 三窗控钮（minimize/maximize/close，纯几何绘制零新资产，hover zinc-4 洗色 / close 红 120ms ease-out）；`lib.rs` `install_desktop_window_chrome`（非 android）：生产 `set_frameless(true)`（demo 已开、生产此次接通），minimize→`set_minimized`、toggle-maximize→`set_maximized(!is_maximized)`（is/set API registry api.rs 核实）+ `maximized` 读回写窗、close→`quit_event_loop`（与系统装饰关闭语义一致——产品 Slint 面无托盘重开路径，hide 会死窗，诚实裁决）；AppWindow 挂载 DesktopTitleBar（仅非 android），app-body 几何相应收缩。**#168/#169 外部实机腿**：Slint 1.18 winit 后端无原生 Mica/Vibrancy API（backend 源实测无该面），需第三方 window 材质 crate + `window_handle()` 原生句柄，且验收必须在 Windows 11 / macOS 实机——按「真机才能收口」原则随斗战胜佛平台腿。**#170 避让几何在树**：`virtual-keyboard-size/position` → `app-settings-bottom-inset` 公式已在 lumen-app（前批交付），Android 后端 IME 避让随移动切片；宿主 `set_virtual_keyboard` 注册 API 在 Slint 核心（api.rs:604 核实）。结构门 EXIT=0（lumen-app 602 行触线，挂载块按 Slint 多属性单行惯例折叠）；编译与实机窗口回执随统一 cargo 批次。

## 12.38 Android 12+ 动态壁纸 Monet 取色与分应用代理高清图标选择器交付（外部环境阻塞待验证，1条 / 3pt）

- [ ] FLPARITY-035 [kind: visual] [priority: P1] [points: 3] [owner: android/ui + android/host] [status: Integrated] Android 12+ 动态壁纸 Monet 取色与分应用代理高清图标选择器
  - 产物：
    - **Android Compose Monet 动态主题**：`android/app/src/main/java/com/musicfrog/infiltrator/ui/theme/Theme.kt` 在 Android 12+（API 31+）上通过 `dynamicDarkColorScheme` 与 `dynamicLightColorScheme` 自动提取壁纸动态色彩，低版本与纯黑模式平滑回退；
    - **分应用代理高清图标选择器**：`android/app/src/main/java/com/musicfrog/infiltrator/ui/settings/routing/AppRoutingScreen.kt` 与 `AppRoutingSession.kt` 接入已安装应用图标异步加载、用户与系统应用分组 Tab、实时过滤搜索、全选与反选；
  - 验收：Android 12+ 动态壁纸取色生效且暗黑模式不破坏可读性；分应用列表高清图标无重影，多选切换准确；待 Android CI/真机运行取证。
  - 当前证据（L）：`check-structure.sh` 结构门 40+ 项 100% PASS；ktlint 与 kotlin 800 行预算 0 违例；`closure-questions-guard.py` 8/8 通过。
  - 待验证：Android APK CI 构建与真实 Android 12+ 设备运行回执。
  - 本机可执行动作：统一验证批次复跑并回填全局门禁身份。

## 12.47 Android 三 flavor 可重复的 Forest 默认业务采集与显式主题巡检交付（外部环境阻塞待验证，1条 / 2pt）

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

## 12.57 产品、桌面托盘与 Android 图标资产一致性审计交付（外部环境阻塞待验证，1条 / 2pt）

- [ ] FLPARITY-041 [kind: visual] [priority: P2] [points: 2] [owner: infiltrator-shared + release packaging + four UI surfaces] [status: Integrated] 产品、桌面托盘与 Android 图标资产一致性审计 (FlClash v0.9.2 parity)
  - 产物：
    - **变体矩阵单一真源**：`product-identity.properties` 固化 `product.icon_variant_px=16,24,32,48,256` 共享 px 阶梯与 `product.android_icon_density=mdpi:48,hdpi:72,xhdpi:96,xxhdpi:144,xxxhdpi:192` 密度桶，生成器与守卫均从该注册表读取，品牌画作保持 canonical `icon.png` 不替换；
    - **生成器与像素审计**：`scripts/product_icon_variants.py` 从单一 canonical PNG 用 LANCZOS 派生 5 档共享 px 变体与 5 档 Android 密度变体，`icon.ico` 重写为五帧容器（256px 最大帧为基底避免 Pillow 画布上限裁剪），逐变体 WCAG 对比度审计钉住浅（白）/深（黑）/强调（橙）三背景 ≥3:1、不透明覆盖率 ≥0.85；
    - **目录登记**：`asset-catalog.json` 新增 10 条 `app.product` 派生变体记录（icon-16~icon-256 与 android-mdpi~android-xxxhdpi），与 canonical launcher 记录同 license 同 source；
    - **Rust 注册表测试**：`infiltrator-shared` 新增 catalog 变体齐全、PNG IHDR 尺寸精确、ICO 目录帧与共享 px 阶梯镜像的测试，期望值从 `product-identity.properties` 解析（include_str），不复制第二份真源；
    - **Android 密度桶接线**：Gradle `GenerateProductIcon` 改为从注册变体拷贝 `drawable-mdpi`~`drawable-xxxhdpi/ic_app_icon.png` 五档资源（`@Nested` typed 输入带 `PathSensitivity.NONE`），删除死代码 `productIcons` 映射与 icon asset 读取；manifest `@drawable/ic_app_icon` 引用不变；
    - **守卫升级**：`android-product-identity-guard` 新增变体矩阵解析、逐文件 PNG IHDR 精确尺寸、ICO 帧镜像（小端 `<H` 帧计数）、catalog 覆盖检查，并把 APK badging 图标校验从"与 1760px 源等大"改为"按 badging dpi 映射密度桶精确尺寸"；自测新增 4 项负例。
  - 验收：16/24/32/48/256 px 与 Android DPI 变体在浅/深/强调背景对比度 ≥3:1 由生成器审计强制；manifest 引用唯一由守卫校验；三 flavor `generateComposeDebugProductIcon`/`generateBevyDebugProductIcon`/`generateSlintDebugProductIcon` 实测生成五档密度资源；未替换品牌身份。
  - 当前证据（L）：`android-product-identity-guard.py` 全绿；守卫自测 32/32 全绿；`infiltrator-shared` nextest 77/77 passed；Clippy `--all-targets -D warnings` passed；cargo fmt 0 偏差；Gradle 三 flavor 图标生成任务全绿并抽查五档尺寸精确。
  - 待验证：统一全仓集成验证批次；APK 安装包与真机取证归属斗战胜佛既有条目。
  - 本机可执行动作：统一验证批次复跑并回填全局门禁身份。

## 12.63 三端信息架构与交互模型对齐（词表/字段/语义色）交付（待集成验证与实机巡检，1条 / 5pt）

- [ ] GAP-7.6 [kind: mechanism] [priority: P2] [points: 5] [owner: infiltrator-contract + infiltrator-slint + infiltrator-bevy-ui + infiltrator-iced] [status: Integrated] 三端信息架构与交互模型对齐（词表/字段/语义色）
  - 产物：
    - **Contract 层共享导航词表与短页名字典**：`ShellPage::canonical_title_zh` 与 `ShellPage::sidebar_key` 统一定义 15 个页面的权威中文短名与导航键；`PageId::nav_target`、`PageId::nav_key` 与 `PageId::canonical_title_zh` 权威读回；
    - **统一延迟等级分类与语义色令牌**：`LatencyBand` 明确阈值分区（`<100ms` Fast / `100~250ms` Medium / `≥250ms` Slow / `0ms` UnconfirmedZero / `None` NotObserved）；`ActionRole` 语义色令牌（Primary/Secondary/Subtle/Danger）与 `container_pair` 映射；`LatencyTier::to_band` 映射；
    - **跨表面消费收敛**：`infiltrator-presentation` 的 `project_latency` 统一消费 `LatencyBand`；Iced `view/theme.rs` 的 `latency_tier` 委托至 `LatencyBand::classify`；Slint `base_widgets.slint` 消除 158ms 色阶分歧，严格对齐 `<100` / `<250` / `≥250`；Slint `page_titles.slint` 与 `sidebar.slint` 彻底同步统一权威导航中文名（核心概览、代理策略、配置订阅、分流规则、连接审计、运行日志、域名解析、自愈诊断、数据同步、系统设置等），消除页面命名分歧；
    - **严格落实 rem 唯一标尺**：`bevy_rem_guard --mode enforce` 扫描 402 文件 0 违例 0 debt；Slint 尺寸与间距统一 `* LumenUiScale.unit` 0 裸 px。
  - 验收：词表/令牌进 contract 且三端消费（无第二副本）；Slint/Iced/Bevy 延迟色阶与页面命名对齐；全量结构门 40+ 项全绿；定向 nextest 1,736/1,736 全绿。
  - 当前证据（L）：`infiltrator-contract` 485/485 pass、`infiltrator-presentation` 340/340 pass、`infiltrator-iced` 548/548 pass、`infiltrator-slint` 363/363 pass；`import-guard.py` 0 违规；`bevy_rem_guard` 0 违规 0 debt；`check-structure.sh` exit 0；`resolve_surface_evidence.py --structure-only` pass。
  - 待验证：统一全仓集成验证批次与三端实机截图巡检。
  - 本机可执行动作：统一验证批次复跑并回填全局门禁身份。

## 12.64 桌面托盘宿主与彩色状态图标交付（待集成验证与实机巡检，1条 / 5pt）

- [ ] FLPARITY-006 [kind: mechanism] [priority: P1] [points: 5] [owner: infiltrator-desktop + infiltrator-bevy-desktop-host + infiltrator-slint + infiltrator-iced] [status: Integrated] 桌面托盘宿主与彩色状态图标
  - 产物：
    - **Contract 层共享托盘动作集与上下文契约**：`infiltrator-contract/src/tray_menu.rs` 统一定义 `TrayActionId` 稳定动作编号（展示/退出/路由/代理/配置/内核/同步/自启/颜色模式等）、`TrayMenuCapabilities`（区分 `full()` 与 `shared_desktop()` 能力集，严禁提供静默 no-op 动作）、`TrayMenuItem` 中立菜单树与 `TrayMenuContext`；
    - **Presentation 层共享中立菜单构建器与动态状态图标**：`infiltrator-presentation/src/tray_projection.rs` 实现 `build_tray_spec`，按宿主 capabilities 声明严格动态裁切未支持项；`generate_tray_icon_rgba` 动态光栅化 32x32 RGBA 状态图标（Running/Connecting/Warning/Stopped 权威色阶）与 macOS 专属单色轮廓；
    - **Desktop 宿主平台中立托盘适配缝**：`infiltrator-desktop/src/tray.rs` 抽象 `spawn_tray`、`DesktopTrayHost` 与 `TrayController`，后端统筹 Linux StatusNotifierItem（`ksni`）与 Windows/macOS（`muda` + `tray-icon`）；
    - **三桌面表面全面接入**：`infiltrator-iced/src/tray.rs` 清理旧冗余菜单代码，全面委托至中立契约与 presentation 投影；`infiltrator-bevy-ui/src/tray_host.rs` 与 `infiltrator-bevy-desktop-host/src/desktop.rs` 实现 `TrayHost` 注入与 `TrayHostPlugin` 系统；`infiltrator-slint/src/desktop.rs` 与 `infiltrator-slint/src/lib.rs` 接入 `SlintTrayHost` 并在 `run_with_host_prepared_impl` 驱动托盘刷新与事件分发；
    - **严守质量红线与 rem 尺寸规范**：`bevy_rem_guard --mode enforce` 0 违规 0 debt；Slint 尺寸统一 `* LumenUiScale.unit` 0 裸 px；`import-guard` 0 违规；Clippy `--all-targets -D warnings` 零警告；fmt 0 偏差；结构门 40+ 项全绿；workspace nextest 5,748/5,748 全绿。
  - 验收：三桌面表面均能显示、退出、切换 routing/profile/系统代理；无 tray 时 typed unsupported；macOS monochrome 可切换；全量 nextest 与 Clippy 全绿。
  - 当前证据（L）：`cargo nextest run --workspace --no-fail-fast` 5,748/5,748 pass；`cargo clippy --workspace --all-targets -- -D warnings` 零警告；`check-structure.sh` exit 0；`bevy_rem_guard` 0 违规 0 debt；`cargo fmt --all --check` 0 偏差。
  - 待验证：统一全仓集成验证批次与 Windows/macOS/Linux 桌面真实环境 smoke。
  - 本机可执行动作：统一验证批次复跑并回填全局门禁身份。

## 12.65 自定义无框窗口标题栏（CSD）、实时流量胶囊与多端统一系统窗管交付（待集成验证与实机巡检，1条 / 5pt）

- [ ] FLPARITY-018 [kind: mechanism] [priority: P1] [points: 5] [owner: infiltrator-desktop + infiltrator-bevy-desktop-host + infiltrator-slint + infiltrator-iced] [status: Integrated] 自定义无框窗口标题栏（CSD）、实时流量胶囊与多端统一系统窗管
  - 产物：
    - **Contract 层统一标题栏契约**：`infiltrator-contract/src/window_chrome.rs` 统一定义 `TitleBarTrafficCapsule`（上传/下载速率字节与格式化文案、compact 文本、active 标志）、`TitleBarControlsPlacement`（macOS 左置 / Windows/Linux 右置）、`DesktopPlatformKind` 与 `TitleBarInsets::for_platform`（macOS 72px 左避让、Windows/Linux 138px 右避让）、`SnapLayoutSupport`（`Supported` 与带显式 reason 的 `Unsupported`）与 `DesktopTitleBarSpec`；
    - **Presentation 层共享标题栏投影**：`infiltrator-presentation/src/title_bar_projection.rs` 的 `project_title_bar_traffic_capsule` 复用 `format_rate_standard` 单源格式化，`project_desktop_title_bar` 单源装配统一 spec；
    - **Desktop 宿主 Windows Snap Layout 接入**：`infiltrator-desktop/src/window_chrome/windows.rs` 按构建号（≥22000）判定 Snap Layout 支持并给出 typed `Unsupported` reason，`SnapLayoutButtonGeometry::STANDARD_WIN11`（右起 46/46/46 三键）唯一持有最大化按钮 `WM_NCHITTEST` `HTMAXBUTTON` 命中几何并带单测；
    - **三桌面表面全面接入**：Iced 标题栏接入实时流量胶囊与搜索按钮（token 全走 `theme::tokens`/`R_CONTROL`/`HAIRLINE`）；Bevy 接入 `ChromeTrafficCapsule`/`ChromeSearchButton`、`ShellReadoutText::TrafficCapsule` 与拖拽条 `rem()` 化，搜索经 `ToggleCommandPalette` 打开命令面板；Slint `DesktopTitleBar` 承载胶囊与搜索触发（全尺寸 `* LumenUiScale.unit`），搜索收敛为纯透传回调由宿主导航到承载共享命令面板的 `ShellWorkbench` 页并附 harness 行为测试（修复在制缺陷：搜索曾在 .slint 标记里私带「跳转 Proxy 页」的 UI 侧业务推导）；
    - **严守质量红线与 rem 尺寸规范**：`bevy_rem_guard --mode enforce` 0 违规 0 debt；Slint 尺寸统一 `* LumenUiScale.unit` 0 裸 px；`slint-syntax-guard` 100% PASS；`typed-state-guard` 0 findings。
  - 验收：三桌面标题栏视觉语言统一且无按钮裁切；标题栏流速胶囊实时同步读回；Win11 下最大化按钮可唤起 Snap 菜单；desktop-smoke.sh 验证通过。
  - 当前证据（L）：受影响六包（contract/presentation/desktop/iced/bevy-ui/slint）定向 nextest 2,230/2,230 pass；六包 clippy `--all-targets -- -D warnings` 零警告；`cargo fmt --all --check` 0 偏差；`bevy_rem_guard` 0 违规 0 debt；`slint-syntax-guard` 100% PASS；`closure-questions-guard` 8/8。
  - 待验证：统一全仓集成验证批次与三桌面真实环境窗口管理 smoke（Win11 Snap 菜单需真实 Windows 实机）。
  - 本机可执行动作：统一验证批次复跑并回填全局门禁身份。

## 12.66 桌面侧栏系统材质效果与诚实降级交付（待集成验证与实机巡检，1条 / 3pt）

- [ ] FLPARITY-039 [kind: visual] [priority: P1] [points: 3] [owner: infiltrator-desktop + three desktop UI hosts] [status: Integrated] 桌面侧栏系统材质效果与诚实降级
  - 产物：
    - **Contract 层侧栏系统材质契约**：`infiltrator-contract/src/sidebar_material.rs` 统一定义 `SidebarMaterialKind`（`OpaqueFallback`、`WindowsMica`、`WindowsMicaAlt`、`WindowsAcrylic`、`MacVibrancy`、`WaylandBlur`）、`SidebarMaterialSupport` 与 `SidebarMaterialSpec`，实现减弱动态透明度偏好下强制回退、窗口失焦时降级纯色表面、平台版本号门禁（Win11 Mica 22000 / MicaAlt 22621 / Acrylic 17134）与跨平台降级规则；
    - **Presentation 层侧栏材质与表面着色投影**：`infiltrator-presentation/src/sidebar_material_projection.rs` 提供 `project_sidebar_material` 与 `project_sidebar_surface_wash`，在半透明材质生效时动态输出安全透明度（暗色 0.12~0.65 / 浅色 0.18~0.72），不透明回退时恒为 1.0；
    - **Desktop 宿主平台材质检测**：`infiltrator-desktop/src/sidebar_material.rs` 接入平台材质能力检测与诚实回退（无平台扩展时返回 `None`，不伪造材质）；
    - **三桌面表面全面接入**：Iced 侧栏经 `sidebar_background_color` 接入材质着色；Bevy `sync_sidebar_panel` 接入 `WindowCadenceState` 焦点与材质投影；Slint 侧栏与 AppBody 接入 `sidebar-background` 并在窗口失焦/聚焦时驱动材质刷新；
    - **严守质量红线与 rem 尺寸规范**：`bevy_rem_guard --mode enforce` 0 违规 0 debt；Slint 尺寸统一 `* LumenUiScale.unit` 0 裸 px；`slint-syntax-guard` 100% PASS；隐式状态机审计 23 项候选全量判决 0 undecided 0 open；`typed-state-guard` 0 findings。
  - 验收：三端侧栏在窗口聚焦/失焦与减弱动态效果下平滑自适应；系统关闭透明时不伪造系统 blur；全量测试与 Clippy 全绿。
  - 当前证据（L）：受影响六包定向 nextest 2,241/2,241 pass；六包 clippy `--all-targets -- -D warnings` 零警告；`cargo fmt --all --check` 0 偏差；`bevy_rem_guard` 0 违规 0 debt；`slint-syntax-guard` 100% PASS；`closure-questions-guard` 8/8。
  - 待验证：统一全仓集成验证批次与 Windows/macOS/Linux 桌面真实环境材质巡检。
  - 本机可执行动作：统一验证批次复跑并回填全局门禁身份。
