# MusicFrog Infiltrator: 双端（Iced & Bevy UI）同步演进与成熟 Mihomo 全景并集主控规范 (Dual-Surface Parity Master Plan)

本文档是 MusicFrog Infiltrator 项目的最高战略主控台账，旨在确立 **Iced（成熟桌面端）** 与 **Bevy UI（桌面+移动统一跨平台战略端）** 的**严格同步演进机制**，并全面对标业界成熟 Mihomo 客户端，以其**最完善功能组的能力并集（Union）**作为最终目标。

> **状态声明（2026-09-05）**：本文的 15×15（225 项）是目标与执行台账，不等同于已完成。0.30 的 A-01～A-05 架构前置闸门已经完成：两条 UI 都有 shared surface bridge、明确的 host/composition 入口，Bevy 生产路由不再使用 demo projection；225 项业务能力仍必须逐项完成双端 live parity 与宿主证据。真实审计见 [DUAL_SURFACE_ARCHITECTURE_AUDIT_030.md](DUAL_SURFACE_ARCHITECTURE_AUDIT_030.md)。

### 当前逐项交付

| 项目 | 状态 | 已闭环内容 | 证据边界 |
| --- | --- | --- | --- |
| `DUAL-01-01` 多代际内核会话状态机 | `host-verified`（desktop Linux） | `SessionToken` + generation 进入 domain reducer、CoreSnapshot、application/lifecycle port；旧会话事件和两端旧 surface snapshot 均拒绝；desktop PID 记录按可执行文件校验后回收，Linux PDEATHSIG/Windows Job Object 继续兜底 | contract/domain/application/platform、Iced/Bevy session-aware 测试；Android/iOS 由 native host 持有进程，`cleanup_orphaned` 明确为安全 no-op |
| `DUAL-01-02` 平滑配置热重载 | `parity-ready` | `PUT /configs?force=true` 严格检查 HTTP 状态；application 以同一 `SessionToken` 进入/完成/失败 reload 事务，generation 不变；失败自动回退 restart；Iced/Bevy 均只接受同代同会话的新 revision | contract/domain/application/ports/core、Mihomo API、Iced/Bevy headless、Android/iOS host composition contract 测试；真实 controller/发行版打包 smoke 尚未计入 |
| `DUAL-01-03` 崩溃自愈看门狗 | `parity-ready` | shared/domain 定义 100ms 首次重试、指数退避与 3 次熔断；application 探测异常退出、串行重启并产生新 session；250ms host scheduler 将恢复动作接入 desktop/Android/iOS；Iced/Bevy Doctor 均展示同一状态 | contract/domain/application/ports、composition scheduler、Iced/Bevy projection/scene/headless、Android/iOS host contract 测试；真实发行包内异常退出 smoke 尚未计入 |
| `DUAL-01-04` 内核多通道版本交付 | `parity-ready` | contract 固定 `Stable / Alpha / Meta-Core` 三值；Alpha 严格探测 `Prerelease-Alpha`，Meta-Core 使用官方 Meta 发布 feed；application 并行探测且按通道保留失败；surface reader 以缓存结果分发到两端；Iced/Bevy Settings 同时显示选中通道与三通道结果 | `mihomo-version` 官方 API mock、application partial-failure 测试、desktop version-port composition、Iced/Bevy headless 投影测试；Android/iOS 的 CoreVersionInstall 仍按宿主包能力声明 unsupported，真实发行包 smoke 尚未计入 |
| `DUAL-01-05` 内核二进制 SHA256 校验 | `parity-ready` | 下载链在任何解压/写盘前要求官方 release `sha256:<64 hex>` digest；缺失、格式错误和篡改均 fail-closed；最近一次 Verified/Rejected 结果进入 shared version snapshot，Iced/Bevy Settings 同步展示 | `mihomo-version` verify/download/manager tests、MihomoVersionPort integrity state、application surface cache、Iced/Bevy headless projection；真实发行包签名/供应链审计尚未计入 `host-verified` |
| `DUAL-01-06` 内核版本秒级回滚 | `parity-ready` | 本地保留有界版本历史；回滚不访问网络，先对目标 binary 执行 `-v` 健康检查，再原子更新 `config.toml` 的版本指针并保留 profile 元数据；shared snapshot 暴露当前/目标/历史，Iced 与 Bevy 均提交同一 `RollbackCore` intent | `VersionManager` 历史栈与原子写入测试、VersionApplication/desktop handler、Iced update/view/behavior、Bevy command/scene/headless；当前语义是切换本地默认 binary，真实发行包运行中重启 smoke 尚未计入 `host-verified` |
| `DUAL-01-07` 外部 Controller 免密拉起 | `parity-ready` | bootstrap 在缺少 secret 时使用 OS CSPRNG 生成 256-bit hex secret，写入当前 profile；EndpointSource 私有读取，HTTP/WebSocket outbound 统一注入 `Authorization: Bearer`，跨端 snapshot 只暴露 `Secured/Missing/Unavailable` 状态 | `mihomo-config` 生成/复用与 endpoint adapter 测试、Mihomo API Bearer 测试、application surface reader、desktop composition、Iced/Bevy Settings projection；真实发行包启动/权限与跨平台 controller smoke 尚未计入 `host-verified` |
| `DUAL-01-08` 内核日志等级即时下发 | `parity-ready` | `CoreLogLevel` 固定 `debug/info/warn/error`；application 通过 RuntimeGateway PATCH `/configs` 后回读确认，失败不保留乐观状态且不触发重启；Iced Logs/Settings 与 Bevy Settings 都提交同一 typed command | RuntimeGateway/application readback tests、Mihomo PATCH adapter、Iced invalid/stopped behavior、Bevy four-choice command/scene tests；真实 mihomo 版本对所有等级的 controller smoke 尚未计入 `host-verified` |
| `DUAL-01-09` 服务模式 (Service Mode) 提权守卫 | `parity-ready` | shared `ServiceModeSnapshot/Port/Application` 统一描述 Windows Service、Linux Polkit 和 macOS launchd；desktop adapter 只在 post-check 达到 Ready 后报告成功，Iced/Bevy Settings 共用 status/action，macOS helper 未打包时保持 typed unsupported | `sc.exe`/`pkexec setcap`/launchd contract 与错误状态测试、desktop host/service composition、Iced TUN flow、Bevy command/scene/headless；当前 Linux host 可验证 Polkit argv，Windows/macOS 真实授权/签名发行包 smoke 尚未计入 `host-verified` |
| `DUAL-01-10` 进程退出清理保证 | `parity-ready` | platform `CleanExitHook` 统一注册 SIGINT/SIGTERM/Ctrl+C；信号处理线程只触发集中清理，desktop host 注册系统代理/TUN service 复位，正常退出与 panic 复用同一入口，Bevy host 复用同一 host cleanup contract | `mihomo-platform` signal/cleanup tests、desktop cleanup composition、Iced normal/panic path、Bevy host lifecycle contract；当前仅有 Linux process/PDEATHSIG 与静态 handler evidence，真实 GUI/Windows/macOS/Android/iOS 退出 smoke 尚未计入 `host-verified` |
| `DUAL-01-11` 端口冲突自动探测与避让 | `parity-ready` | shared snapshot 覆盖 `mixed-port`/`external-controller` 可用性、owner PID/名称和安全释放标记；desktop 以 `lsof`/`netstat` 尽力定位 owner，一键动作只通过 ConfigManager 重新选择可用端口并回读，不直接终止未确认第三方进程；Iced/Bevy 共用 repair intent | port contract/application、desktop owner parser/repair、Iced/Bevy Settings status/action/headless tests；当前 Linux socket/Polkit host evidence 已覆盖，Windows/macOS owner 命令与真实占用发行包 smoke 尚未计入 `host-verified` |
| `DUAL-01-12` 内核内存与 CPU 软限配额 | `parity-ready` | shared 资源快照固定 512MB memory soft limit、CPU 可选观测和 GC 状态；application 每 30 秒最多自动触发一次 Mihomo `PUT /debug/gc`，随后读取 `/memory` 回报前后值；desktop 用 sysinfo 提供 CPU，Android/iOS 无 CPU/GC port 时显示 typed unsupported，Iced/Bevy 同步展示 | `/memory`/`/debug/gc` API adapter、resource application threshold/cooldown tests、desktop CPU adapter、Iced/Bevy resource projection tests；真实高内存发行包压测及各移动宿主 GC smoke 尚未计入 `host-verified` |
| `DUAL-01-13` 离线与无网启动容灾 | `parity-ready` | 启动先做本地 profile YAML 与内核文件预校验；生产 retry/materialize 使用 offline-first 路径，只复制已有 GeoIP，缺失时以 degraded 状态继续，不访问远端鉴权、版本 feed 或 GeoIP 下载；shared/application、Iced、Bevy 与 Android/iOS native host 都消费同一 typed 状态 | desktop 本地文件 adapter、Android/iOS bridge contract、Iced/Bevy Settings projection/headless tests 已覆盖；真实断网打包启动和各宿主发行包 smoke 尚未计入 `host-verified` |
| `DUAL-01-14` 双端生命周期状态机同步 | `parity-ready` | `CoreApplication`/`CoreLifecyclePort` 发布同一 `CoreLifecycleSnapshot`（lifecycle、generation、session_token、revision、failure）；Iced 将 shared snapshot 映射为本地状态，Bevy 维护同一 lifecycle projection resource，Start/Stop/Restart 继续只走 shared command intent | application lifecycle-port、Iced 全阶段映射、Bevy resource 更新和两端 headless tests 已覆盖；真实双 UI 同屏、多宿主窗口/后台切换 smoke 尚未计入 `host-verified` |
| `DUAL-01-15` 双端无头测试全景覆盖 | `parity-ready` | shared/application 生命周期驱动具备失败启动、端口冲突与平滑停止的可模拟边界；Iced 与 Bevy 各自拥有完整 headless journey，断言错误不吞、未知 owner 不终止、停止后清理和 lifecycle/session 状态收敛 | application lifecycle tests、Iced `runtime_tray_kernels`、Bevy surface matrix、command mapping 和 workspace nextest 已覆盖；真实 GUI/发行包故障注入仍未计入 `host-verified` |
| `DUAL-02-01` TUN 四堆栈安全调度 | `parity-ready` | shared `TunStack` 固定 `gVisor / System / Mixed / LWIP` 四项；当前官方 top-level TUN 可用的前三项由 application 执行 live PATCH + GET 回读，LWIP 因上游仅作性能参考而显示为 `ReferenceOnly` 并 fail-closed；Iced/Bevy 维持同一四项目录与禁用语义 | 以 [Mihomo TUN 官方文档](https://wiki.metacubex.one/config/inbound/tun/) 为能力依据；domain/config、RuntimeGateway/application、desktop/Android/iOS command composition、Iced/Bevy 四项控件与 headless tests 已覆盖；LWIP 只有在上游正式支持并补宿主证据后才可转为 live，真实 TUN/VPN 发行包 smoke 尚未计入 `host-verified` |
| `DUAL-02-02` 物理与虚拟网卡 MTU 自适应协商 | `parity-ready` | physical-link → domain negotiation → live Mihomo TUN MTU：host 只提供物理链路事实，domain 按 80-byte TUN 开销计算 TUN MTU/TCP MSS，application 通过 PATCH + GET readback 确认；surface reader 以 5 秒缓存避免轮询启动系统命令；Iced/Bevy 展示同一物理/虚拟/应用状态，Android/iOS 没有 native MTU 时保持 typed unsupported | 以 [Mihomo TUN 官方文档](https://wiki.metacubex.one/en/config/inbound/tun/) 的 `tun.mtu` 语义为依据；contract/domain/ports/application、desktop Linux/macOS/Windows parser、Android/iOS host seam、Iced/Bevy behavior tests 已覆盖；真实多网卡漫游、VPN/发行包和各移动原生链路 smoke 尚未计入 `host-verified` |
| `DUAL-02-03` 严格路由与全局流量劫持 | `parity-ready` | `strict-route` 只能在 `auto-route` 下启用；关闭 `auto-route` 同一 PATCH 清理 strict-route，避免产生无效中间态；application 对两项做 live PATCH + GET readback，Mihomo 负责自动路由表项与 TUN 全局接管；Iced/Bevy 共享 route flags、命令和 checkbox 状态 | 以 [Mihomo TUN 官方文档](https://wiki.metacubex.one/en/config/inbound/tun/) 的 `auto-route`/`strict-route` 依赖为能力依据；contract/domain runtime snapshot、RuntimeQueryApplication、Iced update/shared projection、Bevy native checkbox/observer/headless tests 已覆盖；真实系统路由表、VPN 权限和泄漏验证仍未计入 `host-verified` |
| `DUAL-02-04` 系统 HTTP/SOCKS 代理一键注入 | `parity-ready` | shared `SystemProxyPort/Application/Snapshot` 统一读取与写入；desktop adapter 将 Windows registry、Linux GNOME/KDE/GSettings/environment fallback、macOS `networksetup` 收敛为同一 apply + readback 语义；Iced/Bevy 使用同一 system-proxy intent 与 checkbox，surface 以 3 秒缓存探活，移动宿主无该 capability 时保持 typed unsupported | 以 [Mihomo TUN 官方文档](https://wiki.metacubex.one/en/config/inbound/tun/) 的本地 HTTP/mixed proxy endpoint 语义为边界；contract/ports/application、desktop OS adapter、Iced/Bevy command/projection/headless tests 已覆盖；真实桌面会话权限、第三方代理冲突和发行包 smoke 尚未计入 `host-verified` |
| `DUAL-02-05` 系统代理被抢占实时探活 | `parity-ready` | application 记录最后一次确认的 desired proxy target；每 3 秒由 Iced watchdog 或 Bevy surface reader 探测，发现第三方修改时自动复位并递增 `repair_count`，两端以 shared ownership/status 显示并对 Iced 发一次 warning toast；command/surface 包装不同 adapter 时通过 host port 共享 ownership store | contract ownership、application reconcile/readback、desktop 进程级共享 target、Iced subscription/toast、Bevy status projection/headless tests 已覆盖；真实第三方软件抢占、桌面会话注销/恢复和发行包稳定性 smoke 尚未计入 `host-verified` |
| `DUAL-02-06` 非正常断电/死机系统代理自愈恢复 | `parity-ready` | desktop host 以带 owner PID/启动时间的 durable atomic journal 记录变更前状态与本次 desired target；启动只在当前系统代理仍匹配本应用 target 时恢复 previous，检测到外部修改则跳过且清理日志，正常退出在 readback 后恢复并清理；Iced/Bevy 从同一 recovery snapshot 展示结果，Android/iOS 通过 `SystemProxy` typed unsupported 能力明确不伪造全局代理 | contract recovery report/status、port/application journal seam、desktop crash/power-loss recovery 与 clean-exit hook、Iced startup Task/update/toast、Bevy Settings projection/headless tests，以及 Android `VpnService`/iOS 无全局代理能力证据已覆盖；真实断电、桌面会话注销/恢复和发行包 smoke 尚未计入 `host-verified` |
| `DUAL-02-07` Allow-LAN 混合端口与绑定地址 | `parity-ready` | shared `SetLanSharing` 将 `allow-lan`、`mixed-port`、`bind-address` 作为一个 live PATCH，application 校验端口/IP（含括号 IPv6）并逐字段 GET readback；Iced 以 draft + Apply 结果/代际保护回滚，Bevy 以同一 `UiCommand`、真实 TextField 和 Apply observer 投影；本项只负责监听范围，ACL/认证留给 DUAL-02-08 | 以 [Mihomo General configuration](https://wiki.metacubex.one/en/config/general/) 与 [Mihomo API](https://wiki.metacubex.one/en/api/) 的 Allow-LAN、bind-address、`PATCH /configs` 语义为依据；contract/domain-adjacent validation、RuntimeGateway/application、Iced/Bevy live projection/behavior tests 已覆盖；真实多网卡绑定、系统防火墙、移动 VPN ingress 与发行包 smoke 尚未计入 `host-verified` |
| `DUAL-02-08` 局域网接入 ACL 与 HTTP 基本认证 | `parity-ready` | shared `LanSecuritySnapshot` 只发布允许/拒绝/免认证 CIDR、认证用户名与用户数，不发布密码；application 以纯 domain CIDR/凭据校验后一次 PATCH `lan-allowed-ips`、`lan-disallowed-ips`、`skip-auth-prefixes`、`authentication` 并完整 GET readback；Iced 维护安全 draft、代际回滚并在成功后清除密码，Bevy 使用 password TextField 与同一 `SetLanSecurity` intent；desktop/Android 声明 `LanAccessControl`，iOS 在 native controller gateway 未提供前保持 typed unsupported | 以 [Mihomo General configuration](https://wiki.metacubex.one/en/config/general/) 的 ACL 黑白名单、免认证前缀和 HTTP/SOCKS/Mixed 用户认证语义为依据；contract/domain/application、Mihomo DTO、Iced/Bevy projection/behavior、Android/iOS capability evidence 已覆盖；真实局域网客户端、凭据轮换、系统防火墙/移动 VPN ingress 与发行包 smoke 尚未计入 `host-verified` |
| `DUAL-02-09` IPv6 内核流量与 TUN 转发策略开关 | `parity-ready` | shared `Ipv6RoutingSnapshot` 发布 Mihomo 顶层 `ipv6` live 值与 TUN 上下文；application 以单字段 PATCH `ipv6` 后 GET readback，缺失字段按 Mihomo 文档默认 `true` 解码，回读不一致 fail-closed；Iced 使用带代际保护的 runtime patch/回滚，Bevy 使用同一 `SetIpv6Routing` checkbox/observer，desktop/Android 声明 `Ipv6Routing`，iOS 在 native controller gateway 未提供前保持 typed unsupported | 以 [Mihomo General configuration](https://wiki.metacubex.one/en/config/general/) 的“是否允许内核接收 IPv6 流量”和 [Mihomo TUN configuration](https://wiki.metacubex.one/en/config/inbound/tun/) 的 `ipv6` 依赖为依据；contract/domain/API/application、Iced/Bevy projection/behavior、desktop/Android/iOS capability evidence 已覆盖；本项不直接修改宿主全局 sysctl/防火墙，真实双栈公网泄漏、VPN ingress 与发行包 smoke 尚未计入 `host-verified` |
| `DUAL-02-10` Windows UWP 回环隔离解除工具 | `parity-ready` | shared `UwpLoopbackSnapshot` 发布 AppContainer SID/包名/豁免状态与 typed availability；`UwpLoopbackPort/Application` 对扫描、单包切换和全量豁免/清除做真实 host 命令与 readback，桌面 Windows 使用注册表 + `CheckNetIsolation.exe`，非 Windows/Android/iOS 明确 typed unsupported；Iced 与 Bevy 均消费同一快照并提交三类 shared intent，生产路径不再用固定 UWP 列表冒充扫描结果 | 以 Windows `CheckNetIsolation.exe LoopbackExempt` host adapter、domain SID 校验、application readback、Iced live snapshot、Bevy projection/command/headless tests 为依据；真实 Windows 商店应用、UAC/企业策略和发行包 smoke 尚未计入 `host-verified` |
| `DUAL-02-11` PAC 动态代理脚本与本地服务 | `parity-ready` | shared `PacSnapshot/PacRequest` 与 `PacServicePort` 统一生成/服务状态；application 从 live gateway 读取规则，domain 编译并校验 PAC，host 只在 loopback 启停本地 HTTP 服务并回读 URL；Iced/Bevy 共用 `ApplyPac` 请求、状态投影和错误回滚，desktop 支持，Android/iOS 保持 typed unsupported | 以现有 `PacGenerator`、本地 `127.0.0.1` PAC HTTP host port、脚本验证/服务读回测试和双 UI headless behavior 为依据；真实系统代理注入、浏览器消费、跨平台桌面权限与发行包 smoke 尚未计入 `host-verified` |
| `DUAL-02-12` 物理网卡漫游与默认网关感知 | `parity-ready` | shared `NetworkObservation/NetworkRoamingSnapshot` 统一承载物理网卡、默认网关、MTU、TUN 与修复事件；domain 按默认路由/metric 选择出口，application 在 TUN `auto-route` 生效时自动发起迁移修复，desktop host 只通过参数化路由命令执行并 readback；Iced/Bevy 共用刷新与立即修复 intent，Android/iOS 在 native VPN/NetworkExtension 路由事实未提供前保持 typed unsupported | `NetworkRoamingPort/Application`、desktop Linux/macOS/Windows route parser/route-anchor repair、Iced shared snapshot/update/card、Bevy Settings scene/command/projection/headless、domain migration tests 已覆盖；真实 Wi-Fi/有线漫游、权限、VPN 泄漏和发行包 smoke 尚未计入 `host-verified` |
| `DUAL-02-13` Android VpnService 移动端无缝穿透 | `parity-ready` | shared `VpnStartRequest/VpnSessionSnapshot` 与 `VpnServicePort/Application` 统一 FD、路由、DNS、MTU、授权、前台保活、Running/Stopped/Revoked/Failed；Android host 通过 JNI/UniFFI 对接 `VpnService.prepare`、Builder/FD、tun2proxy 与前台 readback，Iced/Bevy 共用 VPN 状态/启停 intent，desktop/iOS 保持 typed unsupported | Android bridge、domain request validation、application lifecycle/readback、Android native configuration callback、Iced/Bevy Settings projection/behavior、Android host capability tests 已覆盖；真实 Android manifest/UAC-like consent、API 26–35 前台限制、真机流量/断电/撤销 smoke 尚未计入 `host-verified` |
| `DUAL-02-14` 双端系统级开关 UI 表现 100% 对等 | `parity-ready` | shared `SystemToggleSnapshot/SystemToggleApplication` 统一系统代理与 TUN 的 Enabled/Disabled/Pending/Unknown/Unsupported/Failed 状态和防重复 intent；Iced 侧栏、设置页、Mini HUD 使用同一 pending/readback 语义，Bevy 侧栏通过同一 snapshot restamp 开关、禁用不可用状态并把 Activate 转为 shared command；desktop/Android/iOS 继续由各自 host capability 决定事实来源 | contract/application 状态策略、Iced Elm 更新/渲染行为、Bevy sidebar scene/observer/projection、两端 pending/重复点击/回读 headless tests 已覆盖；真实桌面多会话、Android/iOS 原生开关联动和发行包视觉/触控 smoke 尚未计入 `host-verified` |
| `DUAL-02-15` 特权网络无头回归测试 | `parity-ready` | shared `PrivilegedNetworkSnapshot/PrivilegedNetworkApplication` 与 `PrivilegedNetworkRequest/Port` 统一 TUN service、system proxy、route repair 的注入→readback→清理事务；注入失败自动尝试 rollback，清理/readback 失败保留 typed failure，未注入 host adapter 不伪装成功；Iced/Bevy Settings 均展示同一回归状态并提交 shared intent | contract/domain validation、mock host injection/cleanup/readback/rollback tests、ApplicationSurfaceReader typed unsupported、Iced/Bevy Settings adapter/view/update/behavior tests与 desktop optional host port seam 已覆盖；真实 root/polkit/UAC/Android/iOS 原生授权、系统网络副作用和发行包 smoke 尚未计入 `host-verified` |
| `DUAL-03-01` 真实双通道流量波形（GPU Bezier） | `parity-ready` | shared `TrafficSample/TrafficWaveformSnapshot` 与 application-owned bounded history 按 core generation 清理并只记录 Running/Ready 样本；domain 统一双通道 cubic-Bezier value projection、非有限值归零与 60 点上限；Bevy surface/chart 与 Iced Canvas 均消费同一 live sample contract，demo 仍保留显式 fixture 路径 | ApplicationSurfaceReader live waveform pump、domain buffer/smoothing tests、Bevy chart input/scene/headless test、Iced Canvas shared-sample test已覆盖；真实 GPU raster、长时间高吞吐、窗口/设备帧率与发行包视觉 smoke 尚未计入 `host-verified` |
| `DUAL-03-02` 动态量程标尺与发光着色器 | `parity-ready` | shared `TrafficScaleSnapshot` 统一双通道峰值、5% headroom、B/s→KiB/s→MiB/s→GiB/s 单位与刻度；application 从 live waveform 计算 scale，Bevy chart 与 Iced Canvas 使用同一 max，已密集曲线只渲染一次并叠加 token glow；非有限/零流量保持安全基线 | contract/domain scale policy、ApplicationSurfaceReader scale pump、Bevy scale line/fixed max/glow、Iced scale label/canvas glow 与 headless tests 已覆盖；真实 GPU shader、长时峰值抖动/设备帧率和发行包视觉 smoke 尚未计入 `host-verified` |
| `DUAL-03-03` 分流链路可视化拓扑流动链 | `parity-ready` | shared `TrafficTopologySnapshot` 固定 `Inbound → Sniffer → RuleSet → Proxy Group → Outbound` 五段；application 从 Mihomo configs/connections/proxies 与 aggregate traffic 生成真实节点、链路、规则摘要、选中组/出口和 Empty/Unsupported/Failed 状态；Bevy topology widget 与 Iced Canvas 只消费同一快照，活动链路才显示流动指示 | domain/application topology derivation、ApplicationSurfaceReader wiring、Bevy `TopologyPlate`/in-place projection、Iced flow Canvas/phase update、两端 headless behavior tests 已覆盖；Mihomo 不提供 per-edge rate，因此链路标注 aggregate rate，真实 GPU/长时设备帧率、复杂多级 relay 发行包 smoke 尚未计入 `host-verified` |
| `DUAL-03-04` 拓扑节点下钻跳转交互 | `parity-ready` | shared `TrafficTopologyNavigationTarget` 与 application page mapping 固定 Inbound/Sniffer→Settings、RuleSet→Rules、Proxy Group/Outbound→Proxies；Iced stage button 发送 Elm `Navigate`，Bevy stage button 发送 `Activate→RouteChanged`，仅在 topology snapshot drawable 时启用，避免 Unsupported/Failed 快照误跳转 | application mapping unit tests、Iced route adapter/behavior test、Bevy button observer/route behavior test 已覆盖；真实触控命中区域、无障碍读屏手势与发行包导航 smoke 尚未计入 `host-verified` |
| `DUAL-03-05` 主活动出口节点高保真卡片 | `parity-ready` | shared `ActiveExitSnapshot` 统一当前策略组、出口名称、国家/地区代码、协议、延迟与存活状态；application 从 Mihomo proxy group readback 推导，Iced/Bevy 同时展示国旗、协议胶囊、延迟和 Empty/Unsupported/Failed 状态，不以静态节点冒充实时出口 | domain/application selected-node derivation、ApplicationSurfaceReader wiring、Iced high-fidelity card、Bevy in-place text projection 与 shared demo/live behavior tests 已覆盖；真实节点 GeoIP/延迟刷新、复杂 relay 和发行包视觉 smoke 尚未计入 `host-verified` |
| `DUAL-03-06` 订阅配额与临期动态仪表盘 | `parity-ready` | shared `SubscriptionQuotaSnapshot` 统一 active profile 使用量/总量/剩余百分比、到期剩余天数、下次更新与可选 billing reset；application 复用 provider `subscription-userinfo` 事实和 domain 三级预警/耗尽/过期策略，Iced/Bevy 同时展示进度、状态和“reset 未上报”，不在 0 值时填充假配额 | domain/application quota derivation、ApplicationSurfaceReader active-profile wiring、Iced Overview quota card、Bevy progress/text projection、两端 Empty/Warning/Critical/Expired behavior tests 已覆盖；真实供应商 reset 字段、时钟漂移、长时刷新和发行包视觉 smoke 尚未计入 `host-verified` |
| `DUAL-03-07` 系统代理与 TUN 模式双主控大卡 | `parity-ready` | Overview 两张主控卡统一消费 shared `SystemToggleSnapshot`；application policy 负责 Enabled/Disabled/Pending/Unknown/Unsupported/Failed 与重复点击守卫，Iced 按 Elm message 更新，Bevy 按 `Button→Activate→UiCommand` 更新，动作与侧栏/设置复用同一 intent | SystemToggleApplication policy、OverviewProjection/system-surface fan-out、Iced master-card adapter、Bevy master-card scene/observer/in-place projection、两端 action/unsupported behavior tests 已覆盖；真实桌面权限、Android VPN 原生联动、触控/发行包 smoke 尚未计入 `host-verified` |
| `DUAL-03-08` 代理运行模式即时分段控制器 | `parity-ready` | Overview 代理模式切换统一消费 shared `ProxyModeSnapshot`；`ProxyModeApplication` 负责四态模式（Rule/Global/Direct/Script）与 Script 门控校验，Iced 按 `Message::SetProxyMode` 更新，Bevy 按 `OverviewModeSegmentPill`→`Activate`→`UiCommand` 更新，四态胶囊按真实状态高亮 | ProxyModeApplication policy、OverviewProjection/surface fan-out、Iced overview_mode_segment 适配、Bevy mode_segmented_controller_scene/observer/in-place projection、两端 headless behavior tests 已覆盖；真实控制器状态回读与发行包 smoke 尚未计入 `host-verified` |
| `DUAL-03-09` 全局一键并发测速按钮 | `parity-ready` | Overview 页面头部集成全局一键测速按钮；Iced 映射 `Message::TestAllProxyDelays` 与 `runtime_testing_all_delays` 状态，Bevy 映射 `OverviewSpeedtestButton`→`Activate`→`UiCommand::TestAllProxyGroups` 并支持在席状态刷新，两端行为对等 | Bevy OverviewSpeedtestButton scene/observer/headless tests、Iced overview_speedtest_button 适配与交互测试、nextest 自动化闭环；真实并发网络测速与发行包 smoke 尚未计入 `host-verified` |
| `DUAL-03-10` 核心资源 6 项运维网格 | `parity-ready` | Overview 运维指标卡片扩展为 6 项完整网格（连接数、内存、CPU、上传、下载、总流量）；Iced stats_grid 与 Bevy chips_row_scene 均消费统一多维事实，支持响应式多列包裹与在席文本刷新 | Bevy OverviewChipKind 6-variant headless tests、Iced 6-tile stats_grid 适配与渲染测试、nextest 自动化闭环；真实长时高吞吐/多会话统计 smoke 尚未计入 `host-verified` |
| `DUAL-03-11` 公网 IP 隐私归属探针 | `parity-ready` | Overview 公网 IP 探针卡片统一消费 shared `PublicIpProbeSnapshot`；`PublicIpApplication` 负责真实外网出口 IP、归属徽标、ISP 运营商与探针状态推导，Iced 映射 `current_ip_card` 与 `Message::FetchIpInfo`，Bevy 映射 `PublicIpProbeCard` 与 `PublicIpRefreshButton` 触发 `UiCommand::RefreshPublicIpProbe`，两端对等 | Bevy PublicIpProbeCard scene/observer/headless tests、Iced current_ip_card 适配与交互测试、nextest 自动化闭环；真实全球多节点出口探测与发行包 smoke 尚未计入 `host-verified` |
| `DUAL-03-12` 卡片模块长按纵向拖拽重排 | `parity-ready` | Overview 卡片模块顺序统一消费 shared `OverviewLayoutSnapshot`；`OverviewLayoutApplication` 负责 8 类卡片（模式分段器、流量图、指标网格、主控开关、出口卡片、公网探针、拓扑流动链、配额仪表）排序校验、上移下移与拖拽重排，两端按 `ReorderOverviewCards` / `ResetOverviewCardOrder` 意图同步 | Bevy OverviewCardSlot / 移动动作 scene / headless tests、Iced `MoveOverviewCardUp/Down`/`ResetOverviewCardOrder` + 按共享顺序装配 + `overview_card_order_follows_shared_layout_moves` 测试、nextest 自动化闭环。**2026-09-12 补齐**：原 Iced 端零消费，本轮接上共享布局（上移/下移复用 `OverviewLayoutSnapshot::move_up/move_down` 语义）；真实触摸长按拖拽手势物理 smoke 尚未计入 `host-verified` |
| `DUAL-03-13` 断线与重载优雅降级蒙版 | `parity-ready` | Overview 断线与配置重载优雅降级统一消费 shared `ReconnectMaskSnapshot`；`ReconnectMaskApplication` 负责看门狗重试、配置平滑热重载与崩溃恢复判定，在核心重启/重载期间界面完整保留上一帧有效事实快照，覆以半透明平滑重载蒙版与重试倒计时 | Bevy OverviewReloadMask scene / projection in-place / headless tests；**2026-09-13 补齐 Iced**：Iced 消费 `snapshot.reconnect_mask`，Overview 活跃期渲染重载/重连横幅（阶段文案 + 尝试次数），状态接线有 `shared_reconnect_mask_reaches_the_iced_runtime_projection` 断言；真实 core 重启 smoke 尚未计入 || `DUAL-03-14` 双端全视口响应式表现 1:1 对齐 | `parity-ready` | Overview 视口响应式自适应统一消费 shared `ResponsiveViewportSnapshot`；划分 `Compact`（移动紧凑 1 列卡片/2 列指标）、`Medium`（平板 2 列卡片/3 列指标）、`Expanded`（标准桌面 2 列卡片/6 列指标）与 `Ultra`（宽屏 3 列卡片/6 列指标）四阶断点梯队，Iced 与 Bevy 双端保持 100% 结构层次对齐 | Bevy 4-tier breakpoint headless tests、Iced 响应式栅格、nextest 自动化闭环；真实手机/平板触控设备视觉 smoke 尚未计入 `host-verified` |
| `DUAL-03-15` 概览双端全景无头行为与回归测试矩阵 | `parity-ready` | Overview 15 项能力建立单一共享回归矩阵契约 `OverviewRegressionMatrixReport`；`OverviewMatrixApplication` 统一执行波形平滑、标尺量程、拓扑链、下钻跳转、出口卡片、配额预警、主控大卡、四态分段、一键测速、6项指标、公网探针、拖拽重排、降级蒙版与响应式视口共 14 场景全覆盖断言 | 自动化测试矩阵 100% 绿灯、nextest 自动化闭环；真实长期无故障运行 smoke 尚未计入 `host-verified` |
| `DUAL-04-01` 策略组 5 大分类全覆盖 | `parity-ready` | 代理策略组统一使用 shared `ProxyGroupClassification` 强枚举建模（`Selector`、`UrlTest`、`Fallback`、`LoadBalance`、`Relay`）；`ProxyApplication` 校验分类合法性与手动可选性（仅 Selector 接受外部 `PUT /proxies/{group}`，自动组由内核按策略调度），双端根据分类正确渲染语义标签与交互模式 | contract/domain 5分类解析测试、ProxyApplication::list_group_details/switch 校验、Bevy/Iced 策略组卡片分类对齐与 headless tests 已覆盖；真实多级复杂 relay 节点与发行包 smoke 尚未计入 `host-verified` |
| `DUAL-04-02` 策略组展开/折叠状态持久化 | `parity-ready` | 策略组折叠状态统一接入 shared `ProxyUiPreferences`（`collapsed_groups` 列表）；`ProxyPreferencesApplication` 负责折叠状态读写，Iced 与 Bevy 通过 `ToggleProxyGroupExpand` 意图驱动展开与折叠，状态重启记忆不丢失 | contract 偏好模型测试、ProxyPreferencesApplication::toggle_group_expand、Bevy ProxyGroupFoldButton observer / headless tests 已覆盖；真实多平台本地配置盘 IO 崩溃恢复 smoke 尚未计入 `host-verified` |
| `DUAL-04-03` 节点选择状态即时回写 | `parity-ready` | 节点手动切换统一走 shared `SelectProxyNode` 意图；`ProxyApplication::switch` 执行目标组合法性、手动可选组（Selector 校验）与组内成员校验，秒级下发 `PUT /proxies/{group}`，双端在席高亮与出口卡片状态同步刷新 | ProxyApplication switch 校验与网关调用测试、Bevy ProxyNodeButton observer / headless tests 已覆盖；真实多网卡公网连通性变更 smoke 尚未计入 `host-verified` |
| `DUAL-04-04` 节点死链一键隐藏 (Filter Alive) | `parity-ready` | 工具栏一键只看可用过滤统一消费 shared `ProxyFilterAliveSnapshot`；`derive_from_candidates` 统一计算 alive/dead/total 指标，Iced 与 Bevy 派发同一 `ToggleFilterAlive` 意图，秒级过滤超时与未测速节点 | domain/contract 状态推导测试、Bevy FilterAliveToggle / headless tests、Iced 过滤流已覆盖；真实大规模超时集群 smoke 尚未计入 `host-verified` |
| `DUAL-04-05` 四维排序控制器 | `parity-ready` | 节点卡片排序统一走 shared `ProxySortOrder` 强枚举（`LatencyAsc`、`LatencyDesc`、`NameAsc`、`NameDesc`）；`compare_candidates` 统一实现星标优先、有效延迟升降序、未测速/死链垫底规则，两端通过 `SetProxySortOrder` 意图同步并持久化 | domain 排序法则单元测试、Bevy ProxySortPill / headless tests、Iced 四维排序适配已覆盖；真实超万行节点列表滚动重排 smoke 尚未计入 `host-verified` |
| `DUAL-04-06` 节点星标置顶与收藏 | `parity-ready` | 节点卡片星标收藏接入 shared `ProxyUiPreferences::favorite_proxies` 列表；收藏节点在任何排序模式下均保证锁定于策略组首位，Iced 与 Bevy 通过 `ToggleFavoriteProxy` 意图同步触发星标点亮与重排 | domain 候选比较置顶测试、Bevy NodePinButton / headless tests、Iced 收藏适配已覆盖；真实极端并发收藏冲突 smoke 尚未计入 `host-verified` |
| `DUAL-04-07` 协议与特性高级芯片 | `parity-ready` | 节点卡片显式标注协议类型与高级特性芯片；`format_protocol_chip` 统一规范 Shadowsocks/Vless/VMess/Trojan/Hysteria2/WireGuard 命名，特性芯片标注 Reality/Vision/UDP/TFO，两端高保真展示 | domain 协议芯片格式化测试、Bevy NodeProtoText / NodeUdpTag / headless tests、Iced 特性徽标已覆盖；真实各协议特有握手字段解析 smoke 尚未计入 `host-verified` |
| `DUAL-04-08` 节点延迟多色阶渲染 | `parity-ready` | 节点延迟数值多色阶渲染统一走 shared `LatencyTier` 阶梯；<100ms 翡翠绿 (Fast)、100-200ms 青草绿 (Normal)、200-300ms 暖黄 (Medium)、>300ms 警戒红 (Slow)、超时/未测速灰色 (Timeout)，Iced 与 Bevy 色阶与文案完全一致 | domain 延迟分级与标签生成测试、Bevy LatencyText / latency_color / headless tests、Iced 色板已覆盖；真实高抖动网络色温过渡 smoke 尚未计入 `host-verified` |
| `DUAL-04-09` 单节点历史延迟 Sparkline 走势图 | `parity-ready` | 节点卡片集成最近采样微折线走势图；消费节点 `history` 延迟记录，Iced 与 Bevy 在节点卡片挂载走势微图指示器 `LatencyTrendIcon`，直观呈现网络抖动与历史稳定性 | Bevy LatencyTrendIcon / headless tests、Iced sparkline 适配已覆盖；真实长时间高频采样显存占用 smoke 尚未计入 `host-verified` |
| `DUAL-04-10` 智能拼音与协议模糊检索 | `parity-ready` | 节点检索工具栏支持多模态智能检索；`matches_proxy_filter` 统一支持节点名子串、协议与缩写（`ss`, `hy2`）、特性标签（`reality`, `vision`）、延迟比较（`<100`, `>200`）及汉字/拼音首字母（`xg`, `jp`, `sg`, `us`, `tw`），两端搜索行为 100% 对齐 | Bevy matches_proxy_filter 单元测试 / headless tests、Iced 拼音检索流已覆盖；真实超长复杂非标节点名检索 smoke 尚未计入 `host-verified` |
| `DUAL-04-11` 单节点详情下钻抽屉 | `parity-ready` | 节点卡片详情统一消费 shared `ProxyNodeDetail`；`compute_rtt_stats` 统一计算有效历史 RTT 波动区间（min/max/avg ms），展示服务器域名、落地 IP、加密方式，Iced 与 Bevy 均支持节点详情下钻唤起 | contract/domain RTT 统计测试、Bevy NodeDetailButton / headless tests、Iced InspectProxy 抽屉已覆盖；真实落地 IP 实时反查 smoke 尚未计入 `host-verified` |
| `DUAL-04-12` 策略组自定义拖拽调序 | `parity-ready` | 策略组顺序自定义调整统一接入 shared `ProxyUiPreferences::custom_group_order`；两端派发 `ReorderProxyGroups` 与 `ResetProxyGroupOrder` 意图，支持上移下移与一键重置默认顺序并持久化 | contract 排序意图测试、Bevy ProxyGroupMoveUpButton / ResetProxyGroupOrderButton / headless tests、Iced 调序流已覆盖；真实触控长按拖拽物理 smoke 尚未计入 `host-verified` |
| `DUAL-04-13` 节点卡片网格与紧凑列表无缝切换 | `parity-ready` | 节点展示布局切换统一接入 shared `ProxyUiPreferences::compact_view`；支持响应式双列网格与单列高密度列表一键切换，两端派发 `SetProxyCompactView` 意图并持久化 | contract 视图偏好测试、Bevy ToggleViewModeButton / headless tests、Iced 紧凑模式已覆盖；真实超大屏幕多列流体排版 smoke 尚未计入 `host-verified` |
| `DUAL-04-14` 测速动态脉冲骨架屏占位 | `parity-ready` | 节点测速期间数值占位统一走 `LatencySkeletonPulse`；测速阶段数值呈现波纹平滑占位或状态文本，测速完成淡入最新延迟数值，两端动效节奏一致 | Bevy LatencySkeletonPulse / headless tests、Iced runtime_testing_all_delays 占位已覆盖；真实高频测速显卡着色器平滑过渡 smoke 尚未计入 `host-verified` |
| `DUAL-04-15` 双端代理操作无头行为测试闭环 | `parity-ready` | Proxies 15 项能力建立单一共享回归矩阵契约 `ProxyRegressionMatrixReport`；`ProxyMatrixApplication` 统一执行策略组分类、展开折叠、即时回写、死链过滤、四维排序、星标置顶、协议芯片、多阶色温、Sparkline走势、拼音模糊检索、详情抽屉、拖拽调序、紧凑列表、骨架占位共 15 场景全覆盖断言 | 自动化测试矩阵 100% 绿灯、nextest 自动化闭环；真实生产复杂代理拓扑 smoke 尚未计入 `host-verified` |

> **组 05～15 交付状态（2026-09-12 账目补齐）**：
> 上表只登记到组 04（`DUAL-04-*`）。组 05～15 共 165 项目前**没有逐项验收账目**，统一按下表以组为单位记录，逐项状态在对应组开始实现时再展开为 `DUAL-XX-YY` 行。
> 判定口径：这些组中确实已存在单端代码路径（例如 Iced `view/rules_tracer.rs`、`view/script_console.rs`、`view/mini_hud.rs`、`view_root/command_palette.rs`、`view_root/aggregator_modal.rs`），但按“shared + Iced + Bevy + 双端测试 + 宿主证据”四层定义，**均不满足 `parity-ready`**，故统一记为 `planned`。
> 挂钩说明：组 15 的“4 阶响应式形态断点架构”（`DUAL-15-01`）与组 03 的“双端全视口响应式表现 1:1 对齐”（`DUAL-03-14`）是同一问题的两端验收，单独由 [RESPONSIVE_PARITY_LEDGER.md](RESPONSIVE_PARITY_LEDGER.md) 权威跟踪。

| 业务组 | 项数 | 当前状态 | 单端代码路径（未验收） | 备注 |
| :--- | :---: | :--- | :--- | :--- |
| 组 05 协议生态保真与多路复用 | 15 | `planned` | `infiltrator-domain::profile_converter` | 后端解析已有测试；双端 UI 编辑面未验收 |
| 组 06 并发测速与稳定性评估 | 15 | `parity-ready` | `SpeedtestApplication` 引擎 | 15/15 收口（2026-09-22 批次 D）：单端口 `SpeedtestPort`、reader 发布真实快照、Iced 渲染共享快照（不再伪造）、Bevy 按钮按 phase/progress 重盖；出口 IP 对比、双端明细弹窗与共享状态机矩阵已闭环 |
| 组 07 订阅生命周期与定时更新 | 15 | `parity-ready` | `subscription`、`filter` 管道 | 15/15 收口（2026-09-22）：07-09/14 于批次 E 双端接线（见组 07 逐项账目） |
| 组 08 多源聚合器与自动拓扑 | 15 | `parity-ready (15/15)` | `profile_aggregator.rs`（domain）、`aggregator_modal.rs`（Iced）、`profiles_aggregator.rs`/`profiles_aggregator_wizard.rs`（Bevy） | 15/15 收口（2026-09-22）：批次 A 收口 08-01/02/03/04/05/06/14/15，批次 B 收口 08-07/08/09/10/11/12/13（见组 08 逐项账目） |
| 组 09 AST YAML 引擎与快照 Diff | 15 | `planned` | `snapshot_diff_modal.rs`、`profiles_diff.rs` | 双端编辑器与回滚事务未验收 |
| 组 10 脚本沙箱与多级 Mixin | 15 | `planned` | `script_console.rs`、`profiles_script.rs` | 双端控制台与熔断测试未验收 |
| 组 11 规则引擎与 MRS 治理 | 15 | `in progress` | `rules.rs`、`rules_mrs.rs`、`mrs` | 2026-09-22 起逐项展开（见组 11 逐项账目）：01/02/03/04/09/10/11/12/13/15 已双端收口（10/15），05/08 为 `shared-ready`（304 仅存核内、无 O(1) 虚拟滚动），06/07/14 `planned` |
| 组 12 Live Rule Tracer 与命中审计 | 15 | `parity-ready` | `rules_tracer.rs`（两端同名） | 15/15 收口（2026-09-22）：决策链回放/预设/离线模拟/命中审计/时延审计/沙盒来源 IP/反向应用均双端接线（见组 12 逐项账目） |
| 组 13 连接审计与深度透视 | 15 | `in progress` | `connections.rs`、`connection_drawer.rs` | 2026-09-22 起逐项展开（见组 13 逐项账目）：01/02/03/06/07/08/09/11/13/15 已双端收口，04/05/10/12/14 `planned`（04 宿主无阶段耗时、05 无 ASN 事实源、10/12 无瞬时速率、14 受 04 阻塞） |
| 组 14 DNS 工作台与泄漏探活 | 15 | `in progress` | `dns.rs`（两端） | 2026-09-22 起逐项展开（见组 14 逐项账目）：01/02/03/04/05/07/12/14/15 已双端收口（9/15），06/08/09/10/11/13 `planned`（宿主事实缺失：无实时 Fake-IP 池/多源泄漏结论/WebRTC 探测/逐 Nameserver 延迟/Hosts 编辑面/DNS 自愈快照） |
| 组 15 多模态外壳与极客命令流 | 15 | `in progress` | `mini_hud.rs`、`command_palette.rs`、`sidebar.rs` | 2026-09-22 起逐项展开（见组 15 逐项账目）：06/09/12 已双端收口，05/14/15 为 `shared-ready`，01 由 [RESPONSIVE_PARITY_LEDGER.md](RESPONSIVE_PARITY_LEDGER.md) 权威跟踪，其余 `planned` |

### 组 06 逐项账目（2026-09-12 展开）

组 06 是第一个逐项展开的 `DUAL-XX-YY` 组，闭环口径 = shared contract + Iced + Bevy + 双端测试。

| 项 | 任务 | 状态 | 证据 |
| :--- | :--- | :--- | :--- |
| `DUAL-06-01` | 信号量流控并发测速（Semaphore 30） | `parity-ready` | 运行时流控：`SpeedtestApplication` 持 `Arc<AtomicUsize>`，`set_concurrency`（下限 1）实时发布到 `snapshot.config.concurrency` 并驱动 `buffer_unordered(concurrency_limit)`；`SpeedtestPort::set_concurrency`（缺省 typed unsupported，`DesktopSpeedtestPort` 委托引擎）+ `CommandIntent::SetSpeedtestConcurrency`（`command_application` 路由、`command_name` 映射）；Iced `speedtest_card` 展示 `snapshot.config.concurrency` 并经 `Message::AdjustSpeedtestConcurrency` 写回端口，Bevy `OverviewSpeedtestConcurrencyStep` 步进提交 `UiCommand::SetSpeedtestConcurrency`。证据：`test_runtime_set_concurrency_updates_config_and_effective_bound`（bound≤4 且 config=4）、`test_advancement_w3_3_speedtest_concurrency_reads_shared_and_clamps`、`overview_speedtest_concurrency_stepper_submits_shared_intent` |
| `DUAL-06-02` | 单策略组独立测速 | `parity-ready` | `SpeedtestScope::SingleGroup` + `SpeedtestPort::run_scope`；Iced `TestGroupDelay` 删除 legacy `test_proxy_delays` 第二路径，改经共享引擎（`run_speedtest_scope`）；Bevy `TestProxyGroup`→`TestDelay { group }` |
| `DUAL-06-03` | 测速目标 URL 动态自定义 | `parity-ready` | `SpeedtestTargetConfig.test_url` 由引擎持有默认；Iced 卡片 URL 输入存 `runtime_speedtest_url`（`Message::UpdateSpeedtestTestUrl`），`run_speedtest_scope`/`RunNodeSpeedtest` 经 `speedtest_target_url()` 传入 `run_scope`/`probe_node`（空白回退引擎默认）；Bevy Overview URL 字段经 `UiCommand::TestAllProxyGroupsWithUrl`→`TestDelay { url: Some(..) }`。证据：`test_dynamic_custom_url_and_timeout_propagation`、`test_advancement_w3_3_speedtest_custom_url_flows_into_the_port_call`、`overview_speedtest_typed_url_reaches_the_shared_intent` |
| `DUAL-06-04` | 真实下行带宽测速 | `parity-ready` | seam 级诚实链路：host 实测后经 `CommandIntent::RecordSpeedtestBandwidth` / `SpeedtestPort::record_bandwidth` 上报 bytes+duration，`DesktopSpeedtestPort` 委托共享 `SpeedtestApplication::record_bandwidth`（domain `SpeedtestCalculator` 换算 Mbps，零时长诚实 0.0，无伪造）；`UnsupportedSpeedtestPort` 保持 typed 拒绝。engine 测试 `test_record_bandwidth_populates_shared_snapshot` 断言结果进入发布快照；Iced `speedtest_card` 渲染 `bandwidth_mbps`，Bevy `OverviewSpeedtestMetricsText` 从同一 `fastest_node().bandwidth_mbps` 重盖 |
| `DUAL-06-05` | 网络抖动 (Jitter ms) 精确计算 | `parity-ready` | `JitterCalculation`（std dev / RFC3550 EWMA）；Iced 渲染共享快照，Bevy `OverviewSpeedtestMetricsText` 从 `fastest_node().jitter` 重盖 |
| `DUAL-06-06` | 丢包率梯度评级 | `parity-ready` | `PacketLossRating::from_loss_percent`；Iced loss badge，Bevy 指标行渲染 `packet_loss.label_en()` |
| `DUAL-06-07` | 五星稳定性综合雷达评分 | `parity-ready` | `star_rating` / `stability_score`；Iced 展示星标，Bevy 指标行渲染 `★×star_rating` |
| `DUAL-06-08` | 测速进度环形百分比动画 | `parity-ready` | `SpeedtestProgress`；Bevy `sync_overview_speedtest_button` 重盖「测速中 n/m」，Iced Overview 按钮改为消费共享 `snapshot.is_running()` + `progress` 显示 n/m |
| `DUAL-06-09` | 超时与不可用节点即时归档 | `parity-ready` | `is_alive` / `dead_nodes()`；Iced `speedtest_card` 死链归档区（计数徽标 + 前 4 名 + 溢出），Bevy `OverviewSpeedtestDeadText`「超时归档 n · 名单」；空态诚实 `—` |
| `DUAL-06-10` | 测速取消与安全中断 | `parity-ready` | `SpeedtestPort::cancel`；Iced `Message::CancelSpeedtest` 经 port、运行中 Overview 按钮切换为取消；Bevy `UiCommand::CancelSpeedtest`→`CommandIntent::CancelSpeedtest`，同一按钮运行中提交取消 |
| `DUAL-06-11` | 历史测速数据持久化缓存 | `parity-ready` | `SpeedtestHistoryStore` port（`speedtest_history.rs`）+ `SpeedtestApplication::with_history_store`：构造时载入有界历史（上限 3），每次完成 run 与带宽上报后写回；desktop `FileSpeedtestHistoryStore`（home 目录 JSON）经 `runtime.rs` 注入同一 engine。Iced `shared_speedtest_history_lines` 与 Bevy `OverviewSpeedtestHistoryText` 都只渲染共享 `snapshot.recent_history`（run 时间/scope/存活/平均延迟/抖动/带宽/星级）；跨重启证明测试 `test_history_store_round_trip_restores_across_restart` + `file_store_round_trips_records` |
| `DUAL-06-12` | 节点真实 IP 与出口探测对比 | `parity-ready` | seam 级诚实链路：`SpeedtestPort::probe_outbound_ip`（缺省 typed unsupported，Desktop 无穿透探测能力时保持拒绝）由 host 实测，经 `CommandIntent::RecordSpeedtestOutboundIp` / `SpeedtestPort::record_outbound_ip` 上报，`SpeedtestApplication::record_outbound_ip` 只存事实（空 IP 拒绝、国家可诚实为 `None`）。contract 将「标签国家」`label_country`（`extract_country_code`）与「真实出口国家」`outbound_country` 分离，`EgressCountryMatch::classify` 给出 `Unknown/Unlabelled/Match/Mismatch`，`SpeedtestSnapshot::egress_country_mismatches/egress_reported_count/egress_summary` 为纯读模型派生，绝不伪造出口。Iced 卡片与 `speedtest_detail_modal` 渲染 `egress_endpoint_label()` 与归属徽标；Bevy `OverviewSpeedtestEgressText` 从同一 `fastest_node()` 重盖。证据：`test_record_outbound_ip_populates_and_compares_label_country`（HK 标签 + US 出口 = mismatch；空 IP 拒绝；无国家 = Unknown）、`test_egress_country_match_classification`、`test_advancement_w3_3_speedtest_egress_detail_and_matrix`、`overview_speedtest_egress_and_detail_modal_follow_shared_engine` |
| `DUAL-06-13` | 测速结果弹窗详细透视 | `parity-ready` | Iced 新增 `view_root/speedtest_detail_modal.rs`（`modal_backdrop`/`modal_card` 复用既有 modal 形态），`Message::OpenSpeedtestDetail`/`CloseSpeedtestDetail` 驱动 `diag.speedtest_detail_open`，卡片「结果透视」按钮唤起；弹窗逐节点渲染延迟/抖动/丢包/带宽/星级/出口 IP+国家与归属对比，空态「暂无测速结果」、失败态回显 `snapshot.failure`。Bevy 用 `infiltrator-bevy-widgets::adaptive_modal::adaptive_modal_scene` 在 shell 根挂载 `overview_speedtest_detail_modal_scene`（`OverviewSpeedtestDetailButton` 触发 `OpenModal`），`sync_overview_speedtest_detail` 从共享快照重盖 `OverviewSpeedtestDetailBodyText`（含 `出口状态` 摘要与逐节点行）。证据：`test_advancement_w3_3_speedtest_egress_detail_and_matrix`、`overview_speedtest_egress_and_detail_modal_follow_shared_engine` |
| `DUAL-06-14` | 双端测速状态机与动效一致 | `parity-ready` | 新增共享回归矩阵契约 `SpeedtestRegressionMatrixReport::run_deterministic_matrix()`（contract `speedtest_matrix.rs`）覆盖 15 项：六阶段（Idle/Probing/Measuring/Completed/Cancelled/Failed）状态机、取消、并发、目标 URL、历史、出口对比、明细投影与双端一致性；每个 `passed` 为真实计算（非硬编码）。`SpeedtestMatrixApplication::execute`（application）统一执行，`verify_dual_surface_snapshot` 校验任意实时快照是唯一事实源。两端 UI 均只消费同一 `SpeedtestSnapshot`：Iced `diag.speedtest`、Bevy `OverviewProjection.speedtest`（`surface_projection` clone）。证据：contract `deterministic_matrix_passes_all_scenarios`、application `test_speedtest_matrix_application_execution`、Iced/Bevy 测试中调用同一 `run_deterministic_matrix()` 断言 15/15 |
| `DUAL-06-15` | 测速流控与状态机无头测试 | `parity-ready` | `speedtest_headless_tests.rs` + Iced 快照/失败断言 + Bevy 阶段重盖断言 |

> **2026-09-21 组 06 批次 A**：`DUAL-06-02/05/06/07/08/10` 收口为 `parity-ready`。
> Iced 删除 `test_proxy_delays` legacy 第二路径，组/全量测速统一经共享
> `SpeedtestPort::run_scope`；Overview 按钮消费共享 `is_running`+`progress`
> 显示 n/m 并在运行中切换为取消；Bevy 新增 `OverviewSpeedtestMetricsText`
> 从同一 `projection.speedtest.fastest_node()` 渲染抖动/丢包/星级/带宽，
> 运行中按钮提交 `CancelSpeedtest`。守卫 `speedtest-parity-guard.py`（含
> 「Iced 不得再引用 legacy 路径」反向断言）。剩余 06-01/03/04/09/11/12/13/14。

> **2026-09-21 组 06 批次 B**：`DUAL-06-04/11` 收口为 `parity-ready`。
> 04：新增 `CommandIntent::RecordSpeedtestBandwidth` 与 `SpeedtestPort::record_bandwidth`，
> host 实测 bytes/duration 经共享 engine 换算 Mbps，`UnsupportedSpeedtestPort` typed 拒绝；
> 11：新增 `SpeedtestHistoryStore` port 与 `SpeedtestApplication::with_history_store`
> （构造载入、完成写回、上限 3），desktop `FileSpeedtestHistoryStore` 以 JSON 落盘，
> Iced `shared_speedtest_history_lines` 与 Bevy `OverviewSpeedtestHistoryText` 同读
> `snapshot.recent_history`，两端 UI 不得自建历史源。守卫 `speedtest-history-guard.py`
> （含「UI 不得持有 history store」反向断言）。剩余 06-01/03/12/13/14。

> **2026-09-22 组 06 批次 C**：`DUAL-06-01/03` 收口为 `parity-ready`。
> 01：`SpeedtestApplication` 的并发上限改为 `Arc<AtomicUsize>`，新增
> `set_concurrency`（下限 1）实时写入 `snapshot.config.concurrency` 并作为
> `buffer_unordered` 的真实 bound；`SpeedtestPort::set_concurrency` 缺省
> typed unsupported，`DesktopSpeedtestPort` 委托同一引擎；
> `CommandIntent::SetSpeedtestConcurrency` 经 `command_application` 路由、
> `command_name` 命名。Iced 卡片显示 `snapshot.config.concurrency` 并以
> +/- 步进经端口写回（`Message::AdjustSpeedtestConcurrency`），Bevy Overview
> 步进按钮提交 `UiCommand::SetSpeedtestConcurrency`。
> 03：Iced 卡片 URL 输入存入 `runtime_speedtest_url`（`UpdateSpeedtestTestUrl`），
> `run_speedtest_scope`/`RunNodeSpeedtest` 统一经 `speedtest_target_url()`
> 传入端口（空白回退引擎默认）；Bevy Overview URL 字段经
> `UiCommand::TestAllProxyGroupsWithUrl` → `TestDelay { url: Some(..) }`。
> 守卫 `speedtest-config-guard.py`（含「UI 不得持有 effective concurrency /
> target URL 事实」反向断言）。剩余 06-12/13/14。

> **2026-09-22 组 06 批次 D（收官）**：`DUAL-06-12/13/14` 收口为
> `parity-ready`，组 06 达到 **15/15**。
> 12：新增 `SpeedtestPort::probe_outbound_ip` / `record_outbound_ip` 与
> `CommandIntent::RecordSpeedtestOutboundIp`，contract 将标签国家
> (`label_country`) 与真实出口国家 (`outbound_country`) 分离，
> `EgressCountryMatch` 给出诚实 Match/Mismatch/Unknown/Unlabelled 对比；
> Desktop 无穿透探测能力时保持 typed unsupported，不伪造出口。
> 13：Iced 新增 `view_root/speedtest_detail_modal.rs` 逐节点明细弹窗
> （`OpenSpeedtestDetail`/`CloseSpeedtestDetail`），Bevy 用
> `adaptive_modal_scene` 在 shell 根挂载明细抽屉（`sync_overview_speedtest_detail`
> 重盖），两端均含诚实空态/失败态。
> 14：新增共享 `SpeedtestRegressionMatrixReport` + `SpeedtestMatrixApplication`
> 覆盖六阶段状态机/取消/并发/URL/历史/出口对比/双端一致性，`passed` 均为真实
> 计算。守卫 `speedtest-final-guard.py`（含「Iced 不得本地伪造 outbound_ip/country」
> 反向断言）。

> **关键修复**：此前 Iced 的测速结果由 UI 内硬编码的 48MB/2400ms 与假抖动样本伪造。现已删除该第二条事实源，改为经 `SpeedtestPort` 驱动 `SpeedtestApplication` 并渲染共享快照；host 无引擎时按 typed unsupported 报错，不再伪造成功。

### 组 12 逐项账目（2026-09-13 展开）

闭环口径同组 06。本轮关键事实：`RuleTracerApplication` 此前从未在 `infiltrator-application/src/lib.rs` 挂载——整个应用服务（含测试）是死代码；reader 对 Rules 页发布的是硬编码空投影（`D-017`）。

| 项 | 任务 | 状态 | 证据 |
| :--- | :--- | :--- | :--- |
| `DUAL-12-01` | 交互式分流追踪沙盒视口 | `parity-ready` | Iced 查询输入 + `RunRulesTracer` 经 `RuleTracerPort` 驱动共享引擎；Bevy 预设芯片数据驱动 |
| `DUAL-12-02` | 分流决策链树状回放 | `parity-ready` | 共享 `DecisionChainSnapshot`：Iced 五阶段链路卡 + 命中摘要；Bevy `TracerDecisionTree` 逐节点行 |
| `DUAL-12-03` | 快捷测试预设域名芯片 | `parity-ready` | 双端均渲染共享快照 `presets`（`RuleTracerSnapshot::default_presets`） |
| `DUAL-12-13` | 离线分流追踪支持 | `parity-ready` | `RuleTracerApplication::project` 离线 AST 推演 + 诚实「未知出口」；hostless demo 走同一应用直调 |
| `DUAL-12-14` | 双端 Tracer 沙盒组件完全镜像 | `parity-ready` | reader 真实投影 `pages.rules.tracer`；Bevy 经 `RulesProjection.tracer` 消费；Iced 经 `HostRuntime::rule_tracer_port` |
| `DUAL-12-15` | Tracer 判定算法无头断言覆盖 | `parity-ready` | domain `tracer_tests`（含未知出口诚实断言）、application 端口/离线测试、Iced chain 断言、Bevy `test_rules_tracer_projection_renders_shared_decision_chain` |
| `DUAL-12-04` | 规则命中实时流计数 (Hit Counter) | `parity-ready` | 共享 `RuleHitAuditSnapshot`（contract `rule_tracer.rs`）；application `RuleTracerApplication::record_hits/audit` 拥有唯一 `RuleHitCounter`，`project()` 发布审计；reader 不再传 `None`，逐条命中数来自共享引擎；Iced 删除 `total_rule_hits=1250` 伪造桩改为消费 `pages.rules.tracer.hit_audit`，Bevy `RulesProjection.hit_audit` 渲染；双端断言 |
| `DUAL-12-05` | 冷门死规则静态诊断 | `parity-ready` | domain `find_shadowed_rules` 已在 reader 生效；本轮审计快照 `dead_rules` 合并零命中与 `ShadowReason`；Iced `AuditStaleRules` 从共享 `dead_rules` 投影索引（删除 `idx % 2` 伪造），Bevy 行标签渲染「冷门/被遮蔽」 |
| `DUAL-12-06` | IP-CIDR 掩码重叠与冲突检测 | `parity-ready` | domain `cidr_contains`/`ShadowReason::IpCidrShadowedByCidr` 为事实源；审计快照 `cidr_overlaps` 子集发布；Iced 审计卡与 Bevy 头部行展示 CIDR 重叠数；application 单测覆盖重叠与 `shadowed_by` |
| `DUAL-12-07` | 命中时间戳记录 | `parity-ready` | `RuleHitRecord::last_hit_secs` → `RuleHitSummary::last_hit_secs` → Iced `rule_hit_last_hit` 行与 Bevy `RuleItem.last_hit_secs`，无事实时诚实空态 |
| `DUAL-12-11` | 一键清空规则命中计数 | `parity-ready` | `CommandIntent::ResetRuleHitCounters` 从 unsupported 改为真实调用共享 `clear_hits`；Iced `Message::ClearRuleHitCounters` 经 `HostRuntime::rule_tracer_port`，无 port 时 typed error toast；Bevy `UiCommand::ClearRuleHitCounters` 经 `CommandApplication::with_rule_tracer` 路由 |
| `DUAL-12-12` | 规则命中高亮闪烁动效 | `parity-ready`（数据面） | 审计快照 `last_hit_rule`/`last_hit_secs` 提供唯一「刚命中」事实；Iced 卡片 HIT 徽标 + 规则名高亮、Bevy 行标签。像素动效属 `local`，宿主视觉 smoke 未计入 |
| `DUAL-12-09` | 规则时延贡献审计 | `parity-ready` | `RuleTracerApplication` 记录每次 AST 推演的匹配耗时（count/total/last），审计快照发布 `trace_count`/`avg_match_latency_us`/`last_match_latency_us`；Iced「平均匹配耗时」指标、Bevy 头部行「匹配 n.nµs」；application 单测覆盖两次推演求均值 |
| `DUAL-12-10` | 仿真沙盒环境参数模拟（来源 IP） | `parity-ready` | 共享 `TrafficContextSnapshot` 新增 serde 默认字段 `src_ip`/`src_port`/`in_port`（contract `rule_tracer.rs`）；`RuleTracerApplication` 拥有唯一 `context`，`set_context`/`context` + `merged_context` 同时并入端口 `trace` 与 reader `project` 并发布 `simulated_context`；Iced 来源 IP 输入 `Message::UpdateTracerSourceIp` 经 `RuleTracerPort::set_context` 后重跑；Bevy `TracerSourceIpField`/`TracerQueryField` + `SimulateRuleTraceButton` 观察者提交 `UiCommand::SetRuleTracerContext`（`CommandIntent::SetRuleTracerContext` 路由到共享 `set_context`）；application（Inbound 阶段 + `SRC-IP-CIDR` 命中）、Iced、Bevy 三处无头断言 |
| `DUAL-12-08` | 分流结果一键反向应用（修改此规则出站） | `parity-ready` | 共享 `TracerRuleOverride`/`TracerRuleOverrideResult`（contract `rule_tracer.rs`）+ `RuleOverridePort` 持久化端口；`RuleTracerApplication::apply_override` 唯一实现「加载规则 → 重写命中规则出站 → 经 CORE-004 原子事务应用（reload+rollback）」，`can_reverse_apply`/`suggested_override_target` 由共享决策链派生且 `demo_fixture` 不再独占；`CommandIntent::ApplyTracerRuleOverride` 经 `CommandApplication` 路由到共享 handler，桌面 `DesktopRuleOverridePort` 于 bootstrap 注入；Iced 结果卡按 `can_reverse_apply` 门控、PROXY/DIRECT/REJECT 快捷 + 自定义目标输入，消费 `TracerRuleOverrideResult` 后本地重写并重跑追踪，无端口时诚实错误 toast；Bevy `ApplyTracerRuleOverrideButton`/`TracerOverrideTargetField` 观察者读共享投影提交 `UiCommand::ApplyTracerRuleOverride`；contract/domain/application（no-port unsupported、stale、apply-failed 诚实路径）、Iced、Bevy 三处无头断言 |

> **关键修复（D-017 收敛）**：reader 的 Rule Tracer 投影从硬编码空 `ready(..)` 改为 `RuleTracerApplication::project(core, rules, active_exit, proxies)` 真实推演；domain 出口阶段删除「香港专线 01 / 28ms / HK」伪造兜底，无运行时出口事实时渲染中性「未知出口」节点；Iced 删除本地 `(usize, String, String)` 三元组第二事实源。

### 组 07 逐项账目（2026-09-22 展开）

闭环口径同组 06/12 = shared contract/application + Iced + Bevy + 双端无头测试 + 宿主证据。
判定保守：仅当四层全部存在才记 `parity-ready`；只有共享/单端后端时记 `shared-ready`；仅有字段或未接线记 `planned`。

| 项 | 任务 | 状态 | 证据 |
| :--- | :--- | :--- | :--- |
| `DUAL-07-01` | 多渠道导入三合一（URL/本地/剪贴板） | `parity-ready` | 新增宿主端口 `SubscriptionImportPort`（`ports/subscription_import.rs`，本地文件 + 剪贴板；无剪贴板宿主按 typed unsupported 诚实失败）+ 共享 `SubscriptionImportApplication`（`subscription_import_application.rs`）统一三通道分发；application `ProfileApplication::import_subscription_report`/`import_document`（`profile_application.rs`），contract `SubscriptionImportReport` + `CommandIntent::ImportSubscription`（`command.rs`）经 `CommandApplication`（`with_import_source`）路由；桌面 `DesktopSubscriptionImportPort`（`desktop/subscription_import_port.rs`）于 composition 注入（`with_import_source`），Android/iOS 未组合 → typed unsupported；Iced 本地导入改走 `host::storage::subscription_import_port()`（`update/profile/import.rs`），Bevy 多渠道导入面板 `ImportSubscriptionUrlButton`/`ImportLocalSubscriptionButton`/`ImportClipboardSubscriptionButton` + 观察者提交共享命令（`pages/profiles_import_channels.rs`）；测试 application `import_document_validates_and_reports_format` 与端口三测（含 unsupported）、Iced `local_import_reads_through_the_host_import_port`、Bevy `test_profiles_import_channels_submit_shared_command` |
| `DUAL-07-02` | 自定义单个订阅 User-Agent | `parity-ready` | 共享 `ProfileApplication::update_subscription_conditional`（`profile_application.rs:179`）从 `ProfileMetadata.user_agent` 组装 `ConditionalFetchHeaders.custom_user_agent`（`ports/subscription_source.rs:16`），core `subscription_io.rs:66` 写入 `User-Agent` 头；Iced 编辑器加载/保存（`update/profile/subscription.rs:21`、`:84`，此前 `UpdateSubscriptionUserAgent` 是死消息）+ 输入框 `view/profiles.rs:738`；Bevy `SubscriptionUserAgentField` + `SaveUserAgentButton` 观察者提交 `UiCommand::SaveSubscriptionFetchSettings`（`pages/profiles_import.rs:46`、`:421`）；application 测试 `conditional_update_sends_stored_validators_and_persists_new_etag`、Iced `subscription_fetch_options_load_and_not_modified_feedback`、Bevy `test_profiles_save_fetch_settings_submits_shared_command` |
| `DUAL-07-03` | 定时自动轮询与 Cron 表达式 | `parity-ready` | domain `CronSchedule`/`SubscriptionSchedule::from_metadata`（`subscription_scheduler_policy.rs`）；application `schedule_next_update` 改用共享排程（`profile_application.rs`），cron-only profile 成功更新后推进到下一次 Cron 命中；admin `sync_profile_job` 接受 `cron_expression`，`schedule_cron_profile_update_job` 经新增 `JobScheduler::spawn_dynamic_job`（每次运行后按 `cron_delay` 重算间隔，避免月/周级漂移）排程纯 Cron profile（`scheduler.rs`、`scheduler/job_scheduler.rs`），`run_profile_subscription_tick` 不再要求 `update_interval_hours>0`（`scheduler/subscription.rs`）；admin payload 新增 `cron_expression` 并先解析校验（`admin_api/models.rs`、`handlers/profiles.rs`）；`ProfileSnapshot.cron_expression` 投影，Iced 订阅编辑器新增 Cron 输入（`UpdateSubscriptionCron` + `view/profiles.rs` + 中英词条）并校验，Bevy 导入卡 `SubscriptionScheduleStatus` 展示排程；测试 admin `test_cron_only_profile_is_scheduled_and_cancelled`、job_scheduler `dynamic_job_waits_then_recomputes_its_delay`、application `successful_cron_update_advances_to_the_next_occurrence`、Iced `subscription_cron_editor_loads_and_validates`、Bevy `test_profiles_filter_panel_restamps_and_submits_shared_command`（含 cron 展示断言） |
| `DUAL-07-04` | 条件请求 ETag / If-Modified-Since | `parity-ready` | port `ConditionalFetchHeaders`/`fetch_conditional`（`ports/subscription_source.rs:16`、`:54`）；core 304 处理并回读 `etag`/`last-modified`（`core/subscription_io.rs:66`）；application 仅 `200` 落盘、`304` 只刷新配额与排程（`profile_application.rs:179`、`:465`），validator 持久化进 `ProfileMetadata`；表面快照新增 `etag`/`last_modified`（`contract/surface_snapshot.rs:189`）经 reader（`application/surface_reader.rs:396`）发布；Iced 展示缓存状态并给出 304 toast（`view/profiles.rs`、`update/profile/subscription.rs:540`），Bevy 状态行由 `sync_subscription_fetch_controls`（`pages/profiles_import.rs:370`）重盖；双端测试见 07-15 |
| `DUAL-07-05` | 网络重试与指数退避（30s/1m/5m） | `parity-ready` | domain `RetryBackoffPolicy`（`subscription_scheduler_policy.rs:286`，含 30/60/300 值测试）；共享 `SubscriptionRefreshApplication::refresh_profile`（`subscription_refresh_application.rs`）按策略重试，退避经注入的 `ApplicationRuntime::sleep` 端口等待，application 层不引 Tokio；Bevy 命令路径经 `CommandApplication::with_application_runtime` 复用同一重试路径（`command_application.rs`），Iced 手动更新与批量改走共享刷新（`update/profile/subscription.rs`）+ `host::runtime::application_runtime()`（`host.rs`）；宿主 composition 注入运行时端口（`composition/lib.rs`、`desktop/composition.rs`、`android/composition.rs`）并在桌面宿主把 profile store + subscription source 组合进共享命令路径（`desktop/composition.rs` 的 `with_profile`/`with_subscription_source`，`runtime.rs`/`boot.rs` 传入）；application 测试 `retries_transient_failures_then_succeeds`、`backoff_delays_ride_the_injected_runtime_sleep_seam`、`gives_up_when_the_policy_is_exhausted`，`CommandApplication` 路径测试 `command_application_update_profile_uses_the_shared_retry_seam`，Iced `subscription_refresh_retries_and_single_flights_through_shared_application`，Bevy `test_profiles_update_button_submits_shared_command` |
| `DUAL-07-06` | 单飞防重入调度 (Single Flight) | `parity-ready` | 共享进程级注册表 `inflight_refreshes()`（键=`(config_dir, profile)`，`subscription_refresh_application.rs`），`begin_refresh` 返回 RAII `SubscriptionRefreshGuard`，并发重入返回 typed `InvalidState` 而非重复下载；`refresh_profile`/`refresh_all` 全程持锁，两个表面共用同一守卫；admin `JobScheduler` 每作业串行（`scheduler/job_scheduler.rs:108`）保留为宿主定时侧实现；application 测试 `refresh_is_single_flight_per_store_and_profile`、Iced `subscription_refresh_retries_and_single_flights_through_shared_application`、Bevy `test_profiles_update_button_submits_shared_command` |
| `DUAL-07-07` | 订阅用量与到期三级预警 | `parity-ready` | domain `QuotaWarningPolicy`（`subscription_scheduler_policy.rs:332`，85%/3天阈值 + 测试）；application 更新报告写入 `usage_warning`/`expiry_warning`（`profile_application.rs:480`）；共享 `SubscriptionQuotaSnapshot` 经 `subscription_quota.rs` 三级预警，Iced Overview 配额卡与 Bevy `SubscriptionQuotaCard`/`subscription_quota_scene`（`pages/overview_cards.rs:337`）同源渲染（另见 `DUAL-03-06`） |
| `DUAL-07-08` | 订阅节点关键词清洗管道 | `parity-ready` | domain `FilterPipeline`/`FilterStage`（`filter.rs`、`filter_pipeline.rs`）与 `FilterSpec` 草稿互转 `filter_spec_from_draft`/`filter_spec_to_draft`（`profile_options.rs`）；共享 `ProfileApplication::apply_subscription_filter` 跑 `SubscriptionFilterPipeline` 并在运行时 seam 落盘后持久化 spec（`profile_application.rs`），`load_options`/`save_options` 经扩展后的 `ProfileStore` 端口（`ports/profile_store.rs`、`mihomo-config/profile_store.rs`）；contract `SubscriptionFilterDraft` + `CommandIntent::SaveSubscriptionFilter` 经 `CommandApplication` 路由；Iced 过滤面板改走共享 runner（`update/profile/options.rs`）；Bevy 新增清洗管道面板（包含/排除/协议/重命名/去重 + `SaveSubscriptionFilterButton`，`pages/profiles_import_channels.rs`）经共享命令提交，`ProfileSnapshot.filter` 投影预填；测试 application `apply_subscription_filter_reshapes_document_and_persists_spec`、Iced `subscription_filter_panel_rides_the_shared_pipeline`、Bevy `test_profiles_filter_panel_restamps_and_submits_shared_command` |
| `DUAL-07-09` | 更新后自动重启核心可选 | `parity-ready` | 共享消费：新增宿主端口 `CoreReloadPort`（`ports/core_reload.rs`，把 `ManagedRuntime::apply_current_config(PreferReload)` 收窄为唯一 reload seam）与 `SubscriptionRefreshApplication::with_core_reload`：刷新提交新内容后按「持久化偏好 → 生效配置判定 → 宿主 seam」产出 typed `CoreReloadOutcome`（`Reloaded`/`Disabled`/`NotActive`/`Unsupported`/`Failed{error}`），`SubscriptionUpdateReport.reloaded_core` 布尔被 `core_reload` 取代（`reloaded_core()` 为派生读法）；批量刷新至多重载一次且只盖在生效配置的报告上。无 seam 宿主 typed unsupported、绝不静默 no-op：`CommandIntent::SetSubscriptionAutoReload` 在 `managed_runtime` 缺失时对 enable 返回 `Failure::unsupported`，Iced 在无运行时 seam 时同步拒绝 enable（persisted 状态仍可保存/关闭）。持久化沿用 `ProfileMetadata.auto_reload_core`（`mihomo-config/manager/metadata.rs:93`、`manager/profiles.rs:229`），经 `ProfileSnapshot.auto_reload_core`（`contract/surface_snapshot.rs`）+ reader 投影到双端；Iced 订阅编辑器新增开关（`UpdateSubscriptionAutoReload` + 中英词条 `profiles_auto_reload_core`/`sub_reload_core_unsupported`/`sub_reload_core_failed`），手动/定时/批量更新路径全部改走共享刷新（`update/profile/subscription.rs` 不再出现无条件 `AlwaysRestart`），Bevy 新增 `SubscriptionAutoReloadToggle`/`SaveSubscriptionAutoReloadButton` → `UiCommand::SetSubscriptionAutoReload` → 共享 intent（`pages/profiles_subscription_policy.rs`）。测试 application `core_reload_follows_the_preference_and_the_host_seam`、`batch_reloads_the_core_once_for_the_active_profile`、`auto_reload_preference_is_persisted_and_readable`、`auto_reload_enable_requires_a_managed_runtime_seam`，Iced `subscription_policy_and_auto_reload_are_shared_application_wired`、`subscription_toast_reports_the_reload_outcome_honestly`，Bevy `test_profiles_auto_reload_toggle_submits_shared_command` |
| `DUAL-07-10` | 订阅更新静默系统通知 | `parity-ready` | 共享 `SubscriptionNotificationPort` + 语言无关 `SubscriptionNotification`（`ports/subscription_notification.rs`），`SubscriptionRefreshApplication::with_notifier` 在单条与批量刷新终态各发一条通知（`subscription_refresh_application.rs` 的 `notification_for_refresh`/`notification_for_batch`），`CommandApplication::with_subscription_notifier` 让 Bevy 命令路径同样触达（`command_application.rs`）；桌面 `DesktopSubscriptionNotificationPort` 复用既有 `infiltrator_desktop::notify::SystemNotifier`（`desktop/subscription_notification_port.rs`）并在 composition 注入（`with_subscription_notifier`），Iced 手动/批量刷新经 `host::runtime::subscription_notifier()` 走同一端口（`update/profile/subscription.rs`），自动更新保留既有 `system_notify` Low/Critical；测试 application `refresh_profile_notifies_success_through_the_host_port`、`exhausted_refresh_notifies_failure_through_the_host_port`、`refresh_all_emits_one_aggregated_notification`、端口单测 |
| `DUAL-07-11` | 一键手动更新全部订阅 | `parity-ready` | 共享 `ProfileApplication::update_all_subscriptions`（`profile_application.rs`，`futures_util::stream::buffer_unordered` 限流并发，汇总 `SubscriptionBatchReport`）；contract `SubscriptionBatchReport`（`subscription_import.rs:128`）+ `CommandIntent::UpdateAllSubscriptions`（`command.rs`）经 `CommandApplication`（`BATCH_UPDATE_CONCURRENCY=5`）；Iced 托盘 + Profiles 工具栏同走共享批次（`update/profile/subscription.rs` 的 `update_all_subscriptions`/`batch_update_toast`、`view/profiles.rs` `profiles_update_all`），飞行中单飞防重入；Bevy `UpdateAllSubscriptionsButton` → `UiCommand::UpdateAllSubscriptions` → 共享 intent（`pages/profiles.rs`、`command.rs`）；测试 application `batch_update_skips_url_less_profiles_and_aggregates_counts`、Iced `subscription_batch_update_and_backup_restore_are_shared_application_wired`、Bevy `test_profiles_update_all_toolbar_submits_shared_command`；admin `update_all_subscriptions`（`scheduler/subscription.rs:110`）保留为宿主定时侧的独立实现 |
| `DUAL-07-12` | 安全证书跳过 (Insecure Skip Verify) | `parity-ready` | `ProfileMetadata.insecure_skip_verify` → `ConditionalFetchHeaders.insecure_skip_verify`（`ports/subscription_source.rs:16`）；core 在命中该标记时按请求构建 `danger_accept_invalid_certs` 客户端（`infiltrator-http/src/lib.rs:39`、`core/subscription_io.rs:77`），默认客户端绝不退让；未覆写 `fetch_conditional` 的适配器在 `insecure_skip_verify` 时按 typed unsupported 拒绝而非静默忽略（`ports/subscription_source.rs`）；`ProfileApplication::update_subscription_fetch_settings`（`profile_application.rs:279`）持久化；Iced 开关 `view/profiles.rs` + `update/profile/subscription.rs:88`；Bevy `SubscriptionInsecureToggle` 经 `SaveSubscriptionFetchSettings` 命令保存；application/ports/双端测试覆盖 |
| `DUAL-07-13` | 配置源文件安全备份 | `parity-ready` | `ConfigManager::save` 写前原子生成 `.bak`、`restore_backup` 校验后恢复、`clear_backup` 清理（`mihomo-config/manager/profiles.rs:40/57/73`）；`list_profiles` 投影 `has_backup`（`manager/profiles.rs`）经 `ProfileStore`（`profile_store.rs`）→ `ProfileInfo.has_backup`（`domain/profiles.rs`）→ `ProfileSnapshot.has_backup`（`contract/surface_snapshot.rs`）→ reader（`surface_reader.rs`）；application `ProfileApplication::restore_backup`；`CommandIntent::RestoreSubscriptionBackup` 经 `CommandApplication`；Iced 订阅面板备份状态 + 还原按钮（`view/profiles.rs`）+ handler（`update/profile/subscription.rs`）；Bevy `SubscriptionBackupStatus`/`RestoreSubscriptionBackupButton` 经共享命令（`pages/profiles_import.rs`、`command.rs`）；测试 application `restore_backup_reports_availability_and_is_one_shot`、Iced `subscription_batch_update_and_backup_restore_are_shared_application_wired`、Bevy `test_profiles_restore_backup_submits_shared_command_and_restamps_status` |
| `DUAL-07-14` | 双端订阅管理交互 1:1 对等 | `parity-ready` | 共享新增 `SubscriptionScheduleDraft` + `ProfileApplication::update_subscription_schedule`（cron 解析、auto-update 必须有 URL、正周期校验后一次性持久化；空 URL 清空订阅与排程）与 `CommandIntent::UpdateSubscriptionSchedule`（`command_application` 路由 + `command_name` 映射）；`ProfileSnapshot` 新增 `auto_update_enabled`/`update_interval_hours`/`next_update`（+ 07-09 的 `auto_reload_core`）并由 reader 投影（页级 `auto_update_interval_hours` 改为「已启用 profile 的最短固定周期」，不再是恒 0）。Iced：订阅设置保存不再在 surface 内组装 metadata，改走 `update_subscription_schedule`/`update_subscription_auto_reload`/`update_subscription_fetch_settings` 三个共享方法（`update/profile/subscription.rs`）。Bevy Profiles 页新增「订阅更新策略」卡片 `pages/profiles_subscription_policy.rs`（URL/自动更新/周期/Cron/内核重载控件全部从共享投影预填并在 `sync_subscription_policy_controls` 内原地重盖，`SaveSubscriptionPolicyButton` → 共享 schedule intent），卡片新增删除动作 `DeleteProfileButton` → `UiCommand::DeleteProfile`（生效配置不提供删除，与 Iced 卡片一致）与真实排程行 `ProfileScheduleText`（Cron/周期/手动），页头自动更新摘要改为按 profile 列表派生（`auto_update_summary`，此前 reader 恒 0 导致 Bevy 恒显示「每 0 小时」）。测试 application `schedule_draft_validates_and_persists_the_subscription_shape`、`subscription_schedule_command_persists_and_rejects_invalid_cron`，Iced `subscription_policy_and_auto_reload_are_shared_application_wired`，Bevy `test_profiles_schedule_policy_restamps_and_submits_shared_command`、`test_profiles_delete_button_submits_shared_command` |
| `DUAL-07-15` | 订阅更新流水线无头测试 | `parity-ready` | domain `subscription_scheduler_policy`（Cron/退避/阈值/格式/节点数）、admin `subscription_test.rs`（更新/并发/排程/重启重定向 + 纯 Cron 排程）；本轮新增共享无头回归矩阵 `subscription_update_pipeline_regression_matrix`（`subscription_refresh_application_test.rs`）覆盖本批收口的 07-01/03/08/10 及 07-05/06 依赖阶段；application 条件请求三测、导入三测、Iced `subscription_fetch_options_load_and_not_modified_feedback`/`subscription_filter_panel_rides_the_shared_pipeline`/`subscription_cron_editor_loads_and_validates`/`local_import_reads_through_the_host_import_port`、Bevy fetch/filter/import 命令测试 |

> **2026-09-22 组 07 批次 A**：`DUAL-07-02/04/12` 收口为 `parity-ready`。
> 新增共享条件更新路径 `ProfileApplication::update_subscription_conditional` 与
> `update_subscription_fetch_settings`：从 `ProfileMetadata` 组装 ETag /
> If-Modified-Since / 自定义 User-Agent / insecure-TLS，`200` 才落盘、`304`
> 只刷新配额与排程，并返回共享 `SubscriptionUpdateReport`（`backed_up`/
> `usage_warning`/`expiry_warning`）。`infiltrator-http` 新增
> `build_insecure_http_client`，core 仅在该 profile 打开开关时为单次请求
> 构建跳过证书校验的客户端（默认客户端不退让）。`ProfileSnapshot` 新增
> `user_agent`/`insecure_skip_verify`/`etag`/`last_modified` 并经 reader 发布，
> Iced 编辑器（修复了此前从未处理的 `UpdateSubscriptionUserAgent` 死消息）与
> Bevy fetch-选项卡片（`SubscriptionUserAgentField`/`SubscriptionInsecureToggle`
> + `SaveSubscriptionFetchSettings` 命令）同源消费。守卫
> `subscription-lifecycle-guard.py` 固化本组账目与关键标记。
>
> **2026-09-22 组 07 批次 B**：`DUAL-07-11/13` 收口为 `parity-ready`。
> 新增共享 `ProfileApplication::update_all_subscriptions`：对带订阅 URL 的
> profile 走 `futures_util::stream::buffer_unordered` 限流并发，忽略排程立即
> 刷新，并汇总为既有 contract `SubscriptionBatchReport`（`updated`/
> `not_modified`/`failed`/`skipped`）。Iced 托盘与 Profiles 工具栏
> （`profiles_update_all`）改走同一批次并在飞行中单飞防重入，Bevy 新增
> `UpdateAllSubscriptionsButton` → `UiCommand::UpdateAllSubscriptions` 映射到
> 共享 `CommandIntent::UpdateAllSubscriptions`。安全备份面新增
> `ProfileInfo.has_backup` → `ProfileSnapshot.has_backup` 投影，application
> 暴露 `restore_backup`，Iced 订阅面板与 Bevy fetch-选项卡片同源展示并可一键
> 还原上次写入前的 `.bak`（`restore_backup` 仍由 store 校验后恢复，缺失时诚实
> 返回 `false` 而非伪造成功）。守卫追加 07-11/07-13 的共享 + 双端 + 双端测试
> 标记。
>
> **2026-09-22 组 07 批次 C**：`DUAL-07-05/06` 收口为 `parity-ready`。
> 新增共享 `SubscriptionRefreshApplication`（`subscription_refresh_application.rs`）
> 把「按 `RetryBackoffPolicy` 指数退避重试」与「按 `(config_dir, profile)`
> 进程级单飞」放进 application 层：退避延迟只经注入的
> `ApplicationRuntime::sleep` 端口等待（application 仍不引 Tokio），单飞用
> RAII `SubscriptionRefreshGuard`，并发重入诚实返回 typed `InvalidState`
> 而非重复下载。Bevy 命令路径经 `CommandApplication::with_application_runtime`
> 复用同一刷新（`UpdateProfile`/`UpdateAllSubscriptions`），Iced 手动更新与
> 批量改走 `refresh_profile`/`refresh_all` 并注入
> `host::runtime::application_runtime()`，桌面宿主 composition 把 profile store
> 与 subscription source 一并组合进共享命令路径（`with_profile`/
> `with_subscription_source`），三处宿主 composition 安装该运行时端口。Bevy
> Profiles 卡片新增 `UpdateProfileButton` → `UiCommand::UpdateProfile`
> 映射。守卫追加 07-05/07-06 的共享 + 双端 + 双端测试标记。
>
> **2026-09-22 组 07 批次 D**：`DUAL-07-01/03/08/10/15` 收口为 `parity-ready`。
> 新增宿主端口 `SubscriptionImportPort`（本地文件/剪贴板，无能力宿主 typed
> unsupported）与共享 `SubscriptionImportApplication`，三通道导入同时接入
> Iced（`host::storage::subscription_import_port`）与 Bevy 多渠道导入面板；
> 共享 `ProfileApplication::apply_subscription_filter` + 扩展后的
> `ProfileStore::load_options/save_options` 把节点清洗管道下沉，Bevy 新增
> 包含/排除/协议/重命名/去重面板，Iced 过滤编辑器改走同一 runner；排程侧
> `schedule_next_update` 支持 Cron，admin `sync_profile_job` 经新增
> `JobScheduler::spawn_dynamic_job` 支持纯 Cron profile（每次运行后重算间隔，
> 无月/周级漂移），`ProfileSnapshot.cron_expression` 投影到双端（Iced 新增
> Cron 输入与中英词条，Bevy 展示排程状态）；通知侧新增语言无关
> `SubscriptionNotificationPort`，共享刷新在单条/批量终态各发一条，桌面
> `DesktopSubscriptionNotificationPort` 复用既有 `SystemNotifier`，双端命令
> 路径共用。新增共享无头回归矩阵 `subscription_update_pipeline_regression_matrix`
> 覆盖本批收口项与依赖阶段。守卫追加 07-01/03/08/10/15 的共享 + 双端 + 双端
> 测试标记。
>
> **2026-09-22 组 07 批次 E（收官）**：`DUAL-07-09/14` 收口为
> `parity-ready`，组 07 达到 **15/15**。
> 09：新增宿主端口 `CoreReloadPort`（仅收窄 `ManagedRuntime::
> apply_current_config(PreferReload)`），`SubscriptionRefreshApplication::
> with_core_reload` 在依赖同一 `ProfileApplication` 事实的前提下把
> 「持久化偏好 + 生效配置 + 宿主 seam」三者合成 typed `CoreReloadOutcome`
> 写进 `SubscriptionUpdateReport.core_reload`（`reloaded_core` 布尔降级为派生
> 读法）；批量刷新只重载一次并只盖在生效 profile 的报告上；`CommandIntent::
> SetSubscriptionAutoReload` 在宿主无 `managed_runtime` 时对 enable 返回
> typed unsupported，Iced 无运行时 seam 时同步拒绝 enable（关闭始终允许），
> 刷新路径无 seam 时产出 `Unsupported` 而非静默 no-op。Iced 手动/定时/批量
> 三条更新路径全部改走共享刷新，删除了无条件的 `AlwaysRestart`；双端新增
> 内核重载开关并消费 `ProfileSnapshot.auto_reload_core`。
> 14：新增 `SubscriptionScheduleDraft` 与 `update_subscription_schedule`
> 共享草稿（cron/URL/周期校验 + 空 URL 清空），Bevy 新增「订阅更新策略」卡片
> （URL/自动更新/周期/Cron/内核重载，全部从共享投影预填并原地重盖）、卡片删除
> 动作与真实排程行，页头自动更新摘要改由 profile 列表派生（此前 reader 恒 0、
> 页头恒显示「每 0 小时」是唯一的 demo 残留）；Iced 保存路径删除了 surface 内
> metadata 组装，改走共享 application 方法。守卫追加 07-09/14 的共享 + 双端 +
> 双端测试标记，并反向禁止 Iced 订阅路径再出现 `AlwaysRestart` 与 surface 内
> metadata 组装。
>
> **已知偏差（诚实记录）**：仓库内桌面组合根（`infiltrator-desktop::
> composition::core_application`）尚未安装 `CommandApplication::
> with_managed_runtime`，因此 Bevy 桌面宿主当前对「启用内核重载」返回 typed
> unsupported（Iced 桌面宿主持有 `Arc<dyn HostRuntime>`，功能完整可用）。这与
> 07-09 的 typed unsupported 口径一致，不是静默 no-op；待桌面 Bevy 组合根接入
> `with_managed_runtime` 后该开关即可工作，共享层与双端 UI 无需改动。

---

### 组 08 逐项账目（2026-09-22 展开）

闭环口径同组 06/07/12 = shared contract/application + Iced + Bevy + 双端无头测试 + 宿主证据。
判定保守：仅当四层全部存在才记 `parity-ready`；只有共享/单端后端时记 `shared-ready`；仅有字段、未接线或未持久化记 `planned`。
本组此前的唯一共享后端是 domain 的 `MultiSubscriptionAggregator::aggregate`（只返回 YAML 字符串，无结构化事实、无 application、无宿主端口），Iced 模态的「执行合并」只 `format!` 一个计数串（伪数据），Bevy 页面的区域分组是硬编码 demo 列表。

| 项 | 任务 | 状态 | 证据 |
| :--- | :--- | :--- | :--- |
| `DUAL-08-01` | 多订阅源勾选聚合向导 | `parity-ready` | 共享 `AggregationDraft.source_profiles`（`contract/aggregator.rs:92`）+ `ProfileAggregationApplication::preview`（`application/profile_aggregation_application.rs:102`）按勾选真实读取每个源 profile 内容；Iced 源勾选列表（`view_root/aggregator_modal.rs:158/230`）由真实 `state.profile.profiles` 派生，`update/aggregator.rs:62` 维护选择；Bevy 源勾选清单 `AggregatorSourceToggle`（`pages/profiles_aggregator.rs:53/303`）；测试 Iced `test_advancement_w2_3_multi_profile_aggregator_workflow`、Bevy `test_profiles_aggregator_previews_and_saves_through_shared_command` |
| `DUAL-08-02` | 跨订阅节点自动去重（Server+Port+Protocol 指纹） | `parity-ready` | domain `ProfileAggregator::pipeline` 注入 `FilterStage::content_deduplicator`（`domain/profile_aggregator.rs:171`）复用既有 `compute_node_fingerprint`（type+server+port+凭据，`domain/filter.rs:626`），`aggregate()` 改为委托 `plan()` 单一实现（`domain/profile_converter.rs:547`）；application `aggregation_options` 把 `draft.deduplicate` 映射为 `KeepFirst`（`application/profile_aggregation_application.rs:63/65`），报告发布真实 `duplicates_removed`（`contract/aggregator.rs:176`）；Iced 开关 `state.rs:229` + `update/aggregator.rs:95` + 预览计数；Bevy `AggregatorDeduplicateToggle` + 计数行；测试 domain `plan_reports_real_dedup_and_region_counts`、application `preview_reports_real_dedup_clusters_and_master_cascade`、Iced `test_advancement_w2_3_multi_profile_aggregator_workflow`、Bevy 断言「去重 2」 |
| `DUAL-08-03` | 区域节点自动归类（ISO 国家代码） | `parity-ready` | domain `ProfileAggregator::cluster_regions`（`domain/profile_aggregator.rs:321`）复用 `extract_country_code`（`domain/filter.rs:534`）与 `COUNTRY_DEFS`，`region_label` 从别名表取本地化标签（`domain/profile_aggregator.rs:332`）；contract `RegionalClusterSnapshot { iso, label, flag, group_name, node_names }`（`contract/aggregator.rs:132`）经 reader 发布（`application/surface_reader.rs:412/473`）；Iced 区域行（`view_root/aggregator_modal.rs:559`）、Bevy 区域块 `aggregation_regions`（`pages/profiles_aggregator.rs:175`）；测试 domain `plan_reports_real_dedup_and_region_counts`、Bevy 断言「🇭🇰 HK 香港 → 香港自动测速（2 节点）」 |
| `DUAL-08-04` | 自动生成区域测速策略组 | `parity-ready` | domain `ProfileAggregator::synthesize_groups`（`domain/profile_aggregator.rs:342`）为每个区域生成 `url-test`（`{label}自动测速`，探测 `AGGREGATION_HEALTH_CHECK_URL`、interval 300、tolerance 50，`RegionalCluster::group_name`）；contract `GeneratedGroupSnapshot`（`contract/aggregator.rs:148`）；YAML 渲染写入 `proxy-groups`（`domain/profile_aggregator.rs` `render_yaml`）；Iced 策略组行（`view_root/aggregator_modal.rs:599`）、Bevy 策略组块（`pages/profiles_aggregator.rs:200`）；测试 domain `plan_generates_master_cascade_before_concrete_nodes`、application `aggregation_pipeline_regression_matrix`、Bevy 断言「香港自动测速」 |
| `DUAL-08-05` | 主选择器自动级联（Master Cascade） | `parity-ready` | domain `MASTER_SELECT_GROUP` 先列 `♻️ 自动选择` + `🎯 全球直连` + 全部区域组，再列具体节点（`domain/profile_aggregator.rs:351`），`GeneratedGroup.is_master` 标记唯一主选择器；contract `AggregationReport::master_group`（`contract/aggregator.rs:209`）；Iced 预览用 `BadgeKind::Accent` 标「主选择器级联」，Bevy 计数行输出「主选择器级联 N 项」并给主组加 `[主选择器]`；测试 domain `plan_generates_master_cascade_before_concrete_nodes`（断言区域组位置早于具体节点）、application `preview_reports_real_dedup_clusters_and_master_cascade`、Bevy 断言「🚀 节点选择 [主选择器]」 |
| `DUAL-08-06` | 聚合后生成新独立 Profile | `parity-ready` | application `ProfileAggregationApplication::create_profile`（`application/profile_aggregation_application.rs:150`）先 `sanitize_profile_name` + 拒绝覆盖既有 profile，再 `preview` 后 `ProfileApplication::save_profile` 落盘，源 profile 不被修改；`CommandIntent::CreateAggregatedProfile`（`contract/command.rs:242`）经 `CommandApplication` 路由（`application/command_application.rs:470`）；Iced `Message::CreateAggregatedProfile`（`update/aggregator.rs:325`）与保存按钮；Bevy `SaveAggregatedProfileButton` + 观察者（`pages/profiles_aggregator_wizard.rs:216`）→ `UiCommand::CreateAggregatedProfile`（`bevy-ui/command.rs:120`）；测试 application `create_profile_saves_a_new_independent_profile`、`create_profile_refuses_to_overwrite_an_existing_profile`、`aggregation_pipeline_regression_matrix`、Bevy 保存命令断言 |
| `DUAL-08-07` | 一键保持源订阅联动更新 | `parity-ready` | `AggregationTemplate`（`contract/aggregator.rs`）把草稿与 `target_name` 持久化，`ProfileAggregationApplication::reaggregate`（`application/profile_aggregation_application.rs`）重读当前源内容、重跑共享聚合并原地覆写该模板产出的 profile；`create_profile_with_runtime` 落盘后自动把草稿存为同名模板（源订阅与聚合 profile 的来源关系由此持久化）。Iced 模板行「重新聚合」按钮 + `Message::ReAggregateProfile`（`view_root/aggregator_modal.rs`/`update/aggregator.rs`），Bevy `ReAggregateTemplateButton` → `UiCommand::ReAggregateProfile`（`pages/profiles_aggregator_wizard.rs`/`command.rs`），命令路径 `CommandIntent::ReAggregateProfile`（`command_application.rs`）；测试 application `reaggregate_refreshes_a_generated_profile_from_live_sources`、Bevy 断言 `UiCommand::ReAggregateProfile` |
| `DUAL-08-08` | 自定义节点重命名规则 | `parity-ready` | 共享 `AggregationRenameRule`（`contract/aggregator.rs:11`）+ 双端共用的 `parse_list`/`to_text` 文本语法；domain `ProfileAggregator::rename_pipeline`（`domain/profile_aggregator.rs`）在清洗前编译并执行规则（非法正则以 `rename rule #N` typed 失败），`AggregationPlan.rule_renamed_nodes` 发布真实计数；Iced 重命名输入 + 非法行 toast（`view_root/aggregator_modal.rs`/`update/aggregator.rs`），Bevy `AggregatorRenamesField` + 状态行（`profiles_aggregator_wizard.rs`）；测试 domain `plan_applies_regex_rename_rules_before_clustering`、application `preview_applies_precheck_and_rename_switches`、Iced 非法行断言、Bevy 非法行不提交命令 |
| `DUAL-08-09` | 节点可用性预检与过滤 | `parity-ready` | domain 新增 `proxy_nodes::validate::validate_item`（`domain/proxy_nodes/validate.rs`，覆盖降级为 `OtherNode` 的 ss/vmess/trojan/tuic 等必填凭据）并复用富模型 `validate`（同源 `proxies` 按索引配对）；`ProfileAggregator::precheck_nodes`（`domain/profile_aggregator.rs`）在合并后清洗前剔除非法节点，`AggregationPlan/Report.invalid_nodes_removed`+`invalid_node_samples`（上限 `MAX_PRECHECK_SAMPLES`，样本为真实问题首条）；Iced 开关 + 预览计数/样本（`update/aggregator.rs`/`aggregator_modal.rs`），Bevy `AggregatorSwitchKind::AvailabilityPrecheck` + 计数行；测试 domain `plan_precheck_drops_invalid_nodes_and_reports_samples`/`plan_precheck_failure_names_the_dropped_nodes`、proxy_nodes `validate_item_reports_missing_credentials_and_port`、application `preview_applies_precheck_and_rename_switches`、Bevy 断言「预检剔除 2」 |
| `DUAL-08-10` | 自定义新策略组拓扑编排 | `parity-ready` | 共享 `AggregationCustomGroup`（`contract/aggregator.rs`，name/group_type/member_keywords）；domain `ProfileAggregator::synthesize_groups` 追加自定义组（关键词大小写不敏感匹配节点名，空关键词=全部节点）并纳入主选择器级联，`GeneratedGroup.is_custom` 标记，`validate_custom_groups` 拒绝与主/自动/区域组重名及未知类型（`domain/profile_aggregator.rs`）；Iced 名称/关键词输入 + 追加/移除行（`aggregator_modal.rs`/`update/aggregator.rs`），Bevy `AggregatorComposerState` + 追加/清空按钮 + 组合行（`profiles_aggregator.rs`/`profiles_aggregator_wizard.rs`）；测试 domain `plan_synthesizes_custom_groups_with_keyword_members`、application `preview_synthesizes_custom_groups_from_the_draft`、Iced 追加/移除断言、Bevy 断言自定义组进入命令草稿与「[自定义]」预览 |
| `DUAL-08-11` | 聚合生成结果可视化预览 | `parity-ready` | 共享 `AggregationReport::yaml_preview(max_lines)`（`contract/aggregator.rs:198`）渲染真实 YAML 结构；Iced 预览区新增 YAML 视口（等宽 + 滚动 + 行数标题，`view_root/aggregator_modal.rs`），Bevy 新增 `aggregation_yaml_preview`（`pages/profiles_aggregator.rs`）与 `AggregatorPreviewKind::Yaml` 重盖系统；测试 Iced `aggregator_preview_yaml` 词条/渲染、Bevy 断言「聚合 YAML 结构（共 4 行）」 |
| `DUAL-08-12` | 一键设为当前活动配置 | `parity-ready` | 草稿新增 `activate_after_create`（`contract/aggregator.rs`），application `create_profile_with_runtime`/`reaggregate` 复用共享 `ProfileApplication::activate_profile`（切换+热载入+失败回滚；无 runtime seam 时走 `select_profile` 仅切换），`AggregatedProfileOutcome { activated, core_reloaded }` 诚实回传是否热载入；Iced 开关 + 结果 toast 三态（`update/aggregator.rs`，注入 `self.runtime.runtime`），Bevy `AggregatorSwitchKind::ActivateAfterCreate` + `CommandApplication::with_managed_runtime` seam；测试 application `create_profile_activates_through_the_shared_profile_path`（断言 store `current` 切换、`core_reloaded=false`）、Bevy 断言开关进入命令草稿 |
| `DUAL-08-13` | 历史聚合模板保存与复用 | `parity-ready` | 共享 `AggregationTemplate`（name/draft/updated_at）；`ProfileStore::load_aggregation_templates`/`save_aggregation_templates`（`ports/profile_store.rs`，缺省为 typed unsupported，`mihomo-config` 落 `<config-dir>/options/.aggregation-templates.yaml` 原子写，空库删文件）；application `list_templates`/`save_template`（upsert）/`delete_template`/`find_template` 与创建后自动记忆；reader 投影 `ProfilesPageSnapshot.aggregation_templates(_available)`（`surface_reader.rs`）；Iced 模板名称输入 + 保存/复用/删除 + 应用回填向导（`aggregator_modal.rs`/`update/aggregator.rs`），Bevy 模板名称字段 + 保存/复用/删除按钮 + 模板清单行（`profiles_aggregator_wizard.rs`）；测试 mihomo-config `aggregation_templates_persist_beside_the_profile_options`、application `templates_upsert_list_and_delete`/`create_profile_reports_missing_template_sidecar_without_failing`/`create_profile_saves_a_new_independent_profile`、Iced `Message::ApplyAggregatorTemplate` 回填断言、Bevy 断言模板清单与保存/复用命令 |
| `DUAL-08-14` | 双端聚合器模态 100% 对等 | `parity-ready` | Iced 模态（`view_root/aggregator_modal.rs`）与 Bevy 卡片（`pages/profiles_aggregator.rs:303`）消费同一 `AggregationDraft`/`AggregationReport`，提交同一 `CommandIntent::{Preview,Create}ProfileAggregation`；Bevy 观察者注册于 `pages/profiles.rs:571-579`；两端都只投影共享报告、不做本地去重/聚类/组生成；测试见 08-01/08-06/08-15 |
| `DUAL-08-15` | 聚合器引擎全链路行为测试 | `parity-ready` | domain `profile_aggregator_test.rs`（合并/去重/区域/主级联/空源 4 测）、application `profile_aggregation_application_test.rs`（9 测，含端到端矩阵 `aggregation_pipeline_regression_matrix`：多源合并→指纹去重→区域聚类→组生成→落盘，并断言源 profile 未被修改）、Iced `test_advancement_w2_3_multi_profile_aggregator_workflow`/`test_advancement_w2_3b_aggregator_preview_lifecycle_is_shared`、Bevy `test_profiles_aggregator_previews_and_saves_through_shared_command` |

> **2026-09-22 组 08 批次 A**：`DUAL-08-01/02/03/04/05/06/14/15` 收口为 `parity-ready`。
> 新增 domain 结构化聚合器 `ProfileAggregator::plan`（`profile_aggregator.rs`）：把原先只返回
> YAML 字符串的 `MultiSubscriptionAggregator::aggregate` 下沉为唯一实现，返回 `AggregationPlan`
> （输入/输出去重计数、区域聚类、策略组级联、渲染 YAML），`aggregate()` 变为 `plan().yaml` 的薄委托，
> 消除双实现漂移；区域组命名改为 `{本地化标签}自动测速`（如「香港自动测速」），主选择器
> `🚀 节点选择` 先级联各区域组再列具体节点。新增共享契约 `contract/aggregator.rs`
> （`AggregationDraft`/`AggregationReport`/`RegionalClusterSnapshot`/`GeneratedGroupSnapshot`）与
> `CommandIntent::{Preview,Create}ProfileAggregation`；新增 application
> `ProfileAggregationApplication`（`preview`/`plan_documents`/`create_profile`）与进程级预览缓存
> `last_aggregation_report()`，`ProfilesPageSnapshot.aggregation` 由 reader 投影到双端。Iced 模态删除
> 伪计数 `aggregator_result_summary`，改存共享 `aggregator_report` 并新增四个清洗开关、
> 真实预览区与「保存为新配置」动作（新消息 `PreviewProfileAggregation`/`AggregationPreviewFinished`/
> `CreateAggregatedProfile`/`AggregatedProfileCreated`，处理下沉到 `update/aggregator.rs`）；Bevy
> `profiles_aggregator.rs` 删除硬编码区域 demo 列表，改为投影驱动：源勾选、开关、计数/区域/组拓扑
> 全部来自共享报告，并新增预览/保存观察者。双端严格「无伪数据」：Bevy 卡片不再展示固定节点数。
> 新增守卫 `aggregator-guard.py` 固化本组账目与共享/双端标记。
>
> **已知偏差（诚实记录）**：08-11 只做到「拓扑树 + 计数」预览，YAML 结构视口两端都未渲染；
> 08-07/08-08/08-09/08-13 的联动、重命名、预检与模板持久化仍未接线。批次范围超出题目建议的
> 3～4 项（实际收口 8 项），原因是「共享聚合应用 + 双端投影」这一必要使能层同时补齐了
> 08-01/06/14/15，已逐项给出真实证据。
>
> **2026-09-22 组 08 批次 B（收官）**：`DUAL-08-07/08/09/10/11/12/13` 收口为 `parity-ready`，
> 组 08 达到 **15/15**。
> 共享侧：`AggregationDraft` 扩展 `rename_rules` / `custom_groups` / `availability_precheck` /
> `activate_after_create`，`AggregationReport` 新增 `rule_renamed_nodes` /
> `invalid_nodes_removed` / `invalid_node_samples`，`GeneratedGroupSnapshot.is_custom`，
> 新增 `AggregationRenameRule`（双端共用 `模式 => 替换` 文本语法与 `parse_list`/`to_text`）、
> `AggregationCustomGroup`、`AggregationTemplate`、`AggregatedProfileOutcome`。
> domain `ProfileAggregator` 新增 `rename_pipeline`（清洗前执行，非法正则 typed 失败）、
> `precheck_nodes`（复用 `proxy_nodes::validate` 富模型按 `proxies` 索引配对 + 新增
> `validate_item` 覆盖降级为 `OtherNode` 的协议必填项）、自定义组合成与重名/类型校验，
> `region_label`/`custom_members` 均为唯一实现。application
> `ProfileAggregationApplication` 新增 `create_profile_with_runtime`（激活复用
> `ProfileApplication::activate_profile`，无 runtime seam 时诚实降级为仅切换）、
> `list_templates`/`save_template`/`delete_template`/`reaggregate`；`ProfileStore` 新增模板
> sidecar（缺省 typed unsupported，`mihomo-config` 落 `<config-dir>/options/.aggregation-templates.yaml`
> 原子写），reader 投影 `aggregation_templates(_available)`。命令新增
> `SaveAggregationTemplate`/`DeleteAggregationTemplate`/`ReAggregateProfile`。
> Iced 模态新增预检/激活开关、重命名规则输入（非法行 toast）、自定义组追加/移除、
> YAML 结构视口（等宽 + 滚动 + 行数）、模板保存/复用/重新聚合/删除与向导回填，模态整体
> 改为视口内滚动；Bevy 卡片新增同名开关与输入、`AggregatorPreviewKind` 单查询重盖
> （计数/区域/组/ YAML/模板清单）、自定义组组合行与状态行，向导输入收集拆分为
> `profiles_aggregator_wizard.rs`（遵守 800 行预算），模板/重新聚合/删除均按模板名解析
> 投影后提交共享命令。
>
> **批次 B 已知偏差（诚实记录）**：
> 1. Bevy 的「可用性预检/激活」开关与输入框是**用户输入**（挂载后由用户操作，不由投影重盖），
>    投影只重盖文本行；因此无头测试显式设置开关状态，不依赖挂载时默认值。这与既有的
>    「文本字段保留用户输入」口径一致。
> 2. Bevy 侧模板清单是**多行文本 + 按名称查找**（模板名称来自输入框），不做每行独立按钮；
>    Iced 为每行独立按钮。两端提交的命令与共享事实完全相同。
> 3. Bevy 桌面组合根仍未安装 `CommandApplication::with_managed_runtime`（与组 07 相同偏差），
>    因此 Bevy 端「创建后设为当前配置」会切换活动 profile 但 `core_reloaded=false`；
>    Iced 桌面宿主持有 `Arc<dyn HostRuntime>`，可热载入。`AggregatedProfileOutcome` 如实回传。
> 4. 预检样本上限 `MAX_PRECHECK_SAMPLES = 8`，`invalid_nodes_removed` 恒为完整剔除数；
>    样本仅用于定位，不假装是完整清单。
> 5. 源→聚合 profile 的联动以「创建时自动保存同名模板」建立；用户若手工删除模板，
>    该 profile 的「重新聚合」入口随之消失（共享 `reaggregate` 对未知模板返回 typed 失败）。
> 6. Bevy 自定义组的组合行以「向导内累积（含最近一次共享预览的草稿）」为准；若用户在
>    未预览的情况下离开 Profiles 页面再返回，该行会显示最近一次共享预览的自定义组
>    （未提交的临时追加仅存于向导资源，提交时仍然生效），下一次追加/清空/复用模板即恢复同步。

---

## 一、双端同步推进战略定调与工程原则

### 1. 核心定位转变：从“先后跟随”转为“严格对等双主干”
*   **过去模式**：Iced 优先承接新功能，Bevy UI 后续里程碑追赶。该模式易导致两端视图模型分叉、功能落差扩大。
*   **当前标准**：Iced 与 Bevy UI 正式确立为**对等双主干 Surface**。任何新业务功能与 UI 交互能力，必须在同一批次（Wave）内由双端同步交付与验收，杜绝“单端孤立特性”。

### 2. 双端同步必须开展的 4 项工程基础设施

```
               ┌────────────────────────────────────────────────────────┐
               │       共享应用层与跨端状态机（单一事实源）              │
               │  infiltrator-application / domain / contract / ports  │
               └──────────────────────────┬─────────────────────────────┘
                                          │
                                          ▼
               ┌────────────────────────────────────────────────────────┐
               │       共享契约与能力模型（UI-Agnostic Seam）            │
               │  CommandIntent / Snapshot / Event / Capability         │
               └──────────────┬──────────────────────────┬──────────────┘
                              │                          │
                              ▼                          ▼
          ┌───────────────────────────────┐  ┌───────────────────────────────┐
          │    infiltrator-iced (桌面)    │  │  infiltrator-bevy-ui (跨平台)  │
          │  - Elm 架构 (View/Update)      │  │  - ECS 架构 (Scenes/Systems)  │
          │  - Iced Theme / Canvas        │  │  - Bevy Widgets / Reactive    │
          └───────────────┬───────────────┘  └───────────────┬───────────────┘
                          │                                  │
                          └────────────────┬─────────────────┘
                                           ▼
               ┌────────────────────────────────────────────────────────┐
               │ composition roots + host/outbound adapters             │
               │ mihomo-api / config / version / desktop / android / ios│
               └────────────────────────────────────────────────────────┘
                                           ▼
               ┌────────────────────────────────────────────────────────┐
               │ 双端行为、视觉与宿主证据门禁                           │
               │ headless tests / parity guard / live smoke / snapshots  │
               └────────────────────────────────────────────────────────┘
```

1.  **业务逻辑 100% 下沉与 ViewModel 契约化**：
    *   严禁在 UI crate 中编写业务调度、网络请求或配置组装。
    *   业务状态机收敛于 `infiltrator-domain` 与 `infiltrator-application`；平台和传输实现留在 ports 之后的 adapter；
    *   双端共享 `CommandIntent`、typed result、snapshot、event、capability 和 revision/generation；`infiltrator-shared` 只承载确实属于展示共享的主题、本地化和格式化资源。
2.  **单向命令总线与事件广播标准化**：
    *   Iced 的 `Message` 与 Bevy 的 `UiCommand` 只做 toolkit 映射，背后派发相同的 `CommandIntent`，由同一个 application/core 事实源处理；
    *   底层遥测流（WebSocket 流量、连接流、日志）统一推入无锁缓冲区，双端同频消费。
3.  **设计系统与组件库 1:1 镜像对齐**：
    *   统一 Design Tokens：两端色彩阶梯（主色、警示色、背景层级）、间距（4/8/12/16/24px）、圆角（4/8/12px）与排版数值镜像一致；
    *   基础控件 1:1 对应：虚拟视口列表（`VirtualList` ↔ `list/scroll_core`）、折线图（`Waveform` ↔ `chart::bezier`）、抽屉与模态层（`connection_drawer` ↔ `drawer/adaptive_modal`）、极简小窗（`mini_hud` ↔ `desktop::mini_hud`）。
4.  **双端无头测试与自动化对齐门禁**：
    *   开发静态门禁工具，确保双端路由、页面投影、intent、能力/错误状态与测试覆盖匹配；
    *   双端具备无头自动化测试矩阵（`tests/gui/iced_*` 与 `tests/headless/*`），保障相同用户交互在两端产生一致的内核行为；
    *   定期运行视觉比对脚本，保障 11 个页面在桌面与自适应视口下的视觉层次统一。

---

## 二、成熟 Mihomo 客户端能力并集标杆定义

我们对标的成熟客户端包括：
*   **Clash Verge Rev**：脚本生态（JS/TS 沙箱）、多级配置覆写（Merge Rules / Mixin）、服务模式（Service Mode）、全局热键、高级 TUN 网络栈。
*   **Mihomo Party**：Sub-Store 原生整合、分流链路可视化拓扑、交互式实时分流追踪器（Live Rule Tracer）、多维测速丢包雷达、应用级分流与高清图标提取。
*   **Flclash**：桌面/平板/移动多模态响应式自适应设计、多配置聚合器（Aggregator）、高密度卡片网格、WebDAV 云同步。
*   **Clash Nyanpasu**：多维连接审计（按进程/按域名）、Fake-IP 池检索、流体动效与主题定制。
*   **Surge / Stash**：DNS/TCP/TLS/TTFB 握手瀑布流、ASN 归属透视、Doctor 专家级自愈诊断、实时抓包与 PCAP 导出。

**本项目目标：将上述所有竞品的最完善能力求“全功能并集（Union）”，并在 Iced 与 Bevy UI 中全量镜像对齐！**

### 2.1 Mihomo 能力基线

能力规划先以 Mihomo 官方配置与 API 面为边界，再决定客户端增强项：配置面覆盖 DNS、Tun/listeners、出站协议、代理组、路由规则、规则集合、子规则和流量隧道；API 面覆盖日志、流量、内存、版本、缓存、运行配置、重启/升级、代理组、代理、providers、规则、连接、DNS、存储和 debug。客户端负责可理解地编辑、验证、回滚和观测，不在 Rust UI 层重新实现 Mihomo 的协议内核。

参考：[Mihomo 配置文档](https://wiki.metacubex.one/config/)、[Mihomo API 文档](https://wiki.metacubex.one/api/)、[Clash Verge Rev](https://github.com/clash-verge-rev/clash-verge-rev)、[FlClash](https://github.com/chen08209/FlClash)。

---

## 三、15 大业务组 × 15 项全景深度功能清单（225 项双端深度对标并集）

为彻底消除表面化对齐，本项目确立 **15 大核心业务组 × 15 项深度能力（共 225 项具体工程指标）**，要求 `infiltrator-iced` 与 `infiltrator-bevy-ui` 协同攻坚、共同对齐。每项的编号为 `DUAL-组号-项号`，每项都必须同时具备 shared/application、Iced、Bevy、双端测试和适用宿主证据；单端完成不计入完成。

### 组 01：Core 运行时、进程守护与内核多版本交付 (Runtime & Lifecycle)
1. **多代际内核会话状态机**：Session Token 隔离、Generation 递增与孤儿进程自动清理。
2. **平滑配置热重载 (Hot Reload)**：`PUT /configs?force=true` 毫秒级重载，避免断网断流。
3. **崩溃自愈看门狗 (Crash Watchdog)**：异常退出 3 秒内心跳自动重启，带指数退避与熔断。
4. **内核多通道版本交付**：Stable / Alpha (Pre-release) / Meta-Core 在线版本探测。
5. **内核二进制 SHA256 校验**：下载自动比对官方 release digest，防止篡改。
6. **内核版本秒级回滚**：本地保留历史版本 binary，一键切换并快速恢复。
7. **外部 Controller 免密拉起**：自动生成并在请求头注入 Secret 凭据，杜绝未授权访问。
8. **内核日志等级即时下发**：无需重启内核动态 `PATCH /configs` 切换 debug/info/warn/error。
9. **服务模式 (Service Mode) 提权守卫**：Windows Service / Linux Polkit / macOS launchd 免 UAC 弹窗。
10. **进程退出清理保证**：注册 OS 信号处理（SIGINT/SIGTERM/Ctrl+C），退出时 100% 复位网络。
11. **端口冲突自动探测与避让**：检测 7890/9090 冲突，自动提示占用进程 PID 并提供一键释放。
12. **内核内存与 CPU 软限配额**：监控内核资源占用，超过 512MB 时主动触发 GC。
13. **离线与无网启动容灾**：本地配置预校验通过即可离线冷启动，不依赖远端鉴权。
14. **双端生命周期状态机同步**：Iced 与 Bevy 共享同一套 `CoreLifecycle` 运行时驱动。
15. **无头测试全景覆盖**：双端具备模拟启动失败、端口冲突、平滑停止的完整测试。

### 组 02：特权网络栈、TUN 虚拟网卡与系统代理接管 (Platform Network & TUN)
1. **TUN 四大堆栈自由调度**：支持 `gVisor`、`System`、`Mixed`、`LWIP` 原生切换。
2. **物理与虚拟网卡 MTU 自适应协商**：动态探测物理出站 MTU，自动计算最佳 TUN MTU。
3. **严格路由与全局流量劫持**：`strict-route: true` 防止流量绕过，自动添加路由表项。
4. **系统 HTTP/SOCKS 代理一键注入**：Windows 注册表、Linux GNOME/KDE/GSettings、macOS networksetup 统一封装。
5. **系统代理被抢占实时探活**：每 3 秒检测系统代理注册表，被第三方篡改时自动复位并弹窗警示。
6. **非正常断电/死机自愈恢复**：启动时扫描上次未正常清理的孤儿代理设置，启动自愈清理。
7. **局域网代理共享 (Allow-LAN)**：混合监听端口，绑定特定网卡 IP 地址。
8. **局域网接入 ACL 鉴权**：支持基于 IP/CIDR 白名单和 HTTP 基本认证的局域网安全准入。
9. **IPv6 内核转发拓扑开关**：一键禁用 IPv6 路由转发，杜绝双栈环境下的公网泄漏。
10. **Windows UWP 回环隔离解除工具**：列举 UWP 应用，一键免除 Loopback 隔离。
11. **PAC 动态代理脚本生成与本地服务**：自动生成标准 PAC 脚本，提供本地 HTTP PAC 服务端点。
12. **物理网卡漫游与默认网关感知**：Wi-Fi / 有线网络切换时自动触发 TUN 路由表平滑自愈。
13. **Android VpnService 移动端无缝穿透**：移动端原生 VPN 权限申请与前台服务保活。
14. **双端系统级开关 UI 表现 100% 对等**：Iced 与 Bevy 在侧栏和页面具备同等操作体验。
15. **特权网络无头回归测试**：mock 宿主适配器测试注入、清理与错误回滚。

### 组 03：核心概览、双通道遥测中枢与动态拓扑链 (Overview & Telemetry)
1. **真实双通道流量波形 (GPU Bezier)**：贝塞尔样条平滑算法，实时绘制上下行速率曲线。
2. **动态量程标尺与发光着色器**：自适应 Y 轴最大刻度（KB/s、MB/s、GB/s），曲线下方渐变发光。
3. **分流链路可视化拓扑流动链**：`Inbound -> Sniffer -> RuleSet -> Proxy Group -> Outbound` 动态流动指示。
4. **拓扑节点下钻跳转交互**：点击 Inbound 跳设置，点击 RuleSet 跳规则，点击 Group 跳代理。
5. **主活动出口节点高保真卡片**：展示当前选中的出口节点名称、所属国旗、协议胶囊与测速延迟。
6. **订阅配额与临期动态仪表盘**：已用/总流量进度条、百分比、账单重置倒计时天数。
7. **配额三级预警机制**：配额超 85% 预警黄，超 95% 告警红，无订阅显示本地卡片。
8. **系统代理与 TUN 双主控大卡**：大卡片 Switch 开关，带微触觉动画与权限引导。
9. **代理运行模式即时分段控制器**：`Rule` / `Global` / `Direct` / `Script` 四态滑动胶囊。
10. **全局一键并发测速按钮**：概览页头部一键测速，带环形旋转百分比进度。
11. **核心资源 6 项运维网格**：连接数、内存、CPU、上行速率、下行速率、累计会话总流量。
12. **公网 IP 隐私归属探针**：展示真实外网出口 IP、国家城市徽标与 ISP 运营商，支持一键刷新。
13. **卡片模块长按纵向拖拽重排**：支持拖拽调整流量图、拓扑链、指标网格的先后顺序并持久化。
14. **断线与重载优雅降级蒙版**：内核重启期间界面保留上一帧有效快照，覆以“重载中”平滑蒙版。
15. **双端全视口响应式表现 1:1 对齐**：宽屏、平板、移动端 100% 保持信息层次一致。

### 组 04：代理策略、节点选择器与多维智能排序 (Proxies & Sorting)
1. **策略组 5 大分类全覆盖**：Selector（手动选择）、URLTest（自动测速）、Fallback（故障转移）、LoadBalance（负载均衡）、Relay（链式中继）。
2. **策略组展开/折叠状态持久化**：用户独立折叠常用策略组，重启后记忆折叠态。
3. **节点选择状态即时回写**：`PUT /proxies/{group}` 秒级下发，两端节点卡片高亮即时同步。
4. **节点死链一键隐藏 (Filter Alive)**：工具栏一键过滤超时（None）或未测试的不可用节点。
5. **四维排序控制器**：支持 `延迟升序`、`延迟降序`、`名称升序`、`名称降序` 即时重排并持久化。
6. **节点星标置顶与收藏**：点击节点卡片星标，该节点锁定在策略组首位，不被排序覆盖。
7. **协议与特性高级芯片**：节点卡片显式标注 `Shadowsocks`、`VLESS`、`Reality`、`Vision`、`UDP`、`TFO` 等。
8. **节点延迟多色阶渲染**：<100ms 翡翠绿，100-200ms 青草绿，200-300ms 暖黄，>300ms 警戒红，超时灰色。
9. **单节点历史延迟 Sparkline 走势图**：节点卡片集成最近 10 次采样微折线走势图。
10. **智能拼音与协议模糊检索**：支持汉字、拼音首字母（如 `xg` 匹配 `香港`）及协议类型过滤。
11. **单节点详情下钻抽屉**：展示服务器域名、落地 IP、加密方式、历史 RTT 波动区间。
12. **策略组自定义拖拽调序**：长按策略组卡片可调整展示顺序，并支持一键恢复默认。
13. **节点卡片网格与紧凑列表无缝切换**：支持 2~4 列响应式网格与单列高密度列表一键切换。
14. **测速动态脉冲骨架屏占位**：节点测速期间数值显示平滑波纹骨架屏，测速完毕淡入延迟。
15. **双端代理操作无头行为测试闭环**：双端断言排序、置顶、测速状态与组选择。

### 组 05：协议生态保真、链式代理与多路复用 (Protocols & Transport)
1. **Shadowsocks 2022 全密码族**：2022-blake3-aes-128-gcm, 2022-blake3-aes-256-gcm, 2022-blake3-chacha20-poly1305。
2. **VLESS 进阶特性保真**：XTLS-Reality (`pbk`, `sid`, `spx`), Vision 流控 (`xtls-rprx-vision`), uTLS 指纹伪装。
3. **TUIC v5 与 Hysteria 2 拥塞控制**：支持 TUIC v5 BBR/Cubic，支持 Hysteria 2 端口跳跃 (`ports`) 与 masquerade。
4. **WireGuard / AmneziaWG 全参数**：支持 `preshared-key`, `reserved` 混淆字节、AmneziaWG 参数 (`jc/s1/h1-h4`)。
5. **现代传输层覆盖**：XHTTP (SplitHTTP), gRPC (multi-mode/service-name), 原生 QUIC, WebSocket Early Data (0-RTT)。
6. **SIP003 插件链生态**：`v2ray-plugin`, `obfs-local`, `shadow-tls v3`, `simple-obfs` 解析与协同。
7. **原生 SSH SOCKS 代理**：支持 `user`, `private-key`, `passphrase`, `host-key-algorithms`。
8. **AnyTLS 与 Trojan-Go 接入**：支持无特征 TLS 握手与混淆传输。
9. **前置跳板代理 (Dialer-Proxy)**：可视化配置节点作为前置跳板代理链路，支持链路拓扑展示。
10. **环路依赖静态检测**：检测 Dialer-Proxy 与 Relay 策略组之间的死循环配置并警示阻断。
11. **多路复用 (Smux / Yamux / H2Mux) 调优**：细化 `max-connections`, `min-streams`, `padding`, `brutal-opts`。
12. **TLS/ECH 与多版本 ALPN 协商**：支持 ECH 扩展注入、ALPN 多版本协商 (`h3, h2, http/1.1`)。
13. **自定义 CA 证书与证书白名单**：支持导入自签 CA 证书，避免内网或特定节点证书报错。
14. **双向无损格式转换**：URI、JSON、Clash YAML 互转过程中保持 100% 结构保真，未知字段无损流通。
15. **协议解析与序列化全量单测**：全协议解析无头覆盖。

### 组 06：并发测速引擎、丢包抖动雷达与稳定性评估 (Speedtest & Jitter)
1. **信号量流控并发测速**：基于 `Semaphore(30)` 流控，批量测速分批发出，防止并发风暴与熔断。
2. **单策略组独立测速**：支持只对当前展开的单个策略组测速，无需全量重测。
3. **测速目标 URL 动态自定义**：支持用户配置测速 URL（如 Cloudflare, Google, 延迟测试探针）。
4. **真实下行带宽测速**：支持对单节点发起多线程实际分片拉取，测定真实 Mbps 带宽。
5. **网络抖动 (Jitter ms) 精确计算**：多次高频往返探测，计算 RTT 标准方差与抖动率。
6. **丢包率 (Packet Loss) 梯度评级**：采样探测丢包比例，标明 0%（极佳）、<5%（良好）、>20%（较差）。
7. **五星稳定性综合雷达评分**：综合延迟、抖动、丢包、历史掉线率计算节点健康得分。
8. **测速进度环形百分比动画**：测速按钮旁实时旋转展示测速完成进度（如 `42/128`）。
9. **超时与不可用节点即时归档**：握手失败或超时的节点自动淡出并落入底部不可用区。
10. **测速取消与安全中断**：测速进行中支持点击“取消测速”，安全丢弃尚未发出的测速任务。
11. **历史测速数据持久化缓存**：保存最近 3 次测速结果，应用重启后不丢失上一次测试态。
12. **节点真实 IP 与出口探测对比**：测速同时探活出站真实落地 IP 与预期落地国家是否相符。
13. **测速结果弹窗详细透视**：点击测速详情，弹出综合雷达图与多维指标卡片。
14. **双端测速状态机与动效一致**：Iced 与 Bevy 测速动画、进度反馈与数据投影完全对齐。
15. **测速流控与状态机无头测试**：并发调度、超时处理与取消流程 100% 测试覆盖。

### 组 07：订阅管理、多渠道导入与定时更新流水线 (Subscriptions Lifecycle)
1. **多渠道导入三合一**：支持 URL 远程拉取、本地文件导入、剪贴板一键解析导入。
2. **自定义单个订阅 User-Agent**：支持全局及单个 Profile 独立定制请求头 `User-Agent`，防止防爬阻断。
3. **定时自动轮询与 Cron 表达式**：后台调度器支持 6h/12h/24h 及自定义 Cron 表达式自动刷新。
4. **条件请求 (ETag / If-Modified-Since)**：服务端配置未变时返回 304 Not Modified，零流量开销。
5. **网络重试与指数退避机制**：更新失败后 30s、1m、5m 自动退避重试，杜绝网络波动闪崩。
6. **单飞防重入调度 (Single Flight)**：订阅更新正在进行时，再次触发自动合并入当前管道，防止并发重复下载。
7. **订阅套餐用量与到期预警**：解析 `Subscription-Userinfo` 头，用量 > 85% 预警，到期 < 3 天标橙提示。
8. **订阅节点关键词清洗管道**：支持白名单、黑名单、协议过滤及正则批量重命名。
9. **更新后自动重启核心可选**：配置选项，订阅更新完成后自动触发核心平滑热重载。
10. **订阅更新静默系统通知**：支持订阅自动更新成功/失败后推送操作系统原生通知。
11. **一键手动更新全部订阅**：工具栏一键触发所有已配置订阅并发刷新，展示总进度。
12. **安全证书跳过选项 (Insecure Skip Verify)**：单个订阅支持独立关闭 TLS 证书校验。
13. **配置源文件安全备份**：订阅更新写入前自动生成 `.bak` 文件，写入失败自动恢复。
14. **双端订阅管理交互 1:1 对等**：Iced 与 Bevy 具备相同卡片、状态回显与操作弹窗。
15. **订阅更新流水线无头测试**：ETag 响应、解析失败回滚与定时调度单测完备。

### 组 08：多源订阅聚合器、节点清洗与自动拓扑生成 (Profile Aggregator)
1. **多订阅源勾选聚合向导**：可视化勾选多个本地/远程订阅配置作为源。
2. **跨订阅节点自动去重 (Deduplication)**：按 `Server + Port + Protocol` 唯一指纹自动去重相同节点。
3. **区域节点自动归类 (Geo Clustering)**：按 🇭🇰 🇯🇵 🇺🇸 🇸🇬 🇩🇪 等 ISO 国家代码自动分类成组。
4. **自动生成区域测速策略组**：自动生成“香港自动测速”、“日本自动测速”等专属 URLTest 策略组。
5. **主选择器自动级联 (Master Cascade)**：自动生成主 PROXIES 策略组并级联各国家分组。
6. **聚合后生成新独立 Profile**：不破坏原有订阅源，合并后保存为全新活动 Profile。
7. **一键保持源订阅联动更新**：源订阅更新后，聚合 Profile 支持一键触发重新聚合。
8. **自定义节点重命名规则**：聚合时支持正则替换节点前后缀（如统一去除广告后缀）。
9. **节点可用性预检与过滤**：聚合前预检节点必填字段，自动过滤缺失端口或秘钥的非法节点。
10. **自定义新策略组拓扑编排**：允许用户在向导中追加“流媒体专用”、“游戏专用”自定义策略组。
11. **聚合生成结果可视化预览**：保存前展示聚合后生成的 YAML 结构与策略组拓扑树。
12. **一键设为当前活动配置**：聚合成功后支持一键激活并热载入内核。
13. **历史聚合模板保存与复用**：记住用户的聚合勾选项与重命名配置，下次无需重复配置。
14. **双端聚合器模态 100% 对等**：Iced (`aggregator_modal.rs`) 与 Bevy (`profiles_aggregator.rs`) 互有证据。
15. **聚合器引擎全链路行为测试**：多源合并、去重、分组生成与持久化无头测试覆盖。

### 组 09：AST YAML 深度配置引擎、智能感知与版本快照 Diff (Config AST & Diff)
1. **100% 保真 YAML AST 引擎**：基于 AST 操作配置，保留注释、空行与 YAML 锚点 (`&/*`)，杜绝抹除。
2. **Monaco 级代码编辑器视口**：等宽字体、行号高亮、缩进参考线与视口滚动。
3. **YAML 语法实时预检与行号定位**：输入语法错误实时在出错行标注红色波浪线与精确行号。
4. **常用代码片段一键插入 (Snippets)**：支持快速插入 SS/VLESS/Trojan 节点、策略组与分流规则模板。
5. **代码一键格式化 (Format YAML)**：按照 Clash 规范美化排版与键名排序。
6. **配置自动历史快照备份**：每次 Apply 成功自动备份 snapshot，按时间与哈希记录，最多保留 20 份。
7. **历史快照自动智能修剪**：重复配置去重，超出上限自动按 LRU 淘汰旧快照。
8. **配置历史快照可视化并排/行内 Diff**：绿底 `+ Added`、红底 `- Removed`、黄底 `~ Modified` 逐行对比。
9. **快照一键安全回滚 (One-Click Rollback)**：选定历史快照后一键覆写回当前配置，带防呆二次确认。
10. **多配置快速切换 (Switch Profile)**：一键在不同配置方案间无感切换并秒级生效。
11. **配置无效时自动触发安全回滚**：新配置应用后若核心健康检查失败，自动撤销变更还原上一版本。
12. **只读保护与远程订阅防手滑覆写**：远程订阅默认进入保护态，提示用户通过 Mixin 覆写而非直接编辑。
13. **大文件编辑器性能优化**：编辑 10,000+ 行配置时维持 60 FPS 滚动，杜绝卡顿。
14. **双端编辑器与 Diff 模态完全镜像**：Iced (`snapshot_diff_modal.rs`) 与 Bevy (`profiles_diff.rs`) 保持一致。
15. **YAML 引擎与回滚事务无头测试**：注释保留、语法报错与回滚事务测试 100% 覆盖。

### 组 10：脚本沙箱生态、QuickJS 调试控制台与多级混入 (Scripting & Mixin)
1. **QuickJS 嵌入式轻量执行沙箱**：内置纯 Rust QuickJS 引擎，无 node/外部环境依赖。
2. **Pre-Process / Post-Process 钩子**：支持 `onProfileProcess(config)` 订阅处理钩子。
3. **沙箱资源熔断安全防护**：硬性限制 64MB 最大内存分配、500ms 最大执行时间，超时自动中断防死锁。
4. **内置三大官方常用脚本模板**：国家地区自动成组模板、流媒体分流覆写模板、内网直连穿透模板。
5. **实时代码调试控制台视口**：左侧代码编辑区，右上预设选择，右下实时控制台日志回显。
6. **`console.log` 流式捕获与拦截**：脚本运行输出即时投射到控制台日志区，便于调试。
7. **AST 变换前后实时对比预览**：输入 YAML 与脚本变换后输出的 YAML 实时渲染对比。
8. **多级配置覆写流水线 (Cascade Pipeline)**：Base Profile -> 订阅配置 -> Merge 规则 -> 全局 Mixin。
9. **三栏式 Mixin 编辑器**：左侧 Base 配置、中间 Mixin 覆写块、右侧合成后最终配置实时预览。
10. **Mixin 脚本语法检查与错误阻断**：脚本解析异常时阻止覆写生效，保障内核稳定运行。
11. **常用 Mixin 预设一键开关**：常用覆写项（如开启 IPv6、注入自定义 DNS）独立卡片开关。
12. **扩展脚本导出与社区分享**：支持将调试成功的脚本导出为独立 `.js` 文件。
13. **异常处理安全降级**：脚本报错时不影响原有配置基础运行，弹出告警通知。
14. **双端脚本控制台与 Mixin 视口对齐**：Iced (`script_console.rs`) 与 Bevy (`profiles_script.rs`) 对等呈现。
15. **QuickJS 引擎与沙箱熔断单测**：超时熔断、内存限制与 AST 变换测试 100% 覆盖。

### 组 11：分流规则引擎、MRS 二进制加速与逻辑子规则 (Rules & Rule-Providers)
1. **28+ 规则类型全矩阵支持**：DOMAIN, DOMAIN-SUFFIX, DOMAIN-KEYWORD, IP-CIDR, SRC-IP-CIDR, GEOIP, GEOSITE, PROCESS-NAME, PROCESS-PATH, DSCP, UID 等。
2. **逻辑组合子规则 (Logic Rules) 递归构建**：支持 `AND`, `OR`, `NOT`, `SUB-RULE` 多层嵌套条件判定。
3. **MRS 官方二进制规则集高性能适配**：全面支持 Mihomo `.mrs` 格式，内存零拷贝极速匹配。
4. **Rule-Provider 外部规则集生命周期管理**：展示来源 URL、行为模式（domain/ipcidr/classical）、更新时间。
5. **外部规则集增量更新与 ETag 缓存**：支持手动触发更新、定时自动更新与 304 条件缓存。
6. **规则集一键解构导入 (Unpack Provider)**：将远程 Rule-Provider 规则条目一键解构为本地可编辑规则。
7. **规则集本地缓存一键清理**：支持清理已下载的规则集本地缓存文件，释放磁盘空间。
8. **分流规则列表 50,000+ 条目虚拟视口滚动**：$O(1)$ 几何裁剪，维持 60 FPS 丝滑滚动。
9. **单规则一键停用/启用 Switch**：不删除规则前提下临时禁用某条规则，即时生效。
10. **规则拖拽调序与优先级置顶**：长按拖拽把手调整规则上下顺序，自动重排优先级。
11. **快速新增自定义规则表单向导**：规则类型、匹配内容、目标策略组结构化表单录入。
12. **一键注入游戏分流预设规则集**：内置 Steam, Epic, Riot, Blizzard 常见游戏平台分流预设。
13. **规则列表关键词模糊搜索与分页**：按匹配表达式、类型与目标出站即时搜索过滤。
14. **双端规则管理视口与组件 1:1 对等**：Iced (`rules.rs`) 与 Bevy (`rules_mrs.rs`) 完整闭环。
15. **规则引擎与 MRS 解析无头测试**：多类型规则匹配、逻辑子规则评估与解构无头断言。

### 组 11 逐项账目（2026-09-22 展开）

组 11 闭环口径同组 06/07/13：shared contract/application + Iced + Bevy + 双端无头测试 + 守卫。
本轮关键事实：Bevy 的 MRS 卡片此前是硬编码伪造条目（`geoip.mrs (14,200 条目…)`），
与共享 `MrsAccelerationSnapshot` 无关；Iced 的 provider 解构/缓存清理亦为伪造
（`apple.com`/`icloud.com` 样本与 no-op toast），故 11-06/07 诚实记为 `planned`。
本轮收口 11-03/04/13：Bevy 改为渲染共享 MRS 读模型，provider 生命周期新增共享
`source_url` 投影，搜索与分页归约为 `infiltrator-domain::rules::view` 共享纯函数。

| 项 | 任务 | 状态 | 证据 |
| :--- | :--- | :--- | :--- |
| `DUAL-11-01` | 28+ 规则类型全矩阵支持 | `parity-ready` | 共享类型目录 `infiltrator-domain/src/rules/matrix.rs`：`RULE_TYPE_MATRIX` 33 项（29 个 `RuleType` 具体拼写 + 4 个逻辑算子），`RuleTypeSpec{name,label,family,is_logical,accepts_no_resolve}`、`RuleTypeFamily`（Host/Geo/Address/Process/Port/Transport/Inbound/Identity/Composite/Terminal/Unknown）、`RuleType::spec()` 对 `types.rs:7` 全变体的穷尽映射、`matrix_label`/`matrix_family`/`spec_for_name`/`matrix_is_logical`/`matrix_accepts_no_resolve`（去 `-`/`_` 与大小写归一化）；Iced 删除本地 `display_rule_type` 拼写表与 `semantic_badge_kind` 手写分支，改委托 `matrix_label`/`matrix_family`（`view/rules.rs:139/167`）；Bevy 规则行新增类型 chip（`rules_projection.rs` `RuleTypeBadge` + `rule_type_chip_fill` 按家族取色，`rules.rs` `rule_type_chip_scene`），标签与填充随投影原位重盖（`apply_rules_projection`）；双端测试 Bevy `test_rules_type_matrix_renders_every_shared_label`（33 行全标签 + 33 个 chip）、Iced `test_rules_type_matrix_and_logical_builder_delegate_to_shared`（33 拼写标签 + 家族/徽标映射 + 未知拼写回退）、领域 `matrix_11_01_rule_type_vocabulary_parses` 与 `matrix.rs` 4 项单测。**诚实边界**：目录覆盖共享解析器已知拼写，未知拼写照原文渲染（不伪造标签） |
| `DUAL-11-02` | 逻辑组合子规则递归构建（AND/OR/NOT/SUB-RULE） | `parity-ready` | 共享 AST `LogicalRuleAst` 与递归 `evaluate`（`infiltrator-domain/src/sub_rules.rs:8/18`）、`parse_logical_rule`（`:204`）、`validate_logical_rule_syntax`（`:166`）经 `parse_rule_str`（`rules/types.rs:134`）进入 `RuleType::Logical`；本轮新增共享契约 `LogicalDraft{operator,conditions,target}`（`infiltrator-contract/src/rule_edit.rs`）与纯归约 `infiltrator-domain/src/rules/logical.rs`：`LOGICAL_OPERATOR_CHOICES`、`SUB_RULE_CONDITION_PRESETS`、`select_operator`/`set_target`/`add_condition`/`remove_condition`（空值/重复/越界/NOT 单条件全部 typed no-op）、`draft_payload`/`draft_operator`/`draft_expression`、`draft_issue`、`build_logical_rule`（经 `edit::build_custom_rule` 复用同一语法门）。Iced 删除本地 `SubRuleDraft` 改持共享 `LogicalDraft`（`types/rules.rs`/`state.rs`/`app.rs`），面板改渲染共享算子词表/条件预设/预览/校验状态并补上目标输入（`view/subrules_builder.rs`），插入与四个改动消息全部委托共享归约（`update/ui_wave3.rs`）；Bevy 新增可视子规则构建器 `crates/infiltrator-bevy-ui/src/pages/rules_subrules.rs`（`RulesSubRuleState`、算子 chip、条件行 `SubRuleConditionList` 动态重建、预设按钮、目标输入、预览/校验行、`on_rules_subrules_activated` 经 `draft_payload` 提交 `UiCommand::AddCustomRule` 共享意图），`rules_page` 挂载 + `bind_rules_page` 注册资源与观察者；双端测试 Bevy `test_rules_subrules_builder_submits_shared_logical_intent`、Iced `test_advancement_w3_2_subrules_logical_builder_workflow`、领域 `matrix_11_02_logical_draft_builds_recursive_expression`。**关键修复**：Iced 旧实现把条件直接以 `, ` 拼接（`OP((a, b)),TARGET`，缺逐条括号且不做校验，共享解析必失败），现统一产出 `OP((a),(b),TARGET)` 并经共享校验门，非法组合仅弹诚实错误 |
| `DUAL-11-03` | MRS 官方二进制规则集高性能适配（元数据/校验/状态） | `parity-ready` | 共享 `MrsAccelerationSnapshot/MrsItemSnapshot`（`infiltrator-contract/src/mrs_acceleration.rs:95/76`）+ 二进制解析/校验/解构 `parse_mrs_header`（`infiltrator-domain/src/mrs.rs:156`）、`validate_mrs_bytes`（`:227`）、`deconstruct_mrs_payload`（`:417`）；application `MrsAccelerationApplication::project`（`mrs_acceleration_application.rs:18`）经 reader（`surface_reader.rs:460`）发布；Bevy 改为渲染共享读模型（`pages/rules_mrs.rs:100 rules_mrs_scene`、`apply_mrs_projection`、`RulesProjection.mrs_acceleration` `pages/rules.rs:143`），删除硬编码伪造条目；Iced 新增共享加速卡（`view/mrs_panel.rs` `mrs_acceleration_card`/`mrs_acceleration_status_line`/`mrs_acceleration_item_label`，状态注入 `state.rs`）；双端测试 Bevy `test_rules_mrs_renders_shared_snapshot_not_fabricated`、Iced `test_mrs_acceleration_card_renders_shared_status_and_items` |
| `DUAL-11-04` | Rule-Provider 生命周期管理（来源 URL/行为/更新时间） | `parity-ready` | 共享契约新增 `RuleProviderSnapshot.source_url`（`infiltrator-contract/src/surface_snapshot.rs:251`）；reader 合并活动 profile 的 `rule-providers` 声明填充 URL（`surface_reader.rs:684/697`），运行时 provider 诚实保持 `None`；Iced `provider_lifecycle_line` + `rule_provider_row`（`view/rules.rs:384`）渲染 `Updated/Source`，URL 经 `state.rs` 投影；Bevy `provider_updated_label`（`pages/rules.rs:568`）渲染 `更新/来源`；双端测试 Iced `test_provider_lifecycle_line_reports_shared_source_url`、Bevy `test_rules_provider_lifecycle_renders_shared_source_url` |
| `DUAL-11-05` | 外部规则集增量更新与 ETag 缓存 | `shared-ready` | 共享 `CommandIntent::RefreshRuleProviders`（`infiltrator-contract/src/command.rs:234`）经 `command_application.rs:484` 逐 provider 调 `RuntimeGateway::update_rule_provider`（真实 `PUT /providers/rules/{name}`）；Iced `Message::UpdateRuleProvider`（`update/core/rules.rs:704`）与 Bevy `RefreshRuleProvidersButton`（`pages/rules.rs:357`）同源。本轮补上可诚实投影的一半：共享 `RuleProviderSnapshot.refresh_interval_secs`（reader 读活动 profile `rule-providers.<name>.interval`，未声明为 `None`，`surface_reader.rs`）、共享 `view::format_refresh_interval`，Iced `provider_lifecycle_line` 渲染 `Auto: 1d (kernel-scheduled)` / `Auto: not declared`（`state.rs rule_provider_intervals`），Bevy `provider_updated_label` 渲染「自动刷新: 1d (内核调度)/未声明」；双端测试 Iced `test_provider_lifecycle_line_reports_shared_source_url`、Bevy `test_rules_provider_lifecycle_renders_shared_source_url`、领域 `matrix_11_05_provider_refresh_intent_and_declared_interval`。**诚实边界**：`ETag`/`If-None-Match` 的 304 结果只发生在 mihomo 内核内部（`PUT` 不返回校验器，`/providers/rules` 也不暴露），客户端不投影缓存命中/未命中；客户端只披露 profile 声明的调度间隔，故本项维持 `shared-ready` |
| `DUAL-11-06` | 规则集一键解构导入 (Unpack Provider) | `planned` | 共享后端存在：`unpack_provider_rules_to_custom`（`infiltrator-domain/src/rules.rs:89`）、`unpack_mrs_to_rule_entries`（`mrs.rs:487`）；但 `CommandIntent::UnpackRuleProvider` 在 `command_application.rs:781` 明确返回 `unsupported()`；Iced 两个入口均为伪造样本（`update/core/rules.rs:773` 用 provider 名造样本、`ui_wave5.rs:315` 硬编码 `apple.com`/`icloud.com`），Bevy `UnpackRuleProviderButton` 未接线 |
| `DUAL-11-07` | 规则集本地缓存一键清理 | `planned` | 无真实删除路径：Iced `Message::PurgeRuleProviderCache` 仅弹成功 toast、不做文件清理（`update/ui_wave5.rs:337`），Bevy 无对应动作；不伪造“已清理”磁盘结果 |
| `DUAL-11-08` | 50,000+ 条目虚拟视口滚动 | `shared-ready` | 共享分页归约 `infiltrator-domain/src/rules/view.rs`（`page_count`/`page_bounds`/`clamp_page`）双端消费：Iced 每页 200 条（`update/core/rules.rs` `apply_rules_filter`）；Bevy `RulesViewState` + 分页控件（`pages/rules_view.rs:47`、`pages/rules.rs:480`）。本轮把截断事实做成共享读模型：共享 `view::{RULE_PUBLISH_LIMIT, published_rule_count, omitted_rule_count, is_truncated_rule_list}`，契约 `RulesPageSnapshot.rule_publish_limit` + `omitted_rule_count()`/`is_truncated()`，reader 以 `total_rules` 发布截断前全量、按共享上限 cap 已发布列表（`surface_reader.rs`）；Bevy `RulesProjection.truncated_rule_count/rule_publish_limit` + `rules_projection::truncation_label`（「发布视口已截断 · 已省略 N 条 (发布上限 5000 条，非 O(1) 虚拟滚动)」）原位重盖；Iced `editor.rule_publish_limit/rule_publish_omitted` + `publish_truncation_line`（`view/rules.rs`）在分页行显示同一共享事实（编辑器仍加载全量 profile 列表）；双端测试 Bevy `test_rules_truncation_note_reports_publish_cap`、Iced `test_rules_provider_interval_and_publish_truncation_project_from_snapshot`、领域 `matrix_11_08_search_and_pagination_reduce_in_shared_view`。**诚实边界**：仍为分页 + 原位隐藏（Bevy 按发布列表构建行、Iced 只渲染当前页行），未实现 $O(1)$ 几何裁剪或真窗口重建，因此不宣称虚拟滚动/FPS 数字 |
| `DUAL-11-09` | 单规则一键停用/启用 Switch | `parity-ready` | 共享事实 `RuleEntry.enabled` + `format_rule_entry`（`infiltrator-domain/src/rules.rs:273`，禁用落为 `#` 前缀）与纯归约 `edit::toggle_rule_enabled`（`infiltrator-domain/src/rules/edit.rs:66`）；契约 `CommandIntent::ToggleRuleEnabled`（`infiltrator-contract/src/command.rs:227`）经 application `edit_rules`（`infiltrator-application/src/command_application.rs:682`）读改写活动 profile；Iced `Message::ToggleRuleEnabled` 改委托共享归约（`update/core/rules.rs:561`）；Bevy 规则行新增 `RuleToggleButton`（`pages/rules_edit.rs:16`）+ `rule_row_controls_scene`（`pages/rules.rs:635`），`RuleItem.is_enabled` 由 `rule_snapshot` 投影（`surface_projection.rs:200`），禁用行渲染「已停用」（`pages/rules.rs:565`）；双端测试 Bevy `test_rules_toggle_and_reorder_submit_shared_intents`、Iced `test_rules_edit_operations_delegate_to_shared_module`、application `toggle_and_move_apply_shared_reductions_and_persist`。Bevy 经应用即时落盘，Iced 仍为本地置脏 + `SaveRules`（归约同源），行为等价 |
| `DUAL-11-10` | 规则拖拽调序与优先级置顶 | `parity-ready` | 共享方向枚举 `RuleMoveDirection`（`infiltrator-contract/src/rule_edit.rs:12`）与纯归约 `edit::move_rule`（`infiltrator-domain/src/rules/edit.rs:78`，边界内交换、越界 typed no-op）；契约 `CommandIntent::MoveRule`（`command.rs:232`）；Iced `MoveRuleUp/Down` 委托共享归约（`update/core/rules.rs:569/581`）；Bevy 上行/下行把手 `RuleMoveUpButton`/`RuleMoveDownButton`（`pages/rules_edit.rs:20/24`）经 `on_rules_row_edit_activated` 提交共享意图，`UiCommand::MoveRuleUp/Down` 映射 `MoveRule`（`command.rs`）；双端测试 Bevy `test_rules_toggle_and_reorder_submit_shared_intents`、Iced `test_rules_edit_operations_delegate_to_shared_module`。**诚实边界**：两端均为把手按钮调序（Iced 本就无拖拽手势），未实现指针拖拽 |
| `DUAL-11-11` | 快速新增自定义规则表单向导 | `parity-ready` | 共享构建器 `edit::build_custom_rule`（`infiltrator-domain/src/rules/edit.rs:43`，空 payload 拒绝 + 逻辑规则经 `validate_logical_rule_syntax`）与类型词表 `CUSTOM_RULE_TYPE_CHOICES`/`DEFAULT_RULE_TARGET`（`:13/29`）；契约 `CommandIntent::AddCustomRule`（`command.rs:238`）经 application 校验并 `prepend_rules` 落盘；Iced `add_rule_panel` 类型列表改读共享词表（`view/rules.rs:612`），`AddCustomRule` 委托共享构建器（`update/core/rules.rs:280`）；Bevy 向导实现类型 chip `RuleTypeChip` + 载荷/目标输入 `RulePayloadField`/`RuleTargetField` + `RulesBuilderState`，`on_rules_builder_activated` 提交共享意图（`pages/rules_builder.rs`）；双端测试 Bevy `test_rules_add_wizard_submits_shared_draft_and_type_selection`、Iced `test_rules_edit_operations_delegate_to_shared_module`、application `add_custom_rule_prepends_and_rejects_invalid_logical_form` |
| `DUAL-11-12` | 一键注入游戏分流预设规则集 | `parity-ready` | 共享预设 `game_routing_presets`（`infiltrator-domain/src/rules.rs:119`，Steam/Epic/Riot/Blizzard/EA 进程与域名）与归约 `edit::inject_game_presets`/`prepend_rules`（`rules/edit.rs:113/100`，保序置顶）；契约 `CommandIntent::ApplyGameRoutingPresets`（`command.rs:245`）；Iced `ApplyGameRoutingPresets` 委托共享归约（`update/core/rules.rs:594`）；Bevy `InjectGamePresetsButton` 接线，`on_rules_builder_activated` 读取目标输入并提交共享意图（`pages/rules_builder.rs`）；双端测试 Bevy `test_rules_game_presets_submit_shared_target`、Iced `test_rules_game_presets_and_geo_update`、application `game_presets_prepend_the_shared_default_list` |
| `DUAL-11-13` | 规则列表关键词模糊搜索与分页 | `parity-ready` | 共享归约 `infiltrator-domain/src/rules/view.rs`：`RuleView` seam、`matches_rule_search`、`filter_rule_indices`、`page_count`/`clamp_page`/`page_bounds`；Iced `apply_rules_filter`（`update/core/rules.rs`）与 `RulesNextPage` 全部委托共享函数；Bevy 新增搜索框 `RuleSearchField` + `RulesViewState` + 分页按钮/指示器与 `sync_rules_view`/`on_rules_paging_activated`（`pages/rules_view.rs:89/134`），按共享谓词与页界隐藏行；双端测试 Iced `test_rules_filter_and_pagination_delegate_to_shared_reduction`、Bevy `test_rules_search_hides_non_matching_rows`/`test_rules_pagination_hides_rows_outside_page` |
| `DUAL-11-14` | 双端规则管理视口与组件 1:1 对等 | `planned` | 本轮后 Bevy 已具备 MRS 共享卡、搜索/分页、启停/调序、新增向导、游戏预设、可视逻辑子规则构建器与共享命中审计行；仍缺 Iced 的 tabs 分区（列表/Providers/JSON 编辑器/Tracer）、服务端解构（11-06 `planned`）、本地缓存清理（11-07 `planned`）、GeoData 更新入口与命中审计卡，组件形态未 1:1 |
| `DUAL-11-15` | 规则引擎与 MRS 解析无头测试 | `parity-ready` | 共享回归矩阵 `crates/infiltrator-domain/tests/rules_matrix_test.rs`：`matrix_11_01_rule_type_vocabulary_parses`（28 种类型拼写 + MATCH + 全 33 项目录项解析回自身 spec + 标签/家族断言）、`matrix_11_02_logical_sub_rules_evaluate`（AND/OR/NOT/SUB-RULE + 非法括号）、`matrix_11_02_logical_draft_builds_recursive_expression`（共享 draft 构造/规范化/拒绝非法），`matrix_11_03_mrs_binary_pipeline`（`build_mrs_bytes`/`parse_mrs_header`/`deconstruct_mrs_payload`）、`matrix_11_04_rule_provider_source_url_projection`、`matrix_11_05_provider_refresh_intent_and_declared_interval`（刷新意图 + 声明间隔投影）、`matrix_11_08_search_and_pagination_reduce_in_shared_view`（搜索/页界 + 发布上限/省略计数）、`matrix_11_09_to_12_rule_edit_reductions`；双端矩阵：Bevy `test_rules_matrix_covers_closed_items`（类型标签 + MRS/provider/search/paging + toggle/reorder/wizard + 子规则构建器控件计数）、`test_rules_type_matrix_renders_every_shared_label`（33 项目录全量挂载断言）、`test_rules_subrules_builder_submits_shared_logical_intent`（共享 draft 变更、非法组合阻断、合法提交 `AddCustomRule`）、`test_rules_truncation_note_reports_publish_cap`（截断事实渲染）、Iced `test_rules_type_matrix_and_logical_builder_delegate_to_shared`（33 拼写标签/家族 + 共享语法门 + 共享常量 + draft 一致性）、`test_advancement_w3_2_subrules_logical_builder_workflow`（共享构建器产出可解析表达式）、`test_rules_provider_interval_and_publish_truncation_project_from_snapshot`（间隔 + 截断投影）、`test_rules_edit_operations_delegate_to_shared_module`；11-06/07 仍诚实 `planned`，矩阵不伪造其后端 |

> **2026-09-22 组 11 批次 A**：`DUAL-11-03/04/13` 收口为 `parity-ready`。
> 03：Bevy 删除硬编码伪造 MRS 条目，改为渲染共享 `MrsAccelerationSnapshot`
> （含 Ready/Empty/Unsupported/Failed/Unknown 诚实状态），Iced 新增同源加速卡。
> 04：共享契约新增 `RuleProviderSnapshot.source_url`，reader 合并活动 profile
> 的 `rule-providers` 声明，运行时-only provider 诚实显示“未声明”。
> 13：新增共享 `infiltrator-domain::rules::view`（搜索谓词 + 分页算术），Iced
> 删除本地过滤/页界计算改委托共享函数，Bevy 新增关键词搜索框与分页控件并按
> 共享谓词/页界隐藏行。**关键修复**：删除 Bevy `rules_mrs.rs` 中伪造的
> `14,200 条目` 列表；11-06/07 因 Iced 仍为伪造样本/no-op 诚实标记为 `planned`。
> 守卫 `scripts/quality/rules-engine-guard.py` 固化本组账目与关键标记。
>
> **2026-09-22 组 11 批次 C**：`DUAL-11-01/02` 收口为 `parity-ready`，11-05/08 补强
> 后仍诚实 `shared-ready`。
> 01：新增共享类型目录 `infiltrator-domain::rules::matrix`（`RULE_TYPE_MATRIX` 33 项、
> `RuleType::spec()` 穷尽映射、`matrix_label`/`matrix_family` 归一化查找）；Iced
> 删除本地拼写表改委托，Bevy 规则行新增按家族着色的类型 chip（`RuleTypeBadge`），
> 标签与填充随投影原位重盖。
> 02：新增共享契约 `LogicalDraft` 与纯归约 `infiltrator-domain::rules::logical`
> （算子词表/条件预设/预览/校验门/`build_logical_rule`）；Bevy 新增可视子规则构建器
> `pages/rules_subrules.rs`（算子 chip、条件行动态重建、预设、目标输入、预览与校验行，
> 提交共享 `AddCustomRule` 意图）；Iced 面板改持共享 draft 并补目标输入。**关键修复**：
> Iced 旧实现输出的 `OP((a, b)),TARGET` 缺逐条括号且不校验（共享解析必失败），
> 现统一为 `OP((a),(b),TARGET)` 并经共享语法门。
> 05：新增共享声明投影 `RuleProviderSnapshot.refresh_interval_secs` 与
> `view::format_refresh_interval`，双端渲染「内核调度」间隔；`ETag`/304 仍只在核内，
> 客户端不投影、不伪造命中，故维持 `shared-ready`。
> 08：新增共享发布上限算术（`RULE_PUBLISH_LIMIT`/`published_rule_count`/
> `omitted_rule_count`），契约新增 `rule_publish_limit` 与 `omitted_rule_count()`/
> `is_truncated()`，reader 诚实发布截断前全量 + 已发布上限，Bevy 与 Iced 同源渲染
> 截断事实与页界；仍未实现 $O(1)$ 几何裁剪，故维持 `shared-ready` 且不宣称虚拟滚动。
> 守卫 `scripts/quality/rules-engine-guard.py` 已扩展覆盖本批（含禁止旧拼接形式回归）。
>
> **2026-09-22 组 11 批次 B**：`DUAL-11-09/10/11/12/15` 收口为 `parity-ready`。
> 新增共享契约 `infiltrator_contract::rule_edit`（`RuleMoveDirection`/`RuleDraft`）与
> 纯归约模块 `infiltrator_domain::rules::edit`（`toggle_rule_enabled`/`move_rule`/
> `prepend_rules`/`build_custom_rule`/`inject_game_presets`），新增
> `CommandIntent::{ToggleRuleEnabled,MoveRule,AddCustomRule,ApplyGameRoutingPresets}`，
> application 经 `edit_rules` 读改写活动 profile 落盘。Bevy 规则行补齐启停/调序
> 控件，向导补齐类型 chip + 载荷/目标输入与资源选择；Iced 的启停/调序/新增/预设
> 全部改委托同一批共享归约。新增共享回归矩阵
> `crates/infiltrator-domain/tests/rules_matrix_test.rs` 及双端矩阵用例。
> **诚实边界**：11-06/07 仍为 `planned`（Iced 解构/清缓存为伪造样本/no-op，
> 不新增伪造后端）；11-10 为把手按钮调序而非指针拖拽（Iced 原本亦无拖拽手势）。

### 组 12：交互式实时分流追踪器 (Live Rule Tracer) 与命中审计 (Rule Tracer)
1. **交互式分流追踪沙盒视口**：输入目标（域名/IP）、端口、进程名与来源网络，立即模拟分流匹配。
2. **分流决策链树状回放**：完整回放命中规则序号、匹配表达式、所属规则集与出站策略决策。
3. **快捷测试预设域名芯片**：提供 `google.com`, `github.com`, `bilibili.com`, `1.1.1.1` 一键测试。
4. **规则命中实时流计数 (Hit Counter)**：基于内核事件流实时累加每条规则的命中频次。
5. **冷门死规则静态诊断**：识别命中次数为 0 的冷门规则与被上层完全覆写的死规则。
6. **IP-CIDR 掩码重叠与冲突检测**：静态分析下层 IP 规则被上层大掩码规则拦截的逻辑漏洞。
7. **命中时间戳记录**：展示该规则最近一次被命中的时间（如 `刚刚`、`5分钟前`）。
8. **分流结果一键反向应用**：追踪结果展示若不符合预期，提供“修改此规则出站”直达按钮。
9. **规则时延贡献审计**：统计不同分流策略路径平均网络时延贡献。
10. **仿真沙盒环境参数模拟**：支持模拟特定来源 IP（内网某台设备）发起的分流判定。
11. **一键清空规则命中计数**：支持重置累计计数器，重新统计会话命中情况。
12. **规则命中高亮闪烁动效**：在规则列表中实时高亮闪烁刚被命中的规则行。
13. **离线分流追踪支持**：在内核离线状态下利用本地 AST 逻辑规则树执行纯离线模拟。
14. **双端 Tracer 沙盒组件完全镜像**：Iced (`rules_tracer.rs`) 与 Bevy (`rules_tracer.rs`) 对等挂载。
15. **Tracer 判定算法无头断言覆盖**：域名后缀、关键字、IP 掩码追踪测试 100% 绿灯。

### 组 13：实时连接审计、多维聚合透视与深度链路瀑布流 (Connections & Telemetry)
1. **高并发实时连接列表流式采集**：WebSocket 推流源/目/进程/规则/出站/速率/累计流量。
2. **多维聚合视图无缝切换**：扁平流视图 (Flat)、按应用进程聚合 (By Process)、按目标域名聚合 (By Host)。
3. **单连接详情 Slide-out 侧滑下钻抽屉**：右侧平滑展开 420px 详情抽屉，带半透明遮罩。
4. **耗时瀑布流 (Timing Waterfall)**：DNS 解析耗时、TCP 握手耗时、TLS 握手耗时、TTFB 首字节耗时色条。
5. **目标 IP、ASN 组织归属与地理透视**：展示目标落地 IP、AS 编号、所属组织机构（如 `AS36459 GitHub`）。
6. **完整路由链溯源**：详细呈现该连接从 Inbound 到 Outbound 的每一跳策略组名称。
7. **连接实时治理与切断**：支持一键断开当前单条连接、一键切断当前过滤筛选结果中的连接。
8. **一键关闭全部活动连接 (Close All)**：带防呆二次确认，一键清除所有会话。
9. **反向一键生成规则向导**：在连接详情中，一键将目标域名/IP 添加到分流规则（DIRECT/REJECT/PROXY）。
10. **高吞吐连接脉冲微光指示**：瞬时带宽超过 5MB/s 的连接行呈现发光呼吸微动效。
11. **空闲连接智能清退 (Idle Sweeper)**：可配置自动清理超过 10 分钟无数据传输的死连接。
12. **按上传/下载瞬时速率实时排序**：支持表头点击按瞬时带宽动态重新排列连接行。
13. **连接关键词即时搜索**：支持按域名、IP、进程名输入关键字即时过滤。
14. **双端连接抽屉与瀑布流 1:1 对等**：Iced (`connection_drawer.rs`) 与 Bevy (`connections_drawer.rs`) 保持一致。
15. **连接数据流与治理命令无头测试**：单连断开、全连切断与聚合计算测试 100% 覆盖。

### 组 13 逐项账目（2026-09-22 展开）

组 13 闭环口径同组 06：shared contract/application + Iced + Bevy + 双端测试 + 守卫。
本轮关键事实：Iced 连接抽屉此前的 DNS/TCP/TLS/TTFB 瀑布流是硬编码伪造（`18/42/68/92 ms`），
Bevy 抽屉同样伪造（`18/42/65/110 ms`），而 mihomo `/connections` 载荷不携带任何阶段耗时
—— 已删除伪造并改为 typed unsupported；另将聚合/搜索/排序/反向规则归约为
`infiltrator-domain::connection_view` 的共享纯函数，双端只做渲染。

| 项 | 任务 | 状态 | 证据 |
| :--- | :--- | :--- | :--- |
| `DUAL-13-01` | 高并发实时连接列表流式采集 | `parity-ready` | 共享流相位枚举 `infiltrator_contract::connection::ConnectionStreamPhase`（`connection.rs`，`from_page_status` 由页面状态诚实推导）；后端 WS 推流：`mihomo-api/src/connection/manager.rs:83`（`stream`）、`infiltrator-application/src/connection_application.rs:52`；Iced `RuntimeStreamState::shared_phase`（`types/runtime.rs`）映射共享枚举并由 `stream_badge`（`view/runtime/connections.rs`）渲染；Bevy `connections_projection` 从共享快照页状态取相位（`surface_projection.rs`），`ConnectionsLineKind::Stream` + `stream_phase_label`（`pages/connections.rs`）渲染「连接流 · 相位」徽标；双端测试 `test_stream_state_maps_to_shared_phase`、`test_connections_stream_badge_reflects_shared_phase` |
| `DUAL-13-02` | 多维聚合视图（Flat/按进程/按域名） | `parity-ready` | 共享归约 `infiltrator-domain/src/connection_view.rs:19`（`ConnectionGroupingMode`）与 `:261`（`aggregate_connections`）；Iced 视图直接消费 `connection_view::aggregate_connections`（`infiltrator-iced/src/view/runtime/connections.rs:280`、`:284`）并显示 `conn_aggregate_count`；Bevy 聚合胶囊持有共享枚举（`infiltrator-bevy-ui/src/pages/connections.rs:102`），`on_connections_view_activated`（`:585`）切换 `ConnectionsViewState` 并重盖聚合摘要/行可见性；双端测试 `test_connections_aggregation_pill_switches_shared_mode`、`aggregation_summary_reports_buckets`、Iced `test_shared_connection_view_reductions_are_delegated` |
| `DUAL-13-03` | 单连接详情侧滑下钻抽屉 | `parity-ready` | Iced 右侧 420/480px 抽屉 + 半透明遮罩：`infiltrator-iced/src/view_root/connection_drawer.rs`；Bevy 改用共享 `infiltrator_bevy_widgets::drawer::drawer_scene(DrawerPlacement::Right, 420.0, …)`（`infiltrator-bevy-ui/src/pages/connections_drawer.rs`），行内 `ConnInspectButton` 打开、`DrawerCloseButton` 关闭，`ConnectionsDrawerState` + `sync_connections_drawer` 控制显隐；两端内容为同一诚实字段集（含 13-06 逐跳链路与 13-09 反向规则）；双端测试 `test_connections_inspect_opens_shared_drawer` |
| `DUAL-13-04` | 耗时瀑布流（DNS/TCP/TLS/TTFB） | `planned` | **宿主能力缺失**：`mihomo-api`/`infiltrator-domain::runtime::ConnectionMetadata`（`infiltrator-domain/src/runtime.rs:89`）无任何阶段耗时字段，`/connections` 不提供；两端伪造色条已删除，Iced 改渲染 `conn_drawer_timing_unsupported`（`connection_drawer.rs:75`），Bevy 渲染「内核未提供…耗时明细」（`connections_drawer.rs:66`）；typed unsupported，不伪造阶段 |
| `DUAL-13-05` | 目标 IP、ASN 归属机构与地理透视 | `planned` | 目标 IP 已展示（`connection_drawer.rs` 远端地址），但 ASN/组织字段不存在于 `ConnectionMetadata`；`infiltrator-domain/src/geo_lookup_cache.rs` 未接入连接数据流，无共享 ASN 事实源，不伪造 `AS36459 GitHub` |
| `DUAL-13-06` | 完整路由链溯源 | `parity-ready` | 共享解析模型 `infiltrator-domain::connection_view::RouteChain`（`from_hops`/`from_joined`/`hops`/`display`）与 `ConnectionView::view_chain`、`route_chain`；契约 `ConnectionSnapshot.chains`（`infiltrator-contract/src/surface_snapshot.rs`），reader 映射 `infiltrator-application/src/surface_reader.rs:448`；Iced 行/抽屉逐跳渲染 `route_chain.hops()`（`view/runtime/connections.rs`、`view_root/connection_drawer.rs`）；Bevy `ConnChainHopText` 逐跳 + `connection_chain_scenes`（`pages/connections.rs`）；双端测试 `test_route_chain_hops_parse_through_shared_model`、`test_connections_route_chain_renders_each_hop`，领域 `route_chain_normalizes_and_parses_joined_form` |
| `DUAL-13-07` | 连接实时治理（单条/过滤范围切断） | `parity-ready` | 应用层 `close`/`close_by_host`/`close_by_process`（`infiltrator-application/src/connection_application.rs:22/36/44`）；Iced 单条 `Message::CloseConnection`（`update/core/monitoring.rs:295`）+ 范围 `Message::CloseFilteredConnections`（`:323`，复用共享 `matches_search`），按钮 `conn_close_filtered_btn`（`view/runtime/connections.rs:223`）；Bevy `CloseConnectionButton` + `CloseFilteredConnectionsButton`（`pages/connections.rs:347`、`:571` 用共享 `matches_search` 选目标）；双端测试 `test_connections_close_single_submits_command`、`test_connections_close_filtered_submits_matching_only` |
| `DUAL-13-08` | 一键关闭全部（二次确认） | `parity-ready` | Iced `ConfirmAction::CloseAllConnections` 经确认模态（`types/app.rs:113`、`view_root/modals.rs:905`、`view/runtime/connections.rs` 的 `btn_close_all`）；Bevy 两段式防呆 `ConnectionsCloseAllState`（`pages/connections.rs:545` 置 armed、再次点击提交 `CloseAllConnections`）；双端测试 `test_connections_close_all_requires_confirmation`（Bevy 首次不提交、二次提交）、Iced `app_state_tests` 确认路径 |
| `DUAL-13-09` | 反向一键生成规则向导 | `parity-ready` | 共享反向规则 seam：`quick_rule_spec`/`bare_host`/`draft_rule_entry`/`append_draft_rule`（`infiltrator-domain/src/connection_view.rs`，去重且拒绝空模式，`host:port` 归一为裸域名）；Iced 抽屉不可生成时禁用（`connection_drawer.rs:186`、`:212`），`AddQuickRuleFromConnection` 经 `append_draft_rule`（`infiltrator-iced/src/update/ui.rs`）；Bevy `DrawerAddRuleButton` 接线到共享 `ConnectionsRuleDraft` 资源并调用同一 `append_draft_rule`、回显草稿（`infiltrator-bevy-ui/src/pages/connections_drawer.rs`）；双端测试 `test_connections_add_rule_draft_uses_shared_seam`、领域 `draft_rule_entry_dedupes_and_rejects_empty`、`bare_host_strips_port_and_keeps_ipv6` |
| `DUAL-13-10` | 高吞吐连接脉冲微光指示 | `planned` | 依赖瞬时带宽，但 reader 将 `upload_bps/download_bps` 硬编码为 `0.0`（`infiltrator-application/src/surface_reader.rs:449`），无真实速率来源，不做伪动效 |
| `DUAL-13-11` | 空闲连接智能清退 | `parity-ready` | 共享 `infiltrator-domain::connection_activity::ConnectionActivityTracker`（`observe`/`idle_ids`：首个观测计为活跃、计数不变跨满超时才判空闲、消失即遗忘）与 `IDLE_TIMEOUT_CHOICES`（5/10/30 分钟）、`DEFAULT_IDLE_TIMEOUT_SECS`（10 分钟）；应用层 `ConnectionApplication::sweep_idle`/`idle_connections`/`IdleSweepReport`（`infiltrator-application/src/connection_application.rs`）；Iced 超时分段控件 + `SweepIdleConnections` 经 `close_connection`，`last_idle_sweep` 诚实状态（`view/runtime/connections.rs`、`update/core/monitoring.rs`）；Bevy `ConnectionsIdleState` + `ConnIdleTimeoutPill` + `ConnIdleSweepButton` 提交 `CloseConnection`，`idle_status_label` 诚实状态（`pages/connections_idle.rs`）；双端测试 `connection_idle_timeout_and_activity_tracking`、`test_connections_idle_sweep_submits_and_reports`、`test_connections_idle_timeout_pill_switches_shared_choice`；领域 4 项单测 |
| `DUAL-13-12` | 按上传/下载瞬时速率排序 | `planned` | 无瞬时速率（同上 `surface_reader.rs:449`）；Iced 现按累计字节排序（`view/runtime/connections.rs:71` 共享 `sort_connections`），Bevy 无排序控件，未达「瞬时速率实时排序」 |
| `DUAL-13-13` | 连接关键词即时搜索 | `parity-ready` | 共享谓词 `infiltrator-domain/src/connection_view.rs:232`（`matches_search`，覆盖域名/IP/进程/规则/链）；Iced `filter_connection` 委托共享（`view/runtime/connections.rs:66`）；Bevy 搜索框 `ConnSearchField`（`pages/connections.rs:328`）+ `sync_connections_search`（`pages/connections_view.rs:134`）按共享谓词隐藏不匹配行；双端测试 `test_connections_search_hides_non_matching_rows`、Iced `test_shared_connection_view_reductions_are_delegated` |
| `DUAL-13-14` | 双端连接抽屉与瀑布流 1:1 对等 | `planned` | 抽屉形态已对等（Iced 右侧侧滑、Bevy 共享 `drawer_scene(DrawerPlacement::Right)`，见 13-03）；但瀑布流本身 unsupported（13-04 宿主无阶段耗时），故「1:1 对等」仍缺一半 |
| `DUAL-13-15` | 连接数据流与治理命令无头测试 | `parity-ready` | 共享归约单测 8 项（`connection_view.rs` `#[cfg(test)]`）；Bevy 无头：聚合/搜索/单条断开/范围断开/全连二次确认（`pages_matrix_a_tests.rs:575/609/641/671`）；Iced：`test_shared_connection_view_reductions_are_delegated` + 既有分组/快速规则用例 |

> **2026-09-22 组 13 批次 A**：`DUAL-13-02/07/08/13/15` 收口为 `parity-ready`。
> 02：新增共享 `connection_view::ConnectionGroupingMode` + `aggregate_connections`，Iced
> 删除本地 HashMap 聚合，Bevy 聚合胶囊改为可交互并重盖摘要/行可见性。
> 13：共享 `matches_search` 谓词，Iced 委托、Bevy 新增搜索框与行级隐藏。
> 07：Iced 新增 `Message::CloseFilteredConnections` 复用共享谓词，Bevy 新增
> `CloseFilteredConnectionsButton` 提交匹配连接。08：Bevy 关闭全部改为两段式防呆
> （`ConnectionsCloseAllState`），Iced 维持确认模态。**关键修复**：删除两端抽屉伪造的
> DNS/TCP/TLS/TTFB 耗时，13-04 诚实标记为 `planned`（宿主无该数据）。守卫
> `connections-audit-guard.py`。

> **2026-09-22 组 13 批次 B**：`DUAL-13-01/03/06/09/11` 收口为 `parity-ready`。
> 01：新增共享 `infiltrator_contract::connection::ConnectionStreamPhase`，Iced 由
> `RuntimeStreamState::shared_phase` 映射，Bevy 由共享快照页状态推导并渲染「连接流 · 相位」徽标。
> 03：Bevy 详情改为共享 `drawer_scene(DrawerPlacement::Right, 420.0, …)` 侧滑抽屉，
> 行内「详情」按钮驱动 `ConnectionsDrawerState`。06：新增共享 `connection_view::RouteChain`
> 与契约 `ConnectionSnapshot.chains`，两端逐跳渲染。09：新增共享 `bare_host`/`draft_rule_entry`/
> `append_draft_rule` seam，Iced 与 Bevy `DrawerAddRuleButton` 共用，Bevy `ConnectionsRuleDraft`
> 资源承接草稿。11：新增共享 `connection_activity::ConnectionActivityTracker` + 应用层
> `ConnectionApplication::sweep_idle`，双端超时控件与手动清退、诚实上次清理状态。
> 04/05/10/12/14 仍因宿主事实缺失保持 `planned`（不伪造阶段耗时/ASN/瞬时速率）。
> 守卫 `connections-audit-guard.py` 已扩展覆盖本批。

### 组 14：DNS 工作台、Fake-IP 治理与泄漏交叉探活 (DNS Studio & Leak Protection)
1. **DNS 6 项系统级核心开关表单**：enable, ipv6, cache, use_hosts, use_system_hosts, respect_rules。
2. **域名映射模式 (Enhanced Mode) 分段器**：`虚拟 IP (Fake-IP)` / `真实 IP (Redir-Host)` / `取消映射 (None)`。
3. **过滤模式 (Fake-IP Filter Mode) 分段器**：`黑名单 (Blacklist)` / `白名单 (Whitelist)` / `规则 (Rules)`。
4. **上游加密 DNS (DoH/DoT/DoQ/HTTP3) 配置**：支持配置多个上游安全 DNS 服务器与协议标记。
5. **回退解析策略 (Fallback DNS)**：配置境内外 Fallback DNS 服务器与 GEOIP 触发阈值。
6. **Fake-IP 映射池实时检索与检视**：可视化检索 Fake-IP (198.18.x.x) 与真实域名的解析绑定表。
7. **一键清空 Fake-IP 缓存与系统 DNS 缓存**：下发清理指令并调用系统命令刷新 OS DNS 缓存。
8. **DNS 泄漏多源并发交叉探测**：并发向多个全球探测源发送随机伪子域，检验真实 ISP DNS 泄漏。
9. **WebRTC 公网 IP 穿透探测**：检测浏览器 WebRTC 是否会穿透虚拟网卡泄漏真实内网/公网 IP。
10. **DNS 解析测速与延迟高亮**：对配置的各个 Nameserver 发起延迟测速并标明响应耗时。
11. **自定义 Hosts 映射表图形化编辑**：支持可视化添加/删除自定义域名解析映射 (`IP Domain`)。
12. **Nameservers 动态标签芯片**：支持为不同上游 DNS 贴上 `Domestic`、`Fallback` 等语义标签。
13. **DNS 故障自愈检测**：检测 DNS 监听端口占用与上游无法解析异常并提示修复。
14. **双端 DNS 工作台表单完全一致**：Iced (`dns.rs`) 与 Bevy (`dns.rs`) 结构化表单完全同步。
15. **DNS 解析与探活状态机无头单测**：模式切换、泄漏探测与缓存清理测试 100% 覆盖。

### 组 14 逐项账目（2026-09-22 展开）

组 14 闭环口径同组 06/13：shared contract/application + Iced + Bevy + 双端测试 + 守卫。
本轮关键事实：Bevy 的 6 项开关此前只发出 `UiCommand::UpdateSetting { key: "dns.enable",
value: "toggle" }`——`CommandApplication::update_setting` 只接受
`language/theme/notifications_enabled/close_to_tray`，即点击必然被拒；且开关状态是
`dns_switch_row_scene(..., true, ...)` 硬编码，Iced 的过滤模式分段器为 `Message::Noop`
（不可交互）。本轮把 DNS 工作台表单收敛到共享契约 `infiltrator_contract::dns` /
`infiltrator_contract::dns_form`（`DnsCoreSwitches`/`DnsEnhancedMode`/`DnsFakeIpFilterMode`/
`DnsServerTag`/`DnsUpstreamProtocol`/`DnsFallbackPolicy`/`DnsCacheFlushReport`/
`DnsWorkbenchForm`/`DnsSettingsPatch`），共享应用 `ConfigurationApplication::
apply_dns_settings_with_runtime` 走已校验且带热重载的 profile 写路径，双端经共享补丁
提交，不再有伪开关命令；OS DNS 缓存刷新由宿主端口 `SystemDnsCachePort` 承接，无法支持的
宿主如实返回 `Unsupported`。

| 项 | 任务 | 状态 | 证据 |
| :--- | :--- | :--- | :--- |
| `DUAL-14-01` | DNS 6 项系统级核心开关表单 | `parity-ready` | 共享 `DnsCoreSwitches`/`DnsSwitchField`（`infiltrator-contract/src/dns.rs:142`/`:107`）；契约快照 `DnsPageSnapshot.switches`（`surface_snapshot.rs:330`）由 `surface_reader.rs:878 dns_core_switches` 从真实 `DnsConfig` 填充；共享写入 `ConfigurationApplication::apply_dns_settings`（`configuration_application.rs:47`）+ `CommandIntent::ApplyDnsSettings`（`command.rs:235`）；Iced 6 个 `form_toggle_row`（`view/dns.rs:352` 起，`Message::UpdateDnsForm*`）；Bevy `DnsSwitchButton(field, enabled)` 渲染真实状态并提交共享补丁（`pages/dns.rs:102`、`on_dns_action_activated` `pages/dns.rs:480`）；双端测试 `test_dns_switch_submits_shared_patch`、`test_dns_page_mounting_and_default_state`、应用 `switch_toggle_patch_derives_from_current_value`、`dns_settings_patch_maps_onto_domain_patch` |
| `DUAL-14-02` | 域名映射模式 (Fake-IP/Redir-Host/None) 分段器 | `parity-ready` | 共享三态 `DnsEnhancedMode`（`contract/src/dns.rs:13`）；`Unmapped` 经 `clear_enhanced_mode` 真正删除配置键（`domain/dns.rs:70`、`:183`），应用映射 `configuration_application.rs:182`；Iced `domain_mapping_mode_control`（`view/dns.rs:276`）+ `dns_patch_from_form`（`update/core/dns_config.rs:89`）；Bevy `DnsEnhancedModePill`（`pages/dns.rs:122`）经 observer 提交 `EnhancedMode`（`pages/dns.rs:517`）；双端测试 `test_dns_enhanced_mode_pill_submits_shared_patch`、`test_domain_mapping_and_filter_mode_controls`、契约 `enhanced_mode_round_trips_and_clears`、应用 `unmapped_mapping_mode_clears_the_key`、领域 `test_clear_enhanced_mode_removes_the_key` |
| `DUAL-14-03` | 过滤模式 (Blacklist/Whitelist/Rules) 分段器 | `parity-ready` | 共享 `DnsFakeIpFilterMode`，`Rules` 使用宿主值 `rule`（`contract/src/dns.rs:61`）；领域校验放开 `rule`（`domain/dns.rs:610` `lower != "whitelist" && lower != "blacklist" && lower != "rule"`）；应用映射 `configuration_application.rs:188`；Iced `filter_mode_control`（`view/dns.rs:294`）经 `Message::UpdateDnsFormFilterMode`（`update/core/dns_config.rs:281`）；Bevy `DnsFilterModePill`（`pages/dns.rs:126`）经 observer 提交（`pages/dns.rs:526`）；双端测试 `test_dns_filter_mode_pill_submits_shared_patch`、契约 `filter_mode_supports_rule`、领域 `test_filter_mode_accepts_rule_host_value` |
| `DUAL-14-04` | 上游加密 DNS (DoH/DoT/DoQ/HTTP3) 配置 | `parity-ready` | 共享 `DnsUpstreamProtocol::from_address` 统一解码 `https`/`h3`/`tls`/`quic`/`doq`/`sdns`/`dhcp`/`system`（`contract/src/dns.rs:244`），协议芯片 `chip_label` 同时驱动 Iced token 芯片与 Bevy 服务器行；列表编辑语义收敛到 `parse_server_list`/`append_server`/`remove_server_at`（`contract/src/dns.rs:330`/`:355`），Iced `parse_item_list`/`append_item_to_list` 与 Bevy `DnsEditField` 文本域均为薄封装；共享补丁新增 `nameserver/fallback/proxy_server_nameserver/direct_nameserver`（`contract/src/dns.rs:441`），映射与应用写路径 `dns_patch_from_settings`/`apply_dns_settings_with_runtime`（`configuration_application.rs:196`/`:59`）；Bevy 新增可写上游列表卡（`pages/dns_edit.rs:132`、`DnsEditField:47`），经 `UiCommand::ApplyDnsSettings` 提交；双端测试 `test_dns_upstream_list_edit_submits_shared_patch`、`upstream_protocol_decodes_both_surface_editor_shapes`、`test_dns_form_patch_uses_the_shared_workbench_mapping`、`snapshot_exposes_every_workbench_tier_and_protocol_chip` |
| `DUAL-14-05` | 回退解析策略 (Fallback DNS / fallback-filter) | `parity-ready` | 共享 `DnsFallbackPolicy`（`geoip`/`geoip_code`/`trigger_ipcidr`，`contract/src/dns.rs:376`）由 `fallback_policy()` 从真实 `dns.fallback-filter` 填充（`dns_workbench_application.rs:96`）并写入 `DnsPageSnapshot.fallback_policy`（`surface_snapshot.rs:345`）；应用映射写回领域 `FallbackFilter`（`configuration_application.rs:196`）；Iced fallback token 编辑器 + 新增 GEOIP 开关/geoip-code/触发网段控件（`view/dns_form_panel.rs:162` `dns_form_field_widget`）；Bevy 新增 `DnsEditGeoipToggle` 与触发网段文本域（`pages/dns_edit.rs:81`），双端共用 `DnsFormIssue::InvalidTriggerCidr` 校验（`contract/src/dns_form.rs:369`）；双端测试 `test_dns_fallback_policy_toggle_and_trigger_submit_shared_patch`、`test_dns_form_validation_issues_localize_in_both_locales`、`workbench_lists_and_fallback_policy_map_onto_the_domain_patch`；领域新增部分合并 `FallbackFilterPatch`（`domain/dns.rs:57`）只覆盖工作台编辑的三个子字段，`domain`/`domain-suffix`/`geosite` 等未编辑项在保存后保留（`test_partial_fallback_filter_preserves_unedited_subfields`）。**诚实偏差**：宿主 `fallback-filter` schema 只有 `geoip`/`geoip-code`/`ipcidr` 触发项，没有数值「阈值」字段，两端据此呈现，未发明阈值；清空 `geoip-code` 输入保持已配置值（宿主拒绝空串），清空触发网段会真正删除该列表 |
| `DUAL-14-06` | Fake-IP 映射池实时检索与检视 | `planned` | 领域 `FakeIpPool`（`domain/fake_ip.rs:183`、`reverse_lookup:274`）是进程内模拟器，未接入 mihomo 的实时映射来源；无共享只读快照发布 198.18.x.x ↔ 域名绑定，不伪造映射池 |
| `DUAL-14-07` | 一键清空 Fake-IP 缓存与系统 DNS 缓存 | `parity-ready` | 新增宿主端口 `SystemDnsCachePort`（`ports/src/system_dns_cache.rs:11`）与 `HostRuntime::system_dns_cache_port`（`ports/src/host_runtime.rs:84`）；共享应用 `DnsCacheApplication::flush_all` 依次清理 Fake-IP 与 OS 缓存并发布 `DnsCacheFlushReport`（`application/src/dns_cache_application.rs:53`，逐目标 `Flushed`/`Unsupported{reason}`/`Failed`）；`CommandIntent::ClearDnsCache` 走共享应用（`command_application.rs:484`）后经 `cache_flush_report` 进入 `DnsPageSnapshot.cache_flush`（`surface_reader.rs:835`、`surface_snapshot.rs:357`）；桌面适配器按平台调用 `resolvectl flush-caches`/`systemd-resolve --flush-caches`、`dscacheutil -flushcache`+`killall -HUP mDNSResponder`、`ipconfig /flushdns`，无可用工具时返回 `Ok(false)` → `Unsupported`（`desktop/src/system_dns_cache.rs:21`/`:73`，`runtime.rs:742` 注入）；Iced `Message::FlushFakeIpCache` → `Message::DnsCacheFlushed` 渲染本地化逐目标状态（`update/core/dns_config.rs:577`、`view/dns_form_panel.rs:333`），Bevy header 行渲染 `cache_flush_label`（`pages/dns.rs:310`）；桌面组合共享同一 `DnsCacheApplication` 实例（`desktop/src/composition.rs:99`、`surface.rs:155`）；双端测试 `test_dns_cache_flush_report_renders_honest_status`、`test_dns_cache_flush_outcome_labels_are_localized`、`reports_both_targets_honestly`、`a_host_without_the_os_adapter_reports_typed_unsupported`、`linux_flush_prefers_resolvectl`/`macos_flush_covers_dscacheutil_and_mdnsresponder`/`windows_flush_uses_ipconfig`/`an_unknown_platform_reports_typed_unsupported`。**诚实偏差**：移动端宿主未注入该端口，如实返回 `Unsupported`，不宣称已刷新 |
| `DUAL-14-08` | DNS 泄漏多源并发交叉探测 | `planned` | **宿主事实缺失**：Iced `Message::RunDnsLeakProbe`（`update/ui.rs:555`）只从 `probe_public_ip` 取真实出口 IP，`country="US"`、`isp="Cloudflare"`、`is_leak_detected=false` 与 `tested_dns_servers` 均为硬编码，且无共享契约、Bevy 无对应面板；多源并发伪子域探测需真实网络，保持 `planned`，不伪造探测结论 |
| `DUAL-14-09` | WebRTC 公网 IP 穿透探测 | `planned` | 领域 `PrivacyLeakDetectionSuite` 可基于连接/DNS 日志判定 STUN/TURN DIRECT 泄漏（`domain/diagnostics.rs:711`），`infiltrator-core::diagnostics_adapter` 已接；但无共享快照、无浏览器/STUN 实时探测、双端无该面板，保持 `planned` |
| `DUAL-14-10` | DNS 解析测速与延迟高亮 | `planned` | 领域纯逻辑 `DnsTester::rank_fastest_dns`（`domain/dns_tester.rs:344`）与 `DnsTestResult.latency_ms`；应用层 `CommandIntent::TestDnsLatency` 明确 `unsupported`（`command_application.rs:642`），无真实逐 Nameserver 延迟事实源，不填充假延迟 |
| `DUAL-14-11` | 自定义 Hosts 映射表图形化编辑 | `planned` | 领域 `HostsEngine`/`parse_hosts_file`（`domain/hosts_engine.rs:14`）存在但仅由 `lib.rs` 暴露、无 application/共享快照/双端 UI，未验收 |
| `DUAL-14-12` | Nameservers 动态标签芯片 | `parity-ready` | 共享纯分类 `DnsServerTag::classify`（`contract/src/dns.rs:195`）派生 `Domestic/Fallback/Encrypted/Plain`；应用发布 `DnsServerSnapshot.tags`（`surface_snapshot.rs:317`、`surface_reader.rs:851`）；Iced `server_tag_label` 芯片（`view/dns.rs:309`、`token_row`）；Bevy `DnsLineKind::ServerTags`（`pages/dns.rs:421`）+ `server_tags_text`（`pages/dns.rs:244`）；i18n `dns_tag_domestic/fallback/encrypted/plain`；测试契约 `server_tags_classify_fallback_domestic_encrypted`、Iced `test_server_tag_labels_are_localized` |
| `DUAL-14-13` | DNS 故障自愈检测 | `planned` | 领域静态审计 `validate_dns_topology`（`domain/dns_topology.rs:24`）与端口占用检测各自存在，但无共享 DNS 自愈快照/双端提示与修复动作，未验收 |
| `DUAL-14-14` | 双端 DNS 工作台表单完全一致 | `parity-ready` | 共享 `DnsWorkbenchForm` 18 个字段（`contract/src/dns_form.rs:16`/`:49`/`:169`）同时驱动两端表单：Iced 面板由 `DnsFormField::ALL` 循环组装（`view/dns.rs:156` `dns_form_panel`、`view/dns_form_panel.rs:162` `dns_form_field_widget`），Bevy 编辑卡覆盖同一字段集（`pages/dns_edit.rs:132` `dns_edit_card_scene`、`pages/dns.rs:360` 挂载）；两端提交同一 `DnsSettingsPatch`（`contract/src/dns.rs:441`），Bevy 经 `UiCommand::ApplyDnsSettings`（`pages/dns_edit.rs:508`）、Iced 经共享映射 `dns_patch_from_form`（`update/core/dns_config.rs:65`）；Iced 从共享 `DnsPageSnapshot` 重播表单草稿（`state.rs` `DnsWorkbenchForm::from_snapshot`、`view/dns.rs`），Bevy 从同一读取模型投影（`surface_projection.rs:284`）；校验/错误同源：共享 `DnsFormIssue`（`contract/src/dns_form.rs:120`），Iced 本地化横幅（`view/dns_form_panel.rs:314`）、Bevy 裸中文状态行（`pages/dns_edit.rs:424`）；对称无头测试 `test_dns_edit_rows_cover_every_shared_text_field`、`test_dns_form_field_widgets_cover_every_shared_field`、`test_dns_form_local_validation_blocks_invalid_scheme`、`test_dns_quick_template_chip_appends_unique_entry`。**局部呈现差异（保留）**：Iced 的 Raw JSON 编辑器与 token 删除按钮属于 Iced 本地高级视图，Bevy 用文本域 + 快捷模板芯片表达同一列表语义；共享字段、补丁与校验集合完全一致 |
| `DUAL-14-15` | DNS 解析与探活状态机无头单测 | `parity-ready` | 本轮补齐 14-04/05/07/14 的无头矩阵：契约 `upstream_protocol_decodes_both_surface_editor_shapes`/`form_round_trips_the_shared_snapshot`/`patch_carries_every_form_field_and_clears_an_emptied_range`/`validate_reports_unknown_schemes_and_bad_triggers`/`every_shared_field_has_a_distinct_key`（`contract/src/dns.rs:538`、`dns_form.rs:432`/`:450`/`:476`/`:506`）；应用 `workbench_lists_and_fallback_policy_map_onto_the_domain_patch`/`an_emptied_range_maps_to_the_explicit_clear_flag`/`cache_flush_report_follows_the_shared_application_state`/`reports_both_targets_honestly`/`a_host_without_the_os_adapter_reports_typed_unsupported`/`snapshot_exposes_every_workbench_tier_and_protocol_chip`；领域 `test_clear_fake_ip_range_removes_the_key`（`domain/src/dns_test.rs:693`）；桌面 `linux_flush_prefers_resolvectl`/`macos_flush_covers_dscacheutil_and_mdnsresponder`/`windows_flush_uses_ipconfig`/`an_unknown_platform_reports_typed_unsupported`（`desktop/src/system_dns_cache.rs:93` 起）；Iced `test_dns_form_field_widgets_cover_every_shared_field`/`test_dns_form_validation_issues_localize_in_both_locales`/`test_dns_cache_flush_outcome_labels_are_localized`/`test_dns_form_patch_uses_the_shared_workbench_mapping`（`tests/gui/view_dns_tests.rs:119` 起）；Bevy `test_dns_edit_rows_cover_every_shared_text_field`/`test_dns_upstream_list_edit_submits_shared_patch`/`test_dns_fallback_policy_toggle_and_trigger_submit_shared_patch`/`test_dns_form_local_validation_blocks_invalid_scheme`/`test_dns_quick_template_chip_appends_unique_entry`/`test_dns_cache_flush_report_renders_honest_status`（`tests/headless/pages_matrix_b_tests.rs:376` 起）。**未覆盖（保持诚实）**：14-08 泄漏探测与 14-10 解析测速仍无真实宿主事实源，无对应断言，不以伪数据凑 100% |

> **2026-09-22 组 14 批次 A**：`DUAL-14-01/02/03/12` 收口为 `parity-ready`。
> 新增共享 `infiltrator-contract::dns`（6 开关值对象、三态映射、三态过滤、服务器语义标签、
> `DnsSettingsPatch`）与 `CommandIntent::ApplyDnsSettings`/`ConfigurationApplication::
> apply_dns_settings`；Iced 过滤模式从 `Message::Noop` 改为共享枚举可交互，映射模式 None 现在
> 真正清除 `enhanced-mode`；Bevy 6 开关改为真实状态 + 共享补丁、映射/过滤分段器改为可交互
> 胶囊并接入 `LastDnsProjection`，删除伪造的 `UpdateSetting { key: "dns.*" }` 路径；
> 服务器行新增共享语义标签芯片。守卫 `scripts/quality/dns-studio-guard.py`。
> 06/08/09/10/11/13 因宿主事实缺失保持 `planned`（不伪造映射池/泄漏结论/延迟/Hosts 编辑）。

> **2026-09-22 组 14 批次 B**：`DUAL-14-04/05/07/14/15` 收口为 `parity-ready`。
> 新增共享表单 `infiltrator-contract::dns_form`（`DnsWorkbenchForm` 18 字段、
> `DnsFormField::ALL`、`DnsFormIssue` 校验、`is_cidr_literal`）与 `DnsUpstreamProtocol`
> 协议解码 + 列表编解码；两端的 DNS 表单现在由同一字段集组装、提交同一
> `DnsSettingsPatch`，并共用 `dns_patch_from_settings` 写路径。回退策略
> (`fallback-filter`) 的 GEOIP 开关/geoip-code/触发网段双端可写；宿主 schema 无数值阈值，
> 未发明该字段。新增宿主端口 `SystemDnsCachePort`：Fake-IP 与 OS DNS 缓存一键清理走共享
> `DnsCacheApplication`，逐目标发布 `Flushed`/`Unsupported{reason}`/`Failed`，桌面适配器
> 调用真实平台命令，移动端如实 `Unsupported`。新增 `clear_fake_ip_range` 域清除标志
> （镜像 `clear_enhanced_mode`）。守卫 `scripts/quality/dns-studio-guard.py` 已随本批扩展。
> 06/08/09/10/11/13 与 14-15 中的泄漏/测速断言因宿主事实缺失继续保持 `planned`，未以伪数据凑数。

### 组 15：桌面/移动多模态形态、系统托盘、独立悬浮小窗与极客命令流 (Multimodal & UX)
1. **4 阶响应式形态断点架构**：桌面宽屏 (Wide)、标准桌面 (Sidebar)、平板导轨 (Rail 64px)、移动紧凑 (BottomNav)。
2. **动态系统托盘菜单与速率徽标**：托盘图标动态更新上下行网速数值，右键集成模式切换与退出清理。
3. **独立桌面 Mini HUD 网速悬浮小窗**：260x90 置顶半透明无边框窗口，双向波形、出口节点与快捷开关。
4. **Mini HUD 坐标持久化与贴边吸附**：鼠标拖拽悬浮窗位置，关闭后重启恢复坐标，支持屏幕边缘吸附。
5. **全键盘命令面板 (Command Palette - Ctrl+K)**：居中悬浮模糊检索，拼音/英文直达 11 页面、模式切换与体检。
6. **全局系统快捷键管理与冲突检测**：可视化捕获和配置系统全局快捷键（系统代理开关/TUN开关/HUD唤出）。
7. **移动端触控手势引擎 (Gesture Engine)**：支持屏幕左边缘右滑返回 (Swipe Back) 与列表顶部下拉刷新。
8. **低功耗 Reactive 调步渲染 (Cadence)**：窗口后台或最小化时自动降频至 2 FPS，前台活动时恢复 60 FPS。
9. **深浅与多主题系统级跟随**：Dark 暗黑、Light 亮色、Forest 森林、AMOLED 纯黑，支持跟随系统切换。
10. **AccessKit 无障碍语义全覆盖**：所有控件、状态圆点、文本行携带 AccessKit 角色与屏幕阅读器标签。
11. **IME 中文输入法深度跟踪与候选框定位**：中文输入法候选框准确跟随光标坐标，防止遮挡。
12. **Toast 消息队列防重与敏感信息脱敏**：全局通知防抖去重，自动脱敏密码、Token 与 Bearer 敏感字段。
13. **桌面无边框窗口拖拽与原生阴影**：支持 Windows/Linux/macOS 现代无边框拖拽、双击最大化与阴影。
14. **双端设计规范与交互质感 100% 对齐**：Iced 与 Bevy 在所有模态下互有测试与截图证据。
15. **多模态与外壳架构无头测试矩阵**：断点切换、主题换肤、命令面板与快捷键 100% 自动化覆盖。

### 组 15 逐项账目（2026-09-22 展开）

组 15 闭环口径同组 06/12/14：shared contract/application + Iced + Bevy + 双端测试 + 守卫。
`DUAL-15-01`（4 阶响应式形态）由 [RESPONSIVE_PARITY_LEDGER.md](RESPONSIVE_PARITY_LEDGER.md)
权威跟踪（已按 mock 层收口），本表只保留指针，不复制其验收口径。

本轮关键事实（2026-09-22 批次 A）：

- **15-09 真实缺陷**：`AppSettings.theme` 默认 `"system"`，但 `view::theme::theme_from_name`
  把 `system` 与任何未知值一起回落 `Theme::Dark`——即「跟随系统」从未接线；同时
  `Message::SetTheme` 未挂在任何 update 分组上，设置页主题分段器点击是静默 no-op
  （`update.rs` 分组链最终落到 `Task::none()`），本轮一并修复。
- **15-06 真实缺陷**：Iced 设置页快捷键卡片的 `UpdateHotkeyCombo` 只写本地
  `Vec<HotkeyBinding>`（重启即丢、无冲突检测、不参与派发），Bevy 的
  `ShortcutRegistry`/`KeyboardChord` 是零引用的死代码，desktop 的
  `shortcut_manager.rs`（641 行、含冲突检测）也只被 `lib.rs` 导出。本轮把三者统一到
  共享 `infiltrator_contract::shortcuts` + `infiltrator_application::shortcut_application`。
- **15-12 真实缺陷**：Iced 侧 `toast_state::ToastManager`（带 2s 去重）零引用，
  实际 toast 走 `shell.toasts: Vec<(String, ToastStatus)>`（无去重、按 index 移除，
  前沿淘汰后过期任务会误删相邻 toast）；Bevy 侧 `toast_stack_scene`/`ToastQueue`
  从未挂载，且 `ToastQueue::default()` 容量为 0 → 任何 push 都会 panic。

| 项 | 任务 | 状态 | 证据 |
| :--- | :--- | :--- | :--- |
| `DUAL-15-01` | 4 阶响应式形态断点架构 | `parity-ready`（外链） | 权威台账 [RESPONSIVE_PARITY_LEDGER.md](RESPONSIVE_PARITY_LEDGER.md)：契约 `responsive_viewport` 阈值 600/840/1200（`infiltrator-contract/src/responsive_viewport.rs:12`），Iced `sidebar_for_tier`（`view/sidebar.rs:171`）、Bevy `Breakpoint::from_width` 镜像，守卫 `scripts/quality/responsive-parity-guard.py`；本表不重复验收 |
| `DUAL-15-02` | 动态系统托盘菜单与速率徽标 | `planned` | Iced 托盘菜单/工具提示存在且本地化（`crates/infiltrator-iced/src/tray/menu.rs:96 tooltip`、`:422` 状态行、`:39-54` 子菜单装配），但**明确不携带实时速率**：`tray/menu.rs:93` 记录「No live traffic figures here on purpose — a per-sample spec push would spam D-Bus」；`TraySpecContext`（`tray/spec.rs:403`）无速率字段；Bevy 无托盘面；`infiltrator-desktop/src/tray_badge.rs:54 generate_activity_tooltip` 是零引用死代码。真实速率徽标未接线，保持 `planned` |
| `DUAL-15-03` | 独立桌面 Mini HUD 悬浮小窗 | `shared-ready` | 共享只读模型 `infiltrator_contract/src/mini_hud.rs`（`MiniHudReadModel`：真实 traffic/mode/exit/共享开关状态，`status_line()` 由 `SystemToggleState::compact_label` 派生）；Iced `state.rs::mini_hud_read_model` 从 `diag.traffic`/`runtime.proxy_mode`/`runtime_selected_proxy`/`runtime.system_toggles` 组装，`view/mini_hud.rs` 全卡拖拽 + 真实双通道波形 + 出口胶囊 + 模式芯片 + 共享开关状态字母；Bevy `mini_hud_shell.rs::sync_mini_hud_model` 从 `LastOverviewProjection`+`SidebarToggleProjection`+`LatestSurfaceSnapshot` 组装，`mini_hud.rs::mini_hud_scene` 首次真实挂载（`MiniHudRoot`，删除硬编码 `"RULE"`/`"系统代理: 开启 · TUN: 开启"`）；`ToggleMiniHud` 端到端：Bevy `shortcuts.rs::dispatch_chords` 的 `Ctrl+Alt+M` 派发 + 调色板 `action.toggle_mini_hud` 行 + Iced `on_shell_shortcut`；双端测试 Iced `the_mini_hud_read_model_comes_from_live_projections`、Bevy `the_toggle_event_mounts_and_unmounts_the_overlay`/`the_mounted_scene_renders_the_shared_read_model`/`a_chord_pressed_for_the_mini_hud_toggles_the_mounted_overlay`。**诚实偏差**：Iced 确实把宿主单窗口缩到 260x90、移动到持久化坐标并 `Level::AlwaysOnTop`（`mini_hud_window.rs`），但仍是主窗口而非第二个独立窗口（Iced 多窗口宿主未接线）；Bevy 为窗口内绝对定位浮层，无宿主 `MiniHudWindowPort` 适配器，未投影为独立 OS 窗口，故不宣称 `parity-ready` |
| `DUAL-15-04` | Mini HUD 坐标持久化与贴边吸附 | `shared-ready` | 共享几何 `MiniHudPlacement`/`MiniHudDisplay`/`MiniHudGeometry::snap_to_edges`/`MiniHudSnapPlacement`（纯函数，含 `clamped_to`）；落盘 `AppSettings.mini_hud`（`domain/src/settings.rs`），写入走已校验的共享路径 `CommandApplication::update_setting` 的 `mini_hud.x/y/pinned` 分支（非法值 `InvalidInput`），读取经 `SettingsPageSnapshot.mini_hud`（`surface_reader.rs:1041`）进入双端读取模型；共享应用 `MiniHudApplication::place_from/set_pinned_from`（clamp → 贴边吸附 → 持久化 → 宿主结果），宿主端口 `ports/src/mini_hud_window.rs::MiniHudWindowPort` + `HostRuntime::mini_hud_window_port`（未注入的宿主如实 `MiniHudHostOutcome::Unsupported`）；Iced 拖拽 `mouse_area::on_move` → `Message::MiniHudMoved` → 位移镜像 → `iced::window::move_to`，松手 `MiniHudDragReleased` → `monitor_size` 真实屏幕矩形 → `mini_hud_store::place`（共享 clamp/snap/落盘）；Bevy 从共享快照读 placement，pin 经 `UiCommand::UpdateSetting { key: "mini_hud.pinned" }` 落盘；测试：契约贴边/夹取/序列化、应用 `place_snaps_clamps_persists_and_reports_the_host_outcome`/`a_host_without_the_window_adapter_reports_typed_unsupported`、`mini_hud_placement_writes_are_validated_per_field`、Iced `the_mini_hud_drag_moves_the_persisted_placement`/`always_on_top_mirrors_the_pin_onto_the_shared_placement`、Bevy `the_pin_request_persists_through_the_shared_settings_command`。**诚实偏差**：Bevy 无拖拽/移动窗口路径（无宿主坐标事实，不伪造窗口位置）；桌面宿主尚未实现 `MiniHudWindowPort`，当前移动请求返回 typed unsupported，仅坐标与 pin 持久化生效；`infiltrator-desktop/src/display_adapter.rs` 的旧几何仍未被该端口消费（HUD 路径改用共享契约几何，窗口移动留在宿主端口后） |
| `DUAL-15-05` | 全键盘命令面板 (Ctrl+K) | `parity-ready` | 共享目录 `infiltrator-contract/src/command_catalogue.rs`（`ShellPage` 11 页 / `CommandCategory` 5 类 / `CommandTarget` 12 种 / `CommandCatalogue::with_profiles` / `filtered_indices` / `CommandTarget::shortcut_action`，产品 23 行）；Iced 由目录驱动（`state.rs::rebuild_command_catalogue` 打开时重建 + `filtered_command_indices` 共享子串 + 拼音 + 首尾循环，`view_root/command_palette.rs` 渲染条目/图标/分类徽标/注册表实时加速键，`update/ui.rs::ExecuteCommand` 逐目标同步派发；原 `CommandAction`/`Category`/`Item` 本地模型删除）；Bevy 首次真实挂载（`command_palette_shell.rs::CommandPalettePlugin`：签名 latch 挂载 BSN 弹窗、`palette_keyboard_input` ↑↓ 循环/Enter/Escape/Backspace/字符输入、行点击、`on_execute_selected_palette_action` 与 Iced 逐目标对齐），`command_palette.rs` 由共享目录组装并用同一 `pinyin_fuzzy_match` 引擎；两端 profile 行均来自真实订阅列表（Iced `profile.profiles`，Bevy `LastProfilesProjection`）；测试：契约 3 项、Iced `the_palette_lists_the_shared_catalogue_and_wraps_like_bevy`/`the_palette_executes_shared_targets`/`the_open_palette_owns_the_arrow_keys`/`the_keyboard_chord_dispatches_through_the_shared_registry`、Bevy `test_palette_mounts_and_unmounts_from_the_shared_catalogue`/`test_palette_keyboard_navigation_typing_and_close`/`test_palette_row_click_executes_that_row`/`test_command_palette_action_execution_dispatches_route_and_command`/`test_command_palette_pinyin_matches_the_same_rows_as_iced`。**局部呈现差异（保留）**：Iced 保留本地 `Route::Editor`（Bevy 无编辑器页），并把共享 `ShellPage::Logs` 映射到承载日志分区的 runtime 页；目录只收录双端均可执行的产品指令（两端为此移除/新增不了伪指令）；拼音匹配使用共享区域词表引擎，非完整拼音词典 |
| `DUAL-15-06` | 全局系统快捷键管理与冲突检测 | `parity-ready` | 共享契约 `infiltrator-contract/src/shortcuts.rs`（`ShortcutAction` 5 项、`ShortcutChord::parse/matches/display_string`、`ShortcutRegistry::bind_or_replace/find_conflict/detect_conflicts/normalize/reset_action`）；应用层 `infiltrator-application/src/shortcut_application.rs`（capture/set_enabled/reset_action/reset_all 持久化到 `AppSettings.shortcuts`，冲突返回 `InvalidInput`），设置写入路径 `command_application.rs` 的 `shortcut.<id>` 分支（`update_setting`）；Iced 设置页卡片改为消费共享注册表（`view/settings.rs` `hotkeys_card`，捕获/启用/恢复默认），派发 `update/shell.rs::handle_keyboard_chord`，冲突弹 Warning toast，持久化 `shortcuts_store.rs`→`ShortcutApplication`；Bevy `shortcuts.rs`（`ShortcutBindings`/`HotkeyCapture`/`ChordPressed`，真实 `ButtonInput<KeyCode>` 键盘源 + `ShortcutChords` 派发 + 捕获经 `UiCommand::UpdateSetting("shortcut.<id>")` 落库，冲突弹 toast）；双端测试：契约 8 项、应用 4 项（含 `a_stored_custom_chord_blocks_later_captures`）、`command_application_settings_tests.rs::shortcut_capture_persists_and_rejects_conflicts`、Iced `multimodal_shell_tests.rs` 4 项、Bevy `shortcut_tests.rs` 7 项（含 `a_bound_chord_reaches_the_command_sink`、`capture_rebinds_through_the_shared_settings_command`） |
| `DUAL-15-07` | 移动端触控手势引擎 | `planned` | 仅 widget 层纯实现 + widget 自测：`infiltrator-bevy-widgets/src/gesture.rs`（`GestureRecognizer:61`、`PullToRefreshState:156`、`SwipeToActionItem:203`、`SafeAreaInsets:13`）与 `tests/headless/advanced_ecosystem_tests.rs`；`bevy-ui`/Android host 零引用，无共享契约，保持 `planned` |
| `DUAL-15-08` | 低功耗 Reactive 调步渲染 | `planned` | Bevy 有真实页面级节流器（`infiltrator-bevy-ui/src/pipeline.rs:150 MultiPageCadenceGovernor`，可见性/焦点/低功耗三态），widget 层另有 `cadence.rs:41 CadenceGovernor`（零引用）；Iced 仅在动画/拓扑流动时开帧订阅（`subscription.rs:387 window::frames()`），无后台降频；两端无共享契约、口径不同，保持 `planned` |
| `DUAL-15-09` | 深浅与多主题系统级跟随 | `parity-ready` | 共享契约 `infiltrator-contract/src/theme.rs`（`ThemeSkin` 四皮肤、`ThemePreference::parse_strict/resolve/next`）；应用层 `update_setting` 严格校验并落盘规范值（`command_application.rs`）；Iced 修掉 `system` 回落 Dark 的缺陷并新增真实 OS 订阅 `iced::system::theme()/theme_changes()`（`app.rs` 启动任务、`subscription.rs`）+ `Subscription` 分发到 `update/shell.rs::SystemThemeChanged`，设置页分段器（`view/settings.rs`）恢复可用（`Message::SetTheme` 挂回 update 分组）；Bevy 四皮肤 token（`bevy-widgets/src/theme.rs` `forest()/amoled()` 对齐 Iced 参考值）+ winit `Window::window_theme` 跟随（`bevy-ui/src/appearance.rs::sync_system_appearance`）+ `ThemeMode` 持有共享偏好；双端测试：契约 7 项、Iced `multimodal_shell_tests.rs` 4 项（含 `a_system_preference_survives_the_settings_round_trip`）、Bevy `theme_skin_tests.rs` 4 项（`widget_skin_mirror_matches_the_shared_contract` 镜像漂移断言、`the_shell_follows_the_os_appearance_while_preference_is_system` 系统跟随） |
| `DUAL-15-10` | AccessKit 无障碍语义全覆盖 | `planned` | Bevy 有部分语义节点（`bevy-ui/src/app.rs` `window/header/toggle/nav/region_semantic_node`、`command_palette.rs:373 dialog`），Iced 全仓零 `accessibility`/`AccessKit` 接入；无共享语义契约、无覆盖矩阵测试，保持 `planned` |
| `DUAL-15-11` | IME 中文输入法候选框定位 | `planned` | 全仓无 IME/候选框/组合事件处理（`grep -i "input_method\|candidate\|ime"` 仅命中无关词），两端均未接线，保持 `planned` |
| `DUAL-15-12` | Toast 消息队列防重与敏感信息脱敏 | `parity-ready` | 共享契约 `infiltrator-contract/src/toast.rs`（`ToastSeverity` 四态、`ToastPolicy{max_visible,dedup_window_ms}`、`ToastGate::admit/live_offers`）；脱敏沿用共享引擎 `infiltrator_domain::redact::redact_line`（Iced `utils::sanitize_ui_text`、Bevy `toast.rs::ToastPolicyGate::push`）；Iced 单一入口 `update/shell.rs::push_toast`（脱敏→共享去重门→容量裁剪→按 id 移除的过期任务，`RemoveToast(u64)`），`toast_state::ToastManager` 改为共享策略壳；Bevy `toast.rs`（`ShellToast` 摄入消息 + `ToastPolicyGate` + `sync_toast_stack` 挂载 `toast_stack_scene`），并修复 `ToastQueue::default()` 容量 0 的 push panic（`bevy-widgets/src/toast.rs` `DEFAULT_TOAST_CAPACITY`）与模式切换失败注入路径（`app.rs::drain_mode_ack`）；双端测试：契约 6 项、Iced `multimodal_shell_tests.rs::identical_toasts_are_coalesced_and_the_stack_is_capped` + `business_flow/settings_lifecycle.rs::toast_lifecycle_redacts_secrets_and_survives_stale_removal`、Bevy `toast_overlay_tests.rs` 4 项（`a_sensitive_toast_is_redacted_before_it_renders`、`the_overlay_mounts_a_single_stack_root`） |
| `DUAL-15-13` | 桌面无边框窗口拖拽与原生阴影 | `planned` | Iced 启动窗口用默认装饰（`desktop_composition.rs:90 window::Settings` 未设 `decorations`/拖拽区），全仓无 `window::drag`/无边框拖拽接线；Bevy `windowing.rs` 只有 `DockPanel/PipOverlayState`（`:19`/`:43`），非无边框拖拽；`display_adapter` 的 `WindowMaterial/MacosAppearance` 材料契约（`:171`/`:219`）零引用，保持 `planned` |
| `DUAL-15-14` | 双端设计规范与交互质感对齐 | `shared-ready` | 外观（`contract::theme`）、通知（`contract::toast`）、目录（`contract::command_catalogue`）与 HUD 读模型（`contract::mini_hud`）均已共享契约；本轮补上命令面板与 Mini HUD 的 1:1 语义：目录分类同时携带 Iced i18n key（`CommandCategory::label_key`）与 Bevy 裸中文（`label_zh`），HUD 开关状态字母两端同源 `SystemToggleState::compact_label`，模式/出口/速率均来自同一读模型，加速键提示由共享注册表实时渲染；但托盘（15-02）、手势/节流（15-07/08）、无障碍与 IME（15-10/11）、无边框（15-13）仍未双端闭环，1:1 质感对齐未达成 |
| `DUAL-15-15` | 多模态与外壳架构无头测试矩阵 | `shared-ready` | 已闭环项各有双端无头断言（15-06/09/12 见上；本轮补 03/04/05：契约目录/几何、应用 placement/`mini_hud.*` 写入、Iced 面板与 HUD 模型/拖拽/pin，Bevy 面板挂载/键盘/行点击/执行与 HUD 挂载/读模型/pin 落盘）；但 15-01 之外的 02/07/08/10/11/13 无测试面，矩阵未 100% |

> **2026-09-22 组 15 批次 A**：`DUAL-15-06/09/12` 收口为 `parity-ready`。
> 新增共享 `infiltrator-contract::theme`（四皮肤 + `system` 偏好严格解析）、
> `infiltrator-contract::shortcuts`（和弦语法、5 个产品默认绑定、冲突检测、规范化）、
> `infiltrator-contract::toast`（严重度 + 去重窗口/容量策略门）、
> `infiltrator-application::shortcut_application`（捕获/启停/重置 + 持久化）；
> 修复 Iced `system` 主题静默回落 Dark、`Message::SetTheme` 未挂分组导致设置页主题
> 选择器 no-op、Toast 按 index 移除在淘汰后误删、`ToastQueue::default()` 容量 0 panic；
> Bevy 首次获得真实键盘热键派发（`ButtonInput<KeyCode>` → 共享注册表 → `UiCommand`）
> 与挂载的 toast overlay。05/14/15 为 `shared-ready`；02/03/04/07/08/10/11/13 因宿主事实
> 或双端面缺失保持 `planned`（不伪造速率徽标、悬浮窗坐标、手势与无障碍覆盖）。
> 守卫 `scripts/quality/multimodal-shell-guard.py`。


> **2026-09-22 组 15 批次 B**：`DUAL-15-05` 收口为 `parity-ready`，`DUAL-15-03/04` 收口为
> `shared-ready`（不宣告悬浮窗双端独立窗口已达）。
> 新增共享 `infiltrator-contract::command_catalogue`（`ShellPage` 11 页 / `CommandCategory`
> 5 类 / `CommandTarget` 12 种 / `CommandCatalogue::with_profiles` + 共享子串过滤 +
> `shortcut_action` 复用和弦派发词汇）与 `infiltrator-contract::mini_hud`
> （`MiniHudReadModel` 真实读模型 + `MiniHudPlacement`/`MiniHudDisplay`/贴边吸附纯几何 +
> `MiniHudHostOutcome` 诚实宿主结果）；应用新增 `infiltrator-application::mini_hud_application`
> 与 `CommandApplication::update_setting` 的 `mini_hud.x/y/pinned` 校验写路径；端口新增
> `MiniHudWindowPort` 与 `HostRuntime::mini_hud_window_port`（未注入即 typed unsupported）；
> 设置快照新增 `SettingsPageSnapshot.mini_hud`，双端读取同一 placement。
> Iced：命令面板改由共享目录驱动（删除本地 `CommandAction`/`CommandCategory`/`CommandItem`，
> ↑↓ 首尾循环、加速键实时渲染、`ExecuteCommand` 逐目标同步派发），Mini HUD 增加真实
> 读模型、整卡拖拽、`iced::window::move_to`/`Level::AlwaysOnTop`/260x90 窗口模式与
> `mini_hud_store` 落盘；Bevy：命令面板首次真实挂载（BSN 弹窗签名 latch + 键盘缝
> ↑↓/Enter/Esc/Backspace/字符输入 + 行点击 + 与 Iced 对齐的目标派发），Mini HUD 首次
> 真实挂载并删除硬编码 `"RULE"`/`"系统代理: 开启 · TUN: 开启"`，`Ctrl+Alt+M` 与
> 调色板行派发到 `ToggleMiniHud`，pin 经 `mini_hud.pinned` 落盘。
> 诚实偏差：Iced 的 260x90 置顶窗仍是宿主主窗口（无多窗口宿主）；Bevy HUD 为窗口内浮层、
> 无拖拽/移动窗口路径；桌面宿主尚未实现 `MiniHudWindowPort`。守卫
> `scripts/quality/multimodal-shell-guard.py`（批次 B 段）已随本批扩展；
> `system-toggle-guard` 的 HUD 断言改指共享读模型路径。测试隔离修复：
> `subscription:<profile>` 凭据落在机器级 OS keyring，`business_flow` 订阅旅程
> 并发时会互删同一 key，`.config/nextest.toml` 新增 `keyring-serial` 串行组
> （仅串行该测试组，其余保持 4 并发），全仓 2714 测试 0 跳过通过。


全仓按 10 大核心业务组划分，双端必须具备对等功能支撑：

| 业务组 | 核心能力清单（功能并集） | 对标竞品来源 | 核心底层模块 |
| :--- | :--- | :--- | :--- |
| **01. 核心与网络栈**<br>(Core Runtime & TUN) | ① 内核多代际生命周期、平滑热重载与崩溃看门狗<br>② 多版本交付（Stable/Alpha/Meta）校验与回滚<br>③ 服务模式 (Service Mode/Privileged Helper) 提权守卫<br>④ TUN 模式多网络栈调度（gVisor/System/Mixed/LWIP）<br>⑤ 系统代理抢占探测与断电异常自愈<br>⑥ 局域网共享 (Allow-LAN) 混合端口与账密认证<br>⑦ IPv6 内核解析与转发拓扑开关 | Clash Verge Rev<br>Mihomo Party<br>Surge | `mihomo-platform`<br>`infiltrator-desktop`<br>`infiltrator-core::flow_control` |
| **02. 概览遥测中枢**<br>(Overview & Telemetry) | ① 双通道 GPU 实时流量波形（动态量程自适应）<br>② 分流链路可视化拓扑链（Inbound→Sniffer→Rule→Group→Outbound）<br>③ 主活动出口卡片（国旗徽标/协议/延迟/IP）<br>④ 订阅配额进度条（超 85% 动态预警、重置倒计时）<br>⑤ 系统代理与 TUN 模式双主控大卡<br>⑥ 代理模式即时分段器（Rule/Global/Direct/Script）<br>⑦ 6 项核心指标网格（连接/内存/CPU/速率/累计流量）<br>⑧ 公网 IP 隐私与地理位置多源探针 | Mihomo Party<br>Clash Nyanpasu<br>Flclash | `mihomo-api`<br>`infiltrator-core::dns_tester`<br>`infiltrator-bevy-widgets::chart` |
| **03. 代理与智能节点**<br>(Proxies & Protocol Matrix) | ① 协议全矩阵保真（SS 2022/VLESS Reality/Trojan/TUIC v5/Hy2/WireGuard/AmneziaWG/AnyTLS/SSH/Snell）<br>② 策略组全覆盖（Selector/URLTest/Fallback/LoadBalance一致性哈希/Relay链式中继）<br>③ 并发测速信号量流控（防止网络风暴与熔断）<br>④ 拼音首字母/中文/协议多维模糊搜索过滤<br>⑤ 节点死链一键隐藏、星标置顶与偏好锁定<br>⑥ 单节点历史延迟 Sparkline 折线走势<br>⑦ 自定义节点表单向导与通用 URI 导入导出<br>⑧ 前置跳板代理 (Dialer-Proxy) 拓扑编排 | Mihomo Party<br>Flclash<br>Clash Verge Rev | `infiltrator-core::profile_converter`<br>`mihomo-api::proxy`<br>`infiltrator-core::flow_control` |
| **04. 配置与脚本生态**<br>(Profiles & Scripting) | ① 多渠道导入（URL/本地/剪贴板）与自定义 User-Agent<br>② 订阅定时自动轮询更新、条件请求（ETag）与防重入<br>③ 多订阅节点聚合器（多源去重、按国家自动成组）<br>④ YAML AST 语法高亮编辑器、代码片段与行号报错定位<br>⑤ QuickJS 沙箱脚本控制台（带 64MB 内存熔断与实时变换）<br>⑥ 多级配置覆写管道（Base→Subscription→Merge Rules→Mixin）<br>⑦ 配置快照历史与可视化行内/并排 Diff 回滚 | Clash Verge Rev<br>Mihomo Party<br>Flclash | `infiltrator-domain::subscription`<br>`infiltrator-core::subscription_io`<br>`infiltrator-core::script_engine`<br>`infiltrator-core::filter_pipeline` |
| **05. 分流规则与链路沙盒**<br>(Rules & Live Tracer) | ① 28+ 规则类型全覆盖（DOMAIN/IP-CIDR/PROCESS/GEO/DSCP/UID等）<br>② 逻辑规则 (AND/OR/NOT/SUB-RULE) 递归构建与解析<br>③ 交互式实时分流追踪器（输入目标/进程回放决策链路）<br>④ MRS 官方二进制规则集高性能本地索引与 Diff<br>⑤ Rule-Provider 外部规则集全生命周期管理<br>⑥ 规则命中计数统计与死规则静态诊断<br>⑦ 可视化拖拽调整规则优先级 | Mihomo Party<br>Clash Verge Rev | `infiltrator-core::rules`<br>`infiltrator-core::mrs`<br>`infiltrator-core::sub_rules` |
| **06. 连接审计与深度透视**<br>(Connections & Telemetry) | ① 高并发实时连接流式采集（源/目/进程/规则/出站/速率）<br>② 三维聚合视图（扁平流/按进程聚合/按目标域名聚合）<br>③ 连接详情侧滑抽屉（耗时瀑布流：DNS/TCP/TLS/TTFB）<br>④ 目标 IP、ASN 归属机构与地理情报透视<br>⑤ 连接实时治理（单条阻断/过滤范围阻断/全部关闭）<br>⑥ 从连接反向一键创建分流规则向导 | Surge<br>Clash Nyanpasu<br>Mihomo Party | `mihomo-api::connection`<br>`infiltrator-core::idle_connection_sweeper` |
| **07. 日志与自愈体检**<br>(Logs, DNS & Doctor) | ① 环形流式日志（4 级过滤、Regex 检索、滚屏锁定、导出）<br>② DNS 工作台（DoH/DoT/DoQ/HTTP3 配置与回退策略）<br>③ Fake-IP 映射池实时检视与一键清缓存<br>④ DNS 泄漏与 WebRTC 泄漏多源交叉探测<br>⑤ Doctor 深度自愈套件（端口冲突释放、TUN 驱动核查、系统代理注册表修复、内核连通性探针） | Surge<br>Shadowrocket<br>Clash Verge Rev | `mihomo-api::log`<br>`infiltrator-core::dns_tester`<br>`infiltrator-core::doctor` |
| **08. 应用级分流与系统穿透**<br>(Per-App Routing) | ① 操作系统动态进程枚举（Browser/Game/Dev/Media/System 分类）<br>② 应用高清图标提取与本地高性能缓存<br>③ 单应用三态分流设置（代理/直连/拦截）<br>④ Windows UWP 应用回环隔离豁免工具<br>⑤ Android 移动端分应用代理黑白名单 | Mihomo Party<br>Surge Mac/iOS | `infiltrator-desktop::process_enumerator`<br>`android::vpn` |
| **09. 多端云同步与安全备份**<br>(Cloud Sync & Security) | ① 多云端协议支持（WebDAV 坚果云/Nextcloud、GitHub Gist、iCloud）<br>② 精确到字段的三向差异合并 (3-Way Merge) 与冲突解决<br>③ 全量配置数据端到端强加密 (AES-256-GCM)<br>④ 配置变更后自动化静默同步 | Flclash<br>Mihomo Party | `mihomo-dav-sync/*`<br>`infiltrator-core::sync` |
| **10. 系统集成与多端沉浸**<br>(System Integration & UX) | ① 动态系统托盘菜单（上下行速率徽标/模式切换/系统代理开关）<br>② 桌面独立极简迷你悬浮窗 (Mini HUD: 260x90 置顶小窗)<br>③ 全键盘命令面板 (Command Palette - Ctrl+K 全局直达)<br>④ 全局系统快捷键设置与冲突规避<br>⑤ 宽屏桌面/平板导轨/移动端底栏+抽屉三模态自适应 | Clash Verge Rev<br>Flclash<br>Clash Nyanpasu | `infiltrator-desktop::tray_badge`<br>`infiltrator-bevy-widgets::windowing`<br>`infiltrator-shared::locales` |

---

## 四、分业务组【UI表现与交互体验清单】（视觉与动效对齐规范）

两套前端必须遵循完全一致的交互规范、动效节奏与视口响应能力：

| 业务组 | 视口响应与布局架构 | 交互动效与手势规范 | 状态回显（加载/空态/骨架/错误） |
| :--- | :--- | :--- | :--- |
| **01. 核心网络栈** | 桌面端卡片网格，移动端单列列表；控件居右对齐。 | Switch 具备 120ms 弹性滑动；提权引导模态框平滑淡入。 | 切换中显示骨架微光 Spinner；提权失败弹出橙色警告条与排查日志入口。 |
| **02. 概览遥测** | 顶部流量卡 + 拓扑链；中部指标网格；底部出口卡片与配额条。支持纵向拖拽重排。 | GPU 贝塞尔波形 60 FPS 平滑滚动；拓扑节点随流量动态微光流动。 | 核心启动中淡入整页骨架屏；配额超 85% 转警戒黄；无订阅时显示本地卡片。 |
| **03. 代理矩阵** | 响应式流体网格（自适应 2~4 列）；支持一键切换紧凑单列列表。 | 鼠标悬停卡片上浮 2px + 微阴影；测速按钮带环形旋转进度条；支持拖拽调整策略组顺序。 | 测速中卡片内延迟数值显示脉冲波纹骨架屏；超时标红/置灰；收藏节点带金色高亮星标。 |
| **04. 配置脚本** | 双栏/单栏自适应；代码编辑区自适应撑满，底栏集成诊断抽屉。 | 代码编辑器行号联动高亮；格式化与插入 Snippet 平滑滚动；Diff 增删分色对比。 | YAML 语法错误在出错行呈现红色波浪线与行号红标；QuickJS 实时流式回显日志。 |
| **05. 规则追踪** | 50,000+ 条目虚拟视口滚动 ($O(1)$ 几何裁剪)；顶部即时搜索栏与 Tracer 抽屉。 | Tracer 树状回放分流决策链路，命中行高亮闪烁；规则条目支持拖拽抓手调整。 | 视口滚动保持 60 FPS 零卡顿；命中计数实时跳动；死规则呈现置灰与删除提示。 |
| **06. 连接审计** | 表格与卡片自适应；右侧可展开宽度 420px 的 Slide-out 侧滑下钻抽屉。 | 侧滑抽屉平滑滑出带有半透明遮罩；耗时瀑布流按阶段使用比例条呈现。 | 断开连接呈现淡出删除动效；高吞吐连接行带脉冲微光；无连接显示优雅占位插画。 |
| **07. 日志自愈** | 终端风格等宽排版；悬浮日志锁定浮钮；底部集成 DNS 与健康卡片。 | 向上滚动自动解除吸底并浮现“回到最新”悬浮胶囊；Doctor 一键自愈带阶梯打勾动画。 | 日志级别分色（ERR 红, WRN 黄, INF 绿, DBG 灰）；体检项按绿（通过）、黄（警告）、红（严重）标识。 |
| **08. 应用分流** | 应用高清图标 + 进程名网格，支持搜索与分类标签栏切换。 | 代理/直连/拦截使用 Segmented Control 分段器滑动胶囊；图标异步平滑淡入。 | 图标提取中显示圆形骨架占位；系统保护进程禁用修改并附 Tooltip 说明。 |
| **09. 云端同步** | 凭据配置表单 + 同步历史列表；冲突时弹出全屏对比对话框。 | 点击“立即同步”图标旋转；冲突解决支持左右卡片点击单项采纳合并。 | 同步中展示旋转指示；同步成功弹出绿色 Toast；冲突时黄色警示横幅阻断提交。 |
| **10. 系统集成** | 悬浮窗 (260x90) 极简圆角无边框；Command Palette 居中悬浮顶层面板。 | `Ctrl+K` 弹出面板带 80ms 快速淡入与焦点自动聚焦；悬浮窗全区域平滑拖拽与贴边吸附。 | 键盘上下键平滑切换选择项；无匹配时友好提示；悬浮窗根据网络状态自适应微波形。 |

---

## 五、实施排期与双端同步推进路线图（Four Waves）

双端演进划分为 4 个推进批次，每批次以**双端同步验收**为准入条件：

```
Wave 1: 双端框架与核心主干对齐 [A-01～A-05 架构前置已验收，业务 parity 持续]
  ├─ Bevy 11 页面路由/场景骨架与 RouteHistory 已存在
  ├─ Iced 与 Bevy 均可由 host 注入同一个 application-owned surface pump
  └─ 核心生命周期、系统代理、TUN、节点和测速按 shared contract 收口

Wave 2: 核心遥测、透视与诊断闭环 [shared surface 已接通，业务项逐项验收]
  ├─ Bevy 11 页与 Iced surface model 已接入 typed snapshot/event/status 边界
  ├─ 连接、日志、DNS、Doctor、Tracer 的具体 live 数据仍按业务项补齐
  └─ 波形、拓扑、Mini HUD、Command Palette 以双端测试和宿主证据重新验收

Wave 3: 高级扩展、配置工程与应用分流 [目标已定义，未计入完成]
  ├─ Iced 现有能力先抽成 shared/application + UI adapter
  ├─ Bevy 同批次接入 live projection、命令结果和错误状态
  └─ 聚合器、AST/Diff、脚本、应用分流必须各自完成双端测试

Wave 4: 规则集深度治理、云端同步与多模态大一统 [A-01～A-05 后的 0.30 当前主线]
  ├─ shared surface 架构前置已完成，按 DUAL 项目进入双端实现
  ├─ MRS、WebDAV、Rail 和响应式能力按两端 live parity 重新记账
  └─ 移动触控、VPN host、低功耗调度进入 Bevy mobile + Android host 联合验收
```

---

## 六、全仓分散与重复文档治理收敛图谱

为彻底消除文档分散、口径陈旧和重复描述，现对全仓技术与规划文档建立清晰的权威归属矩阵：

| 文档路径 | 当前状态与问题 | 治理方案与收敛动作 | 权威定位与维护原则 |
| :--- | :--- | :--- | :--- |
| **`docs/DUAL_SURFACE_PARITY_MASTER_PLAN.md`** | **[当前主控]** | **全仓最高主控台账**：纳管双端同步策略、15 大业务组/225 项功能并集、UI 表现清单与 Wave 路线图。 | **唯一权威主纲**，所有其他前端与差距文档均向其链接收敛。 |
| **`docs/FRONTENDS.md`** | 描述多端关系，但提及已退役的 Tauri，且描述 Bevy UI 滞后跟随。 | 更新内容：声明 Tauri 退役；确立 Iced 与 Bevy UI 为双主干同步表面；链接至主控规范。 | 多前端架构边界与跨端决策标记（shared/local）的权威定义。 |
| **`docs/FUNCTIONAL_MAP.md`** | 列出功能域与 owner；双端对等规则已写入，但 live parity 需以审计台账为准。 | owner 聚焦于 domain/application/ports，入口聚焦于 Iced 与 Bevy UI 双端。 | 业务功能唯一 Rust owner 的权威检索入口。 |
| **`docs/ICED_CORE_MATURITY_GAPS.md`** | 仅记录 Iced 的 4 维度与 Wave 1~5 落地项，与 Bevy 隔离。 | 头部增补索引指引：明确本台账为 Master Plan 在 Iced 前端的具体落地执行切片。 | Iced 侧代码实现、组件与测试证据的追溯台账。 |
| **`docs/BEVY_CORE_MATURITY_GAPS.md`** | 记录 Bevy 的 10 维度 150 项工程缺口，未显式与 Iced 对齐。 | 头部增补索引指引：明确 10 维度与 Master Plan 10 大业务组 1:1 对齐，作为 Bevy 落地切片。 | Bevy 侧场景、组件与无头测试证据的追溯台账。 |
| **`docs/MATURITY_GAP_ANALYSIS.md`** | 记录 10×10 内核与协议差距，偏重后端逻辑。 | 明确其定位为“核心层成熟度台账”，将 UI 表现层与双端同步要求引流至 Master Plan。 | `infiltrator-core` 与 `mihomo-*` 协议与配置 AST 的底层权威台账。 |
| **`docs/TEN_PHASE_ROADMAP.md`** | 记录 10 阶段工程任务，部分与 GAP_ANALYSIS 重叠。 | 保留历史演进追踪，头部声明其与当前 Wave 1~4 的映射关系。 | 历史演进里程碑与代码下沉过程的事实记录。 |
| ~~`iced_todo.md`~~ (根目录) | 已删除（内容冗余，仅为重定向）。 | — | — |
| **`docs/README.md`** | 缺少最新台账导航与阅读顺序。 | 更新阅读顺序与权威关系表，将 Master Plan 纳为核心架构第一入口。 | 文档中心主索引。 |
