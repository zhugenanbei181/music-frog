# Iced 文档

> 层级：L3

Iced 产品入口见 [crate README](../README.md)。

## 消费端架构纪律

Iced 表面严格作为纯被动消费端：只消费 `SurfaceSnapshot` 权威快照并向底层发送 typed `CommandIntent`；禁止在 `Message`、`update()` 或视图组件中派生私有业务真源或暂存未决生命周期状态。

用户可见文案统一从 `infiltrator-contract::shared_copy` 的 key 经共享本地化资源解析；Iced 不持有第二份产品文案，也不得对 locale 字符串做裁剪派生（原 `short_label()` 已退役）。文案言简意赅：中文条目不夹注英文译名，括号只留配置字段、单位或协议标识。滚动条统一走 `infiltrator-contract::scrollbar` 的悬浮自动隐退规范：静止不可见、仅被指针悬停或拖拽的对应轴显影，并为内容保留 16px 安全槽。`scrollbar-contract-guard.py` 强制 Iced 所有滚动区域经 `auto_hide_scrollable`/`modern_scrollable` 构造。

侧栏是**纯导航**（对标开源 FlClash 桌面侧栏，依据 `docs/reference/FLCLASH_DESKTOP_REFERENCE.md` §2 的逐像素对照）：行集唯一来自共享 typed 注册表 `shell_navigation::desktop_items(SurfaceKind::IcedDesktop)`，圆角、选中态（全宽淡底 + 指示条）、图标语义与分组全部读契约，页面不得再声明第二份目的地表或路由→图标表。壳层面的事实按唯一 owner 归位：品牌与窗口标题、实时速率、运行状态点归**标题栏**（`view/chrome.rs`，`ShellA11yNode::GlobalStatusDot` 按契约注释「与 header actions 同排」挂在标题栏）；路由前进/后退同样归标题栏；系统代理/TUN 主开关与出站模式归**概览页**卡片（`view/overview_master_switches.rs`、`view/overview_routing_intent_segment.rs`）；活动配置与用量归概览配额卡；版本归关于页；折叠页（DNS/诊断/应用路由/同步/工具箱/关于/免责/许可）由**设置页索引**按 `pages_folded_under(ShellPage::Settings)` 列出，索引列表本身也由契约决定。空间利用率强制标准把这条钉成门禁：导航栈之下 `chrome_band` 必须为 0%，任何状态卡、品牌块或版本行都不得回到侧栏。

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
