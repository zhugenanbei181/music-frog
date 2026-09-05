# 0.30 Core 重整计划：领域、应用、端口与宿主

状态：0.30 结构性重整规划。0.20 负责冻结当前可交付形态；0.30 允许对 Rust API、crate 依赖和前端接入方式做破坏性调整。

本文件定义底层重整的目标边界和验收规则。具体实现流水写入本地 `TODO.md`，功能归属仍以 `FUNCTIONAL_MAP.md` 为准。

### 当前 0.30 进度

- [x] `release/0.20` 已冻结为提交 `9c187b2`，并从该点创建 `codex/0.30`。
- [x] `infiltrator-domain`：提取生命周期状态机，依赖树无 Tokio。
- [x] 纯算法 `vector_clock`、`sub_rules`、DNS/Fake-IP/TUN schema 与校验、DNS topology、diagnostics 计算器、脚本引擎、MRS、PCAP、流量审计、故障转移、Geo 缓存、hosts、idle-connection、日志脱敏、per-app routing、规则/PAC、节点 URI、filter、mixin、YAML AST、profile-options 组合、backoff、MTU、丢包和规则命中统计已从 `infiltrator-core` 物理移入 `infiltrator-domain`。
- [x] `infiltrator-contract`：落下跨端命令、快照、事件、能力、失败和 intent 模型。
- [x] `infiltrator-ports`：落下 Core process、Overview、secure store、data store 和 capability provider 端口。
- [x] `EndpointSource`/`ControllerEndpoint` 已移入 ports；profile endpoint adapter 已归位 `mihomo-config::endpoint`。
- [x] application actor/facade 第一批生命周期与 Overview seam：single-flight、adopt、bounded contract events、模式回读和运行态快照。
- [x] `CoreLifecyclePort` 已成为 apply transaction 的生命周期输入；桌面 runtime 与 Android apply 已切到 `CoreApplication`。
- [x] 旧 `infiltrator-core::session` 与未接线的 `session_adapter` 已删除；retry bootstrap 也走 `CoreApplication`。
- [x] Bevy Overview 已改为消费 application snapshot；UI crate 不再直连 `mihomo-api`、Reqwest 或 Tokio。
- [x] Desktop/Android 已把 Core process、secure store、data-dir 和 readiness 组合到 host adapter 端口。
- [ ] application actor/facade 覆盖全部 use-case，并统一前端命令与领域快照通道。
- [x] `CommandApplication` 扩展 handler 已接入 profile、proxy、doctor、routing、sync、settings、connection；Bevy 提供显式 handler 启动入口，未具备宿主 port 的系统能力保留 typed unsupported。
- [x] `SnapshotApplication` / `SnapshotStore` 已统一快照创建、列表、读取和恢复；文件路径安全在 core adapter，恢复复用 profile application 的 apply 事务。
- [x] `VersionApplication` / `VersionPort` 已统一版本查询、远端 release、下载进度/取消、激活和卸载；具体 manager 只在 core adapter 与 desktop boot composition 出现。
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
- [x] `NetworkApplication` / `PublicIpProbe` 已统一出口 IP 探测，页面不再直接构造 Reqwest；Fake-IP cache 作为独立 host IO adapter 保留。
- [x] `RoutingApplication` / `AppRoutingStore` 已统一 Android 应用路由配置与 Iced 桌面模式/进程规则持久化；Android 自身包排除仍留在 Android host 规则内。
- [x] `DoctorApplication` / `DoctorPort` 已统一 doctor 检查、修复、解释和 bootstrap 结果；跨端结果落在 contract，core 只提供 Mihomo doctor host adapter，Admin、CLI、Android FFI 已接入。
- [x] `SyncApplication` / `SyncPort` 已统一 WebDAV 测试、全量同步以及 Iced 上传/下载的 transport seam；进度/取消/冲突结果均用 contract，冲突键 diff 已移入 domain。
- [x] Iced runtime handle 已收敛为 `HostRuntime` trait object；desktop 的具体 `MihomoRuntime` 只在 boot composition 中构造，UI 仅消费 gateway、generation、apply 与 typed host capability。
- [x] `InfiltratorError` 已移入 `infiltrator-contract`；Mihomo/IO 适配通过显式边界转换，不再从 core 暴露 transport error 类型。
- [x] profile projection (`ProfileInfo` / `ProfileDetail`) 与 profile name 校验已移入 `infiltrator-domain::profiles`。
- [x] subscription 的 URL 校验、内容解码、userinfo/配额、UA、WAF 分类和安全审计已移入 `infiltrator-domain::subscription`；HTTP 重试、响应体上限和 HeaderMap 转换集中在 `infiltrator-core::subscription_io`。
- [x] proxy-provider 与 sniffer 的 schema、校验和 YAML 变换已移入 `infiltrator-domain::{proxy_providers,sniffer}`，持久化由 configuration application 通过 `ProfileStore` 完成。
- [ ] 其余标准 adapter（配置、版本、Admin、同步）按同一规则下沉到 composition/outbound 组合根。
- [x] `infiltrator-ios` host crate 已建立端口与保守 capability seam，且 composition root 已有 `IosBridge -> CoreApplication` 入口；Native NetworkExtension bridge 仍待接入。
- [ ] Iced、Admin、Android FFI 的全部 use-case 完成同一 application facade 接入；当前已完成 profile/config/network/routing/doctor 等主要垂直切片。
- [ ] 全端删除对具体 Mihomo client、Reqwest 和 Tokio channel 的公开/直接依赖。

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
