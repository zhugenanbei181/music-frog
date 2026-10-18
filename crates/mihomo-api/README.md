# mihomo-api

> 层级：L3

Mihomo 产品 adapter：在 `mihomo-driver` 的中性控制器客户端之上实现产品端口（连接控制与流、运行配置、规则提供者、Geo 数据库触发、DNS 查询、资源观测、日志级别）与 operation capability。

REST/WebSocket 传输、wire DTO、进程监督和引擎版本知识归 `mihomo-driver`；本 crate 只拥有产品语义，不拥有共享用户意图或 UI 状态。Mihomo adapter 细节见 [MIHOMO_CORE.md](docs/MIHOMO_CORE.md)。
