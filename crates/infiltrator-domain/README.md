# infiltrator-domain

> 层级：L3

纯领域模型与算法：配置语义、连接/DNS/规则状态、路由、快照和校验。

不依赖 Tokio、网络、文件系统、OS API 或 UI。持久化和副作用由 application 通过 ports 调用。

contract 快照接入领域归约的接缝在 `connection_snapshot_view` 与 `rule_snapshot_view`：开放连接与已关闭连接都实现 `ConnectionView`，因此关闭历史与开放列表共用同一套搜索、排序与聚合归约；已关闭记录没有的事实（进程路径、端点）回答空值而不是编造。
