# MusicFrog Infiltrator: 双端 UI/UX 体验演进与视觉系统主控台账 (Dual-Surface UI/UX Experience Roadmap)

> **权威声明**：本文档是 MusicFrog Infiltrator 项目在 **UI/UX 体验、双端视觉设计系统（Design System）、全流体响应式布局（Responsive Layout）、平滑圆角（Squircle / G2 连续曲率）、现代材质着色器与物理微交互** 上的唯一最高权威主控台账。
> 架构分层遵守 [ARCHITECTURE.md](ARCHITECTURE.md)；双端功能并集对标遵守 [DUAL_SURFACE_PARITY_MASTER_PLAN.md](DUAL_SURFACE_PARITY_MASTER_PLAN.md)；多视口弹性断点遵守 [RESPONSIVE_PARITY_LEDGER.md](RESPONSIVE_PARITY_LEDGER.md)；外壳无头回归遵守 [MULTIMODAL_SHELL_MATRIX.md](MULTIMODAL_SHELL_MATRIX.md)。

---

## 1. 双端 UI 战略定位与核心体验愿景

项目确立**“双表面并行演进、单事实源驱动（Single Source of Truth, SSOT）”**的架构主线：

```
                              [ Single Source of Truth ]
                   crates/infiltrator-contract & infiltrator-application
              (ResponsiveViewportSnapshot / TrafficSnapshot / CoreLifecycle)
                                         │
                     ┌───────────────────┴───────────────────┐
                     ▼ (Elm 纯函数 MVU 架构)                  ▼ (Bevy 0.19 ECS + GPU 管线)
          [ infiltrator-iced ]                     [ infiltrator-bevy-ui ]
        桌面生产力与轻量守护基石                  跨平台战略旗舰 (桌面/移动全适配)
      - 常驻内存 < 20MB                         - Bevy 0.19 现代化 UI 引擎
      - Linux/Windows/macOS 原生沉浸           - WGSL 硬件着色器加速、高刷微动效
      - 极致稳定、低功耗后台常驻               - 移动端 (Android/iOS) 触控与手势深度闭环
```

### 1.1 Iced 表面定位：桌面生产力与轻量守护标杆
- **架构范式**：依托 Elm 纯函数响应式架构（Model-View-Update），具备状态单向流动、逻辑纯粹、零额外开销特征。
- **资源底线**：长时常驻内存严格控制在 **20MB** 以内，保障网络代理内核 7×24 小时长期无泄漏稳定运行。
- **体验重心**：高信息密度排版、毫秒级界面回响、系统托盘（SNI/ksni/原生）深度无缝集成、键盘快捷键全覆盖。

### 1.2 Bevy UI 表面定位：跨平台现代战略旗舰
- **架构范式**：依托 Bevy 0.19 ECS 高并发架构与现代 GPU 硬件渲染管线，承载桌面高刷体验并向移动端（Android/iOS/折叠屏）全面辐射。
- **渲染基石**：WGSL 硬件着色器加速、苹果级超椭圆连续曲率圆角、物理阻尼弹簧微动效、全链路触控手势闭环。
- **组件分工**：底层纯 UI 原语完全收敛于业务无关的 `crates/infiltrator-bevy-widgets/`，业务外壳与路由装配由 `crates/infiltrator-bevy-ui/` 承担。

### 1.3 严格单一业务事实源（SSOT）与事务总线
- 业务逻辑与状态推导完全下沉于 `crates/infiltrator-contract/` 与 `crates/infiltrator-application/`。双端仅消费不可变状态快照（`ResponsiveViewportSnapshot`、`TrafficWaveformSnapshot`、`CoreLifecycleSnapshot` 等）。
- 双端所有写操作统一封装为 `UiCommand` 事务总线，确保在 Overview、Proxies、Subscriptions、Rules、Connections、Logs、DNS、Doctor、App Routing、Cloud Sync、Settings 等 11 个业务路由下交互语义 100% 对齐。

---

## 2. 响应式布局（Responsive Layout）全景工程规范

### 2.1 四阶断点契约（4-Tier Breakpoint Architecture）
依据 `crates/infiltrator-contract/src/responsive_viewport.rs` 与 [RESPONSIVE_PARITY_LEDGER.md](RESPONSIVE_PARITY_LEDGER.md)，统一固定为单一事实源：

| 断点梯队 | 物理/逻辑宽度 | 外壳形态 (`SidebarForm`) | 内容边距 | 代理节点网格 | 拓扑与指标布局 |
| :--- | :--- | :--- | :--- | :--- | :--- |
| **Compact** | `< 600px` (手机竖屏/窄分屏) | `BottomNav` (底部导航栏) + 侧滑抽屉 | `16px` | 1 列紧凑卡片 | 指标 2 列；拓扑流水线纵向折叠 |
| **Medium** | `600px .. 840px` (平板/折叠屏/半屏) | `Rail` (64px 纯图标导轨，带悬浮提示) | `24px` | 2 列流体卡片 | 指标 3 列；拓扑两列自适应流 |
| **Expanded** | `840px .. 1200px` (标准桌面/笔记本) | `Standard` (240px 完整文字侧栏) | `40px` | 3 列卡片 | 指标 6 列；拓扑 5 节点横向完整流水线 |
| **Ultra** | `>= 1200px` (2K/4K/带鱼屏) | `Wide` (280px 宽松侧栏) | `48px` | 4 ~ 6 列流体 | 指标 6 列宽幅自适应展开 |

### 2.2 Bevy 端：接入 `ComputedNode` 激活流式网格（Fluid Grid）
- **现状缺口**：当前 Proxies 与 Overview 页面采用固定百分比（如 48%、31%），留有 2%~7% 冗余间隙防止折行穿模，无法做到与容器边缘像素级对齐。
- **演进方案**：
  1. 激活 `crates/infiltrator-bevy-widgets/src/fluid_grid.rs` 中的 `compute_ideal_column_layout` 算法。
  2. 构建 `FluidGridLayoutPlugin`，在 `PostUpdate` 阶段利用 ECS 直接读取父容器的 `ComputedNode::size().x` 实际像素宽度。
  3. 动态求解理想列数：
     $$\text{cols} = \text{clamp}\left(\left\lfloor \frac{W_{\text{container}} + \text{gap}}{W_{\text{min}} + \text{gap}} \right\rfloor, 1, \text{max\_cols}\right)$$
  4. 卡片宽度严格基于求解结果做像素级自适应计算，100% 填满父容器横向空间，消除不平整的非必要留白。

### 2.3 Iced 端：清除刚性尺寸残留，构建自适应表单原语
- **现状缺口**：Settings、Doctor、DNS 等面板中依然残留固定的 `width(150)` 或 `width(180)` 左侧标签容器；拓扑图横向硬编码排布，在 360px ~ 480px 窄窗口下产生水平溢出。
- **演进方案**：
  1. **封装 `ResponsiveFormRow` 自适应表单原语**（新增 `crates/infiltrator-iced/src/view/responsive_form.rs`）：
     - 在 `Compact` 阶：自动解构为垂直纵向流 `column![label, control]`，标签置顶，输入框拉伸至 `Length::Fill`；
     - 在 `Medium` 及以上阶：转换为自适应比例栅格，标签容器使用 `Length::FillPortion(1)` 搭配 `max_width(200)`，控件区域使用 `Length::FillPortion(2)`。
  2. **拓扑面板动态折行**：在 `src/view/overview_topology.rs` 中，Compact 模式下 5 节点横向流自动折叠为纵向 S 型流水线，连接箭头自动从 `->` 切换为向下拐角连接符。
  3. 全面清零 56 处刚性像素，确保在 360px 极端窄屏下实现**零横向滚动条、零文字截断**。

### 2.4 移动端安全区（Safe-Area Insets）与沉浸式适配
- 在 `crates/infiltrator-bevy-ui/src/shell_scene.rs` 中全面联动 `GestureHostReport::insets`。
- 动态注入安全边距：`padding.bottom = insets.bottom + 6.0px`，顶部标题栏避让系统状态栏与前摄挖孔，彻底消除 Android 底部手势横条和 iPhone 灵动岛遮挡交互热区的问题。

---

## 3. 视觉设计系统与圆角/质感升维（Aesthetics & Materials）

### 3.1 设计令牌（Design Tokens）工业级扩充
当前 `crates/infiltrator-contract/src/design_tokens.rs` 仅收录 12 个核心色与 5 个交互色，圆角仅有 `CARD: 16.0` 和 `CONTROL: 10.0`。本次将其扩充为覆盖全场景的工业级设计规范：

```rust
// 1. 扩充后的精细圆角阶梯 (Radius Ladder)
pub mod radius {
    pub const XS: f32 = 4.0;       // 微标签 (Tag)、紧凑型 Tooltip 气泡
    pub const SM: f32 = 8.0;       // 次级按钮、输入框内嵌控件、分段控制器 (Segmented)
    pub const CONTROL: f32 = 10.0;  // 标准表单输入框、主操作按钮
    pub const CARD: f32 = 16.0;     // 核心卡片容器、底部抽屉 (Sheet) 顶部边缘
    pub const MODAL: f32 = 24.0;    // 居中模态对话框、全局命令面板 (Palette)
    pub const PILL: f32 = 999.0;    // 药丸胶囊按钮、网络延迟与在线状态 Chip
}

// 2. 连续曲率超椭圆契约 (Squircle / G2 Continuity)
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CornerCurvature {
    pub radius_px: f32,
    /// 平滑系数: 0.0 为传统圆弧 (G1), 0.6 为 iOS 标准 Squircle, 1.0 为完全超椭圆 (G2)
    pub smoothing: f32,
}

// 3. 统一字阶比例 (Typography Scale)
pub mod type_scale {
    pub const DISPLAY: f32 = 24.0;  // 顶层巨幅数据指标 (如 128 MB/s 实时带宽)
    pub const TITLE: f32 = 20.0;    // 页面主标题
    pub const HEADING: f32 = 16.0;  // 卡片标题、区块分组名
    pub const BODY: f32 = 14.0;     // 常规正文文本、标准表单输入
    pub const CAPTION: f32 = 12.0;  // 辅助说明文字、节点协议标注
    pub const TAG: f32 = 10.0;      // 毫秒延迟微标、紧凑状态标签
    pub const MONO: f32 = 13.0;     // 等宽数字字体 (IP、端口、延迟、流量)
}

// 4. 双层物理环境光阴影系统 (Two-Layer Elevation Shadow)
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ElevationToken {
    pub offset_y: f32,
    pub blur_radius: f32,
    pub spread: f32,
    pub ambient_alpha: f32,
    pub key_alpha: f32,
}
pub mod elevation {
    // 基础贴地卡片 (刻画轮廓 + 贴地微扩散)
    pub const LOW: ElevationToken = ElevationToken {
        offset_y: 2.0, blur_radius: 4.0, spread: 0.0, ambient_alpha: 0.04, key_alpha: 0.08,
    };
    // 悬浮菜单、下拉面板 (中度浮起)
    pub const MEDIUM: ElevationToken = ElevationToken {
        offset_y: 6.0, blur_radius: 12.0, spread: 1.0, ambient_alpha: 0.06, key_alpha: 0.12,
    };
    // 居中模态对话框、底栏抽屉 (高位悬浮)
    pub const HIGH: ElevationToken = ElevationToken {
        offset_y: 16.0, blur_radius: 32.0, spread: 2.0, ambient_alpha: 0.08, key_alpha: 0.20,
    };
}
```

### 3.2 Bevy 端：GPU SDF 连续曲率圆角（Squircle）与实时渲染着色器
- **连续曲率（G2 Continuity）超椭圆数学原理**：
  传统 GUI 圆角直接用切线拼接直线与四分之一圆弧，交界处曲率从 0 突变至 $1/R$（仅满足 G1 连续），在大圆角卡片（16px）与模态框（24px）边缘会产生生硬折角感。
  采用超椭圆（Superellipse）有向距离场（Signed Distance Field, SDF）：
  $$\left(\frac{|x|}{a}\right)^p + \left(\frac{|y|}{b}\right)^p \le 1 \quad \text{其中 } p = 2.0 + 3.0 \times \text{smoothing} \approx 4.5 \sim 5.0$$
- **WGSL 着色器实现与管线挂载**：
  1. 在 `crates/infiltrator-bevy-widgets/src/shader_fx.rs` 中完善 `SdfRoundedBox`，通过 Bevy `UiMaterial` 注册 `ModernSurfaceMaterial`；
  2. 片段着色器中求值超椭圆距离场，结合 `fwidth(d)` 与 `smoothstep` 做亚像素抗锯齿过滤，根除高分屏非整数缩放下的边框发虚与折角跳跃。
- **单 Pass 解析级软阴影（Analytical Drop Shadow）**：
  在 Quad 扩展网格内直接利用解析式 SDF 距离计算阴影半影扩散（Penumbra Falloff），替代昂贵的多 Pass 双重高斯模糊，在 60/120 FPS 高帧率下以微秒级 GPU 开销呈现细腻的物理悬浮投影。
- **状态呼吸发光（GPU Neon Glow）**：
  将 `GlowSpec` 注入着色器 Uniform，为当前选中的活动代理节点、TUN 运行指示灯和网络拓扑通路提供细腻的呼吸脉冲发光。
- **亚克力与毛玻璃材质（Acrylic / Frosted Glass）**：
  Windows 11 启用 Mica、macOS 启用系统原生窗口模糊；通用渲染层基于 `KawasePassMetrics`（双重降采样）实现轻量级背景毛玻璃与抗条带噪点纹理。

### 3.3 Iced 端：边缘裁剪、复合投影与排版重构
- **安全溢出裁剪（Scissor Clipping）**：
  针对 Iced 基础 `container` 设置 `border.radius` 后无法对子元素施加物理裁切的局限，在 `src/view/components.rs` 引入安全裁剪层，解决表格表头背景刺破卡片圆角的视觉瑕疵。
- **双层投影融合**：
  在 `src/view/theme.rs` 中将阴影拆分为贴身轮廓层（Key Light: Blur 2px，强化边缘硬朗感）与立体漫反射层（Ambient Light: Blur 12px~32px，柔和环境扩散），消除大半径单层投影发黑发脏的质感问题。
- **统一字阶规范**：
  全面梳理清理 `src/view/` 中散落的裸写 `.size(...)`，统一收敛至契约约定的 `theme::type_scale`。

---

## 4. 微交互、动效与手势体系（Micro-interactions & Motion）

### 4.1 物理微动效与 ECS 弹簧动力学管线
- **Bevy 端阻尼弹簧动力学（Spring Physics）**：
  - 激活 `crates/infiltrator-bevy-widgets/src/motion.rs` 中的阻尼振子 `Spring` 模型；
  - 注册 `SpringAnimationPlugin`，通过为 UI 实体挂载 `SpringAnimator`，在系统更新中平滑驱动 `Transform.scale` 与坐标偏移：
    - **页面切换过渡**：路由跳转时提供 120ms 弹性平移（Y 轴从 +8px 柔和回弹至 0px）与透明度渐入；
    - **按钮按下反馈**：鼠标按下或手指触摸时执行 Scale 0.96 瞬时微压缩，松手弹性回弹（Press Down & Pop）；
    - **开关切换**：ToggleSwitch 开关滑块具备带阻尼的惯性滑动质感；
    - **卡片交错进场**：节点网格初次渲染时，以每张卡片 25ms 间隔执行阶梯淡入与轻微上浮（Staggered Enter）。
- **Iced 端微光扫光骨架屏（Shimmer Wave）**：
  - 升级 `src/view/components.rs` 的 `skeleton_box`，利用 `canvas::Program` 绘制 45 度对角线性渐变微光扫光波，节点列表加载中呈现流畅脉冲动效，淘汰静态死板的纯灰块。

### 4.2 全局状态机反馈（Hover / Focus / Pressed）
- **全局 Tab 键焦点环（Focus Ring）全面贯通**：
  - 双端表单控件、开关、侧栏按钮全量集成 `Focus` 状态机；
  - 聚焦时高亮显示 2px 外发光轮廓（对齐 `SkinInteractionPalette::focus_ring`），与 AccessKit 无障碍焦点树 100% 联动。
- **卡片悬浮升起（Hover Lift）**：
  - 鼠标悬停卡片时背景亮度微提 2%~4%，阴影层级平滑由 `LOW` 升至 `MEDIUM`，提供真实的微上移层次感。

### 4.3 移动端触控手势全链路闭环
- **下拉刷新（Pull-to-Refresh）**：
  - 联动 `crates/infiltrator-bevy-ui/src/gesture.rs` 中的 `PullToRefreshState`，在代理页、规则页与订阅页顶部集成下拉弹性指示器；
  - 下拉呈现二次方阻尼衰减，拉动超过 80px 释放即触发内核刷新，并带有丝滑回弹。
- **列表侧滑快捷操作（Swipe-to-Action）**：
  - 代理节点卡片支持左滑浮出“设为星标”、“单节点延迟测速”；
  - 连接审计列表支持左滑“瞬间断开连接”。
- **48px 无障碍触控热区强制约束**：
  - 消除当前小尺寸按钮（高度仅 28.8px）在移动端的误触隐患；
  - 在 `ResponsiveContext::is_compact()` 生效时，底层拾取判定盒（Picking Hitbox）自动强制外扩补齐至不低于 48×48 逻辑像素。

### 4.4 可视化遥测交互深度下钻
- **实时网速波形交互十字准星**：
  - 光标划过网速曲线时捕获时间切片，渲染垂直虚线准星与折线吸附交点，动态浮出微型 HUD Tooltip，指示该时点的上下行精准速率。
- **分流拓扑动态下钻**：
  - 拓扑流水线粒子流动速度与实时网络吞吐带宽线性绑定；
  - 鼠标悬停某节点高亮整条分流链，点击节点直接携带过滤条件跳转至对应 Rules 或 Proxies 详情页。

---

## 5. 组件库双端演进与无头回归守护（Governance）

```
                            [ Design Token Schema ]
                           design_tokens.json (W3C)
                                      │
                      ┌───────────────┴───────────────┐
                      ▼ (build.rs 自动化代码生成)      ▼
         [ infiltrator-contract ]        [ infiltrator-bevy-widgets ]
            design_tokens.rs                       theme.rs
                      │                               │
                      └───────────────┬───────────────┘
                                      │
                                      ▼
                        [ Fail-Closed CI 门禁体系 ]
    ├── scripts/quality/multimodal-shell-guard.py (色彩浮点偏差/字阶/圆角/禁忌扫描)
    ├── scripts/quality/responsive-parity-guard.py (600/840/1200 断点与弹性门禁)
    ├── Headless 自动化断言测试套件 (无头集成验证)
    └── 跨分辨率像素级视觉截屏比对流水线
```

1. **业务无关组件库分层**：
   - 保持 `crates/infiltrator-bevy-widgets/` 严格独立于业务逻辑，专注于 UI Primitives（`Button`、`Card`、`Modal`、`Drawer`、`SegmentedControl`、`ToggleSwitch`、`Chip`、`WaveformChart`、`Skeleton`）；
   - Iced 端在 `crates/infiltrator-iced/src/view/components.rs` 维护对称的组件抽象，确保双端 API 命名与行为规范高度镜像。
2. **单一事实源代码生成器**：
   - 引入标准化的 `design_tokens.json`，通过构建脚本统一向 `infiltrator-contract` 与 `infiltrator-bevy-widgets` 生成 Rust 源码常量，彻底根除手动镜像同步可能带来的数值漂移。
3. **Fail-Closed CI 门禁与无头回归网**：
   - **数值守卫**：`multimodal-shell-guard.py` 严格校验 4 套主题皮肤（Dark, Light, Forest, Amoled）在两端各通道色值浮点误差小于 $10^{-6}$，禁止裸写 hairline 尺寸与黑色遮罩，并扩展覆盖字阶与圆角阶梯；
   - **响应式守卫**：`responsive-parity-guard.py` 强制校验 600 / 840 / 1200 门限与各窗口订阅通道，阻断未经流式处理的硬编码容器；
   - **无头端到端测试**：依托 `tests/headless/design_token_tests.rs` 与 `tests/gui/view_theme_tests.rs`，确保在无显示器的 CI 环境下自动化验证所有主题切换与状态断言。

---

## 6. 四大里程碑演进清单（按图索骥执行表）

本清单作为后续任务执行的基准台账，逐项按序推进，完成一项在此打勾 `[x]` 并关联对应 PR/提交。

### 阶段一：设计系统基石扩充与刚性尺寸清零 (Phase 1: Foundation & Fluidity)
- [x] **UI-01-01** `contract::design_tokens` 扩充 `radius`（XS/SM/CONTROL/CARD/MODAL/PILL）、`type_scale`、`elevation` 与 `CornerCurvature` 规范。
- [x] **UI-01-02** `infiltrator-bevy-widgets::theme` 同步对齐上述阶梯常量，通过无头镜像测试锁定。
- [x] **UI-01-03** 新增 `crates/infiltrator-iced/src/view/responsive_form.rs` 自适应表单行原语，支持 Compact 阶单列垂直回落。
- [x] **UI-01-04** 消除 Iced 端 Settings、DNS、Doctor 等页面残留的 56 处固定像素宽度，转为比例栅格或弹性填充。
- [x] **UI-01-05** Overview 拓扑卡片支持 Compact 模式下的纵向流水线折叠与弯角箭头。
- [x] **UI-01-06** 扩展 `scripts/quality/multimodal-shell-guard.py`，加入圆角与字阶守卫，防止裸写魔法数字。

### 阶段二：流式栅格与 4 阶形态自适应闭环 (Phase 2: Fluid Grid & Responsive Shell)
- [x] **UI-02-01** Bevy 激活 `ComputedNode` 读取实际父容器宽度，将 `fluid_grid.rs` 的 `compute_ideal_column_layout` 接入 Proxies 与 Overview 节点网格。
- [x] **UI-02-02** Bevy 补齐在 Medium 阶（600~840px）下的 64px 纯图标 Rail 导轨形态与 Tooltip 悬浮提示。
- [x] **UI-02-03** Bevy 端挂载 `SafeAreaInsets`，联动系统状态栏与底部手势横条安全避让。
- [x] **UI-02-04** Iced 端全量覆盖 4 阶视口变化下的无头弹性断言（360px ~ 1920px 任意视口零截断、零横向溢出）。
- [x] **UI-02-05** `scripts/quality/responsive-parity-guard.py` 补充流体网格与外壳形态的严格校验规则。

### 阶段三：GPU SDF 连续圆角、双层阴影与微动效 (Phase 3: Squircle & Shaders & Micro-motion)
- [x] **UI-03-01** 在 `crates/infiltrator-bevy-widgets/src/shader_fx.rs` 落地基于 WGSL 的 G2 连续曲率 Squircle 着色器（`ModernSurfaceMaterial`）。
- [x] **UI-03-02** Bevy 着色器集成单 Pass 解析式软阴影算法（Analytical Drop Shadow），支持 Low/Medium/High 三阶环境光漫反射。
- [x] **UI-03-03** Iced 端引入安全边缘裁剪层（Scissor Clipping）与双层复合阴影系统（Key Light + Ambient Light）。
- [x] **UI-03-04** Bevy 挂载 `SpringAnimationPlugin`，接管按钮缩放、开关切换、路由切换与卡片交错进场动效。
- [x] **UI-03-05** Iced 交付基于 Canvas 绘制的动态微光扫光骨架屏（Shimmer Wave）。
- [x] **UI-03-06** 全局 Tab 键 2px 发光 Focus Ring 与卡片 Hover Lift 微上移状态机双端贯通。

### 阶段四：移动端触控手势闭环、数据下钻与全平台发布 (Phase 4: Mobile Gestures & Interactive Drilldown)
- [x] **UI-04-01** Proxies、Rules、Subscriptions 页面接入下拉刷新（Pull-to-Refresh）手势指示器与阻尼弹性回弹。
- [x] **UI-04-02** 节点卡片与连接项支持 Swipe-to-Action 侧滑快捷操作（星标、单节点测速、切断连接）。
- [x] **UI-04-03** 移动端 Compact 模式下强制外扩 48px 最小触控拾取热区（Hitbox）。
- [x] **UI-04-04** 流量波形图加入 Hover 交互十字准星与瞬时速率微型 HUD Tooltip。
- [x] **UI-04-05** 分流拓扑流动速度与吞吐带宽线性绑定，节点支持穿透下钻至对应 Rules 或 Proxies 列表。
- [x] **UI-04-06** 深度适配 Windows 11 Mica、macOS 磨砂亚克力与 OLED 纯黑（Amoled）皮肤。
- [x] **UI-04-07** 搭建基于无头渲染的跨分辨率像素级自动化截图视觉回归测试流水线。
