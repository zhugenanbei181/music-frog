# MusicFrog 工程 TODO · 过去佛（2条待验证；归档见 [TODO.md](../TODO.md) 第一百零八批次）

> 层级：L4

## 0. 编写规则

- 本文件里的工作仍未办结：已有实现包等待验证批次、正式产品采用或最终证据复验。已有局部 PASS 不自动变成完整验收。
- **四佛体系（无体外孤岛看板）：**模拟器（`-M`）与 ARM64 真机（`-D`）实机验收条件直接内聚在对应条目的“待验证”中；不设外部 `MACHINE_TODO` 孤岛看板，一处看清代码实现、单元测试与实机验收全貌。
- **执行标准（2026-10-20 起强制）：** 底层结构不够就扩 owner 结构，禁止再塞 `bool`/`Option`；组合优于继承；field 归 field、表现归表现、事务归 owner；每个事实只有一个真源，不留第二份缓存；非身份改动不碰 wire/快照/语料，身份改动集中一次做完。
- **本板只放可推条目：**条目的下一个可执行动作必须能在当前环境完成（统一 nextest/clippy/guard 批次、本地 Gradle 与模拟器 M1、本地真实引擎二进制 smoke、`cargo package`/dry-run）。剩余闭环动作全部依赖外部环境（ARM64 真机、受控 tag CI、用户 secrets、crates.io 上架凭证、异构目标机）的条目，整条迁入[斗战胜佛](CODE_TODO_BLOCKED.md)，本板不得预占。
- 每次先读 task 的验收、依赖与当前证据身份，执行最低充分验证；前置未满足保持 Blocked，验证发现实现缺口则把修复任务回流[现在佛](CODE_TODO.md)。凡发现底层架构未决或在上层修补规避底层问题的条目，不得标为 Integrated 或通过验证，必须将修复与底层裁决任务回流现在佛。
- 验证收口通过后，把条目从看板删除并在 [TODO.md](../TODO.md) 登记归档；不在本文件长期驻留已验证成果。
- `Integrated` 表示实现包已进入工作区、只剩验证/收口；位置不授予验证结论。若仅缺设备/真机证据，在待验证字段中明确标注，本条保持 Integrated 并注明设备依赖；只有实现依赖任务尚未完成时才标 Blocked。外部不可解条件（无真机、等 tag CI、等用户 secrets）不阻塞统一验证批次对其它条目的批量解锁。
- 迁移必须同次移出/移入（含整条正文），保持稳定 task ID、owner、依赖、验收与 points。
- **并行工流与验证纪律（2026-10-20 起强制）：** 并行开发按文件所有权切分，编译不通过的半成品不得放进会被 `cargo nextest --workspace`、结构门或 capture 构建扫到的路径（WIP 先放草稿或 `.evidence/wip/`）；开发期只跑受影响包，全仓 `cargo nextest --workspace` 与全量结构门统一放到收尾批次只跑一次；共享 `/tmp` 配额、cargo lane 争用与合成器起栈失败等外部故障先判定归属，只修本切片能修的那一类。
- 当前证据行只保留 run ID 与计数；完整证据文本见 `.evidence/CODE_TODO_PAST_EVIDENCE.md`（本地，不入库）。

## 一、Android VPN 生命周期与系统服务（0条，已清空；归档见 [TODO.md](../TODO.md) 第一百零六批次）
## 二、Android IPC 与 Kotlin Host 边界（0条，已清空；归档见 [TODO.md](../TODO.md) 第一百零四批次）

## 三、UI 表面与组件（0条，已清空；归档见 [TODO.md](../TODO.md) 第一百零四批次）

## 五、发布流水线与 parity 管线（0条，已清空；归档见 [TODO.md](../TODO.md) 第一百零四批次）

## 十四、真实二进制与 Android 取证（0条，已清空；归档见 [TODO.md](../TODO.md) 第一百零六批次）
## 十六、真实引擎测速（0条，已清空；归档见 [TODO.md](../TODO.md) 第一百零四批次）

### 10.4 Layer 3: Bevy UI 0.20 渲染管线与 WESL 材质深化（0条，已清空；归档见 [TODO.md](../TODO.md) 第一百零四批次）

### 12.1 Slint 桌面 GPU 后端（0条，已清空；EFFECTS-036 与 SLINTUI-001 已由第一百零五批次办结归档）

### 12.3 画质自适应与读屏树（0条，已清空；EFFECTS-049 外部环境读屏腿已转入斗战胜佛 §六）
### 12.4 离屏取证与验收报告（0条，已清空；EFFECTS-050 外部环境 Niri CI 腿已转入斗战胜佛 §六）
### 12.18 Slint 无框一体化窗口（0条，已清空；归档见 [TODO.md](../TODO.md) 第一百零四批次）

## 12.38 Android 12+ 动态壁纸 Monet 取色与分应用代理高清图标选择器交付（0条，已清空；归档见 [TODO.md](../TODO.md) 第一百零四批次）

## 12.47 Android 三 flavor 可重复的 Forest 默认业务采集与显式主题巡检交付（0条，已清空；归档见 [TODO.md](../TODO.md) 第一百零六批次）

## 12.57 产品、桌面托盘与 Android 图标资产一致性审计交付（0条，已清空；归档见 [TODO.md](../TODO.md) 第一百零四批次）

## 12.63 三端信息架构与交互模型对齐（词表/字段/语义色）交付（0条，已清空；归档见 [TODO.md](../TODO.md) 第一百零四批次）

## 12.64 桌面托盘宿主与彩色状态图标交付（0条，已清空；归档见 [TODO.md](../TODO.md) 第一百零四批次）

## 12.65 自定义无框窗口标题栏（CSD）、实时流量胶囊与多端统一系统窗管交付（0条，已清空；归档见 [TODO.md](../TODO.md) 第一百零四批次）

## 12.66 桌面侧栏系统材质效果与诚实降级交付（0条，已清空；归档见 [TODO.md](../TODO.md) 第一百零四批次）

## 12.71 Iced 流量页的审计范围与维度接真交付（1条待验证）

| ID | 状态 / 优先级 | points | owner | 依赖 |
| --- | --- | --- | --- | --- |
| `TRAFFICAUDIT-001` | `Integrated` / P1（b194 域片 + b195 全链接真，待统一批次办结） | 3 | infiltrator-domain（`traffic_audit.rs` 的桶与窗口归约）+ infiltrator-application（`traffic_audit_application.rs` 的喂数与读回）+ infiltrator-presentation（`traffic_audit_options.rs` 的范围/维度词表）+ infiltrator-iced（`state/traffic.rs`、`view/runtime/traffic.rs`） | 无（域 owner 已存在，本批已把喂数、读回与两个控件一次接真） |

**`TRAFFICAUDIT-001` 流量页的两个控件是死的，而事实 owner 在域层从未被喂（b192 实测立项，b194 域片落地，b195 接真）**：
① **域层已有完整 owner**：`crates/infiltrator-domain/src/traffic_audit.rs` 提供 `TrafficAuditAccumulator` 与 `top_processes_by_traffic` / `top_nodes_by_traffic` / `top_groups_by_traffic` / `top_domains_by_traffic` / `top_countries_by_traffic`、`get_process_hourly_series` / `get_node_daily_series` / `get_domain_hourly_series`、`bypass_vs_proxied_ratio` 等 typed 事实。
② **它没有任何消费者**（立项时实测）：全仓搜索 `TrafficAuditAccumulator`（排除 domain crate 自身）**0 处**——四个表面都没有实例化过它。b195 已把它接成 Iced 的审计卡唯一事实源。
③ **Iced 视图曾自己折了一份，业务逻辑沉淀在视图里**：`view/runtime/traffic.rs` 把 `TrafficDimension` 枚举声明在视图内，并用 5 个私有函数 `compute_host_rankings` / `compute_device_rankings` / `compute_proxy_rankings` / `compute_process_rankings` / `compute_dimension_rankings` 直接读 `state.diag.connections` 折叠排名——同一事实两份真源。b195 已把这 5 个函数、视图内的 `TrafficDimension` 与 `HostTrafficRank` 全部删除。
④ **后果是用户可见的两个死控件**：`segmented_control(&time_range_labels, 0, |_| Message::Noop)`（1h/24h/7d/30d）与 `segmented_control(&dim_labels, 0, |_| Message::Noop)`（Domains/Devices/Proxies/Processes），两者都画出「选中」态却对任何点击都回 `Noop`；b192 的守卫把这两处作为唯一两个真实位点登记进只减台账。b195 后台账已清空（`registered=0`）。
⑤ **收口顺序（自底向上，不做第二种真源）**：先喂域 owner，再由 presentation 投影，最后由 Iced 删 fold 并把控件接真——三步已在 b194（域：桶/窗口/增量）+ b195（application 喂数与读回 + presentation 词表 + Iced 接真）内完成。
⑥ **验收**：
   - `grep -rn "compute_.*_rankings" crates/infiltrator-iced` → **0**；`grep -rn "|_| Message::Noop" crates/infiltrator-iced/src/view` → **0**（实测）。
   - `iced-dead-input-guard.py --mode enforce` → **findings=0 registered=0**，`--self-test` PASS（两条死控件已消失，守卫报台账陈旧并已用 `--update-inventory` 清空）。
   - 四维度 × 四时间范围在 domain/application 有覆盖：domain nextest **631/631**（新增 6 条：窗口分层/整流量系列/细粒度裁剪/清空）、application nextest **675/675**（新增 5 条：喂数映射、未上报设备不归属、零增量不记账、读回只答被问的窗口、端到端增量）、presentation 词表 3 条、Iced **634/634**（选区移动与窗口裁剪）。
   - 像素：**两个控件在同一页的不同选中态下有真实像素回执**——捕获装置新增第 8 列 `traffic_audit`（`<range>:<dimension>`，走与控件同一条生产消息路径），发布语料新增 `runtime-traffic-audit-standard` 与 `runtime-traffic-audit-compact`（`30d:process`）两帧；同场景同窗口的 `24h:host` 与 `30d:process` 诊断对实测 **131,853 像素**不同（差异盒 `(280,104)-(1126,740)`），且两帧本身已在 186 帧发布批次里被判定器判过。
   - 发布批的 **29 帧 PNG** 与 HEAD 不同，已逐像素归因而不是记作「只加了两帧」：差异全在 runtime 页身（`x∈[280,1125]`、`y∈[79,740]`），是圆角边框、徽标、字形边缘的 8-bit 抗锯齿（单帧 21,721 px、通道差绝大多数 ≤4），侧栏/页脚/标题栏逐字节不变。根因是 `AlignSection` 的 `target = (translation.y + (section.y - clip.y)).clamp(0, max_offset)` 依赖第三张卡高度；b195 把速率卡的 4 个范围 pill 移进新审计卡并换成窗口分桶条 + 排名行，内容总高改变后 `connections` / `logs` / 测速详情等帧整体滚到不同小数 y，全页 AA 换相。四次复采排除抖动与环境：同二进制两次逐字节相同、`RUSTICL_ENABLE` unset 后相同、只回退 `traffic.rs` 到 HEAD 即回 HEAD 字节、HEAD 只加零高度 `Space` 仍与 HEAD 相同；两次半回退实验因 sccache 给了错位二进制已用 `cargo clean -p` + binary sha 复核撤回，源码与备份 `diff -rq` 全等。归属是本批控件树的可复现可见身份变化，不是判定器漏洞，也不声称 29 帧各有新业务。

**待验证范围**：
- 单元（本机可复跑）：domain 631/631、application 675/675、presentation（含 `traffic_audit_options` 3 条）、`infiltrator-iced --features capture-tools` 634/634。
- 守卫（本机可复跑）：`iced-dead-input-guard --self-test` PASS / `--mode enforce` 0/0、`iced-sidebar-nav-guard` 0、`import`/`line`/`typed-state` 0、`generate_unified_matrix.py --check`（iced+slint）逐行一致、`python3 -m unittest discover -s scripts/quality/tests` **367/367 OK**。
- 像素（本机可复跑）：Iced 发布批 `20261111T015000Z_ac6d8f958d10_dirty_353763`（**186/186**，453s）、`space_utilization --surface iced --enforce` **violations=0**、`receipt-freshness 0`；Slint 批 `20261110T220049Z_7c92aaf890f3_dirty_2457766` 199 帧 violations=0 逐字不变。
- 统一批次：`cargo nextest --workspace`、全仓 clippy `--all-targets -D warnings`、全仓结构门（本批只跑了受影响包的定向腿与相关门）。
- 诚实边界：控件的**点击**本身不进像素回执（捕获台架按设计不投指针），像素证据来自同一条生产消息路径的选区行；Slint 与 Bevy 表面尚未消费这份读回，其审计页由各自条目收口；`FLAGART-001`（Iced/Bevy 旗面依赖平台 emoji 字形）与本条无关。
- 诚实边界（发布范围）：186 帧发布批 `20261111T015000Z_ac6d8f958d10_dirty_353763` 的 binary sha256 `af62c849…` 与本条源码一致；29 帧 AA 变化随 b195 的 runtime 控件树重排，需在统一批次连同全仓门一起复核，不得把它写成「只有两张新回执变化」或「判定器/环境缺陷」。

**证据身份**：domain 631/631、application 675/675、presentation 3 条、iced 634/634、dead-input 0/0、quality 脚本单测 367/367、发布批 `20261111T015000Z_ac6d8f958d10_dirty_353763`（186/186）、诊断对 `20261111T012325Z_..._191669` / `20261111T012342Z_..._192980`（131,853 px）。

## 12.72 Slint 标准控件回执不再随宿主主题翻转（1条待验证）

| ID | 状态 / 优先级 | points | owner | 依赖 |
| --- | --- | --- | --- | --- |
| `SLINT-022` | `Integrated` / P1（b181 实测立项、b196 交付，待统一批次办结） | 2 | `crates/infiltrator-slint/ui/lumen-app.slint`（明暗通道）+ Slint 供应商补丁说明 + `scripts/quality/slint-theme-channel-guard.py` | `SLINT-021`（同一条明暗事实） |

**`SLINT-022` 标准控件的滚动条墨色不再读平台（b181 实测立项，b196 收口，2 pt / P1 / `Integrated`）**：
① **真根因是第二条明暗通道，不是「Slint 没有写入口」**：b181 只把 vendored `@material` 的 `MaterialPalette.scheme` / `Elevation.dark` 收归 `application-dark-mode`；页面 `app_body.slint` 用的却是 `std-widgets.slint` 的 `ScrollView`，其 1px 句柄描边读内置 material style 的 `MaterialPalette.border`，而该 style 的 `MaterialPalette.color-scheme` 默认来自 `ColorSchemeSelector.color-scheme = SlintInternal.color-scheme`（进程级平台方案）。`std-widgets` 导出的 `Palette.color-scheme` 是 `in-out` 全局，因此 b181 记录的「Slint 1.18 未公开应用侧写入口」被本批实测推翻；不是要自建滚动条或给台架加布尔，而是把同一条已裁决事实喂进第二个消费者。
② **修在唯一 owner**：`ui/lumen-app.slint` 显式 `import { Palette as StdPalette } from "std-widgets.slint"`，并在根窗口写 `LumenTheme.dark-mode` 与 `MaterialPalette.application-dark-mode` 的**同两处**（`init` 与 `update-color-scheme()`）同笔写 `StdPalette.color-scheme = root.application-dark-mode ? ColorScheme.dark : ColorScheme.light`。`MaterialPalette.color-scheme` 仍保留为平台观测，只由 `platform-dark-mode`（偏好为 `system` 时）合法读取，不引入第二种真源。
③ **机器不变量**：`slint-theme-channel-guard.py` 从 6 项 planted 扩到 **8/8**，新增 `STD_CHANNEL_WRITE` 判据——`StdPalette.color-scheme` 写点数必须等于 `LumenTheme.dark-mode` 写点数、每个写表达式必须读 `application-dark-mode`；两条新负例分别把标准控件通道删掉、钉死成 `ColorScheme.dark`，实测都会咬。原「写点不喂 Material」负例同步改成同时删两个通道，保持它的原始语义。
④ **真实像素验收（两种宿主主题）**：`gsettings set org.gnome.desktop.interface color-scheme prefer-light` 下诊断批 `20261111T030014Z_bce353de8462_dirty_813495`，`prefer-dark` 下诊断批 `20261111T030327Z_bce353de8462_dirty_837205`，同一 `proxies-dark` 帧两次 `image.png` **sha256 同为 `e43974c613e4…`、全帧与右缘 2px 带差异均为 0 px**；宿主设置随后恢复 `prefer-light`。修复后的 light 宿主诊断帧又与已发布 `proxies-dark.png` 逐字节相同，说明现存发布语料本来就是 light 宿主状态，本批把「碰巧一致」变成结构保证，因此 **199 帧无需重采、零像素变更、`receipt-freshness 0`**。
⑤ **证据（L）**：`cargo check -p infiltrator-slint --features capture-tools` exit 0、`slint-syntax-guard` 100% PASS、`slint-dead-input` findings=0 / allowed=8、`slint-source-import-guard` 718 files / 36 patched / 8,726,178 bytes、`slint-theme-channel-guard --mode enforce` findings=0 且 `--self-test` **8/8**、两宿主诊断帧逐字节相同、`space_utilization --enforce` 199 帧 violations=0。
⑥ **诚实边界**：A/B 只在同一台机器、同一私有 KWin/niri 台架上切换宿主明暗设置；未覆盖 Wayland/Qt/其它桌面环境对 `SlintInternal.color-scheme` 的写入差异，也不声称标准控件的所有颜色都归应用主题（这里只裁决滚动条描边所依赖的 `color-scheme` 选择）。全仓 `--workspace` nextest/clippy、结构门全腿与 `resolve_surface_evidence.py` 仍待统一批次；另一会话未提交的 Bevy 像素仍是全仓身份门唯一红腿。

**待验证范围**：
- 本机可复跑：`slint-theme-channel-guard --self-test`（8/8）与 `--mode enforce`（0）、`cargo check -p infiltrator-slint --features capture-tools`、`slint-syntax-guard`、`slint-source-import-guard`、两宿主诊断批的 PNG sha256 与右缘 2px 带比对、`space_utilization --enforce` 199 帧 0 违规、`receipt-freshness 0`。
- 统一批次：`cargo nextest --workspace`、全仓 clippy `--all-targets -D warnings`、全仓结构门全腿。
- 诚实边界：本批不重采、不改任何 PNG / manifest；`Integrated` 不等于办结，统一验证通过后登记 archived points。

**证据身份**：`slint-theme-channel-guard --self-test 8/8`、`cargo check --features capture-tools` exit 0、诊断批 `20261111T030014Z_..._813495`（light 宿主）与 `20261111T030327Z_..._837205`（dark 宿主）的 `proxies-dark/image.png` sha256 `e43974c613e4…` 相同、发布帧 `proxies-dark.png` 与 light 宿主诊断帧逐字节相同。
