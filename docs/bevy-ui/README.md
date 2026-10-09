# Bevy UI 文档门

> 层级：L2

本目录是 **Bevy UI 产品**的 L2 高层文档门：汇总 Bevy UI 的开发章程、落地台账与跨端规则引用。Bevy UI 与 Iced 是同权、独立发行的产品；求同存异矩阵见 [FRONTENDS.md](../FRONTENDS.md)，平权验收见 [UI_PARITY_AUDIT.md](../UI_PARITY_AUDIT.md)。

## 子文档

| 文档 | 层级 | 角色 |
| --- | --- | --- |
| [BEVY_UI_FRONTEND.md](BEVY_UI_FRONTEND.md) | L2 | Bevy UI 前端章程：受限 ECS、声明式场景、WESL/GPU 资产、验收命令 |
| [BEVY_CORE_MATURITY_GAPS.md](BEVY_CORE_MATURITY_GAPS.md) | L3 | Bevy UI 端 10 维度 150 项工程缺口落地状态及无头测试证据台账 |

## 相关 L2 域门

- 双端平权与场景验收：[UI_PARITY_AUDIT.md](../UI_PARITY_AUDIT.md)
- 双端同步与功能并集：[DUAL_SURFACE_PARITY_MASTER_PLAN.md](../DUAL_SURFACE_PARITY_MASTER_PLAN.md)
- 双端多尺寸弹性：[RESPONSIVE_PARITY_LEDGER.md](../RESPONSIVE_PARITY_LEDGER.md)
- Bevy Android 产品工程：[../android/BEVY_ANDROID_PRODUCT.md](../android/BEVY_ANDROID_PRODUCT.md)
- 代码质量底线：[CODE_QUALITY_BASELINE.md](../CODE_QUALITY_BASELINE.md)
- 分层边界：[ARCHITECTURE.md](../ARCHITECTURE.md)

## 维护规则

- 开发约束以章程为准；缺口与证据登记在 L3 台账，不在本门复制。
- 本门与子文档不得反向引用 L1（`docs/README.md`）。
