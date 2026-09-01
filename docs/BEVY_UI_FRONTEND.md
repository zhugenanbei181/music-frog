# Bevy UI 前端章程

本文是 Bevy UI 前端的公开事实权威。Bevy UI 是 MusicFrog Infiltrator 的**战略统一
surface**：它的上限包含移动端。iced 是 winit 桌面方案，没有 Android 故事；bevy 0.19
的同一棵 UI 树可以直接跑在 `aarch64-linux-android` 上。控件生态薄是事实，但
`bevy_ui_widgets`（官方无样式控件包）加上我们自有的 `infiltrator-bevy-widgets`
层可以补齐——控件是我们自己建的，就归我们自己所有。iced 主桌面维持维护态并继续
承接 X11 会话；新产品能力仍以 Iced 优先落地，Bevy 按 M1→M3 里程碑追赶。

参考实现：taskmanager 的 `taskmanager-bevy-ui`（同为 bevy 0.19 产品级 UI，bsn! 场景法
+ 观察者绑定 + 纯核/场景适配器二分 + 中立主题 token）。本章程的多条铁律直接来自该
项目的成文法律与踩坑记录。

## 1. 基座铁律

- **bevy 锁定 `=0.19.1`**：精确到 patch，与 taskmanager 同锁。升级属于架构与发布
  评审事项（bevy 0.19 已把 UI 布局与渲染拆开，`bevy_ui` 单独只排版不上屏，必须闭合
  `bevy_ui_render`）。
- **依赖白名单**：`infiltrator-bevy-widgets` 只依赖 bevy；`infiltrator-bevy-ui` 只
  依赖 bevy + widgets 层。业务 crate（`infiltrator-core`、`mihomo-*`）在共享
  contract seam（BEVY-005）落地前**不得**进入白名单。
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
| `infiltrator-bevy-ui` | 前端壳：窗口组合（`DefaultPlugins`）、`ShellPlugin`、路由与页面（M2 起） | bevy + widgets 层 |

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
（三态 typed 投影）；**真实数据泵**（`MihomoOverviewSource`：mihomo-api 轮询 +
有界通道 + 每帧排水 + Drop 即停；模式切换 PATCH + 回读验证；`INFILTRATOR_BEVY_CONTROLLER`
配置缺省回退 demo；真内核 v1.19.18 实测 `docs/screenshots/bevy/overview-live.png`）；
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

## 5. 里程碑

| 里程碑 | 内容 | 任务 |
| --- | --- | --- |
| M0（已完成） | 两 crate 落地、bsn! 壳、无头测试、守卫全绿 | BEVY-001 |
| M1（已完成） | 主题双模式热切换、字体嵌入、accesskit 最小闭环、控件包装与自研补齐 | BEVY-002 / BEVY-003 |
| M2（已完成） | bsn 机械守卫；路由+Overview 页面；真实 mihomo 数据泵与模式切换 | BEVY-004 / BEVY-005 |
| M3（模拟器 L3 达成） | Android APK 打包 + 模拟器真实渲染 smoke；真机 ARM smoke 可选 | BEVY-006（收尾打磨：BEVY-011） |

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
