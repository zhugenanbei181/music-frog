# 平台与交付矩阵

平台支持分成四件事：源码能编译、核心二进制能交付、功能行为通过、目标设备/桌面真实验证。它们不能互相替代。

## 1. 当前矩阵

| 平台/形态 | UI/宿主 | mihomo 交付 | 当前依据 | 需要补的证据 |
| --- | --- | --- | --- | --- |
| Linux desktop | Iced 与 Bevy 同权产品 | 锁定 amd64/arm64 资产；AppImage、Deb、tarball 包含原生内核，按内容哈希物化运行副本 | 共享桌面宿主、内核查找与复制行为测试；发布矩阵目前打包 amd64 | 最终源码的安装包 smoke、桌面启动/托盘/TUN 真机验证 |
| Windows desktop | Iced 与 Bevy 同权产品 | 锁定基线 amd64 与 arm64 资产；两端 NSIS/ZIP 均包含匹配内核、许可与来源清单 | 本地实际 NSIS 编译；原生路径转换已接入发布 workflow，目标 Windows CI 待运行 | x64/ARM64 最终包、升级回滚、系统代理和 tray 真机验证 |
| macOS desktop | Iced 与 Bevy 同权产品 | 锁定 amd64/arm64 资产；两端 App bundle 的 Resources 包含匹配内核 | 两套 App/DMG/tarball 模板与共享内核查找代码 | 目标 macOS CI、签名/权限、安装后真实运行验证 |
| Android arm64-v8a | Compose + UniFFI + Kotlin host/VPN | `vendor/mihomo-android-arm64-v8`，构建时复制为 `libmihomo.so` | `scripts/android-build.sh`、Gradle ABI 配置 | 真实设备 VPN、后台、升级和异常退出矩阵 |
| Android x86_64 | Compose + UniFFI + Kotlin host/VPN | `vendor/mihomo-android-amd64`，用于 emulator/ABI | Gradle 与 fetch 脚本 | emulator/CI ABI smoke、性能与网络隔离验证 |
| Android Bevy | NativeActivity 界面入口；原生产品宿主尚未组合 | 已有 APK 入口；当前受限 ECS 的 Bevy UI arm64 库使用真实 NDK 交叉编译与严格 Clippy 通过，尚不能证明完整控制产品交付 | 默认入口显式显示未组合状态；桌面结果不外推 | 按 [Android 产品规范](BEVY_ANDROID_PRODUCT.md) 补 UI/VPN 进程与 IPC、原生命令/全页 reader、文本/Insets/剪贴板、列表/节能；最终默认特性包和真机 VPN/恢复/功耗验收前不得作为完整产品发布 |
| iOS arm64 | `infiltrator-ios` host seam；Native UI/NetworkExtension 未接入 | 由签名 app/extension bundle 交付（策略已定，资产未接入） | `infiltrator-ios` 的 `IosBridge`、保守 capability 测试 | Swift/Objective-C bridge、NetworkExtension entitlement、真机 VPN 与后台验证 |
| Admin API | 桌面管理与诊断 HTTP API；旧浏览器 UI 已退役 | 不拥有独立 core；由产品宿主提供服务 | `infiltrator-admin`；退役事实见 `TAURI_WEBUI_RETIREMENT_LEDGER.md` | API contract 与调用方断线/重连兼容 |
| External mihomo dashboard | 已随 WebUI 于 0.20 退役 | 管理能力由原生 Iced / Bevy 同权产品承担 | `TAURI_WEBUI_RETIREMENT_LEDGER.md` | — |

## 2. 平台边界

- Rust core/use-case 不直接依赖 Compose、Vue 或 Iced 类型。
- 平台 adapter 只处理进程、目录、凭据、VPN、系统代理、托盘、权限和 native lifecycle。
- platform unavailable、permission denied、missing binary、unsupported 和 controller error 必须是不同结果。
- Android 的 `MihomoHost.kt` 可以拥有 Process/VPN 的系统实现，但 profile/config 的 canonical 写入必须回到 Rust。
- Android Bevy 的保留扩展、目标服务进程所有权和完整验收范围由 [BEVY_ANDROID_PRODUCT.md](BEVY_ANDROID_PRODUCT.md) 规定；上表描述当前状态，不将目标架构登记为现有实现。
- Windows/Linux/macOS 的 core 资产与发布方式必须分别列出，不能把 Windows exe 当作“桌面支持”的证明。

## 3. 每个平台的最小验证层级

1. `cargo check`/Gradle compile：证明依赖和条件编译成立；
2. mock API/纯行为测试：证明 use-case 和错误语义成立；
3. package smoke：证明资源、权限、ABI、安装树和启动参数成立；
4. real core smoke：证明指定 mihomo 版本的 readiness、API、配置和退出行为成立；
5. real device/desktop evidence：证明目标环境的 VPN、托盘、系统代理、窗口和后台生命周期。

缺少后两层时，状态只能写“代码路径存在/待验证”，不能写“平台完成”。
