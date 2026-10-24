# Iced 文档

> 层级：L3

Iced 产品入口见 [crate README](../README.md)。

## 消费端架构纪律

Iced 表面严格作为纯被动消费端：只消费 `SurfaceSnapshot` 权威快照并向底层发送 typed `CommandIntent`；禁止在 `Message`、`update()` 或视图组件中派生私有业务真源或暂存未决生命周期状态。

- `screenshots/`：Iced 页面截图及 manifest。

## 截图与像素证据

本流程在确定性 demo 数据上运行 Iced 产品，用私有 KWin 虚拟宿主和嵌套 niri 截图；不会连接或操作宿主桌面。截图按进程、窗口、视口和构建身份核验。

```bash
bash scripts/capture-iced-matrix.sh
INFILTRATOR_CAPTURE_SCENARIOS='<scenario>' bash scripts/capture-iced-matrix.sh
```

场景参数来自 `scripts/capture_iced_scenarios.tsv`。发布截图位于本目录的 `screenshots/`，最近运行的清单由脚本生成；完整运行日志与原始证据保存在 `.evidence/captures/iced/`。

Overview（含 dark/forest/light 三套主题）与各业务场景 standard/compact 双视口截图已由捕获流水线完成捕获并登记入 `screenshots/manifest.tsv`；截图严格绑定当前产品构建，严禁引用旧版缓存图作为当前界面证据。

### 验收约束

- 截图必须来自原生 redraw 后的渲染帧；构建 `view()` 本身不算渲染证据。脚本保存 `rendered-frame.png`，回执校验它与合成器截图中的实际交互区域一致。
- 回执绑定产品二进制、运行资源、进程、窗口、视口、像素、几何和哈希。旧构建的截图不能代替当前构建证据。
- 截图只使用 demo 数据。发布前检查图片不含宿主用户名、路径、窗口标题、网络信息或图像元数据；不要手改 PNG 或生成清单。
- CI 不运行本机截图流程。普通截图不代表跨表面场景验收。
