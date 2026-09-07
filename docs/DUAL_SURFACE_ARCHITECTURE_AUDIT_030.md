# 0.30 双 UI 底层架构审计

审计日期：2026-09-05  
审计分支：`main`（A-01～A-05 架构前置工作树）
审计范围：`infiltrator-application`、`infiltrator-contract`、`infiltrator-ports`、`infiltrator-composition`、`infiltrator-desktop`、`infiltrator-iced`、`infiltrator-bevy-ui`、`infiltrator-bevy-widgets`

## 结论

结论必须分成两层说：

1. **核心分层原则已经清楚**：领域、契约、端口、应用、组合根和宿主适配器的方向已经成立；`infiltrator-application` 的生产代码没有直接依赖 Tokio，Bevy UI 也没有直接依赖 Tokio、Reqwest 或 Mihomo client。
2. **A-01～A-05 架构前置现已完成**：Iced 有明确的 desktop composition/host namespace，11 页 shared surface contract 已落地，Bevy 生产路由不再制造 demo projection，真实 desktop reader/pump 已可组合，parity guard 与双端交付模板已进入 CI。
3. **功能成熟度仍然不能冒充完成**：日志/Doctor 等页面在没有对应事件或用户触发结果时显示 typed `loading`，225 项业务清单仍须逐项完成 shared、Iced、Bevy 和宿主验收；架构前置完成不等于 225 项完成。

这不是否定现有工作，而是把“架构已成立”和“产品已完成”从同一个状态中拆开。后续所有 0.30 任务都以本文件和 [双端主控计划](DUAL_SURFACE_PARITY_MASTER_PLAN.md) 为准。

## 已确认的底层边界

```text
┌──────────────────────────────────────────────────────────────┐
│ Iced UI adapter                 Bevy UI adapter               │
│ View/Update + Iced Task         Scenes/Systems + Bevy ECS     │
│ 只处理本地 view state            只处理 render projection       │
└───────────────┬──────────────────────────┬───────────────────┘
                │ CommandIntent / Snapshot / Event / Capability
                ▼                          ▼
       ┌──────────────────────────────────────────────┐
       │ infiltrator-application                       │
       │ use-case、事务、生命周期、单一 Core 状态源      │
       │ 只依赖 infiltrator-ports                       │
       └──────────────────────┬───────────────────────┘
                              │ ports
             ┌────────────────┴────────────────┐
             ▼                                 ▼
   infiltrator-domain                 infiltrator-contract
   纯模型、算法、校验                  跨端命令、结果、事件、快照
   无 UI / OS / Tokio                 稳定、可序列化、无 toolkit
                              │
                              ▼
       composition roots + outbound/host adapters
       mihomo-api / mihomo-config / mihomo-version
       infiltrator-http / core / desktop / android / ios
```

必须坚持以下解释：

- `Tokio` 可以存在于 composition、HTTP/outbound adapter 和 host adapter；它不能成为 domain、contract、ports、application 的事实模型，也不能从跨端公开 API 泄漏出去。
- Iced 或 Bevy 可以拥有各自 toolkit 的渲染调度和本地 view state，但不能各自拥有一套 Core 事实、订阅更新器、配置写入器或 Mihomo client。
- `infiltrator-desktop`、`infiltrator-android`、`infiltrator-ios` 是与 UI 正交的同级 host adapter。Bevy Android 只是 UI surface，Android VPN 生命周期仍属于 Android host。
- “双端一视同仁”指 shared intent、结果、错误、能力、revision/generation、可达功能一致；窗口、托盘、手势、权限和布局可以是 `local` 或 `accepted difference`。

## 当前代码证据

### 通过项

- `infiltrator-application` 生产依赖树不含 Tokio；Tokio runtime 由 `infiltrator-composition` 注入。
- `infiltrator-domain`、`infiltrator-contract`、`infiltrator-ports` 的具体 UI/传输依赖守卫通过。
- `infiltrator-bevy-ui` 的生产依赖和源码不直接构造 `MihomoClient`、Reqwest 或 Tokio；`controller` 通过 application-owned Overview pump 接入。
- Bevy 的命令入口已经存在：页面提交 `UiCommand`，生产 sink 转为 `CommandIntent`，再交给 `CoreApplication`。

### A-01～A-05 验收状态

| 闸门 | 状态 | 代码/文档证据 |
| --- | --- | --- |
| A-01 Iced host/composition split | `parity-ready` | `infiltrator-iced::desktop_composition` + `infiltrator-iced::host`；UI 模块不再直接触达 desktop/admin concrete path；`run_with_surface_pump` 是显式组合入口 |
| A-02 11-page shared contract | `parity-ready` | `infiltrator-contract::surface_snapshot`、`infiltrator-ports::surface`、Iced `SurfaceModel`/`SurfaceBridge`、Bevy `SurfaceSource`/`SurfaceDrainPlugin` |
| A-03 Bevy live source | `parity-ready` | `ApplicationSurfaceReader`、`SurfacePump`、desktop composition、Bevy `SurfaceDrainPlugin`；生产 route 无 `Projection::demo()` |
| A-04 fail-closed parity guard | `parity-ready` | `scripts/quality/parity-guard.py` 已接入 workspace/Bevy/test CI 入口 |
| A-05 delivery template | `parity-ready` | `docs/DUAL_SURFACE_DELIVERY_TEMPLATE.md`，每项强制 shared + Iced + Bevy + tests + host evidence |

### 后续功能开放项

- 225 项业务项的 live 数据与实际交互仍需逐项接入 `ApplicationSurfaceReader`/事件流，并完成两端 headless/host evidence。
- 日志流和 Doctor 结果按事件/命令生命周期更新，不能由遥测轮询伪造成功数据。
- Iced 仍可使用 Iced toolkit 自身的 executor；这不等于 application/domain 获得 Tokio 依赖。
- 业务功能完成度继续以 [双端主控计划](DUAL_SURFACE_PARITY_MASTER_PLAN.md) 的 `parity-ready` / `host-verified` 口径记账。
- `DUAL-01-01` 已按 desktop Linux host evidence 完成；后续项目必须沿用同一 token/generation 和 orphan ownership 语义，不得重新引入页面私有生命周期。
- `DUAL-01-02` 已完成 desktop Linux、Android 和 iOS host composition contract evidence：热重载不创建新 generation，失败恢复到同一 session 的 Running；发布版本仍需连接真实 controller 做 smoke。
- `DUAL-01-03` 已完成 shared/application、Iced、Bevy 和 host scheduler contract evidence：异常退出进入 Waiting，按指数退避重启，连续失败后 Tripped；真实发行包/真实 mihomo 异常退出恢复仍未计入 `host-verified`。
- `DUAL-01-04` 已完成三通道 shared/application 与两端 Settings contract evidence：Stable、Alpha（`Prerelease-Alpha`）和 Meta-Core 不再互相折叠；桌面 host 注入在线探测，Android/iOS 对核心二进制安装保持 typed unsupported；真实发行包 smoke 尚未计入 `host-verified`。
- `DUAL-01-05` 已完成下载前 digest gate、安装失败清理和双端完整性状态投影；没有官方 SHA256 或 digest 不匹配时不会解压、写盘或进入版本目录，供应链/发行包证据仍待后续 `host-verified` 流水线。
- `DUAL-01-06` 已完成本地版本历史栈和双端回滚 contract：候选 binary 先通过 `-v` 检查，版本指针以原子写入更新且不覆盖 profile 元数据；运行中重启与发行包证据仍未冒充 `host-verified`。
- `DUAL-01-07` 已完成 controller secret 自动生成/复用、EndpointSource 私有传递、REST/WebSocket Bearer 注入和双端脱敏认证状态投影；真实发行包 controller 启动与跨平台权限 smoke 仍未冒充 `host-verified`。
- `DUAL-01-08` 已完成 CoreLogLevel 的 shared command、PATCH 后回读、Iced Logs/Settings 和 Bevy Settings 双端控件；invalid/stopped 与 readback mismatch 均 fail closed，真实 mihomo controller smoke 仍未冒充 `host-verified`。
- `DUAL-01-09` 已完成 ServiceModeSnapshot/Port/Application、desktop 平台映射、post-check 和双端 Settings action；Linux Polkit argv、Windows `sc.exe` 与 macOS launchd/typed unsupported 均保留明确边界，真实授权发行包证据仍未冒充 `host-verified`。
- `DUAL-01-10` 已完成 termination handler、集中清理钩子、desktop host 注册以及 Iced 正常退出/panic 复用；Bevy 不直接管理 OS signal，而由其 host composition 持有同一 cleanup handle，真实多平台退出 smoke 仍未冒充 `host-verified`。
- `DUAL-01-11` 已完成 7890/9090 目标端口的 shared observation、owner parser、配置避让和双端 repair action；未确认的第三方 PID 不会被 UI 直接终止，跨平台真实占用/发行包证据仍未冒充 `host-verified`。
- `DUAL-01-12` 已完成 512 MiB 软限、application GC cooldown、Mihomo `/memory`/`/debug/gc`、desktop CPU adapter 和双端资源投影；高内存发行包压测及移动端 GC 能力仍未冒充 `host-verified`。
- `DUAL-01-13` 已完成本地 profile/core 预校验、offline-first retry/materialize、GeoIP 本地复制与缺失降级；`OfflineStartupSnapshot` 经 application surface reader 同步到 Iced/Bevy，Android/iOS bridge 只提供 typed host evidence，真实断网发行包 smoke 仍未冒充 `host-verified`。
- `DUAL-01-14` 已完成 `CoreLifecycleSnapshot` 与 `CoreLifecyclePort` 统一生命周期读模型；Iced 全阶段映射、Bevy `LatestCoreLifecycle` resource 和两端生命周期/会话测试均已闭环，真实双 UI 同屏宿主 smoke 仍未冒充 `host-verified`。
- `DUAL-01-15` 已完成双端 headless lifecycle matrix：失败启动、未知 owner 端口冲突、平滑停止和停止后的清理/会话状态均有明确断言；真实 GUI/发行包故障注入仍未冒充 `host-verified`。
- `DUAL-02-01` 已完成四项 TUN stack shared catalog、前三项 live PATCH+回读和 LWIP `ReferenceOnly` fail-closed；Iced/Bevy 控件与 desktop/Android/iOS command composition 已同步，真实 TUN/VPN 发行包 smoke 仍未冒充 `host-verified`。
- `DUAL-02-02` 已完成物理链路 MTU host port、domain 开销/MSS 计算、live `tun.mtu` PATCH+GET readback、5 秒 surface cache 和 Iced/Bevy 双端状态/行为测试；Android/iOS 无 native 链路指标时保持 typed unsupported，真实多网卡漫游、VPN/发行包与移动原生链路 smoke 仍未冒充 `host-verified`。
- `DUAL-02-03` 已完成 `auto-route`/`strict-route` shared command、Settings snapshot、严格路由依赖自动路由的原子 PATCH+GET readback，以及 Iced/Bevy checkbox 与 projection 同步；真实系统路由表、VPN 权限和泄漏验证仍未冒充 `host-verified`。
- `DUAL-02-04` 已完成系统代理 shared port/application/snapshot、desktop Windows registry/Linux GNOME-KDE/macOS networksetup 适配、3 秒读取缓存与 Iced/Bevy checkbox/command 投影；真实桌面权限、第三方抢占和发行包 smoke 仍未冒充 `host-verified`。
- `DUAL-02-05` 已完成 system proxy desired ownership、host port 共享状态、3 秒 Iced watchdog/Bevy surface reconcile、外部修改自动复位和 repair warning projection；真实第三方抢占与桌面会话恢复 smoke 仍未冒充 `host-verified`。
- `DUAL-02-06` 已完成 system proxy previous/desired durable journal、owner PID/启动时间判定、启动孤儿恢复的 target-match/readback、外部修改跳过、正常退出恢复与 Iced/Bevy shared recovery projection；Android `VpnService` 与 iOS 无全局代理能力保持 typed unsupported，真实断电/会话注销/发行包恢复 smoke 仍未冒充 `host-verified`。
- `DUAL-02-07` 已完成 Allow-LAN 的 shared contract、`allow-lan`/`mixed-port`/`bind-address` 原子 PATCH+GET readback、IP/括号 IPv6 校验、Iced draft/Apply 代际回滚和 Bevy TextField/Apply intent；ACL/HTTP 认证留给下一项，真实多网卡、防火墙、移动 VPN ingress 与发行包 smoke 仍未冒充 `host-verified`。
- `DUAL-02-08` 已完成 LAN ACL/auth shared snapshot、纯 domain CIDR/凭据校验、四字段 PATCH+GET readback、密码脱敏与成功清理、Iced/Bevy 安全 draft/Apply projection，以及 desktop/Android `LanAccessControl` 支持和 iOS typed unsupported 证据；真实局域网客户端、凭据轮换、防火墙/移动 VPN ingress 与发行包 smoke 仍未冒充 `host-verified`。
- `DUAL-02-09` 已完成顶层 Mihomo `ipv6` 策略的 shared snapshot、缺失字段默认 true、live PATCH+GET readback 与 mismatch fail-closed、Iced/Bevy checkbox/上下文投影，以及 desktop/Android 支持和 iOS typed unsupported 证据；宿主全局 IPv6 sysctl、防火墙、双栈公网泄漏与 VPN ingress smoke 仍未冒充 `host-verified`。
- `DUAL-02-10` 已完成 UWP AppContainer shared snapshot、SID 纯校验、desktop 注册表/`CheckNetIsolation.exe` host port、扫描/单包/bulk application readback、Iced live snapshot 与 Bevy projection/commands；Android/iOS/非 Windows typed unsupported，真实 Windows 商店应用、UAC/企业策略和发行包 smoke 仍未冒充 `host-verified`。
- `DUAL-02-11` 已完成 PAC shared request/snapshot、live Mihomo 规则读取、domain 脚本编译校验、desktop loopback HTTP 服务和启停 readback、Iced/Bevy Apply/status projection，以及 desktop 支持和 Android/iOS typed unsupported；真实浏览器消费、系统代理联动、跨平台权限和发行包 smoke 仍未冒充 `host-verified`。
- `DUAL-02-12` 已完成物理网卡/默认网关 shared observation、domain 默认路由选择与迁移决策、application TUN auto-route 条件守卫、desktop Linux/macOS/Windows 参数化 route-anchor repair/readback、双 UI Settings 刷新/立即修复投影与行为测试；Android/iOS 在 native VPN/NetworkExtension 路由事实未接入前保持 typed unsupported，真实 Wi-Fi/有线漫游、VPN 泄漏、权限和发行包 smoke 仍未冒充 `host-verified`。
- `DUAL-02-13` 已完成 `VpnStartRequest/VpnSessionSnapshot`、VpnServicePort/Application、FD/MTU/路由/DNS/前台校验、Android bridge 的 native configuration/foreground seam、tun2proxy start/stop/readback、Android composition、Iced/Bevy Settings 启停投影与行为测试；desktop/iOS 保持 typed unsupported，真实 Android manifest、API 26–35 授权/前台限制、真机流量、断电与 `onRevoke` smoke 仍未冒充 `host-verified`。
- `DUAL-02-14` 已完成 `SystemToggleSnapshot/SystemToggleApplication`、Iced 侧栏/设置/Mini HUD 的统一投影与 pending policy、Bevy 侧栏动态 restamp/不可用禁用/Activate command observer，以及两端重复点击与 surface readback 行为测试；真实多会话桌面、移动原生开关联动和发行包视觉/触控 smoke 仍未冒充 `host-verified`。
- `DUAL-02-15` 已完成 `PrivilegedNetworkRequest/Snapshot/Port/Application`、domain 去重校验、注入/回读/清理/rollback 失败语义、surface typed unsupported、desktop optional host seam、Iced/Bevy Settings action/status projection 与 mock host adapter/headless tests；真实 root/polkit/UAC、Android/iOS 原生授权、系统网络副作用和发行包 smoke 仍未冒充 `host-verified`。
- `DUAL-03-01` 已完成 `TrafficSample/TrafficWaveformSnapshot`、application bounded live history、generation reset、停止态不追加、domain cubic-Bezier value projection，以及 Bevy chart/Iced Canvas 的 shared live sample adapter 与行为测试；真实 GPU/长时高吞吐/发行包视觉帧率 smoke 仍未冒充 `host-verified`。
- `DUAL-03-02` 已完成 `TrafficScaleSnapshot/TrafficScaleApplication`、双通道峰值/单位/headroom/tick policy、ApplicationSurfaceReader scale 输出、Bevy fixed scale/scale line/glow 与 Iced scale label/canvas glow；真实 GPU shader、长时峰值抖动、设备帧率和发行包视觉 smoke 仍未冒充 `host-verified`。
- `DUAL-03-03` 已完成 `TrafficTopologySnapshot/TrafficTopologyApplication`、domain 五段拓扑推导、真实 connection chain/rule/proxy/config 输入、aggregate flow 约束、Bevy `TopologyPlate` 动态粒子与 in-place restamp、Iced flow Canvas/phase update，以及两端状态/适配行为测试；Mihomo per-edge rate、真实 GPU/长时设备帧率、复杂 relay 与发行包视觉 smoke 仍未冒充 `host-verified`。
- `DUAL-03-04` 已完成 `TrafficTopologyNavigationTarget/TrafficTopologyNavigationApplication`、Iced stage button→shared page mapping→Elm Navigate、Bevy `TopologyStageButton`→Activate→RouteChanged、drawable gating 与两端导航行为测试；真实触控/读屏手势和发行包导航 smoke 仍未冒充 `host-verified`。
- `DUAL-03-05` 已完成 `ActiveExitSnapshot/ActiveExitApplication`、domain selected proxy group/node 推导、country code/protocol/delay/alive facts、ApplicationSurfaceReader 输出、Iced 高保真出口卡片、Bevy ActiveExitText in-place projection 与 shared behavior tests；真实 GeoIP/延迟刷新、复杂 relay、GPU/发行包视觉 smoke 仍未冒充 `host-verified`。
- `DUAL-03-06` 已完成 `SubscriptionQuotaSnapshot/SubscriptionQuotaApplication`、active profile provider quota 推导、used/total/remaining/expiry/next-update 字段、Warning/Critical/Exhausted/Expired 状态、reset 缺失 fail-closed、ApplicationSurfaceReader wiring、Iced/Bevy Overview quota 卡片与行为测试；真实 provider reset 字段、时钟漂移、长时刷新和发行包视觉 smoke 仍未冒充 `host-verified`。
- `DUAL-03-07` 已完成 Overview `SystemToggleSnapshot` fan-out、Iced `overview_master_switches`、Bevy `OverviewMasterSwitchButton`/Activate observer、两端动态 status/action restamp 与 shared policy command sink 行为测试；真实桌面权限、Android VPN 原生联动、触控和发行包 smoke 仍未冒充 `host-verified`。

## 0.30 架构收口顺序

以下五项是 225 项功能开发前的 P0 闸门：

| 闸门 | 收口动作 | 完成证据 |
| --- | --- | --- |
| A-01 | 将 Iced 的 boot、tray、Admin、文件/进程/系统代理 glue 与页面 view/update 分成明确的 UI adapter 和 desktop composition；先允许同 crate 分目录，稳定后再决定是否拆 crate。 | UI 模块不再直接持有 desktop service 的业务状态；启动组合有单独入口测试 |
| A-02 | 在 contract/application 形成覆盖 11 个页面的 snapshot/event/capability vocabulary；Iced `Message` 与 Bevy `UiCommand` 只做 toolkit 映射。 | 同一个 intent 在两个 UI 上产生相同 typed result、error、revision/generation |
| A-03 | 为 Bevy 每一个业务页增加真实 application source；生产路由禁止调用 `Projection::demo()`，demo 只允许出现在 capture/test composition。 | 11 页均能在 live/unavailable/empty/error 状态下渲染并更新 |
| A-04 | 建立双端 parity guard，检查页面、intent、能力状态、错误状态和测试覆盖，不只检查枚举数量。 | CI 对缺页、单端命令、单端测试和 demo 泄漏 fail closed |
| A-05 | 建立“shared + Iced + Bevy + 双端测试 + 视觉/宿主证据”的统一交付模板。 | 任一项缺一个 lane 就不能标记完成 |

## 双端完成定义

225 项中的每一项都必须同时交付四层：

```text
shared contract/application behavior
        + Iced adapter/view/update
        + Bevy adapter/projection/scene
        + Iced + Bevy headless behavior tests
        + applicable desktop/mobile live or host evidence
```

状态只允许使用：

- `planned`：只有目标描述；
- `shared-ready`：契约/application 已具备，但 UI 未双端完成；
- `iced-ready` / `bevy-ready`：单端完成，仍然不能计入交付；
- `parity-ready`：双端行为、错误、能力和测试均通过；
- `host-verified`：适用宿主和打包/真实内核证据也通过。

任何只在 Iced 或只在 Bevy 中完成的项目，都必须继续显示为未完成，不能用“另一端之后补”作为里程碑完成条件。

## 页面与宿主责任矩阵

| 责任 | Iced | Bevy UI | Desktop / Android / iOS host |
| --- | --- | --- | --- |
| 页面布局、控件、手势 | Iced 本地实现 | Bevy scene/system 本地实现 | 不拥有页面状态 |
| 命令提交 | `Message` → `CommandIntent` | `UiCommand` → `CommandIntent` | 不绕过 application |
| 业务快照和结果 | 只读 projection/cache | 只读 projection/cache | 提供 port 实现 |
| Core 生命周期、订阅、同步、配置事务 | 调 application | 调 application | 实现进程、文件、权限、VPN 等 port |
| 系统代理、TUN/VPN、托盘、通知 | desktop capability | 按宿主注入 capability | 真实 owner |
| 移动前后台与 VPN 权限 | 不适用或明确 unsupported | Bevy mobile UI 只消费 capability | Android/iOS host 负责 |

## 审计门禁

本审计结论成立的静态证据：

```bash
python3 scripts/quality/core-boundary-guard.py --mode enforce
python3 scripts/quality/doc-link-guard.py --mode enforce
python3 scripts/quality/import-guard.py --mode enforce
python3 scripts/quality/session-guard.py --mode enforce
```

这些命令只能证明底层依赖边界，没有证明双端页面已具备真实数据。因此它们必须与 live projection、双端行为测试和宿主 smoke 一起使用。
