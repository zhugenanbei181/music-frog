# Core 后端契约

> 层级：L2

当前只接入 Mihomo。新增 adapter 必须实现此中性契约并完成真实二进制验收。资产与平台状态见 [PLATFORM_MATRIX.md](PLATFORM_MATRIX.md)。

- Profile 绑定 `EngineId`；命令、回执与快照绑定 `CoreScope`，身份或代次不符时拒绝更新。
- 能力按 operation、engine/version、host 和 adapter 查询；缺少能力或 scope 时 fail closed。进程运行不代表代理/VPN 已生效。
- 每个 owner process 恰有一个 Core actor。
- Core process、Controller API/readiness、配置应用、VPN consent、TUN data plane 和真实代理流量是不同状态轴，分别用 typed enum 表达。服务启动不得隐式启动 Core；Core 启动不得隐式建立 VPN；任意单个 `running: bool` 不可授权或证明后续阶段。
- Mihomo 与 sing-box 可由同一 application contract 驱动，但 process、配置 codec、runtime API、release identity 和 capability evidence 属于各自 adapter。未真实实现并验收的操作必须返回 typed `Unsupported` 且无副作用，不能从 API 路径同名、版本字段存在、mock 或 trait 实现推断支持。
- `StartProxy` 的 Core admission 只要求 `CoreLifecycleOperation::Start`；宿主 VPN 授权、启动和停止由 typed lifecycle child 命令执行，`StopProxy` 只发送 `StopVpn` child。Core lifecycle capability 只授权 Core 进程的启动、停止与重启；`ProxyControlOperation` 专门表达 routing intent 读写，不表达宿主 VPN activation；缺少 Core lifecycle capability 时 fail closed。
- outbound 批量探测使用 typed `One`、`Collection`、有序 `Selected` 或 `All` scope，并携带预期 inventory revision、已校验的 HTTP(S) target/timeout 与 1–30 的应用层并发上限。`Collection` 展开直接的具体成员，`All` 选择具体 outbound 并排除观测到的 collection 容器；`Selected` 必须非空、ID 唯一，且每个目标都必须是当前 inventory 中的具体 outbound，集合容器、未知 ID、跨 engine ID、重复 ID 或空列表都要在任何 probe 副作用前拒绝。`Selected` 的逐项结果严格保持请求顺序。每个成员返回 `Measured`、`TimedOut`、`Unsupported` 或 `Failed`。请求 scope 或 revision 不匹配时，在 probe 调用前拒绝；adapter 可以进一步串行化请求。
- `ProfileDocument` 保留 engine 身份、原始字节、格式版本和 revision。跨引擎转换须产生目标文档与 loss report，确认前不写入。
- 共享运行态使用中性快照；core 专属字段留在 adapter。`RoutingIntentPort` 在同一 typed port 上观察和应用 `RoutingIntent`，application 依据 observed `CoreScope` 与精确读/写 capability 授权，写入后要求读回一致。当前只有 Mihomo adapter 接入生产组合根；它独占将 `/configs` 的原生 `mode` 转换为中性意图。`EngineDefined` 的读取可表达 Mihomo 专属状态，写入仍返回 typed `Unsupported`。sing-box 已有独立进程/readiness/native JSON adapter 源码，但尚未接入生产组合根；其 [`v1.14.3` 平台 archive digest 锁](../packaging/sing-box-evidence-assets.json) 仅证明发行资产身份，不证明二进制行为。真实二进制 smoke、CoreScope/CAS apply 集成及逐操作 capability evidence 均为 OPEN。
- 资产、安装与回滚按 engine 隔离。

```text
UI → typed intent + CoreScope → application → backend adapter → core
```

新增后端须验证启动、健康探测、原生配置校验与读回、代表性操作、失败语义和资产摘要；实现细节见 [mihomo-api 文档](../crates/mihomo-api/docs/README.md)。

Mihomo 的逐操作、逐版本能力证据由 [`MihomoCoreCapabilityProvider`](../crates/mihomo-api/src/core_capability_provider.rs) 按 [`mihomo-evidence-assets.json`](../packaging/mihomo-evidence-assets.json) 精确登记；它与产品发行用的 [`mihomo-assets.json`](../packaging/mihomo-assets.json) 分开管理。实现步骤、nextest run ID 与 OPEN 剩项（CORE-008）见 [Mihomo adapter 证据](../crates/mihomo-api/docs/MIHOMO_CORE.md)。
