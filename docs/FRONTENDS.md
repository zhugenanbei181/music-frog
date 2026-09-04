# 多 UI 求同存异矩阵

本项目的 UI 不是多份产品逻辑。Iced 是当前成熟主桌面 surface（Tauri + Vue 已退役，台账见 [TAURI_WEBUI_RETIREMENT_LEDGER.md](TAURI_WEBUI_RETIREMENT_LEDGER.md)），Bevy UI 是跨平台统一战略 surface（桌面 + 移动 + iOS 大一统，章程见 [BEVY_UI_FRONTEND.md](BEVY_UI_FRONTEND.md)），Android Compose 是原生移动伴侣。三者共享用户意图和 Rust 结果，不共享 toolkit 的状态和布局实现。

从 0.30 起，三类前端都通过 `infiltrator-application` 的 contract 接入；Tokio 可以存在于 application/runtime 内部，但不能通过 `MihomoClient`、Reqwest 类型或 Tokio channel 进入前端契约。Desktop、Android、iOS 是与 UI 正交的同级 host adapter，负责各自的进程、VPN、权限和系统能力。

> **双端同步战略演进说明（2026-09-03 升级）**：
> 依据最高主控台账 [DUAL_SURFACE_PARITY_MASTER_PLAN.md](DUAL_SURFACE_PARITY_MASTER_PLAN.md)，Iced 与 Bevy UI 正式确立为**对等双主干 Surface**。二者彻底告别“先后跟随”模式，在**功能完备度**与**UI/UX 表现**上步调一致、严格同步演进。双端全面对标 Clash Verge Rev、Mihomo Party、Flclash、Surge 的最完善功能并集。

## 1. 决策标记

| 标记 | 含义 |
| --- | --- |
| `shared` | 命令、数据、错误和生命周期语义共用；实现可在各端独立 |
| `local` | 只属于该端的窗口、手势、布局、导航或宿主能力 |
| `accepted difference` | 有意不同，但对应同一个 shared intent，且有替代路径/原因 |
| `unsupported` | 当前端没有能力或产品价值不足，必须显示 typed 不支持，不得静默隐藏 |

## 2. 功能矩阵（Iced / Bevy UI / Android Compose）

| 用户意图 | Iced 主桌面 | Bevy UI 跨平台主干 | Android Compose | 共享规则与权威契约 |
| --- | --- | --- | --- | --- |
| 启动/停止/重启 mihomo | `shared` + desktop tray `local` | `shared` + desktop/mobile tray `local` | `shared` + VPN/background `local` | 同一 lifecycle state、failure、generation |
| profile 导入/编辑/删除/切换 | `shared` + 桌面编辑器 `local` | `shared` + bsn! 编辑器 `local` | `shared` + 移动表单 `local` | profile identity、revision 和重建结果一致 |
| 订阅更新与聚合 | `shared` | `shared` | `shared` | scheduler、重试、部分失败和聚合语义一致 |
| 代理模式/代理组/节点 | `shared` | `shared` | `shared` | mode、group、node、delay 的类型和错误一致 |
| connections/logs/traffic/memory/IP | `shared` | `shared` | `shared` | snapshot/stream 生命周期与不可用状态一致 |
| DNS/Fake-IP/TUN | `shared` + 多栏表单 `local` | `shared` + 响应式场景 `local` | `shared` + VPN 权限 `local` | 字段能力来自 Rust capability，不以 UI 默认值补齐 |
| rules/providers/sniffer | `shared` + 原始编辑器 `local` | `shared` + 场景编辑器 `local` | `shared` + 移动子页面 `local` | 结构化字段和 raw JSON 的校验规则一致 |
| WebDAV sync | `shared` | `shared` | `shared` | 连接、冲突、取消和结果模型一致 |
| core 下载/安装/切换 | `shared` | `shared` | `accepted difference`：随 APK/ABI 交付 | Android 不提供桌面式任意版本安装时要显式说明 |
| 系统代理/自启动/托盘/悬浮窗 | `local` | `local` | `unsupported` 或 Android 系统设置路径 | 不把 OS 动词泄漏到共享领域模型 |
| 视觉动效、布局密度、导航手势 | `local` | `local` | `local` | 允许差异，不得改变功能可达性和失败语义 |

## 3. 共享层与本地层的切分

### 必须进入 Rust shared contract

- 产品意图和命令名（`CommandIntent`）；
- 输入校验、目标身份、当前 revision/generation；
- 只读视图状态切片（`DomainState`）；
- owned response、capability、availability 和错误枚举；
- 异步任务的开始、进度、终止、取消和重试语义；
- profile/config/runtime 的 canonical owner；
- 跨端无头行为测试需要验证的结果矩阵。

### 必须留在 frontend/host local

- Iced widget、Bevy Scene、Compose composable 和各自的 view model / system；
- 窗口、托盘、悬浮窗、Android permission/VPN service、输入法和返回栈；
- 适合屏幕尺寸的布局、动画、滚动、手势和密度；
- toolkit 自己的缓存，但缓存只能由 shared revision/generation 失效。

### 0.30 接入规则

- 前端提交 `CommandIntent`，消费 owned `Snapshot`、`Event`、`Capability` 和 typed failure；不得逐页构造底层 Mihomo client。
- Bevy 的 ECS resource 是渲染投影，不是 Core canonical state；Android ViewModel 和 Iced `AppState` 也遵守同一规则。
- 平台能力由 host adapter 注入。系统代理、托盘、`VpnService`、`NetworkExtension` 等 OS 动词不进入共享领域模型。

## 4. 双端同步演进规则

1. 业务逻辑下沉到 `infiltrator-core` 与 `mihomo-platform`，共享视图模型定义于 `infiltrator-shared`。
2. 任何功能特性或 UI 交互升级，必须在同一批次同时向 `infiltrator-iced` 与 `infiltrator-bevy-ui` 提交对应实现。
3. 双端必须通过对应的无头自动化测试（`iced_*_tests.rs` 与 `headless/*_tests.rs`），杜绝单端功能漂移。
4. 具体并集功能点与 UI 表现清单，严格对齐 [DUAL_SURFACE_PARITY_MASTER_PLAN.md](DUAL_SURFACE_PARITY_MASTER_PLAN.md)。

## 5. 反分叉检查

- 新按钮是否对应已有 intent？没有就先进入 shared contract。
- 新错误是否来自 Rust typed result？不能在 UI 里靠字符串猜测。
- 同一数据是否在 Iced state、Bevy resource、Android ViewModel 各存一份？若是，区分 canonical 与 render cache。
- 某端不支持时是否明确展示原因？不能用空列表、默认值或隐藏导航伪装完成。
- 差异是否有 layout/permission/toolkit 方面的真实理由？没有理由的差异应合并。
