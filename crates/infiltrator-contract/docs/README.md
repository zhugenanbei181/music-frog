# Contract 文档

> 层级：L2

本目录记录由 `infiltrator-contract` 拥有的跨 UI 语义与布局契约。

- [双端语义与场景验收](UI_PARITY_AUDIT.md)：共享场景身份、同异律和证据要求。
- [使用说明](USAGE_SPEC.md)：两端共用的基本操作。
- ProfileDocument codec 的 Mixin Studio 跨端观察由 [`profile_mixin_studio`](../src/profile_mixin_studio.rs) 定义；展示文案由 presentation 投影。
- Profile 展示名由 [`profile_display_name`](../src/profile_display_name.rs) 用 typed Identity/Custom 读写结果表达，稳定 ProfileId 与文件名不变。
- Profile catalog 顺序由 [`profile_catalog_order`](../src/profile_catalog_order.rs) 以完整引擎身份快照、精确目标排列和 Applied/Unchanged/Stale 回执表达。
- 包职责见 [crate README](../README.md)。
