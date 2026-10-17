# MusicFrog 工程 TODO · 现在佛

> 层级：L4

## 0. 编写规则

- 唯一用途：当前产品交付窗口内未办结的必要能力与直接前置，覆盖 engine-neutral Core、Mihomo/sing-box 双适配、Kotlin VPNService/权限、Bevy/Slint/Compose Android APK、共享 UI 契约和桌面页面/服务管理。按依赖顺序推进，不把必要功能降到未来佛；未来佛只保留可延后的视觉、性能、发行便利和工具链优化。每条只交付一个可观察结果；父级只是章节。
- 每条仅保留唯一 owner、产物、依赖、验收；header ≤240 字符、详情 ≤6 行、整块 ≤1200 字符。证据留 nextest run ID / guard 回执；事实变化改写原条，不追加长过程叙事。
- 已实现只待验证的项在[过去佛](CODE_TODO_PAST.md)（含模拟器/真机实机证据条件）；暂缓排期项在[未来佛](CODE_TODO_FUTURE.md)。已验证结果退出全部看板并在 [TODO.md](../TODO.md) 登记归档，不重开旧交付。
- 状态词汇：`Planned / Blocked / Ready / InProgress`（本板）与 `Integrated`（过去佛，已交付待验证）。`Verified` 只是办结过渡态，不驻留任何看板。
- Rust 测试唯一执行器 `cargo nextest`（统一收口带 `--workspace --no-fail-fast`）；Clippy 用 `--all-targets -D warnings`；禁止裸 `cargo test`。多阶段操作必须 typed enum/sealed，禁止 bool/Option 组合状态机（总纲强制条款）。
- 迁移看板必须同次移出原文件、移入目标文件，保持稳定 task ID、owner、依赖、验收与 points。
- points 合同：`1` 局部证据/文档/配置；`2` 单 owner focused 包；`3` 跨 owner 纵切或普通机制包；`5` 产品采用/终局门。超过 5pt 先拆。priority 与看板位置不改变 points。
- 严格交付率只计 archived points；执行态报告对 Planned/Blocked/Ready 计 0%、InProgress 计 50%、Integrated 计 75%，不以执行态进度代替验收办结。
- 等价映射（避免重复条目）：`BANDROID-001.2 ≡ 040.5`；`BANDROID-004.2 ≡ 040.3`；`BANDROID-036.1` 的语义回归与 `042.3/042.4` 同批验证。
- **自底向上逐层裁决铁律（底层先决，严禁上层修补）：**在推进任何 TODO 时，若遇架构未决（领域模型模糊、所有权未定、状态二义性、缺少权威读回或跨端契约不一致等），必须严格遵循底层先决原则，在 Contract/Domain/Core 完成裁决与真实实现后，再逐步向上推进至 Presentation 与 UI 表面。严禁在 UI 层（Iced/Bevy/Slint/Android）增加临时布尔/状态标志位、增加容错垫片、自行推导业务事实或写空实现/TODO 注释就地修补；违者一律打回重做。
- **Core-first 阶段门：**先完成所有可独立推进的底层 Core、双后端 conformance、Android 唯一 owner/IPC/VPNService 工作。若只剩现有 UI 消费者的旧调用阻塞 Core 契约退役，允许把 typed API 边界迁移作为对应 Core 切片一次完成；禁止同时改页面布局、视觉或新增 UI 功能，迁移完再集中构建验收。无真机证据的设备验收项继续开放，不能以 host/fake 测试替代，也不能报告设备验收通过。
- **不可解依赖不阻塞：**仅受缺失物理设备、受控 CI tag、用户 secrets/keystore 等当前外部不可解依赖约束的条目保持原位与现有状态并注明依赖方；它们不阻塞其它条目选取、统一验证批次执行与已达标条目办结登记。

<!-- POINT_LEDGER:BEGIN -->
- Point 基线 `musicfrog-todo-20261017-v209`：当前现在佛 19 pt（9 条）/ 过去佛 49 pt（18 条）/ 未来佛 47 pt（24 条），联合未办结分母 115 pt；已归档交付 644 pt（243 条）；累计 759 pt，严格归档完成率 84.8%。
<!-- POINT_LEDGER:END -->

## 阶段一：代码健康度与全仓质量门禁（前置基石，已解锁全仓验证批次）

> 阶段门：CI-QUALITY-001～005 质量门禁全量通过；Kotlin 800 行预算守卫（BANDROID-044.3）已办结归档生效。

## 阶段二：底层服务与宿主边界收口（Android Owner / VPNService / Host 闭环）

## 阶段三：应用状态、生命周期与 UI 信息架构（自底向上 UI 接入）

> 阶段门：CORE-UI-002 统一验证批次（结构门 + parity + 全量 nextest 5116 项 + Clippy 0 警告）已全绿收口并办结归档。

## 阶段四：CI 自动化留痕与最终构建产物（交付与发布保障）

- [ ] BANDROID-018.2 [kind: evidence] [priority: P0] [points: 1] [owner: CI] [status: Blocked] tag-only CI 实际全绿回执
  - 产物：受控 `v*` tag release 与 Android workflow 的完整 run 链接和分项证据；普通 push/PR 不运行。
  - 依赖：高成本 tag-only 工作流；CI-QUALITY-001～005；CORE-UI-002（全仓绿）。
  - 验收：tag run 中 nextest、构建、Clippy、APK identity 与 Android tests 全绿并归档。
- [ ] BANDROID-019.2 [kind: evidence] [priority: P0] [points: 1] [owner: CI] [status: Blocked] manifest/aapt 守卫 CI 回执
  - 产物：`android-manifest-contract-guard.py` 与 aapt identity 检查在 CI 实际执行并留存回执。
  - 依赖：BANDROID-018.2。
  - 验收：回执绑定 run 与 APK 哈希。
- [ ] BANDROID-038.2 [kind: evidence] [priority: P0] [points: 1] [owner: CI + release workflow] [status: Blocked] 每个 APK 的 aapt identity CI 检查
  - 产物：tag release 对每个最终 APK 的 aapt identity 检查实际接入并留存回执；普通 push/PR 不触发构建。
  - 依赖：已交付的 identity registry 与 guard（BANDROID-038.1）。
  - 验收：负例（drift APK）在 CI 被拒；回执绑定 APK SHA。

## 阶段五：1.0 中性内核可发布（开源独立框架 · 潜伏者 / Lurker）

> 阶段门 1.0-M1「中性内核可发布」：`lurker-core-contract`、`lurker-core-ports`、`lurker-profile-store`、`mihomo-driver`、`singbox-driver`、`lurker-engine-runtime` 六个中性 crate 按依赖顺序（contract → ports/profile-store → engines → runtime）`cargo package` 通过；`lurker-core-contract` 首发布后，其余五个 `cargo publish --dry-run` 通过，且不依赖任何产品私有 crate。注意：Cargo 的 dry-run 会去 registry 解析中性依赖，故 contract 未实际上架前其余 crate 的 dry-run 必然失败——这是发布顺序门，不是代码缺陷。UI 呈现标准化（PUB-004）、四套模板（PUB-005）、crates.io 自动发布（PUB-006）留在未来佛。中性 crate 采用「潜伏者 / Lurker」品牌前缀，单向权威、不做 re-export 兼容层。

### 5.1 基础契约与中性端口解耦（3条 / 7pt）

- [ ] PUB-001.3 [kind: mechanism] [priority: P1] [points: 3] [owner: profile-store] [status: InProgress] 抽象通用 `lurker-profile-store` 与租约并发接口
  - 产物：基于 CAS 与版本租约锁的通用存储/事务接口；剥离产品私有 Age 凭据、`infiltrator-platform` 依赖与硬编码路径。
  - 进度：part 1 文件系统内核/`ProfileStoreCore`/settings/namespace；part 2 `document_path`/`profile_files`/`workspace`/`workspace_store`（`ProfileDeletionParticipant`、`ProfileSourceFacts` 注入，`EngineProfileStore` 公开 API 不变）均已迁入 `lurker-profile-store`。
  - 裁决：part 3（CAS/租约/owner-switch/reset）与 PUB-003.2 的中性 `EngineSwitchStage<S>` 是同一份抽象，禁止设计两遍；part 3 随 PUB-003.2 一并落地并由其验收覆盖。
  - 依赖：PUB-001.1、PUB-001.2。
  - 验收：单测覆盖活动租约锁竞争、并发 CAS 冲突、安全原子替换；零业务私有路径；`cargo package` 通过（dry-run 待 contract 上架）。

### 5.2 双引擎专属驱动 crate 纯净化与打包（5条 / 11pt）

- [ ] PUB-002.2 [kind: mechanism] [priority: P1] [points: 2] [owner: mihomo-config] [status: Planned] Mihomo YAML 配置 Codec 独立化
  - 产物：独立配置模块，内存 AST 解析、合法性校验、Mihomo 原生 dry-run 检查。
  - 依赖：PUB-001.1、PUB-001.2。
  - 验收：标准与畸形 YAML 校验通过并输出精确 typed DocumentValidation；`cargo publish --dry-run` 通过。
### 5.3 统一双引擎调度器与高层 Facade SDK（4条 / 11pt）

- [ ] PUB-003.1 [kind: mechanism] [priority: P1] [points: 3] [owner: lurker-engine-runtime::registry] [status: Planned] 双引擎工厂注册器 `EngineBackendProviderRegistry` 泛型化
  - 产物：动态引擎注册机制，解耦编译期硬编码；应用可注入单引擎或双引擎 adapter。
  - 依赖：PUB-001.2、PUB-002.1、PUB-002.3。
  - 验收：支持动态挂载/卸载；未注册引擎强类型 fail-closed 拒绝操作；nextest 覆盖。
- [ ] PUB-003.2 [kind: mechanism] [priority: P1] [points: 3] [owner: lurker-engine-runtime::switch] [status: Planned] 双引擎原子切换抽象为 `EngineSwitchCoordinator`
  - 产物：基于 CAS 租约与 Generation/Revision 围栏的无缝切换组件；失败时原主自动恢复。
  - 裁决：中性 `EngineSwitchStage<S>` + 持久化 wire 枚举只做一对一映射（不改 serde 名，免 schema 迁移）；旧 owner 终态之后只能 `RecoveryRequired`，禁止伪造自动恢复。
  - 依赖：PUB-003.1；PUB-001.3 需先裁决 store 是否对 selection 泛型（否则用产品本地 newtype 实现中性 journal）。
  - 验收：nextest 覆盖正常切换与注入故障回滚，证明新旧进程生命周期严格互斥且终态可查。
- [ ] PUB-003.3 [kind: mechanism] [priority: P1] [points: 3] [owner: runtime facade] [status: Planned] 高层 Facade SDK `lurker-engine-runtime`
  - 产物：`LurkerRuntimeBuilder` 式 Builder API，隐藏 Token/Generation 细节，暴露直观异步操控入口。
  - 依赖：PUB-003.2。
  - 验收：第三方 10 行内完成初始化并监听状态流；单测与文档示例验证；`cargo publish --dry-run` 通过。
- [ ] PUB-003.4 [kind: evidence] [priority: P1] [points: 2] [owner: test-support] [status: Planned] 双引擎 Conformance 跨平台验证套件
  - 产物：通用适配器一致性测试套件，规范进程状态机、配置应用、节点读写、错误回读断言。
  - 依赖：PUB-003.1～PUB-003.3。
  - 验收：覆盖 Mihomo 与 sing-box 真实与 fake 适配器；第三方适配器跑通即证明兼容。
