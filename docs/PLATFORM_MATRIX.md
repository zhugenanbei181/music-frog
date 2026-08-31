# 平台与交付矩阵

平台支持分成四件事：源码能编译、核心二进制能交付、功能行为通过、目标设备/桌面真实验证。它们不能互相替代。

## 1. 当前矩阵

| 平台/形态 | UI/宿主 | mihomo 交付 | 当前依据 | 需要补的证据 |
| --- | --- | --- | --- | --- |
| Linux desktop | Iced（唯一桌面端，0.20 起） | 运行时外部 core/本地安装路径，脚本未覆盖 Linux 资产 | `mihomo-platform` Linux keyring、Iced Wayland/X11 features | core 交付策略、桌面启动/托盘/TUN 真机 smoke |
| Windows desktop | Iced（唯一桌面端，0.20 起） | `vendor/mihomo.exe`，打包前由脚本获取 Windows amd64 | MSI workflow、Windows process/no-window 适配 | x64/ARM64 包、升级回滚、系统代理和 tray 真机验证 |
| macOS desktop | Rust platform path 已有 keyring 条件分支 | 当前 fetch 脚本未声明 macOS 资产 | `mihomo-platform` macOS dependency | core 资产/签名/打包/权限与真实运行验证 |
| Android arm64-v8a | Compose + UniFFI + Kotlin host/VPN | `vendor/mihomo-android-arm64-v8`，构建时复制为 `libmihomo.so` | `scripts/android-build.sh`、Gradle ABI 配置 | 真实设备 VPN、后台、升级和异常退出矩阵 |
| Android x86_64 | Compose + UniFFI + Kotlin host/VPN | `vendor/mihomo-android-amd64`，用于 emulator/ABI | Gradle 与 fetch 脚本 | emulator/CI ABI smoke、性能与网络隔离验证 |
| Admin Web | 浏览器内 Vue 管理面 | 不拥有 core；由桌面 Admin server 提供 | `infiltrator-admin` + `config-manager-ui` | API contract、浏览器断线/重连和旧客户端兼容 |
| External mihomo dashboard | 已随 WebUI 于 0.20 退役 | 管理能力由 Iced 本体承担 | `TAURI_WEBUI_RETIREMENT_LEDGER.md` | — |

## 2. 平台边界

- Rust core/use-case 不直接依赖 Compose、Vue 或 Iced 类型。
- 平台 adapter 只处理进程、目录、凭据、VPN、系统代理、托盘、权限和 native lifecycle。
- platform unavailable、permission denied、missing binary、unsupported 和 controller error 必须是不同结果。
- Android 的 `MihomoHost.kt` 可以拥有 Process/VPN 的系统实现，但 profile/config 的 canonical 写入必须回到 Rust。
- Windows/Linux/macOS 的 core 资产与发布方式必须分别列出，不能把 Windows exe 当作“桌面支持”的证明。

## 3. 每个平台的最小验证层级

1. `cargo check`/Gradle compile：证明依赖和条件编译成立；
2. mock API/纯行为测试：证明 use-case 和错误语义成立；
3. package smoke：证明资源、权限、ABI、安装树和启动参数成立；
4. real core smoke：证明指定 mihomo 版本的 readiness、API、配置和退出行为成立；
5. real device/desktop evidence：证明目标环境的 VPN、托盘、系统代理、窗口和后台生命周期。

缺少后两层时，状态只能写“代码路径存在/待验证”，不能写“平台完成”。
