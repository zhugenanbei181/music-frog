# state-store

> 层级：L3

持久化 WebDAV 同步所需的本地状态、时间戳和索引记录。

只提供持久化能力，不发起同步或决定冲突结果；事务由 `sync-engine` 编排。
