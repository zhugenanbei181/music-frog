# Android 文档门

> 层级：L2

本目录是 **Android 平台**的 L2 高层文档门：汇总 Android 原生伴侣（Compose + UniFFI）与 Bevy Android 宿主的工程契约、跨端规则引用与验收分层。平台交付状态唯一来源是 [PLATFORM_MATRIX.md](../PLATFORM_MATRIX.md)。

## 子文档

| 文档 | 层级 | 角色 |
| --- | --- | --- |
| [BEVY_ANDROID_PRODUCT.md](BEVY_ANDROID_PRODUCT.md) | L2 | Bevy Android 产品工程：UI/VPN 进程所有权、原生输入、实体回收、节能与设备验收分层 |

## 相关 L2 域门

- 平台与交付状态：[PLATFORM_MATRIX.md](../PLATFORM_MATRIX.md)
- 平台交互动词契约：[PLATFORM_CONTRACTS.md](../PLATFORM_CONTRACTS.md)
- 功能归属与 Android 入口：[FUNCTIONAL_MAP.md](../FUNCTIONAL_MAP.md)
- 双端平权与场景验收：[UI_PARITY_AUDIT.md](../UI_PARITY_AUDIT.md)
- Bevy UI 章程：[../bevy-ui/BEVY_UI_FRONTEND.md](../bevy-ui/BEVY_UI_FRONTEND.md)
- 分层边界：[ARCHITECTURE.md](../ARCHITECTURE.md)

## 维护规则

- 平台状态回指平台矩阵，实施与验收任务登记在本地 `TODO.md`（`BANDROID-*`）。
- 本门与子文档不得反向引用 L1（`docs/README.md`）。
