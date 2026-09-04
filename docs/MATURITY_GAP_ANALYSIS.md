# Mihomo 客户端全景成熟度差距分析与演进实施台账 (Maturity Gap Analysis & Implementation Ledger)

本文档实事求是地记录本项目与业界一线成熟客户端（Clash Verge Rev、Mihomo Party、Flclash、Clash Nyanpasu 以及 Surge/Shadowrocket/Stash 等）的全景技术差距，并作为分批落地实施的唯一基准台账。

> **主控台账与分工说明（2026-09-03）**：
> 本文档聚焦于底层核心层与协议层（`infiltrator-core` / `mihomo-*`）的 10×10 深度差距。有关 Iced 与 Bevy UI 的**双端同步演进**、**10 大业务组功能并集**与 **UI 表现清单**，统一归属并由最高主控台账 [DUAL_SURFACE_PARITY_MASTER_PLAN.md](DUAL_SURFACE_PARITY_MASTER_PLAN.md) 统摄。

---

## 总体设计原则

1. **核心逻辑下沉**：所有配置 AST 引擎、协议模型、URI 转换管道、规则分析器、DNS 拓扑与特权网络状态机 100% 沉淀在 `infiltrator-core` / `mihomo-config` / `mihomo-platform` 共享层。
2. **零运行时破坏与向后兼容**：所有新增字段、扩展协议与高级参数遵循 serde 默认降级与向后兼容规则，不得破坏已有配置文件与持久化数据。
3. **行数预算与代码整洁**：严格遵守全仓单文件不超过 800 行的硬性规范，业务逻辑庞大时必须按领域拆分为子模块（Submodules）。
4. **门禁与测试严谨**：每个批次的代码改动必须具备单元测试与全仓 `nextest`、`clippy`、`line-guard`、`import-guard` 零违规验证。

---

## 十大维度与百项差距矩阵 (10 Domains × 10 Items)

### 一、协议矩阵与节点生态深度保真 (Protocols, Transports & Advanced Node Fidelity)

| 编号 | 任务与差距项 | 优先级 | 状态 | 涉及模块 |
|:---|:---|:---:|:---:|:---|
| **P01-01** | **现代高级传输层协议覆盖**：完善 XHTTP (SplitHTTP)、gRPC (multi-mode/service-name/health-check)、HTTP/2、原生 QUIC 以及 WebSocket Early Data (0-RTT, max-early-data)。 | P1 | 已落地 | `infiltrator-core::profile_converter` |
| **P01-02** | **VLESS 核心进阶特性**：支持 XTLS-Reality (`pbk`, `sid`, `spx`, `short-id`), Vision 流控 (`xtls-rprx-vision`), uTLS 指纹伪装 (`chrome`, `safari`, `ios`, `random` 等)。 | P1 | 已落地 | `infiltrator-core::profile_converter` |
| **P01-03** | **TUIC v5 与 Hysteria 2 拥塞与伪装**：支持 TUIC v5 的 `congestion_control`、`udp_relay_mode`、`reduce_rtt`；支持 Hysteria 2 端口跳跃 (`ports: 10000-20000`)、`obfs`、`masquerade`。 | P1 | 已落地 | `infiltrator-core::profile_converter` |
| **P01-04** | **Shadowsocks 2022 与多协议插件链**：支持 2022-blake3 密码族，支持 SIP003 插件 (`v2ray-plugin`, `obfs-local`, `shadow-tls v3`, `simple-obfs`) 参数解析与协同。 | P1 | 已落地 | `infiltrator-core::profile_converter` |
| **P01-05** | **WireGuard / AmneziaWG 完整拓扑**：支持 `preshared-key`, `reserved` 字节混淆、AmneziaWG 参数 (`jc/jmin/jmax/s1/s2/h1-h4`)、`workers` 与 `peers` 列表。 | P2 | 已落地 | `infiltrator-core::profile_converter` |
| **P01-06** | **SSH 与 AnyTLS 等协议接入**：支持原生 SSH SOCKS 代理 (`user`, `private-key`, `passphrase`, `host-key-algorithms`) 及 AnyTLS / Trojan-Go 解析。 | P2 | 已落地 | `infiltrator-core::profile_converter` |
| **P01-07** | **Dialer-Proxy 链式代理与 Relay 编排**：支持 `dialer-proxy` 节点跳板链路解析、循环依赖检测与 Relay 策略组拓扑编排。 | P1 | 已落地 | `infiltrator-core::profile_converter` |
| **P01-08** | **多路复用 (Smux / Yamux / H2Mux) 调优**：支持连接复用层参数细化 (`max-connections`, `min-streams`, `padding`, `brutal-opts`)。 | P2 | 已落地 | `infiltrator-core::profile_converter` |
| **P01-09** | **TLS/ECH 与多版本 ALPN 协商**：支持 ECH 扩展参数注入、ALPN 多版本协商 (`h3, h2, http/1.1`)、细粒度域名证书白名单与自定义 CA 证书。 | P2 | 已落地 | `infiltrator-core::profile_converter` |
| **P01-10** | **节点参数无损双向转换**：节点在 URI、JSON、Clash YAML 互转过程中保持 100% 结构保真，未知字段无损流通。 | P1 | 已落地 | `infiltrator-core::profile_converter` |

---

### 二、分流规则引擎与 MRS/Rule-Provider 治理体系 (Rule Engine, MRS & Rule-Provider Ecosystem)

| 编号 | 任务与差距项 | 优先级 | 状态 | 涉及模块 |
|:---|:---|:---:|:---:|:---|
| **P02-01** | **高级规则类型全覆盖**：支持 `IN-TYPE`, `IN-NAME`, `IN-USER`, `PROCESS-PATH-REGEX`, `PROCESS-NAME-REGEX`, `NETWORK`, `DSCP`, `UID`, `PACKAGE-NAME` 等 28+ 规则类型。 | P1 | 已落地 | `infiltrator-core::rules` |
| **P02-02** | **逻辑组合规则 (Logic Rules) 深度解析**：支持 `AND`, `OR`, `NOT`, `SUB-RULE` 多层递归 AST 语法树解析、校验与序列化。 | P1 | 已落地 | `infiltrator-core::sub_rules` |
| **P02-03** | **MRS 二进制规则集深度集成**：支持 Mihomo 官方 `.mrs` 二进制规则集高性能本地索引、格式校验与版本 diff。 | P1 | 已落地 | `infiltrator-core::mrs` |
| **P02-04** | **Rule-Provider 全生命周期治理**：支持多行为模式 (`classical/domain/ipcidr`)、条件请求 (ETag/Last-Modified)、退避重试与自动定时更新。 | P1 | 已落地 | `infiltrator-core::rules` |
| **P02-05** | **规则命中实时流与性能计数**：基于内核事件流统计每条规则累计命中次数、时延贡献与冷门规则检测。 | P2 | 已落地 | `infiltrator-core::rule_hit_counter` |
| **P02-06** | **分流模式全矩阵支持**：支持 `Script` (可编程全局脚本), `Direct`, `Global`, `Rule` 四大模式即时无感切换。 | P1 | 已落地 | `infiltrator-domain::config` |
| **P02-07** | **策略组调度算法完整度**：支持 `LoadBalance` 会话保持 (`sticky-sessions` / `consistent-hashing`)、权重分配；支持 `URLTest`/`Fallback` 退避阈值。 | P1 | 已落地 | `infiltrator-domain::config` |
| **P02-08** | **可视化分流追踪 (Tracer) 与沙盒回放**：提供输入 `URL + 来源 IP + 进程名 + 入口协议` 的模拟分流沙箱与命中全景链路回放。 | P1 | 已落地 | `infiltrator-core::rules` |
| **P02-09** | **规则拓扑排序与冲突检测**：静态分析死规则（如被上层覆写的下层规则）、IP-CIDR 掩码重叠与 GEOIP 判定顺序倒置。 | P2 | 已落地 | `infiltrator-core::rules` |
| **P02-10** | **多配置规则分层覆写 (Cascade Overlay)**：构建 Base Profile -> Subscription -> Custom Overwrite -> Mixin 的清晰多级注入管道。 | P1 | 已落地 | `infiltrator-core::mixin` |

---

### 三、高级 DNS 架构、分流防污染与泄露防护 (Advanced DNS Topology & Leak-Proof Routing)

| 编号 | 任务与差距项 | 优先级 | 状态 | 涉及模块 |
|:---|:---|:---:|:---:|:---|
| **P03-01** | **Fake-IP 完整生命周期管理**：支持 `fake-ip-filter-mode` (whitelist/blacklist)、自定义 IP 池、持久化映射文件与一键清空 Fake-IP 缓存。 | P1 | 已落地 | `infiltrator-core::dns` |
| **P03-02** | **多层级 Nameserver 分流策略矩阵**：支持 `nameserver-policy` 域名/GeoSite 精准分流至特定 DoH/DoT/企业内部私有 DNS。 | P1 | 已落地 | `infiltrator-core::dns` |
| **P03-03** | **四类独立上游 DNS 分层隔离**：严格隔离 `default-nameserver`, `proxy-server-nameserver`, `direct-nameserver`, `nameserver` 的职责与配置。 | P1 | 已落地 | `infiltrator-core::dns` |
| **P03-04** | **现代加密 DNS 传输全协议栈**：全面支持 DNS-over-HTTP/3 (DoH3), DNS-over-QUIC (DoQ), DNSCrypt, DNS-over-TLS (DoT)。 | P1 | 已落地 | `infiltrator-core::dns` |
| **P03-05** | **EDNS Client Subnet (ECS) 注入与保护**：支持 ECS 子网掩码配置、地理位置伪装与防止向海外 DNS 泄露本地真实 IP 的剥离机制。 | P2 | 已落地 | `infiltrator-core::dns` |
| **P03-06** | **DNS Fallback 智能过滤与防污染**：支持 `fallback-filter` (`geoip`, `geoip-code`, `ipcidr`, `domain`, `geosite`) 自动拦截污染解析。 | P1 | 已落地 | `infiltrator-core::dns` |
| **P03-07** | **DNS 劫持与系统 53 端口接管**：在 TUN / 系统代理模式下管控本地 53 端口 UDP/TCP 透明重定向并处理端口冲突。 | P1 | 已落地 | `infiltrator-core::dns` |
| **P03-08** | **DNS 缓存优化与 TTL 动态覆写**：支持 `cache-algorithm` (lru/arc)、`max-ttl`、`min-ttl` 强制改写与乐观缓存 (Stale-While-Revalidate)。 | P2 | 已落地 | `infiltrator-core::dns` |
| **P03-09** | **DNS 泄露全景一键检测**：集成随机子域名探测套件，比对实际解析出口 IP 是否包含本地运营商 DNS。 | P2 | 已落地 | `infiltrator-core::dns_tester` |
| **P03-10** | **系统级 DNS 干净接管与看门狗还原**：在崩溃或异常退出时，100% 可靠还原 Windows/macOS/Linux 原始系统 DNS。 | P0 | 已落地 | `mihomo-platform` |

---

### 四、系统底层特权网络与 TUN 网卡深度治理 (Privileged TUN Stack, Route Engine & OS Integrations)

| 编号 | 任务与差距项 | 优先级 | 状态 | 涉及模块 |
|:---|:---|:---:|:---:|:---|
| **P04-01** | **Windows 免 UAC 静默后台服务**：构建独立 Windows 后台服务 (`infiltrator-service.exe`) 与双向 Named Pipe IPC 握手。 | P0 | 已落地 | `infiltrator-desktop::service` |
| **P04-02** | **macOS Privileged Helper Tool 标准集成**：接入基于 `SMJobBless` / launchd 的特权助手，安全注入网络接口与路由表。 | P0 | 已落地 | `infiltrator-desktop::service` |
| **P04-03** | **Linux 多发行版特权治理**：自动化 `setcap cap_net_admin,cap_net_bind_service+ep` 检查与 polkit 动作规则向导。 | P0 | 已落地 | `infiltrator-desktop::service` |
| **P04-04** | **TUN 网络堆栈深度切换**：支持 `gVisor`, `Mixed`, `System` 堆栈即时切换与 MTU 自适应协商。 | P1 | 已落地 | `infiltrator-core::tun` |
| **P04-05** | **Strict Route 与路由防泄漏**：严格路由模式隔离，阻断非 TUN 接口直接外联与 IPv6 旁路泄露黑洞。 | P1 | 已落地 | `infiltrator-core::tun` |
| **P04-06** | **Full Cone NAT (NAT1) 穿透支持**：提供 Endpoint-Independent Filtering 开关与参数优化，提升游戏联机与 P2P 表现。 | P2 | 已落地 | `infiltrator-core::tun` |
| **P04-07** | **系统休眠/唤醒网络自愈**：监听 OS 电源管理事件，唤醒后自动重置 TUN 路由、清除死连接并重新探活。 | P0 | 已落地 | `mihomo-platform::power` |
| **P04-08** | **网络漫游与默认网关动态迁移**：监听 Wi-Fi 切换、以太网插拔与 VPN 启停事件，动态更新 TUN 网关与路由表。 | P1 | 已落地 | `mihomo-platform::interface_watcher` |
| **P04-09** | **PAC 引擎与智能旁路编辑**：内置 PAC HTTP 服务，支持根据规则编译 `FindProxyForURL` 与 `ProxyOverride` 旁路列表。 | P2 | 已落地 | `infiltrator-core::pac_generator` |
| **P04-10** | **崩溃异常安全自愈看门狗**：在主进程被异常终止时，由独立看门狗强制清退系统代理并还原路由表。 | P0 | 已落地 | `mihomo-platform::crash_reporter` |

---

### 五、运行态遥测、连接深度审计与全链路流控 (Runtime Telemetry, Connection Audit & Traffic Analytics)

| 编号 | 任务与差距项 | 优先级 | 状态 | 涉及模块 |
|:---|:---|:---:|:---:|:---|
| **P05-01** | **连接多维时延下钻与链路透视**：展示单条连接的 DNS、TCP 握手、TLS 握手耗时、TTFB 及前置跳板代理链。 | P2 | 已落地 | `infiltrator-core::traffic_audit` |
| **P05-02** | **离线 GeoIP / ASN 归属地元数据富化**：集成高速 MMDB 索引，实时显示目标 IP 的 ASN、运营商、机房 IDC 标签。 | P1 | 已落地 | `infiltrator-core::geo_lookup_cache` |
| **P05-03** | **细粒度连接阻断与批量清退**：支持按 PID、目标域名后缀、规则组、出站节点一键批量掐断所有相关连接。 | P1 | 已落地 | `infiltrator-core::idle_connection_sweeper` |
| **P05-04** | **长周期流量时序存储与报表**：嵌入式时序数据库记录按日/周/月的订阅流量走势、节点使用排行与分应用报表。 | P2 | 已落地 | `infiltrator-core::traffic_audit` |
| **P05-05** | **真实下行带宽测速 (Speedtest) 引擎**：发起真实多线程数据拉取，测量节点实际下行带宽、峰值速率、丢包率与抖动。 | P1 | 已落地 | `infiltrator-core::diagnostics` |
| **P05-06** | **Sniffer 嗅探状态机监控**：实时展示 HTTP Host、TLS SNI、QUIC SNI 嗅探抓包日志与域名覆写记录。 | P2 | 已落地 | `infiltrator-domain::sniffer` + `infiltrator-core::sniffer_io` |
| **P05-07** | **遥测流有界 RingBuffer 调优**：连接流与日志流采用无锁定长环形缓冲与丢弃策略，杜绝内存膨胀。 | P0 | 已落地 | `infiltrator-core::flow_control` |
| **P05-08** | **内置 Traceroute / MTR 路由跳数诊断**：集成可视化 MTR 工具，诊断本地 -> 代理入口 -> 目标服务器路由跳数与丢包。 | P2 | 已落地 | `infiltrator-core::diagnostics` |
| **P05-09** | **公网出口 IP 多源交叉校验**：多源并发探测出口 IP，交叉比对检测直连真实 IP 泄露与 WebRTC 暴露风险。 | P1 | 已落地 | `infiltrator-core::diagnostics` |
| **P05-10** | **日志高级过滤与一键脱敏导出**：多级别日志过滤、正则关键词高亮匹配与敏感凭据/真实 IP 自动脱敏导出。 | P1 | 已落地 | `infiltrator-core::redact` |

---

### 六、进程分流、应用感知与系统互通 (Process Split Tunneling & System App Containment)

| 编号 | 任务与差距项 | 优先级 | 状态 | 涉及模块 |
|:---|:---|:---:|:---:|:---|
| **P06-01** | **跨平台进程枚举与图标提取**：Windows PE、macOS Bundle、Linux `/proc` 进程扫描，提取路径、签名与图标。 | P1 | 已落地 | `infiltrator-desktop::process_enumerator` |
| **P06-02** | **进程分流规则热注入**：动态无感注入 `PROCESS-NAME` / `PROCESS-PATH` 规则，无需重启核心或中断连接。 | P1 | 已落地 | `infiltrator-core::app_routing` |
| **P06-03** | **Windows UWP 回环隔离解除工具**：集成 CheckNetIsolation 界面，一键扫描、搜索与解除微软商店应用回环限制。 | P1 | 已落地 | `infiltrator-desktop::uwp_loopback` |
| **P06-04** | **WSL2 / Hyper-V / Docker 网桥自动路由**：自动感知 WSL2 镜像网络、Hyper-V 虚拟网卡与 Docker 容器流量。 | P2 | 已落地 | `infiltrator-desktop::proxy` |
| **P06-05** | **主流游戏专线分流与 UDP 加速预设**：内置主流游戏平台（Steam/Epic/Riot/Blizzard/EA 等）规则模板与低延迟专线绑定。 | P2 | 已落地 | `infiltrator-core::rules` |
| **P06-06** | **Android Per-App Split Tunneling 深度集成**：UniFFI 联动 `VpnService.Builder` 黑白名单模式、系统应用过滤与搜索。 | P0 | 已落地 | `infiltrator-android::uniffi_api::app_routing` |
| **P06-07** | **分应用实时带宽监控与流量排行**：按应用进程聚合展示当前上传/下载瞬时速率与后台流量消耗排行。 | P2 | 已落地 | `infiltrator-core::app_routing` |
| **P06-08** | **跨平台进程别名与多架构标准化**：建立跨平台进程别名映射表，将多平台不同二进制名抽象为统一实体。 | P2 | 已落地 | `infiltrator-core::app_routing` |
| **P06-09** | **沙盒与容器进程环境识别**：识别 Flatpak、Snap、AppImage 等 Linux 沙盒环境并穿透映射至真实宿主进程。 | P3 | 已落地 | `infiltrator-desktop::process_enumerator` |
| **P06-10** | **进程分流黑白双模全局面板**：提供“仅代理勾选应用（白名单）”与“仅绕过勾选应用（黑名单）”自由切换。 | P1 | 已落地 | `infiltrator-core::app_routing` |

---

### 七、订阅清洗、转换与配置生命周期 (Subscription Pipeline, Subconverter & Atomic Lifecycle)

| 编号 | 任务与差距项 | 优先级 | 状态 | 涉及模块 |
|:---|:---|:---:|:---:|:---|
| **P07-01** | **全格式节点 URI 解析与无损双向转换**：支持 `ss`, `vmess`, `vless`, `trojan`, `hysteria2`, `tuic`, `wireguard` 各种方言无损互转。 | P1 | 已落地 | `infiltrator-core::profile_converter` |
| **P07-02** | **多订阅深度清洗流水线 (Filter Pipeline)**：支持正则替换、倍率重算、前缀后缀、国家代码标准化与协议过滤。 | P1 | 已落地 | `infiltrator-core::filter` |
| **P07-03** | **多订阅聚合引擎 (Multi-Profile Aggregator)**：勾选多个订阅自动去重合并，按国家/延迟自动生成策略组拓扑。 | P1 | 已落地 | `infiltrator-core::profile_converter` |
| **P07-04** | **订阅 User-Agent 动态伪装与防屏蔽**：内置 User-Agent 库（Clash.Meta, ClashVerge, Shadowrocket 等）防止机场后端拦截。 | P1 | 已落地 | `infiltrator-domain::subscription` |
| **P07-05** | **订阅套餐用量与到期智能预警**：解析 `subscription-userinfo` 头，在流量超 80%/90% 或临期时推送系统通知。 | P1 | 已落地 | `infiltrator-domain::subscription` |
| **P07-06** | **防 Cloudflare / 5秒盾订阅下载器**：支持自定义 HTTP Headers、Cookie 注入与经现有代理拉取更新。 | P1 | 已落地 | `infiltrator-core::subscription_io` |
| **P07-07** | **配置 Diff 可视化比对与智能回滚**：更新前后直观呈现节点增删改差异，支持一键回滚历史快照。 | P2 | 已落地 | `infiltrator-domain::backup` + `infiltrator-core::history` |
| **P07-08** | **配置写入原子事务与语法预检**：在应用新配置前调用独立沙箱预检语法语义，失败时零副作用原子回滚。 | P0 | 已落地 | `infiltrator-core::apply` |
| **P07-09** | **订阅定时轮询调度与并发防重入**：基于 Cron/间隔的调度器，支持断线指数退避与 SingleFlight 并发锁。 | P1 | 已落地 | `infiltrator-core::scheduler` |
| **P07-10** | **自建单节点表单化管理与二维码导入**：支持表单化添加/编辑个人 VPS 节点与扫描/解析二维码导入。 | P2 | 已落地 | `infiltrator-core::profile_converter` |

---

### 八、可编程动态扩展、脚本沙箱与生态集成 (Programmable Scripting Sandbox, Hooks & Extensibility)

| 编号 | 任务与差距项 | 优先级 | 状态 | 涉及模块 |
|:---|:---|:---:|:---:|:---|
| **P08-01** | **工业级 JavaScript (QuickJS/V8) 脚本引擎**：支持业界事实标准的 JS 脚本运行时，兼容 Clash 社区 JS 扩展脚本。 | P1 | 已落地 | `infiltrator-core::script_engine` |
| **P08-02** | **配置生命周期全链路 Hook 注入**：支持 `pre-download`, `post-download`, `pre-merge`, `post-merge` 等生命周期钩子。 | P1 | 已落地 | `infiltrator-core::script_engine` |
| **P08-03** | **基于代码的动态策略组生成器**：根据节点延迟、国家代码、协议类型动态计算组装策略组和分流规则。 | P2 | 已落地 | `infiltrator-core::script_engine` |
| **P08-04** | **脚本沙箱资源与死循环熔断防护**：严格限制脚本执行时间（<500ms）与内存上限（<64MB），防止卡死。 | P0 | 已落地 | `infiltrator-core::script_engine` |
| **P08-05** | **脚本调试器与独立 Console 控制台**：支持 `console.log()` 输出、语法高亮与即时输入测试数据调试。 | P2 | 已落地 | `infiltrator-core::script_engine` |
| **P08-06** | **外部 Web Dashboard 深度无缝嵌入**：内置 Metacubexd / Yacd-Meta 静态托管服务器与自动 Secret 凭据握手。 | P2 | 已落地 | `infiltrator-desktop::runtime` |
| **P08-07** | **开放 RESTful API 与 Webhook 联动**：向第三方工具（Raycast/Alfred/快捷指令）开放节点切换、模式切换与测速接口。 | P2 | 已落地 | `infiltrator-admin::admin_api` |
| **P08-08** | **插件市场与社区扩展生态**：支持在 UI 上一键安装、升级第三方规则集、DNS 预设与主题包。 | P2 | 已落地 | `infiltrator-core::mixin` |
| **P08-09** | **多内核版本热切换与故障回滚**：支持在 Alpha 开发版、Meta 稳定版、开源版之间一键下载、校验与无感切换。 | P1 | 已落地 | `mihomo-version` |
| **P08-10** | **扩展配置包一键导出与跨端分享**：支持将自定义脚本、Mixin 模板与规则打包为单文件并一键分享导入。 | P3 | 已落地 | `infiltrator-domain::backup` + `infiltrator-core::backup_io` |

---

### 九、原生桌面与移动端交互体验及渲染性能 (Desktop/Mobile UX & Virtualized Performance)

| 编号 | 任务与差距项 | 优先级 | 状态 | 涉及模块 |
|:---|:---|:---:|:---:|:---|
| **P09-01** | **原生系统托盘极致响应与动态角标**：支持托盘动态网速角标、右键直选策略组节点与 Linux AppIndicator 兼容。 | P1 | 已落地 | `infiltrator-desktop::tray_badge` |
| **P09-02** | **独立桌面网速悬浮窗与 Mini 控制面板**：支持吸附、置顶、鼠标穿透的迷你网速悬浮窗与极速 Mini 控制面板。 | P2 | 已落地 | `infiltrator-iced` |
| **P09-03** | **上万节点与海量连接高性能虚拟化列表**：实现虚拟滚动 (Virtual Scrolling) 视口裁剪，杜绝海量节点卡顿。 | P1 | 已落地 | `infiltrator-iced::view` |
| **P09-04** | **现代操作系统材质与原生视觉融合**：支持 Win11 Mica/Acrylic 材质、macOS Vibrancy 毛玻璃与 Linux 分数缩放。 | P2 | 已落地 | `infiltrator-desktop::display_adapter` |
| **P09-05** | **跨平台全局快捷键与防冲突**：支持自定义全局快捷键（切换代理、开闭 TUN、唤出搜索）与冲突规避。 | P1 | 已落地 | `infiltrator-desktop::shortcut_manager` |
| **P09-06** | **窗口几何记忆与多显示器 DPI 自适应**：精准记录并复原窗口坐标、尺寸与多显示器 DPI 缩放重绘。 | P2 | 已落地 | `infiltrator-iced` |
| **P09-07** | **GPU 加速实时网速波形图与拓扑图**：提供平滑网速折线波形图、节点延迟分布图等高性能数据可视化。 | P2 | 已落地 | `infiltrator-iced::view` |
| **P09-08** | **跨平台主题引擎与系统深浅色跟随**：毫秒级感知系统深浅色外观变更，支持自定义 Token 配色方案。 | P2 | 已落地 | `infiltrator-iced::view::theme` |
| **P09-09** | **全键盘可访问性与快捷导航**：支持纯键盘操控（上下选节点、快捷聚焦搜索、退出与回车确认）。 | P2 | 已落地 | `infiltrator-iced` |
| **P09-10** | **国际化文案完整度与术语规范化**：消除硬编码文本，支持简繁中/英/日/韩多语言及代理专有名词释义。 | P1 | 已落地 | `infiltrator-shared::locales_table` |

---

### 十、安全架构、凭据保险库与跨设备漫游 (Security Hardening, Vault Storage & Multi-Device Sync)

| 编号 | 任务与差距项 | 优先级 | 状态 | 涉及模块 |
|:---|:---|:---:|:---:|:---|
| **P10-01** | **系统级原生凭据保险库 (OS Keyring)**：WebDAV 密码、订阅 Token 100% 收敛至 Windows Credential / macOS Keychain / Linux Secret Service。 | P0 | 已落地 | `infiltrator-core::settings` |
| **P10-02** | **External Controller 端口严格安全加固**：强制随机强 Token 认证、CORS/CSRF 拦截与局域网 mTLS 双向校验。 | P1 | 已落地 | `infiltrator-domain::config` |
| **P10-03** | **端到端加密 (E2EE) 配置漫游同步**：WebDAV 同步支持客户端本地 AES-256-GCM / ChaCha20 加密，实现零知识备份。 | P1 | 已落地 | `sync-engine` |
| **P10-04** | **版本向量时钟与智能三向合并 (3-Way Merge)**：基于 Vector Clock 版本追踪，提供 Local/Remote/Base 差异可视化合并器。 | P1 | 已落地 | `infiltrator-core::vector_clock` |
| **P10-05** | **局域网 P2P 扫码一键加密快传**：基于 mDNS 发现与 TLS 传输，手机端扫码秒级完成配置与订阅克隆。 | P2 | 已落地 | `infiltrator-domain::backup` + host transport |
| **P10-06** | **配置与私钥加密导出备份 (.encpkg)**：打包所有订阅、规则、Mixin 与设置为主密码加密的归档文件。 | P1 | 已落地 | `infiltrator-domain::backup` + `infiltrator-core::backup_io` |
| **P10-07** | **运行时内存脱敏与零化安全 (Zeroize)**：敏感凭据生命周期结束后全面接入 `zeroize` 覆写，防止内存扫描泄露。 | P1 | 已落地 | `infiltrator-core::zeroize_guard` |
| **P10-08** | **隐私保护与日志脱敏工作流**：导出诊断报告与日志时，自动对所有域名、真实 IP、订阅 Token 进行模糊化。 | P1 | 已落地 | `infiltrator-core::redact` |
| **P10-09** | **恶意订阅与节点配置注入攻击防御**：静态安全审计订阅下发的危险字段（篡改控制器端口、注入脚本等）并拦截。 | P0 | 已落地 | `infiltrator-domain::subscription` |
| **P10-10** | **零第三方未授权遥测声明**：崩溃日志与诊断完全本地自闭环，零隐式第三方统计代码。 | P0 | 已落地 | `mihomo-platform::crash_reporter` |

---

## 落地实施批次排期

- **第 1 批（核心网络底座与协议矩阵）**：P01-01~04（现代协议与 Reality/Vision/Hysteria2/SS2022）、P02-01~02（高级规则与逻辑规则 AST）、P03-01~04（Fake-IP 与 Nameserver 策略矩阵）。
- **第 2 批（特权服务与系统网络集成）**：P04-01~05（Windows Service / macOS Helper / Linux polkit / Strict Route）、P06-01~03（进程枚举 / 规则热注入 / UWP 回环解除）。
- **第 3 批（订阅流水线与流控遥测）**：P07-01~06（全格式转换 / 过滤流水线 / 套餐预警）、P05-01~05（多维时延 / GeoIP 离线富化 / 真实测速引擎）。
- **第 4 批（扩展引擎与凭据安全）**：P08-01~05（QuickJS 脚本沙箱 / 动态策略组）、P10-01~07（OS Keyring / E2EE 同步 / 向量时钟 / 内存 Zeroize）。
- **第 5 批（桌面原生体验与渲染极致优化）**：P09-01~10（虚拟化滚动列表 / 动态托盘 / 快捷键 / 材质 / i18n 规范化）。
