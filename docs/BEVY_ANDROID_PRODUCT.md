# Bevy Android 产品工程规范

本文规定 Bevy Android 从界面入口推进为完整 Mihomo 客户端时必须满足的宿主、交互、资源与验收契约。它细化 [ARCHITECTURE.md](ARCHITECTURE.md) 和 [BEVY_UI_FRONTEND.md](BEVY_UI_FRONTEND.md)，不改变 Iced、Bevy UI 同权产品与 Android Compose 原生伴侣的定位。

当前平台状态只读取 [PLATFORM_MATRIX.md](PLATFORM_MATRIX.md)；功能 owner 只登记于 [FUNCTIONAL_MAP.md](FUNCTIONAL_MAP.md)；实施依赖、现有证据和未完成项记入本地 [TODO.md](../TODO.md)。双端场景状态仍由 [UI_PARITY_AUDIT.md](UI_PARITY_AUDIT.md) 与结构化证据解析，不在本文另建完成率。

## 1. 保留与扩展原则

- 保留并扩展已有正确行为：Kotlin VPN/通知/权限、独立 Mihomo 内核、Rust tun2proxy、共享 application 与有界快照、Bevy 页面与原生控件、触控、安全区消费、IME 状态机、虚拟窗口算法、二维波形和拓扑。
- 新宿主接线与性能整改复用已有 owner。禁止为移动端另建配置事实、业务状态机、Mihomo 客户端语义或第二套控件算法；现有缺陷不属于需要保留的行为。
- Compose 与 Bevy 共用 Android 宿主能力，现有 Compose 用户流程继续可用。桌面 Iced/Bevy 的能力、输入、数据语义和操作深度不得因移动适配退化。
- 界面、组件、宿主组合、发行产物、设备验收分别证明。没有对应平台证据时保留 typed unavailable/unsupported；规划中的能力不能据此发布为已支持。
- 工具链和依赖版本以 manifest/lockfile 为准，升级流程遵守 [UPSTREAM.md](UPSTREAM.md)。场景、受限 ECS、文件预算与本地化继续遵守前端章程及 [CODE_QUALITY_BASELINE.md](CODE_QUALITY_BASELINE.md)。

## 2. UI 与 VPN 宿主的进程及所有权

Android Bevy 产品采用 UI 与 VPN 服务分进程的工程策略；这是本项目的故障隔离选择，不宣称为 Android 对全部 VPN 应用的强制规定。

```text
UI 进程：Bevy Activity / Compose Activity + application 端口代理
    │ typed intent / result / snapshot，经 Android 宿主 IPC
    ▼
:vpn 进程：Kotlin VpnService + Rust application/composition + tun2proxy
    │ Rust outbound adapter：Mihomo REST / WebSocket
    ▼
独立 Mihomo 可执行内核进程
```

- `:vpn` 持有前台服务、TUN、内核生命周期、配置事务、凭据与后台运行事实；UI 只拥有呈现、输入和只读投影。Mihomo 子进程意味着实际进程数可超过两个，不强行为凑“双进程”改成进程内 Go 库。
- 保留可执行内核随 APK 交付、Rust tun2proxy 接管 TUN 的路线。cgo/gomobile 不作为产品可用的前置条件；如另行替换数据平面，必须验证 FD、防回环、协议能力、许可证和回滚。
- UI 与服务的应用控制 IPC 优先采用应用内 Binder。传输实现在 Android host/composition 中，跨端契约只暴露中性值类型；JNI、Binder、Context、FD 与 Tokio 类型不进入 UI 页面或 domain/contract。
- 所有控制操作继续经过 Rust application。UI 不越过该边界直接请求 `/proxies`、修改控制器配置或保存 canonical profile；REST/WebSocket 属于服务侧 outbound adapter。
- 命令与结果携带请求身份、generation/session 与 revision；服务死亡使旧请求明确失效，重新绑定后先读回事实。进程内单例、静态布尔值、JNI 注册表和 channel 不构成跨进程共享协议。
- 服务进程必须在没有 Activity 的情况下完成 native bridge、runtime、目录、凭据、Core/VPN application 与 reader 的初始化。UI 解绑、关闭窗口或销毁 Activity 不停止已授权 VPN；明确停止命令和系统撤销分别有真实终态。
- Service、UI 与内核退出分别回收其拥有的资源。UI 的退出清理不能套用桌面“退出产品即停止内核”的所有权；服务关闭则拒绝新请求、完成或取消事务并回收内核。
- JNI 宿主缓存 `JavaVM` 和有明确 owner 的必要 `GlobalRef`；`JNIEnv`、local ref 与局部字符串只在当前附着线程/调用域内使用，不跨线程缓存。attach/detach、local frame、异常检查和 GlobalRef 释放各有成对路径；Activity 重建后退役旧引用和代次。输入、Insets、clipboard 等注册一次，onDestroy/解绑清理 listener、Binder death recipient 与 UI worker，停止/错误路径同样回收。

## 3. VPN 生命周期与网络资源

- 保留 `VpnService.prepare` 授权、及时前台通知、Builder 路由/DNS/MTU、FD 交接、tun2proxy 异常结果和 `onRevoke` 路径；拒绝授权、前台提升失败、TUN 建立失败、内核退出和用户停止保持不同 typed 结果。
- 服务独立处理系统启动、sticky 重建、开机/always-on、网络切换与权限撤销，重新加载最后一次已提交配置。Activity 尚未创建时也必须能启动；发送启动 Intent 或线程已创建不能作为 VPN 已运行的证明。
- Rust 使用真实 controller readiness 与超时核验内核；固定 sleep 和进程暂时存活不能证明 API 可用。配置写入及来源比较继续经共享事务 owner。
- 明确 TUN FD 的复制、转移与关闭责任；`ParcelFileDescriptor`、`detachFd`、dup 与 Rust owned FD 逐步登记移交，借用的裸 fd 不拥有 close 权。失败、停止、撤销和重启各路径恰好回收实际持有的描述符，不依赖两个 owner 同时关闭同一个 FD；worker 取消与有界 join 记录实际终态后才完成退役，线程/子进程创建不逐次积累。
- 保留现有自身应用绕过策略，并验证全部代理、仅代理选中、选中直连及空选择等模式。需要逐 socket 保护时由 service 执行 `protect(fd)`；必须用真实出站流量证明无防回环缺口，不能只检查调用名称。
- loopback controller 与应用 IPC 分别处理访问控制和认证；密钥不进入日志、文档、截图或用户可复制的诊断原文。应用内存缓存不能替代服务的凭据 owner。
- ForegroundService 不被表述为永不死亡。WakeLock 仅在明确需要的工作期间有时限地持有，在完成、失败与撤销时释放；不能用永久锁代替生命周期恢复或功耗治理。

## 4. Activity、Surface 与原生输入

### 4.1 生命周期与 Surface

- 复用锁定 Bevy/winit 已提供的 Android Surface 销毁与恢复流程，不在业务 ECS 中另写 swapchain owner。业务只消费宿主生命周期事实，暂停呈现和 UI 观察，恢复后重绑读取并拒绝旧事件。
- `AppLifecycle`、焦点、遮挡、窗口可见性与 Activity 重建分别处理；窗口焦点变化不能冒充 Android 生命周期。旋转、锁屏、分屏与 Activity 重建不得重复注册业务资源或丢失已提交状态。
- 恢复不重复启动 VPN、不重复写配置、不重复执行旧用户命令；草稿、导航与确认面按实际来源恢复，失效请求明确提示重新审阅。

### 4.2 输入宿主与安全区

- 正式 Android 文本宿主必须提供原生 `InputConnection` 或 `GameTextInput` 能力，优先评估 GameActivity；NativeActivity 的基本按键输入不能代表完整中文 IME。切换 feature/Activity 后仍须核实锁定 winit/Bevy 的文本事件通路，必要的 native adapter 属于宿主。
- 软键盘显示/隐藏、preedit/commit/取消、选区、删除、粘贴、候选提交与编辑动作进入已有 IME/文本 owner。软键盘返回先关闭键盘，再按既有导航或确认语义处理，不能误触底层业务。
- 原生窗口适配器通过 `WindowInsetsCompat`/`WindowInsets` 获取状态栏、导航手势区、挖孔和 IME Insets，按真实窗口密度转换坐标，注入已有安全区消费链。旋转、分屏、键盘变化实时更新；键盘 Insets 与永久系统安全区分别表达，避免重复加边距。
- 剪贴板由 Android 宿主封装 `ClipboardManager`，复用订阅导入端口和文本控件的同一系统剪贴板来源。写入成功后才提示复制成功；读取受限、空内容、格式无效与平台缺失分别返回结果。
- 触控、最小热区、下拉刷新、侧滑操作和模态输入隔离继续沿用共享语义及现有控件。原生返回、TalkBack、触觉与减少动效偏好按真实平台能力接线，不以语义节点或模拟震动日志宣称设备支持。

## 5. 响应式调度与后台观察

- 保留共享 cadence 与真实 Winit 调步，在其上区分活跃交互、可见动画、静置、后台与 Android 挂起。持续更新只在交互或动画实际需要时启用；动画结束即退出活跃调度。
- 无输入、无动画、无新数据的静态界面进入事件等待；有限 `reactive_low_power(wait)` 仍包含定时唤醒，不能写成完全挂起。无限等待必须同时有可靠的外部数据/命令终态唤醒机制。
- 后台线程通过宿主事件循环代理唤醒 Bevy，再由受限 ECS system 非阻塞排水；不能只往 channel 放数据而期望休眠的 UI 自动刷新。合并重复唤醒并处理入睡与入队竞争，保证终态不会遗失。
- Android 挂起时停止 UI 渲染、图表动效与只服务于可见页面的轮询；VPN 数据平面、必要的通知和服务健康治理继续由服务管理。桌面后台调步可保留，移动端不为了维持固定 2 FPS 而空转。
- 页面观察依据可见页与前后台状态订阅，后台停止不必要的全页采样。日志 owner、VPN 与必须的业务任务不因页面隐藏丢失事实；恢复时获取完整快照，而非重放无界积压。
- CPU、GPU、唤醒次数、帧数、温度和电量分别实测；不得从 governor、枚举值或无头测试推导“零耗电”“不发热”。

## 6. 业务列表实体回收

- 复用现有虚拟窗口、动态行高、overscan 与实体池算法，真正接入节点、连接、日志、规则列表及节点网格。完整业务集合仍由共享 owner 保存，不能通过截断数据实现性能目标。
- 挂载的行根/卡片根数量由可见范围加 overscan 决定；总实体数受每行组件树大小影响，不能把“20 个行槽”描述成“整个页面只有 20 个实体”。网格按列数和实际行高计算窗口。
- 行槽以 `bsn!` 创建，通过受限 Query 原位复用文字、延迟、样式与交互绑定。滚动不反复销毁全部行；尺寸变化只调整必要的池容量和垫片。
- 回收槽绑定稳定业务 ID 和绑定代次；异步延迟、排队点击和详情操作核验身份，不能作用于刚占用旧槽的新节点。焦点、选中、详情与编辑草稿依据业务身份恢复。
- 搜索、排序、分组折叠、日志跟随/锁定、规则重排与来源事务保持完整语义；空集合、重新增长、可变行高和视口变更都须覆盖。
- 基准至少包含 500/2000 节点、10000 连接、50000 规则及日志持续增长，记录实际挂载数、滚动/搜索延迟、帧耗时分布和内存；纯窗口算术测试不能代替原生列表验收。

## 7. 异步结果与遥测图形扩展

- 保留 application/runtime 异步执行和有界 snapshot/event 桥接。UI system 禁止网络请求的同步等待或 `block_on`；执行器由 composition 注入。
- 快照可以合并为最新状态，日志按共享容量保留，命令终态必须可靠关联。队列饱和、断线、取消、超时和服务重启各有明确策略；每次 ECS 排水有界，不能耗尽一帧处理全部后台积压。
- 保留现有真实双通道波形、共享量程、缩放/十字准星、二维拓扑、流动粒子和下钻。修正静态重复光栅化：按数据版本、尺寸、主题及实际动画状态失效缓存，保持纹理和控件身份。
- shader 语言、模块分工、GPU ABI、handle 复用与回收统一遵循 [Bevy 前端章程 §1.2](BEVY_UI_FRONTEND.md)。在现有 CPU 图形路径上安排 Mesh2d/WESL 波形与可选 Bloom 扩展，按同一真实样本和相同显示语义比较 CPU/GPU/帧耗时与功耗。保留兼容回退，不宣称 GPU 路径或高刷动效零成本。
- 已有 Squircle 材质必须接到实际原生卡片，主题/尺寸更新复用材质身份，静态帧不重复上传；平面回退保留原有控件结构和可读性。图形实现与产品宿主可独立推进，设备放行仍联合验收。
- 交互式 3D/2.5D 地球、节点标记与流向弧线进入后续增强。地理位置必须携带真实来源、精度与未知状态；节点名、国家 Emoji 或延迟不能推断精确坐标，图形弧线不能冒充实际网络逐跳路径。
- 高级渲染按需加载、离页释放或休眠，支持低电量、热状态、减少动效及性能模式降级；不能影响代理或基础节点选择。共享业务语义和 Iced 等价操作面继续按双端审计验收，框架特有像素效果可登记 local。

## 8. 分层验收与交付门槛

验收层级使用 [TEST_MATRIX.md](TEST_MATRIX.md) 的 package/core/device 矩阵；UI 场景另按 [UI_PARITY_AUDIT.md](UI_PARITY_AUDIT.md) 的 L1/L2/L3 验收，两个体系的同名层级不能互相替代。

| 范围 | 必须取得的证据 | 不足以替代的证据 |
| --- | --- | --- |
| 共享语义与原生组件 | 请求终态、旧来源拒绝、实体绑定、取消/失败副作用、布局及交互回归 | 源码字符串、算法自证、组件存在 |
| 服务与 IPC | 无 Activity 冷启动、进程身份、UI 死亡后 VPN 存活、服务死亡后明确失效与重绑 | 添加 `android:process`、内存中的服务状态 |
| 最终 APK | 实际默认特性、ABI、NDK/SDK、原生库/内核/字体/图标、manifest 权限与服务、安装/升级/启动 | no-default-features 库检查、桌面二进制、旧 NativeActivity 截图 |
| 实际控制与流量 | 指定 Mihomo 资产的 readiness、节点切换、配置应用、流量、DNS、分应用路由与无回环 | loopback mock、前台通知或 VPN 图标 |
| 移动输入与恢复 | ARM64 真机中文 IME/选区/粘贴、真实 Insets、系统返回、锁屏/切后台/旋转/分屏至少 100 次循环 | 合成 `Ime`/Insets/TouchInput、一次模拟器启动 |
| 资源与长期后台 | 至少 8 小时受控后台运行、Wi-Fi/蜂窝切换与异常退出恢复；分进程内存/CPU、GPU、帧数、唤醒、耗电原始报告 | “低功耗”模式名称、无崩溃日志的短时截图 |

资源报告固定源码/产物哈希、设备/OS/GPU、默认特性、刷新率、亮度、网络、数据规模和采样时长。比较无 VPN 基线、VPN 开启 UI 关闭、静态前台、滚动/动画四种场景；数值预算在基准任务中明确后才判断通过，不复制历史纸面 RAM/APK 目标作为当前承诺。

UI 进程终止、任务划走、服务异常死亡、权限撤销和系统强制停止应用分别记录预期；不要求绕过系统强制停止。模拟器证明适用 ABI 的构建/安装/基础渲染，ARM64 真机证明实际 VPN、输入、后台和功耗，两者均绑定当前产物。

每个任务保留未满足的证据层；缺少真机或上游能力时继续完成可独立实施部分，最终状态仍为 OPEN/IN PROGRESS 或适用的明确缺口，不能把“不验证”变成 DONE。

## 9. CI 与自动化测试分层

本节把 §8 的验收表落到机器入口，**不改变验收强度**：产品实现仍归本地 TODO 的 `BANDROID-001`～`BANDROID-017`，证据层归 `BANDROID-018`～`BANDROID-023`。设备与长时证据只在显式 stage 运行，普通 PR 不得冒充。

| 层 | 入口 | 覆盖 | 不覆盖 |
| --- | --- | --- | --- |
| L0/L1 Android 编译门 | `.github/workflows/android.yml` → `scripts/android-check.sh` | 两个 ABI 的 cargo check/clippy、Compose `assembleDebug`、Bevy APK 构建 | 运行行为、权限生效、设备 |
| L1.5 清单/权限 | `scripts/quality/android-manifest-guard.py`（随 `--guards-only` 运行）、`scripts/build-bevy-apk.sh` 的 aapt 断言 | 产品清单权限/VPN service/`foregroundServiceType`/receiver；Bevy APK 实际打包权限 | service 是否真能起、运行时授权 |
| L2 Kotlin 单元 | `android/app/build.gradle.kts` 的 `testDebugUnitTest`（JUnit4，`BANDROID-021`） | JVM 逻辑 | JNI/VpnService/系统副作用 |
| L2.5 模拟器插桩 | `android/app/src/androidTest/`；编译门跑 `assembleDebugAndroidTest`，模拟器跑 `connectedDebugAndroidTest`（`BANDROID-022`） | 已安装 APK 的权限/service 声明、Activity/service 接线、Insets/IME | 真机 VPN/后台/功耗 |
| L3/L4 真机 | `.github/workflows/android-device.yml`（`workflow_dispatch`/self-hosted，`scripts/android-device-evidence.sh`）（`BANDROID-023`） | VPN 流量、防回环、≥8h 后台、功耗原始报告 | —（最终证据层） |

- 工具链版本必须两端统一：Compose 与 Bevy 共用 min 29 / target 36 / compile 36 / NDK `29.0.14206865`（`BANDROID-020`）。版本分叉时“CI 能编译”只对其中一套目标成立。
- Android 依赖基线跟随**最新稳定版**（`BANDROID-024`）：AGP、Kotlin/Compose 插件、Compose BOM、AndroidX、JNA 与 test 依赖的版本真相只在 `android/build.gradle.kts`、`android/app/build.gradle.kts`；升级前核对 Google Maven / Maven Central 与 AGP↔Gradle↔KGP 兼容表，声明最新版不等于已验证。
- `cargo-apk` 只能声明 `uses_permission`，**不能声明 `<service>`/`<receiver>`/`foregroundServiceType`**；完整 VPN 清单只能由 Gradle 宿主（`BANDROID-003`/`BANDROID-015`）交付。Bevy smoke APK 的权限声明与产品清单由 `BANDROID-019` 保持同步。
- 编译门与守卫通过**不等于**产品完成：后台、省电、权限的产品实现仍按 §2/§3/§5 由服务宿主交付，设备证据按 §8 分层取得。该编译门已于 2026-10-07 在本地用真实 NDK 29 / SDK 36 端到端执行并通过（首个运行暴露并修复了 6 个 Android-only clippy 违规，证明 host clippy 看不到这些 cfg 分支）；CI workflow 本身仍须以第一次 GitHub Actions 运行为准。
