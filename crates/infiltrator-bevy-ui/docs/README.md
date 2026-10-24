# Bevy UI 文档

> 层级：L3

本 crate 只拥有 Bevy 页面实现、通用宿主注入入口和隔离截图工具。桌面 `DesktopProductSession`、Core pump、command sink 与产品可执行入口由 [Bevy desktop host](../../infiltrator-bevy-desktop-host/README.md) 组合；Android driver 只注入 owner IPC client、窗口输入和 Insets。

- [BEVY_UI_FRONTEND.md](BEVY_UI_FRONTEND.md)：ECS、bsn 场景、shader 与资源生命周期章程。
- [BEVY_020_UI_CAPABILITIES.md](BEVY_020_UI_CAPABILITIES.md)：Bevy 0.20 UI 能力唯一权威规范（rem/em 尺寸、ThemeContext 分层主题、headless 控件优先、FixedNode、InlineBox、bsn_list!/Ready/despawn_all）；场景结构守卫（`bevy_bsn_guard.py`）、尺寸通道守卫（`bevy_rem_guard.py`）与强制化台账均以此规范为单一真源。

## 消费端架构纪律

Bevy UI 表面严格作为纯被动消费端：仅消费宿主注入的 `SurfaceSource` 并向 `UiCommandSink` 派发 typed 意图命令；禁止在 ECS System、Resource、Component 或 `bsn!` 场景中派生第二份业务真源或暂存底层未决状态。

- demo 页面矩阵运行 `bash scripts/capture-bevy-matrix.sh`；真实 Mihomo 首页运行 `bash scripts/capture-bevy-live.sh`。产品默认 feature 不包含截图 fixture；页面矩阵显式启用 `capture-tools`，live capture 显式启用 `capture-controller`。截图写入本地 `screenshots/`，默认不入库。
- crate 入口见 [README.md](../README.md)。
