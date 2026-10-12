# mihomo-config

> 层级：L3

Mihomo YAML codec 与配置 adapter：解析、校验、应用观察和 Mihomo 专属选项编解码。共享契约是 `infiltrator-ports::ProfileStore`；engine-scoped document/sidecar/workspace 文件协议、source-bound compare/publish、profile catalog、aggregation-template sidecar、typed active-reference/owner-proof record、owner normalization、current-profile selection projection、active-workspace transaction 与跨-engine reset transaction/fence 由 `infiltrator-profile-store::EngineProfileStore` 拥有。`EngineProfileStore` 直接实现共享 `ProfileStore` 与 `ActiveProfileReferenceStore` ports；`ConfigManager` 只提供 Mihomo YAML 操作、显式 legacy migration 与 namespace 初始化门面，不实现共享 persistence port。

原始用户文档身份和跨引擎意图由共享 contract/application 定义；本 crate 不解释 UI 状态。Mihomo controller 边界见 [`mihomo-api` 本地文档](../mihomo-api/docs/README.md)。

`ConfigManager::with_home_configs_dir_and_store` 是 Mihomo 的 YAML/legacy-migration 组合；通用 `EngineProfileStore` 可由 `with_profile_codec_and_store` 注入 engine-owned codec 与文件后缀，再通过 `shared_engine_profile_store` 交给 application 和 host。两个 engine 复用同一 raw-byte、sidecar、write-lease、owner-fence、revision CAS 与 engine namespace 实现；Mihomo facade 不解析其他引擎文档，也不导入旧 Mihomo 路径。Profile data、metadata、backup 和凭据位于共享 configs 根下的 `engines/<engine-id>/`；产品级 active-profile reference 与 Core owner proof 仍落在共享根，由所有 engine store 读写同一 CAS 协议。

Mihomo profile 文档由本 crate 的 YAML codec 验证；其他引擎的格式和 schema 来自注入的 `ProfileDocumentCodecPort`。未注入对应 engine codec 的 store 不得读写该 engine profile。该排他边界只约束使用同一 lock files 并遵守此 store 协议的进程；直接改写持久化文件的程序会绕过门禁。

Live profile owner replacement 的 `Switching` stage 在进程重启后保持 fail-closed；store 暴露按 stage 重建 lease 与提交 exact terminal snapshot 的 recovery API，不会因进程内 lease 消失而自动清除。Android/Desktop 的进程退出证据接线仍 OPEN；ClosingOld 缺少旧 owner 的 exact Stopped/no-session snapshot 时保持 durable reservation，BindingNew 只接受 generation 为 0 的 Stopped/no-session 候选快照。
