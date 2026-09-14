# 功能差距视图

本文件是差距和证据索引，不是执行顺序。执行顺序、owner 和验收条件统一放在本地 `TODO.md`；功能归属见 [docs/FUNCTIONAL_MAP.md](docs/FUNCTIONAL_MAP.md)。

> 状态依据当前工作树的代码盘点。`已补齐` 只表示入口或主要实现已经出现，不等于完成了跨平台行为、真实 mihomo 和发布验证。

## 2026-09-08 主线稳定检查点

本节记录本次主线收口的实测事实，优先于下方尚未重新审计的历史差距条目：

- `bash scripts/test.sh`：2,371/2,371 通过，0 跳过；包含 Iced、Bevy 和核心/宿主
  mock/headless 测试。
- `cargo fmt --all -- --check`：通过。
- `cargo clippy --workspace --all-targets -- -D warnings`：通过。
- DUAL-01～04 纳管质量守卫：通过，`parity-guard` 报告 11 页、48 intents、0 violation。
- 仍有结构债务：测试布局守卫报告 20 项，行数守卫报告 17 个超 800 非空行文件。
- 仍缺发布级证据：真实 mihomo/controller、L3 像素捕获、真实桌面权限/网络副作用、
  Android 真机、iOS NetworkExtension，以及 Windows/macOS/Linux 打包 smoke。本次检查
  不能把 mock/headless 通过写成跨平台发布完成。

> **2026-09-12 复测**：`bash scripts/test.sh` 2,390/2,390 通过；`cargo fmt --check` 与
> `cargo clippy --workspace --all-targets -- -D warnings` 均通过。组 05～15 的逐项闭环已
> 启动：第一个 `DUAL-XX-YY` 逐项账目（组 06 测速）已展开，见主控台账；其中 Iced 伪造测速
> 数据这一 High 缺陷（D-016）已修复。测试布局与行数结构债务数量不变。

下方 D-001～D-012 保留为历史差距索引，其中个别证据仍指向 0.20/WebUI 时代；在单独
完成 0.30 差距审计前，不应把这些旧行当作当前发布准入结论。

## 2026-09-12 双端多尺寸弹性审计（mock 层）

真实宿主/发布验证已按主线决定挂起，本轮把重心放在 mock/headless 层的 UI 与功能闭环。
多尺寸弹性专项实测发现（权威台账见 [docs/RESPONSIVE_PARITY_LEDGER.md](docs/RESPONSIVE_PARITY_LEDGER.md)）：

- ~~**两套冲突断点**：共享 contract `ViewportTier` 用 `600/840/1200`，Bevy `theme::Breakpoint`
  用 `600/1024/1440`，同名四阶语义不一致。~~ **已收敛**：Bevy 镜像回落至 `600/840/1200`。
- ~~**共享契约是死契约**：`ResponsiveViewportSnapshot` 在 Iced 生产代码与测试中零引用。~~
  **已收敛**：Iced 经 `Message::WindowResized` 写入并驱动侧栏/网格/内边距。
- ~~**Iced 不监听窗口 resize**：rail 侧栏为静态布局，拖拽窗口不重排。~~ **已收敛**。
- ~~**Bevy 弹性网格未接线**：`fluid_grid` 在 `infiltrator-bevy-ui` 中引用次数为 0。~~ **已收敛**：
  `sync_overview_metrics_columns`、`sync_proxies_node_columns` 已消费其列数/基宽算子。
- **Iced 刚性尺寸**（部分收敛）：固定像素宽度/`Fixed` 站点原有 56 处；居中模态首批已改为按视口
  收缩，其余列表/表单页待续。
- ~~**守卫偏弱**：只做字符串存在性检查。~~ **已收敛**：新增 `responsive-parity-guard.py`
  做数值级 fail-closed 校验（阈值一致、Iced 消费、双端网格接线、`fluid_grid` 注册）。

| ID | 严重度 | 当前判断 | 证据 | 后续任务 |
| --- | --- | --- | --- | --- |
| D-013 | High | **已收敛（mock 层）** | 断点单源化（contract 600/840/1200，Bevy 镜像）；Iced 消费 `resize_events` 并驱动 viewport；双端 Overview/Proxies 网格按阶接线；居中模态按视口收缩；分页列表预算随阶 | `DUAL-15-01`/`DUAL-03-14`，`RESPONSIVE_PARITY_LEDGER.md` |
| D-014 | Low | 基本收敛 | 模态/抽屉/搜索框已弹性化；仅剩 150–180px 表单标签/控件宽度，在 420px 最小窗口内不溢出，记为可接受差异 | `DUAL-15-01e` |
| D-015 | Medium | **已收敛** | `responsive-parity-guard.py` 做数值级 fail-closed 校验，已入 `test.sh`/`test-bevy.sh` | `DUAL-15-01f` |
| D-016 | High | **已收敛（mock 层）** | Iced 测速曾用 UI 内硬编码的 48MB/2400ms 与假抖动样本伪造结果；已删除该第二事实源，改经 `SpeedtestPort` 驱动共享 `SpeedtestApplication` 并渲染快照，无引擎时 typed unsupported | `DUAL-06-04/05/06/08/14/15` |
| D-017 | High | **已收敛（mock 层）** | reader 的 Rule Tracer 曾是硬编码空投影，且 `RuleTracerApplication` 从未在 application `lib.rs` 挂载（死代码）。已接线：reader 经真实 `project(core, rules, active_exit, proxies)` 投影；domain 出口阶段删除「香港专线 01/28ms/HK」伪造兜底（无事实时渲染中性「未知出口」）；Iced 删除本地三元组第二事实源，经 `HostRuntime::rule_tracer_port` 驱动共享引擎并渲染五阶段链路；Bevy `RulesProjection.tracer` 数据驱动场景 | `DUAL-12-*`，组 12 逐项账目 |
| D-018 | Medium | **已收敛（mock 层）** | 组 05～15 中多项被主控台账标记 `parity-ready` 的条目实为单端。已复核并修正：`DUAL-03-12`（卡片重排）Iced 端已补齐；`DUAL-03-13`（重载蒙版）Iced 端 2026-09-13 已补齐并升回 `parity-ready` | 逐项审计 |
| D-019 | High | **已收敛（mock 层）** | Iced GeoData 面板三处伪造：`UpdateGeoDatabases` 睡 600ms 即报成功、`CheckGeoDataUpdates` 硬编码 v2026.09.01 版本与字节数、卡片对空值回填假版本/假大小。已收敛：`mihomo-api` 新增 `POST /upgrade/geo`（`upgrade_geo`，含 mockito 测试）经 `RuntimeGateway::upgrade_geo` 真实触发；无 gateway 时 typed error toast；版本/大小无事实时渲染「未知/—」；检查动作诚实提示内核未提供版本查询 | `GeoDataUpdateResult`，geodata locale keys |

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
