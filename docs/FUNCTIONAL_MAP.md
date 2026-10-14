# 功能归属

> 层级：L2

本表只用于定位逻辑 owner，不登记任务进度或逐页功能。每项业务规则只由一个 domain/application owner 定义；crate 职责见各包 README，跨层关系见 [ARCHITECTURE.md](ARCHITECTURE.md)。

| 范围 | 逻辑 owner | 实现边界 |
| --- | --- | --- |
| 共享业务规则与结果投影 | `infiltrator-domain`、`infiltrator-presentation`、`infiltrator-application` | domain 定义业务规则，contract 定义跨端 observation（含 ProfileDocument codec 的 Mixin Studio facts），presentation 统一折叠已接受观察用于显示并持有纯 typed reducer（含 profile metadata、Mixin Studio copy、订阅状态/配额、Failure copy、Doctor、日志页面状态、RoutingIntent 阶段、规则编辑/trace/statistics 阶段，以及日志导出、脚本导出和快照恢复工作台阶段），application 处理用例并执行命令副作用；日志 application 只保留有界、scope-fenced 原始事件与导出事实；UI 不重算业务事实 |
| Core 生命周期与运行控制 | `infiltrator-application` | `infiltrator-composition` 按 engine 组合 `mihomo-api/config/process/version` 或 `singbox-api/config/process/version`；跨引擎规则见 [CORE_BACKENDS.md](CORE_BACKENDS.md) |
| 跨页面 SurfaceSnapshot 装配 | `infiltrator-composition` | `ProductSurfaceReader` 实现 `SurfaceReader` port，汇集 application 观测并发布共享 contract 快照；单个纯展示折叠由 presentation 持有 |
| 配置、Profile、订阅与同步 | `infiltrator-application`、`infiltrator-domain` | `infiltrator-ports::ProfileStore` 是共享契约，由 `infiltrator-profile-store::EngineProfileStore` 直接实现并拥有 engine-scoped 文件、reset、active-reference、workspace、snapshot-fenced catalog order、typed active-selection clear、独立展示名与 sidecar；`mihomo-config`/`singbox-config` 持有各自原生格式，`mihomo-dav-sync` 持有 Mihomo YAML sync |
| 主机能力与系统副作用 | `infiltrator-ports` | `infiltrator-platform` 提供共享 OS/host adapter；各 engine 的进程由独立 process adapter 持有；状态见 [PLATFORM_MATRIX.md](PLATFORM_MATRIX.md) |
| Iced / Bevy / Slint 产品界面 | `infiltrator-iced`、`infiltrator-bevy-ui`、`infiltrator-slint` | 只提交共享意图并显示结果；Bevy desktop composition 由 `infiltrator-bevy-desktop-host` 持有；表面契约见 [UI_PARITY_AUDIT.md](../crates/infiltrator-contract/docs/UI_PARITY_AUDIT.md) |
| Admin REST ingress | `infiltrator-admin` | 仅做 HTTP v1 wire 映射；代理库存、选择与探测通过宿主注入的 typed outbound owner 和真实 `CoreScope` 进入 `OutboundApplication`，不创建第二份 Core/session |
| Android 产品与进程宿主 | `infiltrator-application`、`infiltrator-android` | Kotlin/Compose、Bevy driver 和 Slint NativeActivity 通过既有 Android owner 接线；产品约束见 [Android 文档](../android/docs/README.md) |
| 跨表面文案与视觉资源 | `infiltrator-shared` | 本地化 lookup 与 gettext catalog、SVG/PNG/ICO/字体的语义 ID 和许可证来源由一个无 UI owner 管理，surface 只适配 renderer，契约见 [跨表面资源](SHARED_UI_RESOURCES.md) |
| 版本与发行 | `mihomo-version`、`singbox-version`、`packaging/` | 各 engine version adapter 消费自己的不可变资产锁；发行状态见 [PLATFORM_MATRIX.md](PLATFORM_MATRIX.md) |
