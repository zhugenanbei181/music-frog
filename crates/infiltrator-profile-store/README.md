# infiltrator-profile-store

> 层级：L3

CORE-018.3 的 owner：实现 engine-neutral `ProfileStore` 持久化，包括 engine namespace、原始 `ProfileDocument` 与 sidecar、workspace lease、revision CAS、active-profile reference 和恢复事务。当前 crate 已接管 engine-scoped `EngineProfileStore` 状态、安全 document path、durable document/backup/sidecar I/O、document validation、workspace coherent read/source/protection projection 与成对发布、profile catalog 与 aggregation-template sidecar、共享写锁、profile namespace schema/bootstrap 与 durable TOML backup recovery、profile-state 元数据、订阅凭证映射及 active-reference/owner-proof durable record I/O、schema validation、owner-proof v1 migration 和 durability-error readback reconciliation。

完整 production `ProfileStore` / `ActiveProfileReferenceStore` adapter 与 workspace transaction lease 仍由 `mihomo-config::ConfigManager` 暴露；owner binding/switch leases 和跨文件事务仍由其协调。本 crate 已接管 product-root、owner-state 与 owner-admission locks、进程内 typed owner registry、owner normalization、外部终态证据后的 owner record recovery、active-reference/owner-proof typed record I/O、schema migration、durability reconciliation、revision compare/next-state policy 和 workspace 文件协议。进程内 registry 将 owner token、`Bound`/`Binding`/`Switching` 状态与 owner admission guard 绑定；绑定、切换、取消和候选 finalize 都通过校验前置状态的命名迁移执行，不提供任意状态赋值。registry 只协调单个 owner 进程，不替代跨进程文件锁或宿主 IPC。engine-scoped schema/bootstrap、TOML 原子读写和版本化 legacy file/state/credential migration 都归共享 owner；Mihomo facade 只注入 `LegacyV1` 的旧 settings、sync index 与目录规格。两个 engine 的 production `ProfileStore` 组合尚未迁入共享 adapter。store 不解析 Mihomo YAML 或 sing-box JSON，原生格式与 native check 由对应 config adapter 持有。

可移植 profile 名称由 `infiltrator-domain::profiles` 统一校验，文件路径、sidecar 与远端旧格式迁移共用同一规则；Mihomo 私有的重复校验和消毒入口已退役。
