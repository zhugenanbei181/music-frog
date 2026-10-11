# mihomo-dav-sync

> 层级：L3

WebDAV 同步组件集合：

- [dav-client](dav-client/README.md)：远端 HTTP/XML。
- [indexer](indexer/README.md)：本地文件索引。
- [state-store](state-store/README.md)：同步状态持久化。
- [sync-engine](sync-engine/README.md)：比较、重试与冲突编排。
- [platform-android](platform-android/README.md)：Android FFI 接缝。

冲突语义由同步引擎和上层 application 组合；页面不得自建协议。
