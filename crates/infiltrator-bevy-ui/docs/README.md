# Bevy UI 文档

> 层级：L3

本 crate 只拥有 Bevy 页面实现、通用宿主注入入口和隔离截图工具。桌面 `DesktopProductSession`、Core pump、command sink 与产品可执行入口由 [Bevy desktop host](../../infiltrator-bevy-desktop-host/README.md) 组合；Android driver 只注入 owner IPC client、窗口输入和 Insets。

- [BEVY_UI_FRONTEND.md](BEVY_UI_FRONTEND.md)：ECS、bsn 场景、shader 与资源生命周期章程。
- [BEVY_020_UI_CAPABILITIES.md](BEVY_020_UI_CAPABILITIES.md)：Bevy 0.20 UI 能力唯一权威规范（rem/em 尺寸、ThemeContext 分层主题、headless 控件优先、FixedNode、InlineBox、bsn_list!/Ready/despawn_all）；新 UI 代码必须遵守。**强制化台账与场景律法覆盖差表见该文「强制化台账」一节**：场景结构（`bsn!` 唯一入口 + ECS 访问边界）由 `scripts/quality/bevy_bsn_guard.py` 机器强制，rem/em 尺寸通道由 `scripts/quality/bevy_rem_guard.py` 机器强制（禁硬编码 px 字面量、禁命名/派生 px 值越出物理白名单、禁字面量 `em`、禁非 rem 字号），两者均已接入 `scripts/quality/check-structure.sh` 与 CI；共享包场景门禁 `scripts/quality/bevy_scene_gate.py` 是**上游律法差集探测器**（advisory，不阻塞、不作产品放行依据）；⑦ 小件收敛目前**仅人工评审**（由 `BEVYCAP-002.1` 收口）。

## 消费端架构纪律（强制遵照执行）

Bevy UI 表面严格作为纯被动消费端，必须遵照执行自底向上裁决铁律：
- 严禁在 Bevy 组件、资源、系统或场景中就地修补底层架构未决问题；
- 严禁在 UI 表面自行拼装、推导或维护第二份业务真源；
- 遇契约模糊或底层能力缺失时，严禁使用临时布尔标志或容错垫片绕行，必须先底层裁决收口后再接入。

- demo 页面矩阵运行 `bash scripts/capture-bevy-matrix.sh`；真实 Mihomo 首页运行 `bash scripts/capture-bevy-live.sh`。产品默认 feature 不包含截图 fixture；页面矩阵显式启用 `capture-tools`，live capture 显式启用 `capture-controller`。截图写入本地 `screenshots/`，默认不入库。
- crate 入口见 [README.md](../README.md)。
