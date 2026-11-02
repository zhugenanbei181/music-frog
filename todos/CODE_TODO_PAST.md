# MusicFrog 工程 TODO · 过去佛（0条，已清空；归档见 [TODO.md](../TODO.md) 第一百零四批次）

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

## 二、Android IPC 与 Kotlin Host 边界（0条，已清空；归档见 [TODO.md](../TODO.md) 第一百零四批次）

## 三、UI 表面与组件（0条，已清空；归档见 [TODO.md](../TODO.md) 第一百零四批次）

## 五、发布流水线与 parity 管线（0条，已清空；归档见 [TODO.md](../TODO.md) 第一百零四批次）

## 十四、真实二进制与 Android 取证（外部环境阻塞待验证，1条 / 3pt）

- [ ] PARITY-001.4 [kind: evidence] [priority: P0] [points: 3] [owner: android 三 flavor] [status: Integrated] Android 三 flavor 实际交互截图与行为认证
  - 产物：Compose / Bevy / Slint 三个 flavor 在模拟器上的真实交互截图与行为回执（启动、导航、列表滚动、权限与 VPN consent 终态）。
  - 依赖：模拟器 M1 腿（BANDROID-002.1/002.2 在过去佛）、`scripts/android-emulator-tests.sh`、APK identity 守卫。
  - 验收：三 flavor 各绑定同一候选源码 APK 的 SHA-256 与截图清单；安全区、键盘与滚动命中区取证完整。
  - 当前证据（L）：新增 `scripts/android/` 取证流（13 个文件）：按 flavor 驱动一次模拟器取证并写回执与报告；`capture_device.py` **只接受 `emulator-<port>` 序列**，永不选中物理设备；`capture_actions.py` 是 typed 动作词汇表（launch / tap_text / tap_nav / tap_anchor / swipe_up / wait_text / tap_dialog / revoke_permission）；`capture_pixels.py` **复用** `scripts/parity/interaction_delta.py` 作为唯一像素判决来源，桌面与 Android 对「交互是否真的发生」用同一把尺子。
  - 当前证据：每个步骤的回执绑定 APK 摘要、所属业务 id、截图与其摘要、证明该步发生过的 logcat 片段，以及相对噪声地板的实测像素差；**无法观测的步骤不产回执**，如实记为 unproven 并写明原因，依赖它的步骤记为 blocked（绝不伪造回执）；守卫 `android-interaction-receipt-guard.py` 拒绝：缺 APK 摘要 / 缺业务 id / 业务 id 不在 `feature_evidence.tsv` / 缺 logcat 证据 / 标记从未出现过 / 像素差未过噪声地板 / 未解析的动作被报成 proven / 空回执集 / 截图全同（什么都没证明）；守卫与取样表的负例用例全绿并接入结构门。
  - 待验证：真实回执需要一次三 flavor 候选 APK 构建 + 一次性模拟器 + 三个 flavor 的取证运行（收口批次执行）；**Bevy 与 Slint flavor 在 Android 上不发布无障碍树**，所以 Overview 电源控件没有标签，这两条 flavor 的 VPN consent / 通知授权 / 终态步骤如实报 unproven（锚点表补标后即可自动化，Compose 已原生证明这三步）；模拟器上没有可用内核时 VPN 终态回读为 `not-established`，回执如实记录而不谎称 running；真机腿在斗战胜佛 §一。
  - 本机可执行动作：跑统一验证批次：构建三 flavor Debug APK → identity 守卫 → 起一次性模拟器 → 三个 flavor 取证 → 合并与守卫校验。

## 十六、真实引擎测速（0条，已清空；归档见 [TODO.md](../TODO.md) 第一百零四批次）

### 10.4 Layer 3: Bevy UI 0.20 渲染管线与 WESL 材质深化（0条，已清空；归档见 [TODO.md](../TODO.md) 第一百零四批次）

### 12.1 Slint 桌面 GPU 后端（0条，已清空；EFFECTS-036 与 SLINTUI-001 已由第一百零五批次办结归档）

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

### 12.18 Slint 无框一体化窗口（0条，已清空；归档见 [TODO.md](../TODO.md) 第一百零四批次）

## 12.38 Android 12+ 动态壁纸 Monet 取色与分应用代理高清图标选择器交付（0条，已清空；归档见 [TODO.md](../TODO.md) 第一百零四批次）

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

## 12.57 产品、桌面托盘与 Android 图标资产一致性审计交付（0条，已清空；归档见 [TODO.md](../TODO.md) 第一百零四批次）

## 12.63 三端信息架构与交互模型对齐（词表/字段/语义色）交付（0条，已清空；归档见 [TODO.md](../TODO.md) 第一百零四批次）

## 12.64 桌面托盘宿主与彩色状态图标交付（0条，已清空；归档见 [TODO.md](../TODO.md) 第一百零四批次）

## 12.65 自定义无框窗口标题栏（CSD）、实时流量胶囊与多端统一系统窗管交付（0条，已清空；归档见 [TODO.md](../TODO.md) 第一百零四批次）

## 12.66 桌面侧栏系统材质效果与诚实降级交付（0条，已清空；归档见 [TODO.md](../TODO.md) 第一百零四批次）
