# Desktop 隔离 smoke

> 层级：L3

[`scripts/desktop-smoke.sh`](../../../scripts/desktop-smoke.sh) 在私有 D-Bus、XDG 目录和虚拟 compositor 中运行真实 Iced 二进制，检查托盘菜单交互、通知传输和宿主隔离；不会连接或修改操作者桌面，也不安装依赖。

```bash
bash scripts/desktop-smoke.sh [tray|notify|proxy-isolation|all]
```

报告和日志写入 `target/desktop-smoke/`。运行所需命令由脚本预检；`waybar`、`mako`、`dunst` 仅用于真实视觉宿主，缺少时使用隔离的协议替身。

此 smoke 不证明内核/TUN、通知视觉效果或 Windows/macOS 行为；这些能力需要各自的系统级证据。脚本实现保留完整断言与隔离细节，不在本文复制菜单项清单或内部进程步骤。
