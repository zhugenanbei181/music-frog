# 0.30 Core 重整计划：领域、应用、端口与宿主

状态：0.30 结构性重整规划。0.20 负责冻结当前可交付形态；0.30 允许对 Rust API、crate 依赖和前端接入方式做破坏性调整。

本文件定义底层重整的目标边界和验收规则。具体实现流水写入本地 `TODO.md`，功能归属仍以 `FUNCTIONAL_MAP.md` 为准。

双 UI 的实际落地状态另见 [DUAL_SURFACE_ARCHITECTURE_AUDIT_030.md](DUAL_SURFACE_ARCHITECTURE_AUDIT_030.md)：核心分层和 A-01～A-05 前置边界已成立；Bevy 非 Overview 的具体 live 能力和 225 项业务功能仍在后续推进，不要把本文件的目标边界误读为双端功能已完成。

截至 2026-09-05，A-01～A-05 已完成为架构前置：shared 11-page surface contract、application `SurfacePump`、desktop reader composition、Iced host namespace、Bevy live source adapter、fail-closed parity guard 和双端交付模板均已落地。后续开放项是 225 项具体业务能力，不再修改这组前置边界。

### 当前 0.30 进度

- [x] `release/0.20` 已冻结为提交 `9c187b2`，并从该点创建 `codex/0.30`。
- [x] `infiltrator-domain`：提取生命周期状态机，依赖树无 Tokio。
- [x] `DUAL-01-01`：Core session token 与 generation 已进入生命周期 reducer、application/lifecycle port 和双端 snapshot fencing；desktop orphan ownership 具备可执行文件校验与平台级父死清理。
- [x] `DUAL-01-02`：配置热重载通过 `CoreLifecyclePort` 进入同 session reload 事务，HTTP `force=true` 拒绝非成功状态，失败自动回退 restart；desktop/Android/iOS host composition 均有 session-preserving evidence。
- [x] `DUAL-01-03`：崩溃看门狗策略已从 application 拆出到纯 domain policy，host scheduler 每 250ms 探活，100ms 首次重试、指数退避和三次熔断均进入统一 CoreSnapshot；Iced/Bevy/Android/iOS contract evidence 已覆盖。
- [x] `DUAL-01-04`：Stable/Alpha/Meta-Core 三通道版本探测进入 contract、application surface reader 和 desktop version port；Alpha 使用命名的 `Prerelease-Alpha` release，Meta-Core 保留独立语义并映射官方 Meta 发布 feed。
- [x] `DUAL-01-05`：版本下载在解压和落盘前执行官方 SHA256 digest gate，`MihomoVersionPort` 将 Verified/Rejected 完整性状态提供给 shared surface；Iced/Bevy Settings 同步展示，不把“下载成功”误报为“制品已验证”。
- [x] `DUAL-01-06`：版本管理器保留有界本地选择历史；回滚先执行候选 binary `-v` 健康检查，再原子更新默认版本且保留现有 profile 元数据；VersionApplication、desktop command handler、Iced 与 Bevy 共享同一 rollback contract。
- [x] `DUAL-01-07`：bootstrap 缺少 controller secret 时用 OS CSPRNG 自动生成并写入 profile；EndpointSource 只向 outbound adapter 提供私有 secret，Mihomo REST/WebSocket 统一注入 Bearer header，shared surface 只发布脱敏认证状态。
- [x] `DUAL-01-08`：CoreLogLevel 通过 RuntimeGateway 进入 live `PATCH /configs`，application 读回 Mihomo 配置后才报告成功；Iced 与 Bevy 均提供 debug/info/warn/error 控件和失败时的非乐观状态。
- [x] `DUAL-01-09`：特权服务模式进入 ServiceModeSnapshot/Port/Application；desktop adapter 汇总 Windows Service、Linux Polkit、macOS launchd 状态和 post-check，Iced/Bevy 共享准备命令，未打包 helper 的 macOS 路径保持 typed unsupported。
- [x] `DUAL-01-10`：CleanExitHook 提供 SIGINT/SIGTERM/Ctrl+C 的统一 termination handler；desktop host 注册代理/TUN 清理钩子，Iced 的正常退出、panic 与信号路径都复用同一清理入口，Bevy host 只消费 host 提供的 cleanup handle。
- [x] `DUAL-01-11`：端口冲突进入 PortConflictSnapshot/PortConflictPort/Application；desktop 负责 PID 观测与 ConfigManager 安全避让，Iced/Bevy 使用相同 repair intent，不从 UI 直接 kill 未确认进程。
- [x] `DUAL-01-12`：CoreResourceSnapshot/ResourceApplication 固定 512 MiB 软限和 30 秒 GC 冷却；Mihomo `/memory` 与 `/debug/gc` 由 RuntimeGateway 提供，desktop host 补 CPU 观测，Iced/Bevy 只消费资源与回收状态。
- [x] `DUAL-01-13`：离线启动先校验本地 profile 与 core binary；生产 retry/materialize 走 `bootstrap_offline`，GeoIP 只复制已有本地资产，缺失时报告 degraded 而不访问网络；desktop、Android、iOS 与两端 UI 共享 `OfflineStartupSnapshot`。
- [x] `DUAL-01-14`：`CoreApplication`/`CoreLifecyclePort` 发布统一 `CoreLifecycleSnapshot`；Iced 从 shared snapshot 映射本地状态，Bevy 保持独立的同值 lifecycle resource，所有 Start/Stop/Restart 仍经同一 application intent。
- [x] `DUAL-01-15`：Iced 与 Bevy 均完成失败启动、端口冲突、平滑停止的 headless lifecycle matrix；workspace nextest 对 shared/application、两端 adapter 与宿主 contract 一并验收。
- [x] `DUAL-02-01`：TUN stack 进入 shared `TunStack` 四项目录；gVisor/System/Mixed 经 RuntimeGateway live PATCH+回读，LWIP 按当前 Mihomo 官方 top-level TUN 规范保持 `ReferenceOnly`，Iced/Bevy 同步展示且不可误下发。
- [x] `DUAL-02-02`：物理链路 MTU 通过 `MtuProbePort` 注入，domain 统一计算 TUN MTU/TCP MSS，application 对 live `tun.mtu` 做 PATCH+GET readback；Iced/Bevy 共用带缓存的 `MtuNegotiationSnapshot`，Android/iOS 无 native 指标时明确返回 typed unsupported。
- [x] `DUAL-02-03`：TUN `auto-route`/`strict-route` 进入 shared intent 与 Settings snapshot；application 以单次 PATCH 保持严格路由依赖自动路由，并通过 GET readback 验证，Iced/Bevy 的 checkbox/消息均只映射该 application seam。
- [x] `DUAL-02-04`：系统 HTTP/SOCKS 代理进入 `SystemProxyPort/Application/Snapshot`；desktop 的 Windows registry、Linux GNOME/KDE/GSettings/environment 和 macOS `networksetup` 只实现 host port，Iced/Bevy 通过同一 typed command 与 readback 状态接入。
- [x] `DUAL-02-05`：系统代理 ownership target 由 host port 共享，application 每 3 秒可 reconcile 外部修改并做 readback；Iced subscription 和 Bevy surface reader 使用同一恢复语义，repair 状态通过 contract 展示而非 UI 私有猜测。
- [x] `DUAL-02-06`：系统代理变更以 owner PID/启动时间、previous/desired 状态写入 durable atomic journal；启动只对仍匹配本应用 target 的孤儿状态执行恢复并 readback，外部修改 fail-safe 跳过，正常退出复用同一 host cleanup hook；Iced/Bevy 消费共享 recovery snapshot，Android/iOS 保持 typed unsupported。
- [x] `DUAL-02-07`：Mihomo Allow-LAN 的 `allow-lan`、`mixed-port`、`bind-address` 进入 shared `SetLanSharing` intent；application 对端口和 IP/括号 IPv6 做 fail-fast 校验并 PATCH+GET readback，Iced draft/Apply 与 Bevy TextField/Apply observer 同步，ACL/认证不与本项耦合。
- [x] `DUAL-02-08`：LAN ACL 与 HTTP Basic Authentication 进入 `LanSecuritySnapshot`/`SetLanSecurity`；domain 规范化 CIDR 并校验凭据，application 只在一次 PATCH 后完整回读，密码不进入 snapshot/Debug/serde 输出；Iced/Bevy 同步安全 draft 与成功后清理，desktop/Android 支持 `LanAccessControl`，iOS 在 controller gateway 缺失时 typed unsupported。
- [x] `DUAL-02-09`：Mihomo 顶层 `ipv6` 内核流量策略进入 `Ipv6RoutingSnapshot`/`SetIpv6Routing`；application 以 PATCH+GET readback、缺失字段默认 true 和 mismatch fail-closed 保证不把未知状态显示为禁用；Iced/Bevy 同步 checkbox 与 TUN 上下文，desktop/Android 支持，iOS 在 controller gateway 缺失时 typed unsupported，宿主全局 sysctl 不冒充已接入。
- [x] `DUAL-02-10`：Windows UWP 回环隔离进入 `UwpLoopbackSnapshot`/`UwpLoopbackPort`/三类 shared intent；desktop 以注册表扫描与 `CheckNetIsolation.exe` 变更后 readback，Iced/Bevy 同步真实 AppContainer 快照和 bulk/single action，Android/iOS 保持 typed unsupported，非 Windows 空列表不再伪装成功扫描。
- [x] `DUAL-02-11`：PAC 动态脚本进入 `PacSnapshot`/`PacRequest`/`PacServicePort`；application 从 live gateway 取规则并经 domain 生成/校验，desktop loopback HTTP 服务由 host 负责启停与 URL readback，Iced/Bevy 共用 Apply 请求和状态投影，Android/iOS 以 typed unsupported 记录。
- [x] `DUAL-02-12`：物理网卡/默认网关事实进入 `NetworkObservation`/`NetworkRoamingSnapshot`/`NetworkRoamingPort`；domain 负责出口选择和迁移判断，application 只在 Mihomo TUN `auto-route` 生效时调用 host route-anchor repair 并要求 readback，desktop 以参数化 Linux/macOS/Windows 路由命令执行，Iced/Bevy 共用刷新/立即修复 intent 与 Settings 投影，Android/iOS 保持 native VPN/NetworkExtension 未接入的 typed unsupported。
- [x] `DUAL-02-13`：Android VPN 生命周期进入 `VpnStartRequest`/`VpnSessionSnapshot`/`VpnServicePort`/`VpnServiceApplication`；domain 先校验 FD、proxy endpoint、路由、DNS、MTU 和 foreground 要求，Android host 通过 bridge 配置 native Builder、前台服务并启动 tun2proxy，启动/停止均要求状态 readback，`onRevoke` 复用停止路径，desktop/iOS 不模拟 Android VpnService。
- [x] `DUAL-02-14`：系统代理/TUN 快捷开关进入 `SystemToggleSnapshot`/`SystemToggleApplication`；两端侧栏、设置与 Iced Mini HUD 只消费统一 Enabled/Disabled/Pending/Unknown/Unsupported/Failed 状态，重复点击在 shared policy 层被拒绝，Bevy 的 `Activate` 与 Iced 的 Elm message 都落到同一 `SetSystemProxy`/`ToggleTun` intent。
- [x] `DUAL-02-15`：特权网络回归进入 `PrivilegedNetworkRequest`/`PrivilegedNetworkSnapshot`/`PrivilegedNetworkPort`/`PrivilegedNetworkApplication`；测试 adapter 强制注入、回读、清理与失败回滚，未注入的 desktop/Android/iOS host 显示 typed unsupported，Iced/Bevy Settings 共用 `RunPrivilegedNetworkRegression` intent 与状态投影。
- [x] `DUAL-03-01`：流量历史进入 `TrafficSample`/`TrafficWaveformSnapshot`/`TrafficWaveformApplication`，按 core generation 有界记录真实 Running/Ready 样本；domain 提供共享双通道 cubic-Bezier 值投影，Bevy chart 和 Iced Canvas 不再各自实现一套实时曲线算法，非有限/停止状态不生成假样本。
- [x] `DUAL-03-02`：动态流量 scale 进入 `TrafficScaleSnapshot`/`TrafficScaleApplication`；domain 统一峰值、5% headroom、二进制单位和刻度，Bevy/Iced 使用同一绝对 max，图表只对共享密集曲线进行一次平滑并叠加 token glow，零/非有限流量保持安全基线。
- [x] `DUAL-03-03`：分流拓扑进入 `TrafficTopologySnapshot`/`TrafficTopologyApplication`；domain 从真实 config、connection chain、proxy group 和 aggregate traffic 推导五段 `Inbound→Sniffer→RuleSet→Proxy Group→Outbound`，明确 Empty/Unsupported/Failed，Bevy `TopologyPlate` 与 Iced flow Canvas 共享节点/链路事实，流动粒子只由活动 aggregate flow 驱动。
- [x] `DUAL-03-04`：拓扑下钻进入 `TrafficTopologyNavigationTarget`/`TrafficTopologyNavigationApplication`；shared application 将节点语义映射到 Settings/Rules/Proxies，Iced 使用按钮→Elm Navigate，Bevy 使用 Button→Activate→RouteChanged，并由 drawable 快照统一 gating。
- [x] `DUAL-03-05`：活动出口进入 `ActiveExitSnapshot`/`ActiveExitApplication`；domain 从 Mihomo proxy group 的 selected node 推导地区代码、协议、delay 与 alive，ApplicationSurfaceReader 发布单一事实，Iced/Bevy 只做高保真卡片适配并保留 Empty/Unsupported/Failed。
- [x] `DUAL-03-06`：订阅配额进入 `SubscriptionQuotaSnapshot`/`SubscriptionQuotaApplication`；domain 复用已校验的 active profile `subscription-userinfo` 字段推导使用量、三级预警、过期/临期和到期倒计时，reset 无事实时保持 None，Iced/Bevy 同步展示进度与 typed status。
- [x] 纯算法 `vector_clock`、`sub_rules`、DNS/Fake-IP/TUN schema 与校验、DNS topology、diagnostics 计算器、脚本引擎、MRS、PCAP、流量审计、故障转移、Geo 缓存、hosts、idle-connection、日志脱敏、per-app routing、规则/PAC、节点 URI、filter、mixin、YAML AST、profile-options 组合、backoff、MTU、丢包和规则命中统计已从 `infiltrator-core` 物理移入 `infiltrator-domain`。
- [x] `infiltrator-contract`：落下跨端命令、快照、事件、能力、失败和 intent 模型。
- [x] `infiltrator-ports`：落下 Core process、Overview、secure store、data store 和 capability provider 端口。
- [x] `EndpointSource`/`ControllerEndpoint` 已移入 ports；profile endpoint adapter 已归位 `mihomo-config::endpoint`。
- [x] application actor/facade 第一批生命周期与 Overview seam：single-flight、adopt、bounded contract events、模式回读和运行态快照。
- [x] `CoreLifecyclePort` 已成为 apply transaction 的生命周期输入；桌面 runtime 与 Android apply 已切到 `CoreApplication`。
- [x] 旧 `infiltrator-core::session` 与未接线的 `session_adapter` 已删除；retry bootstrap 也走 `CoreApplication`。
- [x] Bevy Overview 已改为消费 application snapshot；UI crate 不再直连 `mihomo-api`、Reqwest 或 Tokio。
- [x] Desktop/Android 已把 Core process、secure store、data-dir 和 readiness 组合到 host adapter 端口。
- [x] application actor/facade 覆盖全部共享 use-case，并统一前端命令与领域快照通道；系统代理、TUN/VPN、托盘和原生权限属于 host capability，缺少实现时返回 typed Unsupported，不塞进业务层。
- [x] `CommandApplication` 扩展 handler 已接入 profile、proxy、doctor、routing、sync、settings、connection；Bevy 提供显式 handler 启动入口，未具备宿主 port 的系统能力保留 typed unsupported。
- [x] `SnapshotApplication` / `SnapshotStore` 已统一快照创建、列表、读取和恢复；文件路径安全在 core adapter，恢复复用 profile application 的 apply 事务。
- [x] `VersionApplication` / `VersionPort` 已统一版本查询、远端 release、下载进度/取消、激活和卸载；具体 manager 只在 core adapter 与 desktop boot composition 出现。
- [x] Iced 的启动、重建和 settings channel 规范化已移出版本实现：UI 只调用 desktop boot composition 与 contract `CoreReleaseChannel`，生产依赖不再包含 `mihomo-version`。
- [x] 标准 Mihomo Overview adapter 已移入 `infiltrator-composition`；application 不再直接构造 `MihomoClient`。
- [x] profile-options 仍由 host adapter 持有 sidecar IO，rules、DNS、Fake-IP、TUN、proxy-provider、sniffer 的 profile YAML 读写已统一收敛到 `infiltrator-application::configuration_application`；domain 只保留 schema、校验和内存 YAML 变换。
- [x] `AppSettings` / WebDAV / runtime-panel schema 已移入 `infiltrator-domain::settings`；keyring、TOML 和 ConfigManager 读写集中在 `settings_io`。
- [x] YAML 语法诊断与 ApplyStrategy 已移入 domain；core apply 仅保留 Mihomo/config-manager/lifecycle transaction adapter。
- [x] controller 的连接、流量、内存、provider 与 proxy schema 已由 domain 持有；`mihomo-api` 只负责 wire decode 和 adapter conversion，前端消息不再引用 `mihomo_api::types` 或 `mihomo_api::proxy::types`。
- [x] Backup bundle 的加密、ZIP/JSON 编解码、digest 和快照剪枝已移入 `infiltrator-domain::backup`；本地 settings/profile 文件收集与恢复留在 `infiltrator-core::backup_io`。
- [x] `RuntimeGateway` 已拆成 ports 的 transport-neutral controller seam；Iced 与 Admin production code 不再直接依赖 `MihomoClient`，desktop/Mihomo API 只在 outbound/host adapter 与测试组合根出现。
- [x] application 的 dispatch、串行锁和 readiness delay 已改为 `ApplicationRuntime` port；Tokio 实现只在 composition root，`infiltrator-application` production code 不再直接依赖 Tokio。
- [x] `ProfileStore` 已成为配置持久化 port；Iced profile/config flows 不再持有 `ConfigManager`，`mihomo-config` 负责把 keyring、TOML 和文件 CRUD 转换为 domain values。
- [x] `ProfileApplication` 已覆盖 profile 列表、详情、切换、保存、删除、metadata、订阅导入/更新；`SubscriptionSource` 将 HTTP/sidecar 适配留在 core adapter，前端不再调用 `infiltrator-core::profiles`。
- [x] `ConfigurationApplication` 已统一 DNS、Fake-IP、TUN、rules、provider、sniffer 的 profile YAML 读写；Iced production 已移除 core/Reqwest 直连，Iced 的文件、订阅、快照与 Admin API 走 desktop host adapter。
- [x] `NetworkApplication` / `PublicIpProbe` 已统一出口 IP 探测，Iced、Admin 和 Android 页面不再直接构造 Reqwest；Fake-IP cache 作为独立 host IO adapter 保留。
- [x] `RoutingApplication` / `AppRoutingStore` 已统一 Android 应用路由配置与 Iced 桌面模式/进程规则持久化；Android 自身包排除仍留在 Android host 规则内。
- [x] `DoctorApplication` / `DoctorPort` 已统一 doctor 检查、修复、解释和 bootstrap 结果；跨端结果落在 contract，core 只提供 Mihomo doctor host adapter，Admin、CLI、Android FFI 已接入。
- [x] `ProxyApplication`、`ConnectionApplication`、`RuntimeQueryApplication` 已覆盖 CLI 的节点/分组、连接关闭/流式观察和 logs/traffic/memory 查询；CLI handler 不再直接调用 Mihomo manager。
- [x] Android FFI 的 controller、logs、WebDAV、App Routing 与 core session 已收口：FFI 只映射 application/contract，具体 client、文件、凭据和 session apply 留在 Android 顶层 host/composition；公网 IP 也复用 `NetworkApplication`。
- [x] `SyncApplication` / `SyncPort` 已统一 WebDAV 测试、全量同步以及 Iced 上传/下载的 transport seam；进度/取消/冲突结果均用 contract，冲突键 diff 已移入 domain，冲突副本读写和路径沙箱也由 core adapter 承担。
- [x] Admin 周期调度已从 `infiltrator-core` legacy module 移入 Admin host scheduling；调度器只触发 application use-case，不进入业务 contract 或 domain。
- [x] Iced runtime handle 已收敛为 `HostRuntime` trait object；desktop 的具体 `MihomoRuntime` 只在 boot composition 中构造，UI 仅消费 gateway、generation、apply 与 typed host capability。
- [x] Iced 视图测试、application/desktop 测试和 platform interface diff 已按业务边界拆分；test-layout、line-budget、import、Bevy BSN 和 core-boundary guard 均可 enforce 通过。
- [x] `InfiltratorError` 已移入 `infiltrator-contract`；Mihomo/IO 适配通过显式边界转换，不再从 core 暴露 transport error 类型。
- [x] profile projection (`ProfileInfo` / `ProfileDetail`) 与 profile name 校验已移入 `infiltrator-domain::profiles`。
- [x] subscription 的 URL 校验、内容解码、userinfo/配额、UA、WAF 分类和安全审计已移入 `infiltrator-domain::subscription`；HTTP 重试、响应体上限和 HeaderMap 转换集中在 `infiltrator-core::subscription_io`。
- [x] proxy-provider 与 sniffer 的 schema、校验和 YAML 变换已移入 `infiltrator-domain::{proxy_providers,sniffer}`，持久化由 configuration application 通过 `ProfileStore` 完成。
- [x] 标准 adapter（配置、版本、Admin、同步）均按同一规则由 core/outbound 与各 host composition 持有；inbound surface 不再构造 concrete client、HTTP 或文件实现。
- [x] `infiltrator-ios` host crate 已建立端口与保守 capability seam，且 composition root 已有 `IosBridge -> CoreApplication` 入口；Native NetworkExtension bridge 仍待接入。
- [x] Iced、Admin、Android FFI 的共享 use-case 已完成同一 application/port facade 接入；平台独有的文件选择、VpnService、系统代理和权限路径保留在各自 host adapter。
- [x] 跨端 contract/port 不公开具体 Mihomo client、Reqwest 或 Tokio channel；Iced、Bevy、Android FFI 与 CLI handler 不再直接构造它们，Admin 的 SSE/周期调度仅作为 host transport 实现。

## 1. 版本切线

### 0.20 基线

- `release/0.20` 只冻结当前已经形成的产品安排：Iced 桌面端、Android 伴侣、Bevy UI/widgets 线、Admin API、mihomo 配置与同步能力。
- 0.20 的兼容目标是“当前工作树可复现”，不是为 0.30 保留旧 API 的兼容垫片。
- 0.20 基线提交之后，不在该线上进行 Core contract 的结构性替换。

### 0.30 破坏性重整线

- 从 0.20 基线创建 `codex/0.30` 开发线。
- 可以删除旧 runtime/client 这类直接暴露底层实现的 API，可以替换 channel、错误类型、store trait 和模块路径。
- 所有调用方在 0.30 同批次迁移；不增加仅为保留旧路径的 re-export 或长期兼容 facade。

## 2. 目标分层

```text
Iced / Bevy / Compose / Admin REST / CLI
                    │
             inbound adapters
                    │
       infiltrator-application
       （use-case、actor、生命周期、事务）
          只依赖 ports；executor 由宿主注入
                    │
       ┌────────────┴────────────┐
       │                         │
 infiltrator-domain       infiltrator-contract
 纯领域模型与算法          跨端命令/结果/快照/事件
 无 Tokio、无 UI、无 OS     无 toolkit、稳定可序列化
                    │
             infiltrator-ports
             外部能力接口
                    │
  mihomo/http/config/version      host adapters
  REST、WebSocket、文件、下载      desktop/android/ios
```

目标 crate 与当前 crate 的对应关系：

| 目标职责 | 0.30 目标 | 当前迁移来源 |
| --- | --- | --- |
| 纯领域层 | `infiltrator-domain` | `infiltrator-core` 中的状态机、规则、配置变换、编解码和纯计算模块 |
| 跨端契约 | `infiltrator-contract` | `infiltrator-shared` 中的 intent、snapshot、event、capability；主题/本地化另行归类 |
| 外部端口 | `infiltrator-ports` | `CoreProcess`、`CoreLifecyclePort`、`EndpointSource`、`SecureStore`、文件/时间/网络/TUN 等 trait |
| 应用层 | `infiltrator-application`（当前为独立垂直切片） | lifecycle/Overview use-case、apply transaction 入口、actor；其余旧 use-case 仍在迁移 |
| 组合根 | `infiltrator-composition` + 各 host composition | 将具体 outbound/host adapter 注入 application；不承载 UI 状态 |
| Mihomo outbound adapter | `mihomo-api`、`infiltrator-http` | REST/WebSocket 和 HTTP 传输实现 |
| 配置/版本 outbound adapter | `mihomo-config`、`mihomo-version` | 文件、下载、安装和版本切换实现 |
| Desktop host adapter | `infiltrator-desktop` | 进程、系统代理、TUN、凭据、托盘宿主能力 |
| Android host/FFI | `infiltrator-android`，内部拆 `host` 与 `ffi` | VpnService/JNI/UniFFI 与 Android 生命周期 |
| UI surface | `infiltrator-iced`、`infiltrator-bevy-ui` | 只消费 contract/application facade |
| Bevy 组件库 | `infiltrator-bevy-widgets` | 继续只依赖 Bevy，不依赖业务 crate |

第一阶段已经把最小 `domain`、`contract`、`ports`、`application` crate 抽出并验证；剩余 use-case 按垂直切片迁移，不再为保留旧 API 额外制造兼容层。

## 3. Tokio 边界

“业务与前端无关”不要求整个产品 Core 没有异步运行时，要求运行时不成为领域和跨端契约的一部分。

- `infiltrator-domain`、`infiltrator-contract` 禁止依赖 Tokio、Reqwest、Bevy、Iced、Compose、操作系统 API 和文件系统实现。
- `infiltrator-application` 的公开 API 可以是 `async fn`，但 production crate 不直接依赖 Tokio；串行执行与延迟通过 `ApplicationRuntime` port 注入。Tokio 只允许出现在 composition/outbound/host adapter，公开 API 不返回 `tokio::sync::*`、`JoinHandle`、`Runtime`、`reqwest::Response` 或 `MihomoClient`。
- async API 可以公开 `async fn`；Bevy 和 UniFFI 优先使用 `dispatch`、`snapshot`、`poll_events` 这类 message-based seam。
- 进程内只允许一个 Core actor/runtime。Iced、Bevy、Android Kotlin coroutine 都是调用边界，不各自再拥有一套 Core 事实。
- Bevy ECS 是渲染与投影调度器，不是长耗时网络、进程和同步任务的底层 executor。

## 4. Port 与 host 规则

端口按能力拆分，不创建一个同时包含 Desktop 和 Mobile 所有动词的巨型 `Platform` trait：

- `CoreProcess`：启动、停止、探活、退出原因；
- `SecureStore`：凭据读写；
- `DataStore` / `ProfileStore`：配置和快照持久化；
- `SystemProxy`：系统代理和旁路；
- `TunController`：TUN/VPN 生命周期；
- `AppCatalog`、`NotificationSink`、`PowerEvents`：可选宿主能力。

`infiltrator-desktop`、`infiltrator-android`、未来的 `infiltrator-ios` 是同级 host adapter。它们实现端口，不拥有业务 use-case，也不应互相依赖。

不要用一个泛化的 `infiltrator-mobile` 代替 Android/iOS：Android `VpnService` 和 iOS `NetworkExtension` 的生命周期、权限、进程模型和签名约束不同。可以共享能力模型和 contract，但平台实现必须分别存在。

Bevy 运行在 Android 不等于它实现了 Android VPN。Android 组合形态应是：

```text
Bevy UI 或 Compose
          +
Android host adapter / VpnService
          +
同一个 infiltrator-application
```

平台差异通过 `Capability`、`Availability` 和 typed `Unsupported` 表达。桌面支持任意版本下载而 Android 随 APK/ABI 交付时，应是明确的 accepted difference，而不是 UI 猜测或空值降级。

## 5. 迁移批次

1. **Freeze**：提交当前 0.20 工作树，创建并切换 0.30 开发线。
2. **Contract**：定义 `Command`、`CommandResult`、`CoreSnapshot`、`CoreEvent`、`Capability`、稳定错误码和 revision/generation。
3. **Ports**：把平台、凭据、文件、时钟、Mihomo 控制能力改为端口；底层 adapter 实现端口。
4. **Application**：将生命周期、配置应用事务和 scheduler 收敛到一个 application service/actor；application 只持有 runtime port，Tokio 仅由 composition/outbound/host adapter 实现。
5. **Domain**：抽出不依赖 IO 的状态机、规则、配置变换、订阅解析、节点 URI 编解码和诊断计算。
6. **Inbound adapters**：Admin、CLI、Iced、Bevy、Android FFI 统一调用 application facade；删除页面直接访问 `MihomoClient`/`ConfigManager` 的路径。
7. **Host split**：将 Desktop、Android、iOS 宿主实现按端口接入；Android crate 内部至少分离 host 与 FFI。
8. **Cleanup**：删除旧模块路径和兼容转发，收紧 Cargo 依赖与架构守卫，更新全端无头/真实宿主矩阵。

## 6. 0.30 验收门槛

- `infiltrator-domain` 和 `infiltrator-contract` 的依赖树中没有 Tokio、Reqwest、Bevy、Iced 或平台 crate。
- 任何跨端公开 API 都不暴露 Tokio channel、任务句柄、Reqwest 类型、`MihomoClient` 或 toolkit 类型。
- Iced、Bevy、Compose、Admin 对同一个 intent 使用同一套结果、错误、能力和 generation 语义。
- 每个进程只有一个 canonical Core 状态源；前端状态只能是带 revision 的投影或缓存。
- Desktop、Android、iOS 的能力差异都有 typed capability/unsupported 结果和对应测试。
- Bevy UI 最终移除对 `mihomo-api`、`reqwest`、Tokio 具体实现的直接依赖；`infiltrator-bevy-widgets` 继续保持业务零依赖。
- 0.30 全量迁移完成后，才删除旧 API 和旧模块路径；不为“保持编译”牺牲目标边界。
