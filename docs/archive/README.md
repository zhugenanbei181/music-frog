# 归档索引

> 层级：终端（已归档，冻结）

本目录保存**已被取代的历史规划、审计与决策记录**。它们冻结、不维护、**不被正文引用**，内容只作为历史证据；任何现行规则以 L2/L3 文档为准。`doc-link-guard` 不校验本目录链接，页内链接可能指向迁移前的位置。

## 归档清单

| 文档 | 原主题 | 现行取代者（纯文本，不链接） |
| --- | --- | --- |
| [TEN_PHASE_ROADMAP.md](TEN_PHASE_ROADMAP.md) | 十大演进阶段 × 100 项历史里程碑 | `docs/DUAL_SURFACE_PARITY_MASTER_PLAN.md` |
| [CORE_030_REARCHITECTURE.md](CORE_030_REARCHITECTURE.md) | 0.30 Core 破坏性重整计划 | `docs/ARCHITECTURE.md` |
| [DUAL_SURFACE_ARCHITECTURE_AUDIT_030.md](DUAL_SURFACE_ARCHITECTURE_AUDIT_030.md) | 0.30 双 UI 底层架构审计 | `docs/ARCHITECTURE.md`、`docs/UI_PARITY_AUDIT.md` |
| [MATURITY_GAP_ANALYSIS.md](MATURITY_GAP_ANALYSIS.md) | 核心/协议层 10×10 成熟度台账 | `docs/DUAL_SURFACE_PARITY_MASTER_PLAN.md`、`docs/FUNCTIONAL_MAP.md` |
| [PARITY_MIHOMO_RS.md](PARITY_MIHOMO_RS.md) | mihomo-rs 2.2 对标记录 | `docs/FUNCTIONAL_MAP.md`、`docs/MIHOMO_CORE.md` |
| [SCRIPT_ENGINE_DECISION.md](SCRIPT_ENGINE_DECISION.md) | 脚本引擎选型与迁移决策 | `docs/FUNCTIONAL_MAP.md`（脚本执行）、`docs/MIHOMO_CORE.md` |
| [ASN_LOOKUP_DECISION.md](ASN_LOOKUP_DECISION.md) | ASN/GeoIP 归属透视决策 | `docs/FUNCTIONAL_MAP.md`（连接详情事实） |
| [YAML_FIDELITY_PLAN.md](YAML_FIDELITY_PLAN.md) | YAML 注释/锚点保真迁移计划 | `docs/FUNCTIONAL_MAP.md`（配置文档与 Mixin） |
| [DUAL_SURFACE_UI_UX_ROADMAP.md](DUAL_SURFACE_UI_UX_ROADMAP.md) | 双端 UI/UX 体验演进总纲 | `docs/RESPONSIVE_PARITY_LEDGER.md`、`docs/DUAL_SURFACE_PARITY_MASTER_PLAN.md` |
| [MULTIMODAL_SHELL_MATRIX.md](MULTIMODAL_SHELL_MATRIX.md) | 组 15 历史无头回归矩阵 | `docs/RESPONSIVE_PARITY_LEDGER.md`、`docs/UI_PARITY_AUDIT.md` |
| [DUAL_SURFACE_DELIVERY_TEMPLATE.md](DUAL_SURFACE_DELIVERY_TEMPLATE.md) | 双端交付模板 | `docs/UI_PARITY_AUDIT.md` |
| [BEVY_WIDGET_EXTRACTION.md](BEVY_WIDGET_EXTRACTION.md) | 与 taskmanager 的 Bevy 控件抽取评估 | `docs/bevy-ui/BEVY_UI_FRONTEND.md` |
| [TAURI_WEBUI_RETIREMENT_LEDGER.md](TAURI_WEBUI_RETIREMENT_LEDGER.md) | Tauri/WebUI 0.20 退役台账 | `docs/PLATFORM_MATRIX.md` |

## 维护规则

- 归档文档只增不改；如需复用其中结论，先迁移为现行 L2/L3 规则，再引用现行文档。
- 不新增归档文档指向现行文档的链接；取代关系只在上表以纯文本登记。
