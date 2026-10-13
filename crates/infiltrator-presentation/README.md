# infiltrator-presentation

> 层级：L3

持有 Iced、Bevy、Slint 等产品表面共用的纯快照到呈现投影和 typed reducer，包括 Doctor 报告/失败状态与文案、profile metadata、订阅计划/过滤/配额、typed Failure 与编辑会话 copy、watchdog 状态、网络与规则读数、配置编辑提示、流量信息、RoutingIntent 阶段、规则草稿/trace/statistics 阶段、日志/脚本导出与快照恢复工作台阶段，以及控制器原始日志到展示行和页面状态的解析。所有互斥工作台阶段用显式 enum 表达；本 crate 只构造共享命令意图，不执行命令。只依赖 contract、domain、shared 与无副作用格式工具；不依赖 application、ports、composition 或 UI toolkit，不持有运行时或访问 I/O。Profile 只读预览经 `profile_preview` 对 YAML credential 字段进行结构化脱敏；来源文档保持不变。

新建跨表面的纯展示归约时先检查这里是否已有相同事实；Core owner、业务事务和平台副作用仍由各自 owner 实现。
