# Mihomo adapter 约束

> 层级：L3

本页只记录 Mihomo 特有规则；crate 职责见 [README.md](../README.md)。

- `mihomo-api` 封装 REST/WebSocket、wire DTO、鉴权与错误映射；这些类型不得进入共享业务或 UI。
- 运行中 Fake-IP 清理通过 scoped `ControllerFakeIpCachePort` 调用 Mihomo controller。公开 API 将 `POST /cache/fakeip/flush` 定义为返回 `204`，没有缓存内容读回（[Mihomo API 文档](https://github.com/MetaCubeX/Meta-Docs/blob/main/docs/api/index.md)）；当前 adapter 回执只把已验证的请求 scope/operation 与 controller 成功响应关联，不证明缓存条目读回为空。该 operation 尚无逐版本真实 Mihomo evidence，生产 capability gate 继续拒绝它。
- `mihomo-config` 负责 Mihomo YAML 的解析、校验和持久化；校验失败不得发布配置。
- Mihomo 进程归 `mihomo-process`，版本安装归 `mihomo-version`，共享 OS/host 能力归 `infiltrator-platform`；controller 可用不代表代理/VPN 已生效。
- 凭据不进入日志、普通错误或 UI；流式请求须支持取消、断线处理与有界缓存。
- 产品发行资产版本和摘要只以 [`mihomo-assets.json`](../../../packaging/mihomo-assets.json) 为准；逐操作兼容证据另以 [`mihomo-evidence-assets.json`](../../../packaging/mihomo-evidence-assets.json) 锁定，不能改变发行版本。
- capability provider 启动时校验 [`adapter-operation-declarations.json`](../adapter-operation-declarations.json) 覆盖完整 operation catalogue，并将 Supported evidence ID 与 target-scoped schema v2 evidence asset 锁对应。证据须匹配精确 Core version、完整 Rust `binary_target` triple 与 `adapter_revision`；这些字段也进入 `CoreCapabilityScope`，使跨构建目标或 revision 的结果不能复用。当前 evidence lock 只有 `x86_64-unknown-linux-gnu`、revision 1；Android/iOS/其他 target triple 和 revision 均 `HostUnavailable`，version-bound operation 的版本或 operation evidence 缺失为 `VersionUnavailable`。`OwnerManaged` 只用于 Core lifecycle 与 version manager 命令，不冒充 engine API evidence。声明通过不代表 product composition 已接入，对应 adapter operation 仍需单独注入，并以 `AdapterMissing` fail closed。
- Mihomo v1.19.18 与 v1.19.32 的 Linux direct smoke 覆盖 CLI `-t` 原生配置检查、`/dns/query`、shell `PUT /configs?force=true` reload/readback、outbound list/select/probe、log-level 写读和 routing intent 写读。Mihomo 官方 API 将 `/configs` 的 `PUT` 定义为带 `force=true` 的 reload；活动 Profile transaction 通过该真实 reload 应用候选，在 reload/readiness 失败时精确恢复旧 workspace，并在锁外预绑定 controller endpoint。Profile transaction nextest run IDs：v1.19.32 `626fc06a-4fa4-4af1-837c-9149c2a7d783`；v1.19.18 `937ba089-9de1-4718-af9d-1d61563306e8`。
- Rust real-process adapter smoke 在两个 Mihomo 版本都打开 `/logs` WebSocket 并收到 `Connected`，验证 typed log-level 写入/恢复、outbound list/select/probe 与 routing 写入/读回。`MihomoRuntime::stream_logs_scoped` 在打开前后检查精确 CoreScope；桌面 scope 回归在 Core/host nextest `6e33cf86-2b6e-4835-992b-d33157132a43` 中通过。adapter real-process nextest run IDs：v1.19.32 `96f85ae3-6d29-486f-9c53-90916d8df181`；v1.19.18 `c2ed93ff-92ea-4555-abec-4aefc3c18e37`。`logs.stream` 的 Linux amd64 operation evidence 已登记；其它目标仍 fail closed。
- Mihomo v1.19.18 与 v1.19.32 的 real-process smoke 还通过隔离 loopback UDP DNS fixture 验证 `/dns/query` 与 typed `DnsQueryPort`：`core-smoke.test` 的 A 查询返回 `192.0.2.42`。adapter nextest run IDs：v1.19.32 `576168d3-53cd-4e53-a589-01cde7a39ac8`；v1.19.18 `0ad9ef53-3911-45d4-bfd2-bf69afc6bd4c`；focused capability nextest `49583c4c-a33d-4146-af4f-608a861cba95`；non-UI Core unified nextest `02cba78c-b0bc-452c-85dd-1546fa5da69f`（3207 passed、8 skipped）。`dns.query` evidence 只授权锁定的 Linux amd64、adapter revision 1 和这两个精确版本；其它目标与版本仍 fail closed。
- Logs.Read 没有 production Core command route，声明为 Unsupported；Logs.Clear 已从 CoreOperation catalogue 退役，只清理本地 LogApplication collector。Android owner、sing-box 及 Linux 以外平台不由这些 Mihomo 测试证明。Connection.List 因没有真实连接条目证据而未登记.
