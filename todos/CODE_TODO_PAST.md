# MusicFrog 工程 TODO · 过去佛（11条待验证；归档见 [TODO.md](../TODO.md) 第一百零八批次）

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

## 0.1 2026-11-11 第二百零六统一验证批次

- `cargo nextest run --locked --workspace --features infiltrator-iced/capture-tools,infiltrator-bevy-ui/capture-tools --no-fail-fast --build-jobs 4 --test-threads 4`：**6716/6716 passed，15 skipped**。
- `cargo clippy --workspace --all-targets -- -D warnings`、`cargo fmt --all --check`：exit 0。
- `resolve_surface_evidence.py`：`status=pass registry=True discovery=True anchors=360 dangling=0 cells=180 aligned=0 unclosed=180`；本次未给所有表面选定具体 product build IDs 与 pixel receipts，故不把 `ready` cell 误记为已验收。
- `check-structure.sh`：唯一红项为另一会话持有的 Bevy `runtime-dark` / `overview-dark` PNG 别名差异；保留原 owner 文件，四条 Integrated 不办结。
- 本批修正三处真实门禁缺陷：Bevy Android 帧观测把路径绑定到 `Android { internal_data_path }` typed variant，消除 `capture-tools` Clippy 未使用参数；Iced 导航唯一性计数只在其 capture/test 消费端编译；Bevy `rules-list-editor` scenario anchor 改为 HEAD 实际存在的测试名。定向 Bevy **831/831**、Iced **638/638** 均通过。

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
- 统一批次：本文件 §0.1 的 b206 结果已更新；全仓测试/Clippy/fmt 通过，但结构门别名与跨表面产品回执仍未收口，本条保持 `Integrated`。
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
- 统一批次：本文件 §0.1 的 b206 结果已更新；全仓测试/Clippy/fmt 通过，但结构门别名与跨表面产品回执仍未收口，本条保持 `Integrated`。
- 诚实边界：本批不重采、不改任何 PNG / manifest；`Integrated` 不等于办结，统一验证通过后登记 archived points。

**证据身份**：`slint-theme-channel-guard --self-test 8/8`、`cargo check --features capture-tools` exit 0、诊断批 `20261111T030014Z_..._813495`（light 宿主）与 `20261111T030327Z_..._837205`（dark 宿主）的 `proxies-dark/image.png` sha256 `e43974c613e4…` 相同、发布帧 `proxies-dark.png` 与 light 宿主诊断帧逐字节相同。

## 12.73 Slint 侧栏标签截断成为共享 typed 事实（1条待验证）

| ID | 状态 / 优先级 | points | owner | 依赖 |
| --- | --- | --- | --- | --- |
| `FLPARITY-048` | `Integrated` / P2（b176 立项、b197 交付，待统一批次办结） | 1 | `crates/infiltrator-contract/src/sidebar_label.rs` + `crates/infiltrator-presentation/src/sidebar_label_projection.rs` + `crates/infiltrator-slint/src/lumen_projection.rs` + `ui/components/{models,sidebar}.slint` + FlClash 桌面参照基准 | `FLPARITY-011`（跨端 hover 基线） |

**`FLPARITY-048` 目的地行 tooltip 只在标签被截断时出现（b176 实测立项，b197 收口，1 pt / P2 / `Integrated`）**：
① **上游真判据**：pinned `be847366` 的 `test/widgets/sidebar_test.dart` 钉住 rail 每行有 tooltip、展开态短标签无 tooltip、展开态超长西里尔标签保留 tooltip；prism_ui `lib/sidebar.dart` 先算 `labelWidth = rowWidth - iconInset - iconSize - 2*labelGap`，再读 `TextPainter(maxLines: 1).layout(maxWidth: labelWidth)` 的 `didExceedMaxLines`。本仓 Slint 1.18 `Text` 不暴露 elide 读回，b176 因此按自底向上铁律停在上层之外，等底层文字宽度事实。
② **底层先裁决**：新增 `infiltrator_contract::sidebar_label` 的 `SidebarLabelGeometry`（240px 列 / 8px leading / 16px trailing / 24px 图标 / 12px gap / 14px label size / scale），以一处 `available_px()` 给出 180px，并用既有 `text_truncation::plan_truncation` 的 `TextMeasure` 规则回答 `is_truncated()`；非法宽度/NaN 诚实返回「未截断」。`infiltrator_presentation::sidebar_label_projection` 用 `infiltrator_shared::effect_assets::char_width_em` 实现度量桥，缩放同时作用于可用宽度与字号，避免两个真源。
③ **表面纯消费**：`NavigationSlot` 增 `label_truncated: bool`；`lumen_projection.rs` 按当前 `page_scale_percent` 与**实际绘制词**（四个主项走 `PageTitles.page-name` 的 msgid，其余用投影 label）计算它，并在 `project_snapshot` 里按快照语言重新投影导航，杜绝语言切换后标签与截断不同步。`sidebar.slint` 只按 `!labelled || label_truncated` 条件挂载原生 `Tooltip`；rail 每行都有完整词，展开态短标签没有，展开态截断标签保留。
④ **测试与像素**：contract 5 条、presentation 5 条、Slint lib 2 条新增单测覆盖短/超长中文与西里尔标签、缩放、非法盒、空串、主项显示词；三包 nextest **1372/1372**、Slint lib nextest **29/29**、定向 clippy `--lib -D warnings` 与 `cargo fmt --check` 全绿；`slint-syntax-guard` 100% PASS、`slint-dead-input` findings=0、`slint-rail-edge-dialect` 待统一批次复核。诊断批 `20261111T035742Z_d20b90df8159_dirty_1198518` 的 `overview-dark/image.png` 与已发布 `overview-dark.png` **sha256 同为 `b66c11ab17c2…`、全帧差异 0 px**：tooltip 只在 hover 出现，捕获台架按设计不投指针，故零重采、零像素变化、零 manifest 变化。
⑤ **诚实边界**：共享度量是随包字体的半/全/零宽字符近似，不是 shaping/kerning 级 glyph advance；`SidebarLabelGeometry::DESKTOP` 是桌面展开档的度量几何，ultra 280px 列与未来缩放由 `scale_percent` / 新几何实例扩展，不在 UI 层猜列宽；hover tooltip 本身不进像素回执；全仓 `--workspace` nextest/clippy、结构门全腿与 `resolve_surface_evidence.py` 仍待统一批次；另一会话未提交的 Bevy 像素仍是全仓身份门唯一红腿。

**待验证范围**：
- 单元（本机可复跑）：`cargo nextest run -p infiltrator-contract -p infiltrator-presentation -p infiltrator-slint`（1372/1372）、`cargo nextest run -p infiltrator-slint --lib`（29/29）、`cargo clippy -p infiltrator-contract -p infiltrator-presentation -p infiltrator-slint --lib -- -D warnings`、`cargo fmt` 三包 `--check`。
- 守卫（本机可复跑）：`slint-syntax-guard` 100% PASS、`slint-dead-input` 0、`slint-theme-channel-guard` 0/8、`doc-governance`、`closure-questions 8/8`、双表面 `space_utilization --enforce`、`receipt-freshness 0`。
- 像素（本机可复跑）：诊断批 `20261111T035742Z_d20b90df8159_dirty_1198518` 与发布 `overview-dark.png` 逐字节相同；hover tooltip 的交互证据仍需带指针台架，按既有诚实边界不冒充已验收。
- 统一批次：本文件 §0.1 的 b206 结果已更新；全仓测试/Clippy/fmt 通过，但结构门别名与跨表面产品回执仍未收口，本条保持 `Integrated`。

**证据身份**：contract/presentation/Slint 三包 1372/1372、Slint lib 29/29、诊断批 `20261111T035742Z_d20b90df8159_dirty_1198518`（`overview-dark` sha256 `b66c11ab17c2…` == 发布帧）。

## 12.74 Iced 连接与日志成为独立观察页（1条待验证）

| ID | 状态 / 优先级 | points | owner | 依赖 |
| --- | --- | --- | --- | --- |
| `SPACEUTIL-014` | `Integrated` / P1（b190 实测立项、b205 收口，待统一批次办结） | 2 | infiltrator-contract（`FeatureSpec::iced_page`）+ infiltrator-iced（`Route::{Connections,Logs}`、观察页、列表滚动 owner、捕获几何）+ 全量 Iced 像素矩阵与路由守卫 | `SPACEUTIL-013`（页身卡 owner）、`SPACEUTIL-012`（契约形态映射） |

**`SPACEUTIL-014` Iced 的 `Connections` / `Logs` 不再折在 runtime 页里（b190 立项、b205 收口，2 pt / P1 / `Integrated`）**：

① **架构缺口**：共享导航注册表已有 `Connections` / `Logs`，但 Iced 曾把两项都映射到 `Route::Runtime`，多个目的地因此共享选中态，业务捕获矩阵也把独立能力拍在 runtime 页。现由独立 typed route 承载，demo 路由、侧栏单选、运行态订阅与矩阵 `page` carrier 同批对齐；形态仍由共享 `FeatureSpec` 派生，契约阈值和 `page-structure.json` 不变。
② **连接页几何根因**：独立页接线后，紧凑搜索高亮帧仍无法发布——探针测整张连接行，行底部被内部列表滚动窗裁掉；分组列表也没有发布独立页滚动 ID。平铺与分组两种列表现在共用 `CONNECTIONS_PAGE_SCROLL_ID`，搜索高亮捕获量实际可见的文字锚点；窄档 headline 纵排，主机名单行，避免 chip 与操作控件把主机名挤成竖排碎字。
③ **日志页像素根因**：结果视口用森林皮肤近地色 `control_bg` 铺满页身，而实际页面地面采样色与该表面距离不足，被真实像素判定成内部死带；搜索帧实测标准 **62.0%**、紧凑 **26.0%**，超 `framed` 4%。移除这层近地表面并让滚动区透明并入页身卡，没放宽阈值、没记欠账。
④ **定向证据**：Iced nextest 阶段性 **638/638**；路由守卫 `iced-observation-routes-guard.py --self-test` **8/8**、enforce 0；完整矩阵最终为 **188/188**，binary SHA 和 run ID 在 2026-11-11 b205 capture manifest；发布矩阵 `space_utilization --surface iced --enforce` **0 违规**，`receipt-freshness-guard.py` **0 违规**。日志样式修复前诊断 run `20261111T094030Z_8e1c2a6d7c71_dirty_3157883` 的四帧复测已证明根因修复将两条超标收敛到 0；随后完整 188 帧按最终代码重采。
⑤ **待验证范围**：
- 单元：最终代码下重跑 `cargo nextest run -p infiltrator-iced --features capture-tools -j 2` 与 `cargo clippy -p infiltrator-iced --all-targets --features capture-tools -- -D warnings`。
- 守卫：`iced-observation-routes-guard.py` self-test/enforce、`iced-sidebar-nav-guard`、`iced-dead-input-guard`、矩阵 `generate_unified_matrix.py --check --surface=iced --surface=slint`、回执新鲜度。
- 像素：完整 Iced 188 帧发布批、`space_utilization --surface iced --enforce`；刷新 Iced about/settings、sync-restore 既有别名摘要。全仓身份门另有 Bevy 并行工作流像素差异，按文件所有权单独归属。
- 统一批次：本文件 §0.1 的 b206 结果已更新；全仓测试/Clippy/fmt 通过，但结构门别名与跨表面产品回执仍未收口，本条保持 `Integrated`。

**证据身份**：Iced nextest 638/638、路由守卫 self-test 8/8 与 enforce 0 findings、源码指纹回归单测 1/1、最终 188 帧发布 run `20261111T101129Z_8e1c2a6d7c71_dirty_3339388`（binary SHA-256 `59f611a2b60c27b96476e6f46e99ceff61891c48ad3804fe96edd636e5af7ea0`，source fingerprint `25d635ddc8fa5d3941d1ad5deb638f8f1583fc36a656893e212030997eb45a72`）、空间违规 0、回执新鲜度违规 0；搜索日志 near-ground 根因复测 run `20261111T094030Z_8e1c2a6d7c71_dirty_3157883`。

## 12.75 回执的页身原点与桌面标题栏挂载（1条待验证）

| ID | 状态 / 优先级 | points | owner | 依赖 |
| --- | --- | --- | --- | --- |
| `SPACEUTIL-015` | `Integrated` / P1（b207 实测立项并交付，待统一批次办结） | 3 | infiltrator-contract（`shell_receipt` 的 `shell_top_air_px` / `shell_bottom_air_px` 语义）+ infiltrator-slint（`desktop_window_chrome.rs`、`ui/lumen-app.slint`、`ui/components/app_chrome.slint`、`src/demo.rs`）+ `scripts/parity/space_utilization.py`（planted fixture 与像素腿）+ `scripts/quality/slint-viewport-tier-guard.py` | `SPACEUTIL-007`（回执建盒）、`SPACEUTIL-006`（chrome 与密度分离）、`FLPARITY-018`（CSD 标题栏） |

**`SPACEUTIL-015` 回执必须说「窗边到页身」，桌面标题栏不得由页面密度决定（b207 交付，3 pt / P1 / `Integrated`）**：

① **两条真实缺陷都由像素与回执同时证明**：(a) **坐标原点**——已发布帧 `overview-dark`（1180×780）的页身实际从 y=48 起画（40px 自绘标题栏 + 8u 页身空气），而 `CAPTURE_CHROME` 只发布 `shell_top_air_px=8`，判定器 `content_box()` 因此从 y=8 开始量，把 40 行标题栏像素当成页身内容/页尾空地记账；720×480 rail 帧更报 `shell_top_air_px=0`。根因是 Slint 把**页身组件内部的 padding** 当成**窗口坐标**发布。(b) **挂载条件**——`if !root.android-platform && !root.compact-layout: DesktopTitleBar` 让「页面密度」决定「窗口 chrome」，于是 frameless 桌面在 rail / 折叠态**整条标题栏连同最小化/最大化/关闭控件一起消失**（旧发布帧 `overview-compact-dark` 实测顶部无任何窗控，而 FlClash 真实场景图在收起侧栏时窗控仍在）。
② **自底向上收口，不在 UI 猜数**：契约 `shell_receipt::ShellChromeFacts` 把两个字段的语义钉成「窗口边缘到页身测量盒的距离（含祖先偏移：标题栏带 / safe area）+ 页身自己的空气」，不再只描述组件 padding；Slint 顶边发布 `AppBody.y + page-content-padding-top`、底边发布 `window.height - AppBody.y - AppBody.height + page-content-padding-bottom`，即**由画出这些像素的外壳实测**，判定器与任何 Python 常量都不再记标题栏高度（既有「判定器记标题栏高度即违规」腿继续为 0）。
③ **chrome 与密度彻底分离**：新增 `desktop_window_chrome::install()`（宿主唯一写者），一次读 `WindowChrome::FRAMELESS` 同时决定 `set_frameless` 与挂载带 `desktop-title-bar-units`；`.slint` 只消费该带（`if root.desktop-title-bar-units > 0`、页身 `y = safe-top + band`、`height -= band`），**表面不再持有 40px 字面量**；`src/demo.rs` 的第二次 `set_frameless(true)` 同批删除，不留两个窗饰写者。`compact-layout` 从此只回答页面密度。
④ **实测结果**：标准帧 `shell_top_air_px` 8 → **46**、rail 帧 0 → **38**；rail 帧导航栈底 398 → **436**（整条栈随带下移），带标签帧栈底 486 → **484**、折叠钮带 106 → **104**（与 38px 契约带自洽）。`overview-compact-dark` 真实像素复核：顶部出现 `Lumen` + 速率胶囊 + 搜索 + 三枚窗控，页身仍按紧凑密度排布。判定器 planted fixture 的「published shell really draws」注释此前仍写 8/0/106，本批把它改为实测的 46/38/104 与对应栈底，并让 `_icon_rail` 的种植像素随带下移——**修 fixture 而非放宽断言**。收口期实测仅改 fixture 仍不自证：把带高改回 8/0/106，种植像素与种植回执一起漂移，其余 62 项全绿。据此补第 63 项 planted `test_the_planted_origins_follow_the_contract_band`，从契约源码读条带并断言 rail 顶距==带高、带标签顶距>带高、种植导航墨迹不得落在带内；改回旧原点实测恰好该单项 FAIL（`0 != 38`），`--self-test` **63/63**。
⑤ **门禁跟上（对 HEAD 真实旧代码验咬）**：`slint-viewport-tier-guard.py` planted 自测 36 → **41/41**，新增「回执漏掉页身窗口原点（顶/底各一条）」「标题栏挂载不再读规划带」「带高被写成表面字面量」「捕获宿主自行装饰窗口」「宿主不再从契约规划 chrome」五腿；把 HEAD 的旧 `.slint` / `demo.rs` 喂给 enforce 腿实测报 **10 条** findings，改后 0 条。
⑥ **像素身份**：整批 199/199 重采发布 run `20261111T133556Z_14d5cbbf6ef1_dirty_153449`（418s），`space_utilization --surface slint --enforce` **violations=0**（rail 轴 `chrome_band` 0.0% / `void` 6.2%，限 8%），`receipt-freshness 0`；Slint 侧 5 条别名摘要按新字节刷新并逐条实测 摘要==实际字节==canonical，`screenshot-identity` 的 Slint 腿 0 违规（全仓仅剩另一会话的 Bevy `runtime-dark`/`overview-dark` 别名 1 条）。发布后改的仅是纯重构（把 chrome 宿主拆成 `desktop_window_chrome.rs`）与测试/判定器/守卫，两帧复采证明像素逐字节不变，故沿用该满批回执。
⑦ **定向证据（L）**：`cargo nextest run -p infiltrator-contract -p infiltrator-slint --locked` **982/982**（run `7123c0ed-f3dc-4db2-ba50-01b62d48ad08`，含新用例「planned band == 契约 chrome 高度」）；`cargo clippy -p infiltrator-contract -p infiltrator-slint --all-targets -- -D warnings` 与 `cargo check -p infiltrator-slint --features gpu-skia,capture-tools` exit 0；`cargo fmt --all --check` 0 偏差；`slint-viewport-tier-guard` 41/41 + enforce 0、`slint-syntax-guard` 100% PASS、`slint-dead-input` findings=0 / allowed=8、`slint-mobile-chrome` 0、`line-guard` 0（`lib.rs` 937 行曾超 800 预算，按业务缝拆出 chrome 宿主后归零）、`import-guard` 0、`typed-state` 0、`implicit-state` 23/0/0、`doc-governance OK`、`closure-questions 8/8`、`space_utilization --self-test` 63/63。
⑧ **待验证范围**：
- 单元：统一批次复跑 `cargo nextest run --workspace`（含 contract `the_frameless_style_owns_a_real_drag_strip` 新增的 38px 钉值断言）。
- 守卫：`slint-viewport-tier-guard` 自测/enforce、`screenshot-identity`、`receipt-freshness`、`space_utilization --enforce`（两表面）与全仓结构门。
- 像素：Slint 199 帧已发布；Iced 侧回执口径未随本批改写（其 `shell_top_air_px` 由 `chrome_strip` 挂载高度给出，本已含条带），统一批次需复跑 `--surface iced --enforce` 确认无回归。
- 设备：Android 三 flavor 未取本批实机回执——紧凑/触摸外壳不挂桌面标题栏，`desktop-title-bar-units` 恒 0，故 Android 像素不受本批影响，但仍按截图认证硬门要求补证。
- 统一批次：本文件 §0.1 的 b206 结果不适用本批；全仓 `--workspace` nextest/clippy、结构门全腿与 `resolve_surface_evidence.py` 仍待统一批次，Bevy 并行像素差异按文件所有权单独归属。

**证据身份**：contract+Slint nextest **982/982**（run `7123c0ed-f3dc-4db2-ba50-01b62d48ad08`）、`slint-viewport-tier-guard --self-test` **41/41** 且 enforce findings=0、HEAD 旧源码喂入 enforce 腿实测 **10 findings**、发布批 `20261111T133556Z_14d5cbbf6ef1_dirty_153449`（199/199，418s）`--surface slint --enforce` violations=0 / `receipt-freshness` 0、`space_utilization --self-test` 63/63、诊断复采 run `20261111T143739Z_14d5cbbf6ef1_dirty_489754` 两帧与发布帧 sha256 逐字节相同（`f3e687a9…` / `f894f449…`）。

## 12.76 Profiles 页面按上游 300px 规则重排（1条待验证）

| ID | 状态 / 优先级 | points | owner | 依赖 |
| --- | --- | --- | --- | --- |
| `FLPROFILES-001` | `Integrated` / P1（b208 实测立项并交付，待统一批次办结） | 2 | infiltrator-contract（`responsive_viewport::profile_columns`、`page_structure` v12 导出 `profile_grid`）+ `scripts/generate_viewport_ladder.py` + infiltrator-slint（`ui/pages/page_profiles.slint`）+ `scripts/quality/{slint-viewport-tier-guard,sidebar-expansion-owner-guard}.py` | `FLPARITY-003`（仪表盘卡片密度）、`SPACEUTIL-001`（空间利用率标准） |

**`FLPROFILES-001` Profiles 页面按 FlClash 300px 规则重排（b208 交付，2 pt / P1 / `Integrated`）**：

① **上游源码实测**：pinned FlClash `lib/common/layout.dart` 定义 `profileItemMinWidth = 300.0`、`_profilesMaxColumns = 3`；`getProfilesColumns(viewWidth, spacing)` 用 `(viewWidth + spacing) / (300 + spacing)` 取整并夹到 1..3。本仓 Slint Profiles 页 `for profile: ProfileCardSurface` 是固定单列，1180 页身 940px 只画一列卡（旧帧每卡 ~880px），违背同一宽度下参照物的 1–3 列规则。
② **自底向上收口**：契约 `responsive_viewport` 新增 `PROFILE_MIN_ITEM_WIDTH_PX=300` / `PROFILE_MAX_COLUMNS=3` 与 `profile_columns()`；`page_structure` 导出升 **v12** 并新增 `profile_grid`；生成器 `generate_viewport_ladder.py` 升 v12 并新增 `ViewportLadder.profile_columns(page-width, gap)`；页面 `page_profiles.slint` 改为 `GridLayout`，消费生成 lookup，紧凑档仍单列。
③ **像素与门**：整批 199/199 重采发布 run `20261111T163713Z_75e05722ce81_dirty_999303`（394s），`space_utilization --surface slint --enforce` violations=0、`receipt-freshness 0`；实测 1180 页身 940px 为 2 列（每卡 ~420px，超 300px 底线），紧凑 720 档仍 1 列。contract nextest **594/594**、Slint nextest **389/389**、contract/Slint clippy `--all-targets -D warnings`、`cargo fmt --all --check` 全绿；`slint-viewport-tier-guard` 41/41 + enforce 0、`sidebar-expansion-owner-guard` 9/9 + enforce 0、`slint-dashboard-grid-guard` 0、`slint-syntax-guard` 100% PASS。
④ **待验证范围**：统一批次复跑 `cargo nextest run --workspace` 与全仓结构门；`resolve_surface_evidence.py` 与跨表面 parity 收口仍待产品级回执；另一会话未提交的 Bevy `runtime-dark` / `overview-dark` 像素别名仍是全仓身份门唯一红腿，本条不负责该红腿。
⑤ **证据身份**：contract nextest **594/594**、Slint nextest **389/389**、发布批 `20261111T163713Z_75e05722ce81_dirty_999303`（199/199，394s）、`space_utilization --surface slint --enforce` violations=0、`receipt-freshness` 0、`slint-viewport-tier-guard` 41/41、`sidebar-expansion-owner-guard` 9/9、`slint-dashboard-grid-guard` 0、`slint-syntax-guard` 100% PASS。

## 12.77 Iced 侧栏宽度只读契约（1条待验证）

| ID | 状态 / 优先级 | points | owner | 依赖 |
| --- | --- | --- | --- | --- |
| `ICEDRAIL-001` | `Integrated` / P2（b209 实测立项并交付，待统一批次办结） | 1 | infiltrator-iced（`view/sidebar.rs`、`tests/gui/{responsive_elasticity_tests,view_sidebar_tests}.rs`）+ `scripts/quality/iced-sidebar-nav-guard.py` | `SPACEUTIL-011`（Iced 侧栏与 Slint 同一条律法） |

**`ICEDRAIL-001` Iced 侧栏宽度只读契约（b209 交付，1 pt / P2 / `Integrated`）**：

① **缺口**：`view/sidebar.rs` 自持 `SIDEBAR_WIDTH=240.0` / `RAIL_WIDTH=64.0`，注释自称 mirror `SidebarForm::width_px()`，同一几何两个真源。
② **收口**：删除字面量；标签列宽度直接 `form.width_px().expect(...)`，rail 宽度 `SidebarForm::Rail.width_px().expect(...)`；测试改钉契约值（64/240/280）。
③ **门禁**：`iced-sidebar-nav-guard.py` 新增第 8 项 planted（宽度字面量回仓即咬），self-test **8/8** + enforce 0；Iced nextest **547/547**、clippy `--all-targets -D warnings`、`cargo fmt --all --check` 全绿；`space_utilization --surface iced --enforce` 188 帧 violations=0、`receipt-freshness 0`（零像素变化，无需重采）。
④ **待验证范围**：统一批次复跑 `cargo nextest run --workspace` 与全仓结构门；另一会话未提交的 Bevy `runtime-dark` / `overview-dark` 像素别名仍是全仓身份门唯一红腿，本条不负责该红腿。
⑤ **证据身份**：Iced nextest **547/547**、guard self-test **8/8** + enforce 0、Iced 188 帧 `--enforce` violations=0、`receipt-freshness` 0。

## 12.78 Iced 流量统计成为独立目的地（1条待验证）

| ID | 状态 / 优先级 | points | owner | 依赖 |
| --- | --- | --- | --- | --- |
| `ICEDTRAFFIC-001` | `Integrated` / P1（b210 立项、b211 收口，待统一批次办结） | 3 | infiltrator-contract（`shell_navigation`、`page_structure`、v13 导出）+ infiltrator-iced（`Route::Traffic`、侧栏、观察页、捕获几何）+ Iced 矩阵与守卫 | `SPACEUTIL-014`（独立页先例）、`TRAFFICAUDIT-001`（审计读回已交付） |

**`ICEDTRAFFIC-001` Iced 的流量统计成为独立目的地（b211 交付，3 pt / P1 / `Integrated`）**：

① **架构缺口**：共享注册表已有 surface-local `traffic` 行，Iced 却把它折到 `Route::Runtime`，而契约 `destination_for_route` 又把 `Runtime` 自身折成 `traffic`；设置页 Runtime 与流量页因此共用选中态。旧 `runtime-traffic-audit-*` 两帧 `CAPTURE_READY page=runtime` 借 `content` 82% 上限判定，独立流量页从未按自己的形态测量。pinned ClashParty `72af7d8` 的 `/traffic` 是独立路由。
② **自底向上收口**：契约把 `Runtime` 归 Settings、`Traffic` / `RuntimeWorkbench` 归 `SurfaceLocal("traffic")`；`page_structure` 以 typed `PageShapeOwner` 表达「共享页 / surface-local 目的地」，v13 导出 `traffic` slug 为 `framed` 4%，生成器与两个版本守卫共用 `scripts/parity/page_structure_version.py` 一处版本 pin。Iced 新增 `Route::Traffic` 与只读共享契约的 `navigation_target()`，侧栏单选、demo 路由、观察页 body / 滚动 ID、捕获几何与路由守卫同批迁移；`capture-iced-matrix.sh` 的 `runtime-traffic`→`runtime` 别名删除，旧两帧退役，新增页顶与审计卡各一对标准/紧凑帧，矩阵 188 → **190 帧**。
③ **定向证据**：`--locked` contract+shared+Iced nextest **1313/1313**、定向 clippy `--all-targets -D warnings`、`cargo fmt --all --check`；`iced-observation-routes-guard` **13/13**、`iced-sidebar-nav-guard` **8/8**、`sidebar-expansion-owner-guard` **10/10**、`slint-viewport-tier-guard` **41/41**、`flclash-sidebar-void-guard` **7/7**、`space_utilization --self-test` **63/63** + `--check-export` current + Iced **190** / Slint **199** 帧 `--enforce` violations=0；`receipt-freshness` 0、Iced 5 条别名摘要刷新、`generate_unified_matrix --check` 双表面一致、`doc-governance` / `line` / `import` / `typed-state` / `implicit-state` / `closure-questions` 全 0。
④ **待验证范围**：统一批次复跑 `cargo nextest run --workspace`、全仓 Clippy / 结构门全腿与 `resolve_surface_evidence.py`；另一会话未提交的 Bevy `runtime-dark` / `overview-dark` 像素别名仍是全仓身份门唯一红腿，本条不负责该红腿。Bevy 继续把流量并入概览，台架不投指针，选区证据来自同一条生产消息路径。
⑤ **证据身份**：Iced 190/190 帧发布 run `20261111T192135Z_a866e860b2d7_dirty_1788701`（binary SHA-256 `0b9d58144cab3302a8efe139c6845be73c739e4460f3032aa03823ce694eace8`）、Iced nextest 1313/1313（三包合并）、路由守卫 13/13 + enforce 0、空间门 0 违规、回执新鲜度 0。

## 12.79 Slint 代理节点网格按页宽连续裁决（1条待验证）

| ID | 状态 / 优先级 | points | owner | 依赖 |
| --- | --- | --- | --- | --- |
| `FLPROXY-001` | `Integrated` / P1（b212 实测立项并交付，待统一批次办结） | 2 | infiltrator-contract（`responsive_viewport::proxy_node_columns` / `ProxyGridForm` / v14 导出）+ `scripts/generate_viewport_ladder.py` + `crates/infiltrator-slint/ui/pages/page_proxies.slint` + Slint 199 帧回执与守卫 | `FLPROFILES-001`（页宽规则先例）、`SPACEUTIL-005`（导出阶梯单一真源） |

**`FLPROXY-001` 代理节点网格按页宽连续裁决（b212 交付，2 pt / P1 / `Integrated`）**：

① **真实差距**：pinned FlClash `be847366` 的 `getProxiesColumns` 以 `max(ceil(viewWidth/250),2)` 给列数，由 `tab.dart` 传入 `max(width-32,0)` 的页身可用宽，再由 `tight/standard/loose` 偏好 ±1；真实代理页场景图实测 5 列、每卡 371px ≈ 248dp。本仓 `ViewportTier::proxy_grid_columns` 按窗口档固定 1/2/3/4；1180×780 帧页身 910px 只画 3 列、每卡约 282px，而 `proxies-dark` 空间门 `unused 51.3%` 虽在 `content` 82% 上限内，横向密度仍低于同一宽度规则。
② **自底向上收口**：契约新增 typed `ProxyGridForm::{Stacked, Desktop}`、250/32/两列地板常量与 `proxy_node_columns(form, page-width)`；v14 导出 `proxy_grid` 词表，生成器投影为 `ViewportLadder.proxy_columns(form, page-width)`，旧无消费者 tier 查表同批退役。Slint 页面只按 `LumenTheme.labelled-sidebar` 选 typed 形态并传页身宽；`slint-viewport-tier-guard.py` 新增 3 条 planted（生成函数无人读、私写 250/32、重命名词表），自测 **43/43** + enforce 0。存异：不引入上游三档用户偏好；手机/底栏档保持单列可读，Iced/Bevy 仍消费 tier 查表，跨表面接线按文件所有权后续收口。
③ **像素与门**：诊断批 `20261111T205309Z_58a80f5286b1_dirty_2271974` 四帧复核标准 1180 档 4 列无文字裁切、720/400 档仍单列；整批 **199/199** 重采发布 run `20261111T205728Z_58a80f5286b1_dirty_2295244`（369s），仅 4 张 `proxies-{dark,light,forest,amoled}` 帧与 manifest 变化。`space_utilization --surface slint --enforce` 199 帧 **violations=0**、`receipt-freshness 0`、统一矩阵双表面一致、`screenshot-identity` 唯一红腿仍是另一会话未提交的 Bevy 像素别名。`--check-export` current、生成器 `--check` fresh、契约+Slint nextest **984/984**、定向 Clippy `--all-targets -D warnings`、`cargo fmt --all --check`、Slint 语法门 100%、`doc-governance` / `line` / `import` / `typed-state` / `implicit-state` 全 0。
④ **待验证范围**：统一批次复跑 `cargo nextest run --workspace`、全仓结构门与 `resolve_surface_evidence.py`；另一会话未提交的 Bevy `runtime-dark` / `overview-dark` 像素别名仍是全仓身份门唯一红腿，本条不负责该红腿。Iced / Bevy 的同一页宽规则接线仍待其文件所有权落地后由跨表面切片收口。
⑤ **证据身份**：Slint 199/199 发布 run `20261111T205728Z_58a80f5286b1_dirty_2295244`、诊断 run `20261111T205309Z_58a80f5286b1_dirty_2271974`、契约+Slint nextest **984/984**、`slint-viewport-tier-guard` **43/43** + enforce 0、空间门 0 违规、回执新鲜度 0。

## 12.80 Iced 双网格消费共享页身宽（1条待验证）

| ID | 状态 / 优先级 | points | owner | 依赖 |
| --- | --- | --- | --- | --- |
| `ICEDGRID-001` | `Integrated` / P1（b213 实测立项并交付，待统一批次办结） | 2 | infiltrator-contract（`page_body_width_px` / `ProfileGridForm` / v15 导出）+ infiltrator-iced（profiles / proxies 视图、CaptureSection、diagnostic capture 隔离）+ Slint 生成物与双表面回执 | `FLPROFILES-001` / `FLPROXY-001`（两条页宽规则先例）、`SPACEUTIL-005`（导出阶梯单一真源） |

**`ICEDGRID-001` Iced 双网格消费共享页身宽（b213 交付，2 pt / P1 / `Integrated`）**：

① **架构缺口**：b208 / b212 的 Profiles 与代理节点规则都要求「页身可用宽」，本仓却只有窗口宽、导航列宽、页边距三个分散事实；Iced 仍把配置卡叠成单列、代理节点按窗口档固定 3 列。另实测诊断批 `INFILTRATOR_CAPTURE_SCENARIOS` 会逐帧覆盖正式 PNG 并把 190 行 manifest 截成 4 行。
② **自底向上收口**：契约新增 `ViewportTier::page_body_width_px(window_width)`，一处扣除导航列与双侧页边距；v15 导出 `profile_grid.forms` / `proxy_grid.forms`，新增 typed `ProfileGridForm::{Stacked, Desktop}`，生成器投影到 Slint，Iced 与 Slint 都按 `labelled_sidebar` 选形态。720×480 诊断证明 profiles 数值上能放 2 列却会把文本挤成竖排碎字，故轨/底栏档固定单列。Iced 新增 `CaptureSection::ProfilesGrid` 与 `profiles-grid-{standard,compact}` 两帧，把配置列表滚入真实视口；标准帧 2 列且卡高按内容收缩。捕获脚本引入 `PUBLISH_CORPUS=0`：过滤批与矩阵覆盖批只写 `.evidence`，完整 canonical 矩阵才发布图像与索引；`iced-observation-routes-guard` 加 planted 自测钉住这条隔离。
③ **证据（L）**：契约+Slint+Iced nextest **1626/1626**、三包 Clippy `--all-targets -D warnings`、fmt；Iced **192/192**、Slint **199/199** 整批重采，`space_utilization --surface iced|slint --enforce` 均 0 违规、`receipt-freshness 0`、`--check-export` current、生成器 `--check` fresh、统一矩阵双表面一致；`slint-viewport-tier-guard` **45/45**、`iced-observation-routes-guard` **14/14**、`sidebar-expansion-owner-guard` **10/10**，doc/line/import/typed-state/implicit-state 全 0。
④ **待验证范围**：统一批次复跑 `cargo nextest run --workspace`、全仓结构门与 `resolve_surface_evidence.py`；另一会话未提交的 Bevy `runtime-dark` / `overview-dark` 像素别名仍是全仓身份门唯一红腿。Bevy 的 Profiles / 代理网格仍消费 tier 查表，由 `FLPROFILES-002` 在其文件所有权落地后收口。
⑤ **证据身份**：Iced 192/192 发布 run `20261111T220532Z_923ba4af7b2f_dirty_2656390`、Slint 199/199 发布 run `20261111T221050Z_923ba4af7b2f_dirty_2705566`、三包 nextest **1626/1626**、两个网格诊断批 `20261111T220101Z_923ba4af7b2f_dirty_2639332` / `20261111T220246Z_923ba4af7b2f_dirty_2649539`。

## 12.81 已关闭连接的 Core 单一 owner 与权威读回（1条待验证）

| ID | 状态 / 优先级 | points | owner | 依赖 |
| --- | --- | --- | --- | --- |
| `CPPARITY-CLOSED-CORE` | `Integrated` / P1（b214 实测立项并交付，待统一批次办结） | 3 | infiltrator-domain（typed 观察流）+ infiltrator-application（单例 owner、命令读回、Core 装配）+ infiltrator-composition（SurfaceReader 唯一观察入口与 snapshot 发布）+ 桌面/Android/iOS 命令组合 | `TRAFFICAUDIT-001`（typed 读回先例） |

**`CPPARITY-CLOSED-CORE` 已关闭连接不再是空 owner（b214 交付，3 pt / P1 / `Integrated`）**：

① **架构缺口**：契约已有 `ClosedConnectionsSnapshot`，但 `surface_reader.rs` 固定发布空值，clear dispatch 固定返回 `Ok(())`；若把历史挂在每次构造的 `ConnectionApplication` 上，admin、monitoring 与 surface reader 会各持一份影子历史。
② **自底向上收口**：domain 新增 `ClosedConnectionHistory`，以 `NoBaseline / Tracking { scope, revision }` typed 两态表达观察流，首帧只建基线、同一 scope 的后续成员差集留存最后事实、旧 revision 拒绝回滚、换 scope 重置、容量淘汰保留 lifetime total；`ClosedConnectionsApplication` 与 `TrafficWaveformApplication` 同构，由 `CoreApplication` 单例持有，桌面 / Android / iOS 只拿 clone；SurfaceReader 是唯一观察入口，成功观测后喂 owner 并发布同一份 `closed_connections` 读回；`ClearClosedConnections` 走 `LocalOnly` 并校验清空后的权威读回。
③ **定向证据（L）**：`cargo nextest run --locked -p infiltrator-domain -p infiltrator-application -p infiltrator-composition` **1385/1385**；六包 `cargo clippy --all-targets -- -D warnings`、`cargo fmt --all --check`；`line` / `import` / `typed-state` / `implicit-state` / `closure-questions` 全 0；组合回归以 fake port 给出两个 scoped 观测，证明第一次读为空、第二次读发布 `total_closed=1` 与 `c-1`，clear 后下一帧为空。
④ **待验证范围**：统一批次复跑 `cargo nextest run --locked --workspace`、全仓结构门与 `resolve_surface_evidence.py`；`CPPARITY-CLOSED-UI`（typed 页签、演示数据、双视口真实像素回执和截图认证）仍在现在佛，本条不声称 UI 已接入；另一会话的 Bevy 文件按所有权纪律未触碰。
⑤ **证据身份**：三包 nextest **1385/1385**、六包 Clippy 0 warning、`line budget 2968 files / 0`、import `2968 / 0`、`typed-state 0`、`implicit-state 23/0/0`、`closure-questions 8/8`。
