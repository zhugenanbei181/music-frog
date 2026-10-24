# infiltrator-bevy-ui

> 层级：L3

MusicFrog 的 Bevy 产品界面；负责页面、ECS 投影和交互，不创建产品会话或宿主进程。

宿主通过 typed `SurfaceSource`、`UiCommandSink` 与系统能力输入注入业务事实和操作；桌面组合入口由 [infiltrator-bevy-desktop-host](../infiltrator-bevy-desktop-host/README.md) 持有。页面系统使用受限 Query、资源、Commands 与 Observer，不取得整个 World。实现规则和产品证据入口见 [本地文档](docs/README.md)。通用控件与 headless 基础场景接入由 [infiltrator-bevy-widgets](../infiltrator-bevy-widgets/README.md) 组合共享库，本 crate 聚焦产品页面装配与交互流程。
