# Android 产品与宿主规则

> 层级：L3

本文定义 Android 的进程、系统资源、输入和设备验收契约；它不是实现状态报告。Bevy ECS 规则见 [Bevy UI 章程](../../crates/infiltrator-bevy-ui/docs/BEVY_UI_FRONTEND.md)。

## Owner 与进程

```text
Compose / Bevy UI process
        │ typed command, result, snapshot
        ▼
:vpn — Rust application/composition + Android host/VpnService
        │ Mihomo controller adapter
        ▼
Mihomo child process
```

- `:vpn` 是唯一 Rust Core、VPN 与 Mihomo 生命周期 owner；Profiles、DNS、Fake-IP、TUN、Rules/providers、app routing 与 WebDAV 持久化数据均由它读写。Compose、Bevy 和 Slint 通过 typed owner IPC 传递意图与不可变结果；UI 不直接访问 Mihomo REST/WebSocket，也不复制业务状态机。
- Rust → Kotlin host 操作统一使用 Serde-tagged `AndroidBridgeCall`、request ID 与 typed completion；业务调用经 Application 所有的有界 dispatcher 异步回调，Rust 不同步等待 Binder。快速进程元数据仍走短 JNI 调用。dispatcher 将同一个 Android elapsed-realtime deadline 传给持久 Binder client；timeout/cancel 以原 request ID 撤销并丢弃迟到回执，不以缓存值伪造成功。该源码与隔离 JVM 回归不代替 APK 或设备 Binder death/rebind 验收。
- 宿主角色使用 `UiClient`、`CoreOwner`、`Unsupported` typed enum；未知角色 fail closed。Host Messenger 当前协议为 `AndroidHostProtocolVersion::V10`，owner operation 数据协议为 V6；旧版本 client fail closed，不保留 wire fallback。`ShellObservation` 在线路上以显式 enum 编码，生命周期 snapshot 的 128 位 session token 用十进制字符串避免 JSON 精度丢失。Core-scoped 产品控制使用不含 StopProxy 的 `AndroidProductControlIntent`，绑定 Core identity/generation/session；用户停止走独立 typed `STOP_VPN_SERVICE` owner 请求，由 `:vpn` coordinator 原子选择当前 VPN generation 并返回 stop terminal 与 readback。生命周期回读使用 owner-authored snapshot；`AndroidCoreProcessState` 只在 owner 进程通过 JNI 读取。`:vpn` startup 先完成 exact child observation、VPN terminal readback 与 schema-v2 Managed proof recovery，Rust `SharedCore` ingress 在 `Recovering` 阶段 fail closed。上述契约不替代 Binder death/rebind 的设备验收。
- Bevy driver 使用独立 lockfile。Android target 产品闭包必须排除 `infiltrator-application`、`infiltrator-android`、`infiltrator-composition`、`mihomo-api` 与 `infiltrator-platform`；UI 以 `default-features = false` 接入，只消费 owner 注入的 `SurfaceSnapshot` 和 typed command sink。Application 仅由显式 capture tooling 或 script-engine feature 引入，不能进入 Android 产品 driver。Desktop host 仍单独持有其 Application composition。
- Core snapshot 带 owner-authored generation、session token 与 revision。同一代次和 session 内，revision 只能递增；相同 revision 只接受完全相同的 lifecycle 快照。启动 Intent、Binder 可用或前台通知都不代表代理已生效。
- 服务可在没有 Activity 时初始化。Activity 退出不停止已授权的 VPN；明确停止、权限撤销和服务失败分别返回实际终态。
- Binder、JNI 引用、监听器、线程和文件描述符由创建它们的 owner 成对释放；UI 生命周期清理不能关闭服务持有的资源。

## Owner IPC 契约

### Profiles 与订阅

Profiles 与订阅操作通过 `AndroidOwnerOperation` 的 Serde enum 走 `:vpn` Messenger owner，返回 typed 结果与 Profiles snapshot；请求绑定 client/owner session、Core generation 与 owner data revision。V6 `ProfileSelect` 携带 `AndroidProfileSelectionTarget::{Mihomo, SingBox}` 与校验后的 profile ID，成功回执返回实际 active engine/profile/revision。live owner profile replacement 统一走 `ActiveProfileBackendSelector::replace_owner_for_profile`；sing-box 同引擎 selection 也经该事务重建 `:vpn` owner，Mihomo 同引擎选择走共享 ProfileApplication。未知协议版本、variant 或附加字段一律拒绝，UI 进程缺 bridge 时 fail closed。实现见 [owner_operation.rs](../../crates/infiltrator-android/src/uniffi_api/owner_operation.rs)、[profiles.rs](../../crates/infiltrator-android/src/uniffi_api/profiles.rs) 和 [MihomoControlService.kt](../app/src/main/java/com/musicfrog/infiltrator/MihomoControlService.kt)。

### Age credential owner operation

Age 凭据使用独立的 `ExecuteProfileAgeCredentialOwnerOperation` V10 Messenger request。scope、Profile selection 和 credential revision 放入严格解码的 metadata JSON；私钥以单次 `ByteArray` 经 UI bridge、Binder 与 JNI 直接送入 `:vpn`，绝不放入 JSON、日志或 snapshot。`:vpn` 侧验证 Core scope、session/generation、owner data revision 与配置事务 fence，再构造零化的 `AgePrivateKey` 并调用共享 ProfileApplication；回执只含非敏感状态与 revision。拒绝、排队取消、service 关闭及执行终态都会清除所持有的 key buffer。此源码与 JVM/nextest 回归不能代替 ARM64 Binder 设备验收。实现见 [profile_age_credential_owner.rs](../../crates/infiltrator-android/src/uniffi_api/profile_age_credential_owner.rs)、[MihomoControlClient.kt](../app/src/main/java/com/musicfrog/infiltrator/MihomoControlClient.kt)、[MihomoControlService.kt](../app/src/main/java/com/musicfrog/infiltrator/MihomoControlService.kt) 和 [AndroidBridgeCallDispatcher.kt](../app/src/main/java/com/musicfrog/infiltrator/AndroidBridgeCallDispatcher.kt)。

### DNS 与 Fake-IP

`AndroidOwnerOperation::{DnsSettings, DnsSettingsSave, FakeIpSettings, FakeIpSettingsSave, FakeIpCacheClear}` 通过 V6 typed owner wire 进出 `:vpn`。读取和配置写入返回 DNS/Fake-IP snapshot；cache clear 返回 typed mutation receipt，并在确有文件删除时推进 owner data revision。对应的 per-action UniFFI exports 与 UDL 方法已退役。`TunSettings` / `TunSettingsSave` 也由唯一 `:vpn` owner 读取/写入，并返回同 revision 的 TUN settings snapshot。TUN DTO 只包含 MTU、stack 和路由选项；IPv6 与 DNS server 留在 DNS 配置域，避免两路分别写入同一事实。TUN 配置写失败仍推进 owner data revision，因为配置应用可能已发生副作用但 readback 失败；UI 会显示 owner 实际读回并保留失败状态。未知版本、variant、DTO 字段以及过期 revision 在操作执行前拒绝。实现见 [settings.rs](../../crates/infiltrator-android/src/uniffi_api/settings.rs)、[owner_operation.rs](../../crates/infiltrator-android/src/uniffi_api/owner_operation.rs) 与 [vpn.rs](../../crates/infiltrator-android/src/uniffi_api/vpn.rs)。


### Rules 与 rule providers

Compose RulesViewModel 只调用一个 typed UniFFI client：`androidExecuteOwnerOperation(AndroidOwnerOperation)`。`RulesList`、`RulesSave`、`RuleProviders` 和 `RuleProvidersSave` 是 Rust operation enum 的穷尽变体；`:vpn` Messenger service adapter 将同一 enum 的 Serde 编码解码后调用 shared ConfigurationApplication，返回 typed `AndroidOwnerOperationReply` 与 owner-authored snapshot。Rules 专属 per-operation UniFFI exports 和旧 response DTO 已退役。Provider JSON 编辑器仍接受文档文本，但在 UI 侧解码为递归 `AndroidJsonValue` / provider entry records 后再调用 typed client；任意 JSON 字符串不会作为 owner operation contract。读取和写入返回分开的 Rules / RuleProviders snapshot 域，避免同 revision 的不同数据互相覆盖；写入绑定 owner session、Core generation 和 owner data revision，成功结果必须与 owner readback 一致。未知 operation/DTO 字段、未知 JSON variant、重复对象键和无效数值在配置写入前拒绝；无效规则保持 profile 内容和 revision 不变。实现见 [rules.rs](../../crates/infiltrator-android/src/uniffi_api/rules.rs)、[owner_operation.rs](../../crates/infiltrator-android/src/uniffi_api/owner_operation.rs)、[RulesViewModel.kt](../app/src/main/java/com/musicfrog/infiltrator/ui/settings/rules/RulesViewModel.kt) 和 [RuleProviderJson.kt](../app/src/main/java/com/musicfrog/infiltrator/ui/settings/rules/RuleProviderJson.kt)。

### WebDAV Sync

WebDAV settings read/save、connection test 与 sync-now 走 typed `AndroidOwnerOperation` V6 Messenger contract；`:vpn` owner 返回 revision-fenced WebDAV settings snapshot，设置写入即使失败也推进 owner data revision。`SyncRepository` 在接受结果前校验 owner session、Core generation、domain、revision 与 readback；UI 不再调用四个 WebDAV 专用 UniFFI 方法。实现见 [owner_operation.rs](../../crates/infiltrator-android/src/uniffi_api/owner_operation.rs)、[webdav.rs](../../crates/infiltrator-android/src/uniffi_api/webdav.rs)、[SyncRepository.kt](../app/src/main/java/com/musicfrog/infiltrator/ui/sync/SyncRepository.kt) 和 [SyncViewModel.kt](../app/src/main/java/com/musicfrog/infiltrator/ui/sync/SyncViewModel.kt)。

### Per-app routing

Compose 路由页使用 `AndroidOwnerOperation::{AppRoutingSnapshot, AppRoutingModeSet, AppRoutingPackagesSet}`，与 Profiles、DNS、Rules、TUN 共用 V6 session/generation/data revision fence 和 owner-authored readback。批量选择使用 `AndroidAppRoutingPackageSelection::{Selected, Unselected}`；跨端 `CommandIntent`/`SurfaceSnapshot` 的 routing mode/rule/selection 也是共享 Rust enum。**契约要求** VpnService 用 `android_vpn_routing_snapshot` 返回的独立 routing revision 取得 `AndroidVpnRoutingActivationLease` 才能调用 `Builder.establish()`；租约覆盖数据面 ready 与 Kotlin `Running` 发布，释放失败保留 lease 并进入 cleanup retry，不能回滚已就绪数据面或伪报 stopped。真实 TUN/FD 与设备多应用流量证据仍 OPEN。实现见 [app_routing.rs](../../crates/infiltrator-android/src/uniffi_api/app_routing.rs)、[owner_operation.rs](../../crates/infiltrator-android/src/uniffi_api/owner_operation.rs)、[AppRoutingRepository.kt](../app/src/main/java/com/musicfrog/infiltrator/ui/settings/routing/AppRoutingRepository.kt)、[MihomoVpnService.kt](../app/src/main/java/com/musicfrog/infiltrator/MihomoVpnService.kt) 和 [VpnServiceConfiguration.kt](../app/src/main/java/com/musicfrog/infiltrator/VpnServiceConfiguration.kt)。

### 仍需 owner IPC 的持久化入口

以下生产入口仍可能在 UI 进程直接构造 application/store 并读写共享数据；不能因 Core、Profiles 或 DNS/Fake-IP 已唯一化而视为完成：

- VPN session readback：`vpn_session_status` 的 Kotlin observation 已读取 owner service 状态枚举，但 Rust revision/附加元数据仍保存在调用进程，尚不是 owner-authored revisioned `VpnOwnerSnapshot`。TUN settings 已通过 `AndroidOwnerOperation::{TunSettings, TunSettingsSave}` 迁到唯一 owner。入口见 [vpn.rs](../../crates/infiltrator-android/src/uniffi_api/vpn.rs) 和 [owner_operation.rs](../../crates/infiltrator-android/src/uniffi_api/owner_operation.rs)。

Compose 的日志清空与连接关闭统一经 CoreScope 绑定的 `AndroidProductCommand` 发送到 `:vpn` owner，并校验同一 Core activation 的终态；不再为这些动作暴露逐项 UniFFI 函数。`ip_check` 是独立公网探测入口。

## VPN 与 Core 生命周期

- 只有明确的用户意图启动代理。Compose 与 Bevy 共用 `AndroidVpnAuthorizationFlow`：当前可见 Activity 检查 `VpnService.prepare()`、发起系统授权、处理通知权限，再提交 typed `START_PROXY`；`:vpn` owner 回读必须确认 VPN 状态为 `RUNNING` 且前台保护有效，UI 才接受启动成功。`READ_VPN_AUTHORIZATION` 是纯读取；只有实际启动请求遇到未授权时，owner 才通知可见 Activity 发起授权。
- Android VPN consent 返回只区分已授权与未授权；系统将拒绝和关闭授权界面都报告为同一未授权结果，产品不伪称能进一步区分。此结果与授权查询失败、前台服务启动失败、TUN 建立失败、内核未就绪和用户停止保持不同的 typed 状态。
- Android 通知权限回调同样只提供已授予或未授予；拒绝或关闭提示统一记为 `NotGranted`。通知权限提示无法启动时，Compose/Bevy 共用 flow 会把原因记为 warning 并继续提交 VPN 启动；提示结果和提示启动失败都不阻断 VPN 前台服务。
- Compose 使用 AndroidX Activity Result launcher；Bevy 因必须继承 `NativeActivity` 以接入 winit，仅保留 Activity 层的系统结果适配回调，授权排序、状态和启动/回读逻辑仍由共享 flow 实现。
- VPN service 的显式 Start/Stop 请求通过 typed enum extra 和 lifecycle generation 表达；无命令启动只有在 Android 确认 always-on 或旧版系统的 unflagged service delivery 下，才恢复唯一 `:vpn` owner 并复用 Rust `StartProxy`，其他情况 typed fail closed。系统托管的通知不提供本地 Stop；普通 ControlService sticky 重建只恢复 owner host，不启动 Core 或代理。
- `VpnService` 持有系统授权、前台通知、路由/DNS/MTU 与 TUN fd；Rust application 管理配置事务和 Core 结果。路由读取与 package Builder 计划使用 typed `Ready/Failed`，失败会阻止 TUN 建立并进入清理；`Builder.establish()` 前获取的 CoreOwner routing activation lease 会阻止并发路由写入，并覆盖 TUN fd 交接和 Rust worker 启动。fd 的复制、转移与关闭责任唯一，失败、停止、撤销和重启都回收实际持有的描述符。
- Android routing plan 在所有 package mode 下都排除自身 app UID，因此 Mihomo/tun2proxy upstream sockets 不会重新进入自己的 TUN。当前没有把这些 sockets 显式绑定到某个 `Network`，由 Android 管理默认 underlay handover；[VpnService API](https://developer.android.com/reference/android/net/VpnService) 规定只有显式绑定 upstream socket 时才需调用 `setUnderlyingNetworks`。未来若引入该绑定，再增加 network callback seam 并覆盖 network loss/rebind。
- `MihomoVpnService` 只在已 admission 的串行 Start/Cleanup transaction 持有 `PARTIAL_WAKE_LOCK`，最多 45 秒；每个任务的 `finally` 提前释放，Android 定时过期是进程/任务异常退出的兜底。空闲 VPN 不持锁，不能把 WakeLock 存活当成 Core/VPN 成功状态；`WAKE_LOCK` 由主 Manifest 声明并进入 flavor contract guard。
- `prepare_vpn()` 与 `start_vpn(fd)` 必须消费同一份不可变 `VpnConfiguration`。Prepared config 一次性绑定 VPN generation、Core identity/generation/session 与同一 owner serializer 下读取的 `OWNER_DATA_REVISION`；配置、TUN 或 DNS 在 prepare 后变化时，Builder 前 admission 必须 typed 拒绝旧 revision 且不建立 TUN。admission lease 会阻止 owner 写入并以 typed Busy 回执拒绝，持续到 Rust data-plane readiness receipt 后由 Service 的 finally 路径释放；Start consume 仍复核 request、generation 和 revision。配置修改、Core 重启、Stop、Revoke 或重复消费必须让旧准备事务 fail closed。`VpnStartReceipt::Started` 只能在 TUN worker 已越过真实初始化/ready 边界后产生，thread spawn、FD handoff 或固定延迟不能充当 readiness。
- Owner-first Stop 是唯一正式停止路径：`:vpn` coordinator 停止对应 worker 并关闭唯一 FD，Rust 随后验证 owner terminal readback；owner IPC 不可用时 best-effort 本地清理不得伪造 Stopped/Running。系统撤权只有在 worker/FD 清理完成后才发布 Revoked；清理失败须记录可重试的 typed failure，不能无条件发布终态。
- JNI VPN Start 与 Builder 配置结果使用共享 Rust `VpnStartOutcome`、`VpnConfigurationApplyOutcome` enum；Stop 使用 Serde-tagged `VpnOwnerStopReceipt` sum enum，`Stopped` 必须携带 owner snapshot，失败终态携带 typed failure，并按变体明确是否有 readback。服务状态统一读取 typed `AndroidVpnServiceState`，设置页只管理 Mihomo TUN 配置，不直接切换 VPN service。
- 每个 `AndroidBridge` adapter 都必须显式应用 VPN Builder 配置并回报前台服务状态；这两项回调没有默认成功实现。
- `MihomoVpnService` 在用户启动前检查 VPN 授权；用户启动或 Android always-on 系统恢复时才进入前台。Android 14+ 的 `systemExempted` 服务类型须同时出现在 manifest、对应 `FOREGROUND_SERVICE_SYSTEM_EXEMPTED` 权限及 `startForeground()` 调用中。通知权限被拒或提示无法启动时，VPN 启动仍可继续，UI 明确提示状态通知可能被系统隐藏；只有 VPN consent prompt 启动失败会阻断该授权 flow 的 Service start。
- Core readiness 以真实 controller 探测和配置读回确认；Kotlin 子进程 spawn 回执、固定延时、进程存活、通知或 VPN 图标都不能充当成功证据。readiness 超时或失败时 Rust `CoreApplication` 停止刚启动的子进程，再发布 typed `Failed`；实际流量仍须验证 DNS 和无回环。
- `START_STICKY`、系统重建和网络变化只触发恢复流程，不证明 VPN 自动恢复成功。恢复后必须重新读取状态，并拒绝上一个 session 的操作。

## Activity、输入与后台

- Native host 以 `NativeActivityPhase` 和 `NativeWindowFocus` 发布生命周期观察；可见性由 phase 推导，`onDestroy` 在宿主观察资源释放前送达 Rust，Rust 按 generation 丢弃旧 Activity 事件并拒绝非法 phase 跳转。Compose 路由使用 sealed `AppRoute` 的显式 Saver 跨 Activity 重建保存；外部导入 URL 仍从 Android Intent 重新读取，不写入 saved-state Bundle。页面局部草稿的 Activity 重建保留仍需按页面 owner 收口，不能由 Surface 销毁代替 Core 提交。真机生命周期循环仍以 MACHINE 证据收口。
- 文本输入使用真实 Android `InputConnection` 或等效 GameTextInput 接线，覆盖 IME 组合、提交、选区、粘贴和系统返回。状态变量或合成按键不能证明真机输入可用。
- `WindowInsets` 分别表达系统安全区和 IME 区域；剪贴板通过 Android 宿主访问系统 `ClipboardManager`。复制成功须等系统写入结果，缺失和失败显式返回。
- 前后台与可见页面驱动 UI cadence 和观察订阅。静态页面不空转轮询；VPN 服务只保留必要工作。异步队列和快照有界，列表保留完整数据并以稳定业务身份回收可见实体。
- shader、GPU 资源和基础回退遵循 [Bevy UI 章程](../../crates/infiltrator-bevy-ui/docs/BEVY_UI_FRONTEND.md)。性能与功耗结论必须来自绑定当前 APK 和设备条件的实测。

## 验收与自动化入口

正式验收绑定 APK 摘要、ABI、设备/OS 和测试条件，并覆盖最终包安装、owner IPC、实际 VPN 流量、输入/恢复及受控后台。模拟器证明安装和基础系统接线；ARM64 真机用于 VPN、IME、后台和功耗结论。

Compose 与 Bevy APK 的 arm64-v8a / x86_64 Mihomo 都由 `packaging/mihomo-assets.json` 定版。Gradle `preBuild` 对两个 Android target 分别调用 `scripts/fetch-mihomo.py --stage`；脚本复用缓存时校验二进制 SHA-256，下载时同时校验 archive 和解压后二进制，再把 `libmihomo.so` 暂存到 APK 输入目录。`vendor/` 只作为经哈希校验的下载缓存，Gradle 不再从旧 vendor 文件复制；`scripts/quality/android-mihomo-assets-guard.py` 检查锁、target/ABI 映射、Gradle 任务依赖和旧路径清除。静态 guard 说明构建输入已接到锁定资产流程；最终 APK 内的字节与设备启动版本仍须按 APK 摘要和设备记录验收。

Google 推荐的 build-managed device 由 [Gradle Managed Devices](https://developer.android.com/studio/test/managed-devices) 管理模拟器生命周期；Compose 交互使用 [Compose UI testing](https://developer.android.com/develop/ui/compose/testing)，跨应用/系统界面和原生 surface 使用 [UI Automator](https://developer.android.com/training/testing/other-components/ui-automator)。

完整本机/CI 入口是 `bash scripts/android-emulator-tests.sh`。脚本按 `android/local.properties` 或 SDK 环境变量定位 SDK，安装仓库所需 Platform、Build Tools、NDK、Emulator 与 API 36 x86_64 AOSP system image，然后用同一个 Gradle task graph 构建 Compose、Bevy、Slint APK，运行各 flavor JVM 与 Managed Device 插桩测试，最后校验三份 APK 产品身份。Managed Device 自动创建、启动、安装、执行测试并销毁临时 AVD；不需手动安装 APK 或维护本机 AVD。

`OfflineEmulatorTestRunner` 在执行插桩用例前拒绝非 Android Emulator 设备，并在 guest 中启用飞行模式，避免首屏公网 IP 检查发出请求；结束时恢复 guest 原状态。测试不调用宿主网络管理工具。脚本只读取宿主接口地址与 IPv4/IPv6 路由并记录摘要；运行后若摘要变化即失败且不尝试恢复。测试证据写入 `target/android-emulator-evidence/`，Gradle 报告和 APK 由 CI artifact 保存。Compose 使用稳定 semantics tag 验证导航；每个 flavor 使用 UI Automator 冷启动、等待窗口稳定并附上启动截图；合并 Manifest 由 AndroidTest 读取已安装 APK 验证。

ARM64 真机证据只经 `.github/workflows/android-device.yml` 的显式 stage 和 `scripts/android-device-evidence.sh` 取得。模拟器测试覆盖 APK 安装、渲染、Compose 导航和 Android 系统接线；不替代真机 VPN 流量、后台和功耗证据。

当前 Manifest 声明由 `scripts/quality/android-manifest-contract-guard.py` 静态校验并接入 `scripts/quality/check-structure.sh`：它覆盖登记的 Compose/Bevy/Slint source manifests、VPN/通知/前台服务权限、唯一 `:vpn` owner services、VpnService action/type/metadata、BootReceiver、Tile 和 flavor launcher。`ManifestDeclarationsTest` 在三个 flavor 的 Managed Device 上读取已安装 APK 的 merged manifest；UI Automator 同时提供三个 launcher 的冷启动与截图证据，Compose 增加实际导航交互测试。常规 workflow 通过 `scripts/android-emulator-tests.sh` 执行这些测试，不把编译 AndroidTest APK 当作行为通过证据。
