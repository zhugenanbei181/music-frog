# Core 后端契约

> 层级：L2

Mihomo 与 sing-box 均已有 production backend provider，Desktop 与 Android `:vpn` 都按持久化、revision-fenced 的 active-profile reference 选择 engine-keyed provider；未实现或未验证的操作按 operation 返回 typed `Unsupported`，不从另一引擎复制能力。平台交付状态与 run ID 见 [PLATFORM_MATRIX.md](PLATFORM_MATRIX.md)，Mihomo 逐操作证据见 [MIHOMO_CORE.md](../crates/mihomo-api/docs/MIHOMO_CORE.md)。

Mihomo 订阅与 Geo 更新的实现、锁定版本和逐操作 run ID 见 [MIHOMO_CORE.md](../crates/mihomo-api/docs/MIHOMO_CORE.md)；这些证据只授权对应 target/revision，不外推到 sing-box 或 Android。

## Neutral core layer

两个引擎、所有平台和所有 UI 共享的纯契约与纯 port 位于 `lurker-*` 中性层。它们不得依赖任何 `infiltrator-*` 产品 crate、`mihomo-*`/`singbox-*` 引擎适配器、Tokio/Reqwest 运行时或 Bevy/Iced/Slint UI 工具包，也不得包含 OS 专属代码。

| 中性 crate | 内容 | 依赖方向 |
| --- | --- | --- |
| [`lurker-core-contract`](../crates/lurker-core-contract/Cargo.toml) | identity/session/generation、lifecycle、typed failure、outbound 观察模型、routing intent、`Capability`、`DocumentValidation` 与 `ProfileFormatId`/`ProfileSchemaVersion` 等引擎无关身份 | 不依赖产品 crate |
| [`lurker-core-ports`](../crates/lurker-core-ports/Cargo.toml) | `PortError`、`CoreProcess`/`CoreReadiness`、`OutboundPort`、`RoutingIntentPort`、纯 `ProfileDocumentCodecPort`（`scope`/`inspect`/`insert_snippet`）与 `engine_backend` 的 bundle/provider/owner-scope 词汇 | 只依赖 `lurker-core-contract` |
| [`lurker-engine-runtime`](../crates/lurker-engine-runtime/README.md) | `EngineBackendProviderRegistry<B>`：按 `EngineId` 注册、移除、查询可用性与分派 backend，并校验返回 bundle 身份 | 只依赖 `lurker-core-contract`/`lurker-core-ports` 与 `async-trait`；不依赖 driver、产品 crate 或 UI |

产品契约 [`infiltrator-contract`](../crates/infiltrator-contract/Cargo.toml) 与产品 port [`infiltrator-ports`](../crates/infiltrator-ports/Cargo.toml) 单向依赖中性层：它们可以增加产品语义（例如 `Availability`/`CapabilitySnapshot`、workspace identity、typed profile configuration），但中性类型只保留一个定义处，不得再导出同名副本。`scripts/quality/core-boundary-guard.py` 与 `scripts/quality/lurker-neutrality-guard.py` 共同守住该方向。

## Engine switch protocol

`lurker-*` 中性层的 owner 替换协议分三层落地，三者只按 contract → ports → runtime 单向依赖：

- 契约 [`engine_switch`](../crates/lurker-core-contract/src/engine_switch.rs)：`EngineSwitchRevision`（checked `next()`）、`EngineSwitchTarget<S>`、唯一的四阶段 `EngineSwitchStage<S>`（`ClosingOld` → `OldStopped` → `SelectionChanged` → `BindingNew`）、`EngineOwnerState<S>`、`EngineSwitchSnapshot<S>`、`EngineSwitchFailurePhase`（`BeforeReservation`/`ReservationPending`/`OwnerTerminationUnconfirmed`/`OldOwnerClosed`）与 `EngineSwitchDisposition<S>`（`RolledBackToPrevious`/`RecoveryRequired`）。
- 端口 [`engine_switch`](../crates/lurker-core-ports/src/engine_switch.rs)：`EngineSwitchJournal::{snapshot, bind_owner, begin_owner_switch, recover_owner_switch, recover_old_owner_after_terminal, recover_candidate_after_terminal}` 与 `EngineOwnerLease`/`EngineOwnerSwitchLease`/`EngineOwnerStoppedSwitchLease`/`EngineSwitchSelectionChangedLease`/`EngineOwnerBindingLease` 逐阶段 lease。每个 lease 绑定签发时的 revision 与 exact generation，过期 lease 只能得到 stale 拒绝。
- runtime [`switch`](../crates/lurker-engine-runtime/src/switch)：`EngineSwitchCoordinator<B, J>` 固定 validate → snapshot（必须 `Bound` + `Selected`）→ registry dispatch 与 candidate prepare（都在 reservation 之前）→ `begin_owner_switch` → 旧 owner exact terminal → `compare_and_set_target` → `reserve_new_owner` → apply → `finalize`；`MemoryEngineSwitchJournal` 是 facade 与 fake 测试用的内存 journal。

旧 owner terminal 之前失败可 `cancel_before_old_owner_close` 回到原绑定；terminal 之后任何失败只返回 `OldOwnerClosed` + `RecoveryRequired`，由 journal 中保留的 stage 与 terminal generation 续跑，绝不伪造自动恢复。`EngineSwitchRecovery<S>` 是纯 tagged enum、不携带 port trait object，以保持 contract 不依赖 ports；可恢复 lease 由 journal 的 `recover_old_owner_after_terminal` 与 `recover_candidate_after_terminal` 在拿到 terminal 证据后单独返回。本小节只覆盖 PUB-003.2a 的中立协议、coordinator 与 fake/内存 journal；产品 `ActiveProfileReferenceStore`/composition 迁移属于 PUB-003.2b。

## Adapter crate owner

每个引擎都按稳定职责边界拆包；能力尚未验证的操作仍在所属 adapter 中返回 typed `Unsupported`，不能因此把多个职责并回一个 engine umbrella crate。

| owner | Mihomo | sing-box | 边界 |
| --- | --- | --- | --- |
| Neutral engine driver | [`mihomo-driver`](../crates/mihomo-driver/README.md) | [`singbox-driver`](../crates/singbox-driver/README.md) | 中性 controller client/WS、readiness、typed outbound/routing port、原生 document codec 与进程监督；只依赖 `lurker-core-contract`/`lurker-core-ports`，不依赖任何产品 crate |
| Controller API | [`mihomo-api`](../crates/mihomo-api/README.md) | [`singbox-api`](../crates/singbox-api/README.md) | 产品侧 controller adapter：运行态映射、rule provider、geo database、capability evidence 与 operation port；client 本体在 neutral driver |
| Native configuration | [`mihomo-config`](../crates/mihomo-config/README.md) | [`singbox-config`](../crates/singbox-config/README.md) | 原生格式 codec、校验和 engine-specific 配置事务；sing-box JSON codec 已并入 driver，Mihomo YAML codec 待 PUB-002.2；产品配置管理器留在各自 adapter |
| Core process | [`mihomo-driver::process`](../crates/mihomo-driver/src/process) | [`singbox-driver::process`](../crates/singbox-driver/src/process) | engine-specific command、spawn/stop 与生命周期 supervisor，已随驱动收口并入 neutral driver |
| Version assets | [`mihomo-version`](../crates/mihomo-version/README.md) | [`singbox-version`](../crates/singbox-version/README.md) | 发行资产锁、验证、安装、激活、卸载与回滚 |
| Shared profile persistence | [`ProfileStore` port](../crates/infiltrator-ports/src/profile_store.rs) | 同一 port | `infiltrator-profile-store::EngineProfileStore` 直接实现共享 port，拥有 engine-scoped document/sidecar、validation、metadata、namespace/migration、reset、active-reference/workspace/catalog/owner-proof、owner-switch entry/recovery 与进程内 typed owner registry；Mihomo 只注入历史 settings、sync index 与目录规格。细节见 [crate README](../crates/infiltrator-profile-store/README.md) |
| Neutral persistence kernel | [`lurker-profile-store`](../crates/lurker-profile-store/README.md) | 同一 kernel | 引擎/平台/UI 中性的 durable replace、跨进程 instance lock、profile 写锁、on-disk `ProfileStoreLayout`、settings 与 profile-state schema/backup recovery、原子 document I/O 与 `ProfileStoreCore`（engine identity、codec scope、一次性 namespace 初始化）。不含 Age/keyring/订阅语义，也不含产品路径字面量；产品在其上注入 layout 与凭据实现 |
| Shared host capabilities | [`infiltrator-platform`](../crates/infiltrator-platform/README.md) | 同一 owner | 路径、凭据、安全文件替换、Android owner bridge、网络/电源等 OS 能力 |

依赖只向 engine-neutral ports/contracts 汇聚，再由 composition 装配需要的职责包。各 adapter 不得通过跨引擎 adapter 依赖共享事实：

```text
contract + ports
       ↑
 neutral store kernel / shared platform     mihomo-* adapters     singbox-* adapters
                  \                              |                    /
                   └──────────────── composition ─┘
```

`mihomo-dav-sync` 仍是 Mihomo YAML profile sync owner：其 indexer 只选择 YAML 文件，冲突算法解析并合并 YAML。若改为原样同步任意 `ProfileDocument`，应先迁移到 engine-neutral sync contract，再调整名称。

## Backend-neutral adapter contract

[`EngineBackendProvider`](../crates/lurker-core-ports/src/engine_backend.rs) 是 Mihomo 与 sing-box 共用的 backend adapter trait。它按 `EngineId` 生成同一形状的 [`EngineBackendPorts`](../crates/lurker-core-ports/src/engine_backend.rs)：统一承载 Core process/readiness、document inspection，以及有明确 `Available`/`Unsupported` 状态的 outbound/routing ports；产品扩展 [`ProductEngineBackendPorts`](../crates/infiltrator-ports/src/engine_backend.rs) 在同一 core bundle 上追加 native configuration transaction binding、rule provider 与 geo database 操作。中性 [`EngineBackendProviderRegistry`](../crates/lurker-engine-runtime/src/registry.rs) 按 `EngineId` 分派 provider 并校验返回 bundle 身份。Composition 和 conformance 都通过该契约选择 backend；引擎差异留在各自的 process、config 与 controller API owner。

版本发布和操作能力继续使用 [`EngineVersionBackendFactory`](../crates/infiltrator-ports/src/engine_version_backend_factory.rs) 与 [`CoreCapabilityProvider`](../crates/infiltrator-ports/src/core_capability_provider.rs)，并以同一 `EngineId` 关联到 backend。这样共用启动、配置、作用域和能力授权流程，同时保留 Mihomo YAML/REST/release identity 与 sing-box JSON/Clash API/release lock 的原生语义。产品接线按宿主可用能力注册；缺少组件时返回 typed `Unsupported`，不能从另一引擎复制能力声明。

- Profile 绑定 `EngineId`；命令、回执与快照绑定 `CoreScope`，身份或代次不符时拒绝更新。
- 能力按 operation、engine/version、host、binary target 和 adapter revision 查询；需要活动版本的引擎操作必须匹配精确版本/目标/revision 证据，无活动版本的引擎操作仍须匹配目标/revision 的操作证据。[`CoreBinarySelection`](../crates/infiltrator-ports/src/core_binary_selection.rs) 携带所选 executable 的 `EngineBinaryTarget`，产品根在构造 backend 前通过 `validate_for` 校验 engine 与当前 host target；target 事实与 version/provenance verification 分开校验，任一不匹配都 fail closed。缺少能力、证据或 owner-composed port 时 fail closed；进程运行不代表代理/VPN 已生效。
- Core 资源的可用性、失败与 GC 结果必须用带 owner scope 的 typed outcome 表达；`Collected` 需要同 scope 的 post-GC 内存读回，未接入的 engine 明确 Unsupported。契约见 [`resources`](../crates/infiltrator-contract/src/resources.rs)，port 见 [`CoreResourcePort`](../crates/infiltrator-ports/src/resources.rs)，Mihomo 行为证据见 [adapter 文档](../crates/mihomo-api/docs/MIHOMO_CORE.md)。
- Core version management 的共享 capability enum 明确区分 `Inspect`、`Install`、`Activate`、`Uninstall`、`Rollback`，都不要求活动 Core 版本并归属 typed `VersionManagement` owner。Android `:vpn` typed owner IPC 已路由版本查询、安装、激活、卸载、回滚和异步安装观察/取消至共享应用；未实现的其它操作继续声明 `Unsupported`。该 Rust IPC 有 nextest 覆盖，Kotlin 绑定、APK 与 Android 设备验收仍开放。
- 每个 owner process 恰有一个 Core actor。
- [`CoreProcess::start`](../crates/lurker-core-ports/src/core_process.rs) 必须携带显式 `CoreProcessStartContext::{Managed, Standalone}`；`Managed` 携带 identity/generation/session，Android 只接受该变体。`CoreApplication` 先调用 typed `prepare_start` admission，再通过 owner lease 持久化 exact `Managed` context，之后才允许 spawn；owner proof schema v2 持有 launch context，v1 bound 记录迁移为 `LegacyUnverified`，不授权自动恢复。Android `:vpn` 的 child observation、launch reservation 与 recovery 顺序见 [Android 产品章程](../android/docs/BEVY_ANDROID_PRODUCT.md)。
- [`CoreProcess::stop`](../crates/lurker-core-ports/src/core_process.rs) 成功必须证明该 Core 进程实例已退出；收到 kill 请求或 PID 记录缺失都不构成终态。桌面 adapter 校验 PID 对应的二进制并有界等待退出，Android host 返回等待退出后的 typed receipt。`CoreApplication::close` 仅在 `Stopped`、无 session 且 owner identity/generation 匹配后释放 owner。
- Android owner shutdown 由 [`CoreOwnerShutdownApplication`](../crates/infiltrator-application/src/core_owner_shutdown_application.rs) 按 VPN terminal readback → Core close/readback 顺序执行；任一步缺少终态都保留 owner fence，不能先停 Core 后把 VPN/数据面留在活动态。
- `doctor run` 是只读观察：版本检查通过 typed observation 读取已选版本和二进制，也可观察旧版版本目录，不创建或迁移版本 namespace；所有修复只由显式 `doctor fix` action 执行。实现细节见 [mihomo-version 说明](../crates/mihomo-version/README.md)。
- 产品级 Core provider 由持久化的 revision-fenced `ActiveProfileReference::{Unselected, Selected}` 唯一选择，`Selected` 同时携带 `engine_id` 与 `profile_id`。组合层必须在 provider dispatch 前读取该引用、加载精确 `ProfileDocument` 并校验其 engine/profile identity 与目标 capability；每 engine namespace 的 current pointer 不具备跨 engine 选择权。读取是只读快照，选择写入使用跨进程 revision CAS 与 OS 锁。owner guard 绑定到 `CoreApplication`，只有成功关闭后才释放；稳定 owner identity 使用 engine+instance，runtime version 是可变 capability evidence；generation、terminal 状态、session 与 owner token 仍精确校验。新 owner 构造失败必须显式中止候选 reservation 或关闭应用并验证精确 terminal readback；失败回执区分 `BeforeReservation`、`ReservationPending`、`OwnerTerminationUnconfirmed` 和 `OldOwnerClosed`。
- 活动 profile 的内容修改使用 revision-bound `ActiveProfileWorkspaceWriteLease`：同一 lease 持有 reference、owner 与 workspace fence，贯穿原子文件发布、Core apply、精确读回及条件恢复。非活动内容写入/恢复使用 `compare_and_save_inactive_workspace` 与 `restore_inactive_workspace`；活动 profile 不再经过通用的 inactive 写入口。
- Desktop Host retry/rebuild 与 Admin profile 选择现经同一 `DesktopProductSession`：同引擎选择提交带当前 `CoreScope` 的 `SwitchProfile`，跨引擎选择走 revision-fenced owner replacement。`DesktopProductSessionBinding` 将当前 backend 的 `CoreApplication` 与同源 `ProfileApplication` 一起交给 host；切换或恢复后重新绑定新 backend。`RecoveryRequired` 保留目标、失败阶段与运行态，并按 durable journal 阶段续跑；候选缺少精确终态证据时 fail closed。
- Core process、Controller API/readiness、配置应用、VPN consent、TUN data plane 和真实代理流量是不同状态轴，分别用 typed enum 表达。服务启动不得隐式启动 Core；Core 启动不得隐式建立 VPN；任意单个 `running: bool` 不可授权或证明后续阶段。
- 共享 [`SurfaceSnapshot`](../crates/infiltrator-contract/src/surface_snapshot.rs) 的 `runtime_status` 是由 owner 回读派生的只读状态投影：它分别表达 Android control service、Core process、Controller API 连通、VPN 授权、TUN data plane 与代理流量证据。Controller API 只有在同一 Core generation/session 的遥测读取成功时标为 Connected；VPN ready 还要求当前 generation 的 Worker、TUN descriptor 和 routing lease 均被 owner 回读为 Held。当前活动连接尚无 typed outbound identity 时只报告 route unattributed，不把 routing intent、Core Running 或 TUN ready 冒充代理流量已生效。缺少 `runtime_status` 的旧快照按 `Unknown` 读取；此 additive JSON 字段不改变 Binder Bundle 请求/回执结构，因此不提升 AndroidHostProtocolVersion。
- Mihomo 与 sing-box 可由同一 application contract 驱动，但 process、配置 codec、runtime API、release identity 和 capability evidence 属于各自 adapter。未真实实现并验收的操作必须返回 typed `Unsupported` 且无副作用，不能从 API 路径同名、版本字段存在、mock 或 trait 实现推断支持。
- `StartProxy` 的 Core admission 只要求 `CoreLifecycleOperation::Start`；宿主 VPN 授权、启动和停止由 typed lifecycle child 命令执行，`StopProxy` 只发送 `StopVpn` child。Core lifecycle capability 只授权 Core 进程的启动、停止与重启；`ProxyControlOperation` 专门表达 routing intent 读写，不表达宿主 VPN activation；缺少 Core lifecycle capability 时 fail closed。
- outbound 批量探测使用 typed `One`、`Collection`、有序 `Selected` 或 `All` scope，并携带预期 inventory revision、已校验的 HTTP(S) target/timeout 与 1–30 的应用层并发上限。`Collection` 展开直接的具体成员，`All` 选择具体 outbound 并排除观测到的 collection 容器；`Selected` 必须非空、ID 唯一，且每个目标都必须是当前 inventory 中的具体 outbound，集合容器、未知 ID、跨 engine ID、重复 ID 或空列表都要在任何 probe 副作用前拒绝。`Selected` 的逐项结果严格保持请求顺序。每个成员返回 `Measured`、`TimedOut`、`Unsupported` 或 `Failed`。请求 scope 或 revision 不匹配时，在 probe 调用前拒绝；adapter 可以进一步串行化请求。
- `ProfileDocument` 保留 engine 身份、原始字节、格式版本和 revision。跨引擎转换须产生目标文档与 loss report，确认前不写入。
- 中性 `EngineBackendPorts` 持有纯 `lurker-core-ports` 的 `ProfileDocumentCodecPort` inspection 与 `EngineOperationPorts::{Unsupported, Available}` 表达的 outbound/routing owner；产品 `ProductEngineBackendPorts` 追加 `ProfileConfigurationCodecPort` 与独立 typed native transaction，并用 `ProductEngineOperationPorts::{Unsupported, Available}` 表达 rule-provider/geo 操作；`Unsupported` bundle 不含可 dispatch adapter。每个产品 owner 在 Core actor 创建后绑定 typed `CoreScopeSource`，其 `Unavailable`/`Observed(CoreScope)` 状态表达 scope 可用性。codec 可用本身不授权 Core 配置或运行态操作；engine 专属事务与回滚语义见各 L3 adapter 文档。
- 运行时配置读取/写入使用 [scope-bound feature ports](../crates/infiltrator-ports/src/runtime_configuration.rs) 与 [`RuntimeConfigurationApplication`](../crates/infiltrator-application/src/runtime_configuration_application.rs)：写操作绑定已观察的 `CoreScope` 和 feature revision，并验证精确读回；PAC rules 与 proxy ports 是独立只读操作。产品组合直接注入这些窄端口，Mihomo REST/wire 调用只留在 adapter 内部，不暴露宽配置 DTO、任意 JSON patch 或规则列表方法。
- Rule provider 使用 [`RuleProviderPort`](../crates/infiltrator-ports/src/rule_provider_runtime.rs) 与 `RuleProviderRuntimeApplication`：inventory、payload 和单项 refresh 均绑定 `CoreScope`、engine-scoped provider ID 与 inventory revision；refresh 先重新观察并 fence revision，再提交并返回同 scope inventory readback。Geo 更新使用 [`GeoDatabasePort`](../crates/infiltrator-ports/src/geo_database.rs)，回执只表示触发已接受，不表示下载或安装完成。缺少 capability 或后端证据时显式 `Unsupported`。
- 共享运行态使用中性快照；core 专属字段留在 adapter。`RoutingIntentPort` 在同一 typed port 上观察和应用 `RoutingIntent`，application 依据 observed `CoreScope` 与精确读/写 capability 授权，写入后要求读回一致。Mihomo adapter 独占把 `/configs` 原生 `mode` 转换为中性意图；Desktop sing-box 当前只暴露 Core lifecycle，运行态写入仍 `Unsupported`。引擎专属实现与证据见 [MIHOMO_CORE.md](../crates/mihomo-api/docs/MIHOMO_CORE.md) 与 [sing-box driver](../crates/singbox-driver/README.md)。
- 资产、安装与回滚按 engine 隔离。

```text
UI → typed intent + CoreScope → application → backend adapter → core
```

新增后端须验证启动、健康探测、原生配置校验与读回、代表性操作、失败语义和资产摘要；实现细节见 [MIHOMO_CORE.md](../crates/mihomo-api/docs/MIHOMO_CORE.md)。

引擎专属 conformance、真实进程 smoke 与逐操作 capability 证据归各 L3 adapter 文档：Mihomo 见 [MIHOMO_CORE.md](../crates/mihomo-api/docs/MIHOMO_CORE.md)，sing-box 见 [sing-box driver](../crates/singbox-driver/README.md) 与 [sing-box API](../crates/singbox-api/README.md)；平台交付状态见 [PLATFORM_MATRIX.md](PLATFORM_MATRIX.md)。
