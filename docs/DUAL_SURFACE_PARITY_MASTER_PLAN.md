# MusicFrog Infiltrator: 双端（Iced & Bevy UI）同步演进与成熟 Mihomo 全景并集主控规范 (Dual-Surface Parity Master Plan)

本文档是 MusicFrog Infiltrator 项目的最高战略主控台账，旨在确立 **Iced（成熟桌面端）** 与 **Bevy UI（桌面+移动统一跨平台战略端）** 的**严格同步演进机制**，并全面对标业界最成熟客户端（Clash Verge Rev、Mihomo Party、Flclash、Clash Nyanpasu、Surge），以其**最完善功能组的数学并集（Union）**作为最终目标。

---

## 一、双端同步推进战略定调与工程原则

### 1. 核心定位转变：从“先后跟随”转为“严格对等双主干”
*   **过去模式**：Iced 优先承接新功能，Bevy UI 后续里程碑追赶。该模式易导致两端视图模型分叉、功能落差扩大。
*   **当前标准**：Iced 与 Bevy UI 正式确立为**对等双主干 Surface**。任何新业务功能与 UI 交互能力，必须在同一批次（Wave）内由双端同步交付与验收，杜绝“单端孤立特性”。

### 2. 双端同步必须开展的 4 项工程基础设施

```
               ┌────────────────────────────────────────────────────────┐
               │           共享核心层与状态机 (100% 逻辑下沉)            │
               │   infiltrator-core / mihomo-platform / mihomo-config   │
               └──────────────────────────┬─────────────────────────────┘
                                          │
                                          ▼
               ┌────────────────────────────────────────────────────────┐
               │           共享契约与视图模型 (UI-Agnostic Seam)         │
               │   infiltrator-shared (DomainState, Actions, I18n)      │
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
               │         双端对齐自动化测试与视觉回归门禁 (CI Guard)        │
               │  headless tests / parity-guard.py / golden snapshots │
               └────────────────────────────────────────────────────────┘
```

1.  **业务逻辑 100% 下沉与 ViewModel 契约化**：
    *   严禁在 UI crate 中编写业务调度、网络请求或配置组装。
    *   所有业务状态机收敛于 `infiltrator-core` 与 `mihomo-platform`；
    *   双端共享只读视图状态（`DomainState`）与命令意图（`CommandIntent`），由 `infiltrator-shared` 集中管理。
2.  **单向命令总线与事件广播标准化**：
    *   Iced 的 `Message` 与 Bevy 的 `UiAction` 背后直接派发相同的领域命令（Domain Command），交由相同的异步运行时处理；
    *   底层遥测流（WebSocket 流量、连接流、日志）统一推入无锁缓冲区，双端同频消费。
3.  **设计系统与组件库 1:1 镜像对齐**：
    *   统一 Design Tokens：两端色彩阶梯（主色、警示色、背景层级）、间距（4/8/12/16/24px）、圆角（4/8/12px）与排版数值镜像一致；
    *   基础控件 1:1 对应：虚拟视口列表（`VirtualList` ↔ `list/scroll_core`）、折线图（`Waveform` ↔ `chart::bezier`）、抽屉与模态层（`connection_drawer` ↔ `drawer/adaptive_modal`）、极简小窗（`mini_hud` ↔ `desktop::mini_hud`）。
4.  **双端无头测试与自动化对齐门禁**：
    *   开发静态门禁工具 `scripts/quality/parity-guard.py`，确保双端路由定义、页面覆盖与命令枚举严格匹配；
    *   双端具备无头自动化测试矩阵（`tests/gui/iced_*` 与 `tests/headless/*`），保障相同用户交互在两端产生一致的内核行为；
    *   定期运行视觉比对脚本，保障 11 个页面在桌面与自适应视口下的视觉层次统一。

---

## 二、成熟 Mihomo 客户端能力并集标杆定义

我们对标的成熟客户端包括：
*   **Clash Verge Rev**：脚本生态（JS/TS 沙箱）、多级配置覆写（Merge Rules / Mixin）、服务模式（Service Mode）、全局热键、高级 TUN 网络栈。
*   **Mihomo Party**：Sub-Store 原生整合、分流链路可视化拓扑、交互式实时分流追踪器（Live Rule Tracer）、多维测速丢包雷达、应用级分流与高清图标提取。
*   **Flclash**：桌面/平板/移动多模态响应式自适应设计、多配置聚合器（Aggregator）、高密度卡片网格、WebDAV 云同步。
*   **Clash Nyanpasu**：多维连接审计（按进程/按域名）、Fake-IP 池检索、流体动效与主题定制。
*   **Surge / Stash**：DNS/TCP/TLS/TTFB 握手瀑布流、ASN 归属透视、Doctor 专家级自愈诊断、实时抓包与 PCAP 导出。

**本项目目标：将上述所有竞品的最完善能力求“全功能并集（Union）”，并在 Iced 与 Bevy UI 中全量镜像对齐！**

---

## 三、15 大业务组 × 15 项全景深度功能清单（225 项双端深度对标并集）

为彻底消除表面化对齐，本项目确立 **15 大核心业务组 × 15 项深度能力（共 225 项具体工程指标）**，要求 `infiltrator-iced` 与 `infiltrator-bevy-ui` 协同攻坚、共同对齐：

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
14. **双端脚本控制台与 Mixin 视口对齐**：Iced (`script_console.rs`) 与 Bevy (`profiles_script.rs`) 对等呈现。
15. **QuickJS 引擎与沙箱熔断单测**：超时熔断、内存限制与 AST 变换测试 100% 覆盖。

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


全仓按 10 大核心业务组划分，双端必须具备对等功能支撑：

| 业务组 | 核心能力清单（功能并集） | 对标竞品来源 | 核心底层模块 |
| :--- | :--- | :--- | :--- |
| **01. 核心与网络栈**<br>(Core Runtime & TUN) | ① 内核多代际生命周期、平滑热重载与崩溃看门狗<br>② 多版本交付（Stable/Alpha/Meta）校验与回滚<br>③ 服务模式 (Service Mode/Privileged Helper) 提权守卫<br>④ TUN 模式多网络栈调度（gVisor/System/Mixed/LWIP）<br>⑤ 系统代理抢占探测与断电异常自愈<br>⑥ 局域网共享 (Allow-LAN) 混合端口与账密认证<br>⑦ IPv6 内核解析与转发拓扑开关 | Clash Verge Rev<br>Mihomo Party<br>Surge | `mihomo-platform`<br>`infiltrator-desktop`<br>`infiltrator-core::flow_control` |
| **02. 概览遥测中枢**<br>(Overview & Telemetry) | ① 双通道 GPU 实时流量波形（动态量程自适应）<br>② 分流链路可视化拓扑链（Inbound→Sniffer→Rule→Group→Outbound）<br>③ 主活动出口卡片（国旗徽标/协议/延迟/IP）<br>④ 订阅配额进度条（超 85% 动态预警、重置倒计时）<br>⑤ 系统代理与 TUN 模式双主控大卡<br>⑥ 代理模式即时分段器（Rule/Global/Direct/Script）<br>⑦ 6 项核心指标网格（连接/内存/CPU/速率/累计流量）<br>⑧ 公网 IP 隐私与地理位置多源探针 | Mihomo Party<br>Clash Nyanpasu<br>Flclash | `mihomo-api`<br>`infiltrator-core::dns_tester`<br>`infiltrator-bevy-widgets::chart` |
| **03. 代理与智能节点**<br>(Proxies & Protocol Matrix) | ① 协议全矩阵保真（SS 2022/VLESS Reality/Trojan/TUIC v5/Hy2/WireGuard/AmneziaWG/AnyTLS/SSH/Snell）<br>② 策略组全覆盖（Selector/URLTest/Fallback/LoadBalance一致性哈希/Relay链式中继）<br>③ 并发测速信号量流控（防止网络风暴与熔断）<br>④ 拼音首字母/中文/协议多维模糊搜索过滤<br>⑤ 节点死链一键隐藏、星标置顶与偏好锁定<br>⑥ 单节点历史延迟 Sparkline 折线走势<br>⑦ 自定义节点表单向导与通用 URI 导入导出<br>⑧ 前置跳板代理 (Dialer-Proxy) 拓扑编排 | Mihomo Party<br>Flclash<br>Clash Verge Rev | `infiltrator-core::profile_converter`<br>`mihomo-api::proxy`<br>`infiltrator-core::flow_control` |
| **04. 配置与脚本生态**<br>(Profiles & Scripting) | ① 多渠道导入（URL/本地/剪贴板）与自定义 User-Agent<br>② 订阅定时自动轮询更新、条件请求（ETag）与防重入<br>③ 多订阅节点聚合器（多源去重、按国家自动成组）<br>④ YAML AST 语法高亮编辑器、代码片段与行号报错定位<br>⑤ QuickJS 沙箱脚本控制台（带 64MB 内存熔断与实时变换）<br>⑥ 多级配置覆写管道（Base→Subscription→Merge Rules→Mixin）<br>⑦ 配置快照历史与可视化行内/并排 Diff 回滚 | Clash Verge Rev<br>Mihomo Party<br>Flclash | `infiltrator-core::subscription`<br>`infiltrator-core::script_engine`<br>`infiltrator-core::filter_pipeline` |
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
Wave 1: 双端框架与核心主干对齐 [已交付]
  ├─ 11 页面统一路由打通 (Route::ALL) 与 RouteHistory 进退栈
  ├─ 核心生命周期、系统代理与 TUN 主大卡
  └─ 基础节点卡片、并发测速流控与分组选择

Wave 2: 核心遥测、透视与诊断闭环 [已交付]
  ├─ 真实双通道流量波形 (GPU Bezier / Canvas) 与动态拓扑链
  ├─ 交互式 Live Rule Tracer 分流沙盒 (双端挂载)
  ├─ 连接审计深度透视抽屉 (DNS/TCP/TLS/TTFB 瀑布流与一键加规则)
  └─ 独立桌面 Mini HUD (260x90 悬浮小窗) & Ctrl+K 全键盘命令面板

Wave 3: 高级扩展、配置工程与应用分流 [已交付]
  ├─ 系统进程枚举、图标提取与应用级分流 (Per-App 3 态网格)
  ├─ 多订阅节点聚合器 (Profile Aggregator 港日美新自动成组)
  ├─ 配置快照历史可视化 Diff 比对与秒级回滚
  ├─ QuickJS 扩展脚本沙箱调试控制台 (预设、熔断与日志回显)
  └─ 自定义节点表单向导与通用 URI 编解码引擎

Wave 4: 规则集深度治理、云端同步与多模态大一统 [进行中]
  ├─ [已交付] MRS 二进制规则集高性能索引与外部 Provider 解构 (双端挂载)
  ├─ [已交付] WebDAV / Gist 字段级三向冲突合并 (3-Way Merge 场景与双端对齐)
  ├─ [已交付] 桌面 64px 紧凑导轨模式 (Rail Mode) 与双端自适应断点
  └─ [演进中] 移动端触控手势实机闭环与低功耗 Reactive 调步
```

---

## 六、全仓分散与重复文档治理收敛图谱

为彻底消除文档分散、口径陈旧和重复描述，现对全仓技术与规划文档建立清晰的权威归属矩阵：

| 文档路径 | 当前状态与问题 | 治理方案与收敛动作 | 权威定位与维护原则 |
| :--- | :--- | :--- | :--- |
| **`docs/DUAL_SURFACE_PARITY_MASTER_PLAN.md`** | **[新设立]** | **全仓最高主控台账**：纳管双端同步策略、10 大业务组功能并集清单、UI 表现清单与 Wave 路线图。 | **唯一权威主纲**，所有其他前端与差距文档均向其链接收敛。 |
| **`docs/FRONTENDS.md`** | 描述多端关系，但提及已退役的 Tauri，且描述 Bevy UI 滞后跟随。 | 更新内容：声明 Tauri 退役；确立 Iced 与 Bevy UI 为双主干同步表面；链接至主控规范。 | 多前端架构边界与跨端决策标记（shared/local）的权威定义。 |
| **`docs/FUNCTIONAL_MAP.md`** | 列出功能域与 owner，仍有少量旧 surface 描述。 | 刷新表格：将 owner 和入口聚焦于 `infiltrator-core`、`Iced` 和 `Bevy UI` 双端。 | 业务功能唯一 Rust owner 的权威检索入口。 |
| **`docs/ICED_CORE_MATURITY_GAPS.md`** | 仅记录 Iced 的 4 维度与 Wave 1~5 落地项，与 Bevy 隔离。 | 头部增补索引指引：明确本台账为 Master Plan 在 Iced 前端的具体落地执行切片。 | Iced 侧代码实现、组件与测试证据的追溯台账。 |
| **`docs/BEVY_CORE_MATURITY_GAPS.md`** | 记录 Bevy 的 10 维度 150 项工程缺口，未显式与 Iced 对齐。 | 头部增补索引指引：明确 10 维度与 Master Plan 10 大业务组 1:1 对齐，作为 Bevy 落地切片。 | Bevy 侧场景、组件与无头测试证据的追溯台账。 |
| **`docs/MATURITY_GAP_ANALYSIS.md`** | 记录 10×10 内核与协议差距，偏重后端逻辑。 | 明确其定位为“核心层成熟度台账”，将 UI 表现层与双端同步要求引流至 Master Plan。 | `infiltrator-core` 与 `mihomo-*` 协议与配置 AST 的底层权威台账。 |
| **`docs/TEN_PHASE_ROADMAP.md`** | 记录 10 阶段工程任务，部分与 GAP_ANALYSIS 重叠。 | 保留历史演进追踪，头部声明其与当前 Wave 1~4 的映射关系。 | 历史演进里程碑与代码下沉过程的事实记录。 |
| **`iced_todo.md`** (根目录) | 仅有 30 行简略重定向文字。 | 更新内容：直接指向 `docs/DUAL_SURFACE_PARITY_MASTER_PLAN.md` 与 Iced 落地台账。 | 根目录快捷重定向索引。 |
| **`docs/README.md`** | 缺少最新台账导航与阅读顺序。 | 更新阅读顺序与权威关系表，将 Master Plan 纳为核心架构第一入口。 | 文档中心主索引。 |

