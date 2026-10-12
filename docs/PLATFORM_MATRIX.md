# 平台与交付状态

> 层级：L2

平台结论分别记录编译、安装包、行为和目标设备证据；其中一项通过不代表其它项完成。

**Apple target 暂缓**：自动 CI 与 release 当前不构建 macOS/iOS 产物；Windows、Linux、Android 是现行自动发行目标。恢复 Apple 构建前更新本表并取得目标产物证据；`apple-ci-deferral-guard.py` 防止 workflow 意外加回。

| 平台 | 产品与交付 | 当前状态 / 下一证据 |
| --- | --- | --- |
| Linux desktop | Iced、Bevy、Slint；AppImage、Deb、tarball 携带锁定 Mihomo 资产 | Slint 完整导入 Lumen 页面并接入共享 DesktopProductSession，运行时与构建 crate 固定到最新稳定版 `1.18.1`；目标构建、安装包、托盘及 TUN 行为待验证。锁定的双 Mihomo 版本已完成 direct 与 real-process smoke 验收，能力边界与 OPEN 剩项见 [Core 后端契约](CORE_BACKENDS.md) |
| Windows desktop | Iced、Bevy、Slint；NSIS/ZIP 携带对应内核与来源信息 | Slint 完整导入页面并接入共享桌面 owner，x86_64/aarch64 发行目标已登记；目标构建、NSIS 安装、升级回滚及系统集成待验证。本地现有 NSIS 证据仅对应此前产品版本 |
| macOS desktop | Iced、Bevy；App bundle 模板保留 | 当前无 CI/release 产物；恢复后验证签名、权限和安装运行 |
| Android | Compose、Bevy 与 Slint flavor；arm64-v8a、x86_64 | Slint 完整 Lumen 页面已接入现有 Android owner，NativeActivity flavor 和双 ABI Rust 构建入口已进入源码；APK、启动、触摸、权限和运行时行为均待验证。既有 host 与 Compose/Bevy 证据如下，均不代替 Slint 验收：先前完整 workspace nextest `272d97ba-6521-4a54-9ebb-ccfd66eb37f8` 对应更早源码，不作本轮整体验收。本轮 host 证据：最近一次完整受影响包 nextest 2344/2344（run `f09bcff7-5913-48d1-a547-9cd9957fff62`），早于当前连接速率状态 enum 迁移；当前源码 `cargo check` 通过。后续 nextest run `016f2f7c-6319-41b9-b466-cbfc71626696` 开始 2971 项，最后可见 2517 项通过后被中断，未获最终摘要、不计为通过；当前 Clippy 与结构门待复跑。Android driver 12/12（run `6d119b50-92da-46c0-8965-f8c264583074`）及 driver Clippy 属于此前源码；以上均不是 APK/设备证据。Android target Cargo tree 已不含 `infiltrator-android`、`infiltrator-composition`、`mihomo-api`、`mihomo-platform`，但 UI 仍带入其它 runtime-neutral `infiltrator-application` helpers；shell/traffic readout 已迁至纯 `infiltrator-presentation`。aarch64 本机编译检查因缺 NDK 未完成，最终 ELF 符号未核验（BANDROID-041）。现有 x86_64 API 35 模拟器 APK 绑定旧源码 SHA，只证明 Compose 冷启动、Core-only 启动与 API readback、Profiles 切页、通知权限拒绝及 VPN consent Cancel；最新双 flavor Gradle/JVM/AndroidTest 与 APK 尚未重建。VPN lifecycle 五类静态复核缺口由 BANDROID-042 逐项登记，修复与设备证据完成前不可宣称已修复。最新 APK 的点击、触摸与 50 项滚动未验证，Geo Globe 0/0 巨型占位未整改。ARM64 真机、真实 VPN 流量、Binder death/rebind、后台行为与 Compose 长列表/IME 证据仍 OPEN |
| iOS | Rust host seam | 没有 Apple target 构建、应用包或 NetworkExtension 交付 |

2026-10-12 Core owner 源码 checkpoint：Desktop 产品组合与 Android `:vpn` owner 已改为解析同一 typed、revisioned active-profile reference，校验目标 `ProfileDocument` 与 Start capability，再把 owner guard 绑定到 `CoreApplication`。共享 typed owner-switch selector transaction 已有 fake/nextest 覆盖；Desktop/Android 产品根尚未接线，仍只选择 Mihomo，sing-box 继续 fail closed。旧 Desktop Host retry 和 Iced admin/rebuild bootstrap 因不能转交同一 owner session 而 typed `Unsupported`。受影响 Core/Android/desktop-host 包 `cargo nextest` 为 1430 passed、3 skipped、0 failed（run `a28d1e15-a904-4524-918b-a84105ff2b2e`）；此 Rust 源码结果不是 APK、Gradle 或设备验收。revision-aware activation rollback 与 Android close 生命周期证据仍未完成，见[Core 后端契约](CORE_BACKENDS.md)。

2026-10-12 Android Core child identity 与 owner restart checkpoint：Rust typed process observation 与 Android Kotlin 持久 reservation/owned-child record 已接线；`:vpn` startup 已按 process observation → VPN terminal readback → schema-v2 proof recovery → child-record consumption 顺序恢复 owner。Rust owner phase 只在 recovery 完成后开放 SharedCore ingress。`:core-host-tests:test` 直接编译隔离的生产 host 源集，33/33；受影响 Rust nextest 1063/1063（run `90ca9322-927c-4a6d-bf53-f75f36ade83f`）。该证据覆盖 fake process、owner context、PID start ticks、typed receipt、准备/激活 gate、owner IPC ledger 与 fail-closed 顺序；不证明完整 app Kotlin compile、APK、真机 `/proc` 权限、service death/rebind 或真实 child 行为，设备证据仍 OPEN。

2026-10-12 Core 启动 session proof 与 restart recovery checkpoint：`CoreProcess::prepare_start` 用显式 admission enum 管理宿主进程位；`CoreApplication` 在 spawn 前把精确 Managed identity/generation/session 写入 owner proof schema v2，v1 bound proof 显式迁移为不可自动恢复的 `LegacyUnverified`。新 `:vpn` session 已接入 typed recovery application，只有精确退出 child + VPN terminal readback 才恢复 owner 并允许 provider binding；受影响包 nextest 1063/1063（run `90ca9322-927c-4a6d-bf53-f75f36ade83f`），隔离 host JVM 33/33。未做完整 app Kotlin compile、UI/APK 或设备验收；Android service death/rebind、真机 `/proc` 与完整设备行为仍 OPEN。

2026-10-12 engine-scoped ProfileStore checkpoint：`mihomo-config::ConfigManager` 现在可注入 engine-owned document codec，并按 `EngineId` 隔离 ProfileStore/ProfileWorkspace、metadata、credential key 与原始文档扩展；默认 Mihomo 构造保留显式 legacy migration，sing-box 初始化只创建自己的 JSON namespace、不导入 Mihomo 旧文件。两 engine 共用产品级 revisioned active reference 与 owner fence。测试覆盖同名隔离、JSON 与 opaque sidecar 精确读回、stale/foreign 零写入，以及 sing-box active workspace lease 的 engine identity；受影响 Core/Android/desktop-host 包 nextest 825/825、4 skipped（run `36cdeb27-d4ad-4d35-95ac-0ae9cd64a943`），存储包最终复跑 186/186、1 skipped（run `fa92e0a6-af64-496e-a382-2924b2a840b9`）。这只是 store/adapter 源码和 fake/nextest 证据，尚未证明 Desktop 或 Android 组合根注入该 store，也未证明真实进程配置事务；两产品仍只选择 Mihomo。

## 专项规则

- Android 产品与真机验收见 [Android 文档](../android/docs/README.md)。
- Core 后端和 Mihomo 资产规则见 [CORE_BACKENDS.md](CORE_BACKENDS.md) 与 [UPSTREAM.md](UPSTREAM.md)。
