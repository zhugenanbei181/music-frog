# 0.30 Core 重整计划：领域、应用、端口与宿主

状态：0.30 结构性重整规划。0.20 负责冻结当前可交付形态；0.30 允许对 Rust API、crate 依赖和前端接入方式做破坏性调整。

本文件定义底层重整的目标边界和验收规则。具体实现流水写入本地 `TODO.md`，功能归属仍以 `FUNCTIONAL_MAP.md` 为准。

## 1. 版本切线

### 0.20 基线

- `release/0.20` 只冻结当前已经形成的产品安排：Iced 桌面端、Android 伴侣、Bevy UI/widgets 线、Admin API、mihomo 配置与同步能力。
- 0.20 的兼容目标是“当前工作树可复现”，不是为 0.30 保留旧 API 的兼容垫片。
- 0.20 基线提交之后，不在该线上进行 Core contract 的结构性替换。

### 0.30 破坏性重整线

- 从 0.20 基线创建 `codex/0.30` 开发线。
- 可以删除 `CoreSession::client()` 这类直接暴露底层 client 的 API，可以替换 channel、错误类型、store trait 和模块路径。
- 所有调用方在 0.30 同批次迁移；不增加仅为保留旧路径的 re-export 或长期兼容 facade。

## 2. 目标分层

```text
Iced / Bevy / Compose / Admin REST / CLI
                    │
             inbound adapters
                    │
       infiltrator-application
       （use-case、actor、生命周期、事务）
          Tokio 可以是私有实现
                    │
       ┌────────────┴────────────┐
       │                         │
 infiltrator-domain       infiltrator-contract
 纯领域模型与算法          跨端命令/结果/快照/事件
 无 Tokio、无 UI、无 OS     无 toolkit、稳定可序列化
                    │
             infiltrator-ports
             外部能力接口
                    │
  mihomo/http/config/version      host adapters
  REST、WebSocket、文件、下载      desktop/android/ios
```

目标 crate 与当前 crate 的对应关系：

| 目标职责 | 0.30 目标 | 当前迁移来源 |
| --- | --- | --- |
| 纯领域层 | `infiltrator-domain` | `infiltrator-core` 中的状态机、规则、配置变换、编解码和纯计算模块 |
| 跨端契约 | `infiltrator-contract` | `infiltrator-shared` 中的 intent、snapshot、event、capability；主题/本地化另行归类 |
| 外部端口 | `infiltrator-ports` | `CoreController`、`CredentialStore`、文件/时间/网络/TUN 等 trait |
| 应用层 | `infiltrator-application`（可先保留包名 `infiltrator-core`） | `CoreSession`、use-case、apply transaction、actor、scheduler |
| Mihomo outbound adapter | `mihomo-api`、`infiltrator-http` | REST/WebSocket 和 HTTP 传输实现 |
| 配置/版本 outbound adapter | `mihomo-config`、`mihomo-version` | 文件、下载、安装和版本切换实现 |
| Desktop host adapter | `infiltrator-desktop` | 进程、系统代理、TUN、凭据、托盘宿主能力 |
| Android host/FFI | `infiltrator-android`，内部拆 `host` 与 `ffi` | VpnService/JNI/UniFFI 与 Android 生命周期 |
| UI surface | `infiltrator-iced`、`infiltrator-bevy-ui` | 只消费 contract/application facade |
| Bevy 组件库 | `infiltrator-bevy-widgets` | 继续只依赖 Bevy，不依赖业务 crate |

第一阶段不要求一次性改完所有 crate 名称。先在现有 `infiltrator-core` 内建立 `domain`、`ports`、`application` 边界，验证后再抽成独立 crate。

## 3. Tokio 边界

“业务与前端无关”不要求整个产品 Core 没有异步运行时，要求运行时不成为领域和跨端契约的一部分。

- `infiltrator-domain`、`infiltrator-contract` 禁止依赖 Tokio、Reqwest、Bevy、Iced、Compose、操作系统 API 和文件系统实现。
- `infiltrator-application` 可以使用 Tokio，但只在私有实现中使用；公开 API 不返回 `tokio::sync::*`、`JoinHandle`、`Runtime`、`reqwest::Response` 或 `MihomoClient`。
- async API 可以公开 `async fn`；Bevy 和 UniFFI 优先使用 `dispatch`、`snapshot`、`poll_events` 这类 message-based seam。
- 进程内只允许一个 Core actor/runtime。Iced、Bevy、Android Kotlin coroutine 都是调用边界，不各自再拥有一套 Core 事实。
- Bevy ECS 是渲染与投影调度器，不是长耗时网络、进程和同步任务的底层 executor。

## 4. Port 与 host 规则

端口按能力拆分，不创建一个同时包含 Desktop 和 Mobile 所有动词的巨型 `Platform` trait：

- `CoreProcess`：启动、停止、探活、退出原因；
- `SecureStore`：凭据读写；
- `DataStore` / `ProfileStore`：配置和快照持久化；
- `SystemProxy`：系统代理和旁路；
- `TunController`：TUN/VPN 生命周期；
- `AppCatalog`、`NotificationSink`、`PowerEvents`：可选宿主能力。

`infiltrator-desktop`、`infiltrator-android`、未来的 `infiltrator-ios` 是同级 host adapter。它们实现端口，不拥有业务 use-case，也不应互相依赖。

不要用一个泛化的 `infiltrator-mobile` 代替 Android/iOS：Android `VpnService` 和 iOS `NetworkExtension` 的生命周期、权限、进程模型和签名约束不同。可以共享能力模型和 contract，但平台实现必须分别存在。

Bevy 运行在 Android 不等于它实现了 Android VPN。Android 组合形态应是：

```text
Bevy UI 或 Compose
          +
Android host adapter / VpnService
          +
同一个 infiltrator-application
```

平台差异通过 `Capability`、`Availability` 和 typed `Unsupported` 表达。桌面支持任意版本下载而 Android 随 APK/ABI 交付时，应是明确的 accepted difference，而不是 UI 猜测或空值降级。

## 5. 迁移批次

1. **Freeze**：提交当前 0.20 工作树，创建并切换 0.30 开发线。
2. **Contract**：定义 `Command`、`CommandResult`、`CoreSnapshot`、`CoreEvent`、`Capability`、稳定错误码和 revision/generation。
3. **Ports**：把平台、凭据、文件、时钟、Mihomo 控制能力改为端口；底层 adapter 实现端口。
4. **Application**：将 `CoreSession`、配置应用事务和 scheduler 收敛到一个 application service/actor；Tokio 仅留在该层及 adapter。
5. **Domain**：抽出不依赖 IO 的状态机、规则、配置变换、订阅解析、节点 URI 编解码和诊断计算。
6. **Inbound adapters**：Admin、CLI、Iced、Bevy、Android FFI 统一调用 application facade；删除页面直接访问 `MihomoClient`/`ConfigManager` 的路径。
7. **Host split**：将 Desktop、Android、iOS 宿主实现按端口接入；Android crate 内部至少分离 host 与 FFI。
8. **Cleanup**：删除旧模块路径和兼容转发，收紧 Cargo 依赖与架构守卫，更新全端无头/真实宿主矩阵。

## 6. 0.30 验收门槛

- `infiltrator-domain` 和 `infiltrator-contract` 的依赖树中没有 Tokio、Reqwest、Bevy、Iced 或平台 crate。
- 任何跨端公开 API 都不暴露 Tokio channel、任务句柄、Reqwest 类型、`MihomoClient` 或 toolkit 类型。
- Iced、Bevy、Compose、Admin 对同一个 intent 使用同一套结果、错误、能力和 generation 语义。
- 每个进程只有一个 canonical Core 状态源；前端状态只能是带 revision 的投影或缓存。
- Desktop、Android、iOS 的能力差异都有 typed capability/unsupported 结果和对应测试。
- Bevy UI 最终移除对 `mihomo-api`、`reqwest`、Tokio 具体实现的直接依赖；`infiltrator-bevy-widgets` 继续保持业务零依赖。
- 0.30 全量迁移完成后，才删除旧 API 和旧模块路径；不为“保持编译”牺牲目标边界。
