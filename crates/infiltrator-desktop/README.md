# infiltrator-desktop

> 层级：L3

桌面 host adapter 与产品会话：core 进程、目录、权限、系统代理、托盘、凭据和退出清理。Iced、Bevy 与 Slint 桌面产品由相同 application/ports 组合；Slint 入口见 [infiltrator-slint](../infiltrator-slint/README.md)。

桌面宿主负责 OS 资源，不重复业务用例；隔离 smoke 见 [本地文档](docs/README.md)。

`DesktopProductSessionBinding` 同时提供当前 backend 的 `CoreApplication`、`ProfileApplication` 与 host handles。Profile application 与 Mihomo/sing-box backend 使用同一 engine-scoped profile store；跨引擎切换或恢复后，调用方必须重新获取 binding，旧 binding 的 profile 请求仍由 active-reference revision 与 Core owner authorization 围栏。
