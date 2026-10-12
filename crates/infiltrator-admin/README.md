# infiltrator-admin

> 层级：L3

提供 API-only 管理与诊断 HTTP 服务，将请求映射到共享 application/ports，并输出事件流。旧静态 Web UI 已退役。

服务不拥有独立 Core 或第二套业务语义；产品宿主负责注入同一个 Core owner。

## Outbound owner 边界

`AdminApiContext::outbound_owner` 必须从宿主注入真实 outbound port 和 Core scope source。每次读、选择或探测都从该 owner 读取当前 `CoreScope`，通过 `OutboundApplication` 校验 scope、inventory revision、集合成员和写后读回。Admin handler 不自行构造 Core scope，也不绕过注入的 typed owner。

## Typed outbound API

Outbound endpoints use the shared Core contract on the wire; Admin does not accept display names as operation targets or translate them to IDs. `GET /admin/api/outbounds` returns one `OutboundObservationSnapshot` containing the inventory revision, opaque `OutboundId` values, selection sets, and matching details.

`GET /admin/api/capabilities` now returns schema version 2, with routing intent grouped under `runtime` and inventory, selection, single probe, and batch probe grouped under `outbound`. These flags describe the Admin HTTP surface; the active Core owner still reports backend-specific `Unsupported`, stale, and unavailable outcomes through each typed operation.

`POST /admin/api/outbounds/select` accepts the shared `OutboundSelectionRequest` (`collection_id`, `outbound_id`, `expected_revision`) and returns the owner's typed inventory readback. `POST /admin/api/outbounds/probe` accepts `ProbeOutboundRequest` (`outbound_id`, `expected_inventory_revision`, `test_url`, `timeout_ms`) and returns `OutboundProbeSnapshot`. `POST /admin/api/outbounds/probe-batch` accepts `ProbeOutboundBatchRequest` including an explicit `target_scope`, `expected_inventory_revision`, URL, timeout, and bounded concurrency, and returns `OutboundProbeBatchSnapshot` with per-ID `Measured`, `TimedOut`, `Unsupported`, or `Failed` outcomes. Stale revisions and unknown/non-member IDs fail closed through the shared application contract.

An ID keeps its engine namespace on the wire, for example `{"engine_id":"mihomo","value":"GLOBAL"}`. A selection body has both opaque IDs plus the revision returned by the inventory observation:

```json
{
  "collection_id": { "engine_id": "mihomo", "value": "GLOBAL" },
  "outbound_id": { "engine_id": "mihomo", "value": "Proxy-A" },
  "expected_revision": 12
}
```

Outbound selection and probing are available through the shared typed routes above. The webhook is limited to profile, lifecycle, status, diagnostics, subscription, and host-setting actions.

Iced 桌面宿主从已有 `MihomoRuntime` 暴露其 CoreApplication 所安装的 outbound port 与 scope source，因此 Admin 请求不会创建第二份 Core/session。此边界的单测/nextest 不证明桌面运行时或真实 Mihomo 进程的 L3/L4 行为。

## Webhook v1

`POST /admin/api/webhook` 要求 JSON body 中存在字符串字段 `action`。它只接受以下 canonical snake_case 值，大小写敏感；字段缺失或类型错误会被 JSON extractor 拒绝，未知字符串会作为 typed `unsupported` 动作返回 HTTP 400。

| `action` | 直接字段 | 行为 |
| --- | --- | --- |
| `switch_profile` | `profile` | 安排宿主唯一 Core owner 应用 typed profile target；同引擎走 scoped profile command，跨引擎走 owner replacement |
| `start_core` | — | 启动 Core |
| `stop_core` | — | 停止 Core |
| `update_all` | — | 更新全部订阅 |
| `status` | — | 返回运行状态和 routing intent |
| `doctor_fix` | — | 执行诊断修复 |
| `doctor` | — | 返回诊断结果 |
| `enable_system_proxy` | — | 显式启用系统代理 |
| `disable_system_proxy` | — | 显式禁用系统代理 |

例如：`{"action":"start_core"}`。系统代理操作必须明确使用启用或禁用动作。Webhook 不接受 `intent`、`command`、通用 `payload`/`params` 字段，也不再以未知动作返回成功。
