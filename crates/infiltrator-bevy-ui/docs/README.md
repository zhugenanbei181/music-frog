# Bevy UI 文档

> 层级：L3

本 crate 只拥有 Bevy 页面实现、通用宿主注入入口和隔离截图工具。桌面 `DesktopProductSession`、Core pump、command sink 与产品可执行入口由 [Bevy desktop host](../../infiltrator-bevy-desktop-host/README.md) 组合；Android driver 只注入 owner IPC client、窗口输入和 Insets。

- [BEVY_UI_FRONTEND.md](BEVY_UI_FRONTEND.md)：ECS、bsn 场景、shader 与资源生命周期章程。
- [BEVY_020_UI_CAPABILITIES.md](BEVY_020_UI_CAPABILITIES.md)：Bevy 0.20 UI 能力唯一权威规范（rem/em 尺寸、ThemeContext 分层主题、headless 控件优先、FixedNode、InlineBox、bsn_list!/Ready/despawn_all）；新 UI 代码必须遵守，守卫见 `scripts/quality/bevy_ui_capability_guard.py`。
- demo 页面矩阵运行 `bash scripts/capture-bevy-matrix.sh`；真实 Mihomo 首页运行 `bash scripts/capture-bevy-live.sh`。产品默认 feature 不包含截图 fixture；页面矩阵显式启用 `capture-tools`，live capture 显式启用 `capture-controller`。截图写入本地 `screenshots/`，默认不入库。
- crate 入口见 [README.md](../README.md)。
