# 双端平权与验收

> 层级：L2

Iced 与 Bevy 是已建立平权验收关系的独立产品；Slint 是第三套独立 UI 表面，正在按共享 contract 与 owner 接入，当前页面范围尚未达到前两者的功能等价。平台能力缺失可返回带原因的 typed `Unsupported`；已承诺的同一平台产品 UI 缺入口或降级属于缺陷。Slint 每次扩展也须提交同一业务意图并显示 owner 结果，不能引入另一份业务规则。

## 规则

- 意图、结果、失败和业务状态由中性 contract/domain/application 定义；不按 UI toolkit 分支。
- 两端实现相同语义、结果、取消和恢复路径。危险操作须独立确认；布局和输入可不同，但操作必须同等可达。

## 证据

| 层 | 必须证明 | 不足以证明 |
| --- | --- | --- |
| L1 contract | 共享命令、结果和投影行为 | 类型或函数存在 |
| L2 scenario | 两端真实控件路径及适用状态分支 | 仅导航或提交命令 |
| L3 visual | 两端标准与紧凑视口的激活渲染、边界和交互区域 | 静态截图或旧构建 |

取消须无副作用；失败须可恢复。完整操作细节由产品 UI 展示，不能用 Toast、静默写入或隐藏入口替代。

## 唯一登记与验收

- 场景身份和适用分支来自 [`FeatureId::ALL`](../src/parity.rs)；[`cross_surface_manifest.tsv`](../../../scripts/parity/cross_surface_manifest.tsv) 登记两端页面，[`feature_evidence.tsv`](../../../scripts/parity/feature_evidence.tsv) 登记精确行为测试。
- [`resolve_surface_evidence.py`](../../../scripts/parity/resolve_surface_evidence.py) 对照编译后的目录与 nextest 测试 ID，检查归属、缺项和分支。
- [`visual_receipts.py`](../../../scripts/parity/visual_receipts.py) 校验激活帧、视口与交互几何；[`product_build.py`](../../../scripts/parity/product_build.py) 绑定二进制和实际构建输入。证据保存在 `.evidence/`。
- 发布验收须为两端选择具体构建并具备标准、紧凑视口回执。`pending` 是缺口；完整流程见 [parity 工具说明](../../../scripts/parity/README.md)。

## 响应式契约

断点由 `infiltrator-contract::responsive_viewport` 定义；视口变化不得删减操作。

| 视口 | 宽度 | 导航 |
| --- | --- | --- |
| Compact | `< 600px` | BottomNav |
| Medium | `600–839px` | Rail |
| Expanded | `840–1199px` | Standard |
| Ultra | `≥ 1200px` | Wide |

Iced 截图见 [本地说明](../../infiltrator-iced/docs/README.md)；Bevy 实现边界见 [本地章程](../../infiltrator-bevy-ui/docs/README.md)。
