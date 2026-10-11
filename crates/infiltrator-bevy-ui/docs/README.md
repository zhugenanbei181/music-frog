# Bevy UI 文档

> 层级：L3

本 crate 拥有 Bevy 页面实现和截图输出。正式宿主通过注入的 `SurfaceSource`、`UiCommandSink` 与系统能力 receiver 组合 UI；桌面 product pump 和 command sink 仍由独立 `desktop-host` feature 提供，不能进入 Android driver。

- [BEVY_UI_FRONTEND.md](BEVY_UI_FRONTEND.md)：ECS、bsn 场景、shader 与资源生命周期章程。
- [BEVY_020_UI_CAPABILITIES.md](BEVY_020_UI_CAPABILITIES.md)：Bevy 0.20 UI 能力唯一权威规范（rem/em 尺寸、ThemeContext 分层主题、headless 控件优先、FixedNode、InlineBox、bsn_list!/Ready/despawn_all）；新 UI 代码必须遵守，守卫见 `scripts/quality/bevy_ui_capability_guard.py`。
- demo 页面矩阵运行 `bash scripts/capture-bevy-matrix.sh`；真实 Mihomo 首页运行 `bash scripts/capture-bevy-live.sh`。产品默认 feature 不包含截图 fixture；页面矩阵显式启用 `capture-tools`，live capture 显式启用 `capture-controller`。截图写入本地 `screenshots/`，默认不入库。
- crate 入口见 [README.md](../README.md)。
