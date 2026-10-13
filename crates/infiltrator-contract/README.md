# infiltrator-contract

> 层级：L2

定义跨 Iced、Bevy、Android、Admin 和 Rust use case 共用的命令、结果、快照、事件、能力与失败类型。

原文搜索片段使用纯 Rust `zhuge-ui-core::text::SearchTextRun`，序列化字段仍为
`text` 与 `highlighted`；本包不另建同名类型或转发地址。

保持 transport、runtime、平台和 UI toolkit 中立；不得放入 Mihomo wire DTO、OS 类型或前端分支。由本 crate 持有的双端语义与布局规则见 [本地文档](docs/README.md)。

路由策略通过 `RoutingIntent` 表达共同语义；engine-defined 扩展携带显式 owner 与经过校验的 ID，adapter 可以按类型拒绝。

shell 读数使用 `ShellObservation<T>` 区分 `Unobserved`、`Current(T)`、`CurrentEmpty` 与 `Stale(T)`。序列化采用带 `state` 标记的枚举；跨进程或外部传输的载荷形状变更必须同步更新对应协议版本。

连接速率使用 `ConnectionRateObservation::{Unobserved, Measured}` 表示采样是否具备有效的双快照计数窗口；不以 `bool` 搭配零值速率表达采样状态。

Core 资源使用 [`ResourceAvailability<T>` 与 owner-scoped `CoreResourceSnapshot`](src/resources.rs)：缺样、可用、不支持和失败均为显式状态；GC 只有携带同 scope 的前后内存读回时才可表示为 `Collected`。
