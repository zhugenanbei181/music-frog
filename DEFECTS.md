# 功能差距视图

本文件是差距和证据索引，不是执行顺序。执行顺序、owner 和验收条件统一放在本地 `TODO.md`；功能归属见 [docs/FUNCTIONAL_MAP.md](docs/FUNCTIONAL_MAP.md)。

> 状态依据当前工作树的代码盘点。`已补齐` 只表示入口或主要实现已经出现，不等于完成了跨平台行为、真实 mihomo 和发布验证。

## 差距列表

| ID | 严重度 | 当前判断 | 证据 | 后续任务 |
| --- | --- | --- | --- | --- |
| D-001 | High | 已补齐入口，待控制平面收敛 | `crates/infiltrator-admin/src/admin_api.rs` 已有 runtime connections/logs/traffic/memory/IP/delay 路由 | CORE-001/003、FUNC-002 |
| D-002 | High | 已补齐入口，待跨 UI 平价 | `webui/config-manager-ui/src/App.vue` 已挂载 RuntimePanel | FUNC-002、UI-003 |
| D-003 | Medium | 已补齐入口，待统一结果语义 | `mihomo-api::test_delay`、Admin API runtime delay 路由和 RuntimePanel 均存在 | FUNC-002、QA-001 |
| D-004 | Medium | 已补齐主要流程，待交付验证 | Admin API 已有 core versions/latest/download/update/activate 路由，Iced 也有 core update state | CORE-006、QA-004 |
| D-005 | Medium | 部分完成 | `infiltrator-core` 和 UI 已有 DNS/Fake-IP/TUN/rules/providers/sniffer 的结构化/JSON 路径 | FUNC-003、CORE-005 |
| D-006 | High | 主要入口已补齐，待 shared contract | Android profiles 已有 create/select/save/delete、local import 和 subscription settings 路径 | FUNC-001、UI-004 |
| D-007 | High | Rust FFI 已成为主要来源，仍需 canonical 审计 | `AppRoutingViewModel` 通过 `appRoutingLoad/SetMode/TogglePackage`，Rust 侧有 `app_routing_*` | PLAT-002、UI-004 |
| D-008 | Medium | 已补齐入口，待回归 | Android `App.kt` 已路由 Connections，UniFFI 已提供 list/close | FUNC-002、QA-001 |
| D-009 | Medium | 已补齐入口，待与桌面语义对齐 | Android Overview 已包含 rule/global/direct/script 四种模式 | FUNC-002、UI-004 |
| D-010 | Medium | 部分完成 | Android 已暴露 `fallback-filter`、`stack`、`auto-detect-interface` 等字段，但完整字段矩阵仍未建立 | FUNC-003、CORE-005 |
| D-011 | Low | 已补齐入口，待错误/网络策略审计 | Android Overview 已调用 `ipCheck`；Rust 实现仍需纳入统一诊断契约 | FUNC-002、QA-001 |
| D-012 | High | 持续开放 | 现有单元/API 测试较多，但缺少 core version × platform × UI 的统一矩阵 | QA-001/002/004 |

## 使用原则

1. 先处理 `CORE-*`、`QA-*` 和会阻塞多端的 `FUNC-*`，不要按旧的 A/B 编号继续扩张。
2. 差距状态必须由代码、行为测试和适用平台证据共同决定；不能因为页面出现就标记完成。
3. 新发现先写入对应功能域的 TODO，再在本表增加证据；本表不保存临时实现流水。
