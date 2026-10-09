# MusicFrog 文档中心

> 层级：L1

本目录是文档地图的 **L1 门**。最高入口是根目录 `AGENTS.md`（L0）。本文只登记层级、唯一权威与引用方向，不复制任何规则正文；实现细节仍以受影响 crate 的源码与测试为准。

## 一、层级与引用方向（唯一口径）

文档按权威层级组织。引用只允许 **同层或向下**，**严禁下层反向引述上层文档**；同层文档可以相互引用。层级在文件头部用一行 `层级：Lx` 标注。

| 层级 | 文档 | 角色 |
| --- | --- | --- |
| L0 | 根 `AGENTS.md` | 项目地图最高入口：身份、工程不变量、工作协议 |
| L1 | `docs/README.md`（本文） | 文档中心门与权威关系表 |
| L2 | `docs/*.md` 域门 · `docs/{iced,bevy-ui,android}/README.md` UI 门 · UI 章程 | 跨端规则、平台矩阵、工程底线、各 UI 高层门与章程 |
| L3 | `docs/iced/ICED_CORE_MATURITY_GAPS.md`、`docs/bevy-ui/BEVY_CORE_MATURITY_GAPS.md`、`docs/screenshots/README.md` | 各 UI 落地台账与证据 |
| 终端 | `docs/archive/**` | 已归档历史：冻结、不维护、不被正文引用 |

- L2 只引用 L2/L3/终端；L3 只引用 L3/终端；终端只引用终端。
- 根目录非 `AGENTS.md` 文档（`README.md`、`USAGE_SPEC.md`、`DEFECTS.md`、`TESTING.md`、`TODO.md`、`THIRD-PARTY-NOTICES.md`）是 **地图之外的叶文档**：地图可引用它们、它们也可引用地图，但不构成层级节点。根 `README.md` 面向人类、独立于本体系。

## 二、阅读顺序（自顶向下）

1. 根 `AGENTS.md`（L0）：身份、阅读顺序、工程不变量、工作协议。
2. 本文（L1）：层级、权威关系与引用方向。
3. 按任务读取 L2 域门：架构 [`ARCHITECTURE.md`](ARCHITECTURE.md)、功能归属 [`FUNCTIONAL_MAP.md`](FUNCTIONAL_MAP.md)、发布总纲 [`RELEASE_040_MASTER_PLAN.md`](RELEASE_040_MASTER_PLAN.md)、双端主控 [`DUAL_SURFACE_PARITY_MASTER_PLAN.md`](DUAL_SURFACE_PARITY_MASTER_PLAN.md)、平权验收 [`UI_PARITY_AUDIT.md`](UI_PARITY_AUDIT.md)。
4. 涉及具体 UI 时读取对应 UI 门：[Iced](iced/README.md) · [Bevy UI](bevy-ui/README.md) · [Android](android/README.md)。
5. 需要落地台账或证据时读取 L3，或按需查阅 [归档索引](archive/README.md)。

## 三、权威关系

| 主题 | 唯一权威 | 层级 |
| --- | --- | --- |
| 身份、工程不变量、工作协议 | `AGENTS.md` | L0 |
| 文档层级与权威关系 | `docs/README.md` | L1 |
| 0.40 全平台演进与交付总纲 | `docs/RELEASE_040_MASTER_PLAN.md` | L2 |
| 双端同步、功能并集与 UI 表现 | `docs/DUAL_SURFACE_PARITY_MASTER_PLAN.md` | L2 |
| 双端平权与场景验收 | `docs/UI_PARITY_AUDIT.md` | L2 |
| 架构分层与边界 | `docs/ARCHITECTURE.md` | L2 |
| 功能域唯一 Rust owner | `docs/FUNCTIONAL_MAP.md` | L2 |
| 多 UI 求同存异 | `docs/FRONTENDS.md` | L2 |
| 代码质量与工程底线 | `docs/CODE_QUALITY_BASELINE.md` | L2 |
| 平台、架构、打包与验证状态 | `docs/PLATFORM_MATRIX.md` | L2 |
| 平台交互动词契约 | `docs/PLATFORM_CONTRACTS.md` | L2 |
| Rust ↔ mihomo 核心契约 | `docs/MIHOMO_CORE.md` | L2 |
| 上游依赖版本与升级流程 | `docs/UPSTREAM.md` | L2 |
| 分层回归矩阵 | `docs/TEST_MATRIX.md` | L2 |
| 测试质量与断言契约 | `docs/TEST_GOVERNANCE.md` | L2 |
| 双端多尺寸弹性台账 | `docs/RESPONSIVE_PARITY_LEDGER.md` | L2 |
| 桌面行为隔离冒烟 | `docs/DESKTOP_SMOKE.md` | L2 |
| Bevy UI 开发章程 | `docs/bevy-ui/BEVY_UI_FRONTEND.md` | L2 |
| Bevy Android 产品工程 | `docs/android/BEVY_ANDROID_PRODUCT.md` | L2 |
| Iced 落地台账 | `docs/iced/ICED_CORE_MATURITY_GAPS.md` | L3 |
| Bevy UI 落地台账 | `docs/bevy-ui/BEVY_CORE_MATURITY_GAPS.md` | L3 |
| 双端平权剩余缺口（派生视图） | `docs/DUAL_SURFACE_GAP_LEDGER.md` | L2 |
| 产品当前状态 | `README.md` | 叶 |
| 用户使用说明 | `USAGE_SPEC.md` | 叶 |
| 工作台账 | `TODO.md` | 叶 |
| 缺陷视图 | `DEFECTS.md` | 叶 |
| 测试执行规则 | `TESTING.md` | 叶 |

## 四、UI 文档门

| UI | 门 | 章程 / 台账 |
| --- | --- | --- |
| Iced | [`iced/README.md`](iced/README.md) | 台账 `iced/ICED_CORE_MATURITY_GAPS.md` |
| Bevy UI | [`bevy-ui/README.md`](bevy-ui/README.md) | 章程 `bevy-ui/BEVY_UI_FRONTEND.md`；台账 `bevy-ui/BEVY_CORE_MATURITY_GAPS.md` |
| Android | [`android/README.md`](android/README.md) | 章程 `android/BEVY_ANDROID_PRODUCT.md` |

## 五、归档

`docs/archive/` 保存已被取代的历史规划、审计与决策记录，冻结且不被正文引用。索引见 [`archive/README.md`](archive/README.md)。

## 六、维护规则

1. 一个事实只有一个权威来源；其他文档只链接，不复制完整表格。
2. 新增文档先确定层级与归属门，再在对应父门登记；不得让下层反向引述上层。
3. 架构文档写稳定边界；临时方案、实验结果和未决事项写入本地 `TODO.md`。
4. 被取代的整篇文档移入 `docs/archive/` 并在归档索引登记；不得留在正文层。
5. `README.md`（根与各 crate）面向人类，只描述产品/模块本身，不承担导航、索引或任务路由。
6. UI 的“相同”指用户意图、数据语义、失败语义和可达性；像素、布局密度与手势可以是有记录的差异。
