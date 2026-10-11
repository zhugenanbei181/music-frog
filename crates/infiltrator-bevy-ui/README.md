# infiltrator-bevy-ui

> 层级：L3

MusicFrog 的 Bevy 产品界面；负责页面、ECS 投影、交互和 Bevy 原生宿主入口。

业务命令和事实来自共享 application；页面系统使用受限 Query、资源、Commands 与 Observer，不取得整个 World。实现规则和产品证据入口见 [本地文档](docs/README.md)。

通用 UI 功能先扩展对应 `zhuge-ui-*` owner，再在本 crate 接入；不得复制共享实现。
产品适配保留业务职责，更新共享依赖时同时验证其他消费者和可见行为。
