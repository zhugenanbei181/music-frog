# Bevy UI Core Maturity Gaps Ledger (Bevy UI 前端深度成熟度全景台账)

本文档归档 MusicFrog Infiltrator 项目中 `infiltrator-bevy-ui` 与 `infiltrator-bevy-widgets` 前端在演进至成熟生产级桌面与移动统一客户端（对标 Clash Verge Rev、Mihomo Party、Flclash）过程中的 10 大核心维度与 150 项深度工程缺口，作为后续实施的权威交付台账。

> **双端对齐与主纲从属说明（2026-09-03）**：
> 本文档是最高主控台账 [DUAL_SURFACE_PARITY_MASTER_PLAN.md](DUAL_SURFACE_PARITY_MASTER_PLAN.md) 在 `infiltrator-bevy-ui` 前端的专属落地执行切片。本文列出的 10 大维度与 Master Plan 的 10 大业务组 1:1 对齐，所有特性与 Iced 前端保持同步推进与对等验收。

---

## 一、10 大核心维度全景落地矩阵

| 维度 | 序号 | 核心演进域 | 演进目标与 Bevy 原生落地路径 | 核心代码模块与依赖 |
| :--- | :--- | :--- | :--- | :--- |
| **维度一** | 1~15 | **全局框架、导航解耦与自适应多模态布局** | 动态路由标题解耦、侧边栏 11 页全量纳管、移动端 4 键紧凑底栏 + 侧滑抽屉 (Drawer)、Ctrl+K 命令面板、无边框窗口自适应。 | `infiltrator-bevy-ui/src/app.rs`<br>`infiltrator-bevy-ui/src/route.rs`<br>`infiltrator-bevy-widgets/src/drawer.rs` |
| **维度二** | 16~30 | **核心概览、网络拓扑与动态遥测中枢** | 真实双通道上下行实时波形、分流网络可视化拓扑链 (Inbound->Rule->Group->Outbound)、订阅用量进度条、系统代理/TUN 主控双大卡、Mini HUD 悬浮窗。 | `infiltrator-bevy-ui/src/pages/overview.rs`<br>`infiltrator-bevy-widgets/src/chart.rs`<br>`infiltrator-bevy-widgets/src/desktop.rs` |
| **维度三** | 31~45 | **代理策略、节点矩阵与智能测速系统** | 自适应 2~4 列响应式卡片网格、拼音/协议即时搜索过滤、单策略组独立并发测速、动态脉冲骨架屏微动效、节点详情抽屉、URI 编解码导入导出。 | `infiltrator-bevy-ui/src/pages/proxies.rs`<br>`infiltrator-bevy-widgets/src/fluid_grid.rs`<br>`infiltrator-core/src/profile_converter/` |
| **维度四** | 46~60 | **配置订阅、版本管理与规则混入生态** | 多渠道三合一导入对话框、定时自动轮询更新、内嵌 YAML 编辑器、配置快照历史与秒级回滚、可视化 Diff 比对、多订阅聚合器、QuickJS 脚本沙箱。 | `infiltrator-bevy-ui/src/pages/profiles.rs`<br>`infiltrator-bevy-widgets/src/editor.rs`<br>`infiltrator-core/src/script_engine.rs` |
| **维度五** | 61~75 | **分流规则、规则集管理与实时追踪体系** | 交互式实时分流追踪器 (Live Rule Tracer)、分流决策链树状回放、50,000+ 规则 $O(1)$ 高性能虚拟视口滚动、外部规则集 (Rule Providers) 管理、MRS 二进制加速。 | `infiltrator-bevy-ui/src/pages/rules.rs`<br>`infiltrator-bevy-widgets/src/list/mod.rs`<br>`infiltrator-core/src/mrs.rs` |
| **维度六** | 76~90 | **连接审计、多维聚合与深度链路透视** | 高并发连接多条件过滤、平面/按进程/按域名三向聚合、链路透视侧滑抽屉 (Connection Drawer)、DNS/TCP/TLS/TTFB 耗时瀑布流、反向一键生成规则。 | `infiltrator-bevy-ui/src/pages/connections.rs`<br>`infiltrator-bevy-ui/src/view_root/connection_drawer.rs` |
| **维度七** | 91~105 | **运行日志、DNS 探针与系统自愈健康** | 环形流式日志缓冲区、语义化色彩徽章、Regex 正则过滤、智能滚屏锁定、DNS 泄漏多源探测、Fake-IP 映射表检索、全面 Doctor 自愈套件与看门狗。 | `infiltrator-bevy-ui/src/pages/logs.rs`<br>`infiltrator-bevy-ui/src/pages/dns.rs`<br>`infiltrator-bevy-ui/src/pages/doctor.rs` |
| **维度八** | 106~120 | **应用级分流、进程控制与桌面移动平台特化** | 动态系统进程枚举、应用高清图标提取缓存、场景预分类、单应用代理/直连/拦截三态切换、Windows UWP 回环解除、Android VpnService 分应用代理。 | `infiltrator-bevy-ui/src/pages/app_routing.rs`<br>`infiltrator-desktop/src/process_enumerator/`<br>`android/` |
| **维度九** | 121~135 | **云端同步、系统全局设置与高级网络栈** | 多云端后端同步 (WebDAV/Gist/iCloud)、三向差异合并 (3-Way Merge)、异常断电系统代理自愈复位、TUN 协议栈灵活切换 (gVisor/System)、桌面系统托盘生态。 | `infiltrator-bevy-ui/src/pages/sync.rs`<br>`infiltrator-bevy-ui/src/pages/settings.rs`<br>`infiltrator-desktop/src/tray_badge.rs` |
| **维度十** | 136~150 | **引擎调步、低功耗渲染与底层组件架构** | BEVY-005 实时内核双向管道、Cadence 多模态渲染调步（后台 2 FPS）、Reactive 事件驱动重绘、IME 中文输入法深度跟踪、AccessKit 屏幕阅读器语义补齐。 | `infiltrator-bevy-ui/src/controller.rs`<br>`infiltrator-bevy-widgets/src/cadence.rs`<br>`infiltrator-bevy-widgets/src/reactive.rs` |

---

## 二、10 大类 150 项深度工程细化台账

### 大类一：全局框架、导航解耦与自适应多模态布局 (Shell, Navigation & Layout)
1. **BEVY-GAP-001 [DONE] 动态标题与面包屑解耦**：消除 `content_title_row` 中硬编码的 `"核心概览"` 文本；新增 `ContentTitleLabel` 组件与 `ActiveRoute` 观察者联动，动态派发当前页面名称与层级面包屑。模块：`crates/infiltrator-bevy-ui/src/app.rs`。验收：11 个路由的标题文本完全与路由对齐，无头测试断言通过。
2. **BEVY-GAP-002 [DONE] 桌面侧边栏全量入口补齐与状态回显**：重构 `nav_column_scene`，纳管 `Route::ALL` 全部 11 个页面；新增 `sync_nav_item_visuals` 系统，在 `ActiveRoute` 变化时执行原地 compare-and-set 换肤（底色与文字墨色更新，实体 ID 零重建）。模块：`crates/infiltrator-bevy-ui/src/app.rs`。验收：所有页面侧栏可见并能准确高亮。
3. **BEVY-GAP-003 侧栏功能模块层级分组与折叠**：将侧栏划分为“核心业务”（概览/代理/订阅/规则）、“运维审计”（连接/日志/DNS/自愈）与“高级系统”（应用分流/同步/设置）三大分组，基于 `accordion_scene` 支持展开与折叠。模块：`crates/infiltrator-bevy-widgets/src/accordion.rs`。验收：支持持久化保存各分组折叠状态。
4. **BEVY-GAP-004 [DONE] 移动端紧凑底栏四键收敛**：废除移动端竖屏 (<600px) 塞入 11 个入口的设计，底栏固定只展示“概览、代理、配置、更多”4 项核心导航。模块：`crates/infiltrator-bevy-ui/src/app.rs` (`bottom_nav_scene`)。验收：390x844 竖屏视口下单项宽度不小于 80px，彻底消除文字挤压与穿模。
5. **BEVY-GAP-005 呼出式侧滑抽屉导航 (Navigation Drawer)**：点击移动端底栏“更多”触发 `OpenDrawer` 事件，基于 `drawer_scene` 在屏幕侧边平滑滑出覆盖层，纳管其余 7 个次级页面入口。模块：`crates/infiltrator-bevy-widgets/src/drawer.rs`。验收：半透明遮罩 (Scrim) 拦截外部点击，选定或点击遮罩平滑收回。
6. **BEVY-GAP-006 平板与分屏极简 Rail 模式 (600px~1024px)**：中等宽度视口下侧栏自适应收窄为 64px 纯图标导轨，文字标签隐藏，鼠标悬停时通过 `tooltip_scene` 浮现页面名称。模块：`crates/infiltrator-bevy-ui/src/app.rs`。验收：1024x768 下文字自动隐藏，图标居中，Tooltip 正确指示。
7. **BEVY-GAP-007 [DONE] 顶部状态栏全局聚合指示器**：在顶部标题行右侧集成全局常驻状态指示：内核运行状态圆点（绿/黄/灰/红）、代理模式胶囊、系统代理切换 Switch、TUN 模式切换 Switch。模块：`crates/infiltrator-bevy-ui/src/app.rs`。验收：无需跳出当前页面即可监控并切换核心开关。
8. **BEVY-GAP-008 快捷动作托盘区域 (Quick Action Tray)**：在标题栏整合“一键更新订阅”、“一键清空 DNS 缓存”、“重启内核”与“复制终端代理命令”高频快捷按钮。模块：`crates/infiltrator-bevy-ui/src/app.rs`。验收：点击直接触发命令并派发 Toast 提示。
9. **BEVY-GAP-009 跨平台无边框窗口控件与拖拽热区**：为无边框窗口模式定义 `WindowDragArea` 组件以及最小化/最大化/关闭按钮，适配 Wayland、X11、Windows 与 macOS 的原生阴影与拖拽行为。模块：`crates/infiltrator-bevy-widgets/src/windowing.rs`。验收：双击最大化/还原，拖拽平稳，控件响应迅速。
10. **BEVY-GAP-010 全键盘命令面板 (Command Palette - Ctrl+K)**：监听 `Ctrl+K` 全局快捷键，弹出居中悬浮模糊检索面板；支持按拼音首字母和英文搜索直达 11 个页面、切换代理模式与开关 TUN。模块：`crates/infiltrator-bevy-ui/src/command_palette.rs`。验收：全键盘上下键浏览、Enter 执行、Esc 退出。
11. **BEVY-GAP-011 [DONE] 深度双向路由历史栈 (Back/Forward Navigation)**：扩充 `RouteHistory`，在顶部标题行增加返回（<）与前进（>）按钮，监听鼠标侧键与键盘 `Alt+Left/Right`。模块：`crates/infiltrator-bevy-ui/src/route.rs`。验收：页面下钻后可平滑回退，栈底时返回按钮置灰禁用。
12. **BEVY-GAP-012 独立全屏模态管理层 (Adaptive Modal Layer)**：建立顶层 `ModalOverlayHost` 实体，业务页面通过派发 `SpawnModal(Scene)` 事件挂载对话框，自动处理背景暗化与焦点陷阱 (Focus Trap)。模块：`crates/infiltrator-bevy-widgets/src/adaptive_modal.rs`。验收：弹窗拦截底层点击，Esc 触发层级关闭。
13. **BEVY-GAP-013 移动端触控手势识别引擎 (Gesture Engine)**：实现纯核触控手势状态机，支持屏幕左边缘右滑返回（Swipe Back）与列表顶部下拉刷新（Pull-to-Refresh）。模块：`crates/infiltrator-bevy-widgets/src/gesture.rs`。验收：无头测试模拟 Touch 序列正确触发事件，Android 实机跟手响应。
14. **BEVY-GAP-014 页面平滑过渡缓动动效 (Motion Transition)**：在 `sync_route` 挂载新页面场景时注入 `TransitionAlpha(0.0)`，并在 120ms 内通过 ECS 插值缓动至 1.0，配合 Y 轴 4px 微位移。模块：`crates/infiltrator-bevy-widgets/src/motion.rs`。验收：页面切换消除硬切撕裂感，低功耗模式自动跳过。
15. **BEVY-GAP-015 路由守卫与异常重定向 (Route Guard)**：派发 `RouteChanged` 前检查全局状态：若本地订阅为空自动引导至 Profiles 导入页；若内核连续启动失败则强制跳转 Doctor 自愈页。模块：`crates/infiltrator-bevy-ui/src/route.rs`。验收：配置为空时首次启动自动拦截重定向。

### 大类二：核心概览、网络拓扑与动态遥测中枢 (Overview & Telemetry)
16. **BEVY-GAP-016 真实双通道实时流量曲线**：彻底废除静态正弦波模拟代码；后台泵从内核读取实时上下行字节差分，通过贝塞尔样条平滑算法分别绘制上传（绿）与下载（蓝）双曲线。模块：`crates/infiltrator-bevy-ui/src/pages/overview.rs`、`crates/infiltrator-bevy-widgets/src/chart.rs`。验收：空闲时诚实归零，大流量传输时与系统网速吻合。
17. **BEVY-GAP-017 流量波形动态量程标尺与发光着色器**：维护最近 60 秒环形采样，根据瞬时峰值速率动态自适应调整 Y 轴最大刻度（如 1MB/s、10MB/s、100MB/s），曲线下方叠加线性渐变发光半透明填充。模块：`crates/infiltrator-bevy-widgets/src/chart.rs`。验收：高低网速下波形均饱满不溢出。
18. **BEVY-GAP-018 [DONE] 分流网络可视化拓扑链 (Traffic Topology Chain)**：在概览页黄金区域渲染 4 节点拓扑链：`Inbound (Mixed:7890)` -> `RuleSet (分流规则)` -> `Group (策略组)` -> `Outbound (出口节点)`，带有流动微发光指示箭头。模块：`crates/infiltrator-bevy-ui/src/pages/overview.rs`。验收：消除下半部 40% 全黑留白，随流量实时高亮活动链。
19. **BEVY-GAP-019 [DONE] 当前主出口节点高保真卡片**：大卡片显式展示当前主策略组选中的出口节点名称、所属国家/地区旗帜图标、协议类型（Hysteria2/VLESS/Trojan/SS）及最新测速延迟色块。模块：`crates/infiltrator-bevy-ui/src/pages/overview.rs`。验收：点击该卡片可直接下钻至代理节点选择面板。
20. **BEVY-GAP-020 [DONE] 订阅配额可视化进度条卡片**：读取活动 Profile 的 `SubscriptionInfo`，渲染已用流量/总配额对比条（如 `46.43 GB / 186.26 GB`）、使用百分比与账单重置倒计时天数。模块：`crates/infiltrator-bevy-ui/src/pages/overview.rs`。验收：配额超 85% 预警黄，超 95% 告警红，无订阅显示本地模式。
21. **BEVY-GAP-021 [DONE] 系统代理与 TUN 模式双主卡切换**：设计大卡片形式的系统代理与 TUN 接管开关，内置带有微触觉反馈的滑动 Switch，标明当前网络栈接管状态与驱动类型。模块：`crates/infiltrator-bevy-ui/src/pages/overview.rs`。验收：点击立即触发设置变更，加载期显示 Spinner，权限不足提示引导。
22. **BEVY-GAP-022 代理运行模式快速切换分段器**：优化“规则模式 / 全局模式 / 直连模式 / 脚本模式”四段分段控制器，移至概览页核心显眼区域，支持微胶囊滑动背景过渡。模块：`crates/infiltrator-bevy-ui/src/pages/overview.rs`。验收：点击即刻下发 `PATCH /configs`，状态确认后高亮胶囊平滑滑动。
23. **BEVY-GAP-023 首页全局一键并发测速按钮**：在概览页头部集成“一键测速”按钮；点击后并发触发所有激活策略组的延迟测试，并在按钮旁呈现环形百分比进度反馈。模块：`crates/infiltrator-bevy-ui/src/pages/overview.rs`。验收：测速中按钮显示进度动画，完成后刷新出口卡片与各组延迟。
24. **BEVY-GAP-024 核心运维多维统计卡片网格**：扩充指标卡片为 6 项网格：活跃连接数、内核内存占用、CPU 负载、当前上传瞬时速率、当前下载瞬时速率、累计会话总流量。模块：`crates/infiltrator-bevy-ui/src/pages/overview.rs`。验收：每 700ms 刷新，数据变动时仅更新 Text 组件，零全屏重排。
25. **BEVY-GAP-025 内核故障预警横幅与自愈入口**：核心通信中断、TUN 异常丢失或端口冲突时，概览页顶层自动淡入警示红/橙色横幅，说明故障原因并提供“一键自愈修复”直达按钮。模块：`crates/infiltrator-bevy-ui/src/pages/overview.rs`。验收：模拟核心挂起即刻出横幅，点击一键跳转自愈页执行修复。
26. **BEVY-GAP-026 公网出口 IP 与隐私归属探针**：增加出口探测卡片，展示当前实际访问外网的公网 IP 地址、经纬度地理位置徽标及 ISP 运营商名称，提供“刷新探活”按钮。模块：`crates/infiltrator-bevy-ui/src/pages/overview.rs`。验收：探针流量强制走当前代理路由出站，节点切换后数据联动刷新。
27. **BEVY-GAP-027 拓扑链路节点下钻跳转交互**：为分流拓扑中的各节点注册点击观察者：点击 Inbound 跳设置端口页，点击 RuleSet 跳规则页，点击 Proxy Group/Node 跳节点策略页并高亮。模块：`crates/infiltrator-bevy-ui/src/pages/overview.rs`。验收：点击拓扑任一部分均可直达底层配置，交互完全闭环。
28. **BEVY-GAP-028 首页仪表盘模块卡片自定义重排**：实现网格与卡片的轻量纵向重排状态机；允许用户长按拖拽调整流量图、拓扑链、指标网格与出口卡片的先后顺序与可见性。模块：`crates/infiltrator-bevy-widgets/src/reorderable.rs`。验收：拖拽产生浮起阴影，顺序序列化存入本地配置，重启保持。
29. **BEVY-GAP-029 极简桌面网速悬浮窗 (Mini HUD)**：支持将核心概览分离为独立小窗（尺寸 260x90，常驻置顶、半透明、无边框），展示上下行波形图、当前节点与极简开关。模块：`crates/infiltrator-bevy-ui/src/app.rs`。验收：主窗口最小化后悬浮窗独立刷新，支持鼠标拖拽放置并记忆坐标。
30. **BEVY-GAP-030 内核重载断线优雅降级处理**：更新配置导致内核重启时（通常约 200~800ms），概览页卡片保持显示上一帧有效快照并覆以轻微蒙版指示“重载中”，重载完毕无感恢复。模块：`crates/infiltrator-bevy-ui/src/controller.rs`。验收：配置切换过程中界面不出现“数据归零”或“报错红闪”。

### 大类三：代理策略、节点矩阵与智能测速系统 (Proxies & Policies)
31. **BEVY-GAP-031 [DONE] 响应式自适应节点卡片网格**：彻底淘汰单列通栏排版；基于约束构建流体网格，依可用宽度自适应调整为 2 列（移动/分屏）、3 列（标准桌面）或 4 列（宽屏），单卡高度 68px。模块：`crates/infiltrator-bevy-ui/src/pages/proxies.rs`。验收：一屏容纳 12~20 个节点，信息密度提升 300%，卡片整齐对齐。
32. **BEVY-GAP-032 [DONE] 节点多模式模糊搜索与拼音过滤**：顶部工具栏常驻搜索输入框，支持输入中文名称、国家简称、节点协议（如 `vless`）、延迟范围（如 `<100`）或汉字拼音首字母（如 `xg` 匹配香港）。模块：`crates/infiltrator-bevy-ui/src/pages/proxies.rs`。验收：输入字符即时过滤，打字无掉帧延迟，按 Esc 一键清空。
33. **BEVY-GAP-033 多维复合排序引擎与状态记忆**：提供排序模式下拉菜单：按测速延迟升序、按延迟降序、按字母名称排序、按配置原始顺序；偏好设置按策略组独立记录并持久化。模块：`crates/infiltrator-bevy-ui/src/pages/proxies.rs`。验收：切换排序即刻重排，测速刷新后节点平滑滑移到新位置，选中标记保持。
34. **BEVY-GAP-034 [DONE] 单策略组独立并发测速控制**：在每个策略组标题卡片右侧提供“单独测速”闪电按钮；仅对该策略组下挂载的节点并发派发测速请求，杜绝盲目全局测速挤占带宽。模块：`crates/infiltrator-bevy-ui/src/pages/proxies.rs`。验收：点击仅刷新该组节点延迟，其他组不受影响，支持中途取消测速。
35. **BEVY-GAP-035 策略组自定义测速目标与超时阈值**：允许为各策略组自定义设置测速 URL（如 Google、Cloudflare、Bilibili、YouTube）以及超时毫秒阈值（默认 5000ms），专向测试流媒体线路。模块：`crates/infiltrator-bevy-ui/src/pages/proxies.rs`。验收：测速针对目标地址生效，组标题旁呈现测速目标域名简标。
36. **BEVY-GAP-036 动态脉冲骨架屏微动效 (Skeleton Box)**：当某个节点处于测速中时，其延迟文本区域进入渐变脉冲呼吸动效，测速返回后执行 180ms 翻转动画显示毫秒读数。模块：`crates/infiltrator-bevy-widgets/src/motion.rs`。验收：测速过程中视觉反馈流畅积极，杜绝界面假死停滞感。
37. **BEVY-GAP-037 语义化延迟色阶与超时告警标红**：严格规范延迟色谱：<100ms 为亮绿，100~250ms 为明黄，250~500ms 为橙色，>500ms 为深红警示，超时或连接拒绝显示深灰色与 Timeout 标识。模块：`crates/infiltrator-bevy-widgets/src/palette.rs`。验收：明暗双主题下对比度达标，色盲模式提供辅助形状徽标。
38. **BEVY-GAP-038 策略组树形折叠持久化与一键收拢**：顶部提供“全部折叠 / 全部展开”快捷开关；用户手动折叠的状态记录在本地偏好中，并在路由跳转或软件重启后保持原样。模块：`crates/infiltrator-bevy-ui/src/pages/proxies.rs`。验收：大量策略组场景下一键收拢，仅展开关注组，页面清晰干练。
39. **BEVY-GAP-039 策略组优先级纵向拖拽重排**：允许用户长按策略组卡片把手并上下拖拽，调整常用的“节点选择”、“国外媒体”排在最上方；提供“恢复默认顺序”一键重置功能。模块：`crates/infiltrator-bevy-ui/src/pages/proxies.rs`。验收：拖拽时带有浮起阴影，松开后组顺序保存并更新到视图中。
40. **BEVY-GAP-040 节点技术底层元数据详情抽屉**：点击节点卡片上的详情信息图标，弹出抽屉或模态窗口，展示节点服务器 IP、端口、UUID/密码哈希、加密方式、TLS ServerName、ALPN、Reality 公钥及 UDP 支持情况。模块：`crates/infiltrator-bevy-ui/src/pages/proxies.rs`。验收：支持一键脱敏复制节点配置信息到系统剪贴板。
41. **BEVY-GAP-041 手动录入自定义节点表单向导**：提供标准化的新增节点表单向导（支持 Shadowsocks, VMess, Trojan, VLESS Reality, Hysteria2, WireGuard），实时验证必填字段与端口范围。模块：`crates/infiltrator-bevy-ui/src/pages/proxies.rs`。验收：保存后自动追加至本地自定义节点组中并持久化，支持立即测速。
42. **BEVY-GAP-042 标准节点分享链接 (URI) 解析与导出**：集成通用 URI 编解码引擎，支持一键解析剪贴板内的 `ss://`、`vmess://`、`vless://`、`trojan://`、`hysteria2://` 链接；支持将节点导出为 URI 或二维码。模块：`crates/infiltrator-core/src/profile_converter/uri_parse.rs`。验收：单链接和多行链接批量解析成功率 100%。
43. **BEVY-GAP-043 故障死节点一键隐藏与过滤**：顶部工具栏增加“隐藏不可用”切换开关；开启后自动隐藏测速超时、握手错误或连通性失败的节点，仅展示健康存活节点。模块：`crates/infiltrator-bevy-ui/src/pages/proxies.rs`。验收：测速完成后不可用节点自动淡出并隐藏，节点网格紧凑重排。
44. **BEVY-GAP-044 [DONE] 核心节点星标置顶与偏好锁定**：节点卡片右侧带有星标图标；点击点亮星标后，该节点被锁定置顶在策略组网格的首位，不受任何延迟排序或字母排序的重排影响。模块：`crates/infiltrator-bevy-ui/src/pages/proxies.rs`。验收：多策略组间独立记忆各自的置顶节点，重启保持。
45. **BEVY-GAP-045 外部代理提供商 (Proxy Provider) 监控**：在页面底部独立归纳 Proxy Providers 列表，展示各提供商来源 URL、节点总数、健康检查间隔、上次更新时间与手动触发同步按钮。模块：`crates/infiltrator-bevy-ui/src/pages/proxies.rs`。验收：点击拉取可即时从云端同步该 Provider 的节点池。

### 大类四：配置订阅、版本管理与规则混入生态 (Profiles & Subscriptions)
46. **BEVY-GAP-046 多渠道三合一配置导入对话框**：提供标签式导入弹窗：支持从网络 URL 导入（自动探测机场 User-Agent 并携带 Token）、本地 YAML 文件拖拽导入、剪贴板一键提取链接导入。模块：`crates/infiltrator-bevy-ui/src/pages/profiles.rs`。验收：输入无效 URL 时显示明确 HTTP 错误，拖拽损坏文件提示具体报错行。
47. **BEVY-GAP-047 多配置定时自动更新与轮询计划**：支持为每个订阅独立设置自动更新周期（如 6 小时、12 小时、24 小时或不自动更新）；界面显示上次更新成功时间和下一次自动同步倒计时。模块：`crates/infiltrator-admin/src/scheduler/subscription.rs`。验收：后台定时器在应用最小化状态下准时静默更新。
48. **BEVY-GAP-048 订阅卡片上下文操作菜单**：在每个订阅卡片右侧集成功能菜单：立即更新、复制链接、重命名、编辑配置源码、复制配置副本、删除配置（带二次确认防误触弹窗）。模块：`crates/infiltrator-bevy-ui/src/pages/profiles.rs`。验收：操作流畅快捷，删除活动配置时弹出阻止警示并要求用户先切到备用配置。
49. **BEVY-GAP-049 嵌入式高性能 YAML 代码编辑器**：集成基于虚拟视口的高性能纯文本编辑器核，支持行号展示、语法高亮标记、自动缩进、括号补全与非法 YAML 语法行内标红。模块：`crates/infiltrator-bevy-widgets/src/editor.rs`。验收：流畅加载与滚动 10,000 行复杂配置文件，保存前执行语法合法性预校验。
50. **BEVY-GAP-050 配置快照版本管理与秒级安全回滚**：每次订阅远程更新或手动保存配置时，系统自动对前一版本进行本地快照归档（最多保留 20 份）；配置列表展示版本时间轴，支持一键回滚。模块：`crates/infiltrator-core/src/profiles.rs`。验收：回滚后即刻无缝重启内核，订阅下发错误配置时可一键回退自救。
51. **BEVY-GAP-051 可视化配置快照差异比对器 (Visual Diff Modal)**：提供并排与行内双模式的差异对比弹窗，基于 Myers Diff 算法清晰高亮历史版本与当前版本的改动行（绿色新增、红色删除、黄色修改）。模块：`crates/infiltrator-bevy-ui/src/view_root/snapshot_diff_modal.rs`。验收：更新订阅前后可点击“查看差异”，一眼看出节点增减与倍率变动。
52. **BEVY-GAP-052 多订阅配置聚合器 (Multi-Profile Aggregator)**：支持用户勾选多个不同的订阅配置，设置节点去重策略，自动合并为一个统一的“聚合配置”，并在后台维护源订阅的定期联动更新。模块：`crates/infiltrator-bevy-ui/src/view_root/aggregator_modal.rs`。验收：解决持有多个不同机场订阅时的统一管理痛点，自动形成统一节点池。
53. **BEVY-GAP-053 自动拓扑与国家地区策略组生成器**：在聚合或导入配置时，内置智能语义分析器，根据节点名称关键词（如 HK/香港/Tokyo/US/专线）自动生成香港、日本、美国、新加坡等标准策略组。模块：`crates/infiltrator-core/src/profile_converter.rs`。验收：导入杂乱节点订阅时，一键重构为规整标准的分流拓扑架构。
54. **BEVY-GAP-054 Mixin 全局规则混入引擎与配置守卫**：允许定义全局 Mixin 规则（包括自定义 DNS、TUN 虚拟网卡设置、前置/后置自定义规则）；订阅更新时以 AST 保真合并算法自动混入，保证自定规则永不丢失。模块：`crates/infiltrator-core/src/mixin.rs`。验收：机场订阅更新后，用户自定义的广告拦截规则、公司内网直连规则稳固保留。
55. **BEVY-GAP-055 QuickJS 社区脚本扩展沙箱控制台**：内嵌轻量 QuickJS 脚本引擎，开放 `function main(config) { return config; }` 扩展机制；支持在客户端加载配置时以 JavaScript 进行全动态 AST 变换与日志捕获。模块：`crates/infiltrator-core/src/script_engine.rs`。验收：提供脚本调试编辑与测试运行控制台，实时查看 `console.log` 与 YAML 变换预览。
56. **BEVY-GAP-056 开箱即用脚本扩展模板库**：内置社区高频扩展模板：流媒体自动分流解锁、节点名称去广告化清理、节点国家国旗 Emoji 自动追加、特殊端口聚合等，支持一键勾选启用。模块：`crates/infiltrator-core/src/script_engine_shims.rs`。验收：新手用户无需编写 JavaScript，勾选预设即可享受配置后处理能力。
57. **BEVY-GAP-057 脚本执行超时熔断与内存安全保护**：为 QuickJS 沙箱施加硬性资源门禁：单次执行时间限制 <500ms，内存分配上限 64MB；一旦超时或超内存立即熔断终止，并向用户提示安全警报。模块：`crates/infiltrator-core/src/script_engine.rs`。验收：恶意死循环脚本或过度递归脚本无法阻塞 UI 线程或造成客户端崩溃。
58. **BEVY-GAP-058 订阅流量不足与有效期到期系统预警**：定期分析当前生效订阅的 `upload`、`download`、`total` 与 `expire` 字段；当剩余流量不足 10% 或距到期时间不足 3 天时，通过系统通知与界面红点主动提醒用户。模块：`crates/infiltrator-desktop/src/updater.rs`。验收：模拟过期订阅触发清晰的用户通知，杜绝突发性断网。
59. **BEVY-GAP-059 批量节点名称正则重命名与过滤**：提供图形化正则替换面板，允许定义规则批量剔除机场推广后缀（如 `| 官网: xyz.com`），或对特定倍率节点进行自动打标与排序。模块：`crates/infiltrator-core/src/filter.rs`。验收：保存后节点选择界面清爽干净，复杂命名的节点转为标准化直观名称。
60. **BEVY-GAP-060 多格式配置通用转换器集成**：内置协议转换流水线，支持将 Base64 订阅源、V2RayN 格式、Sing-box JSON 格式配置输入后自动转换为 Mihomo 规范的高保真 YAML 结构。模块：`crates/infiltrator-core/src/profile_converter.rs`。验收：在本地完全闭环完成敏感订阅的解析与转换，杜绝订阅泄漏。

### 大类五：分流规则、规则集管理与实时追踪体系 (Rules & Routing)
61. **BEVY-GAP-061 交互式实时分流追踪器 (Live Rule Tracer 沙盒)**：在规则页面头部集成模拟探针测试面板；用户输入待测试的域名、目标 IP、目标端口及发起进程名，即时触发本地路由决策仿真计算。模块：`crates/infiltrator-bevy-ui/src/pages/rules.rs`。验收：无需发起真实网络连接，直观查看该请求会被系统如何分流。
62. **BEVY-GAP-062 分流决策链树状回放视图**：图形化树状呈现仿真结果：测试请求依次经过的规则序号、命中的规则表达式（如 `GEOSITE, google`）、决策的目标策略组（如 `国外媒体`）及最终出站节点（如 `美国 01`）。模块：`crates/infiltrator-bevy-ui/src/pages/rules.rs`。验收：帮助用户快速排查复杂分流覆盖顺序与规则冲突根源。
63. **BEVY-GAP-063 海量规则高性能虚拟视口滚动引擎**：采用严谨的 $O(1)$ 几何视口裁剪算法与顶部/底部空白填充占位（Top/Bottom Spacers），内存中仅实例化屏幕可见的 30~50 个规则实体。模块：`crates/infiltrator-bevy-widgets/src/list/mod.rs`。验收：面对 50,000+ 条海量规则列表，滚动条快速拖拽平稳维持 60 FPS，内存恒定。
64. **BEVY-GAP-064 多条件复合规则检索与实时过滤**：提供过滤工具栏，支持按规则类型（DOMAIN、DOMAIN-SUFFIX、IP-CIDR、GEOIP、GEOSITE、PROCESS-NAME、MATCH）标签快速过滤，并结合关键字实时搜索。模块：`crates/infiltrator-bevy-ui/src/pages/rules.rs`。验收：输入目标域名或 IP 秒级过滤出相关的所有命中规则。
65. **BEVY-GAP-065 规则命中实时计数器与热度分析**：从内核实时拉取每条规则的累计匹配命中计数（Hit Count）与最后一次被命中的相对时间，并在列表右侧显示紧凑的命中徽标与热度进度条。模块：`crates/infiltrator-bevy-ui/src/pages/rules.rs`。验收：一目了然看出哪些规则是高频热规则，哪些规则长期未命中。
66. **BEVY-GAP-066 外部规则集 (Rule Providers) 全景管理面板**：设立专门的 Rule Providers 管理标签页，集中展示当前引用的所有外部规则集，罗列规则条目数、来源 URL、行为类型与缓存大小。模块：`crates/infiltrator-bevy-ui/src/pages/rules.rs`。验收：提供一键检查更新与单规则集手动拉取功能，查看规则集内部条目。
67. **BEVY-GAP-067 MRS 二进制规则集解析与加速状态**：支持 Mihomo 特有的高性能二进制 MRS 规则集格式；在界面上展示 MRS 的编译状态、解析耗时与二进制缓存命中率。模块：`crates/infiltrator-core/src/mrs.rs`。验收：确认客户端优先使用二进制加速规则集，降低冷启动与匹配 CPU 开销。
68. **BEVY-GAP-068 运行期临时规则插入与优先级调整**：允许用户临时插入一条置顶规则（如调试时将某域名强制临时指定为 DIRECT 或 REJECT），无需修改底层 YAML 文件；支持开启或关闭该临时规则。模块：`crates/infiltrator-bevy-ui/src/pages/rules.rs`。验收：临时规则即刻下发内核内存生效，提供一键清除所有临时规则按钮。
69. **BEVY-GAP-069 规则出站目标图形化快速重定向**：在规则列表项右侧，将目标策略设计为下拉菜单，允许将某规则的出站目标从 DIRECT 快速翻转为某个代理策略组。模块：`crates/infiltrator-bevy-ui/src/pages/rules.rs`。验收：点击下拉即可变更，底层自动走原子事务修改配置并重载。
70. **BEVY-GAP-070 GeoSite 与 GeoIP 数据库离线包管理与一键更新**：展示本地 `geosite.dat` 与 `geoip.dat`（或 `Country.mmdb`）的文件构建日期、体积与哈希；提供静默后台自动检查与一键无感更新。模块：`crates/infiltrator-desktop/src/updater.rs`。验收：更新后无需重启客户端，内核自动热重载新数据库。
71. **BEVY-GAP-071 自定义分流规则实时语法与逻辑校验**：在添加或编辑规则时，内置校验引擎即时对 IP-CIDR 掩码合法性、域名格式、GEOIP 两位国家码及引用的策略组是否存在进行静态分析。模块：`crates/infiltrator-core/src/proxy_nodes/validate.rs`。验收：输入非法 IP 或不存在策略组时，输入框即时标红并禁用保存。
72. **BEVY-GAP-072 规则逻辑冲突与死规则 (Dead Rules) 静态检测**：实现分流规则拓扑分析算法，自动扫描规则序列中由于前后位置不当而被完全遮蔽的“死规则”。模块：`crates/infiltrator-core/src/filter.rs`。验收：对被遮蔽的无效规则以黄色警告标签标注，并提供一键调整优先级建议。
73. **BEVY-GAP-073 图形化子规则集构造器 (Sub-Rules Builder)**：提供表单化的自定义规则包组装界面，用户可通过批量粘贴域名/IP 列表，一键生成私有规则集并命名保存。模块：`crates/infiltrator-bevy-ui/src/pages/rules.rs`。验收：无需编写复杂规则代码即可创建个性化公司内网包或去广告包。
74. **BEVY-GAP-074 进程级与应用级专属规则显式徽标**：对于 `PROCESS-NAME`、`PROCESS-PATH` 类型的规则，自动关联对应应用的专属图标与进程标签，与普通域名规则区分排布。模块：`crates/infiltrator-bevy-ui/src/pages/rules.rs`。验收：清晰区分控制整个应用的规则与控制特定网站域名的规则。
75. **BEVY-GAP-075 当前合并生效全量规则集导出**：支持将由基础订阅、外部 Rule Providers、全局 Mixin 和临时规则经计算合并后的全量生效规则清单，一键导出为纯文本文件或分享片段。模块：`crates/infiltrator-bevy-ui/src/pages/rules.rs`。验收：方便资深用户复核实际执行的规则流水线，或导出部署。

### 大类六：连接审计、多维聚合与深度链路透视 (Connections & Auditing)
76. **BEVY-GAP-076 高并发连接复合检索与即时过滤**：构建多条件检索输入框，支持同时按目标主机名（如 `github`）、目标 IP、本地端口、进程名及出站节点名称进行组合过滤。模块：`crates/infiltrator-bevy-ui/src/pages/connections.rs`。验收：数百个并发连接中毫秒级缩窄列表，排查异常连接迅速。
77. **BEVY-GAP-077 协议分类标签与活跃/历史会话切换**：提供协议分类分段器：全部、TCP 连接、UDP 会话；同时提供状态选择器，允许在“活跃中连接”与“已关闭历史连接（最近 100 条）”之间自由切换。模块：`crates/infiltrator-bevy-ui/src/pages/connections.rs`。验收：方便排查刚刚意外中断的短连接会话与报错原因。
78. **BEVY-GAP-078 多维连接聚合视图 (Flat / By Process / By Host)**：支持三种视图形态切换：纯平级瀑布流 (Flat)、按进程名称聚合折叠 (By Process)、按目标域名聚合折叠 (By Host)。模块：`crates/infiltrator-bevy-ui/src/pages/connections.rs`。验收：清晰看到每个进程或每个网站占用的连接数与累计带宽消耗。
79. **BEVY-GAP-079 深度链路透视侧滑抽屉 (Connection Drawer)**：点击任意一条连接，从窗口右侧平滑滑出“连接详情抽屉”；全面展现源 IP:端口、目标 IP:端口、目标主机、入站类型、匹配规则详情、出站策略链条。模块：`crates/infiltrator-bevy-ui/src/view_root/connection_drawer.rs`。验收：无需 Wireshark 抓包即可洞悉单连接全生命周期。
80. **BEVY-GAP-080 阶段耗时瀑布流图表 (Waterfall Timeline)**：在连接详情抽屉内绘制阶段耗时横向柱状图，展现 DNS 解析用时 (ms)、TCP 握手用时 (ms)、TLS 协商握手用时 (ms) 及 TTFB 首包到达耗时。模块：`crates/infiltrator-bevy-ui/src/view_root/connection_drawer.rs`。验收：准确暴露是 DNS 慢、节点抖动还是目标响应慢。
81. **BEVY-GAP-081 目标 ASN 编号与地理位置信息透视**：根据目标 IP 解析其 BGP AS 自治域编号（如 AS15169 Google LLC）、物理归属国旗帜、城市及运营商信息。模块：`crates/infiltrator-bevy-ui/src/pages/connections.rs`。验收：连接列表中直观展示目标地理标识，识别未知连接更直观。
82. **BEVY-GAP-082 单连接动态速率与累计传输吞吐监控**：动态跟踪单条连接当前的实时上行/下行传输速率，以及自建立连接以来累计消耗的上传和下载字节总量。模块：`crates/infiltrator-bevy-ui/src/pages/connections.rs`。验收：大文件传输连接显式高亮显示，便于定位流量偷跑源头。
83. **BEVY-GAP-083 一键快速反向生成分流规则**：在连接抽屉底部提供“一键加规则”按钮，点击弹出微表单，自动填入当前连接的域名或 IP，允许一键设为 DIRECT、REJECT 或指定策略组。模块：`crates/infiltrator-bevy-ui/src/view_root/connection_drawer.rs`。验收：免除手动复制黏贴步骤，直连或阻断需求秒级处置。
84. **BEVY-GAP-084 单连接精准切断与免断核心保护**：每条连接右侧提供红色“断开”按钮；同时支持将特定的系统核心连接（如 systemd-resolved 或 SSH 会话）加入保护名单，避免误切断。模块：`crates/infiltrator-bevy-ui/src/pages/connections.rs`。验收：点击断开即刻向内核发送 `DELETE /connections/{id}` 终结连接。
85. **BEVY-GAP-085 全部断开 (Close All) 与防误触确认交互**：在页面右上角设置“断开全部连接”操作；提供 3 秒长按确认或快速气泡确认机制，防止关键任务中误触。模块：`crates/infiltrator-bevy-ui/src/pages/connections.rs`。验收：确认后清理全部活跃连接，迫使所有应用走新策略组重连。
86. **BEVY-GAP-086 实时数据流智能暂停与冻结排障**：提供“暂停刷新 / 继续刷新”按钮；在高并发请求涌入时，允许随时暂停刷新，将当前连接列表冻结，以便仔细审查和操作。模块：`crates/infiltrator-bevy-ui/src/pages/connections.rs`。验收：冻结状态下列表不再跳动错位，恢复后瞬间补齐差分数据。
87. **BEVY-GAP-087 空闲僵尸连接自动识别与清理守护**：内置空闲连接审计算法，对闲置超过设定阈值（如 10 分钟）且速率持续为 0 的僵尸连接进行黄色标识，并提供一键批量清理。模块：`crates/infiltrator-core/src/idle_connection_sweeper.rs`。验收：有效释放系统文件描述符 (FD) 和套接字内存占用。
88. **BEVY-GAP-088 诊断级 PCAP 数据包导出通道**：允许用户右键特定连接或特定进程开启短暂环形抓包；抓包结束后生成标准 `.pcap` 文件并调用系统目录打开，供 Wireshark 分析。模块：`crates/infiltrator-iced/src/view/pcap_panel.rs`。验收：为复杂网络排障和安全审计提供专业级工具链支撑。
89. **BEVY-GAP-089 系统进程真实图标与精准 PID 提取**：根据连接上报的 PID，调用操作系统枚举接口，提取该进程的可执行文件路径、窗口标题并抓取高清图标进行缓存呈现。模块：`crates/infiltrator-desktop/src/process_enumerator/`。验收：连接列表中直观呈现 Chrome、Steam、Spotify 等熟悉的应用图标。
90. **BEVY-GAP-090 高风险异常网络行为视觉红牌告警**：内置启发式风险检测模型，对短时间内产生数千次重试、大量未解密可疑端口通信或持续解析失败的连接施加显眼的红色警示标签。模块：`crates/infiltrator-bevy-ui/src/pages/connections.rs`。验收：辅助用户快速识别后台流氓软件或异常泄密进程。

### 大类七：运行日志、DNS 探针与系统自愈健康 (Logs & Diagnostics)
91. **BEVY-GAP-091 高性能流式日志环形缓冲区**：采用无锁固定容量的循环内存环（容量可配置如 5000 行）；采用微批次渲染更新机制，面对每秒数百条高频日志推送依然稳定流畅。模块：`crates/infiltrator-bevy-ui/src/pages/logs.rs`。验收：日志爆发时 UI 不卡死、不掉帧，内存恒定不膨胀。
92. **BEVY-GAP-092 多色彩日志级别视觉标识体系**：建立清晰的色彩规范：DEBUG 为淡灰色，INFO 为科技蓝，WARN 为警示黄，ERROR 为醒目红；级别标签渲染为圆角微胶囊形状。模块：`crates/infiltrator-bevy-ui/src/pages/logs.rs`。验收：在日志瀑布流中一眼抓住异常错误行，提升排错效率。
93. **BEVY-GAP-093 实时文本模糊搜索与正则表达式过滤**：顶部提供过滤搜索条，不仅支持关键词模糊查找，还支持完整的 Regex 正则表达式语法（如 `(timeout|refused|dial)`）动态筛选。模块：`crates/infiltrator-bevy-ui/src/pages/logs.rs`。验收：输入正则表达式精准筛选目标日志，语法错误时给出友好微提示。
94. **BEVY-GAP-094 结构化模块标签胶囊徽标解析**：自动提取日志中的模块前缀（如 `[TCP]`、`[DNS]`、`[TUN]`、`[ROUTING]`），自动渲染为语义化的微型彩色徽标，正文采用等宽字体紧凑对齐。模块：`crates/infiltrator-bevy-ui/src/pages/logs.rs`。验收：日志排版如 IDE 终端般专业规整，阅读体验舒适。
95. **BEVY-GAP-095 智能滚屏锁定与回滚跟随机制**：鼠标向上滚动或拖动滚动条翻阅历史日志时，自动挂起“自动滚屏”；滚动条重新拉至最底部时，自动恢复最新日志实时跟随滚动。模块：`crates/infiltrator-bevy-ui/src/pages/logs.rs`。验收：彻底消除查看报错时被新日志不断强制顶到底部的糟糕体验。
96. **BEVY-GAP-096 日志缓冲区一键清空与本地打包导出**：提供“清空日志”快捷按钮；同时提供“导出日志”按钮，点击后将当前缓冲区所有日志自动保存为带时间戳命名的 `.log` 文本文件并弹出通知。模块：`crates/infiltrator-bevy-ui/src/pages/logs.rs`。验收：方便用户在遇到故障时一键导出完整日志并打包反馈。
97. **BEVY-GAP-097 DNS 泄漏多源交叉探测与隐私评估**：并发向多个权威公网探测源（Cloudflare, Quad9, ipify）派发伪随机子域解析请求，检测出口 DNS 服务器归属地，判断是否存在泄漏。模块：`crates/infiltrator-core/src/dns_tester.rs`。验收：给出明确健康安全评级（安全/存在泄漏/严重污染）与修复建议。
98. **BEVY-GAP-098 Fake-IP 内部映射表可视化检索与导出**：提供专属面板查询当前内核中 `198.18.x.x` 虚拟 IP 与真实域名之间的双向映射清单，支持按域名搜索，展示创建时间与 TTL。模块：`crates/infiltrator-core/src/fake_ip.rs`。验收：直观揭示内部映射，配合“一键清空 Fake-IP 缓存”重置网络。
99. **BEVY-GAP-099 上游加密 DNS (DoH/DoT/DoQ) 并发测速**：列出当前配置的所有上游 DNS 服务器（AliDNS, DNSPod, Google, Cloudflare），并发测量各 DNS 服务器的解析握手延迟与丢包率。模块：`crates/infiltrator-bevy-ui/src/pages/dns.rs`。验收：精准识别被运营商劣化的 DNS，辅助优化分流配置。
100. **BEVY-GAP-100 全面系统自愈诊断套件 (Doctor Suite)**：构建 8 项自检项目：TUN 虚拟网卡状态、系统路由表健康度、监听端口独占性 (7890/9090)、DNS 污染防护、系统管理员特权、配置文件语法、系统代理注册状态、外部连通性。模块：`crates/infiltrator-core/src/doctor/doctor_test.rs`。验收：以绿灯/黄灯/红灯清晰呈现，点击展开诊断详情与排错指南。
101. **BEVY-GAP-101 系统常见网络故障一键智能自愈**：在自愈诊断页提供“一键全自动修复”大按钮；当检测到端口冲突、TUN 网卡残留或系统代理异常未清除时，自动执行修复脚本尝试自动排障。模块：`crates/infiltrator-bevy-ui/src/pages/doctor.rs`。验收：帮助小白用户在断网或无法启动代理时一键恢复健康网络。
102. **BEVY-GAP-102 内核崩溃看门狗守护进程 (Crash Watchdog)**：UI 维持对后台 Mihomo 进程的心跳监视；若内核发生 OOM 或意外崩溃退出，看门狗记录崩溃前的瞬时堆栈，并尝试最多 3 次安全静默拉起。模块：`crates/infiltrator-iced/src/view/crash_watchdog_card.rs`。验收：大幅提升极端恶劣网络或大并发下载下的软件整体稳定性。
103. **BEVY-GAP-103 系统物理网络漫游感知器 (Network Roaming)**：监听操作系统的网络网卡变动事件（Wi-Fi 断开、切换热点、插上网线、从睡眠中唤醒）；一旦感知到漫游，自动触发 DNS 缓存重置与节点健康复测。模块：`crates/infiltrator-iced/src/view/net_roam_card.rs`。验收：解决笔记本合盖唤醒或离开 Wi-Fi 后代理卡死的痛点。
104. **BEVY-GAP-104 端口冲突外部进程侦测排查器**：当 7890 混合端口或 9090 控制端口启动报错时，自动通过套接字表反查占用该端口的外部应用程序名称与 PID（如被迅雷或老旧 Clash 占用）。模块：`crates/infiltrator-bevy-ui/src/pages/doctor.rs`。验收：界面精准提示：“端口 7890 正被进程 X (PID: 1234) 占用，请关闭或更换端口”。
105. **BEVY-GAP-105 内核日志级别无重启动态调节**：在日志页面顶部提供日志级别选择器（SILENT, ERROR, WARN, INFO, DEBUG）；切换时通过 REST API 实时修改运行中内核的输出级别，无需重启核心。模块：`crates/infiltrator-bevy-ui/src/pages/logs.rs`。验收：日常使用保持 INFO 节约性能，排错时秒切 DEBUG 捕获报文。

### 大类八：应用级分流、进程控制与桌面移动平台特化 (App Routing & Platform)
106. **BEVY-GAP-106 动态系统网络进程实时枚举器**：调用 Linux procfs、Windows Toolhelp32 与 macOS libproc，动态扫描当前产生外部网络连接的活跃应用程序列表，按活动热度排序呈现。模块：`crates/infiltrator-desktop/src/process_enumerator/`。验收：列表中全部为真实运行的软件，无需手动寻找路径。
107. **BEVY-GAP-107 应用程序高清图标动态提取与多层缓存**：从 Linux `.desktop` 资源、Windows EXE 资源表与 Android APK 资源中高保真提取 48x48 图标，光栅化后在内存中建立 LRU 缓存并高效渲染。模块：`crates/infiltrator-desktop/src/process_enumerator/desktop_entries.rs`。验收：列表中每个应用前均有官方高清图标，视觉质感统一。
108. **BEVY-GAP-108 应用类型智能预分类体系**：内置常见软件指纹库，将枚举出的进程自动分类筛选：浏览器（Chrome, Firefox）、开发工具（Git, Docker）、游戏平台（Steam, Epic）、影音媒体（Spotify, Discord）与系统服务。模块：`crates/infiltrator-bevy-ui/src/pages/app_routing.rs`。验收：可通过顶部标签一键查看指定类别应用，快速批量设置。
109. **BEVY-GAP-109 全局分流、白名单分流与黑名单分流三模切换**：支持三种模式：全部应用走代理、白名单模式（仅勾选的软件走代理，其余直连）、黑名单模式（仅勾选的国内软件直连，其余走代理）。模块：`crates/infiltrator-bevy-ui/src/pages/app_routing.rs`。验收：简化分流策略配置，满足玩游戏时仅为 Discord 或游戏本身开启加速的需求。
110. **BEVY-GAP-110 单应用分流策略三态快速切换交互**：在应用列表右侧设计三态选择胶囊：点击即可在“代理 (Proxy)”、“直连 (Direct)”与“阻断 (Reject)”三态之间即时切换并即刻生效。模块：`crates/infiltrator-bevy-ui/src/pages/app_routing.rs`。验收：操作简单直接，单项设置立即更新内核分流行为。
111. **BEVY-GAP-111 应用名称与二进制执行路径组合搜索**：提供过滤输入框，支持同时按应用程序可见名称（如“网易云音乐”）、可执行文件名称（如 `cloudmusic.exe`）或二进制绝对路径进行过滤。模块：`crates/infiltrator-bevy-ui/src/pages/app_routing.rs`。验收：冷僻后台服务输入关键词后几毫秒内准确定位。
112. **BEVY-GAP-112 无关系统底层后台守护进程智能隐藏**：默认自动过滤掉系统底层无网络需求的进程（如 `kworker`, `systemd`），提供“显示系统进程”独立开关满足高级开发者需求。模块：`crates/infiltrator-desktop/src/process_enumerator/process_filter.rs`。验收：默认列表整洁干净，只有日常软件，大幅降低视觉噪音。
113. **BEVY-GAP-113 Windows UWP 应用网络回环限制一键解除**：针对 Windows 平台特有的 UWP 沙盒网络隔离机制提供检测面板，扫描已安装 UWP 应用并提供“一键全部解除回环代理限制”按钮。模块：`crates/infiltrator-iced/src/view/uwp_card.rs`。验收：彻底解决 Windows 商店、Xbox 游戏与 Office UWP 应用无法走代理的痛点。
114. **BEVY-GAP-114 Android VpnService 分应用代理契约完全对齐**：在 Android 平台自动读取 `PackageManager` 已安装的 APK 列表，并将用户的应用分流选择转换为 Android 原生 `VpnService.Builder.addAllowedApplication` 规则。模块：`android/`。验收：在手机端实现微信、支付宝等国内应用彻底绕过 VPN，免除银行 App 告警。
115. **BEVY-GAP-115 单进程级网络限速与 QoS 优先级控制**：支持对指定大流量进程（如网盘、BT 客户端）单独设置最大上传/下载带宽上限，并配置连接调度优先级。模块：`crates/infiltrator-core/src/flow_control.rs`。验收：防止挂机下载霸占整机带宽导致网页打不开或游戏丢包。
116. **BEVY-GAP-116 一键将指定应用沉淀为持久化分流规则**：勾选应用后，点击底部“加入规则库”，自动在当前分流配置中写入对应的 `PROCESS-NAME` 规则行并完成持久化存储。模块：`crates/infiltrator-bevy-ui/src/pages/app_routing.rs`。验收：免除手写规则语法，让临时应用分流策略沉淀为长期规则资产。
117. **BEVY-GAP-117 未运行离线应用程序手动浏览添加**：提供“添加应用”按钮，调用系统原生文件选择对话框，允许手动选择尚未启动运行的 `.exe`、`.AppImage` 或二进制程序并预先配置其分流策略。模块：`crates/infiltrator-bevy-ui/src/pages/app_routing.rs`。验收：方便在首次启动国外大型网游前预先完成加速节点绑定。
118. **BEVY-GAP-118 游戏加速低延迟直通路由策略**：针对游戏类别应用开启专用直通通道：跳过冗长正则匹配，强制启用原生 UDP FullCone 转发，优先挑选测速延迟最低的专线节点。模块：`crates/infiltrator-bevy-ui/src/pages/app_routing.rs`。验收：为电竞玩家提供极低抖动、极低丢包的网络游戏加速体验。
119. **BEVY-GAP-119 应用程序网络流量消耗动态排行榜**：后台持续统计各应用程序在本次会话中产生的上下行流量总和，在页面右侧绘制柱状排行榜，列出最耗费流量的前 5 名软件。模块：`crates/infiltrator-bevy-ui/src/pages/app_routing.rs`。验收：直观揭示哪个后台应用在偷偷下载，辅助排查流量偷跑。
120. **BEVY-GAP-120 进程自定义中文备注与友好别名管理**：允许双击编辑复杂的后台文件名（如 `cef_sub_process.exe`），赋予自定义中文别名（如“网易云内嵌网页组件”）。模块：`crates/infiltrator-bevy-ui/src/pages/app_routing.rs`。验收：别名持久化保存并在连接审计中统一展示，管理更亲切。

### 大类九：云端同步、系统全局设置与高级网络栈 (Sync, Settings & System Integration)
121. **BEVY-GAP-121 多云端后端备份与漫游支持**：扩展云同步后端协议，除标准 WebDAV 外，集成 GitHub Gist、iCloud Drive 与自建 S3 兼容对象存储通道。模块：`crates/infiltrator-bevy-ui/src/pages/sync.rs`。验收：多设备用户自由选择云端媒介，一键跨设备同步配置与节点偏好。
122. **BEVY-GAP-122 云端快照三方差异智能合并 (3-Way Merge)**：多端同时改动产生同步冲突时，提供三向合并对比视图，呈现本地修改、云端修改与共同祖先版本，允许逐项勾选保留。模块：`crates/infiltrator-bevy-ui/src/pages/sync.rs`。验收：杜绝粗暴覆盖导致本地最新修改遗失的悲剧。
123. **BEVY-GAP-123 同步凭据系统级安全加密托管与连通测试**：云端账户密码与 Token 采用操作系统级钥匙串 (Keyring / Secret Service) 加密保存，绝不明文落盘；提供“测试连接”按钮即时检测连通性。模块：`crates/infiltrator-desktop/src/proxy.rs`。验收：保障云端凭据安全无泄漏，测试反馈明确迅速。
124. **BEVY-GAP-124 配置变动延迟静默自动漫游同步**：支持“变动时自动同步”选项，检测到本地添加节点或改动规则后启动 30 秒防抖计时器，后台静默完成增量上传漫游。模块：`crates/infiltrator-bevy-ui/src/pages/sync.rs`。验收：免除手动点击同步记忆负担，实现多端无感配置漫游。
125. **BEVY-GAP-125 全平台系统代理自动化深度适配与容灾**：深度适配全平台系统代理：Linux GNOME (gsettings)、KDE Plasma (kwriteconfig5)、环境变量；Windows 注册表；macOS networksetup。模块：`crates/infiltrator-desktop/src/proxy/linux/`。验收：不同桌面环境下开启系统代理精准生效，失败给出类型化原因。
126. **BEVY-GAP-126 异常断电与关机系统代理自动清理复位**：启动引导时检查系统代理注册表与环境变量：若发现残留了上次掉电或崩溃导致的代理指向，主动静默重置恢复。模块：`crates/infiltrator-desktop/src/boot.rs`。验收：彻底消除开机后“打不开网页，必须手动关代理”的网络顽疾。
127. **BEVY-GAP-127 TUN 虚拟网卡多协议栈灵活切换与 MTU 调优**：系统设置中提供 TUN 协议栈选项：gVisor（用户态稳定）、System（内核原生高性能栈）与 Mixed 混合栈；开放 MTU 调整（默认 1500）与网卡名自定义。模块：`crates/infiltrator-iced/src/view/tun_stack_card.rs`。验收：满足追求极致性能或特殊游戏网络兼容性的高级需求。
128. **BEVY-GAP-128 虚拟网卡严格路由 (Strict Route) 与 DNS 劫持防护**：提供 Strict Route 独立开关，开启后彻底阻断绕过 TUN 网卡的流量泄漏；提供 DNS 强制劫持选项接管 53 端口流量。模块：`crates/infiltrator-bevy-ui/src/pages/settings.rs`。验收：为高敏感网络工作者提供坚固的反流量泄漏防御体系。
129. **BEVY-GAP-129 局域网代理共享与一键扫码配置生成**：开启“允许来自局域网的连接”后，自动扫描本机局域网 IP，动态生成说明文本与二维码；其他手机、Switch 扫码即可共享上网。模块：`crates/infiltrator-iced/src/view/lan_sharing_card.rs`。验收：让电脑成为家庭或宿舍的透明网络中继站。
130. **BEVY-GAP-130 外部控制器 RESTful API 安全鉴权与端口设置**：允许自定义外部控制器端口（默认 9090）与绑定 IP，设置高强度 API 访问密钥 (Secret)；支持一键复制标准连接串。模块：`crates/infiltrator-bevy-ui/src/pages/settings.rs`。验收：防止恶意网页通过本地回环端口劫持 Mihomo 内核控制权。
131. **BEVY-GAP-131 桌面系统托盘生态 (System Tray) 深度集成**：构建系统托盘图标，随状态变色（未连接灰/代理蓝/TUN绿/故障红）；右键呼出原生层级菜单：模式快切、选节点、测速、打开面板与退出。模块：`crates/infiltrator-desktop/src/tray_badge.rs`。验收：关闭窗口自动最小化至托盘后台运行，双击托盘瞬间唤醒主界面。
132. **BEVY-GAP-132 系统全局热键管理与冲突规避系统**：设置面板提供可视化热键录制界面：支持为切换系统代理、切换 TUN、唤出/隐藏主窗口自定义全局热键，并检测系统冲突。模块：`crates/infiltrator-bevy-ui/src/shortcuts.rs`。验收：全屏办公或打游戏时按热键瞬间完成代理启闭。
133. **BEVY-GAP-133 跨平台开机自启与静默后台启动服务**：跨平台注册自启动服务：Linux systemd/autostart、Windows 注册表/计划任务、macOS LaunchAgent plist；支持“开机时自动最小化到托盘”。模块：`crates/infiltrator-desktop/src/proxy.rs`。验收：开机自动随系统在后台静默拉起，用户开机即用，零弹窗打扰。
134. **BEVY-GAP-134 全界面 100% 中英多语言国际化动态热重载**：全界面文本 100% 接入词条表，静态漏译守卫阻断裸字面量；设置中切换语言时，无需重启应用，全界面文字实时热重载。模块：`crates/infiltrator-shared/src/locales_table.rs`。验收：符合国际化开源社区标准，海外用户体验无障碍。
135. **BEVY-GAP-135 高级主题色谱、纯黑 OLED 模式与排版密度定制**：扩展针对移动端 OLED 屏幕的纯黑 (True Black) 模式以极致省电；提供排版密度调节（舒适 Comfortable / 紧凑 Compact）。模块：`crates/infiltrator-bevy-widgets/src/theme.rs`。验收：紧凑模式下一屏展示更多行数据，主题色自由切换。

### 大类十：引擎调步、低功耗渲染与底层组件架构 (Engine, Performance & Architecture)
136. **BEVY-GAP-136 实时双向内核通信契约通道落地 (BEVY-005 Seam)**：基于 Tokio 异步 Runtime 建立常驻管道，订阅 Mihomo WebSocket 流（`/traffic`、`/logs`、`/connections`）；转换为 typed ECS 事件投递主线程，每帧有界排水消费。模块：`crates/infiltrator-bevy-ui/src/controller.rs`。验收：彻底替代 DemoProjection，11 个页面接入真实内核数据，指令毫秒级往返。
137. **BEVY-GAP-137 多模态引擎渲染调步机制 (Cadence & Low Power Policy)**：实现动态调步状态机：活跃交互时 60 FPS，静置 30 秒后降至 15 FPS；窗口失去焦点或最小化到托盘后，进入完全事件等待或 2 FPS 微息模式。模块：`crates/infiltrator-bevy-widgets/src/cadence.rs`。验收：桌面后台运行 CPU 占用 <0.3%，移动端后台运行彻底杜绝发热耗电。
138. **BEVY-GAP-138 完全响应式事件驱动渲染管线 (Reactive Rendering Pipeline)**：接入 Bevy 0.19 的 `bevy_winit::UpdateMode::ReactiveLowPower`；仅当窗口事件到达、键鼠交互或 WebSocket 推送新数据时，才唤醒 ECS 调度重绘一帧。模块：`crates/infiltrator-bevy-widgets/src/reactive.rs`。验收：网络闲置状态下 GPU 负荷归零，笔记本电池续航不受代理软件影响。
139. **BEVY-GAP-139 中文与东亚多语言输入法 (IME) 深度适配**：完善自研纯核文本输入状态机，对接 `Ime::Preedit` 与 `Ime::Commit` 窗口事件；精准计算光标物理位置，确保输入法候选词浮窗紧随输入光标。模块：`crates/infiltrator-bevy-widgets/src/text_input/ime.rs`。验收：在搜索框与编辑器中流畅输入中文、日文、韩文，候选词不漂移。
140. **BEVY-GAP-140 跨平台原生剪贴板安全与异步交互**：封装原生剪贴板驱动，支持 Wayland `wl-clipboard`、X11、Windows Win32 API 及 Android JNI 通道；集成敏感凭据脱敏保护。模块：`crates/infiltrator-bevy-widgets/src/clipboard_sanitizer.rs`。验收：一键复制节点链接、粘贴订阅 URL 稳定可靠，无死锁或闪退。
141. **BEVY-GAP-141 通用高性能虚拟视口滚动容器组件 (VirtualList Widget)**：将虚拟视口几何裁剪算法抽象为通用控件包，提供 `VirtualListBuilder<T>` 模板；自动根据容器尺寸与项高度计算可视区间，全页面复用。模块：`crates/infiltrator-bevy-widgets/src/list/scroll_core.rs`。验收：规则列表、连接列表、日志流与节点网格统一接入，零冗余代码。
142. **BEVY-GAP-142 矢量派生高清 RGBA 位图渲染与着色器动态染色**：全仓图标统一由矢量 SVG 源文件生成 64px/128px 多分辨率 RGBA 位图；运行时通过 `ImageNode` 结合自定义 Tint 着色器进行色彩注入，告别字形码位乱码与锯齿。模块：`crates/infiltrator-bevy-widgets/src/icon.rs`。验收：在 4K 屏与移动端图标锐利细腻，完美支持主题色实时原地变色。
143. **BEVY-GAP-143 纯函数核与 BSN 场景适配器严格二分契约**：所有业务控件严格贯彻“纯 Rust 数据核（零 Bevy 依赖、无外部副作用、100% 支持无头测试）+ 消费纯核输出的 `*_scene` 适配器（负责 BSN 树组装）”架构。模块：`crates/infiltrator-bevy-widgets/src/`。验收：所有复杂交互边界与状态机均在无窗口和无 GPU 环境下完成自动化测试。
144. **BEVY-GAP-144 ECS 实体生命周期追踪与观察者内存泄漏清剿**：建立页面挂载销毁生命周期守卫，在有界子树执行 `despawn_children` 时，通过层级递归遍历，确保关联的定时器、瞬态资源与未决 Observer 观察者彻底反注册。模块：`crates/infiltrator-bevy-ui/src/lifecycle.rs`。验收：10,000 次高频页面切换压力测试下 World 实体总数与堆内存保持稳定，零泄漏。
145. **BEVY-GAP-145 流体网格与断点动态约束排版引擎 (Fluid Grid Layout)**：构建基于百分比与弹性比例分发的响应式容器组件；依据当前父容器实时宽度自动计算子卡片最佳列数、对齐间距与拉伸权重，消除右侧大块空白。模块：`crates/infiltrator-bevy-widgets/src/fluid_grid.rs`。验收：窗口拉长、压缩或居中分屏时，卡片均能以最优视觉网格紧凑呈现。
146. **BEVY-GAP-146 全链路 AccessKit 屏幕阅读器语义树补全**：为所有自定义按钮、开关、滑块、列表项、输入框与模态窗口注入精准的 `AccessKit::Role`、`Label`、`Value` 与 `Action` 描述，与操作系统无障碍辅助技术握手。模块：`crates/infiltrator-bevy-widgets/src/a11y.rs`。验收：视障用户在开启 Windows Narrator、Linux Orca 或 Android TalkBack 时无障碍听读操作。
147. **BEVY-GAP-147 桌面窗口亚克力 (Acrylic) 与毛玻璃 (Mica) 特效**：接入平台原生窗口装饰 API，在 Windows 11 开启 Mica / DWM 亚克力材质，在 macOS 开启 NSVisualEffectView 原生磨砂玻璃背景，与现代操作系统设计语言共鸣。模块：`crates/infiltrator-bevy-widgets/src/windowing.rs`。验收：窗口背景通透优雅，与桌面壁纸产生高级光影层次。
148. **BEVY-GAP-148 Android 触觉震动反馈与边缘误触抑制**：通过 JNI 封装 Android Vibrator 触觉反馈接口，在移动端执行节点选择、模式切换、开关滑动与长按时提供微妙触觉震动；并在屏幕四周保留 12px 防误触安全边缘。模块：`crates/infiltrator-bevy-widgets/src/haptics.rs`。验收：移动端操作手感厚实精准，防止手指边缘滑动时意外触发切换。
149. **BEVY-GAP-149 跨平台二进制打包极限瘦身与管线预热**：应用 Cargo profile release strip、LTO 优化、冷门着色器分支裁剪与内置字形按需精简；首次启动时后台异步预热 GPU 渲染管线，消除首次点击控件时的着色器编译微掉帧。模块：`scripts/build-bevy-apk.sh`。验收：二进制体积缩小 40%，Android APK 极简轻量，首屏冷启动 <300ms。
150. **BEVY-GAP-150 CI 自动化无头回归与像素级视觉比对门禁**：在 GitHub Actions CI 中持续集成无头 niri 截图矩阵与像素级 Diff 比对，覆盖全 11 个页面在明亮、暗黑模式及多端分辨率下的渲染结果，任何视觉降级直接拦截构建。模块：`scripts/capture-bevy-matrix.sh`、`.github/workflows/test.yml`。验收：后续迭代中 UI 视觉标准与设计规范永不发生意外回退。
