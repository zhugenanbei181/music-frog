# singbox-version

> 层级：L3

产品侧 sing-box target/revision-bound version adapter：不可变 release asset lock、下载与摘要验证、managed install、activation、uninstall、rollback 和 binary selection。

能力必须匹配精确 engine、Cargo target、version 与 adapter revision；资产锁或版本存在本身不授权 Core operation。binary 版本探测（`sing-box -v` 解析与 canonical `v<version>` 校验）由中性 `singbox-driver::version` 提供，本 crate 只消费它。
