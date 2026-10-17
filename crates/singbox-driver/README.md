# singbox-driver

> 层级：L3

sing-box 引擎驱动：Clash API 客户端与 controller preflight、`/version` readiness、routing intent 观测、outbound inventory/selector 切换、原生进程启停与 child 守护、原生 JSON document codec 与官方 `check` 事务钩子、binary 版本探测。

本 crate 只依赖 `lurker-core-contract`、`lurker-core-ports` 与通用库；不携带任何产品类型、平台宿主或 UI 依赖。能力声明证据、profile workspace CAS 事务与版本资产安装锁属产品侧。

Feature 裁剪：`default = ["http", "process", "codec"]`；`http` 为 Clash API/readiness/outbound，`process` 为进程监督，`codec` 为原生 JSON codec（依赖 `process` 的命令边界）。仅 HTTP 的构建使用 `--no-default-features --features http`。

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

## Controller 安全边界

- `external_controller` 必须是 loopback IP 字面量、固定非 0 端口；域名与 `0.0.0.0` 被 typed 拒绝。
- HTTP client 强制 `.no_proxy()` 与 `.redirect(Policy::none())`。
- 不链接任何 TLS 后端；残留的本地端口抢占风险与缓解方式同 `mihomo-driver`。
