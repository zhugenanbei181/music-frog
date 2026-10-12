# infiltrator-profile-store

> 层级：L3

CORE-018.3 的 owner：实现 engine-neutral `ProfileStore` 持久化，包括 engine namespace、原始 `ProfileDocument` 与 sidecar、workspace lease、revision CAS、active-profile reference 和恢复事务。当前 crate 已接管 engine-scoped `EngineProfileStore` 状态、安全 document path、durable document/backup/sidecar I/O、document validation、workspace coherent read/source/protection projection 与成对发布、profile catalog 与 aggregation-template sidecar、共享写锁、profile-state TOML 元数据、订阅凭证映射及 active-reference/owner-proof durable record I/O、schema validation、owner-proof v1 migration 和 durability-error readback reconciliation。

完整 production `ProfileStore` / `ActiveProfileReferenceStore` coordinator、owner binding/switch/recovery transitions 和 workspace transaction lease 仍由 `mihomo-config::ConfigManager` 协调；本 crate 已接管 product-root、owner-state 与 owner-admission locks、进程内 typed owner registration registry、owner normalization、active-reference/owner-proof typed record I/O、schema migration、durability reconciliation、revision compare/next-state policy 和 workspace 文件协议。进程内 registry 将 owner token、`Bound`/`Binding`/`Switching` 状态与 owner admission guard 绑定；它只协调单个 owner 进程，不替代跨进程文件锁或宿主 IPC。namespace 初始化/旧 Mihomo 路径迁移与跨文件 owner transition 尚未迁入，两个 engine 的 production coordinator 尚未迁入。迁移目标是由 store 注入匹配自身 `EngineId`、format 与 schema 的 document codec；store 不解析 Mihomo YAML 或 sing-box JSON，原生格式与 native check 由对应 config adapter 持有。

可移植 profile 名称由 `infiltrator-domain::profiles` 统一校验，文件路径、sidecar 与远端旧格式迁移共用同一规则；Mihomo 私有的重复校验和消毒入口已退役。
