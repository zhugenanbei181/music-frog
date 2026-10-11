# sync-engine

> 层级：L3

组合 `dav-client`、`indexer` 与 `state-store`，执行同步、比较、重试和冲突处理流程。

不包含页面状态；应用层负责把用户意图映射到同步操作并呈现终态。
