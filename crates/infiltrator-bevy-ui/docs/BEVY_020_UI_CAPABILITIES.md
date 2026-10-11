# Bevy 0.20 UI 能力规范

> 层级：L3
> 范围：Bevy 0.20 UI 能力在 `infiltrator-bevy-ui` 与 `infiltrator-bevy-widgets` 中的实现约束。

本文是 Bevy 0.20 UI 能力在本项目的**唯一权威使用规范**。所有 `infiltrator-bevy-ui`、`infiltrator-bevy-widgets` 新代码必须遵守；违反任意一条的代码不允许合入。静态守卫的覆盖范围以实际注册的规则为准，未被机器检查的要求仍须由回归和验收证明。

## 总纲

1. **官方 headless 优先**：交互行为（选择、焦点、键盘导航、a11y 语义）必须来自 `bevy_ui_widgets` headless 控件或 `bevy_feathers` 主题/控件框架；自建交互状态机是例外，必须在本文件记录理由与复审触发条件。
2. **显式状态化**：凡可用有限状态表达的 UI 状态（开关、层级、表单形态、选中态）一律用显式枚举/组件（如 `SurfaceLevel`、`TabActivation`、`SelectedTab`、`MenuFocusState`），禁止用散布的 `bool` 字段加 if/else 推断状态。
3. **token 单一权威**：颜色、字号、间距只有一个来源（widgets crate 的 `theme`/`palette`/`text`），UI 层不得出现颜色/字号字面量。
4. 本文件"落点"一节列出每项能力对应的已删除旧入口，回归即守卫失败。

## ① rem/em 动态字号与尺寸（`Val::Rem`/`Val::Em`、`FontSize::Rem`、`RemSize`/`EmSize`）

- `RemSize` 资源是**全局根字号，也是全 UI 唯一缩放入口**：壳层按用户字号/密度偏好写入，换档不重建树。
- 根基准 `REM_BASE = 15.0`（= `type_scale::BODY`）。文字字号一律 `FontSize::Rem(px_at_baseline / REM_BASE)`；间距、半径、控件高度等尺寸 token 一律 `Val::Rem`（widgets `theme` 提供换算 helper）。
- `Val::Em` 仅用于**相对所在文字字号**的尺寸（行内图标、随文字缩放的内嵌元素）；节点无 `TextFont` 时不得用 em。
- **px 例外白名单**（只能这些场景写 px）：1px 发丝线与 2px 光标（`metrics::HAIRLINE`/`CARET_WIDTH`）、视口断点（`breakpoint`，视口语义而非字号语义）、最小触控目标（`MIN_TOUCH_TARGET`，物理可达性下限）、inset 物理像素换算结果、GPU/测量代码。
- 禁止：新增 `FontSize::Px` 写入、新增把字号/间距换算散落到页面的代码。
- 落点：`W/theme.rs`（type_scale/space/radius/metrics 输出 rem）、`W/text.rs`（role_typography → `FontSize::Rem`）、壳层 `RemSize` 资源。

## ② 两段式上下文主题（`ThemeContext`、`ThemeToken`→`SemanticToken`、`UiTheme`）

- 颜色查找走官方两段式：`ThemeToken`（本项目命名，`mf.*`）→ `SemanticToken` → `Color`；`UiTheme(ThemeProps)` 由 `UiPalette` 按皮肤构建（palette 仍是解析器与唯一 builder），**换肤 = 覆写 `UiTheme` 资源**，由官方 `update_theme` 系统重绘。
- 绘制组件用官方 `ThemeBackgroundColor`/`ThemeBorderColor`/`ThemeTextColor`（+`ThemedText`），不再手写 compare-and-set 重贴系统。
- 悬浮/浮层容器（modal、菜单、popover、下拉、toast、命令面板）根必须声明 `ThemeContext(SurfaceLevel::Floating)`；卡片/分组按 tonal ladder 映射 `Higher`/`Highest`；窗口底座 `Base`。四档是显式枚举，禁止 bool 表达"是否浮层"。
- 数据驱动的语义色（时延带、生命周期）映射到 token（success/warning/danger…）后仍走 Theme 组件，系统只换 token 不算颜色。
- 落点：`bevy_feathers` feature + `FeathersCorePlugin`（仅借主题框架，不采用其样式控件）；`W/palette.rs` 构建 `ThemeProps`；各悬浮层根声明 `ThemeContext`。

## ③ headless 标签页（`TabList`/`Tab`/`SelectedTab`）

- 一切"标签页/分段选择"语义必须用 `bevy_ui_widgets` 的 `TabList { orientation, activation }` + `Tab` + `SelectedTab`：方向键/Home/End/Enter/Space、roving tabindex、`TabList`/`Tab` a11y 角色、禁用传播全部由官方提供。
- 选择归属显式二选一：受控（外部写 `SelectedTab`，监听 `ValueChange<Option<Entity>>`）或自管（挂官方 `tablist_self_update` 观察者）；激活模式用 `TabActivation::{Manual,Automatic}` 显式声明。
- 视觉（滑轨胶囊、高亮）由官方派生状态 `Selected`/`SelectedTab` 驱动；禁止手写选择状态机、手写键盘导航。
- 落点：`W/tabs.rs` 重建于 TabList 之上；消费方 `U/localized_widgets.rs`/`pages/settings.rs` 及 rules/overview 的 chip/pill 选择组迁移。已删除旧入口：`TabSelectEvent`、`SegmentedControlValue` 自研状态机。

## ④ FixedNode 视口定位

- 视口级悬浮层（toast 栈、mini HUD、弹层 scrim、系统级覆盖）根必须挂 `bevy_ui::FixedNode`：相对相机视口定位，不继承父级布局/裁剪/变换。
- 禁止以"必须挂世界根、祖先不可裁剪"为前提的定位写法；悬浮层场景在任意宿主子树内挂载都必须正确。
- 落点：`U/toast.rs`、`U/mini_hud_shell.rs`、`W/popover.rs`、`W/adaptive_modal.rs` 浮层根。

## ⑤ 行内内容（`InlineBox`/`InlineImage`）

- 随文本流排版的图标、徽标、分隔符必须用 `bevy_text::InlineBox`（`InFlow`/`OutOfFlow` 显式枚举）或 `bevy_ui::InlineImage`；禁止用"图标节点+文本节点+column_gap"拼行导致折行错位与基线漂移。
- 行内图片统一走 `InlineImage`（tint 用 token 色），图片资产 owner 明确、原位复用 handle（章程"资源生命周期"节仍然适用）。
- 落点：`W/text.rs` 富文本行（badge 随文流动）、`U/shell_readout.rs` 状态点、logs 行内级别徽标。

## ⑥ 官方行为层优先（Dialog/ModalDialog/Popover/Menu）

- 对话框（模态/非模态）：`bevy_ui_widgets::Dialog`/`ModalDialog` + `RequestClose` + 官方 barrier/焦点圈闭/`DialogStack`。禁止手写 barrier 点击外关闭、Esc 拦截、焦点圈闭。
- 弹层定位：`bevy_ui_widgets::Popover`（`PopoverSide`/`PopoverAlign`/`PopoverPlacement` 自动翻转）。禁止手写视口翻转几何（combobox 下拉、context menu 定位迁到官方 placement）。
- 菜单：`bevy_ui_widgets::MenuPopup`/`MenuItem`/`MenuEvent`（官方 `MenuFocusState::{Opening,Open,Closed}` 显式状态机 + 方向键导航）。`W/menu.rs` 曾以"官方仅指针驱动"为由自建键盘导航；0.20 官方已支持键盘（`MenuLayout` + arrow keys），该理由失效，迁回官方。
- `bevy_feathers` 样式控件（`FeathersSelect`、`FeathersNumberInput`、`ListView`、`ColorInput`）暂不接入：它们绑定 feathers 视觉基线，与自研 token 体系冲突。触发条件：产品立项需要对应控件（如设置页拖拽步进 → `FeathersNumberInput`；主题编辑器 → `ColorInput`），接入时必须连 `UiTheme` 补齐其语义 token。此为已评审的取舍，不是"未验证"。
- 落点：`W/modal.rs`、`W/adaptive_modal.rs`、`U/command_palette.rs` → 官方 Dialog/ModalDialog；`W/popover.rs` → 官方 Popover；`W/menu.rs`、`W/context_menu.rs` → 官方 MenuPopup。已删除旧入口：`MenuNavEvent`/`MenuState` 自研循环、`ModalState.is_open: bool`。

## ⑦ 小件收敛（`bsn_list!`、`Ready`、胶囊圆角、`despawn_all`）

- **`bsn_list!`**：`bsn!` 场景里的多子实体列表（`Children [...]` 内联枚举）一律改用 `bsn_list!`（`--` 分隔、rustfmt 友好、`SceneList` 类型）；新场景函数返回行/项集合时签名用 `SceneList`。
- **`Ready` 事件**：依赖"子树完全生成"的挂载后初始化（投影首帧、焦点落点、测量）必须订阅 `bevy_scene::Ready` 实体事件，禁止依赖 spawn 同帧可见性。
- **胶囊圆角**：胶囊/药丸形一律 `BorderRadius::MAX`（圆形、半短边自适应），椭圆轮廓用 `CornerRadius::new(x, y)`/`MAX_ELLIPTICAL`；禁止手算 `px(高度/2)` 冒充胶囊（rem 缩放后失真）。`radius::PILL` 数值 token 保留给非自适应场景。
- **`despawn_all::<F>`**：按标记批量退役（toast 栈、HUD、沙盒）用 `commands.despawn_all::<With<标记>>()`；清子树用 `despawn_children()`；禁止手写遍历 `Children` 逐个 despawn 的循环。

## 已评审的本地交互例外：ScrollArea 触摸拖动

- 锁定的 Bevy 0.20.0 `bevy_ui_widgets::ScrollAreaPlugin` 提供 `PointerScroll` 与 `ScrollIntoView` 行为，没有将 `PointerDrag` 映射到 `ScrollPosition` 的行为；因此 touch swipe 必须由 widgets 层补足，继续复用官方 `ScrollArea`、`Overflow::scroll_y()` 和 `ScrollPosition`。
- Picking 在同一实体持续悬停时，拖动释放仍可能产生低层 `PointerClick`；`PointerDragStart` 会清除此指针/按键的 tap 资格，保证拖动列表行不触发按钮。原始 `PointerAction::Cancel` 也要直接清理手势、滚动和按下控件，因为取消时可能没有可供 Picking 命中的实体。
- 该适配只消费完整的 `DragStart → Drag → DragEnd/Cancel` 序列；`ScrollMotion::{Direct,Coasting}`、`ScrollMotionPreference`、`ScrollVelocitySampling`、`SoftKeyboardState::{Closed,Open}` 与 `PointerGesture::{Pressed,Dragging}` 显式表达状态，不从缺失事件或布尔组合推断状态。它负责坐标尺度映射、按当前内容尺寸夹紧、减少动效与有界惯性；滚轮行为仍由官方控件负责。
- 复审触发条件：上游提供同时覆盖 touch drag、点击仲裁和原始取消清理的官方 API 后，迁移到该 API，并在同一切片移除本地状态、观察者和被取代的回归路径。
