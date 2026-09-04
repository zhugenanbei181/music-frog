# mihomo 核心控制契约

mihomo 是外部 Go 二进制，不是本仓库内的 Rust library。Rust 对它的生命周期、配置和 controller API 的控制质量，决定了所有 UI 的稳定性，因此这是本项目最高优先级的边界。

## 1. 三个控制面

| 控制面 | 负责内容 | 当前实现 | 目标约束 |
| --- | --- | --- | --- |
| 生命周期面 | binary、config、data dir、PID、启动/停止、退出原因 | `mihomo-platform`、`infiltrator-desktop`、Android `MihomoHost.kt` | 每个平台一个 adapter；上层只看到 typed lifecycle |
| Controller 面 | REST/WebSocket、secret、runtime snapshot、命令和流 | `mihomo-api::MihomoClient` / `infiltrator-ports::overview::OverviewReader` | 所有 UI 共享同一 API 语义；transport DTO 不泄漏成 UI 状态 |
| 配置/版本面 | profile 文件、订阅、reload、core 下载/安装/切换 | `mihomo-config`、`infiltrator-core`、`mihomo-version` | 写入有事务、切换有回滚、版本和配置兼容有矩阵 |

Rust 的目标调用链：

```text
frontend intent
  → Rust application use-case / CoreLifecyclePort
  → lifecycle adapter + MihomoApi
  → mihomo process/controller
  → typed result + event + new generation
  → frontend projection
```

## 2. 生命周期状态机

目标状态至少区分：

```text
Absent → Starting → Ready → Running
                      │         │
                      └─────────┘
             Stopping ←──────────┘
                 │
          Stopped / Failed
```

- `Starting` 表示进程已经请求启动，但 controller 尚未证明可用。
- `Ready` 表示 readiness probe 已成功，不等于所有配置和 VPN 都已生效。
- `Running` 表示目标 profile、controller 和必要的宿主能力已经绑定。
- `Failed` 必须携带阶段、退出码或 controller 错误；不能降级成 `false` 或空快照。
- 每次成功启动、停止、profile 切换或 core 版本切换都产生新的 `core_generation`。旧 generation 的 stream、延迟测试和写命令不得作用于新实例。

当前桌面/Android 主启动和 apply 路径已经由 `CoreApplication` 统一执行 readiness 重试；其余旧 use-case 仍有分散 controller 查询，迁移时继续收敛到同一个 application 结果流。

## 3. CoreApplication / CoreLifecyclePort 的目标形状

地基已于 0.30 先落在 `infiltrator-domain`、`infiltrator-ports` 和 `infiltrator-application`：`CoreLifecyclePort` 使用稳定的 `CoreLifecycle` 与 generation，配置 apply 事务已不再要求具体 session，Desktop/Android 已切换到 `CoreApplication`；profile/订阅 use-case 也已切到 `ProfileApplication`。旧 session 与 profile 高阶 facade 已删除，剩余是把其他 use-case 从旧 core facade 移入 application。

不要求立即创建同名 struct，但所有功能应逐步收敛到以下概念：

```text
CoreApplication
├── CoreIdentity       # version + platform/arch + data directory
├── LifecycleState     # typed state and failure
├── ControllerEndpoint # URL + secret reference, never raw secret in UI
├── ActiveProfile      # profile identity and config revision
├── Generation         # invalidates old async work
└── CapabilitySnapshot # endpoint/config/platform availability
```

它不应成为拥有所有业务的 God object：

- profile、订阅、sync、network config 和 runtime diagnostics 仍由各自 use-case 负责；
- CoreApplication 只提供当前 core 的生命周期、controller 和 generation；
- UI 通过 use-case 返回的 owned snapshot 工作，不保存可变的 session guard；
- Android 的 Kotlin process/VPN 实现可以继续是 native adapter，但不能再形成独立的 profile/config 事实源。

## 4. 操作纪律

### 启停与重建

1. 校验 binary、profile 和 data directory；
2. 取得生命周期锁，合并重复的 start/stop/rebuild 请求；
3. 启动后做 readiness probe，记录 generation 和 controller endpoint；
4. profile 或 core 变更后执行“保存 → reload/restart → readiness → 结果回传”；
5. 任何阶段失败都保留旧可用状态，或明确进入 `Failed`，不能发布半成功状态。

### 配置写入

- 结构化字段先进入 Rust typed model，再序列化为 mihomo 配置；
- 原始 YAML/JSON 编辑必须通过 parse/validate，成功后原子替换；
- reload 前保留旧文件和 revision，reload 失败可以恢复；
- 规则、DNS、Fake-IP、TUN、providers 和 sniffer 的字段支持由版本能力矩阵决定；
- UI 的 `dirty`、`saving`、`error` 是投影，不是第二份配置事实。

### Runtime API

- `mihomo-api` 负责 transport、DTO、HTTP/WebSocket 和错误转换；
- 上层 use-case 负责把多个 endpoint 组合成一个用户意图，例如“刷新运行态”或“批量测速”；
- logs、traffic、connections 等流必须有取消、断线、重连和缓存上限；
- close connection、switch proxy、patch config 等写命令必须目标化并报告当前 generation；
- API 不支持、controller 未启动、鉴权失败、超时和 mihomo 返回错误必须分别可见。

## 5. 安全与交付

- secret 不进入日志、UI 状态、错误字符串或普通配置备份；
- 外部下载只接受允许的官方 release 资产或显式配置的可信镜像；
- 压缩包、解压后二进制和最终安装路径都应校验 digest；
- 安装采用临时目录 + 原子移动，旧版本在新版本健康检查通过前保留；
- Android 的 native library、Windows MSI、Linux/macOS 外部 core 交付路径必须在平台矩阵中分别声明；
- `MIHOMO_VERSION` 的非 pinned 覆盖不能绕过校验策略。

## 6. 核心验收矩阵

| 场景 | 最低验收 |
| --- | --- |
| 首次启动 | binary/config 缺失、端口占用、立即退出、readiness 超时、正常就绪 |
| 重建 | profile 切换、配置解析失败、reload 失败、旧状态保留、成功后 generation 变化 |
| 运行态 | controller 未启动、secret 错误、HTTP 错误、WebSocket 断线/重连、取消读取 |
| 版本更新 | 资产不存在、digest 不匹配、解压失败、安装中断、健康检查失败、回滚 |
| 多端 | Iced、Tauri/Web、Android 对同一 intent 得到同一语义结果；表面差异单独登记 |

普通单元测试不得启动真实 mihomo 或访问外网；用 `MihomoApi` mock、loopback HTTP/WebSocket mock 和 fake lifecycle adapter。真实二进制只在明确的 release/core smoke 中运行。
