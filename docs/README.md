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
| 发布生态门禁（SemVer / 交叉构建 / 无头沙盒 / 开发者旅程） | [RELEASE_ECOSYSTEM.md](RELEASE_ECOSYSTEM.md) |
| 中性 crate 用例展厅 | [EXAMPLES_GALLERY.md](EXAMPLES_GALLERY.md) |
| 平台能力与交付状态 | [PLATFORM_MATRIX.md](PLATFORM_MATRIX.md) |
| 代码与仓库质量底线 | [CODE_QUALITY_BASELINE.md](CODE_QUALITY_BASELINE.md) |
| 跨产品共享 UI 包与资源 | [SHARED_UI_PACKAGES.md](SHARED_UI_PACKAGES.md) |
| 批次收口八问 | [CLOSURE_QUESTIONS.md](CLOSURE_QUESTIONS.md) |
| 架构白皮书与跨 UI 框架选型指南 | [FRAMEWORK_SELECTION.md](FRAMEWORK_SELECTION.md) |
| 中立内核接入与架构迁移指南 | [MIGRATION_GUIDE.md](MIGRATION_GUIDE.md) |
| 上游依赖升级 | [UPSTREAM.md](UPSTREAM.md) |

## 包级文档入口

- Iced：[infiltrator-iced/docs/README.md](../crates/infiltrator-iced/docs/README.md)。
- Bevy UI：[infiltrator-bevy-ui/docs/README.md](../crates/infiltrator-bevy-ui/docs/README.md)。
- Bevy desktop host：[infiltrator-bevy-desktop-host](../crates/infiltrator-bevy-desktop-host/README.md)。
- Slint：[infiltrator-slint](../crates/infiltrator-slint/README.md)。
- 跨表面共享 UI 契约：[infiltrator-contract/docs/README.md](../crates/infiltrator-contract/docs/README.md)。
- 引擎/平台/UI 中性核心层：[lurker-core-contract](../crates/lurker-core-contract/Cargo.toml)、[lurker-core-ports](../crates/lurker-core-ports/Cargo.toml)、[lurker-engine-runtime](../crates/lurker-engine-runtime/README.md)、[lurker-profile-store](../crates/lurker-profile-store/README.md)、[lurker-ui-presentation](../crates/lurker-ui-presentation/README.md)、[mihomo-driver](../crates/mihomo-driver/README.md)、[singbox-driver](../crates/singbox-driver/README.md)。
- 跨表面纯呈现投影：[infiltrator-presentation/README.md](../crates/infiltrator-presentation/README.md)。
- 中性呈现内核（typed reducer 阶段、日志视口策略、本地化 key 端口）：[lurker-ui-presentation](../crates/lurker-ui-presentation/README.md)。
- Android：[android/docs/README.md](../android/docs/README.md)。
- Mihomo adapters：[mihomo-api](../crates/mihomo-api/README.md)、[MIHOMO_CORE.md](../crates/mihomo-api/docs/MIHOMO_CORE.md)、[mihomo-config](../crates/mihomo-config/README.md)、[mihomo-version](../crates/mihomo-version/README.md)。
- Mihomo YAML sync：[mihomo-dav-sync](../crates/mihomo-dav-sync/README.md)。
- sing-box adapters：[singbox-api](../crates/singbox-api/README.md)、[singbox-config](../crates/singbox-config/README.md)、[singbox-version](../crates/singbox-version/README.md)。
- Shared Core infrastructure：[ProfileStore port](../crates/infiltrator-ports/src/profile_store.rs)、[product profile store](../crates/infiltrator-profile-store/README.md)、[neutral store kernel](../crates/lurker-profile-store/README.md)、[platform adapters](../crates/infiltrator-platform/README.md)。
- Engine-neutral test fixtures：[infiltrator-test-support](../crates/infiltrator-test-support/README.md)。
- Desktop：[infiltrator-desktop](../crates/infiltrator-desktop/README.md)。
- 独立 UI 模板工程与脚手架：[templates/README.md](../templates/README.md)。

测试命令见根目录 [TESTING.md](../TESTING.md)。`docs/archive/` 是 Git 忽略的本机历史目录，不是规则来源。
