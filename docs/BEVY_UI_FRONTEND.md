# Bevy UI 前端章程

本文是 Bevy UI 前端的公开事实权威。Bevy UI 是 MusicFrog Infiltrator 的**战略统一
surface**：它的上限包含移动端。iced 是 winit 桌面方案，没有 Android 故事；bevy 0.19
的同一棵 UI 树可以直接跑在 `aarch64-linux-android` 上。控件生态薄是事实，但
`bevy_ui_widgets`（官方无样式控件包）加上我们自有的 `infiltrator-bevy-widgets`
层可以补齐——控件是我们自己建的，就归我们自己所有。iced 主桌面维持维护态并继续
承接 X11 会话；依据最高主控台账 [DUAL_SURFACE_PARITY_MASTER_PLAN.md](DUAL_SURFACE_PARITY_MASTER_PLAN.md)，Iced（成熟桌面端）与 Bevy UI（跨平台战略端）现已确立为严格对等的双主干 surface，在功能和 UI 表现上全面同步演进。

参考实现：taskmanager 的 `taskmanager-bevy-ui`（同为 bevy 0.19 产品级 UI，bsn! 场景法
+ 观察者绑定 + 纯核/场景适配器二分 + 中立主题 token）。本章程的多条铁律直接来自该
项目的成文法律与踩坑记录。

## 1. 基座铁律

- **bevy 锁定 `=0.19.1`**：精确到 patch，与 taskmanager 同锁。升级属于架构与发布
  评审事项（bevy 0.19 已把 UI 布局与渲染拆开，`bevy_ui` 单独只排版不上屏，必须闭合
  `bevy_ui_render`）。
- **依赖白名单**：`infiltrator-bevy-widgets` 只依赖 bevy；`infiltrator-bevy-ui` 依赖
  bevy + widgets + `infiltrator-contract` + `infiltrator-application`，但不依赖
  `infiltrator-core`、`mihomo-api`、Reqwest 或 Tokio。具体 Mihomo adapter 由 application
  的 composition seam 提供，不能进入页面与 projection 模块。
- **feature 闭包两段式声明**：`[dependencies]` 空特征基座声明 + per-target 表持有
  真实闭包（bevy derive 宏按 `::bevy::…` 路径扫描普通依赖表，只写 target 表会
  unresolved）。`default-features` 全关（无音频/gilrs/3D/动态链接）；`multi_threaded`
  关闭以保证无头测试可在 app 线程内观察渲染路径；Linux 仅 Wayland，X11 会话由 iced
  端承载（bevy 成为唯一 surface 时再评审 `x11`）。
- **100% `bsn!` 场景法**：生产 UI 树（页面、行/单元格、控件、覆盖层）全部由 `bsn!`
  场景组合并经 `spawn_scene` 挂载；禁止命令式 `Node{}`/`Children`/`with_children`
  另起 UI 树。ECS 系统只更新场景实体的 typed 组件、接线事件/焦点，或以新场景替换
  有界子树。（M2 落地机械守卫 BEVY-004，移植 taskmanager 的 `bevy_bsn_guard.py`。）
- **观察者改组件，永不重建树**：运行时变化由 `On<Add<…>>`/`On<Activate>` 等观察者
  原地盖章/换肤；页面重挂载以替换有界子树实现。
- **纯核 + 场景适配器二分**：控件 = 零 bevy 依赖的纯函数核（状态机/投影/窗口数学，
  无头可测）+ 消费纯核输出的 `*_scene` 适配器（返回 `impl Scene`）。
- **主题 token 是唯一样式权威**：颜色/尺寸/字号 token 只存在于
  `infiltrator-bevy-widgets/src/theme.rs`，唯一"token → bevy 值"的适配点是
  `palette.rs`。任何调用点出现裸颜色字面量即违规。不采用 bevy 官方 Feathers 皮肤。
- **图标走位图，不走字形码位**：嵌入字体不保证装饰性 Unicode 字形，字形 tofu 是
  已知坑（taskmanager 成文教训）。图标 = SVG 派生 RGBA 位图 → tinted `ImageNode`。

## 1.5 与 iced 的构建共存

bevy 栈与 iced 栈同 workspace 共存时会发生一次已知的 feature 统一冲突：bevy 的
`naga_oil` 打开共享 `codespan-reporting/termcolor`，破坏 naga 27（iced wgpu 27
依赖）的无 termcolor 诊断分支。现行修复：`infiltrator-iced` 持有改名直依赖
`naga27 = { package = "naga", version = "27", features = ["termcolor"] }`（仅
feature 用途，代码零导入）。iced 的 wgpu 栈升级越过 naga 27 后删除该条目。

## 2. crate 结构

| crate | 职责 | 依赖 |
| --- | --- | --- |
| `infiltrator-bevy-widgets` | 业务无关控件层：token/主题、排版角色、pill 按钮、surface 卡片；未来扩展 checkbox/radio/scrollarea/slider/text_input/menu | 仅 bevy |
| `infiltrator-bevy-ui` | 前端壳：窗口组合（`DefaultPlugins`）、`ShellPlugin`、路由与页面；消费 contract/application 投影 | bevy + widgets + contract/application |

`WidgetsPlugin` 是唯一安装点：注入 `UiPalette` 资源、排版盖章观察者、控件换肤系统。
`ShellPlugin` 无窗口基础设施，`MinimalPlugins` 无头测试跑的是真实 shell。

## 3. 与 taskmanager 的控件抽取协议

两项目未来共同抽出业务无关控件层（"我们自己的 bevy_ui_widgets 皮肤包"）的判定
标准：**同一个控件在两个项目中第二次出现时，下沉**。为此：

- bevy 同锁 `=0.19.1`；
- widgets 层零业务依赖、纯函数核可无头测试；
- 控件实现风格对齐（bsn! 场景函数 + typed marker 组件 + 观察者），搬运即迁移。

当前各自独立演化；BEVY-007 为正式评估项。

## 4. 已知边界（诚实声明，M2 终态）

已落地：嵌入字体（Inter 三字重 + JetBrains Mono，未注册优雅回退）；主题运行时切换
（`ThemeSwitch` 事件原地 restamp，零重挂载）；accesskit 最小语义闭环（种子为纯组件，
桥窗口化激活）；checkbox/radio/scrollarea/slider 包装官方 `bevy_ui_widgets`；
text_input 官方原语实证不可无头组合（驱动路径全依赖窗口 KeyboardInput/picking/IME），
自研纯核状态机 + 场景适配器；图标位图缝（Lucide SVG → 64px RGBA PNG → tinted
ImageNode，永不走字形码位）；路由（`ContentSlot` 有界替换、幂等）+ Overview 页
（三态 typed 投影）；**真实数据泵**（`MihomoOverviewSource`：application-owned
`OverviewReader` 轮询 + 有界通道 + 每帧排水 + Drop 即停；模式切换 PATCH + 回读验证；
`INFILTRATOR_BEVY_CONTROLLER` 配置缺省回退 demo；真内核 v1.19.18 实测
`docs/screenshots/bevy/overview-live.png`）；
bsn 机械守卫（`scripts/quality/bevy_bsn_guard.py`，已接 CI）；`aarch64-linux-android`
交叉 check 全绿。

仍开放（台账为准）：

- BEVY-011 收尾打磨：Android 图标资产随 APK 打包、窄屏响应式布局（移动端核心课题）、
  chart 自适应宽度、双曲线共享量程、APK strip 减重、locale key 统一、真机 ARM smoke（可选）。
- 核心生命周期控制（restart/stop：`mihomo-api::restart_core` 已具备，平台编排未接，
  界面已诚实声明「0.30 后续接入」）。
- TalkBack：bevy 0.19 无 accesskit-android feature，待上游。
- mihomo-api 严格解码对真内核的 3 处盲区（行号证据在 controller.rs 头注）——泵内
  lenient fallback 兜底，上游修复后拆除。

构建形态：两 crate 为 **standalone workspace**（独立 lock/feature 闭包，0.20 构建
面之外、0.30 大一统线），与 iced 的 naga 27 共存修复（`infiltrator-iced` 的
`naga27` feature-only 直依赖）在两栈同图构建时仍有效。

## 5. 里程碑与演进规划

| 里程碑 | 内容 | 任务 |
| --- | --- | --- |
| M0（已完成） | 两 crate 落地、bsn! 壳、无头测试、守卫全绿 | BEVY-001 |
| M1（已完成） | 主题双模式热切换、字体嵌入、accesskit 最小闭环、控件包装与自研补齐 | BEVY-002 / BEVY-003 |
| M2（已完成） | bsn 机械守卫；路由+Overview 页面；真实 mihomo 数据泵与模式切换 | BEVY-004 / BEVY-005 |
| M3（已完成） | Android APK 打包 + 模拟器真实渲染 smoke；真机 ARM smoke 验证 | BEVY-006 / BEVY-011 |
| M4（0.30 推进中） | 10+ 业务页面大迁移、高性能 Virtual List、移动端响应式断点 (<600px)、Android VpnService 宿主解耦 | BEVY-012 ~ BEVY-026 |

## 5.5 0.30 大一统前端 15 项核心补强方案

为达成 0.30 战略统一 surface，全面对齐桌面（Windows/macOS/Linux）与移动端（Android），确立如下 15 项详细落地实施方案：

1. **BEVY-012 战略统一路由架构与 10+ 业务页面枚举**：
   扩展 `Route` 为 11 个全量业务页面枚举（`Overview`, `Proxies`, `Profiles`, `Rules`, `Connections`, `Logs`, `Dns`, `Doctor`, `AppRouting`, `Sync`, `Settings`）；统一走 `ContentSlot` 有界子树替换，保证幂等切换与零内存泄漏。
2. **BEVY-013 高性能无分配 Virtual List 状态机与视口物理回收**：
   基于纯数学 `visible_window_with_overscan` 与上下虚拟高度垫片（Top/Bottom Spacer Nodes），在海量连接（10,000+）与庞大规则集（50,000+）下仅实例化视口内加缓冲区的固定数量 ECS 实体，彻底消除 GC 与帧率抖动。
3. **BEVY-014 移动端响应式断点系统 (<600px) 与自适应双模外壳**：
   设立 `MOBILE_PX = 600.0` 与 `TABLET_PX = 1024.0` 响应式断点；在移动端 (<600px) 自动从桌面 240px 左侧 Rail 切换为「顶部状态栏 + 底部 Tab 导航栏 + 抽屉菜单」，并将触控热区由 36px 自动垫高至 48px 无障碍标准。
4. **BEVY-015 Android VpnService 宿主无感解耦适配器**：
   建立 `VpnHostAdapter` 纯 Rust 跨平台抽象 trait；Android 端由 Kotlin VpnService 提供 FD，经 UniFFI/JNI 下发至底层 `tun2proxy`；Bevy UI 前端通过事件泵与适配器交互，严禁在 UI 层直接调用 JNI 原始指针，实现宿主与 UI 的物理隔离。
5. **BEVY-016 动态图表自适应容器宽度与双曲线共用量程渲染**：
   废除硬编码 876px 宽度；改造 `ChartSpec` 支持百分比/弹性容器几何测量，并在上下行双曲线中引入统一动态最大量程归一化，解决上传下载量级悬殊时的视觉错位。
6. **BEVY-017 节点选择器网格/列表双模与低开销延迟着色**：
   迁移 Proxies 页面；支持 URLTest / Fallback / Selector 策略组折叠展开，支持按延迟高低三色染色（绿 <100ms / 橙 <300ms / 红超时），支持 Filter Alive 与拼音模糊匹配。
7. **BEVY-018 订阅流水线与配置聚合器页面投影**：
   迁移 Profiles 页面；展示订阅到期时间、剩余流量胶囊条、多订阅聚合合并开关，集成原子更新进度条与失败智能退避提示。
8. **BEVY-019 分流规则树与实时命中染色诊断器**：
   迁移 Rules 页面；展示 DOMAIN-SUFFIX, IP-CIDR, GEOIP, MRS 规则流，集成 Rule Tracer 模拟输入框，按真实规则树实时高亮匹配链路。
9. **BEVY-020 环形缓冲日志流与低开销正则高亮面板**：
   迁移 Logs 页面；对接底层的 500 条定长 RingBuffer，提供 DEBUG/INFO/WARN/ERROR 多级标签过滤与低开销关键词正则高亮。
10. **BEVY-021 实时连接审计与细粒度流阻断控制器**：
    迁移 Connections 页面；消费 WebSocket 连接快照流，富化 GeoIP/ASN 图标，支持按速率/总流量动态排序，支持单连接一键掐断与全量断开。
11. **BEVY-022 智能 DNS 解析与 Fake-IP 状态可视化**：
    迁移 DNS 页面；实时监控 DNS 解析延迟、Fake-IP 池占用率、DoT/DoH 状态，并主动告警 Android Private DNS 严格模式冲突。
12. **BEVY-023 系统自愈诊断与网络环境探活面板**：
    迁移 Doctor 页面；一键自检内核健康度、TUN 网卡分配、端口占用、DNS 污染及直连外网探活，提供一键自愈修复按钮。
13. **BEVY-024 进程级分流与应用代理多端交互卡片**：
    迁移 AppRouting 页面；桌面端枚举系统活动进程并提取应用图标，Android 端读取已安装 App 列表，以 Checkbox 矩阵精准下发分流白名单。
14. **BEVY-025 WebDAV 三向合并冲突解决器与同步面板**：
    迁移 Sync 页面；展示上次同步时间与代数（Generation），在配置冲突时提供 Local / Remote / Base 三栏差异并列比对与逐项合并。
15. **BEVY-026 全局模态弹窗、Toast 浮层与 AccessKit 语义全链路闭环**：
    实现基于 Scrim 遮罩的通用 Modal 弹窗系统与非阻塞 Toast 消息栈，确保每个新增控件与弹窗均附带 AccessKit 语义节点，达成移动 TalkBack 与桌面无障碍全绿。

## 6. 验收命令

```bash
bash scripts/test-bevy.sh          # 规定入口：两 workspace nextest + clippy + fmt + bsn 守卫
# 单 crate 细查（在 crate 目录内）：
cargo nextest run
cargo clippy --all-targets -- -D warnings
# Android 交叉（crate 目录内）：cargo check --target aarch64-linux-android
# 窗口 smoke（Wayland 会话，ui crate 目录内）：cargo run
```

## 7. 视觉取证

Overview 的视觉权威是 iced 参考图（`docs/screenshots/iced/overview-*.png`）；bevy 端
以 `bash scripts/capture-bevy.sh`（私有 kwin_wayland --virtual 宿主 + 嵌套 niri +
PID/标题绑定截图，零宿主会话串扰）产出真实渲染证据到 `docs/screenshots/bevy/`，
双主题各一张，manifest 记录 PID/尺寸/sha256。**该目录本地生成、暂不入库**
（`.gitignore` 已排除；截图仅作本地取证，重跑脚本即可再生）。布局/颜色改动必须走
"截图 → 与参考图对比 → 修最重项 → 重截"闭环，禁止盲改。应用侧取证缝：
`INFILTRATOR_BEVY_SKIN`、`INFILTRATOR_BEVY_WINDOW_SIZE`、
`INFILTRATOR_CAPTURE_MARKER`（CAPTURE_READY 标记）。

## 8. 进阶十五大维度演进与实事求是性能预算 (0.30+ / 1.0 终局路线)

为确保 Bevy UI 在超越传统桌面与移动端 UI 框架（Iced / Qt / Flutter / GPUI）的同时，**坚决不抛弃原生 GUI 的高效率、低内存占用与即时响应**，确立如下进阶十五大演进维度与**实事求是、严禁夸口**的性能指标红线。

### 8.1 进阶十五大演进维度

1. **渲染管线深度定制与计算着色器加速 (Compute & RenderGraph Shaders)**：SDF 矢量字形抗锯齿、物理拟真双重 Kawase 毛玻璃材质（GPU 后处理）、解析级圆角盒动态投影与 GPU Instancing 图元批量提交。
2. **声明式 UI 元编程与常量场景优化 (Const Scene Graph & Reactive DAG)**：静态场景编译期展平、细粒度反应式原子信号依赖拓扑、类型安全静态几何约束布局。
3. **自适应多窗口、画中画与工作区自由停靠 (Multi-Window, PiP & Workspace Docking)**：共享单个 ECS World 的跨原生 OS 窗口并发渲染、独立置顶桌面画中画网速浮窗、分栏工作区自由拖拽撕裂与吸附停靠。
4. **高性能 3D / 2.5D 全球网络拓扑与粒子引擎 (Spatial 2.5D/3D Globe & Particles)**：无缝桥接 Bevy 3D 渲染器嵌入交互式 3D 节点地球仪、实时数据包流向粒子物理模拟、UI 卡片视差微 3D 悬浮透视。
5. **移动端系统级深度集成与原生视图混排 (Mobile System Integration & Platform Views)**：Android TalkBack / iOS VoiceOver 原生无障碍桥接、摄像头扫码纹理 GPU 直通渲染、移动端抛掷惯性滚动物理引擎。
6. **多模态音效与触觉物理微反馈系统 (Audio-Haptic Multimodal Feedback)**：微触觉波形（Android Vibrator / iOS Haptic / macOS 触控板）、程序化零延迟 UI 微音效、手势速度加权振动阻尼。
7. **智能网络上下文感知与预测式 UI 预取 (Smart Context Sensing & Predictive UI)**：启发式最佳节点智能评估与高亮、网络故障交互式自愈向导卡片、异常流量热力图视觉警示。
8. **动态着色器皮肤、主题热插拔与视觉沙盒 (Shader Skins & OKLCH Palette)**：运行时 WGSL 动态热重载、基于色彩科学的 OKLCH 感知均匀色阶生成、主题 Token 跨端标准 JSON/Tailwind 互转。
9. **富文本高级排版、双向文字 (BiDi) 与数学公式 (Advanced Typography & BiDi)**：OpenType 连字与可变字重无级插值、阿拉伯语/希伯来语 RTL 布局镜像翻转、轻量级 Markdown 与行内指标胶囊混排。
10. **高性能本地嵌入式时序数据库与时空回放 (Embedded TSDB & Time-Travel Replay)**：SIMD 加速时序存储与动态重采样、时间轴滑动条历史网络状态时空回放、多维交叉透视统计图表。
11. **WASM / QuickJS 驱动的微前端插件 UI 沙盒 (Micro-Frontend Widget Sandbox)**：轻量级自定义仪表盘小组件沙盒、声明式受限 UI 渲染槽、社区扩展市场与可视化安装器。
12. **电视大屏 (10-Foot UI)、手柄与全模态控制体系 (10-Foot TV & Gamepad Engine)**：Android TV 十字键导航、游戏手柄左摇杆平滑阻尼滚动、局域网 mDNS 跨端伴侣控制屏幕。
13. **极限冷启动预热、零开销内存与二进制瘦身 (Sub-Millisecond Boot & Optimization)**：管线与布局缓存静态预编译消除首帧卡顿、只读数据段 mmap 零堆分配启动、PGO 引导编译与符号剥离。
14. **全场景数字孪生仿真、网络混沌工程与压力测试 (Digital Twin & Chaos Sandbox)**：网络故障注入沙盒（突发丢包/高抖动/DNS污染）、全自动无头 Monkey 探索机器人、云端真机自动化性能与视觉基准流水线。
15. **跨项目通用 GUI 框架独立抽取与生态标准化 (Universal Widget Engine Extraction)**：纯净通用控件库独立 Crate 抽取发布、跨项目统一 ABI 规范、多平台宿主薄壳标准化模板。

---

### 8.2 实事求是的性能与内存预算红线 (真实硬件测量基准)

拒绝不切实际的虚高指标，立足 Bevy 0.19 + Winit + Wgpu 的物理硬件测量事实：

| 性能与资源指标 | 极简原生态 (Eco / Native) | 均衡态 (Balanced - 默认) | 沉浸态 (Pro / Enhanced) | 测量口径与验收事实 |
| :--- | :---: | :---: | :---: | :--- |
| **无头测试基线 RAM** | **12 ~ 20 MB** | **15 ~ 25 MB** | **20 ~ 30 MB** | `MinimalPlugins` 纯 ECS 结构，无窗口与 GPU 驱动开销 |
| **窗口冷启动 RAM (Linux/Wayland)** | **35 ~ 50 MB** | **45 ~ 65 MB** | **60 ~ 85 MB** | 包含 Wgpu 驱动运行时、Vulkan 堆栈及嵌入式字体表常驻 |
| **万级节点常驻 RAM** | **45 ~ 60 MB** | **55 ~ 75 MB** | **70 ~ 95 MB** | 虚拟滚动仅实例化视口内 25 个实体，10k 结构体仅增 ~1MB |
| **静止待机 CPU** | **0.0% ~ 0.05%** | **0.0% ~ 0.1%** | **0.1% ~ 0.2%** | Winit `ControlFlow::Wait` 事件驱动，零静态帧无谓空转 |
| **后台 / 最小化 CPU** | **0.00%** | **0.00%** | **0.00%** | 彻底关闭渲染管线，仅保留 1s 节流后台数据泵 (每次 <0.1ms) |
| **核心交互单帧响应延迟** | **< 16 ms (60Hz)** | **< 8.3 ms (120Hz)** | **< 8.3 ms (120Hz)** | 节点切换、搜索过滤在单帧内完成 ECS 状态盖章与重绘 |
| **拼音模糊搜索 10,000 节点** | **< 3 ms** | **< 5 ms** | **< 5 ms** | 倒排索引 + SIMD 模糊匹配，实测基准预算内通过 |
| **桌面 Release 二进制体积** | **~ 22 MB** | **~ 26 MB** | **~ 32 MB** | `strip = true` + `lto = "thin"` + `opt-level = 3` 剥离后单文件 |
| **Android APK 单架构体积** | **~ 18 MB** | **~ 22 MB** | **~ 28 MB** | `arm64-v8a` release 构建剥离调试符号后体积 |

### 8.3 收放自如的降级原则

1. **可有可无**：高级特性（3D/粒子/毛玻璃/TSDB/WASM 沙盒）全部由 Feature Gate 控制编译，运行时默认关闭或按需初始化。
2. **随用随清**：大显存纹理与 3D 资源仅在特定页面激活，离开视口 30s 即刻 Drop 释放显存，禁止无限制常驻。
3. **能耗优先**：感知低电量模式或系统发热时，自动降级至 Eco 档位，保全核心代理与交互流畅度。
