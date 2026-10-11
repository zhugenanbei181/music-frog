# infiltrator-bevy-widgets

> 层级：L3

业务无关的 Bevy 控件、场景函数、主题、输入、列表和图表组件，供 Bevy UI 产品组合。

通用 surface 与 headless button 场景树来自共享仓库 `libs/zhuge-ui-kit` 的
`zhuge-ui-kit-bevy`；原文高亮片段归纯 Rust `zhuge-ui-core`（同一仓库）。
本包保留产品主题、Feathers 投影、字体/图标角色和本地化适配，直接导入公共能力 owner。
RenderPlugin 已注册的 Shader 存储不得重复初始化，避免使在途资源句柄失效。

不依赖 Mihomo、application 或具体页面。静态结构使用 `bsn!`，运行态由受限 ECS 系统原位更新；shader、handle 和实体生命周期遵循 [Bevy UI 章程](../infiltrator-bevy-ui/docs/BEVY_UI_FRONTEND.md)。

图标与字体从共享视觉资源目录嵌入；稳定 ID、SVG/PNG/ICO/字体格式变体和许可证来源由同级 crate [infiltrator-shared](../infiltrator-shared/README.md) 的资源 catalog 统一维护。

通用 UI 功能先扩展对应 `zhuge-ui-*` owner，再在本 crate 接入；不得复制共享实现。
固定行高视口范围、overscan 与滚动边界，以及变高列表的前缀索引与可见窗口，
由 `zhuge-ui-core::virtual_list` 唯一计算。本 crate 只把共享窗口映射为 ECS 列表
所需的 spacer 投影；实体池和业务行身份仍属于 Bevy 适配层。触摸拖动与有界惯性
统一由原生 `ScrollArea` adapter 管理，虚拟列表只依据当前滚动位置计算窗口，不另设
fling/physics 状态。行高测量和固定行高修改返回明确枚举。更新共享依赖时同时验证
Iced/Bevy 消费者的可见范围与边界行为。
