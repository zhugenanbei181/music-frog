# MusicFrog 文档中心

本目录记录项目当前有效的架构、功能边界、跨平台策略和上游依赖规则。实现细节仍以受影响 crate 的源码和测试为准；本目录不承担临时开发流水。

## 阅读顺序

代码变更先核对 [CODE_QUALITY_BASELINE.md](CODE_QUALITY_BASELINE.md) 的全仓工程底线。

1. [README.md](../README.md)：产品定位、发行形态和用户可见能力。
2. [DUAL_SURFACE_PARITY_MASTER_PLAN.md](DUAL_SURFACE_PARITY_MASTER_PLAN.md)：**【最高主控台账】**双端（Iced & Bevy UI）同步演进、10 大业务组功能并集与 UI 表现规范。
3. [UI_PARITY_AUDIT.md](UI_PARITY_AUDIT.md)：同权产品、同异律、L1/L2/L3 场景验收与真实证据闭环。
4. [ARCHITECTURE.md](ARCHITECTURE.md)：Rust、mihomo、宿主与多个 UI 的分层边界。
5. [CORE_030_REARCHITECTURE.md](CORE_030_REARCHITECTURE.md)：0.20 基线与 0.30 破坏性 Core 重整计划。
6. [RELEASE_040_MASTER_PLAN.md](RELEASE_040_MASTER_PLAN.md)：**【0.40 实施总纲】**全平台生产级宿主交付、多端生态闭环（Core、Iced 3 桌面端纯 Wayland、Bevy UI 5 跨端平台、原生 Android 客户端）。
7. [FUNCTIONAL_MAP.md](FUNCTIONAL_MAP.md)：按功能域查找唯一 owner、各端入口和待办编号。
8. [FRONTENDS.md](FRONTENDS.md)：Iced 与 Bevy UI 对等双主干、Android 的求同存异矩阵。
9. [RESPONSIVE_PARITY_LEDGER.md](RESPONSIVE_PARITY_LEDGER.md)：双端多尺寸弹性的断点单一事实源、四阶形态规范与逐页收口台账。
10. [DUAL_SURFACE_UI_UX_ROADMAP.md](DUAL_SURFACE_UI_UX_ROADMAP.md)：**【体验演进总纲】**双端 UI/UX 体验演进与视觉系统主控台账（响应式布局、连续曲率圆角 Squircle、着色器动效与按图索骥推进清单）。
11. [MULTIMODAL_SHELL_MATRIX.md](MULTIMODAL_SHELL_MATRIX.md)：组 15 的历史无头矩阵；场景交付证据已迁移到 `scripts/parity/`。
12. [MIHOMO_CORE.md](MIHOMO_CORE.md)：Rust 操作 mihomo 的核心契约、生命周期和安全边界。
13. [PLATFORM_MATRIX.md](PLATFORM_MATRIX.md)：平台、架构、打包和验证状态。
14. [UPSTREAM.md](UPSTREAM.md)：Rust、mihomo、Web、Android 依赖的版本与升级流程。
15. [TEST_MATRIX.md](TEST_MATRIX.md)：功能域、UI、平台和真实 core 的分层回归矩阵。
16. [BEVY_ANDROID_PRODUCT.md](BEVY_ANDROID_PRODUCT.md)：Bevy Android 的保留扩展、服务进程隔离、原生输入、实体回收、事件驱动节能与设备验收规范。
## 文档与待办的权威关系

| 内容 | 唯一入口 | 规则 |
| --- | --- | --- |
| 0.40 全平台演进与交付实施总纲 | `docs/RELEASE_040_MASTER_PLAN.md` | Core、Iced 3 桌面端纯 Wayland、Bevy UI 5 跨端平台、原生 Android 客户端的权威实施总纲与分阶段小任务 |
| 双端同步与功能并集 | `docs/DUAL_SURFACE_PARITY_MASTER_PLAN.md` | Iced 与 Bevy UI 同步演进、功能并集与 UI 表现的权威规范 |
| 0.20/0.30 Core 重整 | `docs/CORE_030_REARCHITECTURE.md` | 0.20 冻结基线；0.30 允许破坏性替换，领域/契约/端口边界以本文件为准 |
| 产品当前状态 | `README.md` | 只写当前可验证事实，不写开发流水 |
| 代码质量与工程底线 | `docs/CODE_QUALITY_BASELINE.md` | 全仓文件预算、明确导入、禁止转发/规避、验证要求；测试不豁免 |
| 架构和边界 | `docs/ARCHITECTURE.md` | 变更先更新边界，再改实现 |
| 功能归属 | `docs/FUNCTIONAL_MAP.md` | 一项功能只指定一个逻辑 owner |
| 双端产品平权与场景验收 | `docs/UI_PARITY_AUDIT.md` | 同异律与三层证据；场景身份只来自 `FeatureId::ALL`，状态只由结构化清单与真实证据解析 |
| UI 求同存异 | `docs/FRONTENDS.md` | 每个前端必须显式选择 shared/local/accepted difference/unsupported |
| Bevy UI 开发规范 | `docs/BEVY_UI_FRONTEND.md` | 声明式场景、受限 ECS、模块化 WESL、GPU ABI、handle owner/复用/回收与分层验证；业务和截图激活禁止整仓 World 访问 |
| Bevy Android 产品工程 | `docs/BEVY_ANDROID_PRODUCT.md` | 现有能力保留扩展，Android UI/VPN 宿主组合、资源治理和设备验收；当前平台状态仍归平台矩阵，排期归本地 TODO |
| Android CI 与测试分层 | `.github/workflows/android.yml` + `docs/BEVY_ANDROID_PRODUCT.md` §9 | 编译门/清单守卫/单元/插桩/真机证据分层；产品状态仍归平台矩阵 |
| 双端多尺寸弹性 | `docs/RESPONSIVE_PARITY_LEDGER.md` | 断点单一事实源、四阶形态规范与逐页弹性收口；`DUAL-03-14`/`DUAL-15-01` 的权威验收台账 |
| 双端 UI/UX 与视觉演进 | `docs/DUAL_SURFACE_UI_UX_ROADMAP.md` | 双端 UI/UX 体验、全流体响应式、设计令牌、连续曲率圆角与微交互的权威实施台账 |
| 多模态外壳回归矩阵 | `docs/MULTIMODAL_SHELL_MATRIX.md` | 组 15 逐项状态与双侧证据；历史证据标记待按 `scripts/parity/` 重新验收 |
| Iced 落地台账 | `docs/ICED_CORE_MATURITY_GAPS.md` | Iced 端 4 维度与各 Wave 落地状态及测试证据 |
| Bevy UI 落地台账 | `docs/BEVY_CORE_MATURITY_GAPS.md` | Bevy UI 端 10 维度 150 项工程缺口落地状态及无头测试证据 |
| 核心协议与 AST 台账 | `docs/MATURITY_GAP_ANALYSIS.md` | 核心层与协议层 10×10 成熟度全景差距台账 |
| mihomo-rs 对标 | `docs/PARITY_MIHOMO_RS.md` | mihomo-rs v2.2 对标与差距补齐记录 |
| 平台交互动词契约 | `docs/PLATFORM_CONTRACTS.md` | 0.30 平台交互动词契约：每类 OS 交互动词一个统一 Rust trait，不支持组合显式 `Unsupported` |
| 上游依赖 | `docs/UPSTREAM.md` | 版本真相来自 manifest/lockfile/脚本，不在多个文档手抄 |
| 回归证据 | `docs/TEST_MATRIX.md` + `TESTING.md` | 测试命令和测试覆盖矩阵分开维护 |
| 工作台账 | 本地 `TODO.md` | 被 `.gitignore` 忽略，按任务 ID 与验收条件维护 |
| 缺陷视图 | `DEFECTS.md` | 只描述差距和证据，具体执行顺序回指 `TODO.md` |
| 用户使用说明 | `USAGE_SPEC.md` | 只描述已经存在或明确承诺的用户操作 |
| 测试执行规则 | `TESTING.md` | 记录命令、隔离策略和回归入口 |
| 桌面冒烟验证 | `docs/DESKTOP_SMOKE.md` | 桌面真实行为隔离冒烟验证（SNI 托盘、通知与 XDG/dconf 副作用） |

## 维护规则

0. `README.md`（根与各 crate）面向人类，只描述产品/模块本身；项目地图以顶层 [AGENTS.md](../AGENTS.md) 为最高入口，逐层细化到本页与其登记的各 canonical 文档。README 不承担导航、索引或任务路由。
1. 一个事实只有一个权威来源，其他文档只链接，不复制完整表格。
2. 架构文档写稳定边界；临时方案、实验结果和未决事项写入本地 `TODO.md`。
3. TODO 只有在代码、行为测试和适用的平台/打包证据都具备时才能标记 `DONE`。
4. 上游升级必须同时检查 API/配置兼容性、锁文件、许可证、二进制校验和回滚路径。
5. UI 的“相同”指用户意图、数据语义、失败语义和可达性；像素、布局密度和手势可以是有记录的差异。
6. 历史报告（如旧 `ISSUE_RESOLVED.md`、`FIX_SUMMARY.md`）已移除；需要复用其中结论时，先迁移为当前规则。
