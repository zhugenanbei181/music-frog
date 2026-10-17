# mihomo-driver

> 层级：L3

Mihomo（Clash.Meta）引擎驱动：REST/WebSocket 控制器客户端、wire DTO、运行态流、出站观测/延迟探针、路由意图切换、控制器就绪与版本能力、原生 YAML codec（含 `-t` dry-run），以及桌面进程监督（启动、就绪、PID 归属、精确终止读回与子进程回收）。

本 crate 只依赖 `lurker-core-contract`、`lurker-core-ports` 与通用库：不含任何 `infiltrator-*` 产品类型、UI 工具包或产品路径策略。产品侧端口（连接控制、运行配置、规则提供者、Geo 数据库、资源观测等）由 `mihomo-api` 实现；核心投递与版本管理由 `mihomo-version` 实现；中性 YAML 文档 codec 由本 crate 的 `codec` 拥有，`mihomo-config` 只保留 product typed 配置扩展与配置管理器。

## 特性

- `default = ["http", "process", "codec"]`
- `http`：控制器 REST 客户端、日志/连接/流量 WebSocket 流、出站探针、路由意图与就绪探测。
- `process`：桌面进程监督。仅需 HTTP 能力时（例如 Android 自管 VPNService）可关闭。
- `codec`：原生 Mihomo YAML codec（`scope`/`inspect`/编辑器片段拼接）与 `mihomo -t` 原生 dry-run。dry-run 驱动进程命令边界，因此 `codec` 总是启用 `process`。

## 模块

- `api::client`：`MihomoClient` 控制器客户端与流生命周期事件 `StreamEvent`。
- `api::proxy`：`/proxies` 观测 wire 解码（`ProxyObservation`）。
- `outbound`：`OutboundPort` 的 Mihomo 实现（清单、选择、延迟探针）。
- `routing_intent`：`RoutingIntentPort` 的 Mihomo 实现（rule/global/direct 模式读回）。
- `driver`：`CoreProcess`（`ProcessCoreController`）与 `CoreReadiness`（`ControllerReadiness`）实现。
- `process::command`：可注入的 `mihomo -t` 命令边界（`MihomoCommandPort`）。
- `version`：引擎标识、版本能力矩阵、目标制品清单与 fixture 生成。
- `types`：控制器 wire DTO。
- `codec::profile_document_codec`：中性 `ProfileDocumentCodecPort` 实现与 `PROFILE_FORMAT`/`PROFILE_SCHEMA_VERSION`。
- `codec::snippet`：字节保真的编辑器片段拼接；片段正文由产品目录以数据注入。
- `codec::config`：`mihomo -t` 原生 dry-run，候选文件写在活动配置旁，命令边界见 `process::command`。

## 运行示例

`examples/lifecycle.rs` 用引擎无关的 port 演示完整生命周期：解析环境变量 → `CoreProcess` 启动 → `CoreReadiness` 就绪与版本 → `OutboundPort` 出站清单 → `CoreProcess` 停止并核对 `Stopped` 且无 PID 的终态读回。

```bash
cargo run -p mihomo-driver --example lifecycle
```

| 环境变量 | 必填 | 说明 |
| --- | --- | --- |
| `MIHOMO_BINARY` | 是 | mihomo 可执行文件 |
| `MIHOMO_CONFIG` | 是 | mihomo 配置文档 |
| `MIHOMO_CONTROLLER` | 是 | 控制器 HTTP 端点，例如 `http://127.0.0.1:9090` |
| `MIHOMO_SECRET` | 否 | 控制器 bearer secret |

未设置 `MIHOMO_BINARY` / `MIHOMO_CONFIG` 时，示例不启动进程，只打印“未配置”的完整执行计划并成功退出，因此干净检出与 CI 无需引擎二进制也能直接运行。

## Controller 安全边界

driver 只控制本机 loopback 引擎，且只允许明文 `http`：

- 端点必须是 `http://` + IP 字面量（`127.0.0.0/8` / `::1`）+ 显式非 0 端口；域名（含 `localhost`）、`0.0.0.0`、userinfo、path/query/fragment 一律 typed 拒绝，不做静默降级。
- HTTP client 强制 `.no_proxy()`（环境代理不得接管 loopback 请求）与 `.redirect(Policy::none())`（302 不得把带 Bearer 的请求带到别处）。
- 不链接任何 TLS 后端：非 loopback controller 不在驱动契约内。
- 残留风险：明文 loopback 下，本地进程若抢先占用 controller 端口可以冒充引擎。缓解是先等引擎 ready、随机端口与随机 secret；彻底隔离需要 Unix domain socket 或 mTLS，明确不在本 crate 提供。
