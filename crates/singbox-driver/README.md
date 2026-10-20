# singbox-driver

> 层级：L3

sing-box 引擎驱动：Clash API 客户端与 controller preflight、`/version` readiness、routing intent 观测、outbound inventory/selector 切换与 typed 节点测速、原生进程启停与 child 守护、原生 JSON document codec 与官方 `check` 事务钩子、binary 版本探测。

本 crate 只依赖 `lurker-core-contract`、`lurker-core-ports` 与通用库；不携带任何产品类型、平台宿主或 UI 依赖。能力声明证据、profile workspace CAS 事务与版本资产安装锁属产品侧。

Feature 裁剪：`default = ["http", "process", "codec", "backend"]`；`backend` 为现成的 `EngineBackendProvider<EngineBackendPorts>` 装配（隐含 `http`、`process`、`codec`），`http` 为 Clash API/readiness/outbound，`process` 为进程监督，`codec` 为原生 JSON codec（依赖 `process` 的命令边界）。仅 HTTP 的构建使用 `--no-default-features --features http`。

## 使用驱动作为后端

启用 `backend` feature（默认启用）后，宿主只需提供已解析的二进制、活动配置、稳定 Core 实例标识与 loopback Clash API 坐标，驱动会构造 client、process、readiness、codec、未绑定的 owner scope 及 outbound/routing 适配器：

```rust
use lurker_core_contract::identity::CoreInstanceId;
use singbox_driver::backend::{SingBoxBackendOptions, SingBoxEngineBackendProvider};
use std::sync::Arc;

let options = SingBoxBackendOptions::new(
    "/opt/sing-box/sing-box",
    "/data/profiles/active.json",
    CoreInstanceId::new("desktop-active")?,
    "127.0.0.1:9090",
    Some("controller-secret".to_owned()),
)?;
let provider = Arc::new(SingBoxEngineBackendProvider::new(options));

// 宿主把 provider 交给 lurker-engine-runtime 的
// EngineBackendProviderRegistry::new([provider]) 完成按 EngineId 注册与分派。
```

返回的 `EngineBackendPorts` 报告 `sing-box` 引擎身份，process/readiness/configuration 可用，operations 为 `Available`。owner scope 在宿主创建 Core lifecycle owner 之前保持 `Unavailable`；创建后通过 `backend.bind_owner_core_scope(owner)` 绑定，绑定前 outbound/routing 操作一律 fail closed。构造 backend 不会启动 Core、访问网络或写文件。就绪等待上限由宿主在启动 Core 后自行施加；`SingBoxBackendOptions::with_readiness_timeout` 只是把该上限随配置携带出来，backend 本身不等待。

## 运行示例

`examples/lifecycle.rs` 用引擎无关的 port 演示完整生命周期：解析环境变量 → 读取 profile 中 `experimental.clash_api` → `CoreProcess` 启动 → `CoreReadiness` 就绪与版本 → `OutboundPort` 出站清单 → `CoreProcess` 停止并核对 `Stopped` 且无 PID 的终态读回。

```bash
cargo run -p singbox-driver --example lifecycle
```

| 环境变量 | 必填 | 说明 |
| --- | --- | --- |
| `SINGBOX_BINARY` | 是 | sing-box 可执行文件 |
| `SINGBOX_CONFIG` | 是 | sing-box JSON 配置文档 |
| `SINGBOX_CONTROLLER` | 是 | loopback 控制器地址，须与 profile 的 `external_controller` 一致，例如 `127.0.0.1:9090` |
| `SINGBOX_SECRET` | 否 | 须与 profile 的 `clash_api.secret` 一致 |

未设置 `SINGBOX_BINARY` / `SINGBOX_CONFIG` 时，示例不启动进程，只打印“未配置”的完整执行计划并成功退出，因此干净检出与 CI 无需引擎二进制也能直接运行。

## 真实二进制验收（PUB-002.1 / PUB-002.3 / PUB-006.8）

`tests/real_process_evidence.rs` 是 PUB-006.8 要求的真实二进制验收入口，也是当前唯一能证明真实进程路径的入口：mock/fake 测试不能替代它。测试只走公开 API——用 `SingBoxBackendOptions` + `SingBoxEngineBackendProvider::create_backend()` 取得 `EngineBackendPorts`，经 `CoreProcess::prepare_start` / `start` 启动真实 sing-box，并对运行中的引擎证明四件事：

| 证明 | 内容 | typed 结果 |
| --- | --- | --- |
| `api_handshake` | Clash API 握手与活引擎版本读回 | `CoreReadiness::probe` / `active_core_version` |
| `node_speed_test` | 对配置好的出站做节点测速 | `OutboundPort::probe` 的 typed 结果：实测延迟、引擎自身超时、typed 失败或不支持 |
| `traffic_dispatch` | 经引擎自己的 SOCKS5 监听器把真实流量送到 loopback fixture | fixture 收到并回显同一批字节 |
| `configuration_write` | 向运行中的引擎写配置并读回 | `OutboundPort::select` 写后读回 |

环境契约是 typed 的：`tests/real_process_evidence/contract.rs` 的 `RealProcessEnvironment` 只有三个互斥结果——`Absent`（环境没有引擎二进制）、`Present`（可运行的引擎目标）、`Invalid`（环境指向不可用的引擎）。`Absent` 产出 `environment_absent` 回执并让测试失败，绝不静默通过；`Invalid` 产出 `invalid_environment` 回执。

回执是机器可读的 JSON：每条证明一个 typed 结果，整体结论由证明记录推导（`complete` / `unsupported_proof` / `defect`），因此回执不可能声称超过证明本身的事实。回执写入 `SINGBOX_REAL_RECEIPT` 指定的文件，并始终打印到 stdout（`MUSIC_FROG_REAL_PROCESS_RECEIPT <json>`）。

### 节点测速：typed 端口与引擎的 delay 接口

驱动的 typed 出站端口 `OutboundPort::probe` 现在真的测速：它向引擎自己的 delay 端点发 `GET /proxies/{name}/delay?url={target}&timeout={ms}`，并按引擎自己的答案回报 typed 状态。引擎（sing-box 1.13 与 1.14）的答案与驱动的映射一一对应：

| 引擎答案 | 驱动的 typed 状态 |
| --- | --- |
| `200` + `{"delay": N}` | `OutboundProbeState::Measured { delay_ms: N }` |
| `504`（引擎自己的 delay 测试超时） | `OutboundProbeState::TimedOut` |
| `503`（delay 测试失败）、`500` | `OutboundProbeState::Failed { failure }`（`Network`） |
| `400`（引擎拒绝该请求） | `OutboundProbeState::Failed { failure }`（`InvalidInput`） |
| `401` / `403`（控制器拒绝 secret） | `OutboundProbeState::Failed { failure }`（`Authentication`） |
| `404` + 引擎自己的 `{"message": ...}` 错误体（引擎不认识该出站） | `OutboundProbeState::Failed { failure }`（`InvalidState`） |
| 引擎自己的错误体以外的答案（delay 路由没有应答） | `OutboundProbeState::Unsupported` |
| `200` 但响应体不是测量值 | `OutboundProbeState::Failed { failure }`（`InvalidState`） |
| 驱动自己的截止时间到（引擎没有在期限内应答） | `OutboundProbeState::TimedOut` |
| 传输层失败 | `OutboundProbeState::Failed { failure }`（`Network`） |

没有任何一种答案会被改写成另一种：测不到就是测不到，失败就是失败，超时就是超时。

上表的每一种答案都用真实 sing-box 1.13.14 与 v1.14.3 的 delay 端点逐条核对过：`200` 的测量体、`400` 的 `{"message":"Body invalid"}`、`404` 的 `{"message":"Resource not found"}`、路由缺失时 `404` 的纯文本 `404 page not found`、`503` 的 `{"message":"An error occurred in the delay test"}` 与 `504` 的 `{"message":"Timeout"}` 都与上表一致。

**超时预算与重试策略**（与 mihomo 驱动同语义）：引擎自己执行 delay 预算（`timeout` 查询参数按有符号 16 位毫秒数解析，因此超过 32767 ms 的预算在发请求之前就被 typed 拒绝）；驱动的截止时间只覆盖引擎的应答，等于引擎预算加 2 秒宽限；传输层超时再留 1 秒余量，所以驱动的截止时间始终是“应答是否迟到”的唯一权威，typed 结果不取决于两个计时器谁先触发。驱动本身不重试；证据 harness 在有界预算内重试失败的测速，而 typed 的 `Unsupported` 结局直接返回、从不重试。

**引擎的目标策略**：sing-box 的 delay 端点只测 HTTPS 目标——`http://` 开头的目标会被引擎替换成它自己的 URL test 目标。驱动把请求的目标原样发出并回报引擎公布的测量值，不改写目标也不猜测被替换成了什么；需要显式目标时，调用方传 HTTPS URL 即可。

该测试带 `#[ignore]`，默认质量门禁不会执行；只有显式请求时才运行：

```bash
SINGBOX_REAL_BINARY=… SINGBOX_REAL_CONFIG=… SINGBOX_REAL_INSTANCE=… \
SINGBOX_REAL_CONTROLLER=127.0.0.1:9090 \
SINGBOX_REAL_PROXY=127.0.0.1:1080 SINGBOX_REAL_FIXTURE=127.0.0.1:9099 \
SINGBOX_REAL_PROBE_OUTBOUND=DIRECT SINGBOX_REAL_SELECTOR=SELECT \
cargo nextest run -p singbox-driver --test real_process_evidence --run-ignored only
```

| 环境变量 | 必填 | 说明 |
| --- | --- | --- |
| `SINGBOX_REAL_BINARY` | 是 | 真实 sing-box 可执行文件 |
| `SINGBOX_REAL_CONFIG` | 是 | 真实 sing-box JSON 配置文档 |
| `SINGBOX_REAL_INSTANCE` | 是 | 稳定的 `CoreInstanceId` 字符串 |
| `SINGBOX_REAL_CONTROLLER` | 是 | loopback Clash API 地址，例如 `127.0.0.1:9090` |
| `SINGBOX_REAL_PROXY` | 是 | 引擎自己的 SOCKS5 监听器，例如 `127.0.0.1:1080` |
| `SINGBOX_REAL_FIXTURE` | 是 | loopback 流量 fixture，例如 `127.0.0.1:9099` |
| `SINGBOX_REAL_PROBE_OUTBOUND` | 是 | 测速证明所测量的出站名 |
| `SINGBOX_REAL_SELECTOR` | 是 | 配置写入证明所切换的组名 |
| `SINGBOX_REAL_SECRET` | 否 | 控制器 bearer secret |
| `SINGBOX_REAL_PROBE_TARGET` | 否 | 测速证明所测量的 HTTPS 目标；缺省是引擎自己的 URL test 目标 `https://www.gstatic.com/generate_204` |
| `SINGBOX_REAL_RECEIPT` | 否 | 回执写入的文件 |

测速证明需要一个引擎能到达的 HTTPS 目标：sing-box 不测 `http://` 目标，所以本腿不像 mihomo 腿那样用本机 HTTP 夹具，而是测引擎的 URL test 目标。目标不可达时引擎回答 `503`，驱动回报 typed 失败，证明如实记为失败，绝不伪造一个测量值。

本地可复跑入口是 `scripts/core-backend-smoke-singbox.sh`（`--driver-evidence-only` 只跑驱动证明）。脚本从契约文件读取环境变量名，自己写好隔离的配置文档、端口与 fixture 地址，并把机器可读摘要 `MUSIC_FROG_SMOKE_SUMMARY <json>` 打到 stdout；`scripts/quality/real-process-evidence-guard.py` 守住这份证据不许退化。

## Controller 安全边界

- `external_controller` 必须是 loopback IP 字面量、固定非 0 端口；域名与 `0.0.0.0` 被 typed 拒绝。
- HTTP client 强制 `.no_proxy()` 与 `.redirect(Policy::none())`。
- 不链接任何 TLS 后端；残留的本地端口抢占风险与缓解方式同 `mihomo-driver`。
