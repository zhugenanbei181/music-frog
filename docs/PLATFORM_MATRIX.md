# 平台与交付状态

> 层级：L2

平台结论分别记录编译、安装包、行为和目标设备证据；其中一项通过不代表其它项完成。

**Apple target 暂缓**：自动 CI 与 release 当前不构建 macOS/iOS 产物；Windows、Linux、Android 是现行自动发行目标。恢复 Apple 构建前更新本表并取得目标产物证据；`apple-ci-deferral-guard.py` 防止 workflow 意外加回。

| 平台 | 产品与交付 | 当前状态 / 下一证据 |
| --- | --- | --- |
| Linux desktop | Iced、Bevy、Slint；AppImage、Deb、tarball 携带锁定 Mihomo 资产；发行目标 x86_64 与 aarch64（arm64 走原生 ARM64 runner） | Slint 完整导入 Lumen 页面并接入共享 DesktopProductSession，运行时与构建 crate 固定到最新稳定版 `1.18.1`；Mihomo/sing-box 真实进程 owner switch smoke 已通过。本机 aarch64 交叉 `cargo check` 已可复跑（zig 0.14.1 + cargo-zigbuild）。目标构建、安装包、托盘、TUN 与完整桌面产品验收仍待验证；详细 Core 能力边界见 [Core 后端契约](CORE_BACKENDS.md) |
| Windows desktop | Iced、Bevy、Slint；NSIS/ZIP 携带对应内核与来源信息 | Slint 完整导入页面并接入共享桌面 owner，x86_64/aarch64 发行目标已登记；目标构建、NSIS 安装、升级回滚及系统集成待验证。本地现有 NSIS 证据仅对应此前产品版本 |
| macOS desktop | Iced、Bevy；App bundle 模板保留 | 当前无 CI/release 产物；恢复后验证签名、权限和安装运行 |
| Android | Compose、Bevy 与 Slint flavor；arm64-v8a、x86_64 | Slint 完整 Lumen 页面已接入现有 Android owner，NativeActivity flavor 和双 ABI Rust 构建入口已进入源码；APK、启动、触摸、权限和运行时行为均待验证。Core owner 源码现按 revisioned active-profile reference dispatch Mihomo 或 sing-box：Android sing-box 已接通 managed ABI/version 的 Core lifecycle 与 typed version-management owner operations；app UI 版本安装/激活入口和 running runtime operations 仍未接入。Core/Android 受影响包 nextest run `cc3c0959-7e30-467e-a1a5-15eca68fca48` 为 2673 passed、6 skipped；隔离 `:core-host-tests:test --rerun-tasks` 为 52/52。这些 Rust/JVM 结果不证明 Android target、APK 或真机 child 行为。既有 host 与 Compose/Bevy 证据如下，均不代替 Slint 验收：先前完整 workspace nextest `272d97ba-6521-4a54-9ebb-ccfd66eb37f8` 对应更早源码，不作本轮整体验收。本轮 host 证据：最近一次完整受影响包 nextest 2344/2344（run `f09bcff7-5913-48d1-a547-9cd9957fff62`），早于当前连接速率状态 enum 迁移；当前源码 `cargo check` 通过。后续 nextest run `016f2f7c-6319-41b9-b466-cbfc71626696` 开始 2971 项，最后可见 2517 项通过后被中断，未获最终摘要、不计为通过；当前 Clippy 与结构门待复跑。Android driver 12/12（run `6d119b50-92da-46c0-8965-f8c264583074`）及 driver Clippy 属于此前源码；以上均不是 APK/设备证据。当前锁定 Android target Cargo tree 与 `android-bevy-driver-dependency-guard.py` 均确认 driver 不含 `infiltrator-application`、`infiltrator-android`、`infiltrator-composition`、`mihomo-api` 或 `infiltrator-platform`；此静态闭包结果不证明最终 `.so`/APK ELF。aarch64 本机交叉 `cargo check` 已通过（NDK 29.0.14206865；`infiltrator-android` 的 `aarch64-linux-android` target），最终 ELF 符号仍待 APK 构建核验（BANDROID-041.4.2）。现有 x86_64 API 35 模拟器 APK 绑定旧源码 SHA，只证明 Compose 冷启动、Core-only 启动与 API readback、Profiles 切页、通知权限拒绝及 VPN consent Cancel；最新双 flavor Gradle/JVM/AndroidTest 与 APK 尚未重建。VPN lifecycle 五类静态复核缺口由 BANDROID-042 逐项登记，修复与设备证据完成前不可宣称已修复。最新 APK 的点击、触摸与 50 项滚动未验证，Geo Globe 0/0 巨型占位未整改。ARM64 真机、真实 VPN 流量、Binder death/rebind、后台行为与 Compose 长列表/IME 证据仍 OPEN |
| iOS | Rust host seam | 没有 Apple target 构建、应用包或 NetworkExtension 交付 |

<!-- platform-evidence:start -->
## 构建-安装-运行证据矩阵（生成）

本节由 `scripts/platform/build-evidence-matrix.py` 从证据登记册 `scripts/platform/evidence/*.json` 生成，登记册是唯一真源；`scripts/quality/platform-evidence-guard.py` 校验本节与登记册逐字一致。

每一格给出状态、证据身份（产物 SHA-256、run id 或命令）与闭合该腿的命令；`Unsupported` 格给出原因与阻塞方。交叉编译检查（`cross_compile`）不能单独证明任何一腿，因此带交叉编译的腿仍是 `Unsupported`。

| 平台 | 构建 | 安装 | 运行 | 目标机 / 设备 | 平台结论 |
| --- | --- | --- | --- | --- | --- |
| Linux desktop | **Unsupported** · 证据：平台：command:bash scripts/cross-check.sh (aarch64-unknown-linux-gnu) · 原因：当前源码的最终产物尚未构建：打包器与身份校验已就位，但本源码没有对应产物 · 命令：`bash packaging/linux/build-linux.sh --product iced --arch x86_64` | **Unsupported** · 证据：无 · 原因：安装冒烟尚未针对当前源码的产物执行 · 命令：`bash packaging/linux/smoke-install.sh --product iced --deb dist/infiltrator_$(python3 scripts/product_identity.py --get version.name)_amd64.deb --appimage dist/Infiltrator-Linux-x86_64.AppImage` | **Unsupported** · 证据：无 · 原因：当前源码的已安装产物尚未启动过 · 命令：`bash packaging/linux/smoke-install.sh --product iced --deb dist/infiltrator_$(python3 scripts/product_identity.py --get version.name)_amd64.deb --appimage dist/Infiltrator-Linux-x86_64.AppImage` | **Unsupported** · 证据：无 · 原因：桌面冒烟跑在私有虚拟合成器里；真实桌面会话（托盘、TUN 接管、会话集成）仍无证据 · 命令：`bash scripts/desktop-smoke.sh all` | Unsupported |
| Windows desktop | **Unsupported** · 证据：平台：command:bash scripts/cross-check.sh (aarch64-pc-windows-msvc) · 原因：NSIS 安装包只能在 Windows 目标机上构建；该证据属于环境阻塞板的 Windows 腿（APPID-001.4） · 命令：`bash packaging/windows/nsis/build-nsis.sh iced $(python3 scripts/product_identity.py --get version.name) target/x86_64-pc-windows-msvc/release/infiltrator-iced.exe dist/Infiltrator-Setup-x86_64.exe` | **Unsupported** · 证据：无 · 原因：安装包的安装与卸载冒烟必须在 Windows 目标机上执行（GAP-2.1） · 命令：`dist/Infiltrator-Setup-x86_64.exe /S` | **Unsupported** · 证据：无 · 原因：Windows 运行证据缺失：托盘、TUN 接管与系统集成从未在 Windows 上被观察 · 命令：`bash scripts/blocked/foreign-target-evidence.sh --dry-run` | **Unsupported** · 证据：无 · 原因：本工作站没有接入 Windows 目标机 · 命令：`bash scripts/blocked/foreign-target-evidence.sh --dry-run` | Unsupported |
| macOS desktop | **Unsupported** · 证据：无 · 原因：Apple CI 暂缓仍然生效：没有 macOS runner、也没有安排 Apple target，当前源码没有 bundle 与 DMG（GAP-2.2） · 命令：`bash packaging/macos/create-dmg.sh dist/Infiltrator-macOS.dmg "dist/MusicFrog Infiltrator.app"` | **Unsupported** · 证据：无 · 原因：没有任何 macOS bundle 在 macOS 机器上被安装过（GAP-2.2） · 命令：`bash scripts/blocked/foreign-target-evidence.sh --dry-run` | **Unsupported** · 证据：无 · 原因：macOS 运行证据缺失：bundle 从未启动，NetworkExtension 从未被观察 · 命令：`bash scripts/blocked/foreign-target-evidence.sh --dry-run` | **Unsupported** · 证据：无 · 原因：本工作站没有接入 macOS 目标机，且 Apple CI 暂缓决议仍然生效 · 命令：`python3 scripts/quality/apple-ci-deferral-guard.py` | Unsupported |
| Android | **Unsupported** · 证据：平台：command:bash scripts/android-check.sh (aarch64-linux-android, x86_64-linux-android) · 原因：当前源码没有 release APK：三个 flavor 的 Gradle 构建未对本源码执行，Rust crate 只有交叉编译 · 命令：`cd android && ./gradlew :app:assembleComposeRelease :app:assembleBevyRelease :app:assembleSlintRelease` | **Unsupported** · 证据：无 · 原因：当前源码的 APK 没有在真机或模拟器上安装过（GAP-3.1） · 命令：`adb install -r dist/app-compose-universal-release.apk` | **Unsupported** · 证据：无 · 原因：三个 flavor 的交互截图与行为回执缺失（PARITY-001.4） · 命令：`bash scripts/android-emulator-tests.sh` | **Unsupported** · 证据：无 · 原因：本工作站没有接入 ARM64 真机（BANDROID-041.3.2R.15） · 命令：`bash scripts/blocked/arm64-device-evidence.sh --dry-run` | Unsupported |
| iOS | **Unsupported** · 证据：无 · 原因：仓库只有 iOS 的 Rust host seam：没有 iOS target 构建、没有应用包、没有 NetworkExtension 包（GAP-3.2） · 命令：`cargo build --locked --release --target aarch64-apple-ios` | **Unsupported** · 证据：无 · 原因：没有 iOS 应用包，因此没有任何可安装对象 · 命令：`bash scripts/blocked/foreign-target-evidence.sh --dry-run` | **Unsupported** · 证据：无 · 原因：iOS 运行证据缺失：host seam 从未在设备上承载过 · 命令：`bash scripts/blocked/foreign-target-evidence.sh --dry-run` | **Unsupported** · 证据：无 · 原因：Apple target 解禁决议与 macOS 工具链都缺失 · 命令：`python3 scripts/quality/apple-ci-deferral-guard.py` | Unsupported |

<!-- platform-evidence:end -->

较早的 Core/Android/sing-box 源码 checkpoint 与 nextest run ID 已移入本地历史归档（不入库）；现行状态以上表为准。

## 专项规则

- 逐业务视觉与截图验收（桌面三 UI 的 standard/compact 双视口像素回执、Android 三 flavor 实际交互截图）与「五大平台 × 三大 UI × 59 条业务」的覆盖矩阵，其当前状态与收口条目登记在[现在佛](../todos/CODE_TODO.md) §二/§三；实测 parity 注册表只含 iced/bevy 两表面，Slint 尚无锚点与像素回执，Android 三 flavor 尚无交互截图回执。**未取得对应像素或交互回执前，不得据本表宣称任何表面已通过视觉验收。**
- Slint 第三 UI 的实测缺口（2026-10-19 全量审计）：页面词汇只有 Home/Proxy/Config/Settings/Network/Connections/Logs/Rules/Traffic/AppSettings/About，**缺 dns、editor、filter、mixin、sync、app_routing、doctor 七类页面**，对应 19 条业务在 Slint 无载体；只派发 42 种 `CommandIntent`；59 条业务里 2 条完整实现、18 条部分实现、39 条未实现，**今天没有任何一条可以合法标记为 Slint scenario 锚点**；4 个测试文件在用例里重实现 Slint 逻辑，构成第二业务真源；两个已声明回调无调用点。parity 注册表已补齐第三表面（177 单元格全部可解析，Slint 57 格 pending + 2 格 implemented，契约目录新增 `slint_page` 载体判定）。补齐由[现在佛](../todos/CODE_TODO.md) `SLINT-001.1`～`001.9` 与 `PARITY-001.2`～`001.4` 承担；在其办结前，Slint 不得被计入任何平台的「已验收」范围。
- Android 产品与真机验收见 [Android 文档](../android/docs/README.md)。
- Core 后端和 Mihomo 资产规则见 [CORE_BACKENDS.md](CORE_BACKENDS.md) 与 [UPSTREAM.md](UPSTREAM.md)。
