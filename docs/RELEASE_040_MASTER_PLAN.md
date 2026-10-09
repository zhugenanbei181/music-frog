# MusicFrog Infiltrator 0.40 全平台演进与交付实施总纲

> 层级：L2
>
> **版本代号**：0.40 — Multi-Platform Production Delivery & Full-Ecosystem Closure
> **战略定位**：多平台生产级宿主交付与全生态功能闭环。
> **覆盖范围**：Core 核心中枢、Iced 3 桌面端平台、Bevy UI 5 跨端平台、原生 Android（Jetpack Compose）客户端。
> **图形底线**：Linux 桌面全面强制纯 Wayland（Wayland-Native Only），彻底剥离 X11 特性与依赖。

---

## 一、架构原则与技术边界

1. **唯一真相与 Clean Architecture 单向依赖**：严格遵循 `domain → contract → ports → application → composition → host adapters / UI surfaces`。UI 仅提交意图（`CommandIntent`）与消费不可变快照（`DomainState`）；Rust 是所有 Mihomo 控制操作与网络业务的唯一产品边界。分层细节见 [ARCHITECTURE.md](ARCHITECTURE.md)。
2. **纯粹 Wayland 桌面标准（Linux 桌面强制 Wayland）**：彻底废除 X11 依赖与特性编译，Linux 桌面端（Iced 与 Bevy UI）仅保留 `wayland` 特性；窗口管理、透明视窗、连续圆角曲率阴影、拖拽与系统托盘全面接入 Wayland 原生协议（`xdg-shell`、`ext-foreign-toplevel-list`、Wayland CSD、SNI）。拒绝 X11 / Xwayland 兜底伪兼容，纯 Wayland 为 Linux 生产发布与质量门禁的唯一图形标准。
3. **真实物理设备与生产级交付**：不支持即明确标注 `unsupported`，未在目标物理平台验证不声明“已支持”；移动平台以真实 Android 设备（ARM64 真机）与 iOS 环境为最终验收标准，不以外壳编译或无头测试替代端侧交付。

---

## 二、覆盖范围与矩阵总表

| 维度 | 目标平台 / 形态 | 核心架构与宿主技术栈 | 交付物形态 |
| :--- | :--- | :--- | :--- |
| **Core** | 通用 Rust 核心中枢 | Clean Architecture（无 Tokio 领域模型）、Mihomo 事务控制、`mihomo-dav-sync` | 共享领域/应用层库 |
| **Iced 3 平台** | **Linux** (`x86_64`)<br>**Windows** (`x86_64`, `aarch64`)<br>**macOS** (`arm64`, `x86_64`) | 原生 Rust + Iced 0.14（**Linux 纯 Wayland**）、系统托盘、多栏密集操作、特权自愈助手 | AppImage/Deb/AUR、NSIS/ZIP、DMG/Tarball |
| **Bevy UI 5 平台** | **Linux** (`x86_64`)<br>**Windows** (`x86_64`, `aarch64`)<br>**macOS** (`arm64`, `x86_64`)<br>**Android** (`arm64-v8a`, `x86_64`)<br>**iOS** (`arm64`) | Bevy UI、受限 ECS、WESL 模块化着色器、GameActivity/IME、`infiltrator-ios` Metal | 跨平台统一视窗、`Infiltrator-Bevy-Android.apk`、iOS 预留 Bundle |
| **原生 Android** | **Android** (`arm64-v8a`, `x86_64`) | Kotlin + Jetpack Compose（Material 3）、`VpnService`、UniFFI（`infiltrator-android`）、Binder IPC | `Infiltrator-Android-Release.apk` 原生伴侣 |

---

## 三、四大核心维度的技术规范

四个维度的规范与验收条件全部落在下方 Four Waves 的任务条目中，本节只列维度归属：

- **Core 核心中枢**：两阶段热重载、真实 MRS、脚本沙箱熔断、WebDAV 三向合并、五段流量拓扑（Wave 1）。
- **Iced 桌面（纯 Wayland）**：Wayland CSD 与 Polkit、Windows Service、macOS launchd Helper 与公证（Wave 4）。
- **Bevy UI 跨端**：WESL/SDF、GPU 遥测、多视窗（桌面 3 端）；GameActivity/IME/Insets/列表/节能（Android）；IosBridge/Metal/内存上限（iOS，Wave 2/3）。
- **原生 Android（Compose）**：双进程 Binder IPC、UniFFI 对齐、Material 3、分应用代理、瓦片与常驻通知（Wave 1/2）。

---

## 四、细化实施任务路线图（Four Waves）

```text
0.40 实施总谱系
├── Wave 1: 核心控制深化与跨进程 IPC 基础设施 (Core & IPC Foundation)
├── Wave 2: 原生 Android 现代化与 Bevy 移动宿主基石 (Android Native & Bevy Mobile Host)
├── Wave 3: Bevy 桌面渲染深化与 iOS 移动端闭环 (Bevy Desktop Maturity & iOS Surface)
└── Wave 4: 桌面纯 Wayland/特权服务与全平台生产发布 (Desktop Privileged Hosts & Production Release)
```

---

### Wave 1：核心控制深化与跨进程 IPC 基础设施 (Core & IPC Foundation)

#### 1.1 核心数据平面与事务增强
- **`CORE-040-01`：两阶段平滑热重载事务**
  - **主 Owner**：`infiltrator-application::core_application`
  - **内容**：实现 `PUT /configs?force=false` 预检校验与 `force=true` 落地切换，注入 Generation 校验锁；失败自动回退并恢复上一有效状态。
  - **验收条件**：构造语法错误配置热重载时不中断现有连接，状态机正确保留旧会话；测试覆盖平滑更新与熔断回滚。
- **`CORE-040-02`：真实 MRS 二进制规则集解析**
  - **主 Owner**：`infiltrator-domain::rules` / `infiltrator-application::mrs_acceleration_application`
  - **内容**：剔除估算 mock 代码，通过 Mihomo 原生 API 读取真实 `.mrs` 二进制编译缓存大小、mmap 映射状态与命中率指标。
  - **验收条件**：读回数据与内核真实提供商状态一致，无静态伪造数据。
- **`CORE-040-03`：规则沙箱安全熔断机制**
  - **主 Owner**：`infiltrator-application::script_application`
  - **内容**：针对 QuickJS 与 Boa 脚本沙箱，设立 50ms 执行超时与 16MB 内存门控，超时安全中止并降级为直连。
  - **验收条件**：包含死循环或大内存分配的脚本被安全熔断并记录错误，不阻塞应用主线程。
- **`CORE-040-04`：WebDAV 三向智能 YAML AST 合并器**
  - **主 Owner**：`mihomo-dav-sync::sync-engine`
  - **内容**：实现基于 YAML AST 的三向差异合并（Base, Local, Remote），非冲突段自动应用，冲突段标记为待审阅草稿。
  - **验收条件**：单元测试验证多端同时修改不同字段自动合并无损，同一字段冲突正确生成审阅标记。
- **`CORE-040-05`：五段流量拓扑与动态 RTT 抖动直方图**
  - **主 Owner**：`infiltrator-domain::traffic` / `infiltrator-application::traffic_topology_application`
  - **内容**：推导 `Inbound → Sniffer → RuleSet → ProxyGroup → Outbound` 五段拓扑，按 1 秒滑动窗口统计 RTT 抖动与有效吞吐。
  - **验收条件**：拓扑结构由真实连接链路推导，节点和连线状态与实际流量驱动一致。

#### 1.2 Android IPC 与 UniFFI 扩展
- **`AND-040-01`：Android UI 与 `:vpn` 服务双进程 Binder IPC**
  - **主 Owner**：`infiltrator-android::composition`
  - **内容**：拆分 UI 进程与 `:vpn` 进程，设计基于 Binder 的单向中性数据传输（`CommandIntent` 与 `DomainSnapshot`）。
  - **验收条件**：kill 杀死 UI 进程后后台 VPN 保持连通且流量不中断；重新启动 UI 能够读回真实运行状态。
- **`AND-040-02`：UniFFI 60 场景业务契约全量对齐**
  - **主 Owner**：`infiltrator-android::uniffi_api`
  - **内容**：更新 `infiltrator_android.udl`，将聚合向导、链路 GeoIP/ASN、规则追踪与 Fake-IP 映射表完整暴露至 Kotlin。
  - **验收条件**：`scripts/android-check.sh` 编译无 warning，Kotlin 侧能完整调用全部新增 API。

---

### Wave 2：原生 Android 现代化与 Bevy 移动宿主基石 (Android Native & Bevy Mobile Host)

#### 2.1 原生 Android（Compose）客户端体验演进
- **`AND-040-03`：Jetpack Compose Material 3 现代化重构**
  - **主 Owner**：`android/app`（UI 模块）
  - **内容**：引入 Material You 动态取色、自适应断点（手机、折叠屏与平板两栏分栏）及 AMOLED 纯黑主题。
  - **验收条件**：不同屏幕尺寸下布局自适应切换，暗色模式对比度符合 WCAG AA 标准。
- **`AND-040-04`：分应用代理（Per-App Proxy）增强**
  - **主 Owner**：`android/app`（Settings 模块）/ `infiltrator-android::vpn_service`
  - **内容**：支持应用包名白名单/黑名单分流，提供应用搜索、系统应用快速过滤与一键全选/反选。
  - **验收条件**：未选中的应用流量直连且不经过 TUN 网卡，选中的应用流量正常经过代理。
- **`AND-040-05`：快速设置瓦片与动态常驻前台通知**
  - **主 Owner**：`android/app::InfiltratorTileService` / `MihomoVpnService`
  - **内容**：实现快速设置瓦片（Quick Settings Tile）单触启停；常驻通知栏展示实时流速曲线并提供快捷节点切换按钮。
  - **验收条件**：在锁屏和通知栏无需进入主界面即可直接启停代理或切换节点。

#### 2.2 Bevy Android 移动宿主与输入基石
- **`BEVY-040-01`：GameActivity 迁移与原生中文 IME 状态机**
  - **主 Owner**：Android 原生窗口 adapter / `infiltrator-bevy-widgets::text_input`
  - **内容**：评估并迁移至 GameActivity / 原生 `InputConnection`，打通中文 IME 状态机（候选词、选区、预输入、撤销，BANDROID-007）。
  - **验收条件**：ARM64 真机上至少使用两个主流输入法输入中文候选词不丢字、无重复提交。
- **`BEVY-040-02`：原生 WindowInsets 安全区与手势避让**
  - **主 Owner**：Android 原生窗口 adapter / `infiltrator-bevy-widgets::responsive`
  - **内容**：通过 `WindowInsetsCompat` 动态计算状态栏、手势导航栏、挖孔与软键盘 Insets，布局动态避让（BANDROID-006）。
  - **验收条件**：横竖屏切换与键盘弹起收起时，输入框和操作按钮完全处于可视可点击区域。
- **`BEVY-040-03`：大规模业务列表虚拟化与实体池有界回收**
  - **主 Owner**：`infiltrator-bevy-widgets::list`
  - **内容**：将虚拟滚动视口接入 Proxies（2000+ 节点）、Connections（10000+ 连接）与 Rules，实体池按视口容量严格复用（BANDROID-009/010）。
  - **验收条件**：加载 2000 节点流畅滚动到尾部，活跃 Entity 数量受限有界，滚动帧率稳定在 60+ FPS。
- **`BEVY-040-04`：低功耗事件驱动调度机制**
  - **主 Owner**：`infiltrator-bevy-ui::cadence` / `contract::cadence`
  - **内容**：落地 `reactive_low_power` 调度策略：静置界面停止无谓渲染，后台切出停止 UI 观察；数据变动通过宿主事件循环主动唤醒（BANDROID-011/012）。
  - **验收条件**：界面静止无输入无动画时 GPU 占用降为 0%，后台挂起时不产生多余唤醒。

---

### Wave 3：Bevy 桌面渲染深化与 iOS 移动端闭环 (Bevy Desktop Maturity & iOS Surface)

#### 3.1 Bevy 桌面端渲染与多视窗
- **`BEVY-040-05`：WESL 模块化着色器资源治理**
  - **主 Owner**：`infiltrator-bevy-widgets::chart` / `infiltrator-bevy-ui::theme`
  - **内容**：严格遵循 Bevy 前端章程 §1.2：完善 SDF 超椭圆连续曲率卡片材质，保障资产隔离与 handle 有界回收。
  - **验收条件**：100 次切换主题与页面挂载，GPU 纹理与材质 handle 数量无泄漏并回归基线。
- **`BEVY-040-06`：GPU Bloom 与高级遥测可视化**
  - **主 Owner**：`infiltrator-bevy-ui::shell_waveform` / `pages::overview_topology`
  - **内容**：落地 Mesh2d / WESL 实时高刷流量波形图，支持可选的高动态范围泛光动效（低配模式自动优雅降级为平面渲染）。
  - **验收条件**：波形曲线平滑拟合实时流量，高刷屏幕下无丢帧与卡顿。
- **`BEVY-040-07`：多视窗 ECS 状态机协同**
  - **主 Owner**：`infiltrator-bevy-ui::window` / `infiltrator-bevy-ui::mini_hud`
  - **内容**：主窗口、独立 Mini HUD 悬浮小窗与系统托盘面板的 ECS 资源状态机同步。
  - **验收条件**：小窗与主窗口同时开启时，开关代理和节点切换状态实时对齐。

#### 3.2 iOS 移动端闭环接入
- **`IOS-040-01`：`infiltrator-ios::IosBridge` 原生桥接**
  - **主 Owner**：`infiltrator-ios`
  - **内容**：实现 `IosBridge` trait，对接 Swift 宿主、PacketTunnelProvider 与 Keychain 凭据安全存储。
  - **验收条件**：编译通过且单元测试验证 bridge 各接口的错误处理与数据传递。
- **`IOS-040-02`：Metal 渲染后端与 SafeArea 适配**
  - **主 Owner**：`infiltrator-ios` / `infiltrator-bevy-ui`
  - **内容**：配置 WGPU Metal 后端，适配动态岛、刘海屏与 iPad 舞台管理器（Stage Manager）；打通原生触控滚动手势。
  - **验收条件**：在 iOS 模拟器/真机上正常渲染 Bevy 页面，安全区边距正确避让。
- **`IOS-040-03`：NetworkExtension 15MB 物理内存上限防护**
  - **主 Owner**：`infiltrator-ios` / `infiltrator-application`
  - **内容**：裁剪扩展进程的 Rust 运行时体积，采用轻量化有界快照传输，杜绝 OOM 崩溃。
  - **验收条件**：扩展进程驻留内存稳定在 15MB 安全阈值以内。

---

### Wave 4：桌面纯 Wayland/特权服务与全平台生产发布 (Desktop Privileged Hosts & Production Release)

#### 4.1 桌面特权服务与纯 Wayland 深化
- **`DESK-040-01`：Linux 纯 Wayland 体验与 Polkit 特权集成**
  - **主 Owner**：`infiltrator-desktop` / `infiltrator-iced`
  - **内容**：全面移除 X11 依赖与特性，深耕 Wayland CSD 圆角视窗与透明拖拽；提供 Polkit 策略文件与 systemd 单元。
  - **验收条件**：在纯 Wayland 环境（Sway/Hyprland/GNOME Wayland/KDE Wayland）下原生启动，无 Xwayland 依赖，无边框渲染撕裂。
- **`DESK-040-02`：Windows Service 特权服务与免 UAC 自愈**
  - **主 Owner**：`infiltrator-desktop::service`
  - **内容**：实现 Windows Service 后台注册，支持开机静默启动与无弹窗的 TUN 自愈；增强 UWP 回环豁免实时扫描。
  - **验收条件**：非管理员用户启动客户端时，后台服务自动接管网络操作且无 UAC 弹窗。
- **`DESK-040-03`：macOS launchd Helper Tool 与代码签名公证**
  - **主 Owner**：`infiltrator-desktop::service` / 打包工程
  - **内容**：交付基于 `launchd` 的特权 Helper Tool；配置 Hardened Runtime，对接 Apple NotaryTool 自动化公证。
  - **验收条件**：首次授权后后续启动不再提示输入密码；安装包在 macOS Gatekeeper 下零报警顺利打开。

#### 4.2 全平台打包矩阵与生产发布
- **`REL-040-01`：全平台 8 大发行形态自动化构建矩阵**
  - **主 Owner**：`.github/workflows/release.yml`
  - **内容**：配置完备的自动化打包矩阵：
    1. Windows x86_64：Iced（NSIS/ZIP）+ Bevy UI（NSIS/ZIP）
    2. Windows aarch64：Iced（ZIP）+ Bevy UI（ZIP）
    3. macOS arm64：Iced（DMG/Tarball）+ Bevy UI（DMG/Tarball）
    4. macOS x86_64：Iced（DMG/Tarball）+ Bevy UI（DMG/Tarball）
    5. Linux x86_64（纯 Wayland）：Iced（AppImage/Deb/Tarball）+ Bevy UI（AppImage/Deb/Tarball）
    6. Android 原生伴侣：`Infiltrator-Android-Release.apk`（Compose + UniFFI）
    7. Android Bevy 原生：`Infiltrator-Bevy-Android-Release.apk`（`cargo-apk`）
    8. iOS 宿主结构：`Infiltrator-iOS` 预留制品与依赖包
  - **验收条件**：GitHub Actions 流水线能够单次全量输出上述 8 大产物包并附带 SHA256 校验清单。
- **`REL-040-02`：真实物理设备端到端回归与能耗验收**
  - **主 Owner**：本地测试矩阵 / `TODO.md`
  - **内容**：执行真机验收（BANDROID-018～023），完成 8 小时受控后台运行、网络切换与功耗评测。
  - **验收条件**：产出结构化真机运行测试报告，验证无死锁、无内存泄漏且发热功耗达标。

---

## 五、质量保障与工程不变量

行数预算、拆分规则、分层守卫命令与“零伪事实 / 零废话断言”底线以 [CODE_QUALITY_BASELINE.md](CODE_QUALITY_BASELINE.md) 与 [TEST_GOVERNANCE.md](TEST_GOVERNANCE.md) 为唯一来源；0.40 新增能力同样受其约束。
