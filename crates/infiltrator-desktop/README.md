# infiltrator-desktop

> 层级：L3

桌面 host adapter 与产品会话：core 进程、目录、权限、系统代理、托盘、凭据和退出清理。Iced、Bevy 与 Slint 桌面产品由相同 application/ports 组合；Slint 入口见 [infiltrator-slint](../infiltrator-slint/README.md)。

桌面宿主负责 OS 资源，不重复业务用例。

## 隔离 smoke

[`scripts/desktop-smoke.sh`](../../scripts/desktop-smoke.sh) 在私有 D-Bus、XDG 目录和虚拟 compositor 中运行真实 Iced 二进制，检查托盘菜单交互、通知传输和宿主隔离；不会连接或修改操作者桌面，也不安装依赖。

```bash
bash scripts/desktop-smoke.sh [tray|notify|proxy-isolation|all]
```

报告和日志写入 `target/desktop-smoke/`。运行所需命令由脚本预检；`waybar`、`mako`、`dunst` 仅用于真实视觉宿主，缺少时使用隔离的协议替身。此 smoke 不证明内核/TUN、通知视觉效果或 Windows/macOS 行为；这些能力需要各自的系统级证据。

`DesktopProductSessionBinding` 同时提供当前 backend 的 `CoreApplication`、`ProfileApplication` 与 host handles。Profile application 与 Mihomo/sing-box backend 使用同一 engine-scoped profile store；跨引擎切换或恢复后，调用方必须重新获取 binding，旧 binding 的 profile 请求仍由 active-reference revision 与 Core owner authorization 围栏。
