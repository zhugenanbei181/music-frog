# Bevy UI 实现规则

> 层级：L3

本页只记录 Bevy 专属实现边界；crate 职责见 [README.md](../README.md)，Android 宿主见 [Android 文档](../../../android/docs/BEVY_ANDROID_PRODUCT.md)。

## 场景与 ECS

> Bevy 0.20 UI 能力（rem/em、ThemeContext、headless 控件、FixedNode、InlineBox、bsn_list!/Ready/despawn_all）的使用规范见 [BEVY_020_UI_CAPABILITIES.md](BEVY_020_UI_CAPABILITIES.md)，本文与其冲突时以能力规范为准。

- 使用 `bsn!` 声明 UI 场景，通过受限系统更新实体；不在业务路径中命令式创建、拆解或重挂 UI 树。
- 结构与 ECS 口径由 `scripts/quality/bevy_bsn_guard.py` 机器强制（接入 `scripts/quality/check-structure.sh` 与 CI）；rem/em 尺寸通道由 `scripts/quality/bevy_rem_guard.py` 机器强制（禁硬编码 px 字面量、禁命名/派生 px 值越出物理白名单、禁字面量 `em`、禁非 rem 字号）；共享包场景门禁是上游律法差集探测器（advisory），⑦ 小件收敛目前仅人工评审，强制状态与收口条目见能力规范的「强制化台账」一节。
- 系统声明所需的 `Query`、资源、事件、`Commands` 与职责明确的 `SystemParam`。禁止透传整个 `World`、`App::world()`/`world_mut()`、`DeferredWorld`、独占系统和全局服务定位器。
- 插件在 `build` 中一次注册资源与观察者。页面根只标记身份；控件用受限 Observer 初始化。实体创建、退役和 SDK 状态同步通过明确的调度顺序与 `ApplyDeferred` 完成。
- 业务状态与投影由共享 owner 持有。UI 原位回放结果和输入状态，不复制业务事实、不因数据刷新重建整页；`infiltrator-bevy-widgets` 保持业务无关。

## 本地化与输入

- 文案统一来自 `infiltrator-shared`。语言变化原位更新文本和无障碍标签，保留实体、焦点、选择、IME 与未提交输入。
- 验收驱动实际 Bevy 控件事件；直接修改组件或状态变量不能证明用户输入、取消或禁用行为有效。
- Android NativeActivity 将系统栏、显示挖孔、系统手势区域和 IME 的 typed inset 观测送入 Bevy host feed。driver 注册当前 Bevy receiver 后同步读取 Activity 缓存作为 seed，后续只由 Activity inset callback 在值变化时送入报告；Activity generation 拒绝已退役实例的迟到报告，receiver 重建时替换旧通道。值以窗口物理像素携带真实 density；壳层先转换到 Bevy 逻辑坐标，再布局固定底栏。紧贴屏幕底边的硬编码偏移不构成安全区适配。

## Shader 与资源

- 项目自有 shader 使用 `.wesl` 模块，禁止在 Rust 中内嵌或拼接第二套 shader。修改时同步核对 Rust/GPU ABI、绑定、坐标和颜色空间，并在真实渲染组合中验证。
- 每项 GPU 资产都有明确 owner。内容更新原位复用 handle；静态帧不重复分配或上传；组件退役、页面离开或效果关闭时释放自己的引用，不删除其他 owner 的资源。
- `RenderPlugin` 与 `ImagePlugin` 已注册的 Shader/Image 仓库属于宿主；产品插件只为缺少对应宿主插件的 headless 组合注册仓库，不能重复 `init_asset` 回收现存句柄。CPU 测试验证保留句柄，原生捕获验证真实绑定维度。
- 可选效果保留清晰的基础回退。性能、显存与设备支持只依据目标产物的实际测量。

## 验证

结构守卫、headless 结果和静态截图都不能替代真实交互及像素验收；每项验收只记录当前产物实际具备的证据，缺少设备交互或像素证据时保持未验收。
