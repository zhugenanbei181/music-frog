# 功能差距视图

> **2026-10-04 平权证据重置**：下方“99.1%”“全量闭环”等历史结论不能作为当前 L2/L3 平权依据。旧字符串 guard 已退役，225 项能力按 [UI_PARITY_AUDIT.md](docs/UI_PARITY_AUDIT.md) 重新验收；当前缺口由 `scripts/parity/` 的结构化清单与实际测试/像素解析报告给出。

本文件是差距和证据索引，不是执行顺序。执行顺序、owner 和验收条件统一放在本地 `TODO.md`；功能归属见 [docs/FUNCTIONAL_MAP.md](docs/FUNCTIONAL_MAP.md)。

Bevy Android 的平台缺口按 [平台矩阵](docs/PLATFORM_MATRIX.md) 记录；保留扩展与补全契约见 [Android 产品规范](docs/BEVY_ANDROID_PRODUCT.md)，本地 TODO 的 `BANDROID-001`～`BANDROID-017` 负责实施与验收，历史组件完成记录不替代当前宿主、APK 和设备证据。

> 状态依据当前工作树的代码盘点。`已补齐` 只表示入口或主要实现已经出现，不等于完成了跨平台行为、真实 mihomo 和发布验证。

## 当前发布阻断

当前检查点与证据入口见 [双端审计](docs/UI_PARITY_AUDIT.md#当前检查点)，执行顺序仍归属本地 `TODO.md`。

- 场景注册表的其余交互仍需真实双端状态机与两种视口验收；应用分流、语言选择和测速参数也必须覆盖能力并集。
- 全仓本地化门禁仍阻断遗留裸中文；原位语言回放不能被静态翻译标签替代。
- Bevy 生产 World/App world 入口及 59 处查询/参数 lint 豁免已撤，受限启动资产、原生查询和概览排除/恢复已有行为证据。像素发现的紧凑测速区域溢出与浅色低对比度已修复，浅色令牌同步共享契约及双端；仍不能外推为全部场景、文案与平台已经闭合。
- 遗留配置编辑仍有真实事务与草稿缺口：Hosts 等表面不能把队列提交当作应用成功，配置读取失败不能覆盖用户草稿；按场景验证取消、保存失败与重试。
- Iced 遗留配置/管理/运行态操作需核对组合宿主、命令和 reader 的所有权；Bevy Android 产品宿主仍待组合。
- 控制器观测已独立于偏好页，可缺失字段保留存在性，设置表单消费同一可选观测。模式动作已关联请求身份、代次与读写边界，两端保留错误操作面与适用重试；实际读回和原生点击有行为回归。专用运行控制/模式场景的完整原生布局与像素尚需闭合，不能据局部测试宣布控制流程完成。
- 最终平台包、远端完整验收工作流与公开发布尚未完成，模板夹具打包不算正式版本。
- MRS 观测仍含伪事实：`MrsAccelerationApplication` 按规则数估算文件/内存字节，并固定格式版本、校验通过和 mmap 开启；原生格式缺失时也有 Domain/HTTP 兜底。必须改为真实读取或明确未观测，不能将已有 mock 测试视为真实加速证明。
- 版本验证回执仍由 `MihomoVersionPort::LAST_VERIFICATION` 在进程内共享，命令与 reader 又各自创建版本服务；需改为产品实例持有、组合根共享，独立产品和来源切换不得借用另一份验证结果。下载取消的类型化整改不证明该所有权问题已解决。
- i18n 已扩大到 Iced update/state，但遗留管理通知、MRS 辅助文案、桌面订阅 OS 通知与其它后端业务字符串仍需迁移；已渲染的异步通知和错误面还需核对语言切换，不能以结构门零违规宣布全量运行态本地化完成。
- 两端壳层计数、活动配置、配额、速率和波形已回放共享真实观测；未知、零值与失效分开，移除了固定示例数字。模式与系统切换的未知、等待、失败和重试分支仍需核对，不能把局部业务场景回执外推为整个页面事实一致。规则稳定行身份与未提交表单的源绑定已有双端行为回归；紧凑构建器类型按钮已换行，但向导字段完整可达性及源变化表面的专用像素证据仍需闭合。

## 2026-09-30 历史检查点（未按现行底线复验）

本节保留当时的报告口径。旧门禁扫描范围、源码字符串检查与纸面状态不能作为现行质量或交付结论；当前事实只取自真实测试、场景像素和正式发布回执。

- `bash scripts/test.sh`：3,399/3,399 通过，0 跳过（涵盖 Iced、Bevy、Core、Desktop、Admin 及全量 Mock/Headless 业务测试）。
- `cargo fmt --all -- --check`：通过。
- `cargo clippy --workspace --all-targets -- -D warnings`：通过（0 warning, 0 error）。
- 质量守卫全量通过：`line-guard`（单文件 ≤800 行）violations=0，`test-layout-guard` violations=0，`import-guard` violations=0，`bevy_bsn_guard` violations=0，`core-boundary-guard` violations=0，`parity-guard` violations=0，`i18n-quality-guard` violations=0，`visual-regression-guard` violations=0，`verify-packaging` 17/17 通过。
- 结构债务清零：超 800 行文件与生产代码测试遗留结构性债务全部清零。
- 双端 Parity 225 项总账：223 项曾被标记为 `parity-ready` 或 `host-verified`（旧统计 99.1%，不代表交互/视觉平权），另有 2 项外部边界（13-04 内核未暴露阶段耗时，15-10 Iced 0.14 生态缺 AccessKit）。
- 现代双端 UI/UX 体验重构（UI-01 至 UI-04）：25 项优化任务全量闭环，四阶流体栅格、SDF 超椭圆连续曲率着色器、阻尼弹簧微动效、触控手势状态机与跨分辨率视觉回归流水线已全量交付。
- 仍需真实平台环境验证事项（已登记于 `TODO.md` 诚实遗留清单）：Windows/macOS 真机行为取证、TUN 真实 root 网络冒烟、安装包代码签名。

## 2026-09-08 主线稳定检查点（历史基线）

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
