# singbox-driver

> 层级：L3

sing-box 引擎驱动：Clash API 客户端与 controller preflight、`/version` readiness、routing intent 观测、outbound inventory/selector 切换、原生进程启停与 child 守护、原生 JSON document codec 与官方 `check` 事务钩子、binary 版本探测。

本 crate 只依赖 `lurker-core-contract`、`lurker-core-ports` 与通用库；不携带任何产品类型、平台宿主或 UI 依赖。能力声明证据、profile workspace CAS 事务与版本资产安装锁属产品侧。

Feature 裁剪：`default = ["http", "process", "codec"]`；`http` 为 Clash API/readiness/outbound，`process` 为进程监督，`codec` 为原生 JSON codec（依赖 `process` 的命令边界）。仅 HTTP 的构建使用 `--no-default-features --features http`。
