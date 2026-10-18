# 功能归属

> 层级：L2

本表只用于定位逻辑 owner，不登记任务进度或逐页功能。每项业务规则只由一个 domain/application owner 定义；crate 职责见各包 README，跨层关系见 [ARCHITECTURE.md](ARCHITECTURE.md)。

| 范围 | 逻辑 owner | 实现边界 |
| --- | --- | --- |
| 共享业务规则与结果投影 | `infiltrator-domain`、`infiltrator-presentation`、`infiltrator-application` | domain 定义业务规则，contract 定义跨端 observation（含 ProfileDocument codec 的 Mixin Studio facts），presentation 统一折叠已接受观察用于显示并持有纯 typed reducer（含 profile metadata、Mixin Studio copy、订阅状态/配额、Failure copy、Doctor、日志页面状态、规则编辑/trace/statistics 阶段，以及日志导出、脚本导出和快照恢复工作台阶段）；其中已与产品 contract 解耦的 typed 阶段、视口策略与本地化 key 端口由中性 `lurker-ui-presentation` 持有；application 处理用例并执行命令副作用；日志 application 只保留有界、scope-fenced 原始事件与导出事实；UI 不重算业务事实 |
| Core 生命周期与运行控制 | `infiltrator-application` | `infiltrator-composition` 按 engine 组合中性 `mihomo-driver` / `singbox-driver`（controller client、process、readiness、codec、version probe）与产品侧 `mihomo-api/config/version`、`singbox-api/config/version` adapter；跨引擎规则见 [CORE_BACKENDS.md](CORE_BACKENDS.md) |
| 引擎 backend 注册与分派 | `lurker-engine-runtime` | 只依赖 `lurker-core-contract`、`lurker-core-ports` 与 `async-trait`；`EngineBackendProviderRegistry<B>` 按 `EngineId` 调度 provider，未注册引擎零副作用返回 typed `Unsupported`，返回 bundle 身份不符则拒绝。bundle/provider 中性词汇归 `lurker-core-ports::engine_backend`，产品 native transaction 与 rule-provider/geo 操作归 `infiltrator-ports::engine_backend`，driver 由宿主组合 |
| 中性呈现内核 | `lurker-ui-presentation` | 只依赖 `lurker-core-contract` 与 `std`；RoutingIntent 改变阶段、代理分组顺序草稿/回执、DNS 泄漏探测阶段、日志视口跟随策略与 `TextCatalog` 本地化 key 端口由它唯一持有，产品侧只实现 `TextCatalog` 并持有文案。仍依赖产品 contract 的投影归 `infiltrator-presentation` |
| 跨页面 SurfaceSnapshot 装配 | `infiltrator-composition` | `ProductSurfaceReader` 实现 `SurfaceReader` port，汇集 application 观测并发布共享 contract 快照；单个纯展示折叠由 presentation 持有 |
| Core/UI 测试 fake 与固定 fixture | `infiltrator-test-support` | 仅持有 engine-neutral port fakes 与测试数据；不创建产品 Core owner，也不选择 Mihomo/sing-box adapter |
| 配置、Profile、订阅与同步 | `infiltrator-application`、`infiltrator-domain` | `infiltrator-ports::ProfileStore` 是共享契约，由 `infiltrator-profile-store::EngineProfileStore` 直接实现并拥有 engine-scoped 文件、reset、active-reference、workspace、snapshot-fenced catalog order、typed active-selection clear、独立展示名、Age 安全凭据与 Core 内 armored Age 解密；其中 durable replace、跨进程写锁、on-disk layout、settings/profile-state schema 与原子 document I/O 由中性 `lurker-profile-store` 内核持有，产品注入历史路径字面量与凭据实现；`mihomo-config`/`singbox-config` 持有各自原生格式，`mihomo-dav-sync` 持有 Mihomo YAML sync |
| 主机能力与系统副作用 | `infiltrator-ports` | `infiltrator-platform` 提供共享 OS/host adapter；各 engine 的进程监督由 `mihomo-driver::process` / `singbox-driver::process` 持有，产品侧只装配启动策略；状态见 [PLATFORM_MATRIX.md](PLATFORM_MATRIX.md) |
| Iced / Bevy / Slint 产品界面 | `infiltrator-iced`、`infiltrator-bevy-ui`、`infiltrator-slint` | 只提交共享意图并显示结果；Bevy desktop composition 由 `infiltrator-bevy-desktop-host` 持有；表面契约见 [UI_PARITY_AUDIT.md](../crates/infiltrator-contract/docs/UI_PARITY_AUDIT.md) |
| Admin REST ingress | `infiltrator-admin` | 仅做 HTTP v1 wire 映射；代理库存、选择与探测通过宿主注入的 typed outbound owner 和真实 `CoreScope` 进入 `OutboundApplication`，不创建第二份 Core/session |
| Android 产品与进程宿主 | `infiltrator-application`、`infiltrator-android` | Kotlin/Compose、Bevy driver 和 Slint NativeActivity 通过既有 Android owner 接线；产品约束见 [Android 文档](../android/docs/README.md) |
| 跨表面文案与视觉资源 | `infiltrator-shared` | 本地化 lookup 与 gettext catalog、SVG/PNG/ICO/字体的语义 ID 和许可证来源由一个无 UI owner 管理，surface 只适配 renderer，契约见 [共享 UI 包与资源](SHARED_UI_PACKAGES.md) |
| 版本与发行 | `mihomo-version`、`singbox-version`、`packaging/` | 各 engine version adapter 消费自己的不可变资产锁；发行状态见 [PLATFORM_MATRIX.md](PLATFORM_MATRIX.md) |
