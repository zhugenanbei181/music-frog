# infiltrator-application

> 层级：L3

实现共享用例、命令编排和跨端业务事务。公开接口使用 contract 类型并经 ports 调用副作用；页面快照装配属于产品组合层，跨表面纯呈现投影由独立 presentation crate 统一提供。

本 crate 不选择 executor、不创建具体网络客户端，也不依赖 UI。异步调度由 composition 注入；一个业务语义只能有一个 owner。

Profile 用例只依赖活动 profile applier 和显式 `Offline` / `CoreOwner` access；网络用例通过 typed `PublicIpProbeRoute` 选择直连或已配置的 HTTP proxy，不把缺失 endpoint 当成直连。

原文搜索片段、Unicode/grapheme 边界与范围重放归纯 Rust `zhuge-ui-core`，
不引入 UI toolkit。产品搜索条件、拼音匹配与连接/日志业务投影继续由本层定义。

通用 UI 功能先扩展对应 `zhuge-ui-*` owner，再在本 crate 接入；不得复制共享实现。
产品适配保留业务职责，更新共享依赖时同时验证其他消费者和可见行为。

`ResourceApplication` 编排 scoped 资源采样、内存阈值与 GC 冷却；并发采样、scope 变化、Unsupported、失败及 GC 后读回应分别保持 typed 状态。

`RuleProviderRuntimeApplication` 负责 owner/revision fence、能力 admission 与 rule-provider readback；`GeoDatabaseApplication` 只将 Core 接受的异步触发建模为 `Accepted`，不报告数据库下载完成。
