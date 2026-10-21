# infiltrator-presentation

> 层级：L3

持有 Iced、Bevy、Slint 等产品表面共用的纯快照到呈现投影和 typed reducer，包括 Doctor 报告/失败状态与文案、profile metadata、protocol field projection/staged input/import-save-readback phases、Mixin Studio copy、订阅计划/过滤/配额、typed Failure 与编辑会话 copy、watchdog 状态、网络与规则读数、配置编辑提示、流量信息、规则草稿/trace/statistics 阶段、runtime probe settings draft/save/readback 阶段、日志/脚本导出与快照恢复工作台阶段，以及控制器原始日志到展示行和页面状态的解析。已经与产品 contract 解耦的部分（RoutingIntent 改变阶段、代理分组顺序草稿/回执、DNS 泄漏探测阶段、日志视口跟随策略、`TextCatalog` key 端口）已迁至中性 [lurker-ui-presentation](../lurker-ui-presentation/README.md)，本 crate 只保留仍消费产品 contract 与 catalog 文案的薄适配。所有互斥工作台阶段用显式 enum 表达；本 crate 只构造共享命令意图，不执行命令。只依赖 contract、domain、shared 与无副作用格式工具；不依赖 application、ports、composition 或 UI toolkit，不持有运行时或访问 I/O。Profile 只读预览经 `profile_preview` 对 YAML credential 字段进行结构化脱敏；来源文档保持不变。

新建跨表面的纯展示归约时先检查这里是否已有相同事实；Core owner、业务事务和平台副作用仍由各自 owner 实现。

跨表面列表过滤只有一个 typed 查询：[`filter_query`](src/filter_query.rs) 的 `FilterQuery` 决定原始文本是空查询、自由文本还是延迟上界；代理页与规则页共用它，规则页只使用适用的自由文本变体，匹配规则留在各自的行事实旁。

设置页模型 [`settings_page_model`](src/settings_page_model.rs) 持有 LAN 共享与 PAC 的草稿：LAN 共享读回以 `LanSharingObservation` 表达“未观测”与“已观测”，缺省读回不表达成写入意图；PAC 草稿读回 owner 请求的 `bypass_lan` 与 `minify`，不再读成默认值。应用分流页的未观测模式有独立文案键（`app_routing_mode_unobserved`），不回落成默认模式。
