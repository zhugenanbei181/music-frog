# 使用说明

> 层级：L2

Iced 与 Bevy 两端共用 Rust Core 和业务语义，桌面导航与布局各自实现；Slint 正接入同一 contract、快照与 owner，目前先提供 Core 概览和运行控制入口，具体范围见 [Slint 表面说明](../../infiltrator-slint/docs/SURFACE.md)。Android 的系统能力和交付状态见 [Android 文档](../../../android/docs/README.md)。

`SurfaceSnapshot.runtime_status` 是跨表面共享的 typed 状态投影。它分开报告控制 service、Core、Controller API、VPN 授权、TUN data plane 与代理流量证据；未知、未连接和未观察到流量都不折叠成 `false` 或“运行中”。Controller API 只有同一 Core session 的真实读取成功才是 Connected。活动连接尚不能关联 typed outbound 时必须显示为 route unattributed，不得据此宣称代理已生效。

## 日常操作

- 在概览页按需启动或停止 Mihomo。操作失败时按界面显示的原因处理并重试；进程启动不代表代理已生效。
- 在代理页浏览、搜索和筛选节点。选择节点会更新当前代理选择；详情与测速是独立操作，缺少的观测保持未知。
- 在配置页添加、导入或编辑 Profile。先查看校验结果，再保存；取消不写入，失败时保留草稿供修正或重试。
- 在运行信息中查看连接、日志、DNS、规则和测速结果。断开连接、清理数据等有副作用的操作须经过明确确认。
- 在设置页管理平台支持的代理与权限选项。以当前显示的 capability、失败和 unsupported 状态为准，不从另一平台推断支持。

两端操作与失败语义见 [UI 平权契约](UI_PARITY_AUDIT.md)。
