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
- Point 基线 `musicfrog-todo-20261018-v236`：当前现在佛 3 pt（3 条）/ 过去佛 69 pt（26 条）/ 未来佛 47 pt（19 条），联合未办结分母 119 pt；已归档交付 682 pt（263 条）；累计 801 pt，严格归档完成率 85.1%。
<!-- POINT_LEDGER:END -->

## 阶段一：代码健康度与全仓质量门禁（前置基石，已解锁全仓验证批次）

> 阶段门：CI-QUALITY-001～005 质量门禁全量通过；Kotlin 800 行预算守卫（BANDROID-044.3）已办结归档生效。

## 阶段二：底层服务与宿主边界收口（Android Owner / VPNService / Host 闭环）

## 阶段三：应用状态、生命周期与 UI 信息架构（自底向上 UI 接入）

> 阶段门：CORE-UI-002 统一验证批次（结构门 + parity + 全量 nextest 5116 项 + Clippy 0 警告）已全绿收口并办结归档。

## 阶段四：v0.42.5 发布管道与构建产物 CI 留痕（已迁入过去佛安排，受控环境阻塞待验）

## 阶段五：1.0 中性内核可发布（开源独立框架 · 潜伏者 / Lurker）

> 阶段门 1.0-M1「中性内核可发布」：`lurker-core-contract`、`lurker-core-ports`、`lurker-profile-store`、`mihomo-driver`、`singbox-driver`、`lurker-engine-runtime` 六个中性 crate 按依赖顺序（contract → ports/profile-store → engines → runtime）`cargo package` 通过；`lurker-core-contract` 首发布后，其余五个 `cargo publish --dry-run` 通过，且不依赖任何产品私有 crate。注意：Cargo 的 dry-run 会去 registry 解析中性依赖，故 contract 未实际上架前其余 crate 的 dry-run 必然失败——这是发布顺序门，不是代码缺陷。UI 呈现标准化（PUB-004）、四套模板（PUB-005）、crates.io 自动发布（PUB-006）留在未来佛。中性 crate 采用「潜伏者 / Lurker」品牌前缀，单向权威、不做 re-export 兼容层。

### 5.1 基础契约与中性端口解耦（已全部迁入过去佛）

### 5.2 双引擎专属驱动 crate 纯净化与打包（已全部迁入过去佛）

### 5.3 统一双引擎调度器与高层 Facade SDK（已全部迁入过去佛）

### 5.4 收口八问缺口（GAP，已全部办结归档）

## 阶段六：0.44 UI 管道标准化与开源合规基线（全部任务已办结归档）

## 阶段七：0.5x 多 UI 模板化与工程脚手架（全部任务已办结归档）

## 阶段八：0.6x 开源发布生态与开发者交付文档（全部任务已办结归档）
