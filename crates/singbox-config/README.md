# singbox-config

> 层级：L3

实现 sing-box 原生 JSON document codec、官方 binary native check、配置文件读回，以及绑定 `CoreScope`、workspace lease 与 store CAS 的配置事务。

本 crate 使用 `singbox-process` 提供的 command executor 与操作 gate，并通过 `infiltrator-ports::ProfileStore` 完成共享 profile transaction。production `ProfileStore` 由 `infiltrator-profile-store::EngineProfileStore` 直接实现；`ConfigManager` 只负责把 sing-box codec 和文件后缀注入共享 owner。本 crate 不拥有 controller API 或 binary version lifecycle。
