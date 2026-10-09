# MusicFrog Infiltrator: 双端（Iced & Bevy UI）同步演进与成熟 Mihomo 全景并集主控规范 (Dual-Surface Parity Master Plan)

> 层级：L2

本文档是 MusicFrog Infiltrator 项目的最高战略主控台账，旨在确立 **Iced** 与 **Bevy UI** 两个同权、独立发行的产品的**严格同步演进机制**，并全面对标业界成熟 Mihomo 客户端，以其**最完善功能组的能力并集（Union）**作为最终目标。

> **状态口径**：本文的 15×15（225 项）是目标与执行台账，不等同于已完成。状态词含义：
> - `host-verified`：仅在列明的宿主（如 desktop Linux）上取得证据，不外推其他平台或 L2/L3 平权；
> - `legacy-unassessed`：旧 `parity-ready` 标记统一降级，仅保留历史实现与 L1 声明，待按 [UI_PARITY_AUDIT.md](UI_PARITY_AUDIT.md) 重新验收；
> - `shared-ready`：共享层就绪，但至少一端缺少可验收的操作面或宿主事实；
> - `planned`：仅有字段或未接线，不满足双端闭环。
>
> `parity-ready` 是历史口径（shared + 双端 + 双端测试 + 适用宿主证据四层齐备），本轮统一降级为 `legacy-unassessed`，仅在 [RESPONSIVE_PARITY_LEDGER.md](RESPONSIVE_PARITY_LEDGER.md) 等台账保留其原始证据范围。225 项均不能凭旧标记宣称 L2/L3 平权；当前验收以 [UI_PARITY_AUDIT.md](UI_PARITY_AUDIT.md)、共享 `FeatureId::ALL` 与 `scripts/parity/`（`feature_evidence.tsv`、`cross_surface_manifest.tsv`、`resolve_surface_evidence.py`）的真实证据解析为准。历史架构审计见 [DUAL_SURFACE_ARCHITECTURE_AUDIT_030.md](archive/DUAL_SURFACE_ARCHITECTURE_AUDIT_030.md)。

Bevy Android 的联合产品推进遵循 [BEVY_ANDROID_PRODUCT.md](android/BEVY_ANDROID_PRODUCT.md)：现有宿主、控件、共享异步与二维图形保留扩展，缺少的进程/IPC、原生输入、业务回收、节能及设备证据纳入本地 TODO 的 `BANDROID-001`～`BANDROID-017`。它们是既有功能与平台闸门的实施任务，不另建 FeatureId 或纸面完成率；Iced/Bevy 同权验收继续适用。

---

## 一、双端同步推进战略定调与工程原则

### 1. 核心定位转变：从“先后跟随”转为“严格对等双主干”
*   **过去模式**：Iced 优先承接新功能，Bevy UI 后续里程碑追赶。该模式易导致两端视图模型分叉、功能落差扩大。
*   **当前标准**：Iced 与 Bevy UI 正式确立为**对等双主干 Surface**。任何新业务功能与 UI 交互能力，必须在同一批次（Wave）内由双端同步交付与验收，杜绝“单端孤立特性”。

### 2. 双端同步必须开展的 4 项工程基础设施

```
               ┌────────────────────────────────────────────────────────┐
               │       共享应用层与跨端状态机（单一事实源）              │
               │  infiltrator-application / domain / contract / ports  │
               └──────────────────────────┬─────────────────────────────┘
                                          │
                                          ▼
               ┌────────────────────────────────────────────────────────┐
               │       共享契约与能力模型（UI-Agnostic Seam）            │
               │  CommandIntent / Snapshot / Event / Capability         │
               └──────────────┬──────────────────────────┬──────────────┘
                              │                          │
                              ▼                          ▼
          ┌───────────────────────────────┐  ┌───────────────────────────────┐
          │    infiltrator-iced (桌面)    │  │  infiltrator-bevy-ui (跨平台)  │
          │  - Elm 架构 (View/Update)      │  │  - ECS 架构 (Scenes/Systems)  │
          │  - Iced Theme / Canvas        │  │  - Bevy Widgets / Reactive    │
          └───────────────┬───────────────┘  └───────────────┬───────────────┘
                          │                                  │
                          └────────────────┬─────────────────┘
                                           ▼
               ┌────────────────────────────────────────────────────────┐
               │ composition roots + host/outbound adapters             │
               │ mihomo-api / config / version / desktop / android / ios│
               └────────────────────────────────────────────────────────┘
                                           ▼
               ┌────────────────────────────────────────────────────────┐
               │ 双端行为、视觉与宿主证据门禁                           │
               │ nextest evidence / scene captures / live host smoke  │
               └────────────────────────────────────────────────────────┘
```

1.  **业务逻辑 100% 下沉与 ViewModel 契约化**：
    *   严禁在 UI crate 中编写业务调度、网络请求或配置组装。
    *   业务状态机收敛于 `infiltrator-domain` 与 `infiltrator-application`；平台和传输实现留在 ports 之后的 adapter；
    *   双端共享 `CommandIntent`、typed result、snapshot、event、capability 和 revision/generation；`infiltrator-shared` 只承载确实属于展示共享的主题、本地化和格式化资源。
2.  **单向命令总线与事件广播标准化**：
    *   Iced 的 `Message` 与 Bevy 的 `UiCommand` 只做 toolkit 映射，背后派发相同的 `CommandIntent`，由同一个 application/core 事实源处理；
    *   底层遥测流（WebSocket 流量、连接流、日志）统一推入无锁缓冲区，双端同频消费。
3.  **共享语义与原生交互表面**：
    *   “同异律”与 L1/L2/L3 的完整定义由 [UI_PARITY_AUDIT.md](UI_PARITY_AUDIT.md) 唯一负责，求同到语义、操作后果与交互深度为止；原生排版和渲染实现可以不同。
    *   模态、抽屉、向导、详情检查器必须提供等价操作表面；不以 Toast、静默执行或空挂路由替代。共享设计 token 不等于像素强制镜像。
4.  **真实测试与像素证据闭环**：
    *   `FeatureId::ALL` 是交互场景目录的唯一事实源；`scripts/parity/cross_surface_manifest.tsv` 与 `feature_evidence.tsv` 绑定两个产品的真实测试 ID。
    *   单一解析器对照 `cargo nextest list` 动态发现结果，悬空、缺端、重复或错误归属直接阻断；源码字符串与正则命中不能证明行为。
    *   场景验收先实际执行绑定测试，再激活真实生产控件，捕获 1180×780 和 720×480 两种视口的双端像素，校验图像/二进制/源码哈希和进程窗口身份。
    *   `pending` 保留可见缺口；只有 L1、L2、L3 全部闭合才可宣称对齐。225 项旧台账是业务范围，不能替代当前 48 个交互场景的证据。

代码规模、明确导入与反规避规则以 [CODE_QUALITY_BASELINE.md](CODE_QUALITY_BASELINE.md) 为准，所有 Rust 源码和测试执行相同预算。

---

## 二、成熟 Mihomo 客户端能力并集标杆定义

我们对标的成熟客户端包括：
*   **Clash Verge Rev**：脚本生态（JS/TS 沙箱）、多级配置覆写（Merge Rules / Mixin）、服务模式（Service Mode）、全局热键、高级 TUN 网络栈。
*   **Mihomo Party**：Sub-Store 原生整合、分流链路可视化拓扑、交互式实时分流追踪器（Live Rule Tracer）、多维测速丢包雷达、应用级分流与高清图标提取。
*   **Flclash**：桌面/平板/移动多模态响应式自适应设计、多配置聚合器（Aggregator）、高密度卡片网格、WebDAV 云同步。
*   **Clash Nyanpasu**：多维连接审计（按进程/按域名）、Fake-IP 池检索、流体动效与主题定制。
*   **Surge / Stash**：DNS/TCP/TLS/TTFB 握手瀑布流、ASN 归属透视、Doctor 专家级自愈诊断、实时抓包与 PCAP 导出。

**本项目目标：将上述所有竞品的最完善能力求“全功能并集（Union）”，并在 Iced 与 Bevy UI 中全量镜像对齐！**

### 2.1 Mihomo 能力基线

能力规划先以 Mihomo 官方配置与 API 面为边界，再决定客户端增强项：配置面覆盖 DNS、Tun/listeners、出站协议、代理组、路由规则、规则集合、子规则和流量隧道；API 面覆盖日志、流量、内存、版本、缓存、运行配置、重启/升级、代理组、代理、providers、规则、连接、DNS、存储和 debug。客户端负责可理解地编辑、验证、回滚和观测，不在 Rust UI 层重新实现 Mihomo 的协议内核。

参考：[Mihomo 配置文档](https://wiki.metacubex.one/config/)、[Mihomo API 文档](https://wiki.metacubex.one/api/)、[Clash Verge Rev](https://github.com/clash-verge-rev/clash-verge-rev)、[FlClash](https://github.com/chen08209/FlClash)。

---

## 三、15 大业务组 × 15 项全景深度功能清单（225 项双端深度对标并集）

为彻底消除表面化对齐，本项目确立 **15 大核心业务组 × 15 项深度能力（共 225 项具体工程指标）**，要求 `infiltrator-iced` 与 `infiltrator-bevy-ui` 协同攻坚、共同对齐。每项的编号为 `DUAL-组号-项号`，每项都必须同时具备 shared/application、Iced、Bevy、双端测试和适用宿主证据；单端完成不计入完成。

> **证据口径**：本节 15×15 账目只登记任务码、条目名称与状态词。每项的双端测试锚点、跨端登记与行为证据统一由 `scripts/parity/` 动态解析：`feature_evidence.tsv` 给出各端精确测试锚点，`cross_surface_manifest.tsv` 登记两端场景，`resolve_surface_evidence.py` 对照编译后的 Rust 注册表与 `cargo nextest list` 交叉核对。本文不再内联测试名清单或实现细节；旧文字 guard 不构成交付依据。

### 组交付状态总览

| 业务组 | 项数 | 当前状态 |
| :--- | :---: | :--- |
| 组 01 Core 运行时与内核交付 | 15 | 1 `host-verified`（desktop Linux）+ 14 `legacy-unassessed` |
| 组 02 特权网络与 TUN | 15 | 15 `legacy-unassessed` |
| 组 03 概览与遥测 | 15 | 15 `legacy-unassessed`（14 由响应式台账权威跟踪） |
| 组 04 代理与排序 | 15 | 15 `legacy-unassessed` |
| 组 05 协议生态与多路复用 | 15 | 15 `legacy-unassessed` |
| 组 06 并发测速与稳定性 | 15 | 15 `legacy-unassessed` |
| 组 07 订阅生命周期 | 15 | 15 `legacy-unassessed` |
| 组 08 多源聚合器 | 15 | 15 `legacy-unassessed` |
| 组 09 AST YAML 与快照 Diff | 15 | 15 `legacy-unassessed` |
| 组 10 脚本沙箱与 Mixin | 15 | 15 `legacy-unassessed` |
| 组 11 规则引擎与 MRS | 15 | 15 `legacy-unassessed` |
| 组 12 Live Rule Tracer | 15 | 15 `legacy-unassessed` |
| 组 13 连接审计 | 15 | 14 `legacy-unassessed` + 1 `planned`（13-04） |
| 组 14 DNS 工作台与泄漏探活 | 15 | 15 `legacy-unassessed` |
| 组 15 多模态外壳与命令流 | 15 | 14 `legacy-unassessed` + 1 `shared-ready`（15-10） |

### 组 01：Core 运行时、进程守护与内核多版本交付 (Runtime & Lifecycle)
1. **多代际内核会话状态机**：Session Token 隔离、Generation 递增与孤儿进程自动清理。
2. **平滑配置热重载 (Hot Reload)**：`PUT /configs?force=true` 毫秒级重载，避免断网断流。
3. **崩溃自愈看门狗 (Crash Watchdog)**：异常退出 3 秒内心跳自动重启，带指数退避与熔断。
4. **内核多通道版本交付**：Stable / Alpha (Pre-release) / Meta-Core 在线版本探测。
5. **内核二进制 SHA256 校验**：下载自动比对官方 release digest，防止篡改。
6. **内核版本秒级回滚**：本地保留历史版本 binary，一键切换并快速恢复。
7. **外部 Controller 免密拉起**：自动生成并在请求头注入 Secret 凭据，杜绝未授权访问。
8. **内核日志等级即时下发**：无需重启内核动态 `PATCH /configs` 切换 debug/info/warn/error。
9. **服务模式 (Service Mode) 提权守卫**：Windows Service / Linux Polkit / macOS launchd 免 UAC 弹窗。
10. **进程退出清理保证**：注册 OS 信号处理（SIGINT/SIGTERM/Ctrl+C），退出时 100% 复位网络。
11. **端口冲突自动探测与避让**：检测 7890/9090 冲突，自动提示占用进程 PID 并提供一键释放。
12. **内核内存与 CPU 软限配额**：监控内核资源占用，超过 512MB 时主动触发 GC。
13. **离线与无网启动容灾**：本地配置预校验通过即可离线冷启动，不依赖远端鉴权。
14. **双端生命周期状态机同步**：Iced 与 Bevy 共享同一套 `CoreLifecycle` 运行时驱动。
15. **无头测试全景覆盖**：双端具备模拟启动失败、端口冲突、平滑停止的完整测试。

#### 组 01 逐项账目

| 项 | 任务 | 状态 |
| :--- | :--- | :--- |
| `DUAL-01-01` | 多代际内核会话状态机 | `host-verified`（desktop Linux） |
| `DUAL-01-02` | 平滑配置热重载 | `legacy-unassessed` |
| `DUAL-01-03` | 崩溃自愈看门狗 | `legacy-unassessed` |
| `DUAL-01-04` | 内核多通道版本交付 | `legacy-unassessed` |
| `DUAL-01-05` | 内核二进制 SHA256 校验 | `legacy-unassessed` |
| `DUAL-01-06` | 内核版本秒级回滚 | `legacy-unassessed` |
| `DUAL-01-07` | 外部 Controller 免密拉起 | `legacy-unassessed` |
| `DUAL-01-08` | 内核日志等级即时下发 | `legacy-unassessed` |
| `DUAL-01-09` | 服务模式 (Service Mode) 提权守卫 | `legacy-unassessed` |
| `DUAL-01-10` | 进程退出清理保证 | `legacy-unassessed` |
| `DUAL-01-11` | 端口冲突自动探测与避让 | `legacy-unassessed` |
| `DUAL-01-12` | 内核内存与 CPU 软限配额 | `legacy-unassessed` |
| `DUAL-01-13` | 离线与无网启动容灾 | `legacy-unassessed` |
| `DUAL-01-14` | 双端生命周期状态机同步 | `legacy-unassessed` |
| `DUAL-01-15` | 双端无头测试全景覆盖 | `legacy-unassessed` |

> **证据与边界**：核心/生命周期/版本/服务模式由 shared contract + application 承载，双端投影与 headless 旅程由 `scripts/parity/` 解析；除 `DUAL-01-01` 已在 desktop Linux 取得 `host-verified` 外，其余真实发行包与跨平台 smoke 未计入 `host-verified`。

### 组 02：特权网络栈、TUN 虚拟网卡与系统代理接管 (Platform Network & TUN)
1. **TUN 四大堆栈自由调度**：支持 `gVisor`、`System`、`Mixed`、`LWIP` 原生切换。
2. **物理与虚拟网卡 MTU 自适应协商**：动态探测物理出站 MTU，自动计算最佳 TUN MTU。
3. **严格路由与全局流量劫持**：`strict-route: true` 防止流量绕过，自动添加路由表项。
4. **系统 HTTP/SOCKS 代理一键注入**：Windows 注册表、Linux GNOME/KDE/GSettings、macOS networksetup 统一封装。
5. **系统代理被抢占实时探活**：每 3 秒检测系统代理注册表，被第三方篡改时自动复位并弹窗警示。
6. **非正常断电/死机自愈恢复**：启动时扫描上次未正常清理的孤儿代理设置，启动自愈清理。
7. **局域网代理共享 (Allow-LAN)**：混合监听端口，绑定特定网卡 IP 地址。
8. **局域网接入 ACL 鉴权**：支持基于 IP/CIDR 白名单和 HTTP 基本认证的局域网安全准入。
9. **IPv6 内核转发拓扑开关**：一键禁用 IPv6 路由转发，杜绝双栈环境下的公网泄漏。
10. **Windows UWP 回环隔离解除工具**：列举 UWP 应用，一键免除 Loopback 隔离。
11. **PAC 动态代理脚本生成与本地服务**：自动生成标准 PAC 脚本，提供本地 HTTP PAC 服务端点。
12. **物理网卡漫游与默认网关感知**：Wi-Fi / 有线网络切换时自动触发 TUN 路由表平滑自愈。
13. **Android VpnService 移动端无缝穿透**：移动端原生 VPN 权限申请与前台服务保活。
14. **双端系统级开关 UI 表现 100% 对等**：Iced 与 Bevy 在侧栏和页面具备同等操作体验。
15. **特权网络无头回归测试**：mock 宿主适配器测试注入、清理与错误回滚。

#### 组 02 逐项账目

| 项 | 任务 | 状态 |
| :--- | :--- | :--- |
| `DUAL-02-01` | TUN 四堆栈安全调度 | `legacy-unassessed` |
| `DUAL-02-02` | 物理与虚拟网卡 MTU 自适应协商 | `legacy-unassessed` |
| `DUAL-02-03` | 严格路由与全局流量劫持 | `legacy-unassessed` |
| `DUAL-02-04` | 系统 HTTP/SOCKS 代理一键注入 | `legacy-unassessed` |
| `DUAL-02-05` | 系统代理被抢占实时探活 | `legacy-unassessed` |
| `DUAL-02-06` | 非正常断电/死机系统代理自愈恢复 | `legacy-unassessed` |
| `DUAL-02-07` | Allow-LAN 混合端口与绑定地址 | `legacy-unassessed` |
| `DUAL-02-08` | 局域网接入 ACL 与 HTTP 基本认证 | `legacy-unassessed` |
| `DUAL-02-09` | IPv6 内核流量与 TUN 转发策略开关 | `legacy-unassessed` |
| `DUAL-02-10` | Windows UWP 回环隔离解除工具 | `legacy-unassessed` |
| `DUAL-02-11` | PAC 动态代理脚本与本地服务 | `legacy-unassessed` |
| `DUAL-02-12` | 物理网卡漫游与默认网关感知 | `legacy-unassessed` |
| `DUAL-02-13` | Android VpnService 移动端无缝穿透 | `legacy-unassessed` |
| `DUAL-02-14` | 双端系统级开关 UI 表现 100% 对等 | `legacy-unassessed` |
| `DUAL-02-15` | 特权网络无头回归测试 | `legacy-unassessed` |

> **证据与边界**：TUN/系统代理/Allow-LAN/PAC/漫游/VPN 由 shared `*Port`/`*Application`/`*Snapshot` 承载，desktop/Android/iOS host 能力按 typed unsupported 声明；双端投影与行为由 `scripts/parity/` 解析，真实多网卡/权限/发行包 smoke 未计入 `host-verified`。

### 组 03：核心概览、双通道遥测中枢与动态拓扑链 (Overview & Telemetry)
1. **真实双通道流量波形 (GPU Bezier)**：贝塞尔样条平滑算法，实时绘制上下行速率曲线。
2. **动态量程标尺与发光着色器**：自适应 Y 轴最大刻度（KB/s、MB/s、GB/s），曲线下方渐变发光。
3. **分流链路可视化拓扑流动链**：`Inbound -> Sniffer -> RuleSet -> Proxy Group -> Outbound` 动态流动指示。
4. **拓扑节点下钻跳转交互**：点击 Inbound 跳设置，点击 RuleSet 跳规则，点击 Group 跳代理。
5. **主活动出口节点高保真卡片**：展示当前选中的出口节点名称、所属国旗、协议胶囊与测速延迟。
6. **订阅配额与临期动态仪表盘**：已用/总流量进度条、百分比、账单重置倒计时天数。
7. **配额三级预警机制**：配额超 85% 预警黄，超 95% 告警红，无订阅显示本地卡片。
8. **系统代理与 TUN 双主控大卡**：大卡片 Switch 开关，带微触觉动画与权限引导。
9. **代理运行模式即时分段控制器**：`Rule` / `Global` / `Direct` / `Script` 四态滑动胶囊。
10. **全局一键并发测速按钮**：概览页头部一键测速，带环形旋转百分比进度。
11. **核心资源 6 项运维网格**：连接数、内存、CPU、上行速率、下行速率、累计会话总流量。
12. **公网 IP 隐私归属探针**：展示真实外网出口 IP、国家城市徽标与 ISP 运营商，支持一键刷新。
13. **卡片模块长按纵向拖拽重排**：支持拖拽调整流量图、拓扑链、指标网格的先后顺序并持久化。
14. **断线与重载优雅降级蒙版**：内核重启期间界面保留上一帧有效快照，覆以“重载中”平滑蒙版。
15. **双端全视口响应式表现 1:1 对齐**：宽屏、平板、移动端 100% 保持信息层次一致。

#### 组 03 逐项账目

| 项 | 任务 | 状态 |
| :--- | :--- | :--- |
| `DUAL-03-01` | 真实双通道流量波形（GPU Bezier） | `legacy-unassessed` |
| `DUAL-03-02` | 动态量程标尺与发光着色器 | `legacy-unassessed` |
| `DUAL-03-03` | 分流链路可视化拓扑流动链 | `legacy-unassessed` |
| `DUAL-03-04` | 拓扑节点下钻跳转交互 | `legacy-unassessed` |
| `DUAL-03-05` | 主活动出口节点高保真卡片 | `legacy-unassessed` |
| `DUAL-03-06` | 订阅配额与临期动态仪表盘 | `legacy-unassessed` |
| `DUAL-03-07` | 系统代理与 TUN 模式双主控大卡 | `legacy-unassessed` |
| `DUAL-03-08` | 代理运行模式即时分段控制器 | `legacy-unassessed` |
| `DUAL-03-09` | 全局一键并发测速按钮 | `legacy-unassessed` |
| `DUAL-03-10` | 核心资源 6 项运维网格 | `legacy-unassessed` |
| `DUAL-03-11` | 公网 IP 隐私归属探针 | `legacy-unassessed` |
| `DUAL-03-12` | 卡片模块长按纵向拖拽重排 | `legacy-unassessed` |
| `DUAL-03-13` | 断线与重载优雅降级蒙版 | `legacy-unassessed` |
| `DUAL-03-14` | 双端全视口响应式表现 1:1 对齐 | `legacy-unassessed` |
| `DUAL-03-15` | 概览双端真实回归闭环 | `legacy-unassessed` |

> **证据与边界**：流量波形/量程/拓扑/出口/配额/主控/模式/测速/指标/探针/布局/蒙版由 shared snapshot + application 派生，双端只消费同一事实；GPU/长时/触控像素与发行包 smoke 未计入 `host-verified`。`DUAL-03-14` 由 [RESPONSIVE_PARITY_LEDGER.md](RESPONSIVE_PARITY_LEDGER.md) 权威跟踪。

### 组 04：代理策略、节点选择器与多维智能排序 (Proxies & Sorting)
1. **策略组 5 大分类全覆盖**：Selector（手动选择）、URLTest（自动测速）、Fallback（故障转移）、LoadBalance（负载均衡）、Relay（链式中继）。
2. **策略组展开/折叠状态持久化**：用户独立折叠常用策略组，重启后记忆折叠态。
3. **节点选择状态即时回写**：`PUT /proxies/{group}` 秒级下发，两端节点卡片高亮即时同步。
4. **节点死链一键隐藏 (Filter Alive)**：工具栏一键过滤超时（None）或未测试的不可用节点。
5. **四维排序控制器**：支持 `延迟升序`、`延迟降序`、`名称升序`、`名称降序` 即时重排并持久化。
6. **节点星标置顶与收藏**：点击节点卡片星标，该节点锁定在策略组首位，不被排序覆盖。
7. **协议与特性高级芯片**：节点卡片显式标注 `Shadowsocks`、`VLESS`、`Reality`、`Vision`、`UDP`、`TFO` 等。
8. **节点延迟多色阶渲染**：<100ms 翡翠绿，100-200ms 青草绿，200-300ms 暖黄，>300ms 警戒红，超时灰色。
9. **单节点历史延迟 Sparkline 走势图**：节点卡片集成最近 10 次采样微折线走势图。
10. **智能拼音与协议模糊检索**：支持汉字、拼音首字母（如 `xg` 匹配 `香港`）及协议类型过滤。
11. **单节点详情下钻抽屉**：展示服务器域名、落地 IP、加密方式、历史 RTT 波动区间。
12. **策略组自定义拖拽调序**：长按策略组卡片可调整展示顺序，并支持一键恢复默认。
13. **节点卡片网格与紧凑列表无缝切换**：支持 2~4 列响应式网格与单列高密度列表一键切换。
14. **测速动态脉冲骨架屏占位**：节点测速期间数值显示平滑波纹骨架屏，测速完毕淡入延迟。
15. **双端代理操作无头行为测试闭环**：双端断言排序、置顶、测速状态与组选择。

#### 组 04 逐项账目

| 项 | 任务 | 状态 |
| :--- | :--- | :--- |
| `DUAL-04-01` | 策略组 5 大分类全覆盖 | `legacy-unassessed` |
| `DUAL-04-02` | 策略组展开/折叠状态持久化 | `legacy-unassessed` |
| `DUAL-04-03` | 节点选择状态即时回写 | `legacy-unassessed` |
| `DUAL-04-04` | 节点死链一键隐藏 (Filter Alive) | `legacy-unassessed` |
| `DUAL-04-05` | 四维排序控制器 | `legacy-unassessed` |
| `DUAL-04-06` | 节点星标置顶与收藏 | `legacy-unassessed` |
| `DUAL-04-07` | 协议与特性高级芯片 | `legacy-unassessed` |
| `DUAL-04-08` | 节点延迟多色阶渲染 | `legacy-unassessed` |
| `DUAL-04-09` | 单节点历史延迟 Sparkline 走势图 | `legacy-unassessed` |
| `DUAL-04-10` | 智能拼音与协议模糊检索 | `legacy-unassessed` |
| `DUAL-04-11` | 单节点详情下钻抽屉 | `legacy-unassessed` |
| `DUAL-04-12` | 策略组自定义拖拽调序 | `legacy-unassessed` |
| `DUAL-04-13` | 节点卡片网格与紧凑列表无缝切换 | `legacy-unassessed` |
| `DUAL-04-14` | 测速动态脉冲骨架屏占位 | `legacy-unassessed` |
| `DUAL-04-15` | 双端代理真实回归闭环 | `legacy-unassessed` |

> **证据与边界**：策略组分类/折叠/选点/过滤/排序/收藏/芯片/色阶/Sparkline/检索/详情/调序/视图/骨架由 shared `ProxyUiPreferences` 与 domain 归约承载；双端行为由 `scripts/parity/` 解析，真实大列表/触控/发行包 smoke 未计入 `host-verified`。

### 组 05：协议生态保真、链式代理与多路复用 (Protocols & Transport)
1. **Shadowsocks 2022 全密码族**：2022-blake3-aes-128-gcm, 2022-blake3-aes-256-gcm, 2022-blake3-chacha20-poly1305。
2. **VLESS 进阶特性保真**：XTLS-Reality (`pbk`, `sid`, `spx`), Vision 流控 (`xtls-rprx-vision`), uTLS 指纹伪装。
3. **TUIC v5 与 Hysteria 2 拥塞控制**：支持 TUIC v5 BBR/Cubic，支持 Hysteria 2 端口跳跃 (`ports`) 与 masquerade。
4. **WireGuard / AmneziaWG 全参数**：支持 `preshared-key`, `reserved` 混淆字节、AmneziaWG 参数 (`jc/s1/h1-h4`)。
5. **现代传输层覆盖**：XHTTP (SplitHTTP), gRPC (multi-mode/service-name), 原生 QUIC, WebSocket Early Data (0-RTT)。
6. **SIP003 插件链生态**：`v2ray-plugin`, `obfs-local`, `shadow-tls v3`, `simple-obfs` 解析与协同。
7. **原生 SSH SOCKS 代理**：支持 `user`, `private-key`, `passphrase`, `host-key-algorithms`。
8. **AnyTLS 与 Trojan-Go 接入**：支持无特征 TLS 握手与混淆传输。
9. **前置跳板代理 (Dialer-Proxy)**：可视化配置节点作为前置跳板代理链路，支持链路拓扑展示。
10. **环路依赖静态检测**：检测 Dialer-Proxy 与 Relay 策略组之间的死循环配置并警示阻断。
11. **多路复用 (Smux / Yamux / H2Mux) 调优**：细化 `max-connections`, `min-streams`, `padding`, `brutal-opts`。
12. **TLS/ECH 与多版本 ALPN 协商**：支持 ECH 扩展注入、ALPN 多版本协商 (`h3, h2, http/1.1`)。
13. **自定义 CA 证书与证书白名单**：支持导入自签 CA 证书，避免内网或特定节点证书报错。
14. **双向无损格式转换**：URI、JSON、Clash YAML 互转过程中保持 100% 结构保真，未知字段无损流通。
15. **协议解析与序列化全量单测**：全协议解析无头覆盖。

#### 组 05 逐项账目

| 项 | 任务 | 状态 |
| :--- | :--- | :--- |
| `DUAL-05-01` | Shadowsocks 2022 全密码族 | `legacy-unassessed` |
| `DUAL-05-02` | VLESS 进阶特性保真（Reality/Vision/uTLS） | `legacy-unassessed` |
| `DUAL-05-03` | TUIC v5 与 Hysteria 2 拥塞控制/端口跳跃/masquerade | `legacy-unassessed` |
| `DUAL-05-04` | WireGuard / AmneziaWG 全参数 | `legacy-unassessed` |
| `DUAL-05-05` | 现代传输层（XHTTP/gRPC/QUIC/WS 0-RTT） | `legacy-unassessed` |
| `DUAL-05-06` | SIP003 插件链生态 | `legacy-unassessed` |
| `DUAL-05-07` | 原生 SSH SOCKS 代理 | `legacy-unassessed` |
| `DUAL-05-08` | AnyTLS 与 Trojan-Go 接入 | `legacy-unassessed` |
| `DUAL-05-09` | 前置跳板代理（Dialer-Proxy）链路 | `legacy-unassessed` |
| `DUAL-05-10` | 环路依赖静态检测 | `legacy-unassessed` |
| `DUAL-05-11` | 多路复用（Smux/Yamux/H2Mux）调优 | `legacy-unassessed` |
| `DUAL-05-12` | TLS/ECH 与多版本 ALPN 协商 | `legacy-unassessed` |
| `DUAL-05-13` | 自定义 CA 证书与证书白名单 | `legacy-unassessed` |
| `DUAL-05-14` | 双向无损格式转换（URI/JSON/Clash YAML） | `legacy-unassessed` |
| `DUAL-05-15` | 协议解析与序列化全量单测 | `legacy-unassessed` |

> **证据与边界**：协议保真/参数/链路/CA 由 shared contract + domain + application 投影，双端只渲染同一 studio 报告；pinned core 无 `quic`/`xhttp` 传输、无 `trojan-go` 节点类型，`masquerade` 仅 listener 字段，均以非阻断 note 诚实标注；分享链接不携带跳板与证书信任。

### 组 06：并发测速引擎、丢包抖动雷达与稳定性评估 (Speedtest & Jitter)
1. **信号量流控并发测速**：基于 `Semaphore(30)` 流控，批量测速分批发出，防止并发风暴与熔断。
2. **单策略组独立测速**：支持只对当前展开的单个策略组测速，无需全量重测。
3. **测速目标 URL 动态自定义**：支持用户配置测速 URL（如 Cloudflare, Google, 延迟测试探针）。
4. **真实下行带宽测速**：支持对单节点发起多线程实际分片拉取，测定真实 Mbps 带宽。
5. **网络抖动 (Jitter ms) 精确计算**：多次高频往返探测，计算 RTT 标准方差与抖动率。
6. **丢包率 (Packet Loss) 梯度评级**：采样探测丢包比例，标明 0%（极佳）、<5%（良好）、>20%（较差）。
7. **五星稳定性综合雷达评分**：综合延迟、抖动、丢包、历史掉线率计算节点健康得分。
8. **测速进度环形百分比动画**：测速按钮旁实时旋转展示测速完成进度（如 `42/128`）。
9. **超时与不可用节点即时归档**：握手失败或超时的节点自动淡出并落入底部不可用区。
10. **测速取消与安全中断**：测速进行中支持点击“取消测速”，安全丢弃尚未发出的测速任务。
11. **历史测速数据持久化缓存**：保存最近 3 次测速结果，应用重启后不丢失上一次测试态。
12. **节点真实 IP 与出口探测对比**：测速同时探活出站真实落地 IP 与预期落地国家是否相符。
13. **测速结果弹窗详细透视**：点击测速详情，弹出综合雷达图与多维指标卡片。
14. **双端测速状态机与动效一致**：Iced 与 Bevy 测速动画、进度反馈与数据投影完全对齐。
15. **测速流控与状态机无头测试**：并发调度、超时处理与取消流程 100% 测试覆盖。

#### 组 06 逐项账目

| 项 | 任务 | 状态 |
| :--- | :--- | :--- |
| `DUAL-06-01` | 信号量流控并发测速（Semaphore 30） | `legacy-unassessed` |
| `DUAL-06-02` | 单策略组独立测速 | `legacy-unassessed` |
| `DUAL-06-03` | 测速目标 URL 动态自定义 | `legacy-unassessed` |
| `DUAL-06-04` | 真实下行带宽测速 | `legacy-unassessed` |
| `DUAL-06-05` | 网络抖动 (Jitter ms) 精确计算 | `legacy-unassessed` |
| `DUAL-06-06` | 丢包率梯度评级 | `legacy-unassessed` |
| `DUAL-06-07` | 五星稳定性综合雷达评分 | `legacy-unassessed` |
| `DUAL-06-08` | 测速进度环形百分比动画 | `legacy-unassessed` |
| `DUAL-06-09` | 超时与不可用节点即时归档 | `legacy-unassessed` |
| `DUAL-06-10` | 测速取消与安全中断 | `legacy-unassessed` |
| `DUAL-06-11` | 历史测速数据持久化缓存 | `legacy-unassessed` |
| `DUAL-06-12` | 节点真实 IP 与出口探测对比 | `legacy-unassessed` |
| `DUAL-06-13` | 测速结果弹窗详细透视 | `legacy-unassessed` |
| `DUAL-06-14` | 双端测速状态机与动效一致 | `legacy-unassessed` |
| `DUAL-06-15` | 测速流控与状态机无头测试 | `legacy-unassessed` |

> **证据与边界**：并发/目标/带宽/抖动/丢包/星级/进度/归档/取消/历史/出口/明细/矩阵由 shared `SpeedtestApplication` 唯一拥有，host 无引擎时 typed unsupported；历史上限 3；双端状态机由 `scripts/parity/` 解析。

### 组 07：订阅管理、多渠道导入与定时更新流水线 (Subscriptions Lifecycle)
1. **多渠道导入三合一**：支持 URL 远程拉取、本地文件导入、剪贴板一键解析导入。
2. **自定义单个订阅 User-Agent**：支持全局及单个 Profile 独立定制请求头 `User-Agent`，防止防爬阻断。
3. **定时自动轮询与 Cron 表达式**：后台调度器支持 6h/12h/24h 及自定义 Cron 表达式自动刷新。
4. **条件请求 (ETag / If-Modified-Since)**：服务端配置未变时返回 304 Not Modified，零流量开销。
5. **网络重试与指数退避机制**：更新失败后 30s、1m、5m 自动退避重试，杜绝网络波动闪崩。
6. **单飞防重入调度 (Single Flight)**：订阅更新正在进行时，再次触发自动合并入当前管道，防止并发重复下载。
7. **订阅套餐用量与到期预警**：解析 `Subscription-Userinfo` 头，用量 > 85% 预警，到期 < 3 天标橙提示。
8. **订阅节点关键词清洗管道**：支持白名单、黑名单、协议过滤及正则批量重命名。
9. **更新后自动重启核心可选**：配置选项，订阅更新完成后自动触发核心平滑热重载。
10. **订阅更新静默系统通知**：支持订阅自动更新成功/失败后推送操作系统原生通知。
11. **一键手动更新全部订阅**：工具栏一键触发所有已配置订阅并发刷新，展示总进度。
12. **安全证书跳过选项 (Insecure Skip Verify)**：单个订阅支持独立关闭 TLS 证书校验。
13. **配置源文件安全备份**：订阅更新写入前自动生成 `.bak` 文件，写入失败自动恢复。
14. **双端订阅管理交互 1:1 对等**：Iced 与 Bevy 具备相同卡片、状态回显与操作弹窗。
15. **订阅更新流水线无头测试**：ETag 响应、解析失败回滚与定时调度单测完备。

#### 组 07 逐项账目

| 项 | 任务 | 状态 |
| :--- | :--- | :--- |
| `DUAL-07-01` | 多渠道导入三合一（URL/本地/剪贴板） | `legacy-unassessed` |
| `DUAL-07-02` | 自定义单个订阅 User-Agent | `legacy-unassessed` |
| `DUAL-07-03` | 定时自动轮询与 Cron 表达式 | `legacy-unassessed` |
| `DUAL-07-04` | 条件请求 ETag / If-Modified-Since | `legacy-unassessed` |
| `DUAL-07-05` | 网络重试与指数退避（30s/1m/5m） | `legacy-unassessed` |
| `DUAL-07-06` | 单飞防重入调度 (Single Flight) | `legacy-unassessed` |
| `DUAL-07-07` | 订阅用量与到期三级预警 | `legacy-unassessed` |
| `DUAL-07-08` | 订阅节点关键词清洗管道 | `legacy-unassessed` |
| `DUAL-07-09` | 更新后自动重启核心可选 | `legacy-unassessed` |
| `DUAL-07-10` | 订阅更新静默系统通知 | `legacy-unassessed` |
| `DUAL-07-11` | 一键手动更新全部订阅 | `legacy-unassessed` |
| `DUAL-07-12` | 安全证书跳过 (Insecure Skip Verify) | `legacy-unassessed` |
| `DUAL-07-13` | 配置源文件安全备份 | `legacy-unassessed` |
| `DUAL-07-14` | 双端订阅管理交互 1:1 对等 | `legacy-unassessed` |
| `DUAL-07-15` | 订阅更新流水线无头测试 | `legacy-unassessed` |

> **证据与边界**：导入/UA/排程/条件请求/退避/单飞/配额/清洗/重载/通知/批量/证书/备份由 shared application 承载；Bevy 桌面组合根尚未安装 `with_managed_runtime`，Bevy 端「启用内核重载」按 typed unsupported 处理（Iced 桌面完整可用）。

### 组 08：多源订阅聚合器、节点清洗与自动拓扑生成 (Profile Aggregator)
1. **多订阅源勾选聚合向导**：可视化勾选多个本地/远程订阅配置作为源。
2. **跨订阅节点自动去重 (Deduplication)**：按 `Server + Port + Protocol` 唯一指纹自动去重相同节点。
3. **区域节点自动归类 (Geo Clustering)**：按 🇭🇰 🇯🇵 🇺🇸 🇸🇬 🇩🇪 等 ISO 国家代码自动分类成组。
4. **自动生成区域测速策略组**：自动生成“香港自动测速”、“日本自动测速”等专属 URLTest 策略组。
5. **主选择器自动级联 (Master Cascade)**：自动生成主 PROXIES 策略组并级联各国家分组。
6. **聚合后生成新独立 Profile**：不破坏原有订阅源，合并后保存为全新活动 Profile。
7. **一键保持源订阅联动更新**：源订阅更新后，聚合 Profile 支持一键触发重新聚合。
8. **自定义节点重命名规则**：聚合时支持正则替换节点前后缀（如统一去除广告后缀）。
9. **节点可用性预检与过滤**：聚合前预检节点必填字段，自动过滤缺失端口或秘钥的非法节点。
10. **自定义新策略组拓扑编排**：允许用户在向导中追加“流媒体专用”、“游戏专用”自定义策略组。
11. **聚合生成结果可视化预览**：保存前展示聚合后生成的 YAML 结构与策略组拓扑树。
12. **一键设为当前活动配置**：聚合成功后支持一键激活并热载入内核。
13. **历史聚合模板保存与复用**：记住用户的聚合勾选项与重命名配置，下次无需重复配置。
14. **双端聚合器模态 100% 对等**：Iced (`aggregator_modal.rs`) 与 Bevy (`profiles_aggregator.rs`) 互有证据。
15. **聚合器引擎全链路行为测试**：多源合并、去重、分组生成与持久化无头测试覆盖。

#### 组 08 逐项账目

| 项 | 任务 | 状态 |
| :--- | :--- | :--- |
| `DUAL-08-01` | 多订阅源勾选聚合向导 | `legacy-unassessed` |
| `DUAL-08-02` | 跨订阅节点自动去重（Server+Port+Protocol 指纹） | `legacy-unassessed` |
| `DUAL-08-03` | 区域节点自动归类（ISO 国家代码） | `legacy-unassessed` |
| `DUAL-08-04` | 自动生成区域测速策略组 | `legacy-unassessed` |
| `DUAL-08-05` | 主选择器自动级联（Master Cascade） | `legacy-unassessed` |
| `DUAL-08-06` | 聚合后生成新独立 Profile | `legacy-unassessed` |
| `DUAL-08-07` | 一键保持源订阅联动更新 | `legacy-unassessed` |
| `DUAL-08-08` | 自定义节点重命名规则 | `legacy-unassessed` |
| `DUAL-08-09` | 节点可用性预检与过滤 | `legacy-unassessed` |
| `DUAL-08-10` | 自定义新策略组拓扑编排 | `legacy-unassessed` |
| `DUAL-08-11` | 聚合生成结果可视化预览 | `legacy-unassessed` |
| `DUAL-08-12` | 一键设为当前活动配置 | `legacy-unassessed` |
| `DUAL-08-13` | 历史聚合模板保存与复用 | `legacy-unassessed` |
| `DUAL-08-14` | 双端聚合器模态 100% 对等 | `legacy-unassessed` |
| `DUAL-08-15` | 聚合器引擎全链路行为测试 | `legacy-unassessed` |

> **证据与边界**：多源聚合/去重/聚类/组生成/级联/落盘/联动/重命名/预检/自定义组/预览/激活/模板由 domain `ProfileAggregator` + application 唯一拥有；预检样本上限 8；模板 sidecar 缺省 typed unsupported。

### 组 09：AST YAML 深度配置引擎、智能感知与版本快照 Diff (Config AST & Diff)
1. **100% 保真 YAML AST 引擎**：基于 AST 操作配置，保留注释、空行与 YAML 锚点 (`&/*`)，杜绝抹除。
2. **Monaco 级代码编辑器视口**：等宽字体、行号高亮、缩进参考线与视口滚动。
3. **YAML 语法实时预检与行号定位**：输入语法错误实时在出错行标注红色波浪线与精确行号。
4. **常用代码片段一键插入 (Snippets)**：支持快速插入 SS/VLESS/Trojan 节点、策略组与分流规则模板。
5. **代码一键格式化 (Format YAML)**：按照 Clash 规范美化排版与键名排序。
6. **配置自动历史快照备份**：每次 Apply 成功自动备份 snapshot，按时间与哈希记录，最多保留 20 份。
7. **历史快照自动智能修剪**：重复配置去重，超出上限自动按 LRU 淘汰旧快照。
8. **配置历史快照可视化并排/行内 Diff**：绿底 `+ Added`、红底 `- Removed`、黄底 `~ Modified` 逐行对比。
9. **快照一键安全回滚 (One-Click Rollback)**：选定历史快照后一键覆写回当前配置，带防呆二次确认。
10. **多配置快速切换 (Switch Profile)**：一键在不同配置方案间无感切换并秒级生效。
11. **配置无效时自动触发安全回滚**：新配置应用后若核心健康检查失败，自动撤销变更还原上一版本。
12. **只读保护与远程订阅防手滑覆写**：远程订阅默认进入保护态，提示用户通过 Mixin 覆写而非直接编辑。
13. **大文件编辑器性能优化**：编辑 10,000+ 行配置时维持 60 FPS 滚动，杜绝卡顿。
14. **双端编辑器与 Diff 模态完全镜像**：Iced (`snapshot_diff_modal.rs`) 与 Bevy (`profiles_diff.rs`) 保持一致。
15. **YAML 引擎与回滚事务无头测试**：注释保留、语法报错与回滚事务测试 100% 覆盖。

#### 组 09 逐项账目

| 项 | 任务 | 状态 |
| :--- | :--- | :--- |
| `DUAL-09-01` | 100% 保真 YAML AST 引擎 | `legacy-unassessed` |
| `DUAL-09-02` | Monaco 级代码编辑器视口 | `legacy-unassessed` |
| `DUAL-09-03` | YAML 语法实时预检与行号定位 | `legacy-unassessed` |
| `DUAL-09-04` | 常用代码片段一键插入 (Snippets) | `legacy-unassessed` |
| `DUAL-09-05` | 代码一键格式化 (Format YAML) | `legacy-unassessed` |
| `DUAL-09-06` | 配置自动历史快照备份 | `legacy-unassessed` |
| `DUAL-09-07` | 快照去重与 LRU 管理 | `legacy-unassessed` |
| `DUAL-09-08` | 快照 Diff 检查器 | `legacy-unassessed` |
| `DUAL-09-09` | 快照安全恢复 | `legacy-unassessed` |
| `DUAL-09-10` | 多配置快速切换 (Switch Profile) | `legacy-unassessed` |
| `DUAL-09-11` | 配置无效时自动触发安全回滚 | `legacy-unassessed` |
| `DUAL-09-12` | 只读保护与远程订阅防手滑覆写 | `legacy-unassessed` |
| `DUAL-09-13` | 大文件编辑器性能优化 | `legacy-unassessed` |
| `DUAL-09-14` | 双端编辑器与 Diff 模态完全镜像 | `legacy-unassessed` |
| `DUAL-09-15` | YAML 引擎与回滚事务无头测试 | `legacy-unassessed` |

> **证据与边界**：AST 保真/预检/片段/格式化/快照/Diff/恢复/切换/回滚/保护由 shared domain + application 承载；filter 与 deep-merge 仍走结构级 serde，L3 只承诺「不触碰即不变」；编辑器为有界窗口而非虚拟滚动。

### 组 10：脚本沙箱生态、QuickJS 调试控制台与多级混入 (Scripting & Mixin)
1. **QuickJS 嵌入式轻量执行沙箱**：内置纯 Rust QuickJS 引擎，无 node/外部环境依赖。
2. **Pre-Process / Post-Process 钩子**：支持 `onProfileProcess(config)` 订阅处理钩子。
3. **沙箱资源熔断安全防护**：硬性限制 64MB 最大内存分配、500ms 最大执行时间，超时自动中断防死锁。
4. **内置三大官方常用脚本模板**：国家地区自动成组模板、流媒体分流覆写模板、内网直连穿透模板。
5. **实时代码调试控制台视口**：左侧代码编辑区，右上预设选择，右下实时控制台日志回显。
6. **`console.log` 流式捕获与拦截**：脚本运行输出即时投射到控制台日志区，便于调试。
7. **AST 变换前后实时对比预览**：输入 YAML 与脚本变换后输出的 YAML 实时渲染对比。
8. **多级配置覆写流水线 (Cascade Pipeline)**：Base Profile -> 订阅配置 -> Merge 规则 -> 全局 Mixin。
9. **三栏式 Mixin 编辑器**：左侧 Base 配置、中间 Mixin 覆写块、右侧合成后最终配置实时预览。
10. **Mixin 脚本语法检查与错误阻断**：脚本解析异常时阻止覆写生效，保障内核稳定运行。
11. **常用 Mixin 预设一键开关**：常用覆写项（如开启 IPv6、注入自定义 DNS）独立卡片开关。
12. **扩展脚本导出与社区分享**：支持将调试成功的脚本导出为独立 `.js` 文件。
13. **异常处理安全降级**：脚本报错时不影响原有配置基础运行，弹出告警通知。
14. **双端脚本控制台与 Mixin 视口对齐**：Iced (`script_console.rs`) 与 Bevy (`profiles_script_workbench.rs` / `profiles_script_view.rs`) 对等呈现。
15. **QuickJS 引擎与沙箱熔断单测**：超时熔断、内存限制与 AST 变换测试 100% 覆盖。

#### 组 10 逐项账目

| 项 | 任务 | 状态 |
| :--- | :--- | :--- |
| `DUAL-10-01` | ECMAScript 嵌入式轻量执行沙箱 | `legacy-unassessed` |
| `DUAL-10-02` | Pre/Post-Process 钩子 | `legacy-unassessed` |
| `DUAL-10-03` | 沙箱资源熔断安全防护（64MB/500ms） | `legacy-unassessed` |
| `DUAL-10-04` | 内置三大官方常用脚本模板 | `legacy-unassessed` |
| `DUAL-10-05` | 实时代码调试控制台视口 | `legacy-unassessed` |
| `DUAL-10-06` | `console.log` 流式捕获与拦截 | `legacy-unassessed` |
| `DUAL-10-07` | AST 变换前后实时对比预览 | `legacy-unassessed` |
| `DUAL-10-08` | 多级配置覆写流水线 (Cascade Pipeline) | `legacy-unassessed` |
| `DUAL-10-09` | 三栏式 Mixin 编辑器 | `legacy-unassessed` |
| `DUAL-10-10` | Mixin 脚本语法检查与错误阻断 | `legacy-unassessed` |
| `DUAL-10-11` | 常用 Mixin 预设一键开关 | `legacy-unassessed` |
| `DUAL-10-12` | 扩展脚本导出与社区分享 | `legacy-unassessed` |
| `DUAL-10-13` | 异常处理安全降级 | `legacy-unassessed` |
| `DUAL-10-14` | 双端脚本控制台与 Mixin 视口对齐 | `legacy-unassessed` |
| `DUAL-10-15` | QuickJS 引擎与沙箱熔断单测 | `legacy-unassessed` |

> **证据与边界**：引擎为纯 Rust `boa_engine`（**非 QuickJS**）且已置默认特性，指令 DSL 仍是每宿主默认选用引擎；沙箱矩阵真实执行 15 项；无真实 QuickJS 引擎，绝不冒充。

### 组 11：分流规则引擎、MRS 二进制加速与逻辑子规则 (Rules & Rule-Providers)
1. **28+ 规则类型全矩阵支持**：DOMAIN, DOMAIN-SUFFIX, DOMAIN-KEYWORD, IP-CIDR, SRC-IP-CIDR, GEOIP, GEOSITE, PROCESS-NAME, PROCESS-PATH, DSCP, UID 等。
2. **逻辑组合子规则 (Logic Rules) 递归构建**：支持 `AND`, `OR`, `NOT`, `SUB-RULE` 多层嵌套条件判定。
3. **MRS 官方二进制规则集高性能适配**：全面支持 Mihomo `.mrs` 格式，内存零拷贝极速匹配。
4. **Rule-Provider 外部规则集生命周期管理**：展示来源 URL、行为模式（domain/ipcidr/classical）、更新时间。
5. **外部规则集增量更新与 ETag 缓存**：支持手动触发更新、定时自动更新与 304 条件缓存。
6. **规则集一键解构导入 (Unpack Provider)**：将远程 Rule-Provider 规则条目一键解构为本地可编辑规则。
7. **规则集本地缓存一键清理**：支持清理已下载的规则集本地缓存文件，释放磁盘空间。
8. **分流规则列表 50,000+ 条目虚拟视口滚动**：$O(1)$ 几何裁剪，维持 60 FPS 丝滑滚动。
9. **单规则一键停用/启用 Switch**：不删除规则前提下临时禁用某条规则，即时生效。
10. **规则拖拽调序与优先级置顶**：长按拖拽把手调整规则上下顺序，自动重排优先级。
11. **快速新增自定义规则表单向导**：规则类型、匹配内容、目标策略组结构化表单录入。
12. **一键注入游戏分流预设规则集**：内置 Steam, Epic, Riot, Blizzard 常见游戏平台分流预设。
13. **规则列表关键词模糊搜索与分页**：按匹配表达式、类型与目标出站即时搜索过滤。
14. **双端规则管理视口与组件 1:1 对等**：Iced (`rules.rs`) 与 Bevy (`rules_mrs.rs`) 完整闭环。
15. **规则引擎与 MRS 解析无头测试**：多类型规则匹配、逻辑子规则评估与解构无头断言。

#### 组 11 逐项账目

| 项 | 任务 | 状态 |
| :--- | :--- | :--- |
| `DUAL-11-01` | 28+ 规则类型全矩阵支持 | `legacy-unassessed` |
| `DUAL-11-02` | 逻辑组合子规则递归构建（AND/OR/NOT/SUB-RULE） | `legacy-unassessed` |
| `DUAL-11-03` | MRS 官方二进制规则集高性能适配（元数据/校验/状态） | `legacy-unassessed` |
| `DUAL-11-04` | Rule-Provider 生命周期管理（来源 URL/行为/更新时间） | `legacy-unassessed` |
| `DUAL-11-05` | 外部规则集增量更新与 ETag 缓存 | `legacy-unassessed` |
| `DUAL-11-06` | 规则集一键解构导入 (Unpack Provider) | `legacy-unassessed` |
| `DUAL-11-07` | 规则集本地缓存一键清理 | `legacy-unassessed` |
| `DUAL-11-08` | 50,000+ 条目虚拟视口滚动 | `legacy-unassessed` |
| `DUAL-11-09` | 单规则一键停用/启用 Switch | `legacy-unassessed` |
| `DUAL-11-10` | 规则拖拽调序与优先级置顶 | `legacy-unassessed` |
| `DUAL-11-11` | 快速新增自定义规则表单向导 | `legacy-unassessed` |
| `DUAL-11-12` | 一键注入游戏分流预设规则集 | `legacy-unassessed` |
| `DUAL-11-13` | 规则列表关键词模糊搜索与分页 | `legacy-unassessed` |
| `DUAL-11-14` | 双端规则管理视口与组件 1:1 对等 | `legacy-unassessed` |
| `DUAL-11-15` | 规则引擎与 MRS 解析无头测试 | `legacy-unassessed` |

> **证据与边界**：规则类型/逻辑子规则/MRS/provider 生命周期/解构/缓存清理/虚拟窗口/启停/调序/向导/预设/搜索分页由 shared domain + application 承载；逐请求 304 只在 mihomo 内核内，客户端只提供本地内容指纹（非 HTTP ETag）。

### 组 12：交互式实时分流追踪器 (Live Rule Tracer) 与命中审计 (Rule Tracer)
1. **交互式分流追踪沙盒视口**：输入目标（域名/IP）、端口、进程名与来源网络，立即模拟分流匹配。
2. **分流决策链树状回放**：完整回放命中规则序号、匹配表达式、所属规则集与出站策略决策。
3. **快捷测试预设域名芯片**：提供 `google.com`, `github.com`, `bilibili.com`, `1.1.1.1` 一键测试。
4. **规则命中实时流计数 (Hit Counter)**：基于内核事件流实时累加每条规则的命中频次。
5. **冷门死规则静态诊断**：识别命中次数为 0 的冷门规则与被上层完全覆写的死规则。
6. **IP-CIDR 掩码重叠与冲突检测**：静态分析下层 IP 规则被上层大掩码规则拦截的逻辑漏洞。
7. **命中时间戳记录**：展示该规则最近一次被命中的时间（如 `刚刚`、`5分钟前`）。
8. **分流结果一键反向应用**：追踪结果展示若不符合预期，提供“修改此规则出站”直达按钮。
9. **规则时延贡献审计**：统计不同分流策略路径平均网络时延贡献。
10. **仿真沙盒环境参数模拟**：支持模拟特定来源 IP（内网某台设备）发起的分流判定。
11. **一键清空规则命中计数**：支持重置累计计数器，重新统计会话命中情况。
12. **规则命中高亮闪烁动效**：在规则列表中实时高亮闪烁刚被命中的规则行。
13. **离线分流追踪支持**：在内核离线状态下利用本地 AST 逻辑规则树执行纯离线模拟。
14. **双端 Tracer 沙盒组件完全镜像**：Iced (`rules_tracer.rs`) 与 Bevy (`rules_tracer.rs`) 对等挂载。
15. **Tracer 判定算法无头断言覆盖**：域名后缀、关键字、IP 掩码追踪测试 100% 绿灯。

#### 组 12 逐项账目

| 项 | 任务 | 状态 |
| :--- | :--- | :--- |
| `DUAL-12-01` | 交互式分流追踪沙盒视口 | `legacy-unassessed` |
| `DUAL-12-02` | 分流决策链树状回放 | `legacy-unassessed` |
| `DUAL-12-03` | 快捷测试预设域名芯片 | `legacy-unassessed` |
| `DUAL-12-04` | 规则命中实时流计数 (Hit Counter) | `legacy-unassessed` |
| `DUAL-12-05` | 冷门死规则静态诊断 | `legacy-unassessed` |
| `DUAL-12-06` | IP-CIDR 掩码重叠与冲突检测 | `legacy-unassessed` |
| `DUAL-12-07` | 命中时间戳记录 | `legacy-unassessed` |
| `DUAL-12-08` | 分流结果一键反向应用（修改此规则出站） | `legacy-unassessed` |
| `DUAL-12-09` | 规则时延贡献审计 | `legacy-unassessed` |
| `DUAL-12-10` | 仿真沙盒环境参数模拟（来源 IP） | `legacy-unassessed` |
| `DUAL-12-11` | 一键清空规则命中计数 | `legacy-unassessed` |
| `DUAL-12-12` | 规则命中高亮闪烁动效 | `legacy-unassessed`（数据面） |
| `DUAL-12-13` | 离线分流追踪支持 | `legacy-unassessed` |
| `DUAL-12-14` | 双端 Tracer 沙盒组件完全镜像 | `legacy-unassessed` |
| `DUAL-12-15` | Tracer 判定算法无头断言覆盖 | `legacy-unassessed` |

> **证据与边界**：追踪/决策链/预设/命中计数/死规则/CIDR 重叠/时间戳/反向应用/时延/来源 IP 由 shared `RuleTracerApplication` 唯一拥有；命中高亮像素动效属 `local`；反向应用经核心原子事务。

### 组 13：实时连接审计、多维聚合透视与深度链路瀑布流 (Connections & Telemetry)
1. **高并发实时连接列表流式采集**：WebSocket 推流源/目/进程/规则/出站/速率/累计流量。
2. **多维聚合视图无缝切换**：扁平流视图 (Flat)、按应用进程聚合 (By Process)、按目标域名聚合 (By Host)。
3. **单连接详情 Slide-out 侧滑下钻抽屉**：右侧平滑展开 420px 详情抽屉，带半透明遮罩。
4. **耗时瀑布流 (Timing Waterfall)**：DNS 解析耗时、TCP 握手耗时、TLS 握手耗时、TTFB 首字节耗时色条。
5. **目标 IP、ASN 组织归属与地理透视**：展示目标落地 IP、AS 编号、所属组织机构（如 `AS36459 GitHub`）。
6. **完整路由链溯源**：详细呈现该连接从 Inbound 到 Outbound 的每一跳策略组名称。
7. **连接实时治理与切断**：支持一键断开当前单条连接、一键切断当前过滤筛选结果中的连接。
8. **一键关闭全部活动连接 (Close All)**：带防呆二次确认，一键清除所有会话。
9. **反向一键生成规则向导**：在连接详情中，一键将目标域名/IP 添加到分流规则（DIRECT/REJECT/PROXY）。
10. **高吞吐连接脉冲微光指示**：瞬时带宽超过 5MB/s 的连接行呈现发光呼吸微动效。
11. **空闲连接智能清退 (Idle Sweeper)**：可配置自动清理超过 10 分钟无数据传输的死连接。
12. **按上传/下载瞬时速率实时排序**：支持表头点击按瞬时带宽动态重新排列连接行。
13. **连接关键词即时搜索**：支持按域名、IP、进程名输入关键字即时过滤。
14. **双端连接抽屉与瀑布流 1:1 对等**：Iced (`connection_drawer.rs`) 与 Bevy (`connections_drawer.rs`) 保持一致。
15. **连接数据流与治理命令无头测试**：单连断开、全连切断与聚合计算测试 100% 覆盖。

#### 组 13 逐项账目

| 项 | 任务 | 状态 |
| :--- | :--- | :--- |
| `DUAL-13-01` | 高并发实时连接列表流式采集 | `legacy-unassessed` |
| `DUAL-13-02` | 多维聚合视图（Flat/按进程/按域名） | `legacy-unassessed` |
| `DUAL-13-03` | 单连接详情侧滑下钻抽屉 | `legacy-unassessed` |
| `DUAL-13-04` | 耗时瀑布流（DNS/TCP/TLS/TTFB） | `planned` |
| `DUAL-13-05` | 目标 IP、ASN 归属机构与地理透视 | `legacy-unassessed` |
| `DUAL-13-06` | 完整路由链溯源 | `legacy-unassessed` |
| `DUAL-13-07` | 连接实时治理（单条/过滤范围切断） | `legacy-unassessed` |
| `DUAL-13-08` | 一键关闭全部（二次确认） | `legacy-unassessed` |
| `DUAL-13-09` | 反向一键生成规则向导 | `legacy-unassessed` |
| `DUAL-13-10` | 高吞吐连接脉冲微光指示 | `legacy-unassessed` |
| `DUAL-13-11` | 空闲连接智能清退 | `legacy-unassessed` |
| `DUAL-13-12` | 按上传/下载瞬时速率排序 | `legacy-unassessed` |
| `DUAL-13-13` | 连接关键词即时搜索 | `legacy-unassessed` |
| `DUAL-13-14` | 双端连接抽屉与瀑布流 1:1 对等 | `legacy-unassessed` |
| `DUAL-13-15` | 连接数据流与治理命令无头测试 | `legacy-unassessed` |

> **证据与边界**：连接流/聚合/抽屉/路由链/治理/关闭/反向规则/脉冲/清退/排序/搜索由 shared `connection_view` + application 归约；`DUAL-13-04` 阶段耗时宿主缺失，保持 `planned`；ASN/geo 为内核规则求值产物，瞬时速率由相邻快照差分。

### 组 14：DNS 工作台、Fake-IP 治理与泄漏交叉探活 (DNS Studio & Leak Protection)
1. **DNS 6 项系统级核心开关表单**：enable, ipv6, cache, use_hosts, use_system_hosts, respect_rules。
2. **域名映射模式 (Enhanced Mode) 分段器**：`虚拟 IP (Fake-IP)` / `真实 IP (Redir-Host)` / `取消映射 (None)`。
3. **过滤模式 (Fake-IP Filter Mode) 分段器**：`黑名单 (Blacklist)` / `白名单 (Whitelist)` / `规则 (Rules)`。
4. **上游加密 DNS (DoH/DoT/DoQ/HTTP3) 配置**：支持配置多个上游安全 DNS 服务器与协议标记。
5. **回退解析策略 (Fallback DNS)**：配置境内外 Fallback DNS 服务器与 GEOIP 触发阈值。
6. **Fake-IP 映射池实时检索与检视**：可视化检索 Fake-IP (198.18.x.x) 与真实域名的解析绑定表。
7. **一键清空 Fake-IP 缓存与系统 DNS 缓存**：下发清理指令并调用系统命令刷新 OS DNS 缓存。
8. **DNS 泄漏多源并发交叉探测**：并发向多个全球探测源发送随机伪子域，检验真实 ISP DNS 泄漏。
9. **WebRTC 公网 IP 穿透探测**：检测浏览器 WebRTC 是否会穿透虚拟网卡泄漏真实内网/公网 IP。
10. **DNS 解析测速与延迟高亮**：对配置的各个 Nameserver 发起延迟测速并标明响应耗时。
11. **自定义 Hosts 映射表图形化编辑**：支持可视化添加/删除自定义域名解析映射 (`IP Domain`)。
12. **Nameservers 动态标签芯片**：支持为不同上游 DNS 贴上 `Domestic`、`Fallback` 等语义标签。
13. **DNS 故障自愈检测**：检测 DNS 监听端口占用与上游无法解析异常并提示修复。
14. **双端 DNS 工作台表单完全一致**：Iced (`dns.rs`) 与 Bevy (`dns.rs`) 结构化表单完全同步。
15. **DNS 解析与探活状态机无头单测**：模式切换、泄漏探测与缓存清理测试 100% 覆盖。

#### 组 14 逐项账目

| 项 | 任务 | 状态 |
| :--- | :--- | :--- |
| `DUAL-14-01` | DNS 6 项系统级核心开关表单 | `legacy-unassessed` |
| `DUAL-14-02` | 域名映射模式 (Fake-IP/Redir-Host/None) 分段器 | `legacy-unassessed` |
| `DUAL-14-03` | 过滤模式 (Blacklist/Whitelist/Rules) 分段器 | `legacy-unassessed` |
| `DUAL-14-04` | 上游加密 DNS (DoH/DoT/DoQ/HTTP3) 配置 | `legacy-unassessed` |
| `DUAL-14-05` | 回退解析策略 (Fallback DNS / fallback-filter) | `legacy-unassessed` |
| `DUAL-14-06` | Fake-IP 映射池实时检索与检视 | `legacy-unassessed` |
| `DUAL-14-07` | 一键清空 Fake-IP 缓存与系统 DNS 缓存 | `legacy-unassessed` |
| `DUAL-14-08` | DNS 泄漏多源并发交叉探测 | `legacy-unassessed` |
| `DUAL-14-09` | WebRTC 公网 IP 穿透探测（重新界定为本机 UDP 出网映射） | `legacy-unassessed` |
| `DUAL-14-10` | DNS 解析测速与延迟高亮 | `legacy-unassessed` |
| `DUAL-14-11` | 自定义 Hosts 映射表图形化编辑 | `legacy-unassessed` |
| `DUAL-14-12` | Nameservers 动态标签芯片 | `legacy-unassessed` |
| `DUAL-14-13` | DNS 故障自愈检测 | `legacy-unassessed` |
| `DUAL-14-14` | 双端 DNS 工作台表单完全一致 | `legacy-unassessed` |
| `DUAL-14-15` | DNS 解析与探活状态机无头单测 | `legacy-unassessed` |

> **证据与边界**：DNS 表单/映射/过滤/上游/回退/映射池/清缓存/泄漏/STUN/测速/Hosts/标签/自愈由 shared contract + application + host port 承载；`DUAL-14-06` 只发布实时连接可观测子集；`DUAL-14-08` 默认第三方公共 TXT 权威区；`DUAL-14-09` 重新界定为本机 STUN UDP 出网映射（非浏览器 WebRTC 结论）。

### 组 15：桌面/移动多模态形态、系统托盘、独立悬浮小窗与极客命令流 (Multimodal & UX)
1. **4 阶响应式形态断点架构**：桌面宽屏 (Wide)、标准桌面 (Sidebar)、平板导轨 (Rail 64px)、移动紧凑 (BottomNav)。
2. **动态系统托盘菜单与速率徽标**：托盘图标动态更新上下行网速数值，右键集成模式切换与退出清理。
3. **独立桌面 Mini HUD 网速悬浮小窗**：260x90 置顶半透明无边框窗口，双向波形、出口节点与快捷开关。
4. **Mini HUD 坐标持久化与贴边吸附**：鼠标拖拽悬浮窗位置，关闭后重启恢复坐标，支持屏幕边缘吸附。
5. **全键盘命令面板 (Command Palette - Ctrl+K)**：居中悬浮模糊检索，拼音/英文直达 11 页面、模式切换与体检。
6. **全局系统快捷键管理与冲突检测**：可视化捕获和配置系统全局快捷键（系统代理开关/TUN开关/HUD唤出）。
7. **移动端触控手势引擎 (Gesture Engine)**：支持屏幕左边缘右滑返回 (Swipe Back) 与列表顶部下拉刷新。
8. **低功耗 Reactive 调步渲染 (Cadence)**：窗口后台或最小化时自动降频至 2 FPS，前台活动时恢复 60 FPS。
9. **深浅与多主题系统级跟随**：Dark 暗黑、Light 亮色、Forest 森林、AMOLED 纯黑，支持跟随系统切换。
10. **AccessKit 无障碍语义全覆盖**：所有控件、状态圆点、文本行携带 AccessKit 角色与屏幕阅读器标签。
11. **IME 中文输入法深度跟踪与候选框定位**：中文输入法候选框准确跟随光标坐标，防止遮挡。
12. **Toast 消息队列防重与敏感信息脱敏**：全局通知防抖去重，自动脱敏密码、Token 与 Bearer 敏感字段。
13. **桌面无边框窗口拖拽与原生阴影**：支持 Windows/Linux/macOS 现代无边框拖拽、双击最大化与阴影。
14. **双端设计规范与交互质感 100% 对齐**：Iced 与 Bevy 在所有模态下互有测试与截图证据。
15. **多模态与外壳架构无头测试矩阵**：断点切换、主题换肤、命令面板与快捷键 100% 自动化覆盖。

#### 组 15 逐项账目

| 项 | 任务 | 状态 |
| :--- | :--- | :--- |
| `DUAL-15-01` | 4 阶响应式形态断点架构 | `legacy-unassessed`（外链） |
| `DUAL-15-02` | 动态系统托盘菜单与速率徽标 | `legacy-unassessed` |
| `DUAL-15-03` | 独立桌面 Mini HUD 悬浮小窗 | `legacy-unassessed` |
| `DUAL-15-04` | Mini HUD 坐标持久化与贴边吸附 | `legacy-unassessed` |
| `DUAL-15-05` | 全键盘命令面板 (Ctrl+K) | `legacy-unassessed` |
| `DUAL-15-06` | 全局系统快捷键管理与冲突检测 | `legacy-unassessed` |
| `DUAL-15-07` | 移动端触控手势引擎 | `legacy-unassessed` |
| `DUAL-15-08` | 低功耗 Reactive 调步渲染 | `legacy-unassessed` |
| `DUAL-15-09` | 深浅与多主题系统级跟随 | `legacy-unassessed` |
| `DUAL-15-10` | AccessKit 无障碍语义全覆盖 | `shared-ready` |
| `DUAL-15-11` | IME 中文输入法候选框定位 | `legacy-unassessed` |
| `DUAL-15-12` | Toast 消息队列防重与敏感信息脱敏 | `legacy-unassessed` |
| `DUAL-15-13` | 桌面无边框窗口拖拽与原生阴影 | `legacy-unassessed` |
| `DUAL-15-14` | 双端设计规范与交互质感对齐 | `legacy-unassessed` |
| `DUAL-15-15` | 多模态与外壳架构无头测试矩阵 | `legacy-unassessed` |

> **证据与边界**：响应式/托盘/Mini HUD/命令面板/快捷键/手势/调步/主题/无障碍/IME/Toast/窗口边框/设计 token 由 shared contract + application 承载；`DUAL-15-01` 由 [RESPONSIVE_PARITY_LEDGER.md](RESPONSIVE_PARITY_LEDGER.md) 权威跟踪，矩阵见 [MULTIMODAL_SHELL_MATRIX.md](archive/MULTIMODAL_SHELL_MATRIX.md)；`DUAL-15-10` 为 `shared-ready`（Iced 无 AccessKit，以真实可见 tooltip 兜底，不伪造语义树）；无移动宿主组合 Bevy 外壳，真机证据缺失。

### 附：10 大核心业务组能力并集总览

| 业务组 | 核心能力清单（功能并集） | 对标竞品来源 | 核心底层模块 |
| :--- | :--- | :--- | :--- |
| **01. 核心与网络栈**<br>(Core Runtime & TUN) | ① 内核多代际生命周期、平滑热重载与崩溃看门狗<br>② 多版本交付（Stable/Alpha/Meta）校验与回滚<br>③ 服务模式 (Service Mode/Privileged Helper) 提权守卫<br>④ TUN 模式多网络栈调度（gVisor/System/Mixed/LWIP）<br>⑤ 系统代理抢占探测与断电异常自愈<br>⑥ 局域网共享 (Allow-LAN) 混合端口与账密认证<br>⑦ IPv6 内核解析与转发拓扑开关 | Clash Verge Rev<br>Mihomo Party<br>Surge | `mihomo-platform`<br>`infiltrator-desktop`<br>`infiltrator-core::flow_control` |
| **02. 概览遥测中枢**<br>(Overview & Telemetry) | ① 双通道 GPU 实时流量波形（动态量程自适应）<br>② 分流链路可视化拓扑链（Inbound→Sniffer→Rule→Group→Outbound）<br>③ 主活动出口卡片（国旗徽标/协议/延迟/IP）<br>④ 订阅配额进度条（超 85% 动态预警、重置倒计时）<br>⑤ 系统代理与 TUN 模式双主控大卡<br>⑥ 代理模式即时分段器（Rule/Global/Direct/Script）<br>⑦ 6 项核心指标网格（连接/内存/CPU/速率/累计流量）<br>⑧ 公网 IP 隐私与地理位置多源探针 | Mihomo Party<br>Clash Nyanpasu<br>Flclash | `mihomo-api`<br>`infiltrator-core::dns_tester`<br>`infiltrator-bevy-widgets::chart` |
| **03. 代理与智能节点**<br>(Proxies & Protocol Matrix) | ① 协议全矩阵保真（SS 2022/VLESS Reality/Trojan/TUIC v5/Hy2/WireGuard/AmneziaWG/AnyTLS/SSH/Snell）<br>② 策略组全覆盖（Selector/URLTest/Fallback/LoadBalance一致性哈希/Relay链式中继）<br>③ 并发测速信号量流控（防止网络风暴与熔断）<br>④ 拼音首字母/中文/协议多维模糊搜索过滤<br>⑤ 节点死链一键隐藏、星标置顶与偏好锁定<br>⑥ 单节点历史延迟 Sparkline 折线走势<br>⑦ 自定义节点表单向导与通用 URI 导入导出<br>⑧ 前置跳板代理 (Dialer-Proxy) 拓扑编排 | Mihomo Party<br>Flclash<br>Clash Verge Rev | `infiltrator-core::profile_converter`<br>`mihomo-api::proxy`<br>`infiltrator-core::flow_control` |
| **04. 配置与脚本生态**<br>(Profiles & Scripting) | ① 多渠道导入（URL/本地/剪贴板）与自定义 User-Agent<br>② 订阅定时自动轮询更新、条件请求（ETag）与防重入<br>③ 多订阅节点聚合器（多源去重、按国家自动成组）<br>④ YAML AST 语法高亮编辑器、代码片段与行号报错定位<br>⑤ QuickJS 沙箱脚本控制台（带 64MB 内存熔断与实时变换）<br>⑥ 多级配置覆写管道（Base→Subscription→Merge Rules→Mixin）<br>⑦ 配置快照历史与可视化行内/并排 Diff 回滚 | Clash Verge Rev<br>Mihomo Party<br>Flclash | `infiltrator-domain::subscription`<br>`infiltrator-core::subscription_io`<br>`infiltrator-core::script_engine`<br>`infiltrator-core::filter_pipeline` |
| **05. 分流规则与链路沙盒**<br>(Rules & Live Tracer) | ① 28+ 规则类型全覆盖（DOMAIN/IP-CIDR/PROCESS/GEO/DSCP/UID等）<br>② 逻辑规则 (AND/OR/NOT/SUB-RULE) 递归构建与解析<br>③ 交互式实时分流追踪器（输入目标/进程回放决策链路）<br>④ MRS 官方二进制规则集高性能本地索引与 Diff<br>⑤ Rule-Provider 外部规则集全生命周期管理<br>⑥ 规则命中计数统计与死规则静态诊断<br>⑦ 可视化拖拽调整规则优先级 | Mihomo Party<br>Clash Verge Rev | `infiltrator-core::rules`<br>`infiltrator-core::mrs`<br>`infiltrator-core::sub_rules` |
| **06. 连接审计与深度透视**<br>(Connections & Telemetry) | ① 高并发实时连接流式采集（源/目/进程/规则/出站/速率）<br>② 三维聚合视图（扁平流/按进程聚合/按目标域名聚合）<br>③ 连接详情侧滑抽屉（耗时瀑布流：DNS/TCP/TLS/TTFB）<br>④ 目标 IP、ASN 归属机构与地理情报透视<br>⑤ 连接实时治理（单条阻断/过滤范围阻断/全部关闭）<br>⑥ 从连接反向一键创建分流规则向导 | Surge<br>Clash Nyanpasu<br>Mihomo Party | `mihomo-api::connection`<br>`infiltrator-core::idle_connection_sweeper` |
| **07. 日志与自愈体检**<br>(Logs, DNS & Doctor) | ① 环形流式日志（4 级过滤、Regex 检索、滚屏锁定、导出）<br>② DNS 工作台（DoH/DoT/DoQ/HTTP3 配置与回退策略）<br>③ Fake-IP 映射池实时检视与一键清缓存<br>④ DNS 泄漏与 WebRTC 泄漏多源交叉探测<br>⑤ Doctor 深度自愈套件（端口冲突释放、TUN 驱动核查、系统代理注册表修复、内核连通性探针） | Surge<br>Shadowrocket<br>Clash Verge Rev | `mihomo-api::log`<br>`infiltrator-core::dns_tester`<br>`infiltrator-core::doctor` |
| **08. 应用级分流与系统穿透**<br>(Per-App Routing) | ① 操作系统动态进程枚举（Browser/Game/Dev/Media/System 分类）<br>② 应用高清图标提取与本地高性能缓存<br>③ 单应用三态分流设置（代理/直连/拦截）<br>④ Windows UWP 应用回环隔离豁免工具<br>⑤ Android 移动端分应用代理黑白名单 | Mihomo Party<br>Surge Mac/iOS | `infiltrator-desktop::process_enumerator`<br>`android::vpn` |
| **09. 多端云同步与安全备份**<br>(Cloud Sync & Security) | ① 多云端协议支持（WebDAV 坚果云/Nextcloud、GitHub Gist、iCloud）<br>② 精确到字段的三向差异合并 (3-Way Merge) 与冲突解决<br>③ 全量配置数据端到端强加密 (AES-256-GCM)<br>④ 配置变更后自动化静默同步 | Flclash<br>Mihomo Party | `mihomo-dav-sync/*`<br>`infiltrator-core::sync` |
| **10. 系统集成与多端沉浸**<br>(System Integration & UX) | ① 动态系统托盘菜单（上下行速率徽标/模式切换/系统代理开关）<br>② 桌面独立极简迷你悬浮窗 (Mini HUD: 260x90 置顶小窗)<br>③ 全键盘命令面板 (Command Palette - Ctrl+K 全局直达)<br>④ 全局系统快捷键设置与冲突规避<br>⑤ 宽屏桌面/平板导轨/移动端底栏+抽屉三模态自适应 | Clash Verge Rev<br>Flclash<br>Clash Nyanpasu | `infiltrator-desktop::tray_badge`<br>`infiltrator-bevy-widgets::windowing`<br>`infiltrator-shared::locales` |

---

## 四、分业务组【UI表现与交互体验清单】（视觉与动效对齐规范）

两套前端必须遵循完全一致的交互规范、动效节奏与视口响应能力：

| 业务组 | 视口响应与布局架构 | 交互动效与手势规范 | 状态回显（加载/空态/骨架/错误） |
| :--- | :--- | :--- | :--- |
| **01. 核心网络栈** | 桌面端卡片网格，移动端单列列表；控件居右对齐。 | Switch 具备 120ms 弹性滑动；提权引导模态框平滑淡入。 | 切换中显示骨架微光 Spinner；提权失败弹出橙色警告条与排查日志入口。 |
| **02. 概览遥测** | 顶部流量卡 + 拓扑链；中部指标网格；底部出口卡片与配额条。支持纵向拖拽重排。 | GPU 贝塞尔波形 60 FPS 平滑滚动；拓扑节点随流量动态微光流动。 | 核心启动中淡入整页骨架屏；配额超 85% 转警戒黄；无订阅时显示本地卡片。 |
| **03. 代理矩阵** | 响应式流体网格（自适应 2~4 列）；支持一键切换紧凑单列列表。 | 鼠标悬停卡片上浮 2px + 微阴影；测速按钮带环形旋转进度条；支持拖拽调整策略组顺序。 | 测速中卡片内延迟数值显示脉冲波纹骨架屏；超时标红/置灰；收藏节点带金色高亮星标。 |
| **04. 配置脚本** | 双栏/单栏自适应；代码编辑区自适应撑满，底栏集成诊断抽屉。 | 代码编辑器行号联动高亮；格式化与插入 Snippet 平滑滚动；Diff 增删分色对比。 | YAML 语法错误在出错行呈现红色波浪线与行号红标；QuickJS 实时流式回显日志。 |
| **05. 规则追踪** | 50,000+ 条目虚拟视口滚动 ($O(1)$ 几何裁剪)；顶部即时搜索栏与 Tracer 抽屉。 | Tracer 树状回放分流决策链路，命中行高亮闪烁；规则条目支持拖拽抓手调整。 | 视口滚动保持 60 FPS 零卡顿；命中计数实时跳动；死规则呈现置灰与删除提示。 |
| **06. 连接审计** | 表格与卡片自适应；右侧可展开宽度 420px 的 Slide-out 侧滑下钻抽屉。 | 侧滑抽屉平滑滑出带有半透明遮罩；耗时瀑布流按阶段使用比例条呈现。 | 断开连接呈现淡出删除动效；高吞吐连接行带脉冲微光；无连接显示优雅占位插画。 |
| **07. 日志自愈** | 终端风格等宽排版；悬浮日志锁定浮钮；底部集成 DNS 与健康卡片。 | 向上滚动自动解除吸底并浮现“回到最新”悬浮胶囊；Doctor 一键自愈带阶梯打勾动画。 | 日志级别分色（ERR 红, WRN 黄, INF 绿, DBG 灰）；体检项按绿（通过）、黄（警告）、红（严重）标识。 |
| **08. 应用分流** | 应用高清图标 + 进程名网格，支持搜索与分类标签栏切换。 | 代理/直连/拦截使用 Segmented Control 分段器滑动胶囊；图标异步平滑淡入。 | 图标提取中显示圆形骨架占位；系统保护进程禁用修改并附 Tooltip 说明。 |
| **09. 云端同步** | 凭据配置表单 + 同步历史列表；冲突时弹出全屏对比对话框。 | 点击“立即同步”图标旋转；冲突解决支持左右卡片点击单项采纳合并。 | 同步中展示旋转指示；同步成功弹出绿色 Toast；冲突时黄色警示横幅阻断提交。 |
| **10. 系统集成** | 悬浮窗 (260x90) 极简圆角无边框；Command Palette 居中悬浮顶层面板。 | `Ctrl+K` 弹出面板带 80ms 快速淡入与焦点自动聚焦；悬浮窗全区域平滑拖拽与贴边吸附。 | 键盘上下键平滑切换选择项；无匹配时友好提示；悬浮窗根据网络状态自适应微波形。 |

---

## 五、实施排期与双端同步推进路线图（Four Waves）

双端演进划分为 4 个推进批次，每批次以**双端同步验收**为准入条件：

```
Wave 1: 双端框架与核心主干对齐 [A-01～A-05 架构前置已验收，业务 parity 持续]
  ├─ Bevy 11 页面路由/场景骨架与 RouteHistory 已存在
  ├─ Iced 与 Bevy 均可由 host 注入同一个 application-owned surface pump
  └─ 核心生命周期、系统代理、TUN、节点和测速按 shared contract 收口

Wave 2: 核心遥测、透视与诊断闭环 [shared surface 已接通，业务项逐项验收]
  ├─ Bevy 11 页与 Iced surface model 已接入 typed snapshot/event/status 边界
  ├─ 连接、日志、DNS、Doctor、Tracer 的具体 live 数据仍按业务项补齐
  └─ 波形、拓扑、Mini HUD、Command Palette 以双端测试和宿主证据重新验收

Wave 3: 高级扩展、配置工程与应用分流 [目标已定义，未计入完成]
  ├─ Iced 现有能力先抽成 shared/application + UI adapter
  ├─ Bevy 同批次接入 live projection、命令结果和错误状态
  └─ 聚合器、AST/Diff、脚本、应用分流必须各自完成双端测试

Wave 4: 规则集深度治理、云端同步与多模态大一统 [A-01～A-05 后的 0.30 当前主线]
  ├─ shared surface 架构前置已完成，按 DUAL 项目进入双端实现
  ├─ MRS、WebDAV、Rail 和响应式能力按两端 live parity 重新记账
  └─ 移动触控、VPN host、低功耗调度进入 Bevy mobile + Android host 联合验收
```
