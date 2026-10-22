# 测试与证据

> 层级：L1

本文件是全仓验证命令和阶段门的唯一入口。Android 产品细节见 [Android 文档](android/docs/README.md)，平台现状见 [平台矩阵](docs/PLATFORM_MATRIX.md)，后端共用契约见 [CORE_BACKENDS.md](docs/CORE_BACKENDS.md)，发布生态门禁见 [RELEASE_ECOSYSTEM.md](docs/RELEASE_ECOSYSTEM.md)。

## 命令入口

- `bash scripts/test.sh`：先运行 `scripts/quality/check-structure.sh` 和共享 UI 证据解析，再运行 workspace `cargo nextest`。
- `bash scripts/test.sh --guards-only`：只运行质量守卫。
- `bash scripts/test-bevy.sh`：Bevy UI 专项结构门、证据解析和 nextest。
- `bash scripts/capture-slint-matrix.sh`：Slint UI 独立无感后台截图矩阵取证。
- focused Rust 回归使用 `cargo nextest run --offline --locked -p <package> [test-filter]`。禁止使用裸 `cargo test`；test policy 会扫描并拒绝。
- Android flavors 在一个 Gradle 调用中验证，命令以 [Android 文档](android/docs/BEVY_ANDROID_PRODUCT.md) 为准；禁止把一次 flavor 的构建结果用于证明另一个 flavor。
- Android Core host mock/JVM 回归使用 `(cd android && ./gradlew :core-host-tests:test)`；该测试源集直接编译 app 中的 Core host 实现，不编译 Compose/Bevy/Slint UI。
- `cargo nextest run --locked -p lurker-sandbox`：中性运行时的无头沙盒混沌矩阵，在四类确定性故障下断言 fail-closed、租约回收与自愈。
- `python3 scripts/quality/examples-guard.py --mode compile`：中性用例展厅的 CI 编译门，四套极简用例逐个编译；`--mode smoke` 运行唯一无显示依赖的终端用例。
- `python3 scripts/quality/semver-compat-guard.py --mode check`：已发布中性 crate 的 SemVer 兼容门（源码层）；`--mode tool` 追加 `cargo-semver-checks` 的类型检查分析；`--mode negative` 是守门负例。
- `python3 scripts/developer-journey-rehearsal.py --profile fast`：开发者全旅程演练与干跑审计，全链路预算五分钟。

### 依赖政策与工程约束守卫

这些守卫是纯离线的结构门，只读 TOML、源码与脚本文本，不编译、不联网，已在 `scripts/quality/check-structure.sh` 汇总：

- `python3 scripts/quality/dependency-policy-guard.py --mode enforce`：仓库根 `deny.toml` 与第三方声明 [THIRD-PARTY-NOTICES.md](THIRD-PARTY-NOTICES.md) 的许可一致性、copyleft 禁止、来源与通配版本约束，并证明生态门禁里有 tag 门控的 `cargo deny check` 作业。
- `cargo deny check advisories bans licenses sources`：依赖政策的执行面，本机可复跑；公告库需要网络，其余检查只读锁文件。
- `python3 scripts/quality/toolchain-consistency-guard.py --mode enforce`：`rust-toolchain.toml` 与各 workflow、manifest 的 Rust 版本一致性，禁止浮动引用。
- `python3 scripts/quality/unsafe-traceability-guard.py --mode enforce`：unsafe 操作与 `scripts/quality/unsafe_inventory.json` 台账一致，缺 `SAFETY:` 说明的数量只能下降。
- `python3 scripts/quality/panic-boundary-guard.py --mode enforce`：`catch_unwind` 边界与 `scripts/quality/panic_boundary_inventory.json` 台账一致，且任何 profile 不得 `panic = "abort"`。
- `python3 scripts/quality/stdout-discipline-guard.py --mode enforce`：库层直接写标准输出的站点与 `scripts/quality/stdout_inventory.json` 台账一致，内层 crate 保持零打印。
- `python3 scripts/quality/build-surface-guard.py --mode enforce`：`build.rs` 与 `scripts/quality/build_surface_inventory.json` 台账一致，构建依赖与 manifest 一致，未声明的进程派生或文件写入被拒绝。
- `python3 scripts/quality/feature-matrix-guard.py --mode enforce`：`scripts/quality/feature_matrix.json` 与全部 `[features]` 表一致，脚本与 CI 的 `--features` 指向已登记特性。

每个守卫都支持 `--self-test` 自检，单元回归在 `scripts/quality/tests/` 下随结构门一起运行。

### 可选审计工具（非门禁）

覆盖率与变异测试是**补充证据**，不替代行为测试，也不进结构门；需要时本地手动跑：

- `cargo llvm-cov --workspace --summary-only`：覆盖率基线报告，用于发现完全没有被测到的路径；结论只作线索，缺口按行为补测试。
- `cargo mutants --package <crate>`：关键 crate 的变异测试，检查已有测试是否真的断言了行为；只在收口批次对少量 crate 抽样跑。

CI 对 macOS 和 iOS 目标暂时不编译、不打包；用户解除暂缓并更新平台矩阵/门禁之前，不得恢复 Apple matrix。

普通 push、PR 和版本分支不会自动启动编译、模拟器或全量测试。只有 `v*` tag 会进入 `release.yml`，并通过 `workflow_call` 执行 workspace `cargo nextest` gate 后再构建发行产物。Android、Bevy、parity 和真机证据的高成本专用 workflow 只能手动从 `v*` tag 派发；分支引用会在 job 启动 runner 前被跳过。Android release workflow 自身会构建三个 flavor 的 APK；专用 Android gate 额外执行两 ABI Rust compile/Clippy、各 flavor JVM 与 Android Emulator 插桩测试、APK 身份检查和静态 manifest contract guard。插桩入口是 `bash scripts/android-emulator-tests.sh`：它使用 Gradle Managed Device 建立一次性 API 36 x86_64 AOSP 模拟器、离线运行来宾系统并保存测试结果；不得用普通 `connected*` 命令把这些用例发到真机。脚本对比运行前后的宿主接口地址和 IPv4/IPv6 路由摘要，发现变化即失败，不尝试修改或恢复宿主网络。ARM64 真机证据仍由 Android 阶段验收补齐。

## 交叉目标检查

产品层 aarch64 交叉检查可在本机复跑（L 级证据；安装包/APK/设备证据仍由 CI 与设备矩阵负责）：

- `aarch64-unknown-linux-gnu`：以 `zig 0.14.1` + `cargo-zigbuild` 作为目标 C 工具链编译 `zstd-sys`/`ring`。复跑示例：`cargo check -p infiltrator-iced -p infiltrator-bevy-desktop-host -p infiltrator-slint --target aarch64-unknown-linux-gnu`；check 阶段 `openssl-sys` 经 `PKG_CONFIG_ALLOW_CROSS=1` 解析宿主头文件，链接与产物仍按 Linux release 腿处理。
- `aarch64-pc-windows-msvc`：以 `cargo-xwin`（`--cross-compiler clang`）提供 MSVC CRT/SDK。复跑示例：`CARGO_FEATURE_NO_NEON=1 cargo xwin check --cross-compiler clang -p infiltrator-iced -p infiltrator-bevy-desktop-host -p infiltrator-slint --target aarch64-pc-windows-msvc`；`CARGO_FEATURE_NO_NEON=1` 只规避交叉 sysroot 下 `blake3` NEON 头文件不兼容，原生 Windows 构建不关闭 NEON。
- `aarch64-linux-android`：使用已安装 NDK，先运行 `python3 scripts/android-toolchain.py check`；`bash scripts/android-check.sh` 是 Android host 侧完整门。

这些命令只证明目标代码可编译，不证明安装包、APK、ELF 或设备行为；八问第 1 问按此记录证据等级。

## 普通测试隔离

workspace nextest 使用 mock、fixture 与 loopback；普通测试不得访问外网、启动真实 Mihomo、改宿主系统代理或启动宿主桌面。需要真实 Mihomo/sing-box 的 backend smoke 是有摘要锁定、显式运行的独立证据，不混进普通单元测试。UI parity 的 L1/L2/L3 行为证据由 [UI parity contract](crates/infiltrator-contract/docs/UI_PARITY_AUDIT.md) 和 `scripts/parity/` 管理，不由截图单独判定。

## 阶段门与通过标准

| 阶段门 | 自动化证据 | 平台/产物证据 | 关闭条件 |
| --- | --- | --- | --- |
| Core 契约切片 | focused nextest；迁移生产消费者后的 workspace nextest；格式、边界守卫；typed `Unsupported` 零副作用/读回回归 | Mihomo 锁定版本真实进程和 digest；后端切换需真实进程 conformance | 新契约覆盖全部生产消费者，同切片删除旧 API/DTO/wire/fallback/旧文案；引擎能力按真实版本登记。sing-box 未验收前明确 unsupported |
| Android owner / IPC | Rust owner request/reply/session/generation/readback 回归；三 flavor JVM suites；两 ABI 编译；链接边界/产品身份 guard | 最终 APK 的 `.so` / ELF 与进程归属；owner death/rebind、旧 reply、重启/adoption | 证明只有 `:vpn` 创建 Rust Core/config/VPN/引擎（Mihomo/sing-box） owner；Compose/Bevy/Slint 命令由同一 owner 受理并回读同一 snapshot；没有设备证据的行为保持 OPEN |
| Android VPN 生命周期 | typed state/transaction 回归；失败注入涵盖 builder、TUN、worker、FD、IPC deadline、取消、lease、撤权、队列饱和；无裸 `bool` 生命周期 API | 三 flavor 最新 APK；merged manifest instrumentation；系统授权拒绝/取消/成功；实际 VPN 流量及 stop/revoke | 配置冻结并绑定 generation/session；只有真实 TUN data-plane ready receipt 和 owner readback 后发布 Running；停止/撤权清理失败不伪报终态且可重试；进程与代理是独立状态轴 |
| Android 输入与 UI | Compose ViewModel/导航/滚动回归；Bevy TouchInput/Picking/gesture/ScrollPosition 回归；layout/token/resource guard | Compose、Bevy、Slint 各自最终 APK；同一设备上的点击、长列表、菜单、返回、Insets/IME；ARM64 真机视频和日志 | 单指点击/拖动/滚动不互相误触；50 项列表首尾可达；导航和弹窗有效；两种宽度/密度不溢出。缺设备时不标交互完成 |
| 产品候选 | 全量质量结构门、workspace nextest、三 flavor JVM/AndroidTest、APK identity/manifest | 安装/升级候选包，记录版本号、包名、显示名、图标、ABI、签名、APK SHA-256 和设备结果 | 版本只由登记的唯一源派生；所有 app 产品名/图标/launcher/artifact/签名规则一致；候选 APK 的证据绑定同一源码与产物 |

单元测试、编译、APK 组装、模拟器、真机是不同证据层。任何一层都不能替代表中其它层；截图证明视觉状态，不证明触控或业务操作成功。测试检查真实结果、错误恢复、取消副作用和身份边界；不以断言数量、实现字符串或测试发现本身证明行为。

## Android 验收场景

Compose 和 Bevy 用相同业务场景编号分别验收，但保留各自输入实现。Compose 核对 Material/Compose hit target、语义节点、滚动容器、导航、IME、系统返回和 Insets；只有 MotionEvent 入口证据异常时才下探原始 Android 输入。Bevy 依次记录 Android MotionEvent/winit、Bevy `TouchInput`、坐标/scale、Picking 命中与遮挡、Tap/Drag 仲裁、目标节点和 `ScrollPosition` 前后值；不能用 MouseWheel 代替触摸。Slint 使用 NativeActivity，须另行核验窗口生命周期、原生触摸和 shared-owner 请求/回读。

Compose 与 Bevy 都需覆盖：连续点击三个操作、按住与拖动、至少 50 项列表上下各 10 次滑动、点击行与滑动不误触、首尾边界、菜单开关、页面切换、系统返回、前后台恢复、窗口宽度/字体大小变化。固定栏与系统手势安全区应分别验证。应用控制另外覆盖清数据冷启动、Core-only start、StartProxy 授权拒绝/取消/允许、VPN 数据面 ready、StopProxy 保留 Core、PauseKernel 停止 Core、系统撤权、Service/owner 死亡和重绑。Slint 使用完整导入页面和同一个产品 owner；设备验收应按 [Slint 表面契约](crates/infiltrator-slint/docs/SURFACE.md) 覆盖桌面与 Android 响应式布局、实际可操作控件和 owner 回读。对共享 owner 明确不支持的导入入口，只核验其 typed unsupported 提示，不计为已实现业务操作。

## 证据记录

| 层 | 证据 | 不得外推到 |
| --- | --- | --- |
| L0–L2 | typed contract、单元/集成、mock、headless 行为、静态 guard | Android 系统资源、真实内核、设备输入 |
| L3 | 锁定 core、资产摘要、API/schema 及真实后端操作 | 其它版本、其它平台或 Android VPN |
| L4 | Android APK、合并 manifest、模拟器/目标设备行为 | 未测试的 flavor、ABI、设备、系统版本 |
| L5 | ARM64 真机授权、VPN 流量、IME、后台/网络/功耗 | 其它设备型号和环境 |

每条设备记录至少包括源码 commit/dirty 状态、产品 flavor、版本、application ID、APK SHA-256、ABI、Android/API、设备型号、屏幕 dp 与物理尺寸、density/fontScale、测试步骤、预期/实际结果和原始 logcat/操作视频路径。只在设备可用时标记已验证；未执行的步骤写为 `OPEN` 或 `INCOMPLETE`，不得使用“推断通过”。

阶段内按边界写代码和 focused tests；跨 crate/跨进程切片合拢后再运行一次统一验证。Compose、Bevy 与 Slint 的 JVM、APK 和 AndroidTest 在同一 Gradle invocation 生成，避免共享 Rust/JNI 增量产物发生并发冲突。格式、`git diff --check`、静态门和 nextest 仍按各自入口单独报告。Android emulator/ARM64 设备验收与高成本 APK 构建集中在阶段门，不为每个组件重复打包。
