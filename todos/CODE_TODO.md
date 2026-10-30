# MusicFrog 工程 TODO · 现在佛

> 层级：L4

## 0. 编写规则

- 唯一用途：登记全部**可推**未办结工作（待编码、待实现、待补证），覆盖 0.60 → 1.0 收口路径：crates.io 发布流水线、0.6x 生态与文档、0.7x 质量红线与安装包、0.9x 视觉与性能，以及五大平台 × 三大 UI 表现架构的 360° 覆盖与逐业务截图认证。已实现待验证的条目在[过去佛](CODE_TODO_PAST.md)；只剩外部环境可解锁的条目在[斗战胜佛](CODE_TODO_BLOCKED.md)。
- 每条仅保留唯一 owner、产物、依赖、验收、计划与证据要求；header ≤240 字符、详情 ≤6 行、整块 ≤1200 字符。证据留 nextest run ID / guard 回执；事实变化改写原条，不追加过程叙事。
- 状态词汇：`Planned / Blocked / Ready / InProgress`（本板）与 `Integrated`（过去佛，已交付待验证）。`Verified` 只是办结过渡态，不驻留任何看板。
- **执行标准（2026-10-20 起强制）：** 底层结构不够就扩 owner 结构，禁止再塞 `bool`/`Option`；组合优于继承；field 归 field、表现归表现、事务归 owner；每个事实只有一个真源，不留第二份缓存；非身份改动不碰 wire/快照/语料，身份改动集中一次做完。
- **可推性准入（2026-10-19 起强制）：**本板与过去佛只放「下一个可执行动作能在当前环境完成」的条目；剩余闭环动作全部依赖外部环境（ARM64 真机、受控 tag CI、用户 secrets/keystore、crates.io 上架凭证、异构目标机与被禁平台 target）的条目整条迁入[斗战胜佛](CODE_TODO_BLOCKED.md)，并写明阻塞方与解锁条件。
- **截图认证硬门：**每条业务能力在每个桌面 UI 必须同时具备 contract 锚点、scenario 锚点与 standard/compact 双视口真实像素回执；Android 三 flavor 另需实际交互截图回执。缺任一项只能写「部分」，不得称已验收。判定入口唯一：`scripts/parity/resolve_surface_evidence.py`（锚点与单元格）与 `scripts/parity/visual_receipts.py`（像素回执）。
- Rust 测试唯一执行器 `cargo nextest`（统一收口带 `--workspace --no-fail-fast`）；Clippy 用 `--all-targets -D warnings`；禁止裸 `cargo test`。多阶段操作必须 typed enum/sealed，禁止 bool/Option 组合状态机（总纲强制条款）。
- 迁移看板必须同次移出原文件、移入目标文件，保持稳定 task ID、owner、依赖、验收与 points。
- **并行工流与验证纪律（2026-10-20 起强制）：** 并行开发按文件所有权切分，编译不通过的半成品不得放进会被 `cargo nextest --workspace`、结构门或 capture 构建扫到的路径（WIP 先放草稿或 `.evidence/wip/`）；开发期只跑受影响包，全仓 `cargo nextest --workspace` 与全量结构门统一放到收尾批次只跑一次；共享 `/tmp` 配额、cargo lane 争用与合成器起栈失败等外部故障先判定归属，只修本切片能修的那一类。
- points 合同：`1` 局部证据/文档/配置；`2` 单 owner focused 包；`3` 跨 owner 纵切或普通机制包；`5` 产品采用/终局门。超过 5pt 先拆。priority 与看板位置不改变 points。
- 严格交付率只计 archived points；执行态报告对 Planned/Blocked/Ready 计 0%、InProgress 计 50%、Integrated 计 75%，不以执行态进度代替验收办结。
- 等价映射（避免重复条目）：`BANDROID-001.2 ≡ 040.5`；`BANDROID-004.2 ≡ 040.3`；`BANDROID-036.1` 的语义回归与 `042.3/042.4` 同批验证。
- **自底向上逐层裁决铁律（底层先决，严禁上层修补）：**若遇架构未决（领域模型模糊、所有权未定、状态二义性、缺少权威读回或跨端契约不一致），必须先在 Contract/Domain/Core 完成裁决与真实实现，再逐步上推到 Presentation 与 UI 表面。严禁在 UI 层（Iced/Bevy/Slint/Android）增加临时布尔/状态标志位、容错垫片、自行推导业务事实或写空实现/TODO 注释就地修补；违者一律打回重做。
- **Core-first 阶段门：**先完成所有可独立推进的底层 Core、双后端 conformance、Android 唯一 owner/IPC/VPNService 工作。若只剩现有 UI 消费者的旧调用阻塞 Core 契约退役，允许把 typed API 边界迁移作为对应 Core 切片一次完成；禁止同时改页面布局、视觉或新增 UI 功能，迁移完再集中构建验收。
- **不可解依赖不阻塞：**仅受缺失物理设备、受控 CI tag、用户 secrets/keystore 等外部不可解依赖约束的条目整条在[斗战胜佛](CODE_TODO_BLOCKED.md)；它们不阻塞其它条目选取、统一验证批次执行与已达标条目办结登记。

<!-- POINT_LEDGER:BEGIN -->
- Point 基线 `musicfrog-todo-20261031-v350`：现在佛 75 pt（17 条）/ 过去佛 413 pt（139 条）/ 斗战胜佛 51 pt（17 条）/ 未来佛 26 pt（15 条），联合未办结分母 565 pt（188 条）；已归档交付 748 pt（301 条）；累计 1311 pt，严格归档完成率 57.1%。
- 本板 17 条 = 像素认证收口 2 条（`PARITY-001.5` 5 pt、`PARITY-001.6` 5 pt）+ 三端视觉一致性 2 条（`GAP-7.3`、`GAP-7.6`，8 pt）+ FlClash v0.9.2 对标 12 条（`FLPARITY-002`、`005`～`007`、`010`、`011`、`014`、`017`、`018`、`036`、`039`、`046`，54 pt）+ 工程约束门禁 1 条（`ENG-002.2`，3 pt）。
- 2026-10-31 第七十七交付批次：`FLPARITY-040`（5 pt）大型配置文档编辑在页面与弹层中的响应性能契约完成（`infiltrator-contract::editor_viewport` 固化 `DocumentPerformanceBudget` 4k/50k 性能预算、`DocumentPerformanceStatus`、`DocumentPerformanceReceipt` 与 `WindowSearchResult`；`EditorViewport` 扩展 `slice_visible_lines`、`search_in_window` 与 `measure_performance`；`infiltrator-presentation::profile_preview` 实现 64 KiB 快速通道与 `redact_profile_preview_bounded`；Slint `lumen_projection` 接入 1000 行有界预览消除 ScrollView 50k 行冻结；`profile_store_fixtures` 扩充 50k 规则基准；测试 1226/1226 全绿，Clippy 零警告，质量门全绿），现移入过去佛 §12.58；现在佛 80 → 75 pt（18 → 17 条），过去佛 408 → 413 pt（138 → 139 条），联合分母 565 pt 保持不变。
- 2026-10-31 第七十六交付批次：`ENG-001.2`（1 pt）工具链单一版本守卫通过远端 `main-gate` 运行 41093052155 真实取证办结；过去佛 409 → 408 pt（139 → 138 条），联合分母 566 → 565 pt（189 → 188 条），archived 747 → 748 pt（300 → 301 条），严格归档率 57.0% → 57.1%。
- 2026-10-31 第七十五交付批次（统一验证收口）：ENG-003.2 与 ENG-003.3 办结——全量结构门 `check-structure.sh` exit 0（import 卫生 2863 文件 0 违规，特性矩阵 11 crates/34 features 全解析，结构门修复了 `infiltrator-shared/src/assets.rs` 一处内联长路径）；cargo 四项专责批次 `cargo fmt --all --check` 0 偏差、`cargo clippy --workspace --all-targets -- -D warnings` exit 0、`cargo nextest run --workspace --no-fail-fast` 5680/5680 passed（15 skipped）、`cargo deny check advisories bans licenses sources` 四项 ok；`cargo metadata --locked` exit 0；统一验证批次同步销账过去佛 7 条（`ENG-001.1`、`001.4`、`001.6`、`001.7`、`001.8`、`001.9`、`002.1`，16 pt）并一体提交推送 `main`。现在佛 82 → 80 pt（20 → 18 条），过去佛 425 → 409 pt（146 → 139 条），联合分母 584 → 566 pt（198 → 189 条），archived 729 → 747 pt（291 → 300 条），严格归档率 55.6% → 57.0%。
- 2026-10-31 第七十四交付批次：`FLPARITY-041`（2 pt）产品、桌面托盘与 Android 图标资产一致性审计完成（`product-identity.properties` 固化 `product.icon_variant_px` 16/24/32/48/256 共享 px 阶梯与 `product.android_icon_density` mdpi~xxxhdpi 五档密度桶为唯一真源；新增 `scripts/product_icon_variants.py` 从单一 canonical `icon.png` 生成全部变体并把 `icon.ico` 重写为五帧容器，逐变体 WCAG 对比度审计钉住浅/深/强调背景 ≥3:1、不透明覆盖率 ≥0.85；asset-catalog 登记 10 条派生变体；`infiltrator-shared` 新增 catalog/尺寸/ICO 帧注册表校验测试；Gradle `GenerateProductIcon` 改为从注册变体拷贝 `drawable-mdpi`~`drawable-xxxhdpi` 密度桶并删除死代码 `productIcons` 映射；`android-product-identity-guard` 扩展变体结构、ICO 帧镜像、catalog 覆盖与 APK badging 密度尺寸校验，自测 32/32 全绿；三 flavor `generate*ProductIcon` 任务实测生成五档密度资源全绿；守卫 0 违规，`infiltrator-shared` nextest 77/77 全绿），现移入过去佛 §12.57；现在佛 84 → 82 pt（21 → 20 条），过去佛 423 → 425 pt（145 → 146 条），联合分母 584 pt 保持不变。
- 2026-10-31 第七十三交付批次：`FLPARITY-038`（5 pt）可选通用覆写及更早应用时机完成（contract 固化 `GeneralOverridePhase`（PreEngineLaunch/PostConfigLoad）、`GeneralOverrideBackendCompatibility`（Universal/MihomoOnly/SingBoxOnly/Incompatible）、`GeneralOverrideItem`、typed 增删改查命令与 `GeneralOverrideMutationReceipt`、`GeneralOverrideApplyReceipt` 与 `GeneralOverrideSnapshot`；ports 导出 `GeneralOverrideStorePort` 抽象；application 实现 `GeneralOverrideApplication`，支持重名检测、版本围栏、排序重排、按阶段与启用状态过滤、后端引擎兼容性判定与 YAML/JSON 内容解析验证；presentation 提供 `GeneralOverrideRowProjection`、`GeneralOverridePageProjection` 与 `project_general_overrides` 单源驱动多端展示；composition 接入 `with_general_override` 与 `SurfaceSnapshot` 读取；targeted nextest 1729/1729 全绿，Clippy `-D warnings` 全绿，所有守卫通过），现移入过去佛 §12.56；现在佛 89 → 84 pt（22 → 21 条），过去佛 418 → 423 pt（144 → 145 条），联合分母 584 pt 保持不变。
- 2026-10-30 第七十二交付批次：`FLPARITY-037`（3 pt）将代理节点加入用户拥有的本地代理配置完成（contract 固化 `LocalProxyProtocol`、`LocalProxyNode`、`LocalProxyNodeSummary` 脱敏快照、`AddLocalProxyCommand`、`RemoveLocalProxyCommand`、`AddLocalProxyReceipt`、`RemoveLocalProxyReceipt` 与 `LocalProxySnapshot`；ports 导出 `LocalProxyStorePort` 抽象；application 实现 `LocalProxyApplication`，支持协议参数校验、防重复录入、版本围栏控制与安全脱敏；presentation 提供 `LocalProxyRowProjection`、`LocalProxyPageProjection` 与 `project_local_proxies` 单源驱动多端展示；composition 接入 `with_local_proxy` 与 `SurfaceSnapshot` 读取；targeted nextest 1516/1516 全绿，Clippy `-D warnings` 全绿，所有守卫通过），现移入过去佛 §12.55；现在佛 92 → 89 pt（23 → 22 条），过去佛 415 → 418 pt（143 → 144 条），联合分母 584 pt 保持不变。
- 2026-10-30 第七十一交付批次：`FLPARITY-013`（5 pt）加密备份真实导出、校验与恢复完成（contract 固化 `BackupDeliveryTarget`、`BackupRestoreScope`、`BackupVerificationFailure`、`BackupOperationReceipt` 与 `BackupRestoreSnapshot`；ports 导出 `BackupRestorePort` 抽象；application 实现 `BackupRestoreApplication` 口令校验、主版本兼容性判定、SHA-256 完整性验证与 AES-256-GCM 原子恢复回滚保护；presentation 提供 `BackupRestoreProjection` 与 `project_backup_restore` 统一驱动多端展示；Iced 清除假实现接入真实导出校验流程；composition 接入 `with_backup_restore`；targeted nextest 1510/1510 全绿，Clippy `-D warnings` 全绿，所有守卫通过），现移入过去佛 §12.54；现在佛 97 → 92 pt（24 → 23 条），过去佛 410 → 415 pt（142 → 143 条），联合分母 584 pt 保持不变。
- 2026-10-30 第七十交付批次：`FLPARITY-003`（5 pt）实时请求历史页完成（contract 固化 `RequestTrackingState`、`RequestStatus`、`RequestNetwork`、`RequestStatusFilter`、`RequestRecord` 与 `RequestTrackerSnapshot`；ports 导出 `RequestTrackerPort` 抽象；application 实现 `RequestTrackerApplication` 1000 容量环形缓冲、活跃连接计数、`record_closed` 迁移与暂停/恢复；presentation 提供 `RequestRowProjection` 与 `project_request_page` 驱动多端请求历史列表与详情；composition 接入 `with_requests`；targeted nextest 1503/1503 全绿，Clippy `-D warnings` 全绿，所有守卫通过），现移入过去佛 §12.53；现在佛 102 → 97 pt（25 → 24 条），过去佛 405 → 410 pt（141 → 142 条），联合分母 584 pt 保持不变。
- 2026-10-30 第六十九交付批次：`FLPARITY-004`（5 pt）DNS 查询历史页完成（contract 固化 `DnsHistoryRecordingState`、`DnsQueryStatusFilter`、`DnsQueryStatus`、`DnsQueryHistoryRecord` 与 `DnsQueryHistorySnapshot`；ports 导出 `DnsQueryHistoryPort` 抽象；application 实现 `DnsQueryHistoryApplication` 500 容量环形缓冲区、FIFO 逐出、暂停/恢复与单飞拉取；presentation 提供 `DnsQueryRowProjection` 与 `project_dns_history_page` 驱动多端查询历史列表与详情；composition 接入 `with_dns_query_history` 与 live 读模型；targeted nextest 1494/1494 全绿，Clippy `-D warnings` 全绿，所有守卫通过），现移入过去佛 §12.52；现在佛 107 → 102 pt（26 → 25 条），过去佛 400 → 405 pt（140 → 141 条），联合分母 584 pt 保持不变。
- 2026-10-30 第六十八交付批次：`FLPARITY-001`（5 pt）IP 质量查询、缓存与多端展示完成（contract 固化 `IpUsageType`、`IpRiskLevel`、`IpQualityStatus`、`IpQualityRecord` 与 `IpQualitySnapshot`；ports 导出 `IpQualityProbePort` 抽象；application 实现 `IpQualityApplication` 3 天 TTL 缓存、60 秒失败冷却与单飞防重；presentation 提供 `IpQualityBadgeProjection` 与 `summarize_ip_quality` 单源驱动四端质量徽标与详情；targeted nextest 1424/1424 全绿，Clippy `-D warnings` 全绿，所有守卫通过），现移入过去佛 §12.51；现在佛 112 → 107 pt（27 → 26 条），过去佛 395 → 400 pt（139 → 140 条），联合分母 584 pt 保持不变。
- 2026-10-30 第六十七交付批次：`GAP-7.4`（3 pt）三端用户可见文案与数据格式收口（英文直出/占位符/单位/版本单一源）完成（Slint demo owner 35+ 项业务错误/拒绝提示全部中文化；Bevy / Iced / test_support 路由意图错误文案收敛至 `routing_intent_token_rejected` / `routing_intent_refused_temporary` 键表；`rule_list_editor` 缺失观测提示中文化；Bevy 节点地理分布 `globe.rs` 全量中文化并接入 `locales_table_surface_*` 键表；`test_interface_flap_guard` 修复高 CPU 负载下的时间窗口抖动 flakiness；i18n-guard 0 违规，targeted nextest 1551/1551 全绿通过），现移入过去佛 §12.50；现在佛 115 → 112 pt（28 → 27 条），过去佛 392 → 395 pt（138 → 139 条），联合分母 584 pt 保持不变。
- 2026-10-30 第六十六交付批次：`FLPARITY-043`（5 pt）将 Mihomo 从 v1.19.32 升级至 FlClash v0.9.2 的 v1.19.33 基线并核对协议能力完成（锁定全平台二进制 SHA-256；`mihomo-api` / `mihomo-driver` / `infiltrator-contract` / `infiltrator-core` 全量升级至 v1.19.33；真实二进制烟测与 conformance 证明双引擎 live owner switch 全绿通过；工具链统一至 Rust 1.99.1），现移入过去佛 §12.49；现在佛 120 → 115 pt（29 → 28 条），过去佛 387 → 392 pt（137 → 138 条），联合分母 584 pt 保持不变。
- 2026-10-29 第六十五交付批次：`FLPARITY-047`（3 pt）系统代理显式关闭的 Core 所有权保护完成（contract/ports/application/desktop 实现 typed claim/release、权威读回、仅精确 claim 可释放、journal 不一致与外部改写时 fail closed），现移入过去佛 §12.48；现在佛 123 → 120 pt（30 → 29 条），过去佛 384 → 387 pt（136 → 137 条），联合分母 584 pt 保持不变。
- 2026-10-29 第六十二登记批次（Forest capture 与 FlClash v0.9.2 色彩复核）：确认 118 条桌面业务 capture 与 Android debug capture 默认 Forest，但它只是取证 skin，不改运行时主题偏好；检查当前代码发现 MaterialSurfaceTokens/HCT scheme 没有生产消费者，Android/Slint 与 Iced/Bevy 的颜色通路也未形成一个共享实时主题。旧主题图比 10-29 palette 变更早，三端 manifest 当前分别只有 8/8/1 行，不能作当前对标结论。将 `FLPARITY-014` 从过去佛退回现在佛（InProgress，5 pt）；扩充 `PARITY-001.6` 由 3→5 pt，覆盖当前构建四主题实图及 Android 三 flavor。现在佛 110→117 pt（27→28 条），过去佛 389→384 pt（137→136 条），联合分母 576→578 pt（196 条不变），严格归档完成率 55.8%→55.7%；本批仅重开/扩充 TODO，不宣称 UI 已修复或通过像素验收。
- 2026-10-29 第六十三登记批次（FlClash v0.9.2 四表面对标补缺）：逐项核对官方 release/tag 与主题、IP quality、桌面材质、Android VPN lifecycle 等对应提交。确认原 45 项已覆盖主题/HCT、四主题像素证据、托盘、动态色、菜单/手势、桌面材质、Profile QR、local proxy、IP quality、general override、Mihomo v1.19.33、DNS 与 Android 壳；新增 `FLPARITY-046`（3 pt）记录 Android dashboard 首页信息架构/简化布局差距（原 `FLPARITY-001` 只记 IP 质量，未记首页组织）。将重开 Activity 保留既有 VPN session 的验收补入 `FLPARITY-005`，将跨端自定义代理表单字段 4/6/9 不一致及 danger/延迟语义补入 `GAP-7.6`，避免重复 ID。Forest 仍只是 capture skin。FlClash 对标增至 46 IDs / 177 pt（Past 25/83、Now 21/94）；现在佛 117→120 pt（28→29 条），联合分母 578→581 pt（196→197 条），累计 1305→1308 pt，严格归档率 55.7%→55.6%；只做差距建档，不宣称视觉/行为已修复。
- 2026-10-29 第六十四登记批次（FlClash v0.9.2 release 清单终核）：核对官方 release/CHANGELOG 全部 feature、fix、performance 项；发现系统代理显式关闭仍无所有权门控（`SystemProxyApplication::set_enabled(false)` 无条件清除，虽 orphan recovery 会保护外部改动），新增 Core-first `FLPARITY-047`（3 pt），要求 typed claim/release、权威读回与并发外部修改保护；该差距不由 `FLPARITY-034` 修复向导覆盖。补强 `FLPARITY-011` 明确纳入 iOS-style switch 视觉/交互、docked menus 与 bar-button hover/lift；现有 `WIDGET-001.1` 仅统一 Bevy 开关几何，不能替代四表面行为/视觉验收。Forest 仍只用于 capture。FlClash 对标共 47 IDs / 180 pt（Past 25/83、Now 22/97）；现在佛 120→123 pt（29→30 条），联合分母 581→584 pt（197→198 条），累计 1308→1311 pt，严格归档率 55.6%→55.5%；仅建档，不宣称缺口已修复。
- 2026-10-29 第六十一交付批次：`FLPARITY-045`（2 pt）Android 三 flavor 可重复的 Forest 默认业务采集与显式主题巡检完成（在 `MainActivity.kt` 与 `Theme.kt` 接入 `ForestColors` 与 `BuildConfig.DEBUG` 保护的 `ThemeOverride` 测试主题注入；`scripts/android/android_interaction_capture.py`、`capture_device.py` 与 `capture_flow.py` 接入 `--theme` 参数并默认 `forest` 采集；`capture_evidence.py` 写入 theme marker，发布构建完全禁止 capture override 保证用户偏好不被篡改；Python 仿真测试与 Android JVM 单元测试 100% 通过），现移入过去佛 §12.47；现在佛 112 → 110 pt（28 → 27 条），过去佛 387 → 389 pt（136 → 137 条），联合分母 576 pt 保持不变。
- 2026-10-29 第六十交付批次：`FLPARITY-042`（3 pt）四表面可配置延迟测速 timeout 并在新探测中即时生效完成（Slint 业务面板接入 `probe_settings_from_draft` 支持自定义 URL 与 timeout 解析及回读保持，单测断言 SetRuntimeProbeSettings 意图与 9000ms 参数；Android 导出 `buildSetRuntimeProbeSettingsCommandJson` 与 `ProxiesViewModel.setProbeSettings` 并支持在 `probeOutbound` 传入动态 timeout，JVM 单元测试覆盖自定义参数序列化），定向测试与 JVM 测试全绿，现移入过去佛 §12.46；现在佛 115 → 112 pt（29 → 28 条），过去佛 384 → 387 pt（135 → 136 条），联合分母 576 pt 保持不变。
- `FLPARITY-014` 历史交付声明已在 2026-10-29 第六十二登记批次撤回：静态 consumer 审计发现 MaterialSurfaceTokens/HCT scheme 无生产消费者、Android 动态色与 Slint 内嵌色表没有接入共享主题 owner；原局部测试仍保留为实现证据，不作为四表面已统一/像素通过的结论。
- 2026-10-29 第五十八交付批次：`FLPARITY-044`（3 pt）FlClash v0.9.2 默认配色质量基准与共享色板校准完成（在 `surface_hierarchy.rs` 完成四主题角色色板与层级校准，校正 Forest 为高对比护眼浅绿纸面色板消除语义倒置，建立对标 FlClash v0.9.2 的 High-Contrast Dark 模式，正文与次级文字全部达到 WCAG AA/AAA 对比度要求，容器与边界非文本对比度达标；单元测试 100% 覆盖四皮肤色板对比度与层级单调性），定向测试 442/442 全绿，现移入过去佛 §12.44；现在佛 123 → 120 pt（31 → 30 条），过去佛 376 → 379 pt（133 → 134 条），联合分母 576 pt 保持不变。
- 2026-10-29 第五十七交付批次：`FLPARITY-024`（5 pt）Material HCT/CAM16 色彩科学与 Monet 调色板完成（实现标准 CAM16 观察条件色貌模型、`solve_to_srgb` 牛顿迭代与二分色域边界求解、TonalPalette 0..100 单调明度生成、MaterialCorePalette 6 核心色板派生、MaterialColorScheme 5 级表面容器层级与 FlClash v0.9.2 对标的高对比深色模式；单元测试覆盖 Red/Green/Blue/M3 Baseline #6750A4/White/Black 官方参考向量与色域容差），定向测试 441/441 全绿，现移入过去佛 §12.43；现在佛 128 → 123 pt（32 → 31 条），过去佛 371 → 376 pt（132 → 133 条），联合分母 576 pt 保持不变。
- 2026-10-29 第五十六复核登记批次（对标 FlClash v0.9.2 全量缺口与 Forest 采集规范建档）：系统梳理四大 UI 表面（Iced、Bevy、Slint、Android Compose）相对 FlClash v0.9.2 的全景差距。明确桌面三端 118 条业务场景已默认配置 Forest 采集，并以 `FLPARITY-045`（2 pt）补齐 Android 三 flavor 测试专用 Forest 默认采集与显式主题巡检 harness；确认深色模式可读性不足与配色质量缺陷由 `FLPARITY-014`、`FLPARITY-024`、`FLPARITY-044` 三位一体收口。全仓 FlClash 对标立项共 45 项 / 174 pt（过去佛 21 项 / 70 pt，现在佛 24 项 / 104 pt）。Now 126→128 pt（31→32 条），联合分母 574→576 pt（195→196 条），累计 1301→1303 pt，严格归档完成率 55.8%。”
- 2026-10-29 第五十五复核登记批次（深色可读性与色板质量）：用户反馈深色模式辨识度不足、整体颜色配置不及 FlClash；核对确认 Iced/Bevy/Slint 的 118 条业务采集行默认 Forest，64 条主题巡检仍需分别覆盖四主题，Android Compose capture 走实际系统/产品主题而非 Forest override。扩充 `FLPARITY-014` 验收范围并新增 `FLPARITY-044`（3 pt）建立 FlClash 同场景色板评审、共享 token 校准与像素/对比证据。Now 123→126 pt（30→31 条），联合分母 571→574 pt（194→195 条），累计 1298→1301 pt，严格归档完成率 56.0%→55.9%。
- 2026-10-29 第五十四复核登记批次：对照 [FlClash v0.9.2 release](https://github.com/chen08209/FlClash/releases/tag/v0.9.2) 与本仓四表面当前代码/证据，发现先前 `FLPARITY-014`/`024` 只完成 contract 初稿，不能声称四表面已采用或官方 HCT 算法已实现：Forest 与护眼浅色语义相反、主题 token 未证明四表面实际消费，`HctColor` 是近似 CIELAB 而非 CAM16/HCT solver。两条从过去佛 §12.43/§12.44 退回现在佛（原 ID、5 pt、`InProgress`）；新增 `FLPARITY-036`～`041` 六条 23 pt，另立 `FLPARITY-042`（3 pt）补齐 Slint/Android 测速 timeout owner readback。后续逐层核对确认 Android command encoder 已支持传入 timeout，但没有运行时设置入口；另发现本仓 Mihomo 制品锁定 v1.19.32，而 release 明确使用 v1.19.33，故立 `FLPARITY-043`（5 pt）做协议/能力审计后再 hash-locked 升级。业务 capture Forest 仍只是采集配置，不代表运行时主题或四主题像素验收。Now 82→123 pt（20→30 条），Past 381→371 pt（134→132 条），联合分母 540→571 pt（182→194 条），累计 1267→1298 pt，严格归档完成率 57.4%→56.0%。
- 2026-10-29 第五十一交付批次：`FLPARITY-029`（3 pt）Android 常驻通知栏流速监控与 Quick Settings 快捷磁贴深度集成完成（`MihomoVpnServiceNotification` 接入实时流速与节点名称 headline 格式化与通知更新通道；`InfiltratorTileService` 接入 `TileVpnState` 6 态同步与安全 Intent 路由），结构门全绿，现移入过去佛 §12.42；现在佛 95 → 92 pt（23 → 22 条），过去佛 368 → 371 pt（131 → 132 条），联合分母 540 pt 保持不变。
- 2026-10-29 第五十交付批次：`FLPARITY-027`（3 pt）Android TV 遥控器 D-pad 全屏焦点光晕与 Leanback 专属体验完成（contract 固化 `DpadKeyAction`、`FocusHaloSpec` 1.05x 缩放与居中滚动规范与 `DpadFocusTopology` 防死锁算法；presentation 导出 `DpadFocusStyleProjection`；Android Compose 实现 `tvFocusHalo` Modifier 与 Manifest Leanback 声明），定向测试与结构门全绿，现移入过去佛 §12.41；现在佛 98 → 95 pt（24 → 23 条），过去佛 365 → 368 pt（130 → 131 条），联合分母 540 pt 保持不变。
- 2026-10-29 第四十九交付批次：`FLPARITY-025`（3 pt）全端骨架屏微光加载与情绪插画空状态完成（contract 固化 `ShimmerAnimationSpec`、`EmptyStateIllustrationKind` 6 大业务插画模型与 `SkeletonCardLayoutKind`；presentation 导出 `SkeletonCardLayoutProjection` 与 `EmptyStateProjection` 统一驱动四端占位与空状态），定向测试全绿，现移入过去佛 §12.40；现在佛 101 → 98 pt（25 → 24 条），过去佛 362 → 365 pt（129 → 130 条），联合分母 540 pt 保持不变。
- 2026-10-29 第四十八交付批次：`FLPARITY-026`（3 pt）局域网代理共享二维码与同网段设备配置向导完成（contract 固化 `NetworkInterfaceInfo`、`LanDevicePlatform`、`LanDeviceGuideStep`、`LanProxyEndpointModel` 与 PAC/导出命令生成；ports 导出 `LanInterfaceProbePort`；presentation 导出 `LanShareModalProjection` 统一驱动四端弹窗与扫码接入），定向测试全绿，现移入过去佛 §12.39；现在佛 104 → 101 pt（26 → 25 条），过去佛 359 → 362 pt（128 → 129 条），联合分母 540 pt 保持不变。
- 2026-10-29 第四十七交付批次：`FLPARITY-035`（3 pt）Android 12+ 动态壁纸 Monet 取色与分应用代理高清图标选择器完成（`InfiltratorTheme` 接入 `dynamicDarkColorScheme`/`dynamicLightColorScheme`；`AppRoutingScreen` 接入应用图标异步加载、用户与系统应用分组 Tab、实时过滤搜索、全选与反选），结构门全绿，现移入过去佛 §12.38；现在佛 107 → 104 pt（27 → 26 条），过去佛 356 → 359 pt（127 → 128 条），联合分母 540 pt 保持不变。
- 2026-10-29 第四十六交付批次：`FLPARITY-034`（3 pt）系统代理与 TUN 模式提权修复向导完成（contract 固化 `PrivilegeRepairKind`、`PrivilegeElevationAction`、`SystemProxyWatchdogStatus` 篡改看门狗与 setcap 脚本格式化；ports 定义 `PrivilegeRepairPort` 提权与检测端口；presentation 导出 `PrivilegeRepairModalProjection` 驱动四端弹窗与一键避让/恢复），定向测试全绿，现移入过去佛 §12.37；现在佛 110 → 107 pt（28 → 27 条），过去佛 353 → 356 pt（126 → 127 条），联合分母 540 pt 保持不变。
- 2026-10-29 第四十五交付批次：`FLPARITY-033`（3 pt）订阅更新故障智能诊断器与节点正则清洗向导完成（contract 固化 `SubscriptionDiagnosticCategory`、`SubscriptionFetchDiagnostic` 敏感 URL 脱敏与 Markdown 报告、`NodeRegexRule`；domain 实现 `CompiledNodeRegexPipeline` 批量清洗与预览；presentation 提供 `SubscriptionDiagnosticDialogProjection` 与向导预览模型），定向测试全绿，现移入过去佛 §12.36；现在佛 113 → 110 pt（29 → 28 条），过去佛 350 → 353 pt（125 → 126 条），联合分母 540 pt 保持不变。
- 2026-10-29 第四十四交付批次：`FLPARITY-032`（3 pt）进程级连接图标提取与应用流量排行看板完成（contract 固化 `ProcessCategory`、`ProcessIconToken`、`AppTrafficAggregate` 与 `TopTalkersReport`；presentation 提供 `TopTalkerCardProjection` 与 `aggregate_top_talkers` 单源驱动应用网络排行），定向测试全绿，现移入过去佛 §12.35；现在佛 116 → 113 pt（30 → 29 条），过去佛 347 → 350 pt（124 → 125 条），联合分母 540 pt 保持不变。
- 2026-10-29 第四十三交付批次：`FLPARITY-030`（3 pt）节点测速动态脉冲与策略组选优状态透视完成（contract 固化 `PingStage`、`StrategyDetail` 与 `NodeLatencyHistory` 抖动计算与归一化 Sparkline 数据；presentation 提供 `PingPulseProjection`、`StrategyCardProjection` 与 `format_jitter_badge` 徽标投影；四表面节点卡片接入平滑脉冲动效与策略透视），定向测试全绿，现移入过去佛 §12.34；现在佛 119 → 116 pt（31 → 30 条），过去佛 344 → 347 pt（123 → 124 条），联合分母 540 pt 保持不变。
- 2026-10-29 第四十二交付批次：`FLPARITY-031`（3 pt）分流规则实时命中统计、流量流向与规则仿真探针完成（contract 固化 `RuleHitSummary` 命中与流量统计契约；domain 提供 AST 匹配与规则判定；application 实现 `RuleTracerApplication` 5 级决策链与内存聚合；presentation 在 `RuleRowChips` 引入 `of_with_volume` 与 `volume_label` 单源驱动规则行流量徽标），定向测试全绿，现移入过去佛 §12.33；现在佛 122 → 119 pt（32 → 31 条），过去佛 341 → 344 pt（122 → 123 条），联合分母 540 pt 保持不变。
- 2026-10-29 第四十一交付批次：`FLPARITY-020`（5 pt）流媒体与 AI 服务可用性一键检测完成（contract 建立 `UnlockServiceKind`、`UnlockStatus`、`ServiceUnlockResult` 与 `NodeUnlockReport` 规范，覆盖 Netflix/YouTube/Disney+/OpenAI/Claude/Spotify 与地区/延迟；ports 定义 `MediaUnlockProbePort` 异步抽象；application 实现 `MediaUnlockApplication` 24 小时缓存、60 秒错误冷却与防重复探测；presentation 导出 `UnlockBadgeProjection` 与节点摘要），定向测试全绿，现移入过去佛 §12.32；现在佛 127 → 122 pt（33 → 32 条），过去佛 336 → 341 pt（121 → 122 条），联合分母 540 pt 保持不变。
- 2026-10-29 第四十交付批次：`FLPARITY-028`（3 pt）实时连接/请求一键添加自定义分流规则完成（contract 建立 `CreateRuleOverrideIntent` 意图与校验规范；domain 实现 `prepend_rule_override` 纯函数去重追加与 `ConnectionRuleSpec::to_override_intent`；application 在 `dispatch` 串接 `CommandIntent::AddCustomRule` 并复用 `edit_rules` 原子落盘与内核热载；presentation 暴露 `connection_rule_override_intent` 投影；四表面对齐详情抽屉一键添加操作），定向测试全绿，现移入过去佛 §12.31；现在佛 130 → 127 pt（34 → 33 条），过去佛 333 → 336 pt（120 → 121 条），联合分母 540 pt 保持不变。
- 2026-10-29 第三十九登记批次（FlClash v0.9.2 深度对标全面补齐建档立项，基线 `musicfrog-todo-20261029-v309`，零交付零办结，纯新增）：全面对照 FlClash v0.9.2，补立 `FLPARITY-030`～`035` 共 6 条 18 pt——节点测速动态脉冲与策略组选优状态透视（3 pt）、分流规则实时命中统计与规则仿真探针（3 pt）、进程级连接图标提取与应用流量排行看板（3 pt）、订阅更新故障智能诊断器与节点正则清洗向导（3 pt）、系统代理与 TUN 模式提权修复向导（3 pt）、Android 12+ 动态壁纸 Monet 取色与分应用代理高清图标选择器（3 pt）。按审计结果新增 FLPARITY-030～035 六项；业务采集皮肤设为 forest（1180x780/720x480），仅是捕获矩阵配置，不代表深色模式可读性或四表面主题集成；后续由 `FLPARITY-014`/`FLPARITY-024` 继续收口。现在佛 112 → 130 pt（28 → 34 条），未办结联合分母 522 → 540 pt（176 → 182 条），累计 1249 → 1267 pt，严格归档完成率 58.2% → 57.4%（727/1267）。
- 2026-10-29 第三十八交付批次：`FLPARITY-023`（5 pt）代理节点多维高级组合筛选器与批量测速工具栏完成（contract 建立 `ProxyFilterQuery` 与 `BatchProxyAction` 规范；presentation 实现 `matches_advanced_filter` 纯函数组合筛选并接入 `ProxiesPageModel`，支持协议/地区多选、延迟区间、关键词包含与负向排除；四表面对齐批量操作），定向测试全绿，现移入过去佛 §12.30；现在佛 117 → 112 pt（29 → 28 条），过去佛 328 → 333 pt（119 → 120 条），联合分母 522 pt 保持不变。
- 2026-10-29 第三十七交付批次：`FLPARITY-022`（3 pt）流量曲线悬停探针（Hover Crosshair Probe）与分流统计完成（presentation 建立 `ChartProbeSample` 统一探针模型与 `probe_traffic_waveform`/`probe_at_pixel` 命中算法，支持双向流速、相对时间戳与标准化速率格式化；Iced Canvas 联动双向十字扫描准星与 Micro HUD Tooltip；Bevy 对齐交互管线），定向测试全绿，现移入过去佛 §12.29；现在佛 120 → 117 pt（30 → 29 条），过去佛 325 → 328 pt（118 → 119 条），联合分母 522 pt 保持不变。
- 2026-10-29 第三十六交付批次：`FLPARITY-021`（3 pt）代理节点国家/地区国旗高分图标与代码映射完成（contract 固化 `GeoRegion` 规范枚举与 36+ 地区代码/emoji/label；shared 补全别名匹配字典；presentation 单源提供 `proxy_node_region` 与 `flag_icon_token` 投影；Slint 修复非出口节点 flag 丢失缺陷，全量展示节点国旗；Iced/Bevy 对齐地区解析），定向测试全绿，现移入过去佛 §12.28；现在佛 123 → 120 pt（31 → 30 条），过去佛 322 → 325 pt（117 → 118 条），联合分母 522 pt 保持不变。
- 2026-10-28 第三十五登记批次（FlClash v0.9.2 深度对标全量补齐建档立项，基线 `musicfrog-todo-20261028-v305`，零交付零办结，纯新增）：新立 `FLPARITY-020`～`029` 共 10 条 36 pt——流媒体与 AI 可用性检测、国家地区国旗高分图标、流量曲线悬停探针、多维高级组合筛选与批量工具栏、Material 3 动态色彩科学引擎、全端骨架屏微光与情绪插画空状态、局域网共享向导与二维码、Android TV 遥控器 D-pad 光晕、实时连接一键加规则、Android 常驻通知栏监控与磁贴。全面梳理四大 UI（Iced、Bevy、Slint、Android Compose）与 FlClash v0.9.2 的差距，业务 capture 默认设为 forest（1180x780/720x480）；该采集皮肤不代表运行时主题、暗色可读性或 M3 色彩已通过验收。现在佛 87 → 123 pt（21 → 31 条），未办结联合分母 486 → 522 pt（166 → 176 条），累计 1213 → 1249 pt，严格归档完成率 59.9% → 58.2%（727/1249）。
- 2026-10-28 第三十四交付批次：`FLPARITY-015`（5 pt）代理节点响应式多列栅格、协议标签与批量测速动效完成（contract 固化 `ProxyProtocolBadge` 规范枚举与色彩标准；presentation 单源驱动 `proxy_protocol_chip`；四表面完成自适应列宽 1~4 列栅格、协议徽标与测速动效对齐），定向测试全绿，现移入过去佛 §12.27；现在佛 92 → 87 pt（22 → 21 条），过去佛 317 → 322 pt（116 → 117 条），联合分母 486 pt 保持不变。
- 2026-10-28 第三十三交付批次：`FLPARITY-016`（5 pt）概览页平滑贝塞尔流量曲线、动态多阶单位与分流模式快捷切换完成（修复 Slint `split_rate` 单位截断丢失 `/s` 缺陷，紧凑视口全宽显示 `MB/s`；Iced 出口节点卡片接入快捷切换代理节点直达路由；Iced/Bevy 贝塞尔样条曲线与渐变着色对齐），定向测试全绿，现移入过去佛 §12.26；现在佛 97 → 92 pt（23 → 22 条），过去佛 312 → 317 pt（115 → 116 条），联合分母 486 pt 保持不变。
- 2026-10-28 第三十二交付批次：`FLPARITY-008`（3 pt）+ `FLPARITY-019`（3 pt）共 6 pt 交付：`FLPARITY-008` Slint 通用工作台产品化完成（DNS/编辑器/过滤/诊断/同步/应用路由 6 大业务抽离专属页面，`page_workbench.slint` 重写为 `WorkbenchPanelCard`，内部 ID 清零，355 项单测全绿），现移入过去佛 §12.24；`FLPARITY-019` 订阅用户信息（流量配额/到期时间）可视化完成（单源 `project_quota`，Iced 双/三色警戒进度条，Bevy 接入 `quota_progress_color_token`，Slint `ProfileCard` 补齐动画进度条与到期提示并在首页展示），现移入过去佛 §12.25；现在佛 103 → 97 pt（25 → 23 条），过去佛 306 → 312 pt（113 → 115 条），联合分母 486 pt 保持不变。
- 2026-10-28 第三十一交付批次：`FLPARITY-012`（3 pt）网络切换自动清理 DNS 缓存完成（contract 扩充 `network_generation` 与 `auto_flush_generation`；application 组合 `DnsCacheApplication` 与权威去重，物理网络变化时自动触发带 generation 标记的 Fake-IP 与系统 DNS 缓存刷新；fake ports 单元测试验证单次触发与重试；desktop host 与 surface 完成依赖接线；Android 保持显式 unsupported 不伪造结果），定向测试全绿，现移入过去佛 §12.23；现在佛 106 → 103 pt（26 → 25 条），联合分母 486 pt 保持不变。
- 2026-10-28 第三十交付批次：`GAP-7.5`（5 pt）三端布局破损与主题令牌硬伤收口完成（Bevy OnAccent 与禁用卡衰减、Slint 容器自适应与叠画修复、Iced 模态按钮悬浮固定/抽屉双行/概览三列限制），定向测试全绿，现移入过去佛 §12.22；现在佛 111 → 106 pt（27 → 26 条），联合分母 486 pt 保持不变。
- 2026-10-28 第二十九登记批次（FlClash v0.9.2 深度对标补齐，零交付零办结，纯新增）：新立 `FLPARITY-015`～`019` 共 5 条 23 pt——代理节点响应式多列栅格与协议徽标、平滑贝塞尔流量曲线与分流胶囊、跨端微动效与页面过渡、无框窗口 CSD 与实时流量胶囊、订阅配额/到期可视化与自动更新；现在佛 88 → 111 pt（22 → 27 条），联合分母 463 → 486 pt，累计 1190 → 1213 pt，严格归档完成率 61.1% → 59.9%（727/1213）。
- 2026-10-28 第二十八交付批次：`BEVYCAP-002`（3 pt）实现、独立交互行为单测、Bevy/Iced 双视口像素回执与 cross_surface_manifest 认证均完成，现移入过去佛 §12.21；现在佛 91 → 88 pt（23 → 22 条），联合分母 463 pt 保持不变。
- 2026-10-27 第二十四登记批次（三端实机截图视觉审计立项，零交付零办结，纯新增）：逐张实读三端 364 张实机 PNG 并对账 capture 矩阵后新立 5 条——`GAP-7.3`（采集坏数据与诚信，P0，3 pt）、`GAP-7.4`（文案与数据格式，3 pt）、`GAP-7.5`（布局与主题硬伤，5 pt）、`GAP-7.6`（IA 与交互模型，5 pt）、`PARITY-001.6`（主题巡检矩阵对齐 + AMOLED 补采，3 pt），共 19 pt；现在佛 13 → 32 pt（5 → 10 条），联合分母 380 → 399 pt，累计 1107 → 1126 pt，严格归档完成率 65.7% → 64.6%（727/1126）。审计全文：`.evidence/wip/2026-10-27-tri-surface-visual-audit.md`；业务双视口捕获欠账仍由 `PARITY-001.5`/`BEVYCAP-002` 承载，本批不重复登记。登记当时 AMOLED 三端 0 张；2026-10-28 工作树已变为三端 16 页 × 4 主题，但唯一 SHA 仅 176/157/172，仍不能宣称全覆盖，以 `PARITY-001.6`/`GAP-7.3` 实测为准。
- 2026-10-28 第二十五登记批次（FlClash v0.9.2 UI 对标，零交付零办结，纯新增）：新立 `FLPARITY-001`～`013` 共 13 条 56 pt——IP 质量、Profile QR/手机到电视、实时请求、DNS 查询历史、Android 移动壳/访问控制、桌面托盘、Tools/About 信息架构、Slint 工作台产品化、Bevy 回滚确认、Slint IME/焦点、跨端交互原语、网络切换自动清 DNS、加密备份真实导出与恢复；现在佛 32 → 88 pt（10 → 23 条），联合分母 399 → 455 pt，累计 1126 → 1182 pt，严格归档完成率 64.6% → 61.5%（727/1182）。
- 2026-10-28 第二十六交付批次：`FLPARITY-009`（2 pt）实现、行为测试、Bevy/Iced 双视口像素回执与 owner readback 均完成，现移入过去佛 §12.20；现在佛 88 → 86 pt（23 → 22 条），联合分母不变。
- 2026-10-28 第二十七登记批次（视觉系统与采集默认，零交付零办结，纯新增）：新立 `FLPARITY-014`（5 pt），记录四主题 surface/material/semantic 层级与 FlClash v0.9.2 动态配色差距；业务采集默认皮肤由 dark 改为 forest，主题巡检保留四主题矩阵（像素验收状态另计）；现在佛 86 → 91 pt（22 → 23 条），联合分母 458 → 463 pt，累计 1185 → 1190 pt，严格归档完成率 61.4% → 61.1%。
- 2026-10-24 第二十三交付批次：`SLINTUI-002`~`007` 六条对账收口（#31 mono 数字本批实物、其余与 E-037~041 同源点逐条核符，见过去佛 §12.19）；第二十二批次见 v290 行（E-041）。
- 2026-10-24 第二十二交付批次：`EFFECTS-041`（#167 无框自绘标题栏本批交付；#168/#169 平台材质为外部实机腿、#170 避让几何在树，见过去佛 §12.18）；第二十一批次见 v289 行（E-040）。
- 2026-10-24 第二十一交付批次：`EFFECTS-040`（#18 持久模型管道本批交付；#163/#164 ListView 随滚动架构切片、#165 FilterModel 第二真源裁决不引入、#166 真增量随流架构，见过去佛 §12.17）；第二十批次见 v288 行（E-039）。
- 2026-10-24 第二十交付批次：`EFFECTS-039`（#160/#161 states 注入 + #162 弹性胶囊控件本批交付，见过去佛 §12.16）；第十九批次见 v287 行（E-038）。
- 2026-10-24 第十九交付批次：`EFFECTS-038`（#155 双 no-op 删除 + 全量分类、#158 实测复核旧式赋值已清零、#159 死属性 8 条删除 233→225，见过去佛 §12.15）；第十八批次见 v286 行（E-037）。
- 2026-10-24 第十八交付批次：`EFFECTS-037`（#150~#153 Zinc 阶梯/双阶阴影/内描边/桌面双列网格本批交付，#154 随布局落地待像素回执，见过去佛 §12.14）；第十七批次见 v285 行（E-035）。
- 2026-10-24 第十七交付批次：`EFFECTS-035`（#137 悬浮淡入本批交付，#138/#139/#141/#144 四点记录找回在树，#142/#143/#145 三腿在案，见过去佛 §12.13）；第十六批次见 v284 行（E-034）。
- 2026-10-24 第十六交付批次：`EFFECTS-034`（#132 导航单一真源本批交付，#135 记录找回在树，#133/#134/#136 视觉腿在案，见过去佛 §12.12）；第十五批次见 v283 行（E-033）。
- 2026-10-24 第十五交付批次：`EFFECTS-033`（#128~#130 流光 Shader 管线 + 矢量孪生本批交付，#131 类型开关在案、运行时腿待实机，见过去佛 §12.11）；第十四批次见 v282 行（E-032）。
- 2026-10-24 第十四交付批次：`EFFECTS-032`（#126 贝塞尔+渐变填充本批交付，#127 脉冲粒子记录找回在树，见过去佛 §12.10）；第十三批次见 v281 行（E-031）。
- 2026-10-24 第十三交付批次：`EFFECTS-031`（#122~#125 三层缓存架构 + 主交通图表迁移，见过去佛 §12.9）；第十二批次见 v280 行（E-030）。
- 2026-10-24 第十二交付批次：`EFFECTS-030`（#119~#121 语义 Token/双 stop 渐变/分阶投影，见过去佛 §12.8）；第十一批次见 v279 行（E-029）。
- 2026-10-24 第十一交付批次：`EFFECTS-029`（#116~#118 官方 Animation 协调器 + 按需帧门 + 抽屉退场保留，见过去佛 §12.7）；第十批次见 v278 行（E-027）。
- 2026-10-24 第十交付批次：`EFFECTS-027`（#99 视口点击→球面逆映射拾取管线本批交付，#98 纹理腿分腿既有 + 架构裁决，见过去佛 §12.6）；第九批次见 v277 行（E-024）。
- 2026-10-24 第九交付批次：`EFFECTS-024`（#86~#88 离屏降采样双 pass 高斯模糊管线本批交付，#89 前景合成为集成边界在案，见过去佛 §12.5）；第八批次见 v276 行（E-050）。
- 2026-10-24 第八交付批次：`EFFECTS-050`（#197 Niri 离屏管线本批交付，#198/#199/#200 分腿既有，见过去佛 §12.4）；第七批次见 v275 行（E-048/049）。
- 2026-10-24 迁移损伤修复（依 168b83c pristine 复位；证据 = e680ed8/1e00aa3 两次波次提交信息 + 代码实测）：e680ed8/3887ef6/1e00aa3 三次看板迁移的行拼接乱码逐行修复；`EFFECTS-003`（残形「TS-003」）与 `EFFECTS-008`（被截断并入 007 尾部且曾误回登本板）复位重建；`EFFECTS-013` 与 `EFFECTS-015`（`ButtonHierarchy`/`ButtonPressScale` 已在树）由误标的 Planned 复位 Integrated；Layer 2 的 `EFFECTS-016`~`020` 错位 ID 全部复位；`EFFECTS-006`/`013`/`020` 的 points 依 pristine 复位（3→2）；本板误留的 `EFFECTS-008` 与 `EFFECTS-020`（曾误标 019）重复项删除，`EFFECTS-012`/`014` 归位本板；空壳节（§四/§五/§六/5.1）与 EFFECTS-007 孤儿残句清除；§7.1 过期事实按强制化台账刷新；`TODO.md` 波次 A 日志的羽化条目 ID 复位为 018。修复前实测两板为 44 条/122 pt 与 68 条/180 pt（v268 所记过去佛 69 条/183 pt 与当时实际不符），修复后以本行实测为准。
- Slint 九条来自 2026-10-19 的表面审计实测：缺 7 类页面导致 19 条业务无载体、只派发 42 种 `CommandIntent`、59 条业务里 2 条完整实现 / 18 条部分实现 / 39 条未实现且今天零锚点、4 个测试文件重实现 Slint 逻辑构成第二业务真源、2 个已声明回调无调用点。
- Bevy UI 能力四条源自 2026-10-20 的强制化实测审计；执行状态（机器强制还是人工评审、存量欠账多少）以 Bevy UI 能力规范文档「强制化台账」为单一事实源，本板不另记第二份。
<!-- POINT_LEDGER:END -->

- **执行次序（2026-10-24 第二十三批次后复核）**：本板条目按 **Core-first** 排成一条可推的链——① Slint 表面（`EFFECTS-037`~`041`）→ ③ Slint 表面（`EFFECTS-037`~`041`）→ ④ Bevy 捕获证据（`BEVYCAP-002`）→ ⑤ 工程约束门禁（`ENG-002.2`）→ ⑥ 像素认证收口（`PARITY-001.5`）。上层条目在前置落地前不得开工。
- **cargo 纪律（减少被 cargo 阻塞）**：同一时刻只有一条流允许跑 cargo；并行期只写代码，编译与像素取证由收口流**统一跑一次**（全仓 nextest → 守卫 → fmt → clippy → 三表面捕获 → 重建回执账本 → 全局验收）。**只有真正因缺少实际环境而无法在本机取回执的验收项**才进[未来佛](CODE_TODO_FUTURE.md)；排队等 cargo lane 不构成阻塞，那些条目留在本板按上面的次序推进。

## 一、大计划主线与推进顺序

| 主线 | 覆盖里程碑 | 条目 | 出口判据 |
| --- | --- | --- | --- |
| A 发布流水线主线 | 0.60 / 0.69 / 1.0.0 | `PUB-006.1`、`PUB-006.6`～`PUB-006.11` | 七个中性 crate 可按顺序 dry-run 发布，docs.rs 构建通过，版本拓扑无环 |
| B 表现与截图认证主线 | 0.44～0.49 / 0.92 | `PARITY-001.2`～`001.4`、`PLATFORM-001.1`～`001.2`、`SLINT-001.1`～`001.9`、`GAP-4.1` | 59 条业务 × 桌面三 UI 锚点齐全、standard/compact 双视口像素回执全绿；Android 三 flavor 交互截图全绿 |
| C 质量红线与安装包主线 | 0.70～0.73 | `BANDROID-044.1`、`044.2`、`044.4`、`044.5`、`APPID-001.3`、`BEVYCAP-001.1`、`001.2`、`002.1`、`003.1` | Lint/ktlint/Detekt/架构守卫强制生效且零违规；Linux deb/AppImage 安装冒烟通过；Bevy UI 的 rem 尺寸通道与场景律法成为唯一机器红线 |
| D 视觉与模型统一主线 | 0.48 / 0.90 / 0.91 | `GAP-7.1`、`GAP-7.3`～`GAP-7.6`、`FLPARITY-001`～`FLPARITY-047`、`BANDROID-016.2`、`BANDROID-016.3`、`BEVYUI-001`～`BEVYUI-007`、`SLINTUI-001`～`SLINTUI-007`、`EFFECTS-001`～`EFFECTS-050` | 三大 UI 共用页面模型；地球增强与帧报告；FlClash v0.9.2 产品面、Bevy UI 105 点、Slint 34 点与跨端深化 200 点自底向上整改闭环 |

**推进顺序：B → A → C/D。**B 是「每个业务都真跑过」的判定器，先把它做成可复跑的流水线；A 解锁 0.69/1.0.0 的发布顺序门；C/D 与 B 并行但不得挤占 B 的取证窗口。

## 二、五大平台 × 三大 UI 表现架构覆盖矩阵（360° 无死角口径）

| 平台 | 表现层 | 当前状态 | 收口条目 | 关闭证据 |
| --- | --- | --- | --- | --- |
| Linux 桌面 | Iced / Bevy / Slint | 源码可编译；parity 注册表已含三表面，但 Slint 侧 59 个 scenario 全为 pending、场景表只覆盖 Slint 自身 24 个场景 | `PARITY-001.2`、`PLATFORM-001.1`、`APPID-001.3`、`SLINT-001.1`～`001.9` | 59 业务 × 三 UI 双视口像素回执 + deb/AppImage 安装冒烟 |
| Windows 桌面 | Iced / Bevy / Slint | 本机仅有交叉 `cargo check`，无产物 | `PLATFORM-001.1`（交叉腿）、`PARITY-001.2`（截图腿） | 交叉编译回执 + 与平台无关的像素回执；NSIS 产物腿在斗战胜佛（`APPID-001.4`、`GAP-2.1`） |
| macOS 桌面 | Iced / Bevy（Slint 未启用） | Apple target 被 `apple-ci-deferral-guard.py` 显式禁建 | 斗战胜佛（`GAP-2.2`） | 平台矩阵登记 `Unsupported` + 斗战胜佛阻塞方登记；本板不假装已覆盖 |
| Android | Compose / Bevy / Slint flavor | 源码链路已通，APK 与交互截图未认证 | `PARITY-001.4`、`PLATFORM-001.1` | 模拟器 M1 交互截图与行为回执 + APK identity；真机腿在斗战胜佛 §一 |
| iOS | Rust host seam（626 行 stub） | 无 target、无包、无 NetworkExtension | 斗战胜佛（`GAP-3.2`） | 平台矩阵登记 `Unsupported` + 阻塞方登记 |

**覆盖口径规则：**任一平台 × UI × 业务单元格若没有可复跑证据，一律记为「缺口」并建条目；禁止用「其它端已通过」外推。平台结论以平台矩阵文档（由 L1 入口导航）为唯一事实源。

## 三、逐业务 360° 覆盖清单（59 条业务能力）

| 业务域 | feature 数 | 桌面三 UI 锚点 | 双视口像素回执 | Android 三 flavor 截图 |
| --- | ---: | --- | --- | --- |
| 总览与运行时（overview / runtime / lifecycle / doctor / core） | 7 | 缺 Slint | 缺 Slint | 缺 |
| 代理与节点（proxies） | 7 | 缺 Slint | 缺 Slint | 缺 |
| 配置档与订阅（profiles / sync） | 12 | 缺 Slint | 缺 Slint | 缺 |
| 规则与提供者（rules） | 6 | 缺 Slint | 缺 Slint | 缺 |
| DNS 与编辑器（dns / editor） | 7 | 缺 Slint | 缺 Slint | 缺 |
| 连接与日志（connections / logs） | 7 | 缺 Slint | 缺 Slint | 缺 |
| 测速与命令面板（speedtest / shell） | 7 | 缺 Slint | 缺 Slint | 缺 |
| 设置（settings） | 5 | 缺 Slint | 缺 Slint | 缺 |
| 应用与路由策略（app） | 1 | 缺 Slint | 缺 Slint | 缺 |
| **合计** | **59** | 三表面全部登记：Iced contract 59/59 + scenario 59/59；Bevy 59/59 + 59/59；Slint contract 58/59、scenario 2/59（另 16 条业务已有载体与表面行为测试，交互路径未齐故仍 pending） | 177 个清单格：implemented 120 / ready 2 / pending 55 | 0 个 flavor 截图回执（取证流已交付） |

缺口来源与实测结论（2026-10-19 全量审计）：`scripts/parity/feature_evidence.tsv` 与 `scripts/parity/cross_surface_manifest.tsv` 原先只登记 iced/bevy 两表面（各 118 单元格），`scripts/parity/visual_receipts.py` 只接受 `--iced-run`/`--bevy-run`。现已补齐第三表面：**177 单元格（59 业务 × 3 表面）全部登记并可解析**，Slint 侧 57 格为 `pending` + 逐条原因，2 格已转 `ready`（`profiles-import-modal` 与 `profiles-delete-confirm`：真实表面行为测试 + 标准/紧凑双视口真实像素回执，PNG 摘要与原生 bounds 全部绑定）。四个 pending 行为锚点已全部 anchored（含运行中测速控件的取消路径）。Slint 表面审计得到五条硬事实：① 页面词汇只有 Home/Proxy/Config/Settings/Network/Connections/Logs/Rules/Traffic/AppSettings/About，**dns、editor、filter、mixin、sync、app_routing、doctor 七类页面不存在**，按契约页面归属共 **19 条业务在第三 UI 无载体**（产品缺口）；② Slint 只派发 42 种 `CommandIntent`，DNS、编辑器、脚本沙箱、测速、快照、聚合器、命令面板等 30 余种意图从未派发；③ 59 条业务里 **2 条完整实现、18 条部分实现、39 条未实现**（2026-10-19 实测），今天这 2 条已各自带上面行为测试与像素回执，其余 57 条仍不能合法标记为 Slint scenario 锚点；④ 四个测试文件在用例里**重实现 Slint 逻辑**（路由模式映射、网络模式序列、主题白名单、十六进制解析、连接排序），构成仓库禁止的「第二业务真源」，且永远无法当表面锚点；⑤ 两个已声明回调 `refresh-profile(string)`、`refresh-home-egress()` 在任何 `.slint` 文件里都没有调用点。收口由 `PARITY-001.2`～`001.4`、`PLATFORM-001.1`～`001.2` 与 `SLINT-001.1`～`SLINT-001.9` 共同承担。

## 七、C 主线：质量红线与安装包

### 7.1 Bevy UI 能力规范强制化（2026-10-20 实测立项：bsn! 已是机器红线，rem 部分机器强制；0条，已清空，BEVYCAP-002 交付见过去佛 §12.21）

立项与现状（2026-10-20 实测立项，2026-10-24 复核刷新）：`bsn!` 场景律法与 rem/em 尺寸通道均已机器强制——产品守卫 `scripts/quality/bevy_rem_guard.py` 是唯一阻塞执行器（命名/派生 px 值、硬编码 px 字面量、非 rem 字号、字面量 `em` 全部拦截，存量台账 `scripts/quality/bevy_rem_inventory.json` 已归零，180 处存量迁移由 `BEVYCAP-001.2` 收口），共享包场景门 `scripts/quality/bevy_scene_gate.py` 为上游 advisory 镜像；⑦ 小件收敛（`bsn_list!`/`Ready`/`despawn_all`/胶囊圆角）已由 `BEVYCAP-002.1` 机器化。执行状态的单一事实源是 Bevy UI 能力规范文档的「强制化台账」一节，本节不另记第二份。`BEVYCAP-002` 的全部捕获模块与双视口像素认证均已收口交付至过去佛 §12.21。

## 十、跨平台 GUI/CLI 表现深化 200 点自底向上攻坚（报告实施；现存 6 条 / 17 pt（SLINTUI），已交付 37 条见[过去佛](CODE_TODO_PAST.md)）

### 10.4 Layer 3: Bevy UI 0.20 渲染管线与 WESL 材质深化（0条，已清空，涵盖 #76~#115）

### 10.5 Layer 4: Iced 0.14 原生动画、Canvas 三层缓存与 Shader（0条，已清空，涵盖 #116~#145）

### 10.6 Layer 5: Slint 1.18+ Skia 硬件加速、桌面多列网格与原生状态机（0条，已清空，涵盖 #146~#170）

### 10.9 Slint 质感与架构攻坚（0条，已清空，涵盖 #1~#34；逐条见过去佛 §12.19）

## 10.10 像素认证收口（2条 / 10pt）

- [ ] PARITY-001.5 [kind: evidence] [priority: P0] [points: 5] [owner: infiltrator-iced + infiltrator-bevy-ui] [status: InProgress] 桌面两端剩余业务的真实捕获与像素回执
  - 产物：Iced 与 Bevy 两端尚未认证的业务各有一条真实交互与双视口像素回执；缺载体或缺可滚动身份的业务逐条立项。
  - 依赖：本批交付的 owner 能力（13 个端口 fixture、同步冲突与概览卡片排序的应用实现、契约层的 LAN/PAC/路由读回）。
  - 验收：每条业务在两端都有「demo/捕获真的执行了业务」的证据与黑盒像素差回执。
  - 计划：① 用 `crates/infiltrator-test-support` 新增的端口 fixture 补 Bevy 捕获（规则提供者刷新、订阅源失败、配置导入、端口冲突修复、LAN 安全、TUN 权限、版本回滚、快照存储、测速历史、文档 codec、核心进程）；② 用同一批 fixture 补 Iced demo 的 owner 绑定（脚本沙箱与导出评审、差异视图、同步冲突与恢复、编辑器文档会话、聚合预览、应用路由枚举）；③ 逐条重跑 `scripts/parity/accept_surface_interactions.py --all --surfaces iced,bevy` 并把 certified 格登记为 `ready`。
  - 已知阻塞（不是本条的缺口）：部分业务的成功态与「仅路由到页面」的基线像素相同（如端口冲突修复后的「无冲突」空态），需要契约层补一个「已修复」标记，否则像素判据无法区分。
  - 进展（2026-10-21）：已建**已验证回执账本** `.evidence/visual_receipts.json`（159 份进程绑定像素回执，逐份绑定场景图与哈希、激活标记与原生边界、像素签名、进程与窗口身份、归档的产品构建与捕获源码指纹），解析器对齐格 **0 → 76**，验收把 **74 格提升为 `ready`**（注册表：76 ready / 44 implemented / 57 pending）。Iced 全量矩阵 93/93、Slint 60/60。
  - 剩余缺口（诚实列出）：① Bevy 全量矩阵 10 场失败——`dns-hosts-editor`（双视口）、`runtime-telemetry-observation`（双视口）、`proxies-uri-import-preview`（双视口）、`profiles-filter-editor`（双视口）、`dns-query-details-compact`、`doctor-failure-recovery-compact`，app 日志显示表面自己的捕获几何拒绝该控件（`capture geometry rejected …`、`filter capture progress: stage=2 pane=Filter editable=false`）；② 13 个 Iced 业务按契约没有 demo activator；③ 16 个 Slint 业务虽然 `activated=true`，但激活前后像素完全一致（0 像素变化）。
  - **最终认证（2026-10-21 静止树，一次跑完）**：账本 `.evidence/visual_receipts.json` **208 份回执**（iced 108 / bevy 69 / slint 31），**全部通过严格 `verified_receipts`**；解析器 `aligned` **97 / 177**、`unclosed` 80、`complete` false、`anchored` 312、`invalid` 1；验收 **208/272 格**认证、**97/136 业务-表面对**、64 格未认证、11 个锚点包全 `ok`、`receipt_failures {}`。注册表：ready 76 / implemented 44 / pending 57（iced 46/13、bevy 28/31、slint 2/57）。**`tree_moved_after_the_capture` 不再出现**（捕获写 `.evidence/`，不落在指纹路径内）。
  - 诚实缺口（未弱化任何断言）：① 唯一 invalid 是 `rules-list-editor/bevy` 的**紧凑视口缺回执**（该格已是 ready）；② Bevy 全量矩阵 **79/81**（`profiles-filter-editor` 两场 failed-marker，app 日志 `filter capture progress: stage=2 pane=Filter editable=false tabs=3 fields=5`，该格在注册表里是 implemented 非 ready，未损失已证像素）；验收矩阵 Bevy 11 场 failed-marker（几何拒绝：`script console/clipped: size=Vec2(874.0, 38.0)`、`rule-list-editor/clipped: size=Vec2(622.0, 85.0)`、`rules keyword field/clipped: size=Vec2(590.0, 36.0)`、`app routing rule/clipped: size=Vec2(44.0, 18.0)`）、Iced 10 场（编辑页窄档不滚动、无刷新控件、同步页无该业务、目录页名错配）；③ 剩余 43 格未认证是像素差为 0（激活像素与纯路由渲染相同）；④ Slint 侧 57 格 pending 的主因是该表面**没有对应业务页**（20 格报 `required feature lacks L1/L2/L3 closure`）。
  - 账本来源说明：严格入口拒绝任何含失败或非交互行的整跑，故按**验收自己的容错路径**（`build_receipts_tolerant` + 严格逐份验签）从三份验收跑批构建——全量矩阵只覆盖 27/27/18 个特性，直接用会让 25 个 ready 格失去像素、`aligned` 掉到 72。
  - 下一步：修 Bevy 那 10 场的捕获几何拒绝；为 13 个 Iced 业务补 activator 或按契约记为不承载；给 16 个 Slint 业务补真实交互差异；然后重跑三表面矩阵并重签账本。
  - 追加进展（2026-10-21，Iced 认证簇交付）：**8 个此前无 activator 的业务拿到真实交互与像素回执**——`app-routing-policy-editor`（std 52 268 / compact 12 370 px）、`profiles-subscription-error`（13 506 / 9 590）、`profiles-aggregator-preview`（151 895 / 168 566）、`profiles-script-sandbox`（7 008 / 8 073）、`profiles-script-export-review`（453 891 / 262 626）、`profiles-diff-viewer`（561 725 / 279 470）、`editor-yaml-error-navigation`（48 063 / 33 862）、`sync-conflict-resolver`（122 986 / compact 4 382——首跑 compact 为 0，原因是测的合并面板 404px 高于 393px 紧凑视口永远居不中，改测**第一条冲突键行**后复取复证）。每个业务都是真手势经真实 update 管道（共享 `ProfileApplication`/`ProfileAggregationApplication`/`ScriptApplication`/`ScriptExportApplication`/快照 Myers diff/codec 诊断），不是路由切换。
  - Iced 官方化与修复：`UI-FRAMEWORK-001` 的字形勾选框改官方 `iced::widget::checkbox`（**行为变更**：点击目标从整行收窄为方框+标签，行尾状态徽章不再可点、成为装饰）；`capture_frame.rs` 的已发布 `rendered-frame.png` 改为**跟随表面**（读回变化就重写），此前它冻结在写回执那一刻的帧，与合成器截图差 2.64% 且回执的帧证明是陈旧的。Iced 发布矩阵 **93/93**，新增业务场景 **16/16**。
  - **验收现状**：解析器 `aligned` **76 → 108**、`unclosed` 77、`complete` 仍 false；认证 209/272 格、100/136 业务-表面对。回执账本**仍被拒**（严格构建器拒绝任何含失败场景的整跑：`ValueError: iced: failed or non-interaction capture editor-large-document-window-standard`），须待剩余格闭合后重建；验收同时报 `tree_moved_after_the_capture`（树在跑批中移动），重绑要等树静默后由收口批次做。
  - **5 项未交付（确切原因）**：① `editor-large-document-window`/`editor-snippet-insertion`——编辑页窄档**不滚动**，文档面板被工具条与横幅挤出 480px 折叠线，业务像素不可观测（修法：窄档让编辑页像 App Routing 一样滚动并给文档/行号槽加 region；demo 手势已就绪，`ProfileStoreCapture::CAPTURE_LARGE_PROFILE` 4 000 条规则）；② `rules-provider-refresh-error`——demo 的规则提供者 fixture 带 `controller: None`，页面**没有可按的刷新控件**，须发布带 `RuleProviderControllerReference` 的提供者并经命令门面拒绝刷新（共用订阅错误那条 region）；③ `sync-restore-confirm`——Iced 同步页**根本没有快照恢复业务**（不渲染快照列表、不投恢复动作，共享 `SyncPageSnapshot.snapshots` 未投影），不伪造手势：要么表面补该业务，要么目录诚实写"该表面不承载"；④ `overview-topology-drilldown`——手势与就绪判据**已工作**（下钻落到共享导航目标），但捕获标记发布的是**已挂载的路由**（`rules`）而目录写的捕获页是 `overview`，属共享目录缺陷。
  - 共享改动需求（契约级，须一次落地）：① `crates/infiltrator-contract/src/parity.rs` 的 `OverviewTopologyDrilldown` 行把捕获页由 `overview` 改为 `rules`（Iced 下钻落到规则页；Bevy 是概览页原位面板，保持 `overview`）；② `SyncRestoreConfirm` 行按"该表面不承载"改为空页（或表面补业务后保持 `sync`）；③ `scripts/capture_iced_scenarios.tsv` 补 16 行新业务场景（app-routing-policy-editor / profiles-subscription-error / profiles-aggregator-preview / profiles-script-sandbox / profiles-script-export-review / profiles-diff-viewer / editor-yaml-error-navigation / sync-conflict-resolver，各 standard+compact）。
  - `UI-FRAMEWORK-001` ⑥（Iced 规则虚拟窗口改官方 `iced::widget::lazy`）**未交付并附证据**：开了 `lazy` feature 后规则页**整片渲染为空**（分页器仍报"15 rules"）；逐段二分（每次都是真实捕获）：纯 `text` 的段渲染、`column![text]` 的段渲染、**含 `Length::Fill` 子项的行渲染为空**、同一段放在 lazy 外渲染正确（`rules-dark.png` 与 HEAD 逐字节相同）；给段 `Length::Fill` 无效。结论：iced 0.14.2 的 `lazy` 会压掉规则行的 fill 宽度，故保留表面正确、**不开这个死依赖**；要落地必须让该带不含 `Length::Fill`（用共享内容宽度的固定宽）或修上游——属产品决定。

- [ ] PARITY-001.6 [kind: evidence] [priority: P0] [points: 5] [owner: infiltrator-slint + infiltrator-bevy-ui + infiltrator-iced + scripts/android] [status: Planned] 四表面四主题当前构建像素回执与 FlClash 同场景复核
  - 产物：三桌面 16 页 × 4 themes standard 视口及 Android Compose 三 flavor 核心页四主题图，均绑定当前 binary/source/build、PNG SHA、视口与 theme marker；附逐页并排差距索引，发现项必须映射到唯一 TODO ID。
  - 实测：桌面主题 PNG 最近时间为 2026-10-22/26/27，早于 10-29 palette 变更；manifest 仅 Iced 8 行、Bevy 8 行、Slint 1 行。Android 有 debug-only theme injection 测试，尚无主题图像回执；Forest capture 不代表生产主题。
  - 计划：① 三桌面重采 192 张、Android 三 flavor 对 overview/proxies/settings 采 36 张；② 与 FlClash v0.9.2 对同页同视口并排审阅暗色层级、Forest、强调/语义色、文字与卡片边界；③ 缺口逐项认领 `FLPARITY-*`/`GAP-*`，不可只留在审计笔记。
  - 验收：桌面 192/192、Android 36/36 回执由 `visual_receipts.py` 绑定本轮构建并通过主题/像素/几何验证；逐页评审表覆盖率 100%，每个未通过项均有唯一 task ID；旧 PNG、单测、TSV 配置不能代替实图。
  - 依赖：复用 `GAP-7.3` 唯一性守卫与 `FLPARITY-045` debug-only capture override；Forest 是 capture skin，不改变用户运行时主题。

## 10.11 三端视觉一致性实测整改（2026-10-27 视觉审计立项；4条 / 16pt）

逐张实读三端 364 张实机 PNG 的视觉审计（方法、逐域明细与全部文件证据见 `.evidence/wip/2026-10-27-tri-surface-visual-audit.md`）；采集矩阵欠账不在此节，仍由 §10.10 承载。

- [ ] GAP-7.3 [kind: evidence] [priority: P0] [points: 3] [owner: infiltrator-slint + infiltrator-bevy-ui + infiltrator-iced] [status: Planned] 三端截图坏数据与身份唯一性收口
  - 产物：坏截图全部重采；capture/manifest 统一身份守卫，非显式 alias 的同 SHA 产物失败。
  - 实测（2026-10-28）：三端各 182 PNG，唯一 SHA 仅 176/157/172；I `connections=logs`、`connections=logs=runtime(amoled)`、`about=settings(amoled)`；B `editor/filter/mixin/profiles/import` 同图、`about=settings`、`sync-conflict=restore`、`aggregator preview=wizard`；S `editor=mixin=diff/export/restore` 同图。
  - 其他坏数据：S editor 三张 compact 同 md5；I 脚本导出 SHA 非 hex 且 68 位；B 多张 compact 折叠线下同画面；B `overview-live` 与 demo 无差异；S 四业务只有错误 snackbar、无业务 UI。
  - 计划：① 修 Slint/Bevy compact 场景滚动锚点；② Iced artifact SHA fixture 改为真实 64-hex；③ 接 live fixture 与 Slint 成功态；④ manifest 生成器做全矩阵身份唯一守卫，alias 必须在独立清单显式声明。
  - 验收：所有非 alias 截图同页同主题唯一；目标状态可见；守卫对当前矩阵先报告、修后 enforce 全绿；`resolve_surface_evidence.py --structure-only` pass。

- [ ] GAP-7.6 [kind: mechanism] [priority: P2] [points: 5] [owner: infiltrator-contract + infiltrator-slint + infiltrator-bevy-ui + infiltrator-iced] [status: Planned] 三端信息架构与交互模型对齐（词表/字段/语义色）
  - 产物：contract 层共享导航词表 + 页面命名字典 + 语义色令牌（danger/延迟色阶）；底层先决，UI 只消费。
  - 实测：① 导航三套（S 8 项/B 4 项/I 3 项）、页面命名各异（首页 vs 核心概览、连接 vs 连接审计、节点 vs 代理节点 vs 节点选择、同步 vs 数据同步 vs WebDAV 同步、DNS vs DNS 设置 vs DNS & Fake-IP）；② 能力分叉——S proxies 无分组/测速、S 连接页缺聚合/空闲清理/断开筛选、S 三业务模态缺失、自定义节点字段数 S/B/I 为 4/6/9、遥测页三端三构且 S 只读卡配「确认/取消」按钮属语义误用；③ 冲突解决三套交互模型须先 contract 裁决；④ 语义色——危险操作角色与延迟等级/阈值不一致（S 158ms 绿 vs B/I 约 150ms 橙）、主色青 vs 蓝；⑤ S compact 导航丢连接/日志/规则/流量入口。
  - 计划：① `infiltrator-contract` 在 `design_tokens.rs` 既有结构上扩 danger/延迟色阶令牌与导航词表/页名字典（补 owner 结构，禁止散落 bool）；② 能力分叉逐条与 B 主线 `SLINT-001.1`～`001.9` 对账——已在其范围内的挂过去佛对应条目核销，确认无载体才在本条立项；③ 三端接入替换后全仓 grep 旧命名清零；④ 冲突模型裁决后三端统一并补行为测试。
  - 验收：词表/令牌进 contract 且三端消费（无第二副本）；旧命名 grep 清零；重采巡检核对页面命名与语义色；`resolve_surface_evidence.py --structure-only` pass。
  - 依赖：contract 裁决先行（自底向上铁律）；Tools/About 与 Android 壳层分别由 `FLPARITY-007`/`FLPARITY-005` 承载；`design_tokens.rs` 有未提交改动（另一会话），开工前对齐。

## 10.12 FlClash v0.9.2 对标缺口（2026-10-29 更新；19条 / 82pt）

对标 [FlClash v0.9.2 release](https://github.com/chen08209/FlClash/releases/tag/v0.9.2) 的产品面与 UI。四表面问题按唯一 owner 归档：Iced 为静态内建 palette 已开始消费、但运行时动态 scheme/外观读回与全角色一致性未闭环（`FLPARITY-014`/`036`）；Bevy 的外观解析目前只选 skin，实际控件仍绘制本地 `Theme` 镜像（`FLPARITY-014`）；Slint 仍在 `theme.slint` 独立维护 hex/zinc 与状态角色（`FLPARITY-014`/`036`）；Android 仍由 `Color.kt`/Compose 动态色走独立 palette，且无主题实图（`FLPARITY-014`/`PARITY-001.6`）。118 条桌面业务 capture 与 Android debug capture 默认 Forest 只代表采集参数，不改变用户运行主题（`FLPARITY-045`）。47 个已知差距均映射到本节 ID 或 §10.10/§10.11 的唯一任务；旧 PNG 不作为当前结论。

- [ ] FLPARITY-002 [kind: mechanism] [priority: P1] [points: 5] [owner: infiltrator-contract + infiltrator-application + infiltrator-ports + three desktop UIs + android/ui] [status: Planned] Profile 二维码分享与手机到电视导入
  - 产物：owner 产出脱敏 share payload 与一次性 pairing token；Profile share sheet 显示 QR/复制链接，TV/Android 经 typed import 接收并回读；四表面消费同一 contract。
  - 实测：locales 明说“二维码扫描尚未接入”；Slint 只有 share sheet，Iced/Bevy/Android 无 QR 生成/扫描；FlClash v0.9.2 提供 Profile QR 与 phone-to-TV links。
  - 计划：① contract 定义 share/import 状态与 token 生命周期；② application/ports 实现本地生成与扫描/配对，订阅凭据不入 QR 或日志；③ 各 UI 接 typed intents；④ 先同端回环，再做手机到电视。
  - 验收：QR 只编码 owner 给出的脱敏 payload；扫描/配对失败可重试且不伪造成功；过期 token 拒绝；四表面行为测试与像素/交互回执。
  - 依赖：安全边界与 TV 发现/传输协议先裁决；外部网络依赖不可推时拆出 blocked 腿。

- [ ] FLPARITY-005 [kind: mechanism] [priority: P1] [points: 5] [owner: infiltrator-contract + android/ui + Android Bevy/Slint surfaces] [status: Planned] Android 移动壳、访问控制状态与系统入口
  - 产物：contract/Android owner 裁决一级导航与二级目的地；三 Android 表面共用可发现性模型；Connections/Logs/DNS/About 可达；底部状态条显示 access-control 模式、已选应用数与开关；内容边到边并避让系统栏/IME；TV 返回聚焦 dashboard，QS Tile 有深链。
  - 实测：Compose `AppSection` 5 个 tab，Connections/Logs 藏在 AppRouting 设置，AboutScreen 无调用；`Scaffold(innerPadding)` 阻止内容滚到导航栏下；无 access 状态条；`InfiltratorTileService` 仅系统入口，UI 不可发现。
  - 计划：① contract/Android owner 定唯一导航/状态 DTO；② 修 Compose route 与共享导航模型；③ Bevy/Slint Android 对齐；④ 接 edge-to-edge、TV 返回、QS 深链和状态条；⑤ 三 flavor 实机/模拟器截图。
  - 验收：每个目的地有唯一入口与返回栈；access 状态来自 owner 读回；内容在系统导航栏下可见且可滚；TV/QS 深链可达；重开 Activity 时保留既有 VPN session/Core identity 与权威 readback，不重复启动或重载运行中的 VPN；三 flavor 交互截图与行为测试。
  - 依赖：BANDROID owner/IPC 先决；与 GAP-7.6 桌面词表分开，不在 Compose 自建第二套业务状态。

- [ ] FLPARITY-006 [kind: mechanism] [priority: P1] [points: 5] [owner: infiltrator-desktop + infiltrator-bevy-desktop-host + infiltrator-slint + infiltrator-iced] [status: Planned] 桌面托盘宿主与彩色状态图标
  - 产物：desktop host 为 Bevy/Slint 提供真实 SNI/Windows/macOS tray；共享 tray status contract 统一 tooltip、menu、状态色与 macOS 单色选项；Iced 收敛为参考实现；`desktop-smoke.sh tray` 覆盖三表面。
  - 实测：contract 明确 Bevy 无 tray，Slint 无 tray host，只有 Iced 有完整 tray；FlClash v0.9.2 有 colored tray status icons 与 macOS monochrome option。
  - 计划：① 抽 desktop tray host seam，不改 Iced 行为；② Bevy/Slint host 接同一 `TraySupport`/menu snapshot；③ 加状态色与 macOS 单色 preference；④ 扩 smoke/协议回执。
  - 验收：三桌面表面均能显示、退出、切换 routing/profile/系统代理；无 tray 时 typed unsupported；macOS monochrome 可切换；Linux SNI 与 Windows smoke 回执。
  - 依赖：平台 host 能力与 GAP-2.1 真机腿分离；不得用页面内伪托盘冒充系统托盘。

- [ ] FLPARITY-007 [kind: mechanism] [priority: P1] [points: 5] [owner: infiltrator-contract + infiltrator-shared + three desktop UIs + android/ui] [status: Planned] Tools 信息架构与 About/Disclaimer/Licenses 页面
  - 产物：contract 页面词表增加 Tools/About/Disclaimer/Licenses；四表面有可达 Tools hub（设置/备份/热键/访问控制/高级/资源/免责声明/关于），About、许可证列表/详情独立路由；Settings 不再内嵌完整字体许可证正文。
  - 实测：Slint Settings 聚合 DNS/编辑器/过滤/诊断/同步/应用路由且 About 独立；Iced 无 About route，Settings 直接铺 HarmonyOS 字体许可证全文且热键只在 Settings；Bevy 只有字体许可 dialog、无热键管理；Android AboutScreen 存在但无路由。FlClash v0.9.2 Tools 聚合这些入口。
  - 计划：① 定 Tools 与 Settings 边界及页面词表；② 四表面加 route 和导航入口；③ 许可证/免责声明迁共享 catalog；④ 移除 Settings 内嵌正文。
  - 验收：四表面均可从主导航两步内到达 About/免责声明/许可证；无裸许可证全文铺页；旧 route/命名 grep 清零；四表面双视口像素回执。
  - 依赖：与 GAP-7.6 词表裁决同批；Android 路由由 `FLPARITY-005` 提供壳层。

- [ ] FLPARITY-010 [kind: mechanism] [priority: P1] [points: 3] [owner: infiltrator-contract + infiltrator-slint] [status: Planned] Slint 输入法、焦点与键盘导航模型
  - 产物：Slint 表面消费共享 typed focus/composition 状态；文本框支持中文 IME 组合、候选框定位、Tab 循环、Esc/Enter 语义与候选框跟随；组合期间不触发全局快捷键。
  - 实测：Slint parity gap 文档明确 `shell-keyboard-focus` pending，原因是 Slint 没有 IME/窗口焦点模型；`SLINT-FOCUS-RIG-001` 仅登记取证阻塞，未登记产品实现。
  - 计划：① 定 contract 的 focus/composition 状态和命令；② Slint adapter 接 text-input composition 与键盘事件；③ 补可键盘到达的控件顺序；④ 用真实 IME/按键台架取回执。
  - 验收：中文输入不丢字、不误发全局 chord；Tab/Shift-Tab/Esc/Enter 路径行为测试；焦点像素回执；`shell-keyboard-focus` 可认证。
  - 依赖：与 `SLINT-FOCUS-RIG-001` 的取证环境分开；不得用合成按键状态冒充真实 IME。

- [ ] FLPARITY-011 [kind: mechanism] [priority: P1] [points: 5] [owner: infiltrator-contract + infiltrator-presentation + three desktop UIs + android/ui] [status: Planned] 跨端 docked search、弹性 sheet、长按排序与 hover 交互基线
  - 产物：contract/共享 presentation 定义 shell search dock、menu、sheet sizing、long-press reorder、iOS-style switch 状态与 hover/press/focus typed intent/readback；三桌面同一语义，Android 保留 touch/keyboard 等价路径。
  - 实测：搜索分散在页面内，无统一 dock；sheet 高度多为固定/内容拼接；长按识别只在 Bevy gesture 层，未形成跨端产品模型；部分 hover 缺 lift；开关几何虽有 Bevy 共享 owner（`WIDGET-001.1`），四表面 iOS-style 外观、状态反馈与指针/焦点行为未完成同场景验收。FlClash v0.9.2 明确 docked search/menus、stretchy sheets、long-press reordering、bar-button lift 与 iOS-style switches。
  - 计划：① 裁决 dock/menu/sheet/reorder/switch 状态和 owner，禁止各 UI 自造；② 共享 presentation 提供快照/意图并复用 shared switch owner；③ 三桌面实现，Android 用 touch/keyboard/无障碍等价；④ 加行为测试与像素/指针回执。
  - 验收：同一手势在四表面产生同一 typed intent；菜单可发现且可键盘操作；sheet 按内容/可用高度伸缩且可滚；长按排序有取消/失败/回读；switch 开/关与 bar button hover/lift 四表面行为/外观一致、hover 不触发布局重算；无障碍焦点顺序可达。
  - 依赖：`PIXEL-POINTER-001` 仍负责真实指针像素回执；本条只登记产品行为与实现。

- [ ] FLPARITY-014 [kind: visual] [priority: P0] [points: 5] [owner: infiltrator-contract/design_tokens + infiltrator-bevy-widgets + infiltrator-iced + infiltrator-slint + android/ui] [status: InProgress] 四主题共享调色板真实消费与暗色/Forest 层级重校
  - 现状：Iced 已将内建 `ThemePalette` 部分接入 skin，但 HCT 生成 scheme 与系统外观完整读回未贯通；Bevy 只用共享解析结果选 skin，绘制仍走本地 `Theme`；Slint `theme.slint` 与 Android `Color.kt`/Compose 各维护独立色路。Forest capture override 不是产品主题设置。
  - 产物：Contract/Core 明确 built-in theme 的唯一 typed palette/readback 与系统动态色优先级；四端消费同一 surface ladder、primary/secondary ink、accent/semantic pairs，不保留互相漂移的第二色表。
  - 实测：对比值/契约测试不能证明视觉层级或色彩质感；用户明确反馈深色卡片与底色难区分、整体配色弱于 FlClash。现有主题截图陈旧且覆盖不齐；复核不得只看 WCAG 数字，还需评价冷暖角色、accent 层级、状态色和 dashboard 信息主次。
  - 计划：① Contract/Owner 裁决 palette 生成、System/动态色解析及权威读回；② Iced/Bevy/Slint/Compose 接同一 scheme 与 surface ladder，移除本地镜像；③ 逐端验证 dark/Forest/Amoled 的卡片、图表、按钮、状态色，并把每个残余问题落入唯一任务。
  - 验收：四端生产 consumer 均有颜色角色断言；文本 WCAG AA、非文本边界≥1.5:1、相邻表面有量化差；FlClash 同场景并排评审与当前构建像素回执按 `PARITY-001.6` 收口。依赖 `FLPARITY-024`/`044`；缺 Core owner 结构先补底层，禁止 UI 私改 hex。

- [ ] FLPARITY-017 [kind: visual] [priority: P1] [points: 5] [owner: infiltrator-contract/design_tokens + infiltrator-bevy-widgets + infiltrator-iced + infiltrator-slint + android/ui] [status: Planned] 跨端微动效与页面过渡系统（Motion System & Transitions）
  - 产物：contract 裁决 typed `MotionTokens`（duration/easing/stiffness/damping）与统一转场语义（Shared Axis、Fade Through、Drawer Slide）；三桌面与 Android 表面接入平滑路由过渡、模态弹性入场与卡片悬停按压反馈。
  - 实测：当前三大桌面 UI 路由切换均为 0 毫秒硬切（瞬时帧置换），弹窗与抽屉瞬间蹦出毫无缓动，按钮与卡片 hover/active 缺乏平滑过渡（或仅硬跳色）；FlClash v0.9.2 拥有完整的 Material 3 动效体系：页面滑动淡入、抽屉平滑阻尼滑出、卡片轻微抬升与涟漪回馈。
  - 计划：① contract 建立统一动效令牌枚举（standard, emphasized, decelerated, spring）；② Bevy 接入 ECS 缓动插值，Iced 接入 iced_animation/canvas 缓动，Slint 接入 property animation，Compose 接入 AnimatedContent；③ 为页面路由、弹窗遮罩、抽屉滑出统一配置转场动效。
  - 验收：路由切换具有连续感知，模态弹窗平滑入场无闪烁；动效支持无障碍 Reduced Motion 关闭开关；定向单元测试与实机帧率测试。
  - 依赖：不破坏无头捕获与截图测试（截图模式自动禁用动效或跳至终态）。

- [ ] FLPARITY-018 [kind: mechanism] [priority: P1] [points: 5] [owner: infiltrator-desktop + infiltrator-bevy-desktop-host + infiltrator-slint + infiltrator-iced] [status: Planned] 自定义无框窗口标题栏（CSD）、实时流量胶囊与多端统一系统窗管
  - 产物：desktop host 为三桌面表面提供统一无边框 CSD 协议，集成标题栏实时流速胶囊、搜索快捷入口与原生窗口控制按钮（Win11 Snap 布局菜单、macOS 红绿灯位置避让、Linux 窗口主题对齐）。
  - 实测：三大桌面标题栏严重分叉：Slint 自绘 chrome 与系统窗口拖拽冲突且 hover 不自然；Bevy 悬浮按钮在不同分辨率被视口下缘截断；Iced 为传统自绘条且无流量胶囊集成；三端均无 Windows 11 Snap 布局支持；FlClash v0.9.2 深度集成桌面 CSD，标题栏常驻网络流速胶囊与搜索按钮，窗口控制手感极佳。
  - 计划：① 在 infiltrator-desktop 抽象统一无框标题栏协议与窗口拖拽/缩放/双击最大化控制；② 各桌面 Surface 集成顶栏流速胶囊组件与窗口按钮；③ 对接 Windows 11 Snap Layout API 与 macOS 交通灯内边距避让；④ 补全窗口控制 smoke 脚本。
  - 验收：三桌面标题栏视觉语言统一且无按钮裁切；标题栏流速胶囊实时同步读回；Win11 下最大化按钮可唤起 Snap 菜单；desktop-smoke.sh 验证通过。
  - 依赖：EFFECTS-046 平台抽象；与 FLPARITY-006 托盘宿主协同。

- [ ] FLPARITY-036 [kind: visual] [priority: P1] [points: 5] [owner: infiltrator-contract + infiltrator-application + infiltrator-presentation + four UI surfaces] [status: Planned] 自定义种子色与 Material 色彩风格选择
  - 产物：提供可保存的 seed color 与有限、显式的 palette style preference；由唯一 owner 读回并向四表面发布同一 `MaterialColorScheme`。
  - 实测：Android 仅消费系统壁纸动态色；桌面没有统一的 seed/style 设置与持久读回；contract 色彩工具本身不等于用户可选主题。
  - 计划：依赖真实 HCT 算法定可表达的 style 集合，再建立 typed preference、application readback 与 presentation 投影。
  - 验收：seed 修改、取消、保存、重启读回、无效输入均有 fake/nextest；四表面使用相同 scheme，主题场景与像素回执可追溯偏好身份。
  - 依赖：`FLPARITY-024`、`FLPARITY-014`；持久化 owner 未裁决前不得在 UI 本地保存。

- [ ] FLPARITY-039 [kind: visual] [priority: P1] [points: 3] [owner: infiltrator-desktop + three desktop UI hosts] [status: Planned] 桌面侧栏系统材质效果与诚实降级
  - 产物：为桌面 sidebar 提供 host-owned Mica/Acrylic/Vibrancy 类系统材质能力及 typed `Unsupported`/fallback，材质强度随主题与减少透明偏好收敛。
  - 实测：Bevy 有模态 backdrop blur，不是系统窗口材质；Iced/Slint sidebar 当前是普通不透明 surface，三端无统一 host effect contract。
  - 计划：先做平台能力/窗口生命周期与主题合约，再接入各 surface；Android 与无 blur compositor 明确走静态 surface。
  - 验收：Win/macOS/Linux 实际窗口截图证明效果和 fallback；系统关闭透明/窗口失焦/主题切换不闪烁，headless 不伪造系统 blur。
  - 依赖：desktop host capability 与平台效果 owner 先裁决；只剩真实目标机才能验收的腿留未来佛。

- [ ] FLPARITY-046 [kind: visual] [priority: P1] [points: 3] [owner: infiltrator-contract + infiltrator-presentation + android/ui + scripts/android] [status: Planned] Android dashboard 信息层级与简化布局对标
  - 产物：基于 owner snapshot 裁决 Android 首页主次与紧凑/宽屏布局；保留详情页承载次要信息，不在 Compose 私自推导状态。
  - 实测：当前首页把 VPN、路由模式、流量、Core、出口 IP 与日志预览分成独立卡片；尚无与 FlClash v0.9.2 simpler Android dashboard 的当前构建同场景并排评审，窄屏下主状态与主要操作的层级/首屏密度待核。
  - 计划：绑定当前 Compose APK 与 FlClash v0.9.2 同视口截图，对账信息优先级、首屏可见内容、重复状态和滚动长度；仅按差距调整共享 projection/页面结构，并把非首页功能差异归回既有任务。
  - 验收：compact/expanded 首页评审表覆盖率 100%；VPN 主状态、主操作与关键流量指标在目标视口层级明确；无重复业务真源；三 Android flavor 截图、scenario 与交互回执齐备。
  - 依赖：复用 `FLPARITY-001` 的出口 IP 质量 owner 与 `FLPARITY-005` 的 Android 导航/生命周期；以 `PARITY-001.6` 登记图像，不重复生成验收台账。

## 十一、工程约束门禁补齐（错误类型纪律与合规收尾）

### 11.1 错误类型纪律与可选审计工具（2026-10-22 实测登记；manifest 批次已交付）

- [ ] ENG-002.2 [kind: refactor] [priority: P1] [points: 3] [owner: infiltrator-domain + infiltrator-core] [status: Planned] domain / core 层的 `anyhow` 收敛到 typed 错误
  - 产物：`infiltrator-domain`（31 文件 / 208 处引用）与 `infiltrator-core`（22 文件 / 167 处引用）的错误类型从 `anyhow::Result` 收敛为每层一个 typed 错误枚举（可复用的失败类别 + 上下文），边界处保留一处 `From<anyhow::Error>` 收敛（`lurker-core-contract` 已有该范式）。
  - 依赖：`infiltrator-domain` 与 `infiltrator-core` 的独占写窗口；需按 owner 分层逐个 crate 推进，不跨层并行改同一批文件。
  - 验收：两个 crate 内 `anyhow` 引用归零或只剩边界收敛处；工作区 `cargo nextest` 全绿；契约层错误类型零变化（对外错误面不得因本条漂移）。
  - 事实依据：2026-10-22 实测——`infiltrator-contract`/`infiltrator-ports`/`infiltrator-application`/`infiltrator-presentation` 的 anyhow 引用均为 0，只有 domain 与 core 两层大面积使用，错误类型纪律目前只在两层失守。
  - 计划：先 domain 后 core，每层一个纵切批次；每批补齐失败类别的行为测试再迁调用点。

### 11.2 依赖政策落地后的合规缺口与收尾（2026-10-22 实测登记）
