# mihomo-driver

> 层级：L3

Mihomo（Clash.Meta）引擎驱动：REST/WebSocket 控制器客户端、wire DTO、运行态流、出站观测/延迟探针、路由意图切换、控制器就绪与版本能力、原生 YAML codec（含 `-t` dry-run），以及桌面进程监督（启动、就绪、PID 归属、精确终止读回与子进程回收）。

本 crate 只依赖 `lurker-core-contract`、`lurker-core-ports` 与通用库：不含任何 `infiltrator-*` 产品类型、UI 工具包或产品路径策略。产品侧端口（连接控制、运行配置、规则提供者、Geo 数据库、资源观测等）由 `mihomo-api` 实现；核心投递与版本管理由 `mihomo-version` 实现；中性 YAML 文档 codec 由本 crate 的 `codec` 拥有，`mihomo-config` 只保留 product typed 配置扩展与配置管理器。

## 特性

- `default = ["http", "process", "codec", "backend"]`
- `backend`：现成的 `EngineBackendProvider<EngineBackendPorts>` 装配（隐含 `http`、`process`、`codec`），宿主无需自行拼装控制器客户端、进程、就绪、codec 与 outbound/routing 适配器。
- `http`：控制器 REST 客户端、日志/连接/流量 WebSocket 流、出站探针、路由意图与就绪探测。
- `process`：桌面进程监督。仅需 HTTP 能力时（例如 Android 自管 VPNService）可关闭。
- `codec`：原生 Mihomo YAML codec（`scope`/`inspect`/编辑器片段拼接）与 `mihomo -t` 原生 dry-run。dry-run 驱动进程命令边界，因此 `codec` 总是启用 `process`。

## 模块

- `api::client`：`MihomoClient` 控制器客户端与流生命周期事件 `StreamEvent`。
- `api::proxy`：`/proxies` 观测 wire 解码（`ProxyObservation`）。
- `backend`：现成的 `MihomoBackendOptions` 与 `MihomoEngineBackendProvider`（实现 `EngineBackendProvider<EngineBackendPorts>`）。
- `outbound`：`OutboundPort` 的 Mihomo 实现（清单、选择、延迟探针）。
- `routing_intent`：`RoutingIntentPort` 的 Mihomo 实现（rule/global/direct 模式读回）。
- `driver`：`CoreProcess`（`ProcessCoreController`）与 `CoreReadiness`（`ControllerReadiness`）实现。
- `process::command`：可注入的 `mihomo -t` 命令边界（`MihomoCommandPort`）。
- `version`：引擎标识、版本能力矩阵、目标制品清单与 fixture 生成。
- `types`：控制器 wire DTO。
- `codec::profile_document_codec`：中性 `ProfileDocumentCodecPort` 实现与 `PROFILE_FORMAT`/`PROFILE_SCHEMA_VERSION`。
- `codec::snippet`：字节保真的编辑器片段拼接；片段正文由产品目录以数据注入。
- `codec::config`：`mihomo -t` 原生 dry-run，候选文件写在活动配置旁，命令边界见 `process::command`。

## 使用驱动作为后端

启用 `backend` feature（默认启用）后，宿主只需提供已解析的二进制、活动配置、数据目录与 loopback 控制器坐标，驱动会构造 client、process、readiness、codec、未绑定的 owner scope 及 outbound/routing 适配器：

```rust
use mihomo_driver::backend::{MihomoBackendOptions, MihomoEngineBackendProvider};
use std::sync::Arc;

let options = MihomoBackendOptions::new(
    "/opt/mihomo/mihomo",
    "/data/profiles/active.yaml",
    "/data/home",
    "http://127.0.0.1:9090",
    Some("controller-secret".to_owned()),
)?;
let provider = Arc::new(MihomoEngineBackendProvider::new(options));

// 宿主把 provider 交给 lurker-engine-runtime 的
// EngineBackendProviderRegistry::new([provider]) 完成按 EngineId 注册与分派。
```

返回的 `EngineBackendPorts` 报告 `mihomo` 引擎身份，process/readiness/configuration 可用，operations 为 `Available`。owner scope 在宿主创建 Core lifecycle owner 之前保持 `Unavailable`；创建后通过 `backend.bind_owner_core_scope(owner)` 绑定，绑定前 outbound/routing 操作一律 fail closed。构造 backend 不会启动 Core、访问网络或写文件。

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

## 真实二进制验收（PUB-002.1 / PUB-002.3）

`tests/real_process_smoke.rs` 是 PUB-002.1 / PUB-002.3 要求的真实二进制验收证据，也是当前唯一能证明真实进程路径的入口：mock/fake 测试不能替代它。测试只走公开 API——用 `MihomoBackendOptions` + `MihomoEngineBackendProvider::create_backend()` 取得 `EngineBackendPorts`，经 `CoreProcess::prepare_start` / `start` 启动真实 mihomo，在 30 秒预算内轮询 `CoreReadiness` 直至就绪，用 `OutboundPort` 读回非空出站清单，最后 `CoreProcess::stop()` 并要求终态精确为 `CoreLifecycle::Stopped` 且 `pid()` 为 `None`。

该测试带 `#[ignore]`，默认质量门禁不会执行；只有显式请求时才运行。缺少任一必填环境变量会立即失败并给出变量名，不会静默跳过：

```bash
MIHOMO_REAL_BINARY=… MIHOMO_REAL_CONFIG=… MIHOMO_REAL_HOME=… MIHOMO_REAL_CONTROLLER=http://127.0.0.1:9090 cargo nextest run -p mihomo-driver --run-ignored -E 'test(real_process_smoke)'
```

| 环境变量 | 必填 | 说明 |
| --- | --- | --- |
| `MIHOMO_REAL_BINARY` | 是 | 真实 mihomo 可执行文件 |
| `MIHOMO_REAL_CONFIG` | 是 | 真实 mihomo 配置文档 |
| `MIHOMO_REAL_HOME` | 是 | 存放 PID 记录与引擎日志的数据目录（不存在时由 harness 创建） |
| `MIHOMO_REAL_CONTROLLER` | 是 | loopback 控制器端点，例如 `http://127.0.0.1:9090` |
| `MIHOMO_REAL_SECRET` | 否 | 控制器 bearer secret |

## Controller 安全边界

driver 只控制本机 loopback 引擎，且只允许明文 `http`：

- 端点必须是 `http://` + IP 字面量（`127.0.0.0/8` / `::1`）+ 显式非 0 端口；域名（含 `localhost`）、`0.0.0.0`、userinfo、path/query/fragment 一律 typed 拒绝，不做静默降级。
- HTTP client 强制 `.no_proxy()`（环境代理不得接管 loopback 请求）与 `.redirect(Policy::none())`（302 不得把带 Bearer 的请求带到别处）。
- 不链接任何 TLS 后端：非 loopback controller 不在驱动契约内。
- 残留风险：明文 loopback 下，本地进程若抢先占用 controller 端口可以冒充引擎。缓解是先等引擎 ready、随机端口与随机 secret；彻底隔离需要 Unix domain socket 或 mTLS，明确不在本 crate 提供。
