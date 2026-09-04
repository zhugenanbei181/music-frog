# 平台交互动词契约（Platform Contracts）

> 0.30 迁移文档。目标：**每个 OS 交互动词一个统一 Rust 契约（trait），各平台各给实现；
> 不支持的组合显式返回 `Unsupported`，而不是 `cfg` 静默消失。**
>
> 与 [`PLATFORM_MATRIX.md`](PLATFORM_MATRIX.md) 的关系：那边回答"每个平台要交什么证据"；
> 本文回答"每个动词的 Rust 契约长什么样、现状差在哪、怎么迁"。
> 与 [`TAURI_WEBUI_RETIREMENT_LEDGER.md`](TAURI_WEBUI_RETIREMENT_LEDGER.md) 的关系：§1.2 记录了
> 旧 Tauri 宿主集成能力，本文把其中仍缺的动词（提权重启、is_elevated 三端化）纳入契约。

## 0. 契约设计原则（以 `SecureStore` 为样板）

`infiltrator-ports/src/secure_store.rs` 的 `SecureStore` 是本仓库的凭据端口，
平台实现位于 `mihomo-platform`，后续所有外部能力向它的依赖方向对齐：

1. **中性 trait**：`async fn get/set/delete`，入参全是 `&str`/值类型，不泄漏任何平台类型；
2. **平台实现隔离**：desktop 走 `keyring` crate（`desktop.rs:146-209`，`spawn_blocking` 包住阻塞
   API），Android 走 UniFFI bridge（`android.rs:38-62`）——调用方一个 `cfg` 都看不到；
3. **类型别名选默认**：`DefaultCredentialStore`（`defaults.rs`）按 target 选实现，调用方
   只依赖 trait；
4. **错误走 `mihomo_api::Result`**：失败是数据，不是 panic，也不是静默 no-op。

已知小缺陷（迁移时顺手修）：`KeyringCredentialStore::get`（`desktop.rs:163-176`）把 keyring 错误
吞成 `Ok(None)`，丢掉了"keyring 锁定"与"无条目"的区分。

---

## 1. 动词 × 平台 能力矩阵（现状 → 目标）

图例：`✅ 真实实现` / `🟡 有实现但有缺陷` / `stub`（编译过、log-only）/ `❌ 缺失` / `n/a 不适用`。

| # | 动词 | Linux 现状 | Windows 现状 | macOS 现状 | 契约形态现状 | 目标 |
| --- | --- | --- | --- | --- | --- | --- |
| 1 | 系统托盘 | ✅ ksni/SNI（子菜单/勾选/tooltip/图标激活全支持） | ✅ muda/tray-icon（同样全支持） | ✅ muda/tray-icon（左键开菜单惯例，`IconActivated` 语义存疑） | ✅ trait 已统一（`TrayController`） | 保持 trait；补能力矩阵常量 + macOS 点击语义决策 + 事件 handler 注销 |
| 2 | OS 通知 | ✅ notify-rust z（zbus） | stub（log-only，`notify.rs:92-102`） | stub（同左） | ❌ 自由函数 + cfg，非 trait | trait `SystemNotifier`；Win=WinRT Toast，mac=UNUserNotification |
| 3 | 系统代理读/写 | 🟡 gsettings（仅 GNOME；无 DE 探测） | ✅ 注册表 WinINET + rundll32 刷新 | 🟡 networksetup（服务名推断简陋、写不查退出码） | ❌ 单文件内 cfg 分支 | trait `SystemProxyManager`，Linux 增加后端探测/显式 Unsupported |
| 4 | 开机自启 | 🟡 XDG `.desktop`（第二份实现，iced 未用） | 🟡 reg.exe 子进程（iced 用）+ winreg（闲置实现） | 🟡 launchd plist 只写文件不 launchctl | ❌ **两套互不相干实现**，iced 只用 Windows-only 那套 | 合并为一个 trait，三端真实 |
| 5 | TUN 提权服务 | 🟡 pkexec setcap（start/stop 空、uninstall 针对 GUI exe） | ✅ sc.exe 服务 | stub（install 显式 bail；start/stop 竟用 `sudo`） | ❌ 静态方法 + 函数体内 cfg | trait `TunPrivilegeManager`；macOS 近期显式 Unsupported |
| 6 | 凭证存储 | ✅ keyring | ✅ keyring | ✅ keyring | ✅ trait（样板） | 达标；补错误透传 |
| 7a | 单实例锁 | ✅ flock（`instance_lock.rs`）+ `single-instance` crate 并存 | ✅ 命名互斥体（同两套并存） | ✅ flock（同两套并存） | 🟡 两套机制并存，iced 用的是 crate 那套 | 收敛为 `instance_lock` 一套 + IPC |
| 7b | 第二实例 IPC | ✅ Unix socket（`instance_ipc.rs`） | ✅ TCP+port_file | ✅ Unix socket | ✅ 中性 API | 达标 |
| 7c | 崩溃日志钩子 | 🟡 手写 panic hook 写 `<home>/infiltrator_crash.log` | 同左 | 同左 | ❌ 钩子内联在 `lib.rs`；`crash_reporter.rs` 的结构化+脱敏版**未接线** | trait `CrashReporter`，接上现成脱敏实现 |
| 7d | 打开配置目录 | ✅ xdg-open | ✅ explorer | ✅ open | 🟡 单函数 cfg 分支（`update/ui.rs:288-309`），语义达标但不成文 | 小 trait 或保持函数 + 显式 Unsupported 分支入契约文档 |
| 7e | 提权重启 | ❌（静默改道 TUN 安装，`update/ui.rs:277-280`） | 🟡 PowerShell RunAs，**丢失端口参数**、无 is_elevated 前置 | ❌ 无 | ❌ 消息处理里 cfg，动词混杂 | trait `PrivilegeManager`（is_elevated/restart_elevated）；恢复 ledger §1.2 的参数保留行为 |
| 7f | 端口探测 | ✅ TcpListener::bind 127.0.0.1 | ✅ 同左 | ✅ 同左 | ✅ 平台无关自由函数（`mihomo-config/src/port.rs`） | 达标；仅记 TOCTOU 注意事项 |

**最不一致的三个动词：4 开机自启（两套实现且 iced 用的是三端最残的）、2 OS 通知（三端只有一端真）、
7e 提权重启（ verb 改道 + 参数丢失）。**

---

## 2. 各动词目标契约签名 + 实现模块建议

模块路径建议：契约与实现一律收进 `mihomo-platform`（托盘除外，理由见 §2.1）：

```
crates/mihomo-platform/src/
  defaults.rs             # 当前平台的 Default* 类型别名
  platform/mod.rs        # 平台能力实现的组合入口
  platform/linux.rs
  platform/windows.rs
  platform/macos.rs
  platform/unsupported.rs # 显式 Unsupported 实现集合
```

统一错误约定：`PlatformError::{Unsupported { verb, reason }, PermissionDenied, Io(..), Backend(..)}`，
**`Unsupported` 是一等返回值，不是注释**。

### 2.1 系统托盘（已是契约，只差收口）

现状：`crates/infiltrator-iced/src/tray/spec.rs:437-445` `TrayController` trait、`:449-457`
`TrayStartup{Ready,Unavailable}`（typed 降级，好样板）；ksni 后端 `ksni_backend.rs:205`、
muda 后端 `native.rs:195`；spawn 分发 `tray.rs:47-53`（cfg 门控：ksni =
`unix&&!macos&&!native-tray-backend`，muda = `windows|macos|feature`）；事件统一
`std::sync::mpsc`（`tray.rs:57-67`），iced 订阅 20ms 轮转（`tray.rs:83-97`）。

能力差实测：**ksni 与 muda 实际都支持子菜单 / CheckMenuItem / tooltip / 图标 RGBA / 分隔线**
（`ksni_backend.rs:77-163`、`native.rs:50-136`），不是当初担心的能力鸿沟。真正的不一致：

- muda 的 `MenuEvent::set_event_handler` 是**全局单例**（`native.rs:208-218`），shutdown 只隐藏
  图标不注销 handler（`native.rs:185-189`）；ksni 靠 drop 收敛。二次 spawn 会互相覆盖。
- 勾选即时回显：ksni 有 `checked_overrides`（`ksni_backend.rs:33,127-144`）处理"shell 自己翻
  checkbox"的竞态；muda 侧没有等价物，点击后视觉状态要等下一次 `update_spec` 才对齐。
- macOS 惯例是左键弹菜单，`TrayEvent::IconActivated`（spec.rs:245）在 macOS 上可能极少触发；
  `TrayIntent::ShowWindow` 的主入口需要决策（如接受 macOS 差异，或提供菜单首项"显示主窗口"）。

```rust
// 保持 spec.rs 现有签名不动（已达标），只追加能力自描述：
pub trait TrayController {
    fn update_spec(&self, spec: TraySpec);
    fn shutdown(&mut self);
    /// 新增：后端能力 + 生命周期钩子说明，供 UI 层做 macOS 等差异决策。
    fn capabilities(&self) -> TrayCapabilities; // icon_activated_opens_menu: bool, ...
}
```

实现路径：ksni/muda 留在 `infiltrator-iced/src/tray/`（依赖 iced 无关、纯中性，但与 UI 启动
时序强耦合，跨 crate 移动收益低）。收口动作：spawn 前 `MenuEvent::clear_event_handler`/
记录 handler 归属；muda 后端补 click 时勾选覆盖；在 `TrayCapabilities` 里显式声明 macOS 点击
语义。

### 2.2 OS 通知（P0，缺口最大）

现状：`crates/infiltrator-iced/src/notify.rs` —— Linux notify-rust z 后端（`:62-90`），其余平台
log-only stub（`:92-102`）；`send()` 是自由函数返回 `bool`，`AppState::system_notify`
（`:109-139`）做 spawn_blocking + 2s 超时 + 限频告警。依赖在 `Cargo.toml` 按
`cfg(all(unix, not(macos)))` 门控。另有**第二套未使用实现**
`crates/infiltrator-desktop/src/notify.rs`（子进程 notify-send/osascript/PowerShell MessageBox，
仅 `infiltrator-desktop/src/lib.rs:9` 挂载，无调用方）——MessageBox 是模态阻塞的，语义就是错的。

```rust
// infiltrator-ports/src/secure_store.rs
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NotifyUrgency { Low, Normal, Critical }

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NotifyOutcome {
    Delivered,
    /// 守护进程/系统拒绝（含超时）。info 带原因，供限频日志。
    Rejected(String),
    /// 平台/环境不支持（无通知守护进程、未授权等）——显式，不静默。
    Unsupported(String),
}

#[async_trait]
pub trait SystemNotifier: Send + Sync {
    /// fire-and-forget 语义；不得阻塞调用线程超过超时上限。
    async fn notify(&self, title: &str, body: &str, urgency: NotifyUrgency) -> NotifyOutcome;
    /// 供 UI 提前隐藏通知开关（如无守护进程的 headless Linux）。
    fn availability(&self) -> NotifyAvailability { NotifyAvailability::Unknown }
}
```

选型建议（只写方案，本次不引依赖）：
- **Windows**：WinRT `ToastNotificationManager`/`ToastNotifier`（`windows` crate 的
  `UI_Notifications` feature，或现成 `winrt-notification` 风格薄封装）。需注册 AUMID
  （开始菜单快捷方式或运行时 `SetCurrentProcessExplicitAppUserModelID`）；无 AppUserModelID
  时 toast 不显示——`availability()` 返回 `Unsupported("missing AUMID")`。
- **macOS**：`UNUserNotificationCenter`（`objc2-user-notifications` 或 `mac-notification-sys`
  风格）。需 app bundle + 用户授权；未 bundle 的裸二进制降级 `Rejected`/`Unsupported` 并如实上报。
- **Linux**：保留 notify-rust z 后端（与 ksni 共享 zbus 5），无守护进程时报 `Rejected`。
- 迁移时把 `warn_throttled`/2s 超时留在**调用侧**（`AppState::system_notify`），trait 实现保持
  纯投递；退役 `infiltrator-desktop/src/notify.rs`。

### 2.3 系统代理（P0）

现状：`crates/infiltrator-desktop/src/proxy.rs` 全部为函数体内 cfg 分支：`apply_system_proxy:18`、
`apply_system_proxy_with_bypass:22-43`、`read_system_proxy_state:45-62`。
- Windows：winreg 直写 HKCU Internet Settings（`:72-103`）+ `rundll32 user32.dll,UpdatePerUserSystemParameters` 刷新（`:135-149`）——达标。
- Linux：`gsettings` 子进程，org.gnome.system.proxy 三协议 + ignore-hosts（`:151-230`）；gsettings 不存在时报错，但**不区分 GNOME/KDE/其他 DE**，KDE 用户拿到的是失败而非 Unsupported。
- macOS：`networksetup`（`:232-322`）；`get_active_network_service:233-245` 取"第一个服务否则 Wi-Fi"过于天真；`set_macos_system_proxy:247-278` 对每条 networksetup 调用只查 spawn 成功不查退出码；`read` 只看 `-getwebproxy` 不看 secure/socks。
- 未知 target：apply 端 `endpoint.is_some()` 才报 Unsupported（`:35-42`），read 端直接返回
  `Ok(default)`（`:58-61`）——**读端在撒谎**，违反 Unsupported 原则。

```rust
// infiltrator-ports/src/system_proxy.rs
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SystemProxyState { pub enabled: bool, pub endpoint: Option<String>, pub bypass: Option<String> }

#[async_trait]
pub trait SystemProxyManager: Send + Sync {
    async fn read_state(&self) -> Result<SystemProxyState, PlatformError>;
    /// None = 关闭系统代理；bypass 语义各端统一为 `;` 分隔（Linux 端内转数组）。
    async fn apply(&self, endpoint: Option<&str>, bypass: Option<&str>) -> Result<(), PlatformError>;
    /// 新增：本端能否安全操作（gsettings 缺失 = Unsupported 原因，供设置页解释）。
    fn availability(&self) -> Result<(), PlatformError> { Ok(()) }
}
```

实现路径：`platform/windows.rs`（现 winreg 逻辑平移）、`platform/linux.rs`（gsettings 保留 +
`availability()` 探测 gsettings/DE，非 GNOME 显式 `Unsupported`；后续可加 KDE KConfig 后端）、
`platform/macos.rs`（networksetup 平移 + 逐条检查退出状态 + 枚举全部活动服务）。调用方
`infiltrator-iced/src/update/ui.rs:211-268`（含错误回滚 `:257-268`）改为持 trait 对象，行为不变。

### 2.4 开机自启（P0/P1 边界，因双实现混乱提到 P0 一起清）

现状：
- **iced 实际使用**：`crates/infiltrator-shared/src/autostart.rs` —— 仅 Windows（`reg.exe` 子进程，
  `HKCU\...\Run`，名字 `MusicFrogInfiltrator`，常量在 `infiltrator-iced/src/lib.rs:27`）；非
  Windows `is_autostart_enabled` 恒 `false`（`:36-40`，**静默假**）、`set` 报错（`:77-81`）。
  调用方：`app.rs:59`、`admin_server.rs:603-607`、`update/core/lifecycle.rs:165`、
  `update/core/kernels.rs:250`。
- **闲置第二套**：`crates/infiltrator-desktop/src/autostart.rs` `AutostartManager` —— winreg 直写
  （`:28-75`）、XDG `.desktop`（`:77-124`）、launchd plist（`:126-182`，只写文件不调 launchctl，
  依赖 RunAtLoad 下次登录生效；enable/disable 的"立即生效"语义未定义）。带 tempfile 单测。

```rust
// infiltrator-ports/src/autostart.rs
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AutostartState { Enabled, Disabled, Unsupported(&'static str) }

#[async_trait]
pub trait AutostartManager: Send + Sync {
    async fn state(&self) -> Result<AutostartState, PlatformError>;
    async fn set_enabled(&self, enabled: bool) -> Result<(), PlatformError>;
}
```

实现路径：`platform/windows.rs`（用 winreg 直写替换 reg.exe 子进程；名字沿用
`MusicFrogInfiltrator`，与旧 Tauri `MihomoDespicableInfiltrator` 共存契约不变）、
`platform/linux.rs`（XDG，base_dir 可注入以保留 tempfile 测试能力）、`platform/macos.rs`
（launchd plist + `launchctl bootstrap/bootout gui/$UID` 可选增强，至少文档化延迟生效语义）。
**删除 `infiltrator-shared/src/autostart.rs`，四个调用点改持 trait。**

### 2.5 TUN 提权服务（P1）

现状：`crates/infiltrator-desktop/src/tun_service.rs` `TunServiceManager` 静态方法 + 函数体内
cfg。`ServiceModeStatus`（`:8-14）已含 `Unsupported` 枚举（好）。Windows sc.exe 全套
（`:152-230`）达标；Linux pkexec setcap 安装（`:253-267`，无 shell 注入风险，注释明确）但
`start/stop` 是空 `Ok(())`（`:285-292`）、`uninstall` 对 `current_exe()`（GUI 自身！）去
setcap -r（`:270-271`，应针对 mihomo 二进制）；macOS `check` 查 PrivilegedHelperTools/LaunchDaemons
（`:299-317`）、`install` 显式 bail（`:320-322`）、`start/stop` 用 `sudo launchctl`
（`:329-348`）——GUI 会话里 sudo 无 tty 必败，应属 Unsupported 而非假可用。iced 侧接线在
`update/core/runtime_config.rs:80-100,234-240`、`view/settings.rs:206,388`。

```rust
// infiltrator-ports/src/tun.rs
pub use /* 现 ServiceModeStatus 平移 */ ServiceModeStatus;

#[async_trait]
pub trait TunPrivilegeManager: Send + Sync {
    async fn status(&self, core_binary: &Path) -> Result<ServiceModeStatus, PlatformError>;
    async fn install(&self, core_binary: &Path) -> Result<(), PlatformError>;
    async fn uninstall(&self, core_binary: &Path) -> Result<(), PlatformError>;
    async fn start(&self) -> Result<(), PlatformError>;   // setcap 模型下为 no-op+Unsupported 语义文档化
    async fn stop(&self) -> Result<(), PlatformError>;
}
```

实现路径：Windows（sc.exe 平移）、Linux（setcap 模型平移 + uninstall 改针对 core_binary +
start/stop 返回 `Unsupported("capability model has no daemon")`）、macOS（近期统一
`Unsupported("requires SMJobBless helper")`，删除 `sudo launchctl` 假路径）。

### 2.6 其余动词（P2）

**单实例锁**：`mihomo-platform/src/instance_lock.rs:59-76` 是好契约（flock/命名互斥体、
`Ok(Some/None)/Err` 三态、未知 target fail-closed `:69-76`、注释解释了为何不用 pid 文件）。
但 `infiltrator-iced/src/lib.rs:58-69` 用的是 `single-instance` crate（`com.musicfrog.infiltrator`），
与仓库自己的 `instance_lock` **双轨并存**。目标：iced 切到 `instance_lock::try_acquire_instance_lock`
+ `instance_ipc.rs`（Unix socket / TCP+port_file，`IpcCommand::{FocusWindow,OpenUrl,Ping}`
`:16-20`），失败时走 IPC 通知首实例聚焦——即 ledger §1.2 "第二实例仅重开界面"的等价物。

**崩溃日志钩子**：现状 `infiltrator-iced/src/lib.rs:53-77`：panic hook 手写 `<home>/infiltrator_crash.log`
+ eprintln；而 `mihomo-platform/src/crash_reporter.rs` 已有结构化 `CrashReport`（时间戳/os/版本/
backtrace 摘要）+ 脱敏（Bearer token、unix/win home 路径）——**未接线**。目标 trait：

```rust
pub trait CrashReporter: Send + Sync {
    fn install(&'static self);                     // 内部 panic::set_hook
    fn last_report(&self) -> Option<CrashReport>;  // 供"上次崩溃"UI/上传
}
```
Linux/Windows/macOS 实现共用结构化写盘路径（差异仅日志目录，经 `paths::get_home_dir`），Android
后续可接 tombstone。迁移时保留 `startup_critical.log` 行为（`lib.rs:60-65`）。

**打开配置目录**：`infiltrator-iced/src/update/ui.rs:288-309` `open_directory` —— explorer/open/
xdg-open，未知 target 显式 `ErrorKind::Unsupported`。**语义已达标**，收编动作仅为：移入
`mihomo-platform/src/platform/` 并包成 `fn reveal_dir(path) -> Result<(), PlatformError>`；
顺带统一 `infiltrator-desktop/src/editor.rs:114-141` 的编辑器/xdg-open 探测，避免两处
open-verb 分叉。

**提权重启**：现状 `update/ui.rs:262-280`：Windows 用 PowerShell `Start-Process -Verb RunAs`
重启（**不带任何参数**，丢失 ledger §1.2 旧 Tauri 保留的 `--static-port`/`--admin-port`），
非 Windows 静默改道 `InstallTunService`（`:277-280`，动词混淆）；`is_elevated` crate 已在
依赖里且用于 `app.rs:270-276` 的 `is_admin`（仅 Windows，非 Windows 恒 false）。目标：

```rust
#[async_trait]
pub trait PrivilegeManager: Send + Sync {
    fn is_elevated(&self) -> bool;                                  // Win=crate; macOS/Linux=geteuid()==0
    async fn restart_elevated(&self, args: &[String]) -> Result<(), PlatformError>;
}
```
Windows：ShellExecuteW("runas")（或保留 PowerShell 方案）+ 参数透传；macOS：`osascript
"do shell script ... with administrator privileges"` 或显式 Unsupported；Linux：不支持整程序提权
重启（TUN 走 setcap 模型），显式 `Unsupported`——**取消静默改道**。

**端口探测**：`mihomo-config/src/port.rs:4-11` `is_port_available`/`find_available_port`，
平台无关，达标。备忘：bind-then-release 有 TOCTOU 窗口（boot 重试逻辑 `boot.rs:343` 已兜底），
契约文档里记一句即可，不需要抽象。

---

## 3. 实现优先级

- **P0（三端真实行为）**
  1. OS 通知 trait（§2.2）：唯一"三端只有一端真"的用户可感动词；同时退役两处死代码
     （`infiltrator-desktop/src/notify.rs`）。
  2. 系统代理 trait（§2.3）：修 macOS 退出码/服务枚举、Linux DE 探测、read 端 Unsupported 撒谎。
  3. 托盘收口（§2.1）：不是重写，是 `TrayCapabilities` + handler 注销 + muda 勾选即时回显 +
     macOS 点击决策。
  4. 开机自启合并（§2.4）：两套实现并存的清理成本随时间上涨，且 Linux/macOS 用户现在点开关
     得到的是静默假状态。
- **P1**：TUN trait（§2.5，修 uninstall 目标错位与 macOS sudo 假路径）。
- **P2**：单实例锁双轨收敛、CrashReporter 接线、open-verb 收编、PrivilegeManager（提权重启）、
  SecureStore 错误透传小修。

排序理由：P0 全部是"用户点开关但结果错误/静默无效"的正确性问题；P1 是 macOS 不可用但已有显式
降级；P2 是健壮性/卫生。

## 4. 验证方式（对齐 TEST_MATRIX L0-L5）

| 动词 | 可进 headless/desktop-smoke（本机 Linux 即可跑） | 需真实 OS 证据 |
| --- | --- | --- |
| 托盘 | spec 构建/事件解析（已有 `tests/gui/tray_tests.rs`）；ksni 菜单映射（已有）；muda id codec（`native.rs:226-265` 已有）；muda 后端可在 Linux 用 `native-tray-backend` feature 编译+冒烟 | SNI 真机托盘点击、Windows 托盘、macOS 左键语义 |
| 通知 | `NotifyOutcome` 状态机、限频、超时护栏（纯内存）；stub 平台 `send=false`（已有 `notify.rs:164-168`） | Linux 有/无守护进程两态；Windows toast 实显（AUMID）；macOS 授权弹窗 |
| 系统代理 | `parse_endpoint`/bypass 编解码（已有 `proxy.rs:324-347`）；gsettings 序列化纯函数化后单测；**新增 desktop-smoke**：`read_state` 只读探测输出 `{:?}` 不写 | WinINET 生效（浏览器实测）；GNOME 设置面板回读；macOS 网络偏好回读 |
| 自启 | XDG/plist 内容断言（已有 `infiltrator-desktop/src/autostart.rs:185-238` tempfile 模式，迁到新实现）；winreg 路径需真机 | 注册表 `reg query` 回读；登录实测拉起；launchd RunAtLoad 实测 |
| TUN | `ServiceModeStatus` Display/状态机（已有 `tun_service.rs:352-393`）；命令参数构造函数化后单测（sc.exe/setcap 的 argv 快照） | Windows UAC 流；Linux pkexec 弹窗+getcap 回读；macOS 显式 Unsupported 提示 |
| 凭证/锁/IPC/端口 | 已有单测覆盖（`instance_lock.rs:181-249` 等）；desktop-smoke 可加 keyring roundtrip 可选项 | keyring 真机桌面钥匙环（gnome-keyring/Credential Manager/Keychain） |
| 崩溃/提权/open | `CrashReport` 脱敏单测（`crash_reporter.rs` 已有基础）；argv 构造单测 | 真实 panic 落盘；UAC/管理员授权流程；文件管理器拉起 |

建议新增 `scripts/quality/desktop_smoke.rs`（或 `bash scripts/test.sh` 套件）：
只读探测 + 纯函数断言，输出逐动词 PASS/UNSUPPORTED(reason) 清单，作为 Linux CI 之外的
Windows/macOS 打包机一键取证入口（对应 TEST_MATRIX L3）。

## 5. 迁移步骤（不破坏现有 Linux 行为）

1. **先立契约不接线**：`PortError` + 新 trait 全部进 `infiltrator-ports` 的按能力模块；
   `platform/{linux,windows,macos}.rs` 先由现有函数体平移填充，`platform/unsupported.rs` 提供显式
   Unsupported 兜底实现；平台默认实现通过 `mihomo-platform/src/defaults.rs` 暴露。
2. **逐动词双跑**：新 trait 实现先用"包装旧函数"的方式落地（`proxy.rs` 的函数体直接被
   `platform/linux.rs` 调用），iced 调用点逐个切换到 trait 对象；每动词单独 PR，Linux 行为
   diff 为零（`tests/gui` 全绿 + 手测托盘/代理/自启开关）。
3. **清死代码与双轨**（随对应动词 PR 一起）：删 `infiltrator-desktop/src/notify.rs`、
   `infiltrator-shared/src/autostart.rs`；单实例双轨与 CrashReporter 接线放 P2 窗口。
4. **补 Windows/macOS 编译证据**：CI 现仅 ubuntu（`.github/workflows/test.yml`），每动词 PR
   附 `cargo check --target x86_64-pc-windows-gnu`（仓库已有该 target 目录）结果；macOS 暂以
   `cargo check --target aarch64-app-darwin` 本地取证，进 PLATFORM_MATRIX 的"需要补的证据"列。
5. **每动词收尾动作**：在本文 §1 矩阵把"现状列"改写为"目标达成列"并注明 PR；带 `Unsupported`
   的组合必须在设置页有可见文案（`availability()` 驱动），不允许只在日志里出现。
