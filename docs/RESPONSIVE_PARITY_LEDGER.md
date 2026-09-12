# 双端多尺寸弹性（Responsive Parity）权威台账

本文件是 Iced 与 Bevy UI 在**多尺寸/多形态弹性布局**上的唯一权威台账，主控对象为 `DUAL-03-14`（双端全视口响应式表现 1:1 对齐）与 `DUAL-15-01`（4 阶响应式形态断点架构）。其余文档只链接，不复制。

> 定位说明（2026-09-12）：真实宿主/发行包验证已按主线决定**挂起**。本台账的验收口径因此限定在 **mock/headless 层**：shared contract 正确、双端各自在四阶视口下产生一致的 typed 布局投影、且双端各有可执行的无头断言。`host-verified` 仍保留为目标，但当前不作为可交付里程碑。

---

## 1. 当前问题（实测证据）

2026-09-12 盘点的实测事实，这不是推测：

| 编号 | 问题 | 证据 |
| :--- | :--- | :--- |
| R-01 | **存在两套互相冲突的断点系统** | 共享契约 `infiltrator-contract::responsive_viewport::ViewportTier` 阈值为 `600 / 840 / 1200`；Bevy 实际布局引擎 `infiltrator-bevy-widgets::theme::Breakpoint` 阈值为 `600 / 1024 / 1440`。同名四阶（Compact/Medium/Expanded/Ultra）语义不一致。 |
| R-02 | **共享契约是“死契约”** | `ResponsiveViewportSnapshot` 被写入 `surface_snapshot` 与 Bevy `projection`，但 Iced 生产代码与测试**零引用**；Bevy 实际布局由 `bevy-widgets::responsive::ResponsiveContext` 驱动，投影里的 viewport 字段不参与布局。 |
| R-03 | **Iced 不响应窗口 resize** | `infiltrator-iced/src` 中不存在 `Event::Resized`、`window::resize_events` 或等价订阅；Iced 的 rail 侧栏是一个静态布局函数，不受断点驱动，因此拖拽窗口不会重排。 |
| R-04 | **Bevy 弹性网格引擎未被使用** | `infiltrator-bevy-widgets::fluid_grid` 已实现且带测试，但在 `infiltrator-bevy-ui` 中引用次数为 **0**。 |
| R-05 | **Iced 存在大量刚性尺寸** | `infiltrator-iced/src` 固定像素宽度/`Fixed` 站点 **56 处**，`max_width` 使用仅 1 处，窄视口下大概率横向溢出或截断。 |
| R-06 | **现有守卫是字符串存在性检查** | `scripts/quality/overview-responsive-viewport-guard.py` 只校验文档/模块/测试名里出现指定字符串，不校验阈值一致、不校验 Iced 消费、不校验双端测试语义。 |

结论：`DUAL-03-14` 记为 `parity-ready` 的依据不足。在 mock 层面它至少缺三块——**单一断点来源、Iced 实际消费、Bevy 实际使用弹性网格**。

---

## 2. 单一事实源决策

**唯一权威断点模型 = `infiltrator-contract::responsive_viewport`。** 两端布局都必须由它派生，禁止再出现第二套阈值。

- 四阶与阈值固定为：`Compact < 600 ≤ Medium < 840 ≤ Expanded < 1200 ≤ Ultra`（保留共享契约现值，Bevy 的 `600/1024/1440` 必须回落到该模型）。
- `infiltrator-bevy-widgets::theme::Breakpoint` 改为从 contract 派生/别名，不再自持阈值常量；阈值常量集中在 contract。
- `ViewportTier` 的布局算子扩展为覆盖双端实际需要：侧栏形态、页面内边距、网格列数、抽屉形态。算子放在 contract，两端只做 toolkit 映射。

---

## 3. 四阶形态规范（双端共同遵守）

| 阶 | 宽度 | 侧栏形态 | Overview 卡片列 | 指标网格列 | 列表/网格 | 抽屉与模态 |
| :--- | :--- | :--- | :---: | :---: | :--- | :--- |
| Compact | < 600 | BottomNav 底栏 | 1 | 2 | 单列紧凑列表 | ActionSheet 底部抽屉 |
| Medium | 600–839 | Rail 图标导轨 | 2 | 3 | 双列卡片 | ActionSheet 或窄抽屉 |
| Expanded | 840–1199 | Standard 240px 侧栏 | 2 | 6 | 双列卡片（可切列表） | 居中模态 |
| Ultra | ≥ 1200 | Wide 280px 侧栏 | 3 | 6 | 3–4 列流体网格 | 居中模态 |

「形态差异」允许，但**同一阶下两端的信息层次、可达操作与失败语义必须一致**。像素密度、动画曲线属 `local`。

---

## 4. 逐项任务与验收

状态词沿用主控台账：`planned` / `shared-ready` / `iced-ready` / `bevy-ready` / `parity-ready`。

| 项 | 任务 | 状态 | 验收（mock/headless） |
| :--- | :--- | :--- | :--- |
| `DUAL-15-01` | 统一断点事实源：contract 扩展布局算子，`Breakpoint` 回落镜像 | `parity-ready` | contract 单测覆盖四阶边界与算子；守卫数值级断言两端 600/840/1200 一致 |
| `DUAL-15-01a` | Iced 订阅窗口 resize 并驱动共享 viewport 投影 | `parity-ready` | `view_root` 经 `sidebar_for_tier` 消费 `Message::WindowResized`；Iced 无头测试断言四阶尺寸 → 形态/列数/内边距 |
| `DUAL-15-01b` | Iced 侧栏按阶切换 bottom-nav / rail / standard / wide | `parity-ready` | Compact 堆叠底栏、Medium 导轨、Expanded/Ultra 标签侧栏；`content_padding_px` 随阶收敛 |
| `DUAL-15-01c` | Overview 指标网格与 Proxies 节点网格按共享阶列数接线 | `parity-ready` | Iced 从 `metrics_grid_columns`/`proxy_grid_columns` 换行渲染；Bevy `sync_overview_metrics_columns` 与 `sync_proxies_node_columns` 覆盖 flex_basis/width；双端无头测试断言四阶列数与行内不溢出 |
| `DUAL-15-01d` | 双端从 contract 派生布局判阶 | `parity-ready` | Bevy 头less 测试在九处边界断言 `Breakpoint::from_width` 与 `ViewportTier::from_width` 同值 |
| `DUAL-15-01e` | 消除 Iced 刚性尺寸按阶收敛 | `in progress` | 已收敛：Node/聚合/Diff 模态与 Command Palette 宽度走 `clamped_modal_width`；连接详情抽屉走 `detail_panel_width_px`（窄视口近全宽）；Profiles 搜索框改为 Fill+max_width。剩余为表单标签/控件宽度（150–180px，420px 最小窗口内不溢出，记为可接受差异） |
| `DUAL-15-01f` | `responsive-parity-guard.py` 纳入 `test.sh` 与 `test-bevy.sh` | `parity-ready` | 守卫 fail-closed：断点漂移、Iced 未消费、双端网格未接线、列表页/抽屉未接线、`fluid_grid` 未注册、测试缺失均报错 |
| `DUAL-15-01g` | 分页列表页预算随窗口阶收敛 | `parity-ready` | 契约 `list_page_rows` 派生 Rules(200→66) 与 Connections(100→33) 页预算；Iced 无头测试断言四阶与恢复。Bevy 列表用 `scroll_y` 百分百高度，天然弹性 |
| `DUAL-03-14` | Overview 双端四阶视口 1:1 对齐 | `parity-ready` | 指标网格与卡片列数双端对齐；marker 守卫 + 语义测试。真实多显示器 smoke 挂起 |

**已闭环（mock 层）**：断点单一事实源、Iced resize 消费与侧栏形态切换、Overview 指标网格与 Proxies 节点网格双端按阶接线、居中模态按视口收缩、连接抽屉近全宽、分页列表预算随阶、数值级 fail-closed 守卫。
**仍未闭环**：`DUAL-15-01e` 剩余的少量表单标签/控件固定宽度（当前最小窗口下不溢出），以及第 5 节列出的逐页深度收口。

---

## 5. 页面级弹性收口顺序（11 页）

按价值与风险排序，逐页把「响应式」从写死变成从契约派生：

1. ✅ Overview：卡片网格与指标网格列数（已完成，双端按阶换行）。
2. ✅ Proxies：2–4 列卡片 ↔ 单列紧凑列表切换（已完成，双端按阶列数，`DUAL-04-13` 偏好项仍可强制单列）。
3. ✅ Connections / Logs / Rules：分页列表页预算随阶（Iced `list_page_rows`）；连接详情抽屉近全宽；Bevy 列表 `scroll_y` 百分百高度天然弹性。
4. 🔸 Settings / DNS / Doctor / AppRouting / Profiles / Sync：居中模态已按视口收缩、Profiles 搜索框已弹性化；剩余为 150–180px 表单标签/控件宽度（当前 420px 最小窗口内不溢出，记为可接受差异）。

每页完成的判定：四阶各有一条无头断言，且页面在 Compact 下**不出现横向滚动或文本截断**。

---

## 6. 与其它文档的关系

- 功能并集与状态口径：[DUAL_SURFACE_PARITY_MASTER_PLAN.md](DUAL_SURFACE_PARITY_MASTER_PLAN.md)。
- 跨端 shared/local 取舍：[FRONTENDS.md](FRONTENDS.md)（布局与密度属 `local`，但形态阶与可达性属 `shared`）。
- Iced 单端实现追溯：[ICED_CORE_MATURITY_GAPS.md](ICED_CORE_MATURITY_GAPS.md)。
- Bevy 单端实现追溯：[BEVY_CORE_MATURITY_GAPS.md](BEVY_CORE_MATURITY_GAPS.md)。
- 差距索引：[../DEFECTS.md](../DEFECTS.md)。
