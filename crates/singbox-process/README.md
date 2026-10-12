# singbox-process

> 层级：L3

实现 sing-box Core process adapter：官方命令调用、spawn/stop/readiness 前置，以及进程 start/stop 与 native config transaction 共用的操作 gate。

进程所有权与宿主 OS 能力由 `infiltrator-platform` 配合；Clash API readiness 和运行态操作由 `singbox-api` 提供。本 crate 不实现配置持久化或版本发布流程。
