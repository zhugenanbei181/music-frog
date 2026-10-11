# 公共文档地图

> 层级：L1

根 `docs/` 只保存跨 crate、跨 UI 或跨平台的公共规则。crate、UI 和 Android 产品专属文档放在所属包的 `docs/`，由对应 README 导航。

文档按公共契约、具体实现和本地叶台账分层。本文只登记公共文档入口；实现细则由所属产品入口导航。

第三方组件与许可证记录见 [第三方声明](../THIRD-PARTY-NOTICES.md)。

| 公共主题 | 唯一入口 |
| --- | --- |
| 分层与依赖边界 | [ARCHITECTURE.md](ARCHITECTURE.md) |
| 功能域 owner | [FUNCTIONAL_MAP.md](FUNCTIONAL_MAP.md) |
| Core 后端中性契约 | [CORE_BACKENDS.md](CORE_BACKENDS.md) |
| 平台能力与交付状态 | [PLATFORM_MATRIX.md](PLATFORM_MATRIX.md) |
| 代码与仓库质量底线 | [CODE_QUALITY_BASELINE.md](CODE_QUALITY_BASELINE.md) |
| 跨产品共享 UI 包 | [SHARED_UI_PACKAGES.md](SHARED_UI_PACKAGES.md) |
| 跨表面本地化与视觉资源 | [SHARED_UI_RESOURCES.md](SHARED_UI_RESOURCES.md) |
| 上游依赖升级 | [UPSTREAM.md](UPSTREAM.md) |

## 包级文档入口

- Iced：[infiltrator-iced/docs/README.md](../crates/infiltrator-iced/docs/README.md)。
- Bevy UI：[infiltrator-bevy-ui/docs/README.md](../crates/infiltrator-bevy-ui/docs/README.md)。
- Slint：[infiltrator-slint/docs/README.md](../crates/infiltrator-slint/docs/README.md)。
- 双端共享 UI 契约：[infiltrator-contract/docs/README.md](../crates/infiltrator-contract/docs/README.md)。
- 跨表面纯呈现投影：[infiltrator-presentation/README.md](../crates/infiltrator-presentation/README.md)。
- Android：[android/docs/README.md](../android/docs/README.md)。
- Mihomo API：[mihomo-api/docs/README.md](../crates/mihomo-api/docs/README.md)。
- Desktop：[infiltrator-desktop/docs/README.md](../crates/infiltrator-desktop/docs/README.md)。

测试命令见根目录 [TESTING.md](../TESTING.md)。`docs/archive/` 是 Git 忽略的本机历史目录，不是规则来源。
