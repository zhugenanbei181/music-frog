# Bevy 0.20 UI 能力规范

> 层级：L3
> 范围：Bevy 0.20 UI 能力在 `infiltrator-bevy-ui` 与 `infiltrator-bevy-widgets` 中的实现约束。

本文是 Bevy 0.20 UI 能力在本项目的**唯一权威使用规范**。所有 `infiltrator-bevy-ui`、`infiltrator-bevy-widgets` 新代码必须遵守；违反任意一条的代码不允许合入。静态守卫的覆盖范围以实际注册的规则为准，未被机器检查的要求仍须由回归和验收证明。

## 总纲

1. **官方 headless 优先**：交互行为（选择、焦点、键盘导航、a11y 语义）必须来自 `bevy_ui_widgets` headless 控件或 `bevy_feathers` 主题/控件框架；自建交互状态机是例外，必须在本文件记录理由与复审触发条件。
2. **显式状态化**：凡可用有限状态表达的 UI 状态（开关、层级、表单形态、选中态）一律用显式枚举/组件（如 `SurfaceLevel`、`TabActivation`、`SelectedTab`、`MenuFocusState`），禁止用散布的 `bool` 字段加 if/else 推断状态。
3. **token 单一权威**：颜色、字号、间距只有一个来源（widgets crate 的 `theme`/`palette`/`text`），UI 层不得出现颜色/字号字面量。
4. 本文件"落点"一节列出每项能力对应的已删除旧入口，回归即守卫失败。

## 强制化台账（2026-10-20 实测）

本节是本规范**执行状态的单一事实源**：每项能力当前由机器强制还是仅靠人工评审、存量欠账多少、由哪个台账条目收口。静态守卫的覆盖范围以实际注册的规则为准；下表"强制方式"列写"人工评审"的条目，在守卫落地前不得宣称"已强制"。

| 能力 | 强制方式 | 当前实测 | 缺口 / 收口条目 |
| --- | --- | --- | --- |
| 场景结构（`bsn!` 唯一入口、ECS 访问边界） | 机器：`scripts/quality/bevy_bsn_guard.py`（BEVY-BSN-001～006、BEVY-ECS-007～009），注册于 `scripts/quality/check-structure.sh` 与 CI | 两个 crate 生产树 349 文件零违例：结构原语、手动挂子 API、直接 spawn、临时结构值、裸 `ChildOf` 重挂、`World`/`App::world()` 访问与 lint 抑制全部被拒 | 无，本口径已闭合 |
| ① rem/em 尺寸通道 | 机器：产品守卫 `scripts/quality/bevy_rem_guard.py`（`BEVY-REM-001`～`005`：禁命名/派生 px 值越出本文①物理白名单、禁硬编码 px 字面量、禁非 rem 字号、禁字面量 `em`、禁未命名 rem 尺寸），已注册 `scripts/quality/check-structure.sh` 与 CI；共享包场景门禁 `scripts/quality/bevy_scene_gate.py`（`UIK-REM-001/002`）为上游镜像（advisory） | **已强制且存量清零**：生产树 180 处非白名单 px 全部迁到 rem 通道（字面量 132 + 命名 48），胶囊圆角改 `BorderRadius::MAX`（29 处），一次性布局尺寸改角色命名的文件内常量（70 余处）；字号通道 `FontSize::Px` 0 处；权威台账 `scripts/quality/bevy_rem_inventory.json` 归零（`{"debt": {}}`），白名单外新增即失败 | 字号档位的真实像素回执（`INFILTRATOR_BEVY_FONT_SCALE` 两档截图）由 `BEVYCAP-001.2` 的待验证项承担 |
| ② 两段式上下文主题 | 机器：`BEVY-BSN-011`（浮层 owner 模块必须声明 `ThemeContext(SurfaceLevel::Floating)`；owner 由标记定义自动推导，不写死文件表） | 九个浮层 owner（modal / adaptive modal / 菜单 / popover / combobox / 上下文菜单 / mini HUD / toast 栈 / 命令面板）全部声明 | 无 |
| ③ headless 标签页 | 人工评审 | 已迁到官方 `TabList`/`Tab`/`SelectedTab`；无守卫（判定需语义规则，机械不可靠） | 保持评审口径 |
| ④ `FixedNode` 视口定位 | 机器：视口级浮层根必须挂官方 `FixedNode`（与 ② 同批补齐，9 个浮层 owner 全部挂载） | 官方 `FixedNode` 已挂在全部九个浮层 owner | 无 |
| ⑤ 行内内容 | 人工评审 | 只迁到 `InlineImage`（`bevy_ui::widget::InlineImage`，6 处：状态点与图标）；`bevy_text::InlineBox` **直接使用 0 处**，行内文字/徽标仍随文本流动这条语义未落地；无守卫（判定需语义规则） | 行内文字内容迁到 `InlineBox`，或按 ⑤ 的落点清单逐条证明不需要 |
| ⑥ 官方行为层优先 | 机器：全部模态面（共享 modal、命令面板、13 个产品模态页、adaptive modal）挂载官方 `ModalDialogBarrier`／`ModalDialog`／`RequestClose`，barrier 点击外关闭、焦点圈闭（`Dialog` → `TabGroup { modal: true }`）、`DialogStack` 与 Esc 拦截由 Bevy 提供；打开时把输入焦点交接进对话框（`TabIndex` + `AcquireFocus`），关闭时释放，手写 Esc 分支已删除；模态阶段为 typed `ModalStage{Hidden,Open,Dismissed}`（旧 bool 对已消失）；**menu 层已迁回官方**（`infiltrator-bevy-widgets/src/menu.rs` 用官方 `MenuButton`/`MenuPopup`/`MenuItem`/`MenuEvent`/`MenuFocusState`；本地 `MenuState`/`MenuNav`/`MenuNavEvent`/`Menu`/`MenuScrim`/`menu_overlay_scene`/`advance_menus` 与 `context_menu.rs` 的 `ContextMenuPlacement`/`ContextMenuState.is_open` 已删除）；无守卫（判定需语义规则）；圆角角色按契约阶梯互斥裁决：**居中**模态对话框/居中命令面板 = `MODAL`(24)，面板式对话框卡/浮层面板 = `CARD`(16)，底部抽屉 = `BOTTOM_SHEET`(28)，旧 `DIALOG` 档已退役（阶梯不得有两种读法） | 无 |
| ⑦ 小件收敛 | 机器：`scripts/quality/bevy_bsn_guard.py`（`BEVY-BSN-008` 列表场景必须 `bsn_list!`、`BEVY-BSN-009` 批量退役必须 `despawn_all`、`BEVY-BSN-010` 胶囊必须 `BorderRadius::MAX`、`BEVY-BSN-007` `template_value` 硬零）+ 评审口径（`Ready` 事件语义无法词法判定） | 6 处真实列表场景改 `bsn_list!` + `SceneList`；1 处挂载后初始化改订阅 `bevy_scene::Ready`；1 处批量退役改 `despawn_all`；胶囊圆角 29 处全量 `BorderRadius::MAX`；固定 2–3 个手写子项的静态 `Children [ ... ]` 明确保留（非列表场景） | 真实交互与像素验收随本批截图批次 |
| ⑧ 跨端条件编译与诚实能力 | 机器：`scripts/quality/mobile-cfg-discipline-guard.py`（禁平台排除通用类型、禁公开函数悬垂签名、移动端禁谎报桌面窗口能力） | 全仓跨端 UI 生产树零违例：`UiPalette`/`Token` 无条件通用，`chrome.rs` Android 诚实声明 `Unsupported` 并按能力挂载 observers | 无，本口径已闭合 |

### 0.20 未采用能力台账（2026-10-21 实测）

本节是 **0.20 提供而本仓尚未使用** 的能力的单一事实源。实测方式：在 `infiltrator-bevy-ui/src` 与
`infiltrator-bevy-widgets/src` 两个生产树上按官方符号名计数，0 处即"未采用"。未采用必须是**有记录的取舍**，
不得是"以为用了"。

| 编号 | 0.20 能力 | 实测 | 事实与处置 |
| --- | --- | --- | --- |
| SCROLL-01 | `bevy_ui_widgets::Scrollbar` / `ScrollbarThumb` / `ScrollbarPlugin` | 0 处 | 本仓只挂了 `ScrollAreaPlugin`（官方滚动容器），**没有滚动条**：既未用官方滑块，也无自有滑块；规则列表只有一行文字页码指示器。处置：接官方 `Scrollbar`+`ScrollbarThumb`（滑块需 `Scrollbar::new(target, orientation, min_thumb_length)`），并把列表容器的滚动位置与滑块绑定；几何比例与交互阶段严格对齐 `infiltrator-contract::scrollbar`（`ScrollbarMetrics` / `ScrollbarPhase`）跨端单一权威。 |
| SCROLL-02 | `bevy_ui_widgets::ScrollIntoView`（事件） | 0 处 | 官方 `ScrollAreaPlugin` 已注册 `on_scroll_into_view` 观察者，但本仓**从不触发该事件**，而是在 `infiltrator-bevy-widgets/src/list.rs` 自算偏移（`scroll_to_item`）。处置：改为触发官方 `ScrollIntoView`，删除自算偏移路径。 |
| MENU-01 | `bevy_ui_widgets::MenuPopup` / `MenuItem` / `MenuFocusState` / `MenuLayout` | **已采用**（2026-10-21 迁完） | 官方 `MenuButton`/`MenuPopup`/`MenuItem`/`MenuEvent`/`MenuFocusState` 接管打开/关闭/方向键导航/焦点圈闭；本地菜单状态机与手写视口翻转（`ContextMenuPlacement`）已删除。宿主需具备 `InputDispatchPlugin`（键盘→焦点实体）、`PointerFocusPlugin`（点外关闭）与可见性传播（`VisibilityPlugin` + mesh asset store），否则官方焦点账本够不到条目。 |
| TEXT-01 | `bevy_text::InlineBox`（`InFlow`/`OutOfFlow`） | 0 处 | 只用了 `InlineImage`（图片侧）。行内文字/徽标随文流动这条语义未落地，见台账 ⑤。 |
| NAV-01 | `bevy_ui::auto_directional_navigation` | **已采用**（2026-10-21） | 官方 `DirectionalNavigationPlugin` + 仓库配置 + 箭头手势已接（`crates/infiltrator-bevy-widgets/src/directional_nav.rs`），且官方 `InputFocus` 已成为唯一焦点真源（本地空间搜索与影子焦点账本已删除，控件补官方 `TabIndex`）。未证明层：官方**自动**空间搜索无法 headless 演练（相机字段 crate 私有），headless 证据走官方手动导航图；焦点环跟随焦点无像素证据（捕获台不发指针/键盘）。 |
| RENDER-01 | `bevy_ui::gradients`（`LinearGradient`/`RadialGradient`） | 0 处 | 0.20 的 UI 渐变未接入。处置：纯视觉增强，按需立项（当前 token 体系无渐变语义 token）。 |
| RENDER-02 | `bevy_ui::stack`（CSS 式层叠） | 0 处 | 0.20 的层叠原语未接入；本仓的浮层层级靠显式 `SurfaceLevel` 枚举与 `FixedNode` 表达。处置：按需立项。 |
| EXP-01 | `bevy_ui::experimental::ghost_hierarchy` | 0 处 | 0.20 的实验性幽灵层级未接入（上游标 experimental）。处置：上游转正后评估。 |
| LABEL-01 | `bevy_ui::widget::Label` | 0 处 | 文字一律走 `bevy::ui::widget::Text` + 本仓 role 排版（token 驱动字号/行高）。**这是有意取舍**（官方 Label 绑定 feathers 式排版），不是遗漏。 |

### 产品侧 rem 守卫（`BEVYCAP-001.1` 交付物）

- **执行器**：`scripts/quality/bevy_rem_guard.py`（规则 `BEVY-REM-001`～`004`），已注册进 `scripts/quality/check-structure.sh` 与 CI；规则回归在 `scripts/quality/tests/test_bevy_rem_guard.py`。
- **职责边界**：共享包场景门禁（`UIK-REM-001/002`）负责**硬编码 px 字面量**与 `FontSize::Px`；本守卫负责字面量看不见的另一半——**命名/派生 px 值**（`Val::Px(gap)`、`Val::Px(palette.control_radius_px)`）越出本文①物理白名单即失败，并额外禁止字面量 `Val::Em` 与非 `FontSize::Rem` 字号。两段互补即本文①的闭集白名单，两份台账互不重叠（字面量一份、命名值一份），不存在同一站点的双基线。
- **存量台账**：`scripts/quality/bevy_rem_inventory.json` 登记当前 32 个表达式 / 52 处命名 px 存量；台账**只允许缩小**（`--prune-inventory` 拒绝登记新债），白名单外新增即失败。
- **边界说明**：本守卫不判定视觉像素是否正确，只判定尺寸是否走对通道；字号档位下的真实缩放仍需截图回执（`BEVYCAP-001.2`）。

### 场景律法覆盖差表（`BEVYCAP-003.1` 裁决：产品守卫 = 唯一阻塞执行器，且是上游律法的严格超集）

共享包 `zhuge-ui-kit` 0.1.0 的 README 明确「products choose rem/em/px and native scaling policy explicitly」——**单位与构造策略属产品权威**；上游律法是跨产品共享的**下限**，产品律法在其之上收紧。因此：产品自有守卫是唯一阻塞执行器并覆盖上游全部规则，共享包场景门禁降为**上游律法镜像（advisory）**，只报差集、不阻塞、不作产品放行依据，也不存在两份台账同时放行同一站点。

| 事实 | 产品守卫（阻塞） | 共享包门禁（镜像） | 裁决 |
| --- | --- | --- | --- |
| 结构原语/遗留 bundle 只能在 `bsn!` 内 | `BEVY-BSN-001` | `UIK-BSN-001` | 同义，产品阻塞 |
| 手动挂子 API 禁止 | `BEVY-BSN-002`（另含 `add_related`/`insert_related` 与 spawner 类型） | `UIK-BSN-002`（子集） | 产品更严，产品阻塞 |
| 直接 spawn 禁止（`Camera2d`/`Observer::new` 豁免） | `BEVY-BSN-003` | `UIK-BSN-003` | 同义，产品阻塞 |
| `bsn!` 括号失衡 | `BEVY-BSN-004` | `UIK-BSN-004` | 同义，产品阻塞 |
| `template_value` 硬零 | `BEVY-BSN-007` | `UIK-BSN-005` | 同义，产品阻塞（对齐实现） |
| 结构临时值（`Node::default()`/`Text::new`/`Children::from_*`） | `BEVY-BSN-005` | `UIK-BSN-006` | 同义，产品阻塞 |
| 裸 `ChildOf` 重挂 | `BEVY-BSN-006` | 未覆盖 | 产品独占 |
| ECS 访问边界与 lint 抑制 | `BEVY-ECS-007`～`009` | 未覆盖 | 产品独占 |
| 硬编码 px 字面量 | `BEVY-REM-001` | `UIK-REM-001` | 同义，产品阻塞 |
| `FontSize::Px` | `BEVY-REM-002` | `UIK-REM-002` | 同义，产品阻塞 |
| 视口相对字号（`Vw`/`Vh`/`VMin`/`VMax`） | `BEVY-REM-002` | 未覆盖 | 产品独占 |
| 命名/派生 px 值越出物理白名单 | `BEVY-REM-001` | 未覆盖（上游允许命名 token） | 产品独占 |
| 字面量 `Val::Em` | `BEVY-REM-003` | 未覆盖 | 产品独占 |
| 列表场景必须用 `bsn_list!` | `BEVY-BSN-008` | 未覆盖 | 产品独占（⑦） |
| 批量退役必须用 `despawn_all` | `BEVY-BSN-009` | 未覆盖 | 产品独占（⑦） |
| 胶囊圆角必须 `BorderRadius::MAX` | `BEVY-BSN-010` | 未覆盖 | 产品独占（⑦） |
| 挂载后初始化订阅 `bevy_scene::Ready` | 不可机械判定 | 未覆盖 | 评审口径（`Ready` 事件语义无法词法判定） |

**台账口径**：权威台账唯一——`scripts/quality/bevy_rem_inventory.json`（产品库存，只允许缩小，`--prune-inventory` 拒绝登记新债）；共享包棘轮 `scripts/quality/bevy_scene_ratchet.json` 归其 owner，是上游镜像，只读参考，不作为产品的放行依据。

**硬结论：**"`bsn!` 已强制"成立（机器红线、CI 生效）；**"rem 已强制且存量已清零"成立**——产品守卫是唯一阻塞执行器且覆盖共享包律法全部规则，180 处非白名单 px 全部迁到 rem 通道，存量台账归零，白名单外新增即失败；**证据层已闭环**：字号档位下的真实缩放已由像素回执证明（`INFILTRATOR_BEVY_FONT_SCALE=1.0`/`1.5` 同场景真实截图逐像素比对 46.2% 像素变化、差异覆盖整帧），静态守卫与 headless 结果不能替代它，但本次两者同时具备。

## ① rem/em 动态字号与尺寸（`Val::Rem`/`Val::Em`、`FontSize::Rem`、`RemSize`/`EmSize`）

- `RemSize` 资源是**全局根字号，也是全 UI 唯一缩放入口**：壳层按用户字号/密度偏好写入，换档不重建树。
- 根基准 `REM_BASE = 15.0`（= `type_scale::BODY`）。文字字号一律 `FontSize::Rem(px_at_baseline / REM_BASE)`；间距、半径、控件高度等尺寸 token 一律 `Val::Rem`（widgets `theme` 提供换算 helper）。
- `Val::Em` 仅用于**相对所在文字字号**的尺寸（行内图标、随文字缩放的内嵌元素）；节点无 `TextFont` 时不得用 em。
- **px 例外白名单**（只能这些场景写 px）：1px 发丝线与 2px 光标（`metrics::HAIRLINE`/`CARET_WIDTH`）、视口断点（`breakpoint`，视口语义而非字号语义）、最小触控目标（`MIN_TOUCH_TARGET`，物理可达性下限）、inset 物理像素换算结果、GPU/测量代码。
- 禁止：新增 `FontSize::Px` 写入、新增把字号/间距换算散落到页面的代码。
- 落点：`W/theme.rs`（type_scale/space/radius/metrics 输出 rem）、`W/text.rs`（role_typography → `FontSize::Rem`）、壳层 `RemSize` 资源。
- 强制状态：见[强制化台账](#强制化台账2026-10-20-实测)与[覆盖差表](#场景律法覆盖差表bevycap-0031-裁决产品守卫--唯一阻塞执行器且是上游律法的严格超集)。产品守卫 `scripts/quality/bevy_rem_guard.py` 是唯一阻塞执行器：命名/派生 px 值、硬编码 px 字面量、非 rem 字号、字面量 `em` 与未命名 rem 尺寸全部拦截（存量台账 `scripts/quality/bevy_rem_inventory.json` 已归零）；共享包场景门禁 `scripts/quality/bevy_scene_gate.py` 为上游镜像（advisory）。180 处存量迁移已由 `BEVYCAP-001.2` 收口，字号档位的像素回执是唯一未闭环的证据项。

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
- 菜单：`bevy_ui_widgets::MenuPopup`/`MenuItem`/`MenuEvent`（官方 `MenuFocusState::{Opening,Open,Closed}` 显式状态机 + 方向键导航）。`W/menu.rs` 曾以"官方仅指针驱动"为由自建键盘导航；0.20 官方已支持键盘（`MenuLayout` + arrow keys），该理由失效，**已于 2026-10-21 迁回官方**，本地 `MenuState`/`MenuNavEvent` 导航状态机已退役。宿主侧保留的 `MenuOutcome` 只是宿主面向的类型化结果，其来源是官方 `MenuEvent`，不再自带第二份导航状态。
- `bevy_feathers` 样式控件（`FeathersSelect`、`FeathersNumberInput`、`ListView`、`ColorInput`）暂不接入：它们绑定 feathers 视觉基线，与自研 token 体系冲突。触发条件：产品立项需要对应控件（如设置页拖拽步进 → `FeathersNumberInput`；主题编辑器 → `ColorInput`），接入时必须连 `UiTheme` 补齐其语义 token。此为已评审的取舍，不是"未验证"。
- 落点：`W/modal.rs`、`W/adaptive_modal.rs`、`U/command_palette.rs` → 官方 Dialog/ModalDialog；`W/popover.rs` → 官方 Popover；`W/menu.rs`、`W/context_menu.rs` → 官方 MenuPopup。已删除旧入口：`MenuNavEvent`/`MenuState` 自研循环、`ModalState.is_open: bool`。

## ⑦ 小件收敛（`bsn_list!`、`Ready`、胶囊圆角、`despawn_all`）

- **`bsn_list!`**：`bsn!` 场景里的多子实体列表（`Children [...]` 内联枚举）一律改用 `bsn_list!`（`--` 分隔、rustfmt 友好、`SceneList` 类型）；新场景函数返回行/项集合时签名用 `SceneList`。
- **`Ready` 事件**：依赖"子树完全生成"的挂载后初始化（投影首帧、焦点落点、测量）必须订阅 `bevy_scene::Ready` 实体事件，禁止依赖 spawn 同帧可见性。
- **胶囊圆角**：胶囊/药丸形一律 `BorderRadius::MAX`（圆形、半短边自适应），椭圆轮廓用 `CornerRadius::new(x, y)`/`MAX_ELLIPTICAL`；禁止手算 `px(高度/2)` 冒充胶囊（rem 缩放后失真）。`radius::PILL` 数值 token 保留给非自适应场景。
- **`despawn_all::<F>`**：按标记批量退役（toast 栈、HUD、沙盒）用 `commands.despawn_all::<With<标记>>()`；清子树用 `despawn_children()`；禁止手写遍历 `Children` 逐个 despawn 的循环。
- 强制状态：见[强制化台账](#强制化台账2026-10-20-实测)与[覆盖差表](#场景律法覆盖差表bevycap-0031-裁决产品守卫--唯一阻塞执行器且是上游律法的严格超集)。机器规则：`BEVY-BSN-008`（列表场景必须 `bsn_list!`）、`BEVY-BSN-009`（批量退役必须 `despawn_all`）、`BEVY-BSN-010`（胶囊圆角必须 `BorderRadius::MAX`）；`Ready` 事件语义无法词法判定，保持**评审口径**（人工核验挂载后初始化是否订阅 `Ready`）。

## 已评审的本地交互例外：ScrollArea 触摸拖动

- 锁定的 Bevy 0.20.1 `bevy_ui_widgets::ScrollAreaPlugin` 提供 `PointerScroll` 与 `ScrollIntoView` 行为，没有将 `PointerDrag` 映射到 `ScrollPosition` 的行为；因此 touch swipe 必须由 widgets 层补足，继续复用官方 `ScrollArea`、`Overflow::scroll_y()` 和 `ScrollPosition`。
- Picking 在同一实体持续悬停时，拖动释放仍可能产生低层 `PointerClick`；`PointerDragStart` 会清除此指针/按键的 tap 资格，保证拖动列表行不触发按钮。原始 `PointerAction::Cancel` 也要直接清理手势、滚动和按下控件，因为取消时可能没有可供 Picking 命中的实体。
- 该适配只消费完整的 `DragStart → Drag → DragEnd/Cancel` 序列；`ScrollMotion::{Direct,Coasting}`、`ScrollMotionPreference`、`ScrollVelocitySampling`、`SoftKeyboardState::{Closed,Open}` 与 `PointerGesture::{Pressed,Dragging}` 显式表达状态，不从缺失事件或布尔组合推断状态。它负责坐标尺度映射、按当前内容尺寸夹紧、减少动效与有界惯性；滚轮行为仍由官方控件负责。
- 复审触发条件：上游提供同时覆盖 touch drag、点击仲裁和原始取消清理的官方 API 后，迁移到该 API，并在同一切片移除本地状态、观察者和被取代的回归路径。

## ⑧ 跨端测试分层与条件编译纪律

对齐 Bevy 0.20 自动化测试标准，UI 验证体系分为三层独立通道，不得以一类替代另一类：

1. **通道一：状态与 View Model（无渲染无头 ECS 测试）**：
   - 入口为 `tests/headless/`，基于 `MinimalPlugins + AssetPlugin + ScenePlugin`，不申请 GPU device，不创建 OS 窗口。
   - 职责：验证纯数据投影、ECS 实体关系挂载、BSN 结构生成、输入事件处理（通过 `PointerPress`/`PointerClick`/`Activate` 真实派发）、状态机迁移（如 `WindowChromeReport`）。
   - 速度：数十毫秒完成整组用例，提供极速反馈。
2. **通道二：真实布局与文本度量边界防范**：
   - 注意：无渲染无头测试由于未加载字体栅格引擎（HarfBuzz/Swash），无法测量真实字形物理尺寸与换行折行。
   - 规则：关键卡片与列表容器必须显式定义裁剪（`Overflow::clip()`）或滚动容器，防范字形撑爆；不得单凭无头测试通过宣称文本不溢出。
3. **通道三：真实像素与物理交互取证**：
   - 依赖真实进程在真实或虚拟显示合成器（Wayland/X11）下的渲染回执（`scripts/parity/visual_receipts.py`）。
   - 强制绑定进程 PID、二进制摘要 SHA-256、ROI 交互包围盒与有效像素亮度标准差（`luma_stddev > 2`），彻底杜绝空白帧假阳性。
4. **时间域受控推进**：
   - 涉及定时器、双击阈值判定（`CHROME_DOUBLE_CLICK_INTERVAL_MS`）或过渡动画（如 `BackdropBlur` 渐变、`ModernCard` lift）的测试，提倡注入受控时间步进（`TimeUpdateStrategy`），禁止依赖无参 `app.update()` 假定时间推进。
5. **跨平台条件编译纪律（机器红线）**：
   - 严禁对全平台通用主题与契约类型（`UiPalette`、`Theme`、`Token`、`WindowChrome` 等）施加 `#[cfg(not(target_os = "android"))]` 排除。
   - 跨端暴露的公开 API 函数签名中，所有参数与返回值类型在全平台必须具备完整引用。
   - 移动端宿主能力必须诚实声明：Android 等无桌面窗口平台必须报告 `WindowChromeSupport::Unsupported`，严禁谎报 `Hosted`。
   - 机器守卫：`scripts/quality/mobile-cfg-discipline-guard.py` 纳入 CI 结构门禁强制执行。
