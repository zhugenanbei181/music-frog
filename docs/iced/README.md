# Iced 文档门

> 层级：L2

本目录是 **Iced 产品**的 L2 高层文档门：汇总 Iced 端的章程入口、落地台账与跨端规则引用。Iced 与 Bevy UI 是同权、独立发行的产品，语义平价规则以 [UI_PARITY_AUDIT.md](../UI_PARITY_AUDIT.md) 为准。

## 子文档

| 文档 | 层级 | 角色 |
| --- | --- | --- |
| [ICED_CORE_MATURITY_GAPS.md](ICED_CORE_MATURITY_GAPS.md) | L3 | Iced 端 4 维度与各 Wave 落地状态及测试证据台账 |

## 相关 L2 域门

- 双端平权与场景验收：[UI_PARITY_AUDIT.md](../UI_PARITY_AUDIT.md)
- 双端同步与功能并集：[DUAL_SURFACE_PARITY_MASTER_PLAN.md](../DUAL_SURFACE_PARITY_MASTER_PLAN.md)
- 多 UI 求同存异：[FRONTENDS.md](../FRONTENDS.md)
- 双端多尺寸弹性：[RESPONSIVE_PARITY_LEDGER.md](../RESPONSIVE_PARITY_LEDGER.md)
- 平台与交付状态：[PLATFORM_MATRIX.md](../PLATFORM_MATRIX.md)
- 分层边界：[ARCHITECTURE.md](../ARCHITECTURE.md)

## 维护规则

- Iced 端的实现与测试证据登记在 L3 台账；上层规则不在此复制。
- 本门与子文档不得反向引用 L1（`docs/README.md`）；导航自 `AGENTS.md` 与 `docs/README.md` 自顶向下进入。
