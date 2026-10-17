# lurker-ui-presentation

> 层级：L3

跨引擎、跨平台、跨 UI 的中性呈现内核。本 crate 只持已经从产品 contract 中剥离出来的纯呈现逻辑：typed reducer 阶段、视口策略和本地化 key 端口。

- `copy`：`TextCatalog` 端口与单次占位符插值。内核只拥有稳定的 key，绝不内置任何文案、语言选择或 catalog 数据；各产品对自己的 catalog 实现该 trait。
- DNS 泄漏探测阶段：待定/失败/完成阶段与相关化 receipt，禁止用快照完成写入。
- typed Failure 投影：把 typed `Failure` 投影为闭集 key，并按 `TextCatalog` 渲染标题与原始诊断详情；原始 adapter 文本始终当作不透明诊断，不再被解释成 key 或参数。
- 日志视口跟随策略：锁定视口从不停止日志摄取。
- 代理分组顺序草稿：库存围栏、上下移动/重置操作和带 token 关联的回执。
- RoutingIntent 改变阶段：观察/待定/失败阶段与乱序回执撤销。

本 crate 的所有状态都用显式 enum 表达互斥阶段；非法迁移由 typed transition 与穷尽匹配排除。依赖面只有 `lurker-core-contract` 与 `std`：不依赖任何 `infiltrator-*` 产品 crate、engine adapter、异步 runtime、传输层或 UI toolkit，因此可发布且可被任意表面复用。仍依赖产品 contract 的呈现投影不在本 crate，由各产品 presentation 层持有。
