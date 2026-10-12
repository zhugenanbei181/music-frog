# singbox-config

> 层级：L3

实现 sing-box 原生 JSON document codec、官方 binary native check、配置文件读回，以及绑定 `CoreScope`、workspace lease 与 store CAS 的配置事务。

本 crate 使用 `singbox-process` 提供的 command executor 与操作 gate，并通过 `infiltrator-ports::ProfileStore` 完成共享 profile transaction。production store 当前由 codec-bound `mihomo-config::ConfigManager` 提供，迁往 `infiltrator-profile-store` 由 CORE-018.3 收口；本 crate 不拥有 controller API 或 binary version lifecycle。
