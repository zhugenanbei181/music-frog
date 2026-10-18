# infiltrator-profile-store

> 层级：L3

本 crate 是产品 owner：耐用持久化的通用机制（原子替换、跨进程写锁、on-disk layout、settings/profile-state schema 与原子 document I/O）已抽到中性 [`lurker-profile-store`](../lurker-profile-store/README.md)，这里通过 `ProfileStoreCore` 复用，并保留产品私有语义：Age/订阅凭据、profile catalog/展示名/顺序、aggregation-template、named override、active reference/owner record、reset transaction 与 `ProfileStore` port 实现。产品历史路径字面量集中在 `src/layout.rs` 并由测试固定。

本 crate 拥有 engine-neutral `ProfileStore` 持久化：包括 engine namespace、原始 `ProfileDocument` 与 sidecar、workspace lease、revision CAS、active-profile reference 和恢复事务。当前 crate 已接管 engine-scoped `EngineProfileStore` 状态、安全 document path、durable document/backup/sidecar I/O、持有 typed selection lease 的 inactive 文档保存与删除（按 root→profile 顺序加锁，拒绝写入当前选中配置）及 sidecar 故障精确回滚、document validation、workspace coherent read/source/protection projection 与成对发布、profile catalog 与 aggregation-template sidecar、共享写锁、profile namespace schema/bootstrap 与 durable TOML backup recovery、typed profile-reset intent 解析/迁移、跨 engine pending 扫描、root→profile mutation fence 与完整 unowned reset transaction、profile-state 元数据、订阅凭证映射及 active-reference/owner-proof durable record I/O、schema validation、owner-proof v1 migration 和 durability-error readback reconciliation。

`EngineProfileStore` 直接实现 production `ProfileStore` 与 `ActiveProfileReferenceStore` ports；selection reader、active-workspace transaction lease、跨文件 workspace transaction、owner-switch entry/recovery、reset intent/fence 与完整 unowned reset transaction 都由共享 owner 协调。profile display name 在 `profile-state.toml` 中单独持久化，不改变 `ProfileId`、文件名、活动选择或 Core/VPN 生命周期；写入持有 root→profile reset/write fence 并返回真实读回。两个 engine 注入各自 codec 与文件后缀，共用同一 persistence 实现。owner-switch entry/recovery orchestration 已迁入共享 coordinator。本 crate 还持有 selection-to-owner-binding lease、secure owner-token generation、live Core owner lease 的 managed start proof 与 exact terminal release、外部终态证据后的 owner record recovery、owner normalization、product-root/owner-state/admission locks、进程内 typed owner registry、active-reference/owner-proof typed record I/O、schema migration、durability reconciliation、revision CAS authorization/target-evidence and next-state policy、owner-switch 与 typed stage leases、workspace 文件协议。进程内 registry 将 owner token、`Bound`/`Binding`/`Switching` 状态与 owner admission guard 绑定；绑定、切换、取消和候选 finalize 都通过校验前置状态的命名迁移执行，不提供任意状态赋值。registry 只协调单个 owner 进程，不替代跨进程文件锁或宿主 IPC。engine-scoped schema/bootstrap、TOML 原子读写和版本化 legacy file/state/credential migration 都归共享 owner；Mihomo facade 只注入 `LegacyV1` 的旧 settings、sync index 与目录规格。store 不解析 Mihomo YAML 或 sing-box JSON，原生格式和 native check 由对应 config adapter 持有。

Profile catalog 顺序也存于 `profile-state.toml`，compare 与 write 在 selection lease 和 profile-write fence 内完成；旧数据继续按 ProfileId 排序，新建项在显式顺序末尾出现，已删除 ID 只从读模型过滤。Stale 请求不写入，成功回执包含实际顺序读回。

活动选择清除在 owner authorization 与 root CAS fence 内验证；无 owner 可清除当前引擎选择，精确匹配的 bound owner 也可清除自身引擎选择而保持 owner 绑定不变。foreign/stale/错误 owner 凭据不会写入；该 CAS 不调用 Core 或 VPN 生命周期接口。

Age 私钥以 engine/profile 作用域存入注入的 `SecureStore`，不会进入 profile TOML、文档、metadata 或普通列表快照。凭据 owner 独立于 profile 文档存在与否，允许首次加密订阅先配置密钥、在 owner 内解密，再创建 Profile；解密/校验失败不保存正文，凭据保留供重试或显式清除。owner 读写持有 root → owner-state → profile fence，并校验完整 selection、owner 授权与凭据 revision；每次变化都通过安全存储读回验证，失败文案不带 adapter 原始错误。保存的 secure envelope 内以 `NotConfigured` tombstone 保留 revision，避免删除后 revision 回退。

Armored Age 订阅正文只在此 owner 内读取私钥并解密；错误只返回通用 typed 状态，私钥不离开 SecureStore 边界，解密正文用 drop 时清零的 contract 类型返回给 application 校验/提交。删除 Profile 前先写入并读回递增 revision 的 `NotConfigured` tombstone；文件/sidecar 删除失败时恢复凭据，防止 profile ID 重用后旧 key 或过期 edit 生效。

可移植 profile 名称由 `infiltrator-domain::profiles` 统一校验，文件路径、sidecar 与远端旧格式迁移共用同一规则；Mihomo 私有的重复校验和消毒入口已退役。
