# Mihomo adapter 约束

> 层级：L3

本页只记录 Mihomo 特有规则；crate 职责见 [README.md](../README.md)。

- `mihomo-api` 封装 REST/WebSocket、wire DTO、鉴权与错误映射；这些类型不得进入共享业务或 UI。
- 运行中 Fake-IP 清理通过 scoped `ControllerFakeIpCachePort` 调用 Mihomo controller。公开 API 将 `POST /cache/fakeip/flush` 定义为返回 `204`，没有缓存内容读回（[Mihomo API 文档](https://github.com/MetaCubeX/Meta-Docs/blob/main/docs/api/index.md)）；当前 adapter 回执只把已验证的请求 scope/operation 与 controller 成功响应关联，不证明缓存条目读回为空。该 operation 尚无逐版本真实 Mihomo evidence，生产 capability gate 继续拒绝它。
- `mihomo-config` 负责 Mihomo YAML 的解析、校验和持久化；校验失败不得发布配置。
- 进程、版本安装和宿主权限归 platform adapter；controller 可用不代表代理/VPN 已生效。
- 凭据不进入日志、普通错误或 UI；流式请求须支持取消、断线处理与有界缓存。
- 产品发行资产版本和摘要只以 [`mihomo-assets.json`](../../../packaging/mihomo-assets.json) 为准；逐操作兼容证据另以 [`mihomo-evidence-assets.json`](../../../packaging/mihomo-evidence-assets.json) 锁定，不能改变发行版本。
- capability provider 启动时校验 [`adapter-operation-declarations.json`](../adapter-operation-declarations.json) 覆盖完整 operation catalogue，并将 Supported evidence ID 与 target-scoped schema v2 evidence asset 锁对应。证据须匹配精确 Core version、完整 Rust `binary_target` triple 与 `adapter_revision`；这些字段也进入 `CoreCapabilityScope`，使跨构建目标或 revision 的结果不能复用。当前 evidence lock 只有 `x86_64-unknown-linux-gnu`、revision 1；Android/iOS/其他 target triple 和 revision 均 `HostUnavailable`，version-bound operation 的版本或 operation evidence 缺失为 `VersionUnavailable`。`OwnerManaged` 只用于 Core lifecycle 与 version manager 命令，不冒充 engine API evidence。声明通过不代表 product composition 已接入，对应 adapter operation 仍需单独注入，并以 `AdapterMissing` fail closed。
- v1.19.18 与 v1.19.32 的 Linux direct smoke 包含 Mihomo CLI `-t` 原生配置检查、shell `PUT /configs?force=true` reload/readback、outbound list/select/probe、log-level 写读和 routing intent 写读；CLI 检查不证明 production controller `force=false` preflight。Profile transaction 的 ignored nextest real-process smoke 已由双版本 harness 对 v1.19.18 与 v1.19.32 实际执行通过，覆盖 live version、`force=false` preflight、有效工作区变更、持久化、controller `/configs` readback 与 fixture restore（run IDs：`6e8c801e-00eb-481f-87fd-168dd6d0e8c4`、`8f2726f5-dca9-476a-a082-f3fc228a07cc`）。Rust `OutboundPort`、`RoutingIntentPort` 与 `CoreLogLevelPort` typed adapter smoke 也在两版本实跑通过，覆盖 outbound observation/list/select/readback/probe、routing GET/SET/readback 和 log-level 写入/读回/恢复（run IDs：`d99b30f4-8208-469f-bb5d-df9700acf2d5`、`aaeac5b5-7677-4bcf-9abe-b518deed3796`）。Logs.Stream 的 Rust real-process test 与 scope-bound composition 已加入 harness 源码，但本轮新增检查尚未运行并生成 run ID，故此操作仍 EvidenceOpen。Logs.Read 没有 production Core command route，声明为 Unsupported；Logs.Clear 已从 CoreOperation catalogue 退役，它只清理本地 LogApplication collector。其它逐操作状态以 [`adapter-operation-declarations.json`](../adapter-operation-declarations.json) 为准。Android owner、sing-box 及 Linux 以外平台不由这些 Mihomo 测试证明。Connection.List 因没有真实连接条目证据而未登记。
