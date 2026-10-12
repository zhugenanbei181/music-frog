# infiltrator-profile-store

> 层级：L3

CORE-017.3 的目标 owner：实现 engine-neutral `ProfileStore` 持久化，包括 engine namespace、原始 `ProfileDocument` 与 sidecar、workspace lease、revision CAS、active-profile reference 和恢复事务。

当前 production `ProfileStore` 实现仍由 `mihomo-config::ConfigManager` 持有；本目录尚未进入 workspace，代码迁移与两个 engine 的生产注入属于 CORE-017.3。目标 store 注入匹配自身 `EngineId`、format 与 schema 的 document codec，不解析 Mihomo YAML 或 sing-box JSON；原生格式与 native check 由对应 config adapter 持有。
