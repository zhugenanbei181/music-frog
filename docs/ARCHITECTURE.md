# 架构边界

> 层级：L2

Rust 是 Core、配置和业务编排的唯一边界。跨引擎契约见 [CORE_BACKENDS.md](CORE_BACKENDS.md)，各 crate 职责由其包内说明承载。

```text
contract → domain ────────────────┐
shared ───────────────────────────┼→ presentation → Iced / Bevy / Slint UI
contract + domain + ports ────────┼→ application → composition → host/core adapters
```

- `contract` 定义跨端意图、结果与快照；`domain` 定义业务规则；`ports` 定义副作用接口；`application` 实现用例；`presentation` 只折叠已观测快照供多表面复用，不拥有事实或副作用。
- `composition` 注入具体 adapter，并把 application 观测组装为跨页面 contract 快照；`ProductSurfaceReader` 在此实现 `SurfaceReader` port。共享 OS 能力归 `infiltrator-platform`；engine-neutral profile 文件、workspace、catalog、owner record、active-reference CAS 与 `ProfileStore` port 由 `infiltrator-profile-store::EngineProfileStore` 实现。`mihomo-config::ConfigManager` 只保留 Mihomo YAML、legacy migration 与 engine facade，不实现共享 persistence port。各 engine 按 controller API、原生配置、进程生命周期、版本资产拆分 adapter crate。准确 owner 与依赖方向见 [Core 后端契约](CORE_BACKENDS.md)。
- Core 生命周期、活动 profile 写入、controller 操作、proxy endpoint 观察和公网探测路由分别由窄 port 表达；一个宽泛 runtime trait 不承担多个 owner 的组合职责。
- 依赖向外流动。共享层不依赖 runtime、网络、OS 或 UI toolkit；UI 不启动 core、不写 canonical 配置、不创建第二个业务 owner。
- Bevy desktop 的 `DesktopProductSession` 组合、Core surface pump、命令 adapter 和进程入口由 `infiltrator-bevy-desktop-host` 持有；`infiltrator-bevy-ui` 只消费注入的 typed surface、command sink 与宿主能力。
- 有副作用的操作由单一 owner 串行执行并校验 scope/revision/generation；迟到结果不得覆盖新状态。成功须有真实终态或读回。
- 可观察业务事实只有一个 owner；前端仅保存输入草稿和呈现缓存。
- 跨表面的纯 copy、状态展示折叠和快照到视图模型映射由 `infiltrator-presentation` 复用。迁移期间仍留在 application 的纯展示模块必须逐项登记并搬迁，不得在 Iced/Bevy 各自复制。
- “共享一个 Core”指共享 engine-neutral use case、身份/能力/命令/快照契约，不代表跨 Android 进程共享 Rust 静态变量。Android 中只有 `:vpn` 能创建 Core application、配置事务、VPN 和内核进程；Compose/Bevy 通过 typed IPC 访问 owner。桌面可在本机进程内组合 Core 与 adapter，但仍只允许一个活动 Core actor。
- Core 必须允许每个后端使用各自独立的进程、原生配置格式、controller API 和版本资产 adapter；公共层只表达经验证的共同意图。`Unsupported` 属于对应 engine adapter 的逐操作能力，不改变职责拆分，细则见 [CORE_BACKENDS.md](CORE_BACKENDS.md)。

Iced、Bevy 与 Slint 的共享边界和验收状态见 [UI 契约](../crates/infiltrator-contract/docs/README.md)。Android owner 与生命周期见 [Android 文档](../android/docs/README.md)；Bevy ECS 规则见 [Bevy UI 文档](../crates/infiltrator-bevy-ui/docs/README.md)。
