# indexer

> 层级：L3

把本地同步文件集合转换为可比较的索引和元数据。

索引算法不发起 DAV 请求，也不决定 UI 冲突处理；远端 transport 归 `dav-client`，事务编排归 `sync-engine`。
