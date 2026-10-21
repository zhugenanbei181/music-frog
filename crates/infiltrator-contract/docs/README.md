# Contract 文档

> 层级：L2

本目录记录由 `infiltrator-contract` 拥有的跨 UI 语义与布局契约。

- [跨表面语义与场景验收](UI_PARITY_AUDIT.md)：共享场景身份、同异律和证据要求。
- ProfileDocument codec 的 Mixin Studio 跨端观察由 [`profile_mixin_studio`](../src/profile_mixin_studio.rs) 定义；展示文案由 presentation 投影。
- Profile 展示名由 [`profile_display_name`](../src/profile_display_name.rs) 用 typed Identity/Custom 读写结果表达，稳定 ProfileId 与文件名不变。
- Profile catalog 顺序由 [`profile_catalog_order`](../src/profile_catalog_order.rs) 以完整引擎身份快照、精确目标排列和 Applied/Unchanged/Stale 回执表达。
- 清除活动 profile 选择由 [`profile_selection_clear`](../src/profile_selection_clear.rs) 表达精确期望状态、owner 授权与 `Unselected` 读回；不会暗中停止独立 Core/VPN service。
- Age 私钥由 [`profile_age_credential`](../src/profile_age_credential.rs) 表达不序列化的敏感输入、revisioned 配置状态和显式 owner 授权；owner 执行 Read 时自行读取并原子栅栏当前 active-reference，将确切 selection 与非敏感凭据状态/revision 一并回传，UI 无须从派生 `is_active` 拼出授权上下文；engine/ProfileId 凭据独立于文档是否已创建，支持首次加密订阅导入预置密钥；常规 profile 快照只读取配置状态，不包含密钥。
- 同一契约以非格式化、zeroizing 的 armored/decrypted 内容类型表达 Core 内订阅解密的瞬时载荷；解密结果不是 UI wire/read model。
- 包职责见 [crate README](../README.md)。

## 契约权威性与自底向上裁决铁律

本包所定义的跨 UI 语义契约是所有 Core 与所有 UI 表面（Iced、Bevy、Slint、Android）之间的法定唯一通信边界与真源。

- **先底层契约后上层接入**：遇到新业务场景、底层架构未决或语义不清晰时，必须首先在本契约与 domain/ports 层完成明确裁决，定义不可变的 typed 意图、显式状态机与权威读回；
- **严禁上层私设与修补**：UI 表面严禁自行定义业务结构、严禁以布尔或派生字段推导业务事实、严禁在契约未裁决前在上层进行修补性开发；
- **全端同一律**：所有 UI 表面均平等且严格消费本契约，不得为单一 UI 开辟私有后门或兼容分支。

## 使用说明

Iced、Bevy 与 Slint 桌面三端及 Android 各表面共用 Rust Core 和业务语义，桌面导航与布局各自实现；Slint 完整导入页面并接入共享 contract、快照与 owner，具体范围见 [Slint 表面说明](../../infiltrator-slint/docs/SURFACE.md)。Android 的系统能力和交付状态见 [Android 文档](../../../android/docs/README.md)。

`SurfaceSnapshot.runtime_status` 是跨表面共享的 typed 状态投影。它分开报告控制 service、Core、Controller API、VPN 授权、TUN data plane 与代理流量证据；未知、未连接和未观察到流量都不折叠成 `false` 或“运行中”。Controller API 只有同一 Core session 的真实读取成功才是 Connected。活动连接尚不能关联 typed outbound 时必须显示为 route unattributed，不得据此宣称代理已生效。

### 日常操作

- 在概览页按需启动或停止底层内核（Mihomo / sing-box）。操作失败时按界面显示的原因处理并重试；进程启动不代表代理已生效。
- 在代理页浏览、搜索和筛选节点。选择节点会更新当前代理选择；详情与测速是独立操作，缺少的观测保持未知。
- 在配置页添加、导入或编辑 Profile。先查看校验结果，再保存；取消不写入，失败时保留草稿供修正或重试。
- 在运行信息中查看连接、日志、DNS、规则和测速结果。断开连接、清理数据等有副作用的操作须经过明确确认。
- 在设置页管理平台支持的代理与权限选项。以当前显示的 capability、失败和 unsupported 状态为准，不从另一平台推断支持。

操作与失败语义见 [UI 平权契约](UI_PARITY_AUDIT.md)。

