# MusicFrog Agent 总纲

> 层级：L0

## 阅读顺序

先读 [文档地图](docs/README.md)，再读受影响包的 `README.md`、其链接文档与源码。产品说明见 [README](README.md)，使用说明见 [contract 文档](crates/infiltrator-contract/docs/README.md)。

## 强制编码规则（所有 Agent）

多阶段流程一律用显式 `enum`/sealed 状态表达互斥状态，并以 typed transition 和穷尽匹配限定合法迁移。**禁止用多个 `bool`、`Option`/null 或标志位组合藏出隐式状态机；`bool` 只表示真实二值事实。**修改后必须通过 typed-state 质量门禁；细则见 [代码质量底线](docs/CODE_QUALITY_BASELINE.md)。

Core-first 顺序同样是硬门：先完成所有可独立于 UI 推进的底层 Core TODO（domain/contract/ports、application 与 fake/nextest、Mihomo/sing-box adapter 与 conformance、Android owner/IPC/VPNService）。若剩余 Core 收口唯一被现有 UI 消费者的旧调用阻塞，只允许把迁移到新 typed API 的边界接线作为该 Core 切片的一部分一次完成；不改页面布局、视觉或新增 UI 功能。该迁移批次完成后再统一构建/测试 UI，并继续 Core 收口；没有可独立推进的必要 Core 工作后，才进入各 UI 的专属整改。Core 切片用 mock/nextest 验收并提交。

### 架构未决与自底向上逐层裁决铁律（所有 Agent、Core 与 UI 强制执行）

**严禁在架构未决时做上层修修补补！**凡遇到架构没有裁决清楚之处（包括但不限于：所有权 Owner 模糊、领域数据模型缺失、显式状态机未定义、读写与读回契约缺失、跨端语义分歧、生命周期与阶段二义性等），**绝对严禁**在 UI 层（Iced、Bevy、Slint、Android Compose）、Presentation 呈现层或 Host 组装层（Desktop Composition / Android Driver）就地打补丁、加容错旁路、增加临时布尔/状态标志位、自行推导业务事实（如通过本地比对 ID 猜测是否 active），或留下 `// TODO: 待底层支持` 的半吊子实现。

遇到任何架构未决或底层能力缺失，必须**立即暂停上层开发**，回溯到底层按严格顺序逐步裁决并逐层收口，所有 Core、所有 UI 全员遵照执行：
1. **底层先裁决（Domain / Contract / Ports / Owner）**：在权威底层明确唯一所有权（Single Owner）、不可变业务规则、显式 typed 状态机枚举（杜绝隐式标志位组合）、typed 意图命令与权威读回（Readback）契约。
2. **Core 内核与适配层收口（Core Application / Ports / Adapters）**：在 application 与各引擎适配器（Mihomo/sing-box adapter、Android VPNService/IPC）中完成真实业务实现与端口闭环；必须编写单元测试与 Mock/Fake 并在 nextest 中验收通过。
3. **跨端呈现与宿主装配（Presentation / Host Composition）**：在底层稳定后，更新纯呈现投影与宿主依赖注入，不保留私有业务逻辑与中间状态。
4. **上层 UI 消费端接入（UI Surfaces: Iced / Bevy / Slint / Android）**：所有 UI 表面严格作为纯粹的被动消费端接入，仅消费底层已裁决的 typed 快照并向底层发送 typed 意图命令；禁止在 UI 表面沉淀任何第二个业务真源或私有推导逻辑。

**TODO 执行纪律**：执行 `TODO.md` / `todos/` 看板的任何任务时，若发现所依赖的底层架构未决或存在缺陷，严禁为了“办结当前条目”或“让界面先跑通”而在上层做临时修补！必须先建立或认领底层任务，自底向上彻底解决底层后，再推进上层；不能依赖人工审核反复提醒，违者一律视为架构违规打回重做。

### 批次化开发与组团集中验收铁律（高吞吐工程效率法则）

**必须坚持“改动一批一批做、验收一组一组办”，杜绝零敲碎打和盲目大杂烩！**

1. **改动成批推进（Batching by Cohesive Slice）**：
   - 每次开发切片必须限定在同一个清晰边界内：聚焦于同一层级（如纯 Contract 契约组、Core Application 适配组、Presentation 投影组或特定 UI 消费端组），或者围绕一组强耦合的原子条目（通常 ≤5 pt）闭环推进；
   - **严禁跨层随意散布修改**：严禁在一个切片中同时东改底层、西改前端、顺手碰无关模块，导致工作区脏乱、心智上下文分散且无法独立提交与回滚；
   - **原子闭环交付**：同批改动必须在切片内一次性完成新旧入口迁移、旧废弃符号清理与相关单元测试补齐，同次提交。
2. **验收分组分级收口（Grouped Verification Batches）**：
   - **过程期快速定向反馈**：在切片开发过程中，严禁每改动几个文件就盲目执行全量构建或全仓重型测试！只允许运行当前修改包的定向测试（如 `cargo nextest run -p <crate>`）与受影响文件的局部守卫，在数秒内获得反馈，保障极速开发节奏；
   - **产出期组团集中验收**：当过去佛中积攒了同组或同阶段的 `Integrated` 待验证条目时，启动**统一验证批次（Unified Verification Batch）**，一次性跑通全套集成测试、Clippy（`--all-targets -D warnings`）、结构门与契约守卫；
   - **批量解锁与办结**：统一验证批次通过后，**一次性批量解锁并办结一组任务**，在台账中批量登记 archived points。通过集中验收消除重复运行全仓测试的巨大开销，成倍提升工程吞吐量。

## 工作规则

- **分支管理：** 本仓库只在 `main` 开发、提交和推送。开始操作先确认当前分支是 `main`；禁止创建或切换到其它分支、禁止另建 worktree。清理分支前先核对独有提交和未保存改动，任何已有工作都必须先安全归并到 `main`，不得用 reset 或删除分支丢弃。
- **批次收口八问（强制）：** 每次提交批次前必须按 [收口八问](docs/CLOSURE_QUESTIONS.md) 逐条回答并更新状态表；缺项、空答案或证据等级缺失一律视为未收口，禁止提交。只有 `L`/`C`/`D` 证据才能把结论从「部分/缺失」改为「完整」，`scripts/quality/closure-questions-guard.py` 会拒绝不合格的问答。
- **任务真源台账：** 任务真源是 `TODO.md`（三佛导航、points 台账）与 `todos/` 看板：`CODE_TODO.md` 现在佛（全部可推未办结工作：待编码/待实现/待补证）、`CODE_TODO_PAST.md` 过去佛（已交付待验证，含单元、集成与实机取证要求）、`CODE_TODO_BLOCKED.md` 斗战胜佛（只剩外部环境才能解锁的条目）。全仓任务纯粹收敛于这三块看板，不设体外孤岛看板。开发会话先按 `TODO.md`「三佛工作流程」的 Agent 执行循环行事：定阶段 → 现在佛按 P0→P1、InProgress→Ready→Planned 选原子条目 → 实现完成同次迁入过去佛（`Integrated`）→ 验证收口通过后办结退出并在台账登记 archived points；三板之间如何流转按该节流转总表执行，不得在旧描述里另记第二份状态。
- **可推性与截图认证双硬门（2026-10-19 起强制）：** 进入现在佛/过去佛的条目必须能指出下一个可在本机执行的动作；只剩真机、tag CI、用户 secrets、上架凭证或异构目标机才能收口的条目整条留在斗战胜佛，不得占用可推看板的注意力。每条业务能力在每个桌面 UI 必须同时具备 contract 锚点、scenario 锚点与 standard/compact 双视口真实像素回执，Android 三 flavor 另需实际交互截图回执；判定入口唯一是 `scripts/parity/resolve_surface_evidence.py` 与 `scripts/parity/visual_receipts.py`，缺任一项只能写「部分」。
- **隐式状态机是质量红线（2026-10-20 起强制）：** 多阶段流程必须是一个 typed enum + typed transition + 穷尽匹配；禁止用多个 `bool`、`Option`/null 或标志位组合藏出阶段表，`bool` 只表示真实二值事实。`scripts/quality/typed-state-guard.py` 是有界词法读器（看不到跨字段的阶段表），因此另有语义审计 `scripts/quality/implicit-state-audit.py` 配合判决台账 `scripts/quality/implicit-state-verdicts.json`：**每个候选都必须有人工判决**（`legitimate` 独立事实 / `fixed` 已改写 / `open` 仍待改），未判决或 `open` 即门禁失败；改写时发现一处改一处，不得攒着。
- **并行工流隔离纪律（2026-10-20 起强制）：** 并行开发必须按**文件所有权**切分，同一文件同一时刻只有一个 owner，跨界修改先与 owner 对齐；**编译不通过的半成品不得放进会被 `cargo nextest --workspace`、`check-structure.sh` 或 capture 构建扫到的路径**（crate 的 `src/`、`tests/`、脚本目录），WIP 先放草稿或 `.evidence/wip/`，收尾时一次性搬入；**开发期验证只跑受影响包**（`cargo nextest run -p <crate>` 与对应 guard），`cargo nextest --workspace` 与全量结构门统一放到收尾批次**只跑一次**，禁止让单个 crate 的 WIP 反复触发全仓重编重挂。
- **环境故障先归属再动手（2026-10-20 起强制）：** 共享 `/tmp` 配额、cargo lane 争用、合成器起栈失败、真机缺席等外部故障，先判定归属（本切片缺陷 / 他工流 WIP / 纯环境），只修本切片能修的那一类；不得让环境噪声反复打断全仓验证，也不得把环境故障写成代码缺陷。已知缓解：capture 脚本把链接器临时文件放在构建输出旁（`CAPTURE_BUILD_TMP`），共享 `/tmp` 只留合成器 socket 短路径。
- **执行标准（2026-10-20 起强制，所有切片适用）：** ① **底层结构不够就扩 owner 结构**——禁止再塞 `bool`/`Option` 表达阶段或所有权，缺结构就补结构；② **组合优于继承**——跨域行为用组合与显式依赖表达，不用继承层次；③ **该分开的业务分开**——field 归 field、表现归表现、事务归 owner；④ **每个事实只有一个真源**，不留第二份缓存或影子副本；⑤ **非身份改动不碰 wire/快照/语料**，身份改动集中一次做完，不拆成多批半改。判据：每个切片结束时，状态字段应有唯一真源、阶段应有 typed 表达、wire 与快照只随身份变更一次性变更。
- 文档引用遵守单向层级：L0（本文件）→ L1（根 README、TESTING、第三方声明与文档地图）→ L2（跨 crate/surface 公共契约，含共享 UI contract）→ L3（具体 crate、Android、资源和工具实现细则）→ L4（本地 TODO 叶台账）。只允许同级或向下引用，包含 Markdown 链接及正文中的文档路径；实现细则和叶台账不得回引上位契约，公共入口负责向下导航。门禁按文件范围分配层级，校验头部声明与范围一致，并拒绝未登记 Markdown，不能靠改层级声明绕过反向引用检查。
- 新功能先查 [owner 表](docs/FUNCTIONAL_MAP.md)，按 [架构边界](docs/ARCHITECTURE.md) 修改；每项事实只留一个权威来源。遇到架构或契约未决，必须遵循底层先决、自底向上推进，严禁在上层修补绕行。
- 每个新功能切片都先盘点会被新语义取代的旧入口和旧描述；先迁完全部生产消费者，再在同一切片删除旧公开接口、命令、port、DTO、FFI/wire、兼容分支、重复状态表达与过期产品文案。以仓库搜索确认受影响范围内旧符号和旧说法已清零，并以新契约行为测试取代旧路径测试。不得因为旧入口仍能编译、暂时有调用点或“以后再清理”而保留新旧双轨、fallback、重复描述或仅换名入口。只有读取历史持久化格式时可保留旧格式，并且必须由显式版本迁移器隔离。Core 后端的具体约束见 [Core 后端契约](docs/CORE_BACKENDS.md)。
- UI 与平台规则分别见 [共享 UI 契约](crates/infiltrator-contract/docs/README.md)、[Bevy 文档](crates/infiltrator-bevy-ui/docs/README.md) 和 [Android 文档](android/docs/README.md)。**Bevy UI 新代码必须先读并遵守 [Bevy 0.20 UI 能力规范](crates/infiltrator-bevy-ui/docs/BEVY_020_UI_CAPABILITIES.md)**：rem/em 尺寸随 `RemSize` 缩放、`ThemeContext` 两段式主题、官方 headless 控件优先（TabList/Dialog/Popover/Menu）、`FixedNode` 悬浮层、`InlineBox` 行内内容、`bsn_list!`/`Ready`/`despawn_all`。
- 不支持标为 `unsupported`；没有对应证据不写“已验证”。代码规则与验证入口见 [质量底线](docs/CODE_QUALITY_BASELINE.md) 和 [TESTING.md](TESTING.md)。

- 修改 Iced/Bevy 通用组件前，先读 [跨产品共享包契约](docs/SHARED_UI_PACKAGES.md)：
  优先扩展共享 owner，保留必要差异，检查其他消费者和版本兼容性。
