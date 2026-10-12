# singbox-version

> 层级：L3

实现 sing-box target/revision-bound version adapter：不可变 release asset lock、下载与摘要验证、binary 检查、managed install、activation、uninstall、rollback 和 binary selection。

能力必须匹配精确 engine、Cargo target、version 与 adapter revision；资产锁或版本存在本身不授权 Core operation。
