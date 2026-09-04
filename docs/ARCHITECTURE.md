# MusicFrog 当前架构与重整目标

MusicFrog 是以 mihomo 为数据平面和代理内核的跨平台客户端。mihomo 是外部 Go 二进制，Rust 负责生命周期、配置、版本、REST/WebSocket 控制和面向 UI 的业务编排；UI 不应直接拥有进程或配置文件事实。

0.20 冻结当前产品安排；0.30 是有意的底层破坏性重整线。领域层、跨端契约、外部端口和 application/runtime 的详细目标见 [CORE_030_REARCHITECTURE.md](CORE_030_REARCHITECTURE.md)。

## 1. 总体数据流

```text
UI intent / user action
          │
          ▼
  application / use-case boundary
          │
   ┌──────┴─────────┐
   ▼                ▼
configuration     mihomo control plane
and sync          (REST / WebSocket)
   │                │
   └──────┬─────────┘
          ▼
 platform host / core lifecycle adapter
          │
          ▼
     mihomo process
          │
          ▼
 typed snapshot / event / failure
          │
          ▼
 UI-specific projection and rendering
```

方向性规则：

- UI 只提交意图、读取不可变结果和能力描述；不直接启动进程、读写 mihomo 配置或拼接 PID 文件。
- Rust 是所有 mihomo 控制操作的唯一产品边界；外部 mihomo Web UI 只能作为受控的浏览器 surface。
- 同一个用户意图只能有一个业务语义和一个错误语义；Iced、Android（以及 0.30 起的 Bevy UI）只负责自己的呈现与宿主适配。
- 生命周期、配置应用和版本切换属于有副作用的命令，必须经过 Rust 的串行化协调；运行态观察可以流式化，但必须有界。

### 导入规范：单一权威路径（2026-08-29，2026-08-30 全仓强制）

- 全仓（业务代码与测试）**禁止一切 re-export 转发层**：`pub use`、`pub(crate) use`、`pub use x::*`（glob）一律不得出现；crate 根与 crate 内子模块一律 `pub mod` 直接暴露，调用方从**定义模块**的规范路径导入。一个事实只允许一个 Rust 路径。例如 `mihomo_api::client::MihomoClient`、`mihomo_api::error::MihomoError`、`mihomo_config::manager::ConfigManager`、`infiltrator_ports::core_process::CoreProcess`、`mihomo_platform::paths::get_home_dir`、`mihomo_version::manager::VersionManager`、`infiltrator_core::settings::AppSettings`、`infiltrator_desktop::runtime::MihomoRuntime`、`infiltrator_admin::admin_api::state::AdminApiContext`。
- **禁止 `use ... as 别名`**：同名冲突一律用完整路径书写（如 `axum::extract::State`、`mihomo_config::profile::Profile`）。唯一豁免是 `use Trait as _;` 匿名 trait 导入——它不绑定新名字，不算别名。
- 两项例外（白名单同时登记在 `scripts/quality/import-guard.py`）：`infiltrator_http::reqwest` 是依赖版本收敛点（全 workspace 统一 reqwest 版本），不是名字转发；`infiltrator-android/src/lib.rs` 与其 `uniffi_api.rs` 的导出面属 UniFFI FFI 表面，随 FFI 专项另行处理。
- 机械化强制：`scripts/quality/import-guard.py --mode enforce`（CI `test.yml` 执行），违规即红；新增公开类型时直接在定义模块登记，不得为省事加转发——避免同一类型出现两个可用路径后调用方随机分叉。

### 异步与调度模型（0.30 重整决策）

- Tokio 继续是 native workspace 的唯一异步执行器，但它只属于 application/runtime 和具体 outbound/host adapter；不进入纯领域层和跨端 contract。
- `async fn` 本身不是前端耦合；真正禁止的是公开 `tokio::sync::*`、`JoinHandle`、`Runtime`、Reqwest 类型和 `MihomoClient`。
- Bevy ECS 不是控制面的底层 executor。长耗时 controller、进程、下载和同步任务由 application/host 的 Core actor/runtime 管理，Bevy 只接收有界 snapshot/event 投影。
- 0.30 的 scheduler 属于 application/runtime；它可以继续使用 Tokio，但不能成为 domain API 的一部分。

## 2. 分层与当前 crate 归属

| 层 | 当前实现 | 应拥有的事实 | 不应拥有 |
| --- | --- | --- | --- |
| Mihomo transport | `mihomo-api` | REST/WebSocket DTO、HTTP 错误、API 能力 | UI 状态、配置文件路径、平台进程句柄 |
| Core configuration | `mihomo-config`、`infiltrator-core` | 配置解析、profile、订阅、DNS/Fake-IP/TUN/rules 等领域操作 | toolkit 类型、Android Compose 状态 |
| Core lifecycle/platform | `mihomo-platform`、`infiltrator-desktop`、Android `MihomoHost` | 进程/VPN/凭据/目录/平台资源 | 业务页面和第二套配置模型 |
| Application/admin | `infiltrator-admin`、`infiltrator-http` | use-case 编排、Admin API、调度、事件和重建流程 | 另一套 mihomo client 或 UI 专属状态 |
| Desktop primary UI | `infiltrator-iced` | Iced 路由、布局、交互、托盘呈现 | OS 进程控制、核心 API 语义 |
| Desktop secondary/legacy | ~~`src-tauri` + `webui/config-manager-ui`~~ | 已于 release/0.20 退役（台账：TAURI_WEBUI_RETIREMENT_LEDGER.md） | — |
| Android surface | `infiltrator-android` + Compose | UniFFI DTO、Android VPN/权限/生命周期和移动布局 | 直接复制桌面业务实现 |
| External dashboard | ~~`webui/mihomo-manager-ui/dist`~~ | 已于 release/0.20 退役 | MusicFrog 自己的配置事实 |
| Sync | `mihomo-dav-sync/*` | WebDAV 传输、索引、状态、冲突处理 | 页面私有同步协议 |

当前依赖图仍有收敛空间：多个上层 crate 同时依赖 `mihomo-api`、`mihomo-config`、`mihomo-platform`，Iced 还持有较大的 `AppState`，application 的标准 Overview 构造仍是临时 outbound seam。Bevy UI 已经只依赖 application/contract；0.30 下一步是把 application 的具体 adapter 构造下沉到 composition root，并迁移 Iced/Admin/Android FFI。

## 3. taskmanager 参照下的求同存异

借鉴 taskmanager 的核心方法，但不照搬它的系统监视器领域模型：

### 共享的“同”

- 用户意图：启动/停止、切换 profile、切换代理模式、选择节点、刷新/保存配置、查看连接和日志、更新内核。
- 领域事实：profile 身份、mihomo core 生命周期、当前配置、运行态快照、能力可用性和错误类型。
- 命令与事件：命令可枚举、结果可关联、异步操作可取消或报告失败，不能靠多个 `bool` 猜状态。
- 版本兼容：mihomo 版本、API endpoint、配置字段、二进制架构和发布包必须有矩阵。
- 验收语义：成功、失败、超时、取消、未安装、无权限、不支持和旧版本不兼容必须可区分。

### 允许的“异”

- Iced 适合桌面密集操作、托盘和多栏布局；Android 适合 VPN 权限、后台服务和窄屏导航；Web Admin 适合浏览器、深度编辑和远程调试入口。
- 同一命令可以有不同按钮、手势、导航层级和信息密度；不能因此改变命令结果或失败含义。
- Android 的核心二进制交付和 VPN 生命周期是平台特化；桌面可以支持下载/切换多个 core，二者不能用伪造的“完全平价”掩盖能力差异。
- WebUI/Tauri 已于 0.20 退役；内嵌 admin server 保留 API-only（Doctor 诊断 loopback + 未来 embedder 契约）。

每个差异都必须挂在一个共享用户意图上，并在 [FRONTENDS.md](FRONTENDS.md) 标记为 `accepted difference` 或 typed `unsupported`。没有对应共享意图的 UI 特性属于分叉，不应直接落地。

## 4. 迁移顺序

1. **0.20 freeze**：提交当前工作树，保留当前行为作为可复现基线。
2. **0.30 contract**：先定 `Command`、`Snapshot`、`Event`、`Capability`、错误码和 generation。
3. **0.30 ports/application**：把 controller、store、网络、文件、时间、TUN 等能力改为端口；将 `CoreSession`、事务和 scheduler 收敛到 application actor。
4. **0.30 domain**：抽出状态机、规则、配置变换、订阅解析、节点编解码和纯诊断计算。
5. **0.30 vertical slice**：优先迁移“profile 切换 → 配置应用 → core readiness → 状态回传”，同时接 Iced、Bevy、Android FFI 和 Admin。
6. **0.30 host/frontends**：Desktop、Android、iOS 作为同级 host adapter；UI 只消费 application contract；最后删除旧路径和直接底层依赖。

## 5. 不可违反的边界

- 不在前端 crate 重新实现 `MihomoApi` 的 endpoint 语义。
- 不在 Android `SharedPreferences`、Iced `AppState`、Vue composable 中保存另一份 canonical profile/config/runtime 事实。
- 不以 `sleep(500ms)` 代替 core readiness；不以空列表、0 或默认模式代替“不支持/不可用”。
- 不把 `cfg(target_os)` 扩散到业务模型；平台差异通过 port、adapter、能力描述和 typed failure 表达。
- 不把真实 mihomo 二进制、外网请求或系统 VPN/托盘作为普通单元测试前提。
