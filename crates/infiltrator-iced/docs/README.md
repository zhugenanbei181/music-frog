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

矩阵一行是 `name page shape skin window_size interaction sidebar_expansion`：`page`
是表面打开的路线（如 `runtime`、`connections`、`logs`），Iced 的连接和日志入口分别
落在独立路线；`shape` 是**契约页形态**——
判定器按它选密度上限，所以 manifest 的 `page` 列写的是形态而不是路线；`interaction`
是这条行要打开的业务夹具（空＝只开页），不再由场景名推导（推导曾让「折叠侧栏」这种
非业务行的夹具 panic）；`sidebar_expansion` 是夹具发布的 owner 偏好，只有本表面
自己的两行折叠帧填它。三列都参与校验：形态必须在契约导出表里，夹具必须是
`parity.rs` 登记的业务，偏好只能是 `expanded`/`collapsed` 或空。空列在
`IFS=$'\t'` 下会被 shell 折叠成错位（Slint 侧早已踩过），两个循环都用 `awk`
把空字段换成占位符后再读。

**b189 追加**：Iced 的第一批真实像素回执（184/184 帧）由
`python3 scripts/parity/space_utilization.py --surface iced --enforce` 判定，
欠账台账 `scripts/quality/space_utilization_debt_iced.json` 首份基线 19 帧；两条腿
已接入结构门。同批修好两个真实缺陷：DNS 查询弹窗的探针要求 5 个区域却漏声明
reveal 目标（滚动区也没有 id），于是紧凑档的记录区永远被裁掉 0.8px、该帧永不发布；
以及几何判定里缺亚像素容差——布局是分数像素，区域边缘正好落在裁剪边界上时会被
浮点尘埃判成「被裁掉」，而 doctor 页的恢复面板正是如此（边界 458.3 / 456.0，
滚动区全部行程就是这 2.3px）。两处都修在根因，并有该帧的真实发布为证。

**b190 追加**：这批 Iced 密度缺口一次收口，欠账台账 **19 → 0**（`--update-debt` 只
收紧），184/184 帧仍发布自己的几何。页身有了自己的 owner：`view/components.rs` 的
`page_body_card` 让「卡就是页身」（Slint 侧的同一 owner 是 `CardHeight.page-body`），
列表在自己的滚动窗里滚、卡的地面画到页身最后一像素，`rules` 页不再包第二个滚动窗，
行不再直接画在页面地面上。回执的两个空气值不再写死 0，而由画出这些像素的外壳发布
（顶 = 标题条挂载高度（`view/chrome.rs::mounted_strip_height_px`，与 `chrome_strip`
同一挂载规则）+ 页边距；底 = 页边距）。表与地面的可辨性也进了契约：浅色页身令牌落到
阶梯的页身档（`#EFF0F3` = Slint `zinc-0` 浅色），并有逐皮肤判据要求卡至少高出地面
3 个 L\*（此前只有暗色一档，这就是「白卡压在近白地面上」能活到今天的原因）。

Overview（含 dark/forest/light 三套主题）与各业务场景 standard/compact 双视口截图已由捕获流水线完成捕获并登记入 `screenshots/manifest.tsv`；截图严格绑定当前产品构建，严禁引用旧版缓存图作为当前界面证据。

### 验收约束

- 截图必须来自原生 redraw 后的渲染帧；构建 `view()` 本身不算渲染证据。脚本保存 `rendered-frame.png`，回执校验它与合成器截图中的实际交互区域一致。
- 回执绑定产品二进制、运行资源、进程、窗口、视口、像素、几何和哈希。旧构建的截图不能代替当前构建证据。
- 截图只使用 demo 数据。发布前检查图片不含宿主用户名、路径、窗口标题、网络信息或图像元数据；不要手改 PNG 或生成清单。
- CI 不运行本机截图流程。普通截图不代表跨表面场景验收。
