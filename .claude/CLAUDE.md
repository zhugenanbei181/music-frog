# CLAUDE.md — Claude Code 宿主适配

本文件只保留 Claude Code 的宿主差异与读取顺序，**不复制规则、地图、状态或命令矩阵**。
共享章程与项目地图的唯一权威是 [../AGENTS.md](../AGENTS.md)（L0）；冲突时以 `AGENTS.md`、`docs/**` 或守卫为准，
并在同一变更里修正本文件。

## 读取顺序

1. [../AGENTS.md](../AGENTS.md) — 项目地图最高入口（L0）：文档边界、身份、阅读顺序、工程不变量与工作协议。
2. [../docs/README.md](../docs/README.md) — 文档中心门（L1）：层级、权威关系与引用方向。
3. 按 `AGENTS.md` 的路由读取对应 L2 域门 / UI 门（`docs/*.md`、`docs/{iced,bevy-ui,android}/README.md`）与受影响 crate 的 `README.md`。

## 宿主差异

- Claude Code 从本文件进入；canonical 读取顺序仍自 `AGENTS.md` 开始。
- 本文件不得成为任何事实的第二真源：需要新增规则时，写入对应 canonical 文档，本文件只留链接。
