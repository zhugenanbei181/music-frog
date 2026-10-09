# 双端平权剩余缺口总清单 (Dual-Surface Parity Gap Ledger)

> 层级：L2

> 本文是**派生的缺口视图**，不是新的权威来源：场景身份只来自 `FeatureId::ALL` 与 `scripts/parity/`；执行顺序、owner 与验收条件只登记在本地 `TODO.md`；平台状态只读取 [PLATFORM_MATRIX.md](PLATFORM_MATRIX.md)；差距索引见 [DEFECTS.md](../DEFECTS.md)。本文只做一件事：把每一处尚未“一碗水端平”的隐患映射到 owner 编号、文件级动作与所需证据，供多 agent 按 lane 并行认领。**本清单不构成任何完成声明。**

- 生成基线：HEAD `536b40b` 之后、依赖 lockfile 更新提交 `d7f853d`。
- 依赖锁已由 `cargo update` 更新到最新 semver 兼容版本；`cargo nextest run --workspace` = **4005/4005 通过**。

## 0. 本轮推进（代码侧，证据待补）

> 以下仅表示**代码已落地并通过 workspace/守卫/Android 编译门**；设备、像素、真机与长时证据仍未取得，不构成 DONE。

| 提交 | 覆盖 | 代码侧状态 | 仍缺（环境阻断） |
| --- | --- | --- | --- |
| `421e87c` | BANDROID-009 | Proxies 大列表接入虚拟窗口/实体池/overscan；2000 节点挂载有界、尾项可达、槽位稳定 | 真机帧耗时/内存/池数基准 |
| `723142b` | BANDROID-011 | `RenderCadence::Idling` 在 Bevy 可达：静置降到 idle 帧间隔，输入/动画唤醒 | Android 挂起停绘、真机帧/唤醒报告 |
| `a141aa6` | BANDROID-012 | `SurfacePump` 宿主唤醒句柄（可合并、无界积压修复） | UI 侧 EventLoopProxy 消费与真机后台唤醒 |
| `9806aa0` | BANDROID-013/017 | 拓扑静置帧不再重栅格化/重上传；真实卡片启用阴影高度 | 桌面像素、Android 设备 |
| `01f2168`,`6f55ca8` | BANDROID-013 | 遥测 Mesh2d GPU 管线完成（View/`clip_from_world`、`Material2d`、handle 复用/退役）+ CPU 回退 | 真实 Vulkan/Android GPU 出图（§1.2 需设备） |
| `4e3c13a` | BANDROID-010 | Connections/Logs 大列表接入回收组件 | 真机滚动/过滤/重排基准；Rules 对齐回收组件 |
| `6f55ca8` | BANDROID-016 / BEVY-030 | 2.5D 地球（来源/精度/未知语义、经纬度不可编造、弧线/粒子、Flat/Eco 回退） | 真机帧/内存/功耗 |
| `ea7f053` | 依赖 | `brotli 9`、`toml 1`、`jni 0.22`（JNI bridge 迁移） | — |
| `9004631`,`7e26067` | BANDROID-001..008 | 服务进程独立初始化、`:vpn` bridge 注册、Android `SurfacePump`/`attach_application` 入口、UI↔`:vpn` 绑定、生命周期/Insets/剪贴板原生适配、IME 归一化（BANDROID-007 经核实为 **typed unsupported**：winit 0.30 Android 后端不产 `WindowEvent::Ime`，android-activity 0.6.1 无 GameTextInput） | 全部真机/APK/Gradle 证据 |
| `ac51a7a` | BEVY-020 / BANDROID-014 | 日志有界环形缓冲；返回栈/触觉端口/减少动效接缝 | 真机返回/读屏/触觉 |
| `f3437d9` | BEVY-028/033/034/036/037/038/039/040/041 | 反应式 DAG、自愈启发式、OKLCH 色阶、TSDB+时间回放、沙盒、游戏手柄、冷启动缓存、混沌数字孪生、跨项目 ABI | 产品页接线与设备 |
| `ff95457` | BEVY-027/029/031/032/035 | SDF 边抗锯齿+实例化批次+降级开关、多窗口/PiP、无障碍/相机/原生视图接缝、程序化音频+全局静音、可变字重+BiDi 镜像 | 真机 a11y/相机/音频 |

**硬阻断（本机不可验证，必须留 OPEN）**：BANDROID-001…008/015/022…024 的设备/APK/Gradle 部分需真机 + JDK/Gradle；平台安装包需各 OS 真机；parity L3 需虚拟 KWin/niri 采集；≥8h 后台与功耗需硬件；BEVY-031 的 TalkBack/VoiceOver 与相机、BEVY-032 的真实音频输出、BANDROID-007 的完整中文 IME 亦归设备/上游。

**未落地（明确登记，不静默）**：BANDROID-015 的默认特性 release APK 整合仅到“代码就绪”（`android/bevy-host` 模块 + driver 模板，未接入 `settings.gradle.kts`，未构建）；BANDROID-018..024 的首次 CI 运行、模拟器插桩 stage、真机长时 stage 均待 CI/设备；BEVY 高级能力的产品页接线已逐项落地（`e40530b` 地球页/时间回放/Doctor 自愈/OKLCH；`9f17631` 手柄/BiDi/读屏/反馈/渲染策略/反应式投影/沙盒/ABI/混沌/冷启动/PiP），仅其真机/GPU/设备证据待补。

**发布**：本轮将 workspace 版本由 `0.40.5` 提升至 `0.41.0` 并打 `v0.41.0` tag；真机/设备/像素证据未取得的部分不得据此宣称完成。


## A. 横切阻断（先决，串行）

| # | 隐患 | 证据/位置 | 影响 |
| --- | --- | --- | --- |
| A1 | **Gradle 产品宿主与 Bevy UI 之间不存在共享进程边界**：Gradle 产品（`com.musicfrog.infiltrator`）持有 `<service>`/`<receiver>`/`foregroundServiceType`，Bevy UI 只被打包成独立的 `app.musicfrog.infiltrator_bevy_ui` fixture-demo APK | `scripts/build-bevy-apk.sh`；`android/app/src/main/AndroidManifest.xml`；`android/settings.gradle.kts`（仅 `:app`） | BANDROID-001/003/004/015 的共同根因 |
| A2 | **`attach_application` 无任何调用方**，移动产品入口在无宿主时走 `run_demo()`（或 `fixture-demo`） | `crates/infiltrator-bevy-ui/src/lib.rs:87-94`；`src/launch.rs:70-95`；仓库级 grep 无 caller | BANDROID-004 |
| A3 | **`:vpn` 进程未注册 JNI bridge**：`MihomoApplication.onCreate` 只 `loadLibrary`；bridge 仅在 UI 进程 `MainActivity.initRustBridge()` 注册 | `android/app/src/main/java/.../MihomoApplication.kt`；`MainActivity.kt`；`crates/infiltrator-android/src/jni_bridge.rs` | BANDROID-001/002：服务进程 `get_android_bridge()==None` |
| A4 | **`scripts/verify-bevy-apk.sh` 从未被任何 workflow 调用**，且硬编码用户态工具链路径 | `scripts/verify-bevy-apk.sh:18-24`；`.github/workflows/android.yml` | BANDROID-015/018 |

## B. Bevy Android 产品宿主（BANDROID-001..008、015）

| ID | 现状 | 缺失（文件级） | 依赖 | 所需证据 |
| --- | --- | --- | --- | --- |
| BANDROID-001 | manifest 已声明 `:vpn` 进程；仅有进程内 `OnceLock` 单例 | 新 IPC 模块（`crates/infiltrator-android/src/`）；AIDL/Binder 绑定服务；请求身份+代次跨进程；重绑读回 | 无 | 旧回执拒绝、断线/重绑副作用、真实 PID/manifest、UI 终止 VPN 存活 |
| BANDROID-002 | Kotlin `VpnService`（`START_STICKY`/前台通知/`onRevoke`/`BootReceiver`）已存在 | 服务进程独立初始化（bridge/runtime/目录/凭据）；always-on/`prepare` 路径；有时限 WakeLock；真实 controller readiness（现为 `Thread.sleep(500)`） | 001 | 无 Activity 冷启动；拒权/前台失败/TUN 失败/内核退出各有真实终态；真实流量路由/DNS/分应用/无回环；FD/worker 回收 |
| BANDROID-003 | 仅 NativeActivity smoke | 真正的 Gradle 承载 Activity + 与 VPN 服务绑定；GameActivity/GameTextInput 或 InputConnection adapter | 001 | 默认特性 APK 的 Activity 启动/重建并保持服务绑定；原生文本事件进入 Bevy |
| BANDROID-004 | `run_with_application*` 与 `ApplicationSurfaceSource` 存在；`attach_application` 无 caller | 组装 `CoreApplication` + Android `SurfacePump`（对照 `crates/infiltrator-desktop/src/surface.rs`）并调用 `attach_application` | 001/002/003 | 原生 UI→IPC→Rust→真内核→读回闭环；失败保留类型；未组合保持 unavailable |
| BANDROID-005 | 仅上游 Bevy/winit surface | 真实 `AppLifecycle`/焦点/遮挡/可见性；暂停恢复；Activity 重建退役旧操作 | 003/004 | 合成生命周期测试；真机锁屏/切后台/旋转/分屏 ≥100 次循环 |
| BANDROID-006 | Compose 用 `enableEdgeToEdge()`；Bevy 侧仅合成安全区 | `WindowInsetsCompat`/`WindowInsets` 获取与密度换算注入现有安全区消费 | 003 | 真机竖横屏/分屏/键盘/手势可达，无重复边距 |
| BANDROID-007 | `ime.rs`/`ime_native.rs` 状态机存在；无 Android `InputConnection`/`GameTextInput` | 原生文本宿主接入既有 IME owner；返回先关键盘再导航 | 003/006 | ARM64 真机中文候选不重复/丢字；≥2 输入法记录版本；桌面 IME 回归 |
| BANDROID-008 | `ClipboardHost` 资源存在；Compose 仅 `LogsScreen` 用 `LocalClipboardManager` | Android `ClipboardManager` 宿主适配器，连接复制/订阅导入/SDK 粘贴共用 | 003/004 | 跨应用复制粘贴互通；空/拒绝/无效/写失败零假成功；桌面回归 |
| BANDROID-015 | `build-bevy-apk.sh` 产 fixture-demo smoke；`verify-bevy-apk.sh` 孤立 | 默认特性 release APK 的宿主/内核/资产/权限整合入口；修复 `verify-bevy-apk.sh` 并接入 CI | 001..014 | APK/源码哈希、默认引擎/ABI/SDK/NDK/许可证/回滚；真机 VPN+输入、100 次生命周期、≥8h 后台；四场景能耗原始报告 |

## C. Android CI 与工具链（BANDROID-018..024）

| ID | 现状 | 缺失 | 依赖 |
| --- | --- | --- | --- |
| BANDROID-018 | `scripts/android-check.sh` + `.github/workflows/android.yml` 已就绪，本地验证通过 | 仅缺**首次 GitHub Actions 运行**证据 | 与 020 联合 |
| BANDROID-019 | `android-manifest-guard.py` 已接入 `--guards-only`；产品清单合规 | Bevy APK 无法经 `cargo-apk` 声明 `<service>`；最终归属 003/015 | 003/015 |
| BANDROID-020 | 两端 min29/target36/NDK `29.0.14206865` 已统一 | 首次 CI 运行确认 | 无 |
| BANDROID-021 | `VpnStateManagerTest.kt` + JUnit4 已就绪 | 首次 CI 运行确认 | 018 |
| BANDROID-022 | `ManifestDeclarationsTest.kt` + runner 已就绪，CI 只 `assembleDebugAndroidTest` | **x86_64 API36 模拟器 stage 跑 `connectedDebugAndroidTest` 不存在** | 018/021 |
| BANDROID-023 | `android-device-evidence.sh` + `android-device.yml`（self-hosted）已就绪 | 无 self-hosted 真机运行；≥8h soak 与四能耗场景未采集；脚本本身不验证 VPN 流量/无回环 | 015 |
| BANDROID-024 | 依赖版本已在 build 文件声明 | 首次 Gradle resolve/编译确认 AGP↔Gradle↔KGP 兼容 | 018/020 |

## D. Bevy 列表与运行时（BANDROID-009..014、016、017；BEVY-006/013/015/020）

| ID | 现状 | 缺失（文件级） | 所需证据 |
| --- | --- | --- | --- |
| BANDROID-009 | 回收引擎（`VirtualListState`/`VirtualEntityPool`/overscan/动态行高）**完整** | **零页面消费**：`pages/proxies/scene.rs`、`pages/proxies_card.rs` 仍全量建实体 | 500/2000 节点滚到尾项；挂载槽受视口+overscan 约束；槽 ID 复用；旧槽操作被拒；帧耗时/内存基准 |
| BANDROID-010 | Rules 走 domain window；Connections/Logs 全量重建 | `pages/connections_rows.rs`、`pages/logs_rows.rs` 接回收组件；Rules 改用回收组件 | 10k 连接/50k 规则/持续日志；过滤/重排/空后增长/resize；池/纹理/observer 有界 |
| BANDROID-011 | `contract::cadence` 策略与 Winit 投影存在 | `RenderCadence::Idling` 在 Bevy **不可达**；widgets `CadenceGovernor`/`RequestCadenceWake` 未注册；无 Android 挂起停绘 | 动画结束退出活跃；无变化不排帧；挂起不渲染；输入唤醒；Iced/Bevy 策略一致；真机帧/唤醒报告 |
| BANDROID-012 | `SurfacePump` 有界 channel + drain 存在 | **无 host 唤醒**（无 `EventLoopProxy`）；`pump_loop` 的 wake 通道从不被触发；无可见页观察/取消 | UI 睡眠仍收命令终态；入睡/入队竞争、饱和、断线、恢复均不丢终态；后台停无用轮询 |
| BANDROID-013 | 2D 波形/拓扑已交付 | **拓扑每帧重复光栅化**：`advance_topology_flow` 每帧改 `flow_phase` → `sync_topology_charts` 每帧重传；GPU Mesh2d/WESL 原型未安装 | 无变化不重栅格化/上传；变化刷新同一纹理身份；100 次资产基线；CPU/GPU 同样本帧耗时/功耗 |
| BANDROID-014 | 桌面返回栈/AccessKit/触控存在；haptics 纯数据 | 无 Android 返回栈接线、TalkBack 桥、Vibrator、减少动效/节能偏好 | 真机返回关闭正确层级；侧滑/下拉不穿透模态；读屏可达；触觉可关 |
| BANDROID-016 | 仅 2.5D 数学原型 | 3D 视口/插件、拖拽、地理标记、流量弧线、Eco 回退 | 不从节点名/Emoji/RTT 编造坐标；离页释放；静置休眠；帧/内存/功耗报告 |
| BANDROID-017 | Squircle 材质 + WESL 拆分 + 插件**已注册产品** | 真实卡片恒用 `ModernSurfaceElevation::None`（`surface_shader.rs`）→ `shadow.wesl` 未在产品启用 | 桌面实机模块链接 + 双主题/视口像素；Android 编译/APK/设备证据分别登记 |
| BEVY-006 | 历史模拟器截图 | 真机 ARM smoke、图标打包、窄屏响应式、TalkBack、blake3 `no_neon`、strip | 由 BANDROID-001..015 承接 |
| BEVY-013 | 回收算法存在 | 页面级回收未证明（同 BANDROID-009/010） | 原生滚动、稳定身份、全可达、资源基准 |
| BEVY-015 | VPN/JNI/Kotlin 基础存在 | UI/VPN 进程隔离、无 Activity 启动、Bevy 产品接通未证明 | 同 BANDROID-001/002/004 |
| BEVY-020 | 日志搜索/高亮已接线；`FixedRingBuffer` 存在 | Logs 页仍 `despawn_children` 全量重建，未用 ring buffer | 真实增长流 + 正则过滤 + 高亮 L2/L3；内存有界 |

## E. Bevy 高级能力（BEVY-027..041）——纯原型，零产品接线

每个 ID 在 `infiltrator-bevy-widgets` 均有业务无关纯核心原型 + 无头测试，但 `infiltrator-bevy-ui` **零接线**：

| ID | 原型 | 产品缺口 |
| --- | --- | --- |
| BEVY-027 | `shader_fx.rs` SDF、`chart/mesh.rs` | 计算着色器、SDF 字形 AA、GPU Instancing、降级开关 |
| BEVY-028 | `signal_dag.rs`、`reactive.rs` | `bsn!` 常量折叠、signal↔ECS 集成 |
| BEVY-029 | `windowing.rs`、`mini_hud_shell.rs` | 真多窗口共享 ECS、OS PiP 置顶/穿透 |
| BEVY-030 | `particle.rs` | 3D 视口/插件、2D 无损回退 |
| BEVY-031 | `text_input/native`、`mobile_view.rs` | TalkBack/VoiceOver JNI、相机扫码纹理 |
| BEVY-032 | `haptics.rs` | 平台 Vibrator/音频输出、全局静音 |
| BEVY-033 | `auto_heal.rs`、`chaos.rs` | 启发式最佳节点、异常可视化 |
| BEVY-034 | `OklchColor`、`theme_export.rs` | 运行时着色器热重载、OKLCH 产品接线 |
| BEVY-035 | `bidi.rs`、`text_runs.rs` | OpenType 可变字重插值、RTL 镜像 |
| BEVY-036 | `tsdb.rs` | SIMD 存储、时间轴回放 UI |
| BEVY-037 | `sandbox.rs` | 隔离组件渲染槽、社区市场 |
| BEVY-038 | `gamepad_ui.rs` | 十字键导航接线、TV 布局 |
| BEVY-039 | `boot_cache.rs` | 管线预热、mmap 零分配、PGO |
| BEVY-040 | `chaos.rs` | UI 级故障注入、无头 monkey 探索 |
| BEVY-041 | `abi.rs` | 独立 crate 抽取、跨项目 ABI 验证 |

## F. 双端场景 L3 视觉验收（parity）

- 声明层：`cross_surface_manifest.tsv` 120 格全 `implemented`；`feature_evidence.tsv` 240 格全 `anchored`；**无 `ready`**。`anchored`/`implemented` ≠ accepted。
- 机器验收层：`.evidence/` 存在**互相矛盾**的产物——最新 selected-product run（2026-10-07）报 0/60 未闭合，而治理报告报 118 未闭合。必须以 `resolve_surface_evidence.py --require-complete` + 具体 `--product-builds` 重新裁定。
- 真实缺口：**每个 feature 需要 4 张 schema-4 receipt（两端 × 1180×780/720×480）**，含激活标记、原生重绘帧、交互区像素、PNG SHA256、几何/亮度签名、归档二进制与 build manifest。

## G. 发布阻断（DEFECTS.md §当前发布阻断）

保留的阻断：①场景交互双端状态机+两视口；②全仓 i18n 裸中文；③Bevy 结构守卫+逐场景 L3；④遗留配置事务/草稿；⑤Iced 遗留所有权 + Bevy Android 宿主未组合；⑥模式场景原生布局+像素；⑦最终平台包/远端验收/公开发布；⑧i18n 后端字符串；⑨壳层未知/等待/失败/重试分支 + 向导可达性像素。

**已疑似过期、需核对后销账**：MRS 伪事实（`mrs_acceleration_application.rs`，`6391a73` 声称已修）；版本验证回执 `LAST_VERIFICATION`（grep 0 命中，已改为 `with_shared_verification`）。按 DEFECTS.md 规则，销账需真实测试/像素/发布回执。

## H. 平台与打包证据（PLATFORM_MATRIX.md §1）

Linux 安装包 smoke/托盘/TUN；Windows x64+ARM64 包/回滚/系统代理/托盘；macOS CI/签名/安装后运行；Android arm64 真机 VPN/后台/升级/异常退出；Android x86_64 模拟器 ABI/隔离；Android Bevy 全套；iOS 桥/NetworkExtension 真机；Admin API 契约。

## I. 依赖现代化（UPSTREAM.md §1/§3）

- 已完成：`cargo update` 工作区 lockfile → 最新 semver 兼容（提交 `d7f853d`，4005 测试通过）。
- **重大版本滞后（需单独评估 + 代码迁移，禁止与其它族混在同一任务）**：
  - `toml` 0.8 → 1.x（lock 已解析到 1.1.8）。
  - `jni` 0.21 → 0.22（lock 已含 0.22.4，Android crate 迁移）。
  - `brotli` 8 → 9（lock 已含 9.0.0，需核对 API）。
- 供应链疑点：`src-tauri/Cargo.lock` 与根 `Cargo.toml:4`“Tauri 已退役”矛盾，疑为陈旧第二 lockfile（UP-003）。
- 其它族（tokio/reqwest/axum/rustls/sqlx/serde/uniffi/muda/tray-icon/ksni/notify-rust/boa）当前 major 无观察到的滞后。

## J. 多 agent 并行推进（lanes）

> **铁律**：**写操作串行、每 lane 单一 writer**。并行只用于只读盘点与独立文件域。禁止两个 agent 同时对同一仓库 `git add -A`（本仓已发生过一次误捆绑提交）。

| Lane | 范围 | owner 文件域 | 依赖 |
| --- | --- | --- | --- |
| L1 | Android 宿主血脉 A1/A3 + BANDROID-001/002/003/004 | `crates/infiltrator-android/src/**`、`android/app/src/main/java/**` | 串行，先于 L2 |
| L2 | 默认特性 APK 整合 BANDROID-015 + A4 | `scripts/build-bevy-apk.sh`、`scripts/verify-bevy-apk.sh`、`.github/workflows/release.yml` | L1 |
| L3 | 列表/运行时回收 BANDROID-009/010/012 + BEVY-020 | `crates/infiltrator-bevy-ui/src/pages/**`、`crates/infiltrator-bevy-widgets/src/list/**` | 独立 |
| L4 | 图形/调步 BANDROID-011/013/016/017 | `crates/infiltrator-bevy-widgets/src/{chart,shader_fx,surface_shader}.rs`、`crates/infiltrator-bevy-ui/src/cadence.rs` | 独立 |
| L5 | 依赖现代化 §I | 根 `Cargo.toml`、`Cargo.lock`、Android build 文件 | 每族串行 |
| L6 | 双端 L3 证据 §F | `scripts/parity/**`、`.evidence/**` | 依赖 L1/L3/L4 落地 |
| L7 | CI/工具链 BANDROID-018..024 | `.github/workflows/android*.yml`、`android/**` | 独立 |
