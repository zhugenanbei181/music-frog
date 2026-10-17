# mihomo-driver

> 层级：L3

Mihomo（Clash.Meta）引擎驱动：REST/WebSocket 控制器客户端、wire DTO、运行态流、出站观测/延迟探针、路由意图切换、控制器就绪与版本能力，以及桌面进程监督（启动、就绪、PID 归属、精确终止读回与子进程回收）。

本 crate 只依赖 `lurker-core-contract`、`lurker-core-ports` 与通用库：不含任何 `infiltrator-*` 产品类型、UI 工具包或产品路径策略。产品侧端口（连接控制、运行配置、规则提供者、Geo 数据库、资源观测等）由 `mihomo-api` 实现；核心投递与版本管理由 `mihomo-version` 实现；配置文档编解码由 `mihomo-config` 实现。

## 特性

- `default = ["http", "process"]`
- `http`：控制器 REST 客户端、日志/连接/流量 WebSocket 流、出站探针、路由意图与就绪探测。
- `process`：桌面进程监督。仅需 HTTP 能力时（例如 Android 自管 VPNService）可关闭。

## 模块

- `api::client`：`MihomoClient` 控制器客户端与流生命周期事件 `StreamEvent`。
- `api::proxy`：`/proxies` 观测 wire 解码（`ProxyObservation`）。
- `outbound`：`OutboundPort` 的 Mihomo 实现（清单、选择、延迟探针）。
- `routing_intent`：`RoutingIntentPort` 的 Mihomo 实现（rule/global/direct 模式读回）。
- `driver`：`CoreProcess`（`ProcessCoreController`）与 `CoreReadiness`（`ControllerReadiness`）实现。
- `version`：引擎标识、版本能力矩阵、目标制品清单与 fixture 生成。
- `types`：控制器 wire DTO。
