# MusicFrog 当前架构

> 层级：L2

MusicFrog 是以 mihomo 为数据平面和代理内核的跨平台客户端。mihomo 是外部 Go 二进制，Rust 负责生命周期、配置、版本、REST/WebSocket 控制和面向 UI 的业务编排；UI 不应直接拥有进程或配置文件事实。

当前分层边界见下文 §1、§2；0.30 破坏性重整的历史规划、迁移台账与验收门槛见归档 [CORE_030_REARCHITECTURE.md](archive/CORE_030_REARCHITECTURE.md)（仅作历史，不构成现行规则）。

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
 shared application projection → UI-specific rendering
```

方向性规则：

- UI 只提交意图、读取不可变结果和能力描述；不直接启动进程、读写 mihomo 配置或拼接 PID 文件。
- Rust 是所有 mihomo 控制操作的唯一产品边界；外部 mihomo Web UI 只能作为受控的浏览器 surface。
- 同一个用户意图只能有一个业务语义和一个错误语义；Iced、Bevy UI 和 Android 只负责自己的呈现与宿主适配。
- 生命周期、配置应用和版本切换属于有副作用的命令，必须经过 Rust 的串行化协调；运行态观察可以流式化，但必须有界。
- 独立 UI 二进制的原生入口属于组合根：桌面产品会话在 `infiltrator-desktop::product` 持有宿主与执行器，命令服务和全页 reader 共享同一组引擎。演示必须显式选择；宿主初始化失败呈现真实失败，不退回模拟数据或模拟成功命令。
- Core 的入站命令串行门与生命周期操作门分别负责整条业务命令和进程状态转换。handler 内部的配置/版本事务通过 `CoreLifecyclePort` 操作生命周期，不能重入公共 `execute` 或派发到当前 worker 后等待自身。产品关闭先停止接收请求，等待当前事务完成，再解除 handler 所有权并停止内核；watchdog 不重启已关闭的产品。
- Core 在发布生命周期快照前同步退役旧日志会话。流驱动只能向当前 scope 提交记录，不能用迟到的绑定恢复旧会话。日志导出先在共享层固定脱敏字节与来源，确认只提交该身份；准备与取消均无文件写入，查看器筛选和后续增长不改变已审阅快照。宿主保存完整字节后返回真实路径与字节回执，失败不能伪装成功；两端共用确认和重试状态机。当前原生接入与平台交付状态以功能 owner 及场景证据清单为准。
- 脚本引擎、熔断保护和导出审阅属于独立产品会话的共享 application owner；命令 facade 与 reader 注入同一实例。UI 不新建引擎或读取全局缓存；执行异步提交完整 typed 请求，导出先冻结草稿与字节，确认只携带审阅身份调用宿主，取消无写入。当前原生交付范围以场景证据为准。
- 配置与选项组成来源绑定的 workspace：共享用例读取一致快照，携带实际文档身份提交；有运行宿主时经 `ManagedRuntime::apply_profile_workspace`，没有宿主时经 `ProfileStore` 比较持久化。活动配置和非活动配置均在写入边界重新检查身份；内核应用沿用同一生命周期能力。失败恢复保留原始选项字节，并拒绝覆盖后续编辑；详细过滤结果及实际提交来源由共享 Application 返回。

- 配置快照观测归每个产品的 `SnapshotApplication` 实例，克隆共享同一 owner，独立产品不能借用同进程结果。desktop 组合根将绑定同一 runtime 配置目录的配置、配置读取和快照实例同时注入命令服务与 surface reader，reader 不另从全局配置存储重建这些实例，Iced 和 Bevy 提交同一中性意图；UI 不自行创建文件存储服务。历史按配置名隔离，Diff 还核对实际文档字节哈希，变更后旧观测不可回放。创建、读取、比较和修剪返回类型化回执，空历史与尚未计算、读取失败分别处理；`Unit` 不能替代这些结果。恢复先读取一致 workspace 与实际快照、冻结全文和两个来源哈希，再由实例审阅身份准入；确认重新检查快照，采用 workspace 来源比较提交，成功或取消使身份失效。两端共用工作台和真实回执，框架只负责独立审阅、滚动、按钮与输入隔离。未修改的选项在同一持久化边界保留原始字节；失败保留审阅，来源变化要求重新审阅，不以旧身份覆盖新修改。L2/L3 仍须使用对应原生测试与当前像素证明。

### 导入规范：单一权威路径

- 全仓（业务代码与测试）**禁止一切 re-export 转发层**：`pub use`、`pub(crate) use`、`pub use x::*`（glob）一律不得出现；crate 根与 crate 内子模块一律 `pub mod` 直接暴露，调用方从**定义模块**的规范路径导入。一个事实只允许一个 Rust 路径。例如 `mihomo_api::client::MihomoClient`、`mihomo_api::error::MihomoError`、`mihomo_config::manager::ConfigManager`、`infiltrator_ports::core_process::CoreProcess`、`mihomo_platform::paths::get_home_dir`、`mihomo_version::manager::VersionManager`、`infiltrator_domain::settings::AppSettings`、`infiltrator_desktop::runtime::MihomoRuntime`、`infiltrator_admin::admin_api::state::AdminApiContext`。
- **明确导入，禁止内联长路径与命名别名**：类型、trait、函数从定义模块 `use` 导入。同名冲突先导入定义模块，使用简短 `module::Type`；不在签名、函数体、字段或场景模板中堆叠完整路径（规则见 [CODE_QUALITY_BASELINE.md](CODE_QUALITY_BASELINE.md)）。唯一豁免是 `use Trait as _;` 匿名 trait 导入——它不绑定新名字，不算别名。
- 两项例外（白名单同时登记在 `scripts/quality/import-guard.py`）：`infiltrator_http::reqwest` 是依赖版本收敛点（全 workspace 统一 reqwest 版本），不是名字转发；`infiltrator-android/src/lib.rs` 与其 `uniffi_api.rs` 的导出面属 UniFFI FFI 表面，随 FFI 专项另行处理。
- 机械化强制：`scripts/quality/import-guard.py --mode enforce`（CI `test.yml` 执行），违规即红；新增公开类型时直接在定义模块登记，不得为省事加转发——避免同一类型出现两个可用路径后调用方随机分叉。

### Tokio 边界、port 与 host 规则

- 运行时不成为领域和跨端契约的一部分：`infiltrator-domain`、`infiltrator-contract` 禁止依赖 Tokio、Reqwest、Bevy、Iced、Compose、OS API 和文件系统实现。
- Tokio 不是业务层契约：`infiltrator-application` 公开 API 可以是 `async fn`，但 production crate 只依赖 `ApplicationRuntime` port，串行执行与延迟由 composition root 注入；Tokio 实现只允许出现在 composition/outbound/host adapter。
- `async fn` 本身不是前端耦合；真正禁止的是公开 `tokio::sync::*`、`JoinHandle`、`Runtime`、Reqwest 类型和 `MihomoClient`。Bevy 和 UniFFI 优先使用 `dispatch` / `snapshot` / `poll_events` 这类 message-based seam。
- 进程内只允许一个 Core actor/runtime；Iced、Bevy、Android Kotlin coroutine 都是调用边界，不各自再拥有一套 Core 事实。scheduler 属于 application/runtime，可由 Tokio composition 驱动，但不得把 Tokio 类型带入 domain、contract 或 application API。
- Bevy ECS 是渲染与投影调度器，不是长耗时 controller、进程、下载和同步任务的底层 executor；这些由 application/host 的 Core actor/runtime 管理，Bevy 只接收有界 snapshot/event 投影。
- 端口按能力拆分，不创建同时包含 Desktop 与 Mobile 所有动词的巨型 `Platform` trait：`CoreProcess`（启停/探活/退出原因）、`SecureStore`（凭据）、`DataStore`/`ProfileStore`（配置与快照持久化）、`SystemProxy`、`TunController`，以及可选宿主能力 `AppCatalog`/`NotificationSink`/`PowerEvents`。
- `infiltrator-desktop`、`infiltrator-android`、`infiltrator-ios` 是同级 host adapter：实现端口，不拥有业务 use-case，也不互相依赖。不用泛化的 `infiltrator-mobile` 代替 Android/iOS——`VpnService` 与 `NetworkExtension` 的权限、进程和签名约束不同；平台差异通过 `Capability`、`Availability` 和 typed `Unsupported` 表达。
- Bevy 运行在 Android 不等于它实现了 Android VPN；组合形态是 UI（Bevy 或 Compose）+ Android host adapter/VpnService + 同一个 `infiltrator-application`。
- Bevy 的运行态访问与实体生命周期遵循 [BEVY_UI_FRONTEND.md](bevy-ui/BEVY_UI_FRONTEND.md)：页面、控件与截图激活使用受限 ECS 参数，不能把整个 World 作为业务服务入口；装配、调度与行为验收的边界由该章程统一定义。
- Bevy Android 的目标组合遵循 [BEVY_ANDROID_PRODUCT.md](android/BEVY_ANDROID_PRODUCT.md)：UI 与 VPN 服务分进程，服务持有 Rust application、配置事务与独立 Mihomo 内核，UI 经宿主 IPC 消费中性命令/结果/快照。现有可执行内核与 tun2proxy 路线继续保留扩展；UI 退出不停止服务，服务回收不依赖 Activity。

## 2. 分层与当前 crate 归属

| 层 | 当前实现 | 应拥有的事实 | 不应拥有 |
| --- | --- | --- | --- |
| Mihomo transport | `mihomo-api` | REST/WebSocket DTO、HTTP 错误、API 能力 | UI 状态、配置文件路径、平台进程句柄 |
| Core configuration | `mihomo-config`、`infiltrator-core` | 配置解析、profile、订阅、DNS/Fake-IP/TUN/rules 等领域操作 | toolkit 类型、Android Compose 状态 |
| Core lifecycle/platform | `mihomo-platform`、`infiltrator-desktop`、Android `MihomoHost` | 进程/VPN/凭据/目录/平台资源 | 业务页面和第二套配置模型 |
| Application/admin | `infiltrator-admin`、`infiltrator-http` | use-case 编排、Admin API、调度、事件和重建流程 | 另一套 mihomo client 或 UI 专属状态 |
| Composition roots | `infiltrator-composition`、各 host 的 `composition` 模块 | 组装 application 与具体 outbound/host adapter | 页面状态和第二套业务语义 |
| Iced peer product | `infiltrator-iced` | Iced 路由、布局、交互、托盘呈现 | OS 进程控制、核心 API 语义 |
| Android surface | `infiltrator-android` + Compose | UniFFI DTO、Android VPN/权限/生命周期和移动布局 | 直接复制桌面业务实现 |
| iOS host/surface | `infiltrator-ios` + Swift/Compose | NetworkExtension bridge、Keychain 和 iOS 生命周期 | Android VpnService 实现、未接入的 native bridge |
| Sync | `mihomo-dav-sync/*` | WebDAV 传输、索引、状态、冲突处理 | 页面私有同步协议 |

`src-tauri`、`webui/config-manager-ui` 与外部 dashboard `webui/mihomo-manager-ui/dist` 已于 release/0.20 退役（台账见归档 [TAURI_WEBUI_RETIREMENT_LEDGER.md](archive/TAURI_WEBUI_RETIREMENT_LEDGER.md)）。上层 crate 不得各自构造 mihomo client、配置或平台 adapter；标准 outbound/host adapter 统一由 `infiltrator-composition` 与各 host composition 注入，UI 只接收 application pump。

### 运行态控制器与配置模型

`Proxy::Observed(RuntimeProxyObservation)` 保存控制器明确报告的部分运行事实，缺失的健康、UDP、服务器、端口和加密信息保持未知。`mihomo-api::runtime_proxy` 负责列表与单节点的入站解码，配置节点的原有必填校验不因运行态兼容而放宽。序列化保留原协议名称，不输出虚构凭据或端口。

控制器读取和写入均检查 HTTP 状态。认证、权限和非法参数在端口边界转成中性的 typed failure；`PortError::Rejected` 保留已有 Failure 的类别、消息和恢复策略，UI 不重新分类具体 HTTP 错误。发送完成不能作为远端写入成功。

延迟的测量来源属于契约：已成功测量的 0ms 与控制器历史中的零值、未观测是不同状态。历史记录未附逐条结果，零值可能是舍入或失败，不能判定为超时；原生端消费共享呈现与语义色阶。

## 3. 双端求同存异

Iced 与 Bevy UI 的产品地位、同异律和三层验收统一遵循 [UI_PARITY_AUDIT.md](UI_PARITY_AUDIT.md)。两个 UI crate 与二进制独立发行；domain/contract/ports/application/composition 禁止按前端身份编译。平台能力不支持不能掩盖同一平台上的前端交互缺口。

### 共享的“同”

- 用户意图：启动/停止、切换 profile、切换代理模式、选择节点、刷新/保存配置、查看连接和日志、更新内核。
- 领域事实：profile 身份、mihomo core 生命周期、当前配置、运行态快照、能力可用性和错误类型。
- 命令与事件：命令可枚举、结果可关联、异步操作可取消或报告失败，不能靠多个 `bool` 猜状态。
- 版本兼容：mihomo 版本、API endpoint、配置字段、二进制架构和发布包必须有矩阵。
- 验收语义：成功、失败、超时、取消、未安装、无权限、不支持和旧版本不兼容必须可区分。

### 允许的“异”

- Iced 适合桌面密集操作、托盘和多栏布局；Android 适合 VPN 权限、后台服务和窄屏导航。
- 同一命令可以有不同按钮、手势、导航层级和信息密度；不能因此改变命令结果或失败含义。
- Android 的核心二进制交付和 VPN 生命周期是平台特化；桌面可以支持下载/切换多个 core，二者不能用伪造的“完全平价”掩盖能力差异。

每个差异都必须挂在一个共享用户意图上，并在 [FRONTENDS.md](FRONTENDS.md) 标记为 `accepted difference` 或 typed `unsupported`。没有对应共享意图的 UI 特性属于分叉，不应直接落地。

## 4. 不可违反的边界

- 不在前端 crate 重新实现 `MihomoApi` 的 endpoint 语义。
- 不在 Android `SharedPreferences`、Iced `AppState`、Bevy 资源中保存另一份 canonical profile/config/runtime 事实。
- 不以 `sleep(500ms)` 代替 core readiness；不以空列表、0 或默认模式代替“不支持/不可用”。
- 不把 `cfg(target_os)` 扩散到业务模型；平台差异通过 port、adapter、能力描述和 typed failure 表达。
- 不把真实 mihomo 二进制、外网请求或系统 VPN/托盘作为普通单元测试前提。

DNS 多源泄漏探测是独立的宿主观察，不依赖 DNS 配置读取成功。`SurfaceSnapshot::dns_leak` 是唯一发布字段，由共享 `DnsLeakApplication` owner 提供运行态和最后事实；DNS 配置页失败仍可读取该探测结果。配置 DTO 不得复制这份报告。

Hosts 静态映射由顶层 `hosts` 提供，`dns.hosts` 只保留为旧版误写数据。`SurfaceSnapshot::dns_hosts` 是独立、带配置身份的唯一共享读模型，不由运行态 DNS 回退合成空映射。`ConfigurationApplication` 校验配置身份与原映射后写入实际顶层键；Hosts 专用写入不重写其它 DNS 字段。写入必须先进入共享草稿并显式应用，取消不改变文件；命令持久终态之前不发布已应用事实。

配置应用终态是具体产品运行宿主的观测。实际事务写入同一 ConfigManager 实例的回执记录，application 经 ProfileStore 端口按配置身份读取；独立 manager 和产品实例不能借用其它实例的提交、回滚或错误。领域/契约不持有进程全局发布缓存，测试不得用全局串行锁掩盖隔离失败。

编辑器文档和选项禁止由 contract 的 OnceLock 发布。各产品 ProfileApplication 持有独立观测 owner，命令与 reader 通过同一组合根实例共享；配置切换、较早请求和跨实例票据不能覆盖后续观测。原始内容哈希与选项身份不同的两个 facet 不得同时呈现为同一来源，读取来源随不可变文档返回。

配置文档与 Mixin 的保存意图携带用户实际观测的完整 `ProfileSourceIdentity`，成功返回本次 workspace 提交产生的类型化内容与来源，不以保存后再次读取的其它写入者结果冒充回执。工作区写入区分直接编辑与派生变更；直接编辑的远端保护在 adapter 的同一来源比较/发布边界重新核验，元数据更新也进入该边界。两端从组合根注入的命令服务提交，不临时构造编辑业务 owner。
