# MusicFrog Infiltrator 本地工作台账

> 层级：L4

这是项目内部执行台账。公开文档只描述当前边界；这里记录任务、依赖、未决问题和验收顺序。**2026-10-11 起采用三佛体系（学习 ZhuGeTower 模式），2026-10-19 起收敛为三板：现在佛（全部可推工作）、过去佛（已交付待验证）、斗战胜佛（只等外部环境）；原未来佛全部并入现在佛并退役。本页只导航与记账；任务正文、执行态与验收在 `todos/` 看板。**

## 看板导航

| 看板 | 文件 | 用途 |
| --- | --- | --- |
| 现在佛 | [todos/CODE_TODO.md](todos/CODE_TODO.md) | 全部可推未办结工作：待编码、待实现、待补证（0.60 → 1.0 收口路径） |
| 过去佛 | [todos/CODE_TODO_PAST.md](todos/CODE_TODO_PAST.md) | 已交付实现，等待统一集成验证批次或实机取证收口（待验证） |
| 斗战胜佛 | [todos/CODE_TODO_BLOCKED.md](todos/CODE_TODO_BLOCKED.md) | 剩余闭环动作只能靠外部环境（真机/tag CI/secrets/上架凭证/异构目标机）解锁的条目（环境阻塞） |

## 三佛工作流程（2026-10-11 起生效，2026-10-15 纯粹化收敛，2026-10-19 增设斗战胜佛并退役未来佛）

三佛是同一工程队列的三种未办结形态，不是三套范围或三份完成台账；位置描述用途，状态描述执行态，二者不能相互推断。**全仓任务纯粹收敛于三板，不设任何体外孤岛看板（模拟器与真机证据要求直接内聚在条目本体的“待验证”中）。**`Verified` 只是办结过渡态，不驻留任何看板；真正验收完成的工作退出看板并在本页登记 archived points。

**可推性是看板的准入条件：**现在佛与过去佛只放「下一个可执行动作能在当前环境完成」的条目——统一 nextest/clippy/guard 批次、本地 Gradle 与模拟器 M1、本地真实引擎二进制 smoke、`cargo package`/dry-run、纯代码实现。剩余闭环动作全部依赖外部环境的条目整条迁入斗战胜佛，两块可推看板不得预占，也不得把斗战胜佛条目当作可推进任务选取。

| 形态 | 准入与必需事实 | 下一出口 |
| --- | --- | --- |
| 现在佛 | 可推的必要产品能力与直接前置，以及可推的增强项；明确产物、依赖、验收、计划与证据要求 | 实现包完成只剩验证时迁入过去佛；同片验证完成则直接办结退出 |
| 过去佛 | 已交付实现包；列清待验证范围（含统一测试批次与实机/模拟器证据）、证据身份与未证明层 | 验证收口通过后退出并登记 archived points；发现实现缺口回流现在佛 |
| 斗战胜佛 | 条目的下一个可执行动作只能靠外部环境：ARM64 真机、受控 tag CI、用户 secrets/keystore、crates.io 上架凭证、异构目标机与被禁平台 target | 环境到位后同次迁回过去佛/现在佛；办结后登记 archived points |

### Agent 执行循环（每次开发会话按此执行：先做什么、再做什么）

1. **定范围：**当前窗口覆盖用户指定的全部必要产品工作：engine-neutral Core 与 Mihomo/sing-box 双适配；Kotlin VPNService/权限与 Android APK 链路；Bevy/Slint/Compose 页面和最大化共享；桌面各页与服务管理；Windows/Linux/Android 目标构建；五大平台 × 三大 UI 的 360° 覆盖与逐业务截图认证。按 task 依赖推进，不得因为前置或阶段门没过就把必要功能降级或搁置。
2. **选条目：**读 [现在佛](todos/CODE_TODO.md)，先做最高杠杆动作——过去佛攒有 `Integrated` 待验证批次时，**优先执行统一验证批次（一次组团解锁多条办结，成倍提高效率）**；否则在当前阶段内按 P0 → P1 → P2、状态 `InProgress`（续作）→ `Ready` → `Planned`（前置已满足）选一组关联切片推进；**斗战胜佛条目一律跳过，不空等，只在外部环境到位时迁回可推看板。**
3. **读依赖：**只读所选条目及其依赖链上的条目与证据，不全文加载看板。
4. **实现：**改写原条、不追加过程叙事；范围变大先拆分再动工。必要功能、平台接线和基本构建缺口先进现在佛；可推的体验/性能/发行便利/工具增强同样进现在佛（按里程碑与 priority 排序），只有「下一个动作只能等外部环境」的条目才进斗战胜佛。禁止把必要产品能力包装成“以后优化”。**遇到架构未决与契约未清之处，严格遵照自底向上铁律：底层先决、上层后跟，严禁在上层修修补补留下一堆隐患；若前置底层未裁决，必须先立项并彻底收口底层。改动必须一批一批做，严禁跨层随意东拼西凑散布修改。**
5. **交接：**实现 + 定向单测写完 → 把该条**同次**移入过去佛，状态改 `Integrated`，写清待验证范围（含统一批次与模拟器/真机设备要求）与证据身份；纯证据/文档条（1pt）若验证同片完成则直接办结。**若该条剩余闭环动作只剩外部环境（真机/tag CI/secrets/上架凭证/异构目标机），同次整条移入斗战胜佛并写明阻塞方与解锁条件，不得留在可推看板空等。**
6. **验证收口：**按该条验收选择最低充分证据层（统一 nextest 批次、Clippy `--all-targets -D warnings`、旧符号/文案搜索、结构门、guard、真实进程 smoke、模拟器/真机取证）。**验收一组一组做，通过统一批次集中跑全量门禁，一次性批量解锁过去佛中的关联条目。**前置未满足保持 `Integrated` 并注明依赖，不得宣称通过；仅剩外部环境门槛的条目按第 5 条移交斗战胜佛。
7. **办结：**全部验收通过 → 从过去佛删除该条，在本页「已归档交付原子条目」登记（ID、pt、证据身份：run ID / guard 回执），同步更新现在佛 §0 point ledger 与本页交付率；文档状态跟随证据更新。
8. **证据等级（强制）：**任何「通过 / 完成 / 就绪」必须标注证据等级：`L` = 本地（cargo/nextest/guard，需给命令或 run 标识）、`C` = CI（workflow run ID）、`D` = 真机或目标产物（设备/安装包回执）。没有 `C`/`D` 的条目只能写「本地通过，CI/真机未验证」，禁止简写成「已就绪」。平台能力以平台矩阵文档（由 L1 入口导航）为唯一事实源，本页与任何汇报不得高于它。

### 流转总表（状态 × 触发 × 操作）

| 从 | 触发 | 到 | 操作 |
| --- | --- | --- | --- |
| 现在佛 | 实现 + 行为测试写完 | 过去佛 | 同次迁移；状态 → `Integrated`；补写待验证范围（含设备证据）与证据身份 |
| 现在佛 | 纯证据/文档条且验证同片完成 | （退出） | 从看板删除；本页登记 archived |
| 过去佛 | 验证全部通过 | （退出） | 删除并登记 archived（附 run ID/guard 回执）；更新 ledger 与交付率 |
| 过去佛 | 验证发现本条实现缺口 | 现在佛 | 同次迁回；状态 → `InProgress`；保留原证据身份，不复制任务 |
| 过去佛 | 缺口属于另一 task | 停留 | 状态 → `Blocked`，注明依赖 task ID；由对应 owner 修复后复验 |
| 现在佛/过去佛 | 剩余闭环动作只剩外部环境（真机/tag CI/secrets/上架凭证/异构目标机） | 斗战胜佛 | 同次整条迁入；写明阻塞方、解锁条件与本板停留期间的可执行动作；状态 → `Blocked` |
| 斗战胜佛 | 外部环境到位 | 过去佛/现在佛 | 同次整条迁回；恢复原状态与证据身份；本板 §七 登记迁出记录 |
| 斗战胜佛 | 迁出后验证通过 | （退出） | 删除并登记 archived；更新 ledger 与交付率 |
| 任何看板 | 上游决定取消/被新语义取代 | （退出） | 删除；本页台账记一行注销原因与去向 |

### 恒定规则

1. **原子硬门**：一条 task 只交付一个有界业务结果，必须能独立说明"做出什么、依赖什么、怎样关闭"；禁止"完善/闭环/全部能力"式抽象范围；header ≤240 字符、详情 ≤6 行、整块 ≤1200 字符；不能明确判定关闭条件的条目不得进入 Ready/InProgress。
2. **points 合同**：`1` 局部证据/文档/配置；`2` 单 owner focused 包；`3` 跨 owner 纵切或普通机制包；`5` 产品采用/终局门；超过 5pt 先拆。联合分母 = 三板全部未办结 task 的 points，只在现在佛 §0 声明一次。
3. **严格交付率**只计 archived points；执行态报告对 Planned/Blocked/Ready 计 0%、InProgress 计 50%、Integrated 计 75%，不以执行态进度代替验收办结。
4. **迁移纪律**：看板间移动必须同次移出原文件、移入目标文件；保持稳定 task ID、owner、依赖、验收与 points；位置变化不重置执行态，也不构成办结。
5. **证据规则**（沿用统一阶段门）：Rust 测试唯一执行器 `cargo nextest`；单测/模拟、静态 guard、Android 编译、APK 安装、模拟器行为、ARM64 真机流量是不同证据层，不能彼此外推；缺物理设备或缺外部环境的任务整条迁入[斗战胜佛](todos/CODE_TODO_BLOCKED.md)并写明阻塞方与解锁条件，不用源码推断补齐，也不占用可推看板的注意力。
6. **等价映射**（避免重复条目）：`BANDROID-001.2 ≡ 040.5`；`BANDROID-004.2 ≡ 040.3`；`BANDROID-036.1` 的语义回归与 `042.3/042.4` 同批验证。
7. **自底向上逐层裁决铁律**：遇到架构、模型、契约或 Owner 权责未决时，严禁在 UI 层（Iced/Bevy/Slint/Android）、呈现层或组装层就地修补（禁止添加临时标志位、禁止推导业务事实、禁止空实现占位）。必须严格按「Domain/Contract/Ports/Owner → Core Application/Adapters → Presentation/Composition → UI Surfaces」四阶顺序自底向上逐步解决，所有 Core 与所有 UI 遵照执行。
8. **批次开发与组团集中验收铁律（高吞吐效率准则）**：改动必须一批一批做（同一层级、同一内聚切片），严禁跨层随意散布修改；验收必须一组一组做，开发期用定向轻量测试获得秒级反馈，收工期通过统一验证批次集中跑全量门禁并批量解锁办结，成倍提高工程吞吐效率。
9. **可推性准入（2026-10-19 起强制）**：条目进入现在佛/过去佛前必须能指出下一个可在本机执行的动作；只有「等外部环境」剩余动作的条目一律留在[斗战胜佛](todos/CODE_TODO_BLOCKED.md)。斗战胜佛条目不得被选取为开发任务，也不得以「等待」名义延长其在可推看板的停留。
10. **截图认证硬门（2026-10-19 起强制）**：每条业务能力在每个桌面 UI 必须同时具备 contract 锚点、scenario 锚点与 standard/compact 双视口真实像素回执，Android 三 flavor 另需实际交互截图回执；判定入口唯一是 `scripts/parity/resolve_surface_evidence.py` 与 `scripts/parity/visual_receipts.py`。缺任一项只能写「部分」，禁止用静态页面截图或旧 APK 回执冒充平权通过。
11. **并行工流隔离纪律（2026-10-20 起强制）**：并行开发按**文件所有权**切分，同一文件同一时刻只有一个 owner；**编译不通过的半成品不得放进会被 `cargo nextest --workspace`、结构门或 capture 构建扫到的路径**，WIP 先放草稿或 `.evidence/wip/`，收尾时一次性搬入；**开发期只跑受影响包**，全仓 `cargo nextest --workspace` 与全量结构门统一放到收尾批次**只跑一次**，禁止单个 crate 的 WIP 反复触发全仓重编重挂。
12. **环境故障先归属再动手（2026-10-20 起强制）**：共享 `/tmp` 配额、cargo lane 争用、合成器起栈失败、真机缺席等外部故障，先判定归属（本切片缺陷 / 他工流 WIP / 纯环境）再决定动作，只修本切片能修的那一类；不得让环境噪声反复打断全仓验证，也不得把环境故障写成代码缺陷。已知缓解：capture 脚本把链接器临时文件放在构建输出旁（`CAPTURE_BUILD_TMP`），共享 `/tmp` 只留合成器 socket 短路径。
13. **执行标准（2026-10-20 起强制，所有切片适用）：** ① 底层结构不够就扩 owner 结构，禁止再塞 `bool`/`Option` 表达阶段或所有权；② 组合优于继承，跨域行为用组合与显式依赖表达；③ 该分开的业务分开——field 归 field、表现归表现、事务归 owner；④ 每个事实只有一个真源，不留第二份缓存或影子副本；⑤ 非身份改动不碰 wire/快照/语料，身份改动集中一次做完。判据：切片结束时状态字段有唯一真源、阶段有 typed 表达、wire 与快照只随身份变更一次性变更。
13. **cargo 并行预算与分组收口（2026-10-20 起强制，仿 ZhuGeTower 组批节奏）：** 构建资源全局共享（本机两条 cargo lane），并行流的 cargo 调用必须错峰：同一时刻只允许一条流跑 cargo，其余流只写源码与测试；**重构按组推进、一组一收口**——同一主题的重构（同一 owner 结构、同一族类型、同一契约面）作为一组同时落地，由收口方统一跑一次该组验证（一次 `--workspace` nextest + 一次结构门 + 该组专属 guard），禁止每条改动各跑一遍全仓；收口顺序：各流先交付源码，最后一条流统一构建、统一验证、统一提交。

## Points 与完成度台账（2026-10-20，基线 `musicfrog-todo-20261020-v251`）

| 看板 | task | points |
| --- | ---: | ---: |
| 现在佛 | 13 | 35 |
| 过去佛 | 38 | 87 |
| 斗战胜佛 | 29 | 72 |
| **未办结联合分母** | **80** | **194** |

- **严格交付率：682 / 876 = 77.9%**（archived 682pt / archived 682 + 未办结 194pt）。执行态参考仍以看板状态计算，不替代验收。
- 交付率分母变化的唯一原因：上一基线（`musicfrog-todo-20261018-v236`）的看板计数已过期（记为现在佛 3/3pt、过去佛 26/69pt、未来佛 19/47pt，合计 119pt），本次按看板实际条目重算为 131pt；随后未来佛 16 条 / 36pt 全部并入现在佛，新立 6 条 / 17pt 截图认证与平台覆盖条目（+23pt），按 2026-10-19 的 Slint 表面审计实测补立 7 条 / 16pt 第三 UI 补齐条目（+16pt），本批次再补立 `SLINT-001.8`/`001.9` 3pt 并把已交付的 `PUB-006.1`、`PARITY-001.1` 4pt 迁入过去佛（净 +3pt）；2026-10-20 按 Bevy UI 强制化实测审计补立 3 条 / 8pt（`BEVYCAP-001.1` rem 守卫、`BEVYCAP-001.2` 存量 px 迁移、`BEVYCAP-002.1` ⑦ 小件收敛），未办结条目集合净增 56pt；2026-10-20 再补立 `BEVYCAP-003.1` 2pt（Bevy 场景律法唯一 owner 收敛：仓库自有 `bevy_bsn_guard.py` 与共享包场景门禁当前双份执行同一条 BSN 律法），未办结条目集合净增 58pt；2026-10-20 再补立 `WIDGET-001.1` 3pt（rem 迁移实测暴露的重复事实：开关轨道与旋钮几何在四个页面各写一份常量，widgets 缺 switch 控件真源），未办结条目集合净增 61pt；同日再补立 `BEVYCAP-004.1` 3pt（能力规范②④的浮层存量缺口：八个浮层 owner 补 `ThemeContext(SurfaceLevel::Floating)` 与 `FixedNode`，并加机器规则 `BEVY-BSN-011`）、`BEVYCAP-004.2` 3pt（modal 层迁官方 `Dialog`/`ModalDialog`/`RequestClose`，bool 对改 typed `ModalStage`）、`TOKEN-001.2` 2pt（契约圆角阶梯「居中对话框」角色重叠待裁决）；前三条已交付并迁入过去佛，未办结条目集合净增 69pt。
- 2026-10-20 本批迁移：Android 静态质量门 4 条（`BANDROID-044.1`／`044.2`／`044.4`／`044.5`，9 pt）由现在佛迁入过去佛（状态 `Integrated`，待统一批次与 CI runner 回执）；联合分母不变，严格交付率仍只计 archived points，故仍为 682 / 876 = 77.9%。
- 2026-10-20 本批再迁移：隐式状态机全量审计（`STATE-001.1`，3 pt）与公共 API 文档质量门（`PUB-006.6`，2 pt）由现在佛迁入过去佛（状态 `Integrated`，待统一批次复跑审计器与 docs.rs 等价门禁）；联合分母不变，严格交付率仍只计 archived points，故仍为 682 / 876 = 77.9%。
- 2026-10-20 本批再迁移：开源生态四条（`PUB-006.7` 无头沙盒与混沌容错、`PUB-006.9` 最小化用例展厅、`PUB-006.10` 交叉构建与 SemVer 兼容守卫、`PUB-006.11` 开发者旅程演练，7 pt）由现在佛迁入过去佛（状态 `Integrated`）；联合分母不变，严格交付率仍只计 archived points，故仍为 682 / 876 = 77.9%。
- 2026-10-20 本批再迁移：平台证据与安装包两条（`PLATFORM-001.1` 五大平台证据矩阵、`APPID-001.3` Linux deb/AppImage 产物，5 pt）由现在佛迁入过去佛（状态 `Integrated`）；联合分母不变，严格交付率仍只计 archived points，故仍为 682 / 876 = 77.9%。
- 2026-10-20 本批再迁移：地球增强与其报告两条（`BANDROID-016.2` 按需地球与流量弧线、`BANDROID-016.3` 帧/内存报告，4 pt）由现在佛迁入过去佛（状态 `Integrated`，宿主接线与报告文件仍待补）；共享控件真源 `WIDGET-001.1` 的证据补记为「控件是一个整体、页面不再推导偏移」；联合分母不变，严格交付率仍只计 archived points，故仍为 682 / 876 = 77.9%。
- 2026-10-20 本批再迁移：四个 pending 行为锚点补齐（`PARITY-001.3`，2 pt）由现在佛迁入过去佛（状态 `Integrated`）；联合分母不变，严格交付率仍只计 archived points，故仍为 682 / 876 = 77.9%。
- 归档口径：累计 263 条 / 682 pt；明细表内可查 262 条 / 681 pt（本次补登 `PUB-004.2`、`PUB-004.3`、`PUB-006.3` 三条 6pt），与累计口径的 1 条 / 1pt 差额是 v143 继承的历史差额，旧任务 ID 与证据无法还原，未猜填。
- 主工作流口径：59 个父工作流（Android 主线及身份/CI 40、owner/VPN/Kotlin 质量 5、Engine-neutral Core 10、Slint 第三 UI 1、桌面共享页面/服务管理 1、跨 UI typed operation states 1、开源独立框架 1.0 中性内核 1）全部未完整关闭（0/59）。父工作流只有全部原子条目办结才能关闭；本页不再逐条罗列，执行态以看板为准。
- 计数边界：iOS 原生 UI / NetworkExtension 未登记为活动任务，Apple 构建按当前决定暂缓；Bevy 概览 `SHOT-OV-01..08` 是 BANDROID 页面任务的验收分解（仅 P0 的 OV-07 在现在佛建条），阶段门、历史回执、旧 ID 目录与跨端队列映射是说明/记录，不另计工作项。

## 版本迭代路线图（0.42.5 → 1.0）

> 每档只收纳能独立关闭的原子条目；上一档出口标准未达成不进入下一档。`0.4x` 系列负责通用底层与既有三大桌面/移动 UI 的单向接入与模型去重；`0.5x` 系列负责对外独立模板与脚手架；`0.6x` 系列负责开源合规发布、自动化 CI 流水线、质量门禁与开发者生态；`0.7x` 系列负责质量红线守卫与桌面多端安装包；`0.8x` 系列负责正式发布签名与 ARM64 真实设备取证；`0.9x` 系列负责视觉体验打磨、性能基准与 1.0-RC 全平台终局收敛；设备与外部环境依赖项不阻塞代码版本节奏。
>
> **当前档位：0.60（`PUB-006.1` Crates.io 自动化发布 CI）实现已交付并迁入过去佛，等待受控 `v*` tag 的真实 workflow 与 crates.io 上架回执（顺序上架腿在斗战胜佛）。**0.43～0.54 与 0.61、0.62 已全部办结归档；下表「归属条目」列登记每档条目的**当前所在看板**（斗战胜佛 = 只等外部环境，代码侧已闭环）。下一批可自主推进的最高杠杆条目是 B 主线截图认证（`PARITY-001.2`～`001.4`）与 Slint 第三 UI 补齐（`SLINT-001.1`～`001.9`）。

| 里程碑 | 主题 | 归属条目 | 出口标准 |
| --- | --- | --- | --- |
| **v0.42.5** | 发布管道与构建产物 CI 留痕（环境阻塞） | 斗战胜佛 `BANDROID-018.2` `019.2` `038.2` `020.1` `024.1` | 受控 tag release 工作流回执与各 APK SHA 检查回执留存；条目整体停在斗战胜佛等受控 tag，不阻塞代码主线 |
| **0.43** | 潜伏者 Lurker 1.0-M1 中立内核与驱动解耦 | 过去佛 `PUB-002.1`～`002.5`；斗战胜佛 `PUB-001.3` `PUB-003.2` `PUB-003.3`；已归档 `PUB-001.1` `PUB-001.2` `PUB-003.1` `PUB-003.4` | 七个中性 crate（contract/ports/profile-store/mihomo-driver/singbox-driver/engine-runtime/ui-presentation）各自 `cargo package` 通过，零产品私有依赖；统一验证批次全绿 |
| **0.44** | 单向通信管道协议与通用呈现内核 | **已全部办结归档**（`PUB-004.1` `PUB-004.2`） | 固化 `SurfaceReader`（只读快照流）与 `CommandSink`（强类型意图通道）；通用呈现内核 `lurker-ui-presentation` 闭环；杜绝 UI 直调底层副作用 |
| **0.45** | Iced 桌面端纯化与统一管道全面接入 | **已全部办结归档**（`PUB-004.3` Iced 切片） | Iced Elm 架构 (Update/View) 全量接入 `SurfaceReader`/`CommandSink`；剥离私有推导；GUI 与 headless 测试全绿 |
| **0.46** | Bevy 桌面端纯化与 ECS 管道调度收口 | **已全部办结归档**（`PUB-004.3` Bevy 切片） | Bevy ECS System 桥接单向管道；严格落实 Bevy 0.20 UI 规范（RemSize/两段式主题/官方 headless 控件）；无头测试 0 告警 |
| **0.47** | Slint 桌面端纯化与三大 UI 功能对齐 | **已办结归档**（`PUB-004.3` Slint 切片）；现在佛 `SLINT-001.1`～`001.9`、`GAP-4.1` | Slint 回调层全量重构为 `CommandSink`；属性绑定直连 `SurfaceReader`；六大页面达成 100% 功能 Parity；**新增出口**：Slint 补齐 dns/editor/filter/mixin/sync/app_routing/doctor 七类缺失页面、建立 headless 行为台并消除形状级测试的第二业务真源 |
| **0.48** | 三大 UI 呈现模型统一去重与联合总验收 | 现在佛 `GAP-7.1` | Iced/Bevy/Slint 共享 presentation 模型，彻底消灭三大前端各自重复维护的同义状态机；桌面三大表面统一 Parity 守卫通过 |
| **0.49** | Android 移动端管道接入与 0.4x 全平台大圆满 | **已全部办结归档**（`PUB-004.3` Android 切片、`PUB-006.3`） | Android Compose 接入单向管道；JNI/Binder typed 适配层闭环；根 License、per-crate 元数据与独立版本拓扑对齐；0.4x 全面封盘 |
| **0.50** | Slint 桌面轻量独立模板工程 | **已全部办结归档**（`PUB-005.1`） | 独立的 Slint 模板工程 `templates/slint` 直接 `cargo run` 成功，内存占用超低，零产品私有依赖 |
| **0.51** | Iced 桌面响应式独立模板工程 | **已全部办结归档**（`PUB-005.2`） | 独立的 Iced 模板工程 `templates/iced` 跨平台编译通过，支持 Elm 单向意图通道与明暗主题切换 |
| **0.52** | Bevy 0.20 游戏级渲染/ECS 独立模板工程 | **已全部办结归档**（`PUB-005.3`） | 独立的 Bevy 模板工程 `templates/bevy` 在无头模式排帧稳定，ECS 调度与内核状态流无锁桥接 |
| **0.53** | Android 双引擎 VPN 移动端独立模板工程 | **已全部办结归档**（`PUB-005.4`） | 独立的 Android 模板工程 `templates/android` 满足 Android 规范，Compose UI + VpnService 授权前台服务闭环 |
| **0.54** | `cargo-generate` 统一工程脚手架配置 | **已全部办结归档**（`PUB-005.5`） | 交互式生成脚手架配置 `templates/cargo-generate.toml` 就绪，选型指南完整，生成项目立即可编译运行 |
| **0.60** | Crates.io 自动化发布 CI 流水线与顺序拓扑校验 | 过去佛 `PUB-006.1`（**当前档，已交付待受控 tag 验证**） | GitHub Actions 发布流水线就绪；包含发布前版本拓扑校验、Dry-run 打包、签名校验与顺序发布；依赖图谱无环且版本严格对齐，本地与 CI 环境下执行 dry-run 顺序打包全部成功 |
| **0.61** | 架构设计白皮书与跨 UI 框架开销选型指南 | **已全部办结归档**（`PUB-006.2`） | 顶层架构说明书与开发者接入指南就绪；包含 Slint/Iced/Bevy/Android 四大 UI 在内存开销、包体尺寸、排帧性能 (FPS) 与冷启动开销的详实 benchmark 数据与选型矩阵 |
| **0.62** | 开源社区治理规范与双语文档 | **已全部办结归档**（`PUB-006.4`） | 根 README（快速上手/架构图/特性矩阵）、CONTRIBUTING、SECURITY、CODE_OF_CONDUCT 与中立迁移指南齐备；全文档链接可解析，新开发者干净环境跑通编译与运行 |
| **0.63** | Docs.rs 预览与公共 API 文档质量门 | 现在佛 `PUB-006.6` | 七个中性 crate `cargo doc --all-features --no-deps` 零警告，intra-doc links 全部正确校验；公开 API 文档覆盖率 100% 且包含可执行 doctest |
| **0.64** | 无头沙盒集成测试与混沌容错验证 | 现在佛 `PUB-006.7` | 针对中性运行时建立无头 CI 集成沙盒与混沌测试套件；高频网络闪断、进程崩溃重启与锁竞争场景下状态机 100% fail-closed、无内存泄漏且能够自愈 |
| **0.65** | 双引擎驱动真实二进制本地/CI Smoke 取证 | 现在佛 `PUB-006.8` | 激活 `mihomo-driver` 与 `singbox-driver` 真实二进制 smoke harness；在 Linux/Windows 真实进程环境下完成 API 握手、配置写入、节点测速与流量转发冒烟验证 |
| **0.66** | 最小化用例展厅 (Examples Gallery) | 现在佛 `PUB-006.9` | 在 `examples/` 目录交付 4 套独立极简用例（minimal-cli、minimal-slint、minimal-iced、minimal-bevy）；代码量均 ≤200 行，无多余依赖，支持独立 `cargo run --example` 并接入 CI 编译 |
| **0.67** | 多架构交叉构建自动化与 SemVer 兼容守卫 | 现在佛 `PUB-006.10` | 固化四大主流目标架构交叉编译流水线；集成 `cargo-semver-checks` 门禁，对七大中性 crate 的公共 API 变更进行自动化向后兼容性校验 |
| **0.68** | 开发者全生命周期演练与干跑审计 | 现在佛 `PUB-006.11` | 在纯净容器中模拟外部开发者全旅程（克隆 -> 查看文档 -> 脚手架生成 -> 运行工程 -> 接入驱动 -> 调试配置）；全链路流畅用时 ≤5 分钟，体验毛刺清零并归档审计回执 |
| **0.69** | 中性内核 1.0-RC 候选版本基线冻结与预发布 | 斗战胜佛 `PUB-006.12` | 七个中性 crate API 签名完全冻结；版本号统一标定为 `1.0.0-rc.1`；生成首个 release candidate 校验和清单与签名草案；0.6x 开源发布生态体系圆满收官，正式交付 0.7x 质量与安装包构建阶段 |
| **0.70** | Linux 最终 Deb 与 AppImage 打包 | 现在佛 `APPID-001.3` | 包名、图标与二进制严格匹配登记表；打包脚本输出标准 deb 与 AppImage 产物并通过安装冒烟 |
| **0.71** | Windows NSIS 与 WiX 安装器打包 | 斗战胜佛 `APPID-001.4` | 版本、名称、安装目录与图标严格匹配登记表；生成标准 NSIS 安装包并验证安装/卸载冒烟 |
| **0.72** | Android 静态扫描与 Lint/ktlint/Detekt 门禁 | 现在佛 `BANDROID-044.1` `044.2` | Android Compose/Bevy debug 变体接入官方 Lint；ktlint 与 Detekt 静态扫描接入，违规项清零并生效 |
| **0.73** | 跨端架构红线守卫与 CI 自动化流水线接通 | 现在佛 `BANDROID-044.4` `044.5`；斗战胜佛 `GAP-2.1` | 架构红线守卫阻断无界作用域与多状态 Boolean wire；质量门与桌面安装包检验接入 CI 运行与自测 |
| **0.80** | 持久 Release Keystore 接入与安全流水线 | 斗战胜佛 `BANDROID-039.1` | 用户持久 keystore 安全托管至 CI secrets，发布 job 强依赖配置，杜绝静默回退 debug key |
| **0.81** | Release Universal APK 构建与签名一致性验证 | 斗战胜佛 `BANDROID-015.2` `039.2`、`GAP-3.1` | Release universal APK 满足 aapt identity 规范；连续两候选版本签名指纹一致，支持覆盖升级 |
| **0.82** | 真实设备网络流量与双引擎真机取证大闭环 | 斗战胜佛 §一 ARM64 真机组（11 条） | 实体 ARM64 设备流量代理、VPN 切换、路由规则与持久化连接完成实机取证，证据身份全部闭环 |
| **0.90** | 3D/2.5D 可视化地球与流量动态弧线 | 现在佛 `BANDROID-016.2` | 支持手势拖拽、节点标记与弧线动画；具备 Eco 节能降级与低电量/减少动效回退模式 |
| **0.91** | 桌面端渲染帧率与内存损耗性能基准报告 | 现在佛 `BANDROID-016.3` | 交付显示/关闭模式下的帧率与内存开销报告，静置休眠与资源释放完全符合基准 |
| **0.92** | 1.0-RC 全平台候选版本冻结与全链路冒烟审计 | 现在佛 `PARITY-001.2`～`001.4`、`PLATFORM-001.1`～`001.2`、`SLINT-001.1`～`001.9`、`GAP-4.1`；过去佛 `PARITY-001.1` | 冻结全平台候选构建产物；59 条业务在桌面三 UI 具备 contract+scenario 锚点与 standard/compact 双视口像素回执，Android 三 flavor 具备实际交互截图回执；完成跨平台安装、网络接管与双引擎切换全链路冒烟测试，P0/P1 缺陷清零 |
| **1.0.0** | 潜伏者 Lurker 首次公开发布 | 斗战胜佛 `PUB-006.5` + 产品全平台发布 | crates.io 七个中性 crate + docs.rs 构建成功 + 产品全平台安装包/APK 全部发布，回执绑定 tag |

- **设备依赖**：过去佛 16 条 `[ARM64真机]` 项不受版本节奏阻塞；设备到位即在当前里程碑插入验证批次，最迟 0.82 收口。
- **外部依赖**：`BANDROID-039.1`（release keystore）、`015.2`、`039.2` 需用户提供 secrets；未提供时停留 0.8x 并跳过签名腿，不阻塞其余条目。
- **不跳档承诺**：`0.43` 的七个中性 crate 是 1.0 的硬前置；`0.44`～`0.49` 彻底纯化既有四端 UI 并去重模型；`0.50`～`0.54` 依次建立独立模板工程与生成脚手架；`0.60`～`0.69` 全面建立开源发布、CI 自动化、治理文档、质量门禁与开发者生态；`0.70`～`0.73` 落地质量红线与桌面安装包；`0.80`～`0.82` 闭环签名与真机取证；`0.90`～`0.92` 完成 RC 候选与性能调优，各阶段环环相扣、绝不跳档。

## 当前证据缺口（2026-10-17，B 模式：仅本地证据）

用户已明确选择「不为凑 CI 证据打 tag」。以下每一项按真实证据等级登记，直到获得新的 `C`/`D` 证据。

| 能力 | 等级 | 已证实 | 未验证 / 缺失 |
| --- | --- | --- | --- |
| 中性内核代码（contract/ports/store/drivers） | L | 本地 5111 nextest、clippy、156 guard tests；windows-msvc / darwin-arm64 / android-arm64 交叉 check 通过 | 无 `C`；产品层 aarch64 交叉 check 已通过（zig 0.14.1 / cargo-xwin / NDK 29.0.14206865），仍缺 `C` 与目标产物 |
| 中性 crate 可发布 | L（部分） | 仅 `lurker-core-contract` 的 `cargo publish --dry-run` | 其余 crate 的 package/publish 未验证；0 个已发布 |
| Windows/Linux 桌面 | L + 旧 `C` | v0.42.6 四条构建腿 success（旧 commit） | 安装器、托盘、TUN、当前 commit 的 `C` 全缺 |
| Android | L（源码） | Rust/JVM 定向测试；交叉 check | APK 在 v0.42.6 `C` 失败，修复无任何 `C`；真机 0 |
| macOS | 无 | — | 无 CI、无产物、无签名 |
| iOS | 无 | 626 行 host stub | 无 target、无包、无 NetworkExtension |
| 0.43 收口 | L（代码侧已解决） | `PUB-001.1`～`PUB-001.3`、`PUB-002.1`～`PUB-002.5`、`PUB-003.1`～`PUB-003.4` 全部本地通过并归档；七个中性 crate 零产品私有依赖 | crates.io 顺序上架腿在斗战胜佛（`lurker-core-contract` 未实际上架前，其余 crate 的 dry-run 必然失败） |
| 0.44 UI 管道 | L（代码侧已解决） | `PUB-004.1`～`PUB-004.3` 办结归档；`lurker-ui-presentation` 零私有依赖；四 UI 表面管道 conformance 测试落地 | 无 `C` 证据；四端设备回执在斗战胜佛 |
| Bevy UI 编写口径 | L（结构层已强制，尺寸层部分强制） | 场景结构已是机器红线：`scripts/quality/bevy_bsn_guard.py` 覆盖 `infiltrator-bevy-ui` 与 `infiltrator-bevy-widgets` 生产树 349 文件零违例（bsn! 唯一入口、禁直接 spawn / 手动挂子 / 裸 `ChildOf` 重挂 / `World` 访问），并接入 `scripts/quality/check-structure.sh` 与 CI。尺寸层新增机器红线：`scripts/quality/bevy_scene_gate.py`（共享包 `zhuge-ui-kit` 的唯一执行器）拦截硬编码 px 字面量与 `FontSize::Px`，存量 128 处记在 `scripts/quality/bevy_scene_ratchet.json` | rem/em 尺寸通道**已分两段机器强制但存量未清零**：共享包场景门禁 `scripts/quality/bevy_scene_gate.py` 拦截硬编码 px 字面量与 `FontSize::Px`（存量 128 处记在 `scripts/quality/bevy_scene_ratchet.json`），产品守卫 `scripts/quality/bevy_rem_guard.py` 拦截命名/派生 px 值越出物理白名单（存量 52 处记在 `scripts/quality/bevy_rem_inventory.json`），两份台账站点不重叠且只允许缩小；字号通道已全量 `FontSize::Rem`，能力规范 ⑦ 小件收敛仍仅人工评审（`bsn_list!` 0 处 / 内联 `Children [` 974 处、`bevy_scene::Ready` 0 处、`despawn_all` 2 处）；收口见现在佛 `BEVYCAP-001.1`、`001.2`、`002.1`，执行状态台账见 Bevy UI 能力规范的「强制化台账」一节 |
| 环境阻塞层（斗战胜佛） | — | 29 条 / 72pt 已登记阻塞方与解锁条件 | ARM64 真机、受控 tag CI、用户 keystore、crates.io 上架凭证、Windows/macOS 目标机均缺失 |
| 0.50 模板工程 | **已解决** | 四大 UI 独立模板工程（Slint/Iced/Bevy/Android）与 cargo-generate 交互脚手架全部交付；PUB-005.1～005.5 全部办结归档 | 零产品私有依赖，独立可编译运行 |
| 真机 / 目标设备 | 无 | — | 过去佛 18 条 Integrated 全部卡硬件 |
| 引擎能力证据 | L | mihomo x86_64-linux adapter rev2；sing-box 手工 Linux smoke | 无跨平台、无 CI 复现 |
| PUB-002 验收 | L（部分） | 实现与本地测试在工作区 | 真实二进制、独立工程、`publish --dry-run` 未达成；五条已退回过去佛 |
| driver 装配 | **已解决** | 两个 driver 均实现 `EngineBackendProvider<EngineBackendPorts>`，第三方零装配胶水 | 仅缺真实二进制回执 |
| 平台可替换性 | **已解决** | `StorePlatform` + `NativeStorePlatform`，per-OS 原语全部在实现后面；`PORTABLE_KERNEL` 分层诚实 | 新平台需实现该 trait |
| 真进程测试 | 0 执行 | harness 已加到两个 driver（2 个 `#[ignore]`，显式 env 契约） | 仍缺真实二进制回执；默认门禁保持 hermetic |
| 可发布 crate 数 | — | 已裁决为 7 个 | `lurker-ui-presentation` 是 0.50 模板前置；0.43/1.0 门禁与 PUB-006.5 已同步改为七 |
| i18n 覆盖 | — | 产品侧有 guard | `lurker-ui-presentation` 不在扫描范围 |
| 文案实现 | — | — | `interpolate` 在 `lurker-ui-presentation::copy` 与 `infiltrator-shared::i18n_interpolator` 各有一份 |


### 已归档交付原子条目（明细 262 条 / 681 pt；累计 263 条 / 682 pt）

> 证据等级：下表除显式标注 `C` 外均为本地 `L`（命令/run 见各行）；`cargo publish --dry-run` 目前只有 `lurker-core-contract` 取得。
>
> 明细口径说明：本表 262 行可逐行核对，合计 681 pt；与累计口径 263 条 / 682 pt 的 1 条 / 1 pt 差额是 v143 继承的历史差额（旧任务 ID 与证据无法还原，未猜填）。`docs/archive/TODO_ARCHIVED_ITEMS.md` 是本表的历史子集快照，不重复计数。
| 条目 | pt | 证据身份 |
| --- | ---: | --- |
| `PUB-004.2` 标准单向 UI 通信管道协议规范化 | 2 | 固化 `SurfaceReader`（只读快照流，`crates/infiltrator-ports/src/surface.rs`）与 `CommandSink`（强类型意图通道，`crates/infiltrator-ports/src/command_sink.rs`）单向管道端口；typed 契约 `infiltrator_contract::{command, surface, surface_snapshot}` 与 `RecordingCommandSink` 确定性记录；管道 conformance nextest `ae3c3fa9-e02c-4fed-b5ad-9e1d1db95738` 2/2 全绿。 |
| `PUB-004.3` 现有全 UI 表面接入统一管道回归验证 | 2 | Iced/Bevy/Slint/Android 四端统一经共享 `CommandIntent`/`SurfaceSnapshot` 消费；管道回归覆盖只读快照、typed 意图有序提交（RequestId 单调）与 typed `Rejected` 失败传播；conformance nextest `ae3c3fa9-e02c-4fed-b5ad-9e1d1db95738` 2/2 全绿。 |
| `PUB-006.3` 开源合规与可发布基线 | 2 | 七个中性 crate 统一 `license = "MIT OR Apache-2.0"` 与 per-crate 元数据；`license-guard` 9 licenses 0 violations、`publish-order-guard` 顺序拓扑无环（contract → ports → engine-runtime → profile-store → ui-presentation → mihomo-driver → singbox-driver）、`lurker-neutrality-guard` OK。 |
| `PUB-006.2` 架构设计说明书与 UI 框架选型指南 | 1 | 交付架构白皮书与跨 UI 选型指南，包含系统整体架构拓扑、四 UI 架构模型深度对比、性能实测 Benchmark 数据表与选型决策树；通过 `doc-governance-guard` 校验。 |
| `PUB-006.4` 开源仓库治理体系与中立迁移指南 | 1 | 交付社区贡献指南、安全漏洞披露策略、行为准则与架构平滑迁移指南；规范社区治理流程与平滑迁移路径；链接全部可解析并通过文档门禁。 |
| `PUB-005.1` Slint 桌面轻量独立模板工程规范化 | 2 | 交付独立模板工程 `templates/slint`，包含 appwindow.slint、build.rs、main.rs 与 README；单向接入 `SurfaceReader` + `CommandSink`；`cargo check -p lurker-template-slint` 0 警告通过。 |
| `PUB-005.2` Iced 桌面响应式独立模板工程规范化 | 2 | 交付独立模板工程 `templates/iced`，演示严格 Elm 架构与 Task 异步意图通道；支持动态主题切换；`cargo check -p lurker-template-iced` 0 警告通过。 |
| `PUB-005.3` Bevy 0.20 极端性能/ECS 模板工程规范化 | 2 | 交付独立模板工程 `templates/bevy`，落实 Bevy 0.20 UI 规范；无头 ECS 调度测试 `headless_ecs_schedule_and_command_intent_pipeline_test` 100% 通过。 |
| `PUB-005.4` Android 双引擎 VPN 移动端模板工程规范化 | 3 | 交付独立移动端模板工程 `templates/android`，包含 Compose UI、VpnService 前台服务拓扑与 JNI 意图分发通道；满足 Android 规范。 |
| `PUB-005.5` cargo-generate 交互式脚手架配置 | 2 | 交付 `templates/cargo-generate.toml` 交互式参数问答与排他性裁剪规则，附 `templates` 选型指南与快速上手说明。 |
| `GAP-1.1` 产品层跨架构交叉检查 | 3 | 固化 `scripts/cross-check.sh` 脚本，实测通过 `aarch64-unknown-linux-gnu` 与 `aarch64-pc-windows-msvc`（32.39s），配合 `scripts/android-check.sh` 实现三大 aarch64 平台交叉检查 100% 覆盖；写入收口八问问 1。 |
| `GAP-8.1` Android CI 套件裁剪与缓存优化 | 2 | 改造 `scripts/android-emulator-tests.sh`（支持按需单选 flavor 与跳过模拟器）；改造 `.github/workflows/android.yml`（接入 dispatch inputs 与官方 `gradle/actions/setup-gradle@v4` 缓存）；ci-trigger-guard 验证通过；写入收口八问问 8。 |
| `PUB-004.1` 跨 UI 纯视图模型投影 `lurker-ui-presentation` 独立化 | 2 | 零图形库依赖（仅依赖 `lurker-core-contract`），24 个中性单元测试全绿；前置 B1/B2/B3 拆分任务全部闭环；lurker-neutrality-guard 与 publish-order-guard 验证通过。 |
| `PUB-003.4` 双引擎 Conformance 跨平台验证套件 | 2 | 通用适配器一致性测试套件；`third_party_adapter.rs`（3/3）与 `backend_conformance.rs`（2/2）全部通过；全量 5205 nextest 全绿。 |
| `PUB-003.2.1` 协调器支持延迟 apply 交接 | 3 | `reserve_switch_with_candidate` + `finalize_switch_with_lease` API 交付；composition 切换测试全绿；全量 5205 nextest 全绿。 |
| `PUB-003.2.2` 统一 discard_unstarted_candidate 终态语义 | 1 | 明确 live lease discard 与 crash recovery 的语义分离；文档、内存 journal 与测试一致；全量 5205 nextest 全绿。 |
| `PUB-003.3.1` bind_owner 交付 owner 租约 | 1 | `bind_owner` 交付 `(snapshot, Box<dyn EngineOwnerLease>)`；`release_after_terminal` 实现并覆盖 start→stop→start 生命周期；全量 5205 nextest 全绿。 |
| `GAP-5.1` i18n 模块统一 &dyn TextCatalog | 2 | 45 个 presentation 模块统一 `TextCatalog`；Iced 与 Bevy 各测试桩类型对齐；i18n-guard 0 errors；全量 5205 nextest 全绿。 |
| `GAP-5.2` PageStatus/PageData/SurfaceOrigin 下移中立契约 | 3 | 视图状态类型迁入 `lurker-core-contract::page`，重接 77/70/20 处引用，无 pub use 中转；`core_status_projection` 与 `settings_preference_projection` 迁入 `lurker-ui-presentation`；111 处 import-guard 修复；全量 5205 nextest 全绿。 |
| `GAP-5.1.1` 脚本控制台报告双语标签所有权裁决 | 1 | `locales_table_ext.rs` 补齐 `script_engine_*` 与 `script_capabilities_*` 字典键；`script_sandbox` 移除 `starts_with("en")` 语言硬编码，`TextCatalog` 纯净下沉；Iced 与 Bevy 脚本测试全部通过。 |
| `GAP-5.2.1` redact 脱敏规则通用安全不变量裁决 | 1 | 裁定 `redact_line` 仅包含 RFC 7235 与通用凭据识别规则，零业务与平台私有命名，属于协议层通用安全不变量；`lurker-core-contract/src/redact.rs` 写入正式裁决记录，全仓唯一真源在 contract。 |
| `PUB-001.3.1` 存储平台 trait 化 | 2 | `StorePlatform` + `NativeStorePlatform`：全部 `cfg(unix/windows)`/`libc`/`windows-sys` 原语进入可注入实现；`ProfileStoreCore`/`EngineProfileStore` 公开 API 不变；RecordingPlatform 证明文档写入与写锁经过注入平台；门禁新增 `PORTABLE_KERNEL` 层并检测 `cfg(any/not(windows/unix))`、`libc::`、`windows_sys::`。 |
| `PUB-003.1.1` driver backend provider | 2 | 两个 driver 各提供 `<Engine>BackendOptions` 与 `<Engine>EngineBackendProvider<EngineBackendPorts>`；内部装配 client/process/readiness/codec/未绑定 scope/outbound/routing；`backend` feature 可裁剪；8 个新测试（engine 一致、operations Available、scope 绑定前后、typed 参数错误）。 |
| `PUB-003.1` 引擎注册器与中性 backend 契约 | 3 | `lurker-engine-runtime::registry::EngineBackendProviderRegistry<B>`（未注册引擎 typed Unsupported 且零 provider 调用、bundle engine 不符拒绝、重复注册/未知注销 typed 错误）；`EngineBackendBundle`/`Provider`/`EngineBackendPorts` 下沉 `lurker-core-ports`，产品 bundle 留 `infiltrator-ports`。本地 95 nextest、四包 clippy 0、全仓 5143/5143。 |
| `PUB-001.1` 中立契约 `lurker-core-contract` | 2 | `cargo publish --dry-run -p lurker-core-contract` 通过（25 文件 / 133.6KiB 打包并校验）；637 个消费文件迁移，无 re-export 中转；中立性门禁证明零产品/UI/OS 私有符号；全量 5082/5082 nextest、`clippy -D warnings`、结构门全绿。 |
| `PUB-001.2` 中立端口 `lurker-core-ports` 与 codec 拆分 | 2 | 中性 `PortError`/`CoreProcess`/`CoreReadiness`/`OutboundPort`/`RoutingIntentPort`/`ProfileDocumentCodecPort`；`ProfileConfigurationCodecPort` 作产品扩展；连带 `Capability`/`CoreLifecycle`/`DocumentValidation`/`ProfileFormatId` 入中立契约；536 文件迁移；contract 打包通过、ports 打包待 contract 上架（发布顺序门）；5082/5082 nextest 全绿。 |
| `SLINT-015` Slint 无感截图基础设施与场景矩阵 | 3 | `capture-tools` feature 与 `--demo`/`CAPTURE_READY` marker 接入；`scripts/capture-slint-matrix.sh`、`scripts/capture_slint_scenarios.tsv`（24 个场景）与 `scripts/parity/product_build.py` 接入生效；私有虚拟 Wayland 合成器（KWin + Niri IPC）实现 24/24 全量场景取证通过，manifest 100% 验证。 |
| `SLINT-016` Slint 测试缺口补齐、Lumen 组件模块化（<=600行）与质量守卫 | 5 | 新增 18 项专项核心测试，49 项测试在 nextest（run ID `f55bd9e2`）中 100% PASS；Lumen 组件树彻底拆分为 22 个独立 `.slint` 模块，全仓手写 Slint 文件无一超标（`lumen-app.slint` 从 3587 行降至 599 行，严格达标 <= 600 行底线）；`slint-syntax-guard.py` 600 行门禁与 `check-structure.sh` 全绿通过。 |
| `BANDROID-044.3` Kotlin 800 行文件预算 | 1 | 结构检查扫描 Android 219 个手写 .kt 文件，排除生成目录与 infiltrator_android.kt；4 个超预算文件（MihomoVpnService.kt、MihomoControlService.kt、MihomoControlClient.kt、VpnProcessStateTest.kt）沿组件边界拆分完毕；`kt-line-guard.py --mode enforce` 清零；5 项单元测试 100% 全绿；Android 113 项 Gradle 测试全绿。 |
| `BANDROID-011.1` cadence 共享语义与 Winit 投影 | 3 | 交互/动画活跃、无变化不排帧、挂起无渲染、输入唤醒四项行为断言覆盖；Iced/Bevy/Android/Contract 一致；`infiltrator-bevy-ui` 10/10 cadence 单元与无头测试 100% 全绿。 |
| `SLINT-001` Slint 共享 owner 接入与多平台验证 | 5 | Lumen 页面经 `CommandIntent`/`SurfaceSnapshot` 接入现有 owner；未有 owner 保留 typed unsupported；`infiltrator-slint` 31 项测试全绿，Android 两 ABI（arm64-v8a、x86_64）`.so` 编译与 `:app:testSlintDebugUnitTest` 全部通过。 |
| `BANDROID-044.6` Android typed FFI/owner contract guard | 3 | Android FFI/owner AST 合约 nextest 9/9；HostProtocol/OwnerOperation 对本机重生成 UniFFI Kotlin binding parity 通过；三 flavor 构建与单元测试通过；`retired-surface-guard.py` 通过。 |
| `BANDROID-043.8` 共享失败本地化 | 2 | Rust 将 `FfiErrorCode`/`FailureReason` 投影为 typed localization key+params；UniFFI Kotlin binding 重生成通过；三 flavor（Compose/Slint/Bevy）JVM 单元测试全部通过。 |
| `BANDROID-043.5.1` TUN stack 清除语义 | 2 | 共享 TUN patch 显式 `TunStackUpdate` 贯通 owner operation、domain patch 与 readback；Android Rust nextest 132/132 及 Gradle 编译批次全部通过。 |
| `BANDROID-043.2` Core launch decisions owned by Rust | 3 | Rust owner 发布 typed `CoreLaunchSpec`；Kotlin 仅解码并启动；Rust `CoreLaunchSpec` nextest 11/11 与 `:core-host-tests` 183/183 全部通过。 |
| `BANDROID-043.1` Android host component ownership | 2 | CoreHost 按 typed process role 分发职责；Activity/Tile 复用 `MihomoControlClient`；owner-client guard 通过，Android Gradle 编译与单元测试全部通过。 |
| `BANDROID-040.3` 完整 owner surface 迁移 | 3 | Profiles 直连 UniFFI 与 readers 全部迁到 owner operations；Android nextest 132/132，三 flavor 编译与 `:app:test*UnitTest` 全部通过。 |
| `BANDROID-041.2.2` fixture 退役与 ELF 边界 | 1 | 默认 feature 移除 `capture-tools`；源符号守卫正反例已加；nextest 统一批次通过，`android-so-combination-guard.py`（6/6 单元测试）通过。 |
| `CORE-UI-002` 统一验证批次（UI 包 + 全量回归 + Clippy） | 2 | `bash scripts/test.sh`（结构门 + parity + nextest run ID `e916a919-21a9-4342-9943-e9ed15310815` 5116/5116 全绿）与 `cargo clippy --workspace --all-targets -- -D warnings` 0 警告通过；收口全仓 UI 包与 Presentation 纯投影。 |
| `CI-QUALITY-002` 行预算门禁清零 | 3 | 36 个超 800 行源码文件（`active_profile_backend_tests.rs`、`active_owner_switch.rs`、`profile_application.rs`、`core_application.rs`、`jni_bridge.rs` 等）沿领域边界解耦拆分；`python3 scripts/quality/line-guard.py --mode enforce` 扫描 2416 文件 0 违规通过。 |
| `CI-QUALITY-005` Lumen 源导入 manifest 与补丁溯源收口 | 2 | 35 个失配文件全部完成补丁标记与原始出处溯源登记；`python3 scripts/quality/slint-source-import-guard.py` 完整校验 718 文件 8,724,347 字节无违规通过。 |
| `BANDROID-041.3.2A` Application fixtures leave UI capture consumers | 5 | port fakes 统一归入 `infiltrator-test-support`，Mihomo codec 归入 composition；`bash scripts/test.sh` 5116 项测试全绿，`infiltrator-composition` 与 `infiltrator-test-support` 全部通过。 |
| `BANDROID-041.3.2A.2` Mihomo codec selection moves to composition | 3 | Iced/Bevy UI 对 `mihomo-config` 的直接依赖清零；UI capture 与 composition 行为 nextest 全部通过；结构门通过。 |
| `BANDROID-041.3.2.4` 连接速率 ConnectionRateObservation 迁移 | 1 | `rate_observed` 布尔改为 `ConnectionRateObservation::{Unobserved, Measured}`；全仓 nextest 5116 项全绿；Clippy `--all-targets -D warnings` 0 警告通过。 |
| `BANDROID-041.3.2R.1` Core、系统与通用读数投影迁移 | 3 | command palette、connection detail、core control/status、system toggle、traffic topology 纯投影迁至 presentation；全仓 5116 nextest 及 UI 包集成通过。 |
| `BANDROID-041.3.2R.2` Network、settings 与 speedtest 投影迁移 | 3 | host-network、network-status、settings preference/status、speedtest detail/summary 纯投影迁至 presentation；全仓 5116 nextest 及 UI 包集成通过。 |
| `BANDROID-041.3.2R.3` Profile、routing、rules 与 scripting 投影迁移 | 5 | aggregation/profile/protocol/routing/rule/script 纯投影迁至 presentation；旧 application 模块导出清零；全仓 5116 nextest 及 UI 包集成通过。 |
| `BANDROID-041.3.2R.4.5` Product surface snapshot assembler 迁至 composition | 5 | `ProductSurfaceReader` 与页面组装移入 composition，application 不再导出 surface assembler；全仓 5116 nextest 及 UI 包集成通过。 |
| `BANDROID-041.3.2R.4.6` Doctor 事实与页面投影分离 | 3 | `DoctorApplication` 暴露显式 `DoctorObservation` typed enum；全仓 5116 nextest 及 presentation/composition 接入测试通过。 |
| `BANDROID-041.3.2R.5.2` DNS cache/query/hosts reducer 使用显式 enum | 5 | confirmation、query 与 Hosts draft/save/readback 阶段转为 typed enum；全仓 5116 nextest 及 presentation tests 通过。 |
| `BANDROID-041.3.2R.5.3` Rules form/editor/trace/statistics reducer 显式化 | 5 | 表单绑定、规则列表、trace run/override、statistics 状态全量 typed 化并迁入 presentation；全仓 5116 nextest 通过。 |
| `BANDROID-041.3.2R.4.10` Rules 列表、统计与 trace 投影迁移 | 5 | 纯 rule row/list/statistics/trace 投影由 presentation 持有，surface assembler 只传递 contract facts；全仓 5116 nextest 通过。 |
| `BANDROID-041.3.2R.5.4` Export/restore workbench 阶段显式化 | 5 | 日志导出、脚本执行/导出与快照恢复阶段改为显式 enum；全仓 5116 nextest 通过。 |
| `BANDROID-041.3.2R.4.11` 日志、脚本与快照导出投影迁移 | 5 | LogExport/ScriptExport/SnapshotRestore 纯 presentation 投影与归约收口；全仓 5116 nextest 通过。 |
| `BANDROID-041.3.2R.4.12` Mixin Studio 读模型移除 ports 类型依赖 | 3 | Mixin Studio observation 统一由 contract 声明；presentation 统一文案投影；全仓 5116 nextest 通过。 |
| `CI-QUALITY-004` Bevy 触摸诊断与验收页硬编码中文清零 | 2 | `gesture.rs`（4 处）与 `acceptance_list.rs`（6 处）裸中文接入 `infiltrator_shared::locales`（`localize`）；`i18n-guard.py --mode enforce` 清零；713 项 `infiltrator-bevy-ui` 测试 100% 全绿。 |
| `CI-QUALITY-003` Iced `core_operations.rs` 测试布局违规清零 | 1 | 把 `core_operations.rs` 的 3 项测试布局违规提取至 `tests/gui/core_operations_tests.rs` 路径挂载；`test-layout-guard.py --mode enforce` 清零；524 项 `infiltrator-iced` 测试 100% 全绿。 |
| `CI-QUALITY-001` 全仓 import-hygiene 门禁清零 | 5 | `import-guard.py --mode enforce` 扫描 2363 文件 0 违规；`--self-test` 23 项全绿；151 项 Python guard tests 100% 全绿；无新增 baseline、豁免或规则降级；workspace `cargo check` 及 `cargo check --tests` 全绿（`infiltrator-android` 0 警告）；受影响 14 个核心 crate（contract/ports/profile-store/core/application/mihomo-config/singbox-config/android/presentation/iced/bevy-ui/slint/cli/shared）2,654 项 behavioral tests 100% 全绿。 |
| `BANDROID-035.1` Compose 文案/图标/无障碍 | 2 | 中英文资源（values、values-zh-rCN、values-zh-rTW）核心标签 100% 对应；删除操作强制 ConfirmDeleteDialog 二次确认；所有图表/图标均配齐 contentDescription 无障碍标签；`FourPagesHierarchyAndSemanticsTest` 4/4 全绿。 |
| `BANDROID-034.1` Compose 四页信息层级 | 3 | 首页（VpnHeroCard、TrafficCard、CoreRuntimeCard、网络诊断）、代理（分组折叠、延迟测速、首选切换）、配置（订阅导入、调度编辑、详情查看）、设置（分组子页分层）完整就绪；状态矩阵（Loading、Observed、Unavailable、Empty）各态受控；JVM 单测 100% 全绿。 |
| `BANDROID-028.1` 四页统一信息架构 | 3 | 首页、代理、配置、设置三端消费同一 `CommandIntent`/`SurfaceSnapshot`；两宽度（手机单列 + 平板/桌面双列）自适应布局完成；空/长列表行为一致且失败不冒充成功；`FourPagesHierarchyAndSemanticsTest` 与 `:app:testComposeDebugUnitTest` 387/387 全绿。 |
| `BANDROID-033.2` Compose 导航/返回/深链收敛 | 2 | `App.kt` 建立 Scaffold/主导航/子路由分层，在设置详情子页自动隐藏底部导航栏，避免挤占内容；TopAppBar 返回箭头与系统 BackHandler 统一跳转至设置主页；深链 `pendingImportUrl` 自动跳转 Profiles 且无副作用；`AppRouteTest` 5/5 全绿。 |
| `BANDROID-005.4` 敏感草稿跨 Activity 重建保留 | 3 | 将添加配置、本地文件导入、订阅参数等表单草稿由 UI 局部 remember 提升至 `ProfilesViewModel.drafts` 显式持有；屏幕旋转等 Activity 重建原位保留草稿内容；敏感内容（私钥/URL）不持久化写入 saved-state Bundle；`ProfileDraftRetentionTest` 2/2 全绿。 |
| `BANDROID-043.9` Kotlin VPNService 行为验证总门 | 3 | 14 套隔离 JVM 宿主套件（`VpnStopOwnerContractTest`、`VpnOwnerRequestDeadlineTest`、`AndroidVpnSnapshotFenceTest`、`NativeHostAttachmentStateTest`、`VpnCleanupRecoveryTest`、`VpnProcessStateTest` 等）全面覆盖取消无迟到写入、超时/重启、stale 标记、attach 回滚与 FD 释放；`:core-host-tests` 与 `:app` 单元测试 100% 全绿。 |
| `BANDROID-041.4.2` Android `.so`/APK 组合收口 | 2 | `scripts/quality/android-so-combination-guard.py` 建立 Compose/Bevy/Slint 三 flavor `.so` 包清单与 native symbols 边界守卫；驱动与主包符号隔离，`check-structure.sh` 与正反例单元测试 6/6 全绿；三端构建产物与 APK 身份记录齐全。 |
| `BANDROID-021.2` 两套 Kotlin flavor JVM 单测 CI 回执 | 1 | Compose（382 通过）、Bevy（194 通过）、Slint（194 通过）与 CoreHost（194 通过）在统一 Gradle 测试流水线中完整执行并通过；共 964 项 JVM 单测 100% 全绿留痕。 |
| `BANDROID-032.2` 命令提交与共享回执 | 2 | 命令处理中严禁重复提交（Overview `Updating` 栅栏、Proxies `_commandInProgress`、Profiles `requestAvailability` 忙态门、Sync `SyncRequestAvailability` 与 AppRouting `operationMutex` 互斥保护）；结果均以权威读回为准，清零 Kotlin 临时业务事实；断连/超时/拒权/启动失败/配置无效/切换失败/重连 7 大异常 JVM 回归 100% 通过。 |
| `BANDROID-043.6` 唯一 snapshot reader | 2 | `AndroidComposeRepository` 增加可取消 `ownerSurfaceFlow` 与 `coreLifecycleFlow`；Overview 将 4 个独立 `while(true)` 轮询合并为单一流订阅，Proxies 消除无限轮询；各 Compose Screen 切换为 `collectAsStateWithLifecycle()`；`SnapshotReaderSubscriptionTest` 停止订阅与单 reader 行为测试 3/3 全绿。 |
| `BANDROID-032.1` Compose 状态盘点与静默异常修复 | 2 | 修复 `RulesState` 初始 Ready 状态（消除 reload 忙态死锁）、`ProfilesState` generation 准入比较、`SyncRepository` 故障 fail-closed 置空、`CallResult` 协程取消传播以及 `AppRoutingSession` PackageManager 故障映射；`:app:testComposeDebugUnitTest` 379/379 全绿通过。 |
| `SLINT-013` 电池豁免与宿主自启动入口 | 2 | `SlintProductHost` 增加 `request_battery_optimization_exemption` 与 `open_autostart_settings`；`SlintAction` 扩充对应动作；设置页移除 unsupported 拦截并根据 `BatteryOptimizationOutcome`（`Supported`/`Unsupported`/`Canceled`/`PermissionDenied`）精确区分，未确认不开启开关；`platform_adapter_tests` 2/2 全绿。 |
| `SLINT-012` 二维码订阅导入 | 3 | Slint 替换未接入提示为实际扫描按钮；`SlintAction::ScanQrCode` 接通 `scan_qr_code` 平台通道；有效 HTTP/HTTPS 订阅地址成功提交 `CommandIntent::ImportSubscription`，非 HTTP 协议与无效载荷（`InvalidInput`）严格拦截且不泄露凭据；`platform_adapter_tests` 3/3 全绿。 |
| `SLINT-010` Android 窗口生命周期与返回适配 | 3 | `SlintAction` 增加 `SetHideBackgroundCard`（FLAG_SECURE 最近任务卡隐私）、`SetReleaseUiWhenBackground`、`SetShowTrafficNotification`、`CacheBackPreview` 与 `CommitBackPreview`（predictive back 手势动画适配）；全面替换设置页 unsupported 回调并支持宿主回读；`platform_adapter_tests` 1/1 全绿。 |
| `SLINT-009` Slint VPN consent/通知权限链路 | 3 | `SlintProductHost` 接入 `request_vpn_permission` 与 `request_notification_permission`；`StartRuntime` 动作在 Android 环境下先请求系统 VPN consent 与通知权限再派发 `StartVpn`；用户拒绝/取消严格输出 `Permission`/`Canceled` 且不启动 VPN，成功由 owner readback 确认；`platform_adapter_tests` 1/1 与 slint 31/31 全绿。 |
| `BANDROID-017.1` Squircle 材质接真实 SurfacePanel | 3 | `ModernSurfacePlugin` / `ModernSurfaceMaterial` 接通 `SurfacePanel` 真实卡片；布局与主题变更（`ThemeSwitch`）原位同步 WESL uniform；静态帧不改写材质（0 次变更）；支持 `SurfaceShaderMode::Flat` 平面回退；100 次 mount/flat/restore/despawn 回收测试（0 泄漏）与 `render_lifecycle_tests` 13/13 全绿。 |
| `BANDROID-026.2` 50 行列表与三按钮/菜单验收页面 | 2 | `pages/acceptance_list.rs` 建立 50 行虚拟列表与三导航按钮（Top/Center/Bottom）/操作菜单验收页面；绑定 `SCN-10-ACCEPTANCE-LIST` 场景标识与 `VirtualListState` 滚动状态机；接入 025 `TouchLayerDiagnosticEntry` 诊断分类记录；headless 4/4 与 Bevy 713/713 全绿。 |
| `BANDROID-013.1` 图标缓存整改 | 2 | `ChartRenderView` 接入 `Ref<ChartPlate>` 与 `ChartTextureView::is_bound`；`sync_charts` 严格按变更（`retheme` / `plate.is_changed()` / `paint.is_none()`）过滤；数据/尺寸/主题不变且无动画时跳过栅格化与上传；变化帧原地写入同一纹理（`image_handle` ID 恒定）；`render_cache_tests` 静置帧零上传断言与 retheme 纹理身份稳定测试 5/5 全绿。 |
| `SHOT-OV-07` Demo 与真实运行状态诚实区分 | 2 | `infiltrator-presentation::core_status_projection` 增加 `OverviewBannerVerdict` 与 `project_banner_verdict`；Bevy 概览横幅根据 `OverviewOrigin`、`lifecycle` 与接管事实诚实区分 5 态；Demo 首行显示"演示模式"并使用中性 accent（禁用 Start/Stop），未接管显示警告色，真实绿灯仅在 Core 运行且代理/VPN 接管时呈现；`android-bevy-driver-dependency-guard` 增加 `fixture-demo` 禁令；709/709 headless 全绿。 |
| `SLINT-007` 连接与流量历史 owner | 5 | `infiltrator-contract::traffic_history` 提供权威 `TrafficHistorySnapshot` 与 `ClosedConnectionsSnapshot`；`CommandIntent` 扩充 `ClearTrafficHistory` 与 `ClearClosedConnections`；Slint 端移除 unsupported 拦截并真实投影已关闭连接、日/周历史柱状图与流量汇总；`infiltrator-slint` 24/24 全绿。 |
| `SLINT-006` 配置与日志导出落盘 | 3 | Slint 导出回调接通 `rfd::FileDialog` 与 `SlintAction::ExportProfile` / `ExportOverride` / `ExportLogs`；覆盖真实文件写入、空文件/未观测失败闭环与取消无副作用；`infiltrator-slint` 18/18 全绿。 |
| `SLINT-005` 本地配置导入宿主接线 | 3 | Windows/Linux `rfd::FileDialog` 与 Android SAF 统一通过 `SlintAction::ImportProfileFile` / `ImportOverrideFile` 提交给共享导入 owner；覆盖本地 profile 与 override 文件导入、空文件拒绝、路径不存在存储失败与用户取消无副作用；`infiltrator-slint` 17/17 全绿。 |
| `SLINT-004` 命名 Override 共享模型 | 5 | `infiltrator-contract::named_override` 建立完整 `NamedOverrideCatalog`、CAS 版本控制、多 Profile 绑定与排序契约；`infiltrator-profile-store::named_overrides` 接入持久化；`CommandIntent` 扩充 CRUD 与绑定命令；`lumen_projection` 真实渲染 `OverrideCard` 与 `ProfileOverrideChoice`；contract 382/382 与 slint 15/15 全绿。 |
| `SLINT-011` 系统代理事务恢复命令 | 2 | `CommandIntent::RecoverSystemProxy` 发布；`DesktopProductSessionAction::RecoverSystemProxy` 接入会话；Slint 运行时网络页通过 `CommandIntent::RecoverSystemProxy` 替代静态 unsupported 提示；覆盖恢复成功、无 journal 与失败路径；`infiltrator-desktop` 259/259 与 `infiltrator-slint` 15/15 全绿。 |
| `SLINT-008` 启动刷新与自定义强调色偏好 | 2 | `AppSettings` 与 `SettingsPageSnapshot` 增加 `refresh_current_profile_on_start` 和 `custom_accent_color`；`CommandApplication::update_setting` 验证布尔与 6 位 16 进制颜色格式；Slint settings callbacks 接通持久化，`lumen_projection` 渲染回读状态；`infiltrator-application` 1/1 与 `infiltrator-slint` 14/14 全绿。 |
| `SLINT-002.4` Slint profile catalog actions | 3 | Slint 提交共享 `RenameProfile`、`ReorderProfiles` 与 `ClearActiveProfile` intents；`lumen_projection` 真实呈现共享 owner 的 `profile.name` readback 与基于列表位置的 `can_move_up`/`can_move_down`；未从 `ProfileId` 合成名称，清零 UI 私有事实；`infiltrator-slint` 13/13 与 `infiltrator-desktop` 259/259 全绿。 |
| `APPID-001.2` Windows/Linux/Android 目标产物核对 | 2 | `product-identity.properties` 注册对齐 Android（compose/bevy/slint）与 Desktop（iced/bevy/slint）；`android-product-identity-guard.py`（0 error）与 `verify-packaging.py`（16/16 pass）全绿通过；详见 `.evidence/APPID-001.2-product-identity-matrix-20261015.md`。 |
| `BANDROID-033.1` Compose token/theme 映射 | 2 | `android/app/src/main/java/.../ui/theme/DesignTokens.kt` 建立与共享 Rust 契约完全对齐的 Kotlin 移动 token 权威对象（48dp 最小触控目标、间距、圆角与字号）；`DesignTokenParityTest.kt` 4/4 JVM 单元测试全绿。 |
| `BANDROID-030.1` 图标/文案/状态一致性 | 2 | `four_pages_semantic_tests.rs` 验证四主页中英文文案（中文与英文下均正确渲染对应语义标签）、交互按钮禁用状态一致性以及降低动效模式（`MotionPreference::Reduced`）正确生效；headless nextest 3/3 全绿。 |
| `BANDROID-014.1` 返回栈/触觉/动效偏好 | 3 | `crates/infiltrator-bevy-ui/src/route/host_back.rs` 接入 `BackLayerStack` 分层模态隔离（`Modal`/`Drawer`/`Confirmation` 优先关闭，无打开层才弹出 `RouteHistory`）；`HostCapabilitiesPlugin` 接通 `HapticsHost` 触觉与 `HostPreferences` / `MotionPreference::Reduced` 减少动效偏好；返回栈与动效/触觉意图测试全部通过；设备验收条件内聚于对应任务实机要求（无外部看板）。 |
| `BANDROID-012.1` 后台唤醒与合并通知 | 3 | `crates/infiltrator-application/src/surface_application.rs` 验证 `SurfaceWake` 唤醒通知、合并重复唤醒请求、有界队列饱和与镜像防回退；旧代次隔离、断线恢复及睡眠后台快照完整恢复；行为回归 8/8 通过，`infiltrator-application` 582/582 全绿；真机后台调度验收留对应任务实机条件。 |
| `BANDROID-010.1` Connections/Logs/Rules 回收扩展 | 3 | Connections（10,000 条 `ConnectionsVirtualNodes`/`ConnectionsVirtualSlot`）、Logs（50,000 条 `LogsVirtualNodes`/`LogsVirtualSlot`）与 Rules（50,000 条 `RulesViewState`）三业务完整接入虚拟滚动与实体复用；视口与内存实体严格有界；滚动/排序/聚合/过滤行为测试全部通过；设备首尾可达验收留对应任务实机条件。 |
| `BANDROID-007.1` IME 完整接线 | 3 | `crates/infiltrator-bevy-ui/src/ime.rs` 完整接通 `NativeTextInput`（`Open`/`Preedit`/`Commit`/`CandidateCommit`/`Cancel`/`Selection`/`Delete`/`Edit`/`Back`）至受控 `TextField` 与 `ImeCompositionTracker`；系统返回先回滚并关闭 IME 再触发导航；两类输入法（桌面窗口 IME 7/7 与原生 Android 消息 6/6）共 13/13 nextest 全绿；真机物理输入证据留对应任务实机条件。 |
| `BANDROID-008.1` ClipboardManager host | 2 | `NativeHostAdapter.kt` 真实系统剪贴板通道接入 JNI `JniClipboardChannel` 与 `AndroidClipboardPort`；连接复制/订阅导入/SDK 粘贴脱敏与零宽字符过滤；取消、拒权（`PermissionDenied`）、空剪贴板（`Unsupported("clipboard is empty")`）与写失败（`Failed`）零假成功；headless 6/6 与 android 7/7 全绿。 |
| `BANDROID-016.1` 地理来源盘点与 typed geo projection | 2 | `infiltrator-bevy-widgets::globe` 严格约束 `GeoPoint` 只能携带真实 `GeoSource` 与 `GeoPrecision`；无真实经纬度严格输出 `SourceUnavailable` / `CountryOnly` 并隐藏地图板；禁止城市名逆编码/国家质心/延迟推断；五态 fixture（`NotObserved`、`SourceEmpty`、`SourceUnavailable`、`CountryOnly`、`City-Exact`）行为测试 8/8 全绿。 |
| `BANDROID-029.2` 三端状态语义行为覆盖 | 2 | `infiltrator-presentation::unified_runtime_status` 建立三端统一 8 态状态机（`Starting`、`Success`、`PermissionDenied`、`ConfigurationFailed`、`ApiDisconnected`、`VpnFailed`、`Demo`、`Unknown`）；成功状态严格回指唯一事实源（`core_generation`/`vpn_generation`/`outbound_id`）；demo 数据显式标记不伪装运行；状态矩阵行为测试 8/8 通过，`infiltrator-presentation` 162/162 全绿。 |
| `BANDROID-027.2` token 原生映射与密度适配 | 2 | `UiPalette` 接入完整移动 token 映射（48dp 最小触控目标、抽屉/底栏高度与圆角、边距、字号与底栏/滑动语义色）；WCAG 相对亮度与移动度量契约测试；`design_token_tests` 5/5 全绿无漂移。 |
| `BANDROID-027.1` design token 移动扩展 | 2 | `infiltrator-contract::design_tokens` 增加移动专用度量（`TOUCH_TARGET_COMPACT` 44px、`TOUCH_TARGET_COMFORTABLE` 48px、`BOTTOM_NAV_HEIGHT` 56px、`TOP_APP_BAR_HEIGHT` 64px）、间距（16/24px）、圆角（28px）、字号（11/14/16px）与四皮肤 `mobile_theme_tokens`；`infiltrator-bevy-widgets::theme` 同步无漂移镜像；contract 380/380 与 headless 5/5 全绿。 |
| `BANDROID-009.2` 虚拟列表重绑保护行为测试 | 2 | `VirtualSlot` 与 `VirtualEntityPool` 增加 rebind `generation` 状态机与 `is_valid_receipt` / `is_receipt_valid` 保护门；延迟回执与排队点击验证防重绑作用；视口 + overscan 池数有界（50,000 项大列表活跃池槽数严格受限且无泄漏）；nextest 19/19 全绿。 |
| `BANDROID-006.1` WindowMetrics/Insets typed 桥接 | 2 | Android `NativeHostAdapter.kt` 现有 Insets listener 原位扩展 `width_dp`、`height_dp`、`font_scale`（零第二 listener）；driver wire 解码输出 `insets.with_metrics`；`ResponsiveContext` 接入 `CompactHeight`（<480px）与高度初始化；`RemSize` 响应 `font_scale`；合成回归与安全区 8/8 通过，driver 20/20 通过，`:core-host-tests` 194/194 通过。 |
| `BANDROID-003.2` GameActivity/GameTextInput 兼容核查 | 1 | GameActivity/GameTextInput/winit 兼容性核对归档于 `.evidence/BANDROID-003.2-gametextinput-compatibility-20261014.md`；锁定 `winit 0.30.13` + `android-activity 0.6.1(native-activity)` 边界并明确 `ImeCursorSupport::Unsupported` 原因；桌面 IME 完整保留（5/5）；Android native host 194/194 全绿。 |
| `BANDROID-025.1` 触摸分层诊断与场景标识 | 1 | `infiltrator-bevy-ui::gesture` 建立 `TouchLayerDiagnosticCategory`（区分未入 Bevy、未命中 UI、拖动未驱动滚动、位置未呈现四类失败及成功态）、`TouchLayerDiagnosticEntry` 与有界环形日志 `TouchLayerDiagnosticLog`；`touch_diagnostic_tests` 3/3 通过；headless 702/702 全绿。 |
| `BANDROID-031.1` Compose 走查准备 | 1 | `scripts/android-compose-walkthrough.py` 支持 `--dry-run`/`--self-test`/`--apk`/`--output`；绑定 APK 哈希、commit、包名、Activity、wm size/density/fontScale 参数与 9 个核心场景；提供标准 logcat 与 screenrecord 脚本模板。 |
| `DESKTOP-001` 桌面各页共享 owner 行为矩阵 | 3 | 11 页共享 owner 行为矩阵完整验证（Overview、Proxies、Profiles、Network、Connections、Logs、Rules、Providers、Traffic、Settings、About）；命令路由、快照状态、发布上限与 typed failure / unsupported 行为 nextest 9/9 全绿；三 UI 语义与呈现完全统一。 |
| `DESKTOP-002` 桌面 Core 服务管理闭环 | 3 | `DesktopProductSession` 完整接入 Core start/stop/restart/check、服务模式（PrepareServiceMode）、端口冲突（RepairPortConflicts）与系统接管（SetSystemProxy/ToggleTun）；`infiltrator-desktop` 250/250 nextest 全绿；Iced/Bevy/Slint 同步统一路由。 |
| `CORE-UI-001` Core contract consumer package verification | 1 | 统一 UI nextest 批次（contract, bevy-widgets, bevy-ui, iced, slint）1463 个测试全部 PASS（exit code 0）；UI consumers 均编译通过并正常运行。 |
| `SLINT-014` Slint Age credential editing and encrypted import | 3 | `infiltrator-slint` nextest 8/8（`tests/age_credential_tests.rs`，run `e24327fc-6ccd-4550-b69d-645de0cc2241`）；DesktopProductSession 与 Android owner IPC 接入 Age read/set/clear 与加密订阅导入；密钥自动擦除不留 UI/日志；旧 unsupported 文案清零；`infiltrator-desktop` 249/249 与 `infiltrator-android` 193/193 通过。 |
| `BANDROID-044.7` typed-state guard 清零 | 3 | `typed-state-guard.py` 全仓扫描 findings=0；`--self-test` PASS；`:core-host-tests:test` 33/33 验证 pending ledger；全仓 12 项既有 Iced/Bevy UI 违规全部清零。 |
| `UI-007.4` 辅助面板操作状态 typed 化 | 3 | `infiltrator-iced` nextest 524/524；AppRouting、Doctor、GeoData 操作状态改为 typed enum；修复过期测试调用并实现 OperationId 偏移支持；typed-state-guard findings=0；rustfmt/retired-surface 通过。 |
| `UI-007.3` 档案与同步操作阶段 typed 化 | 5 | `infiltrator-iced` nextest 524/524；ProfileState 10 项操作阶段转为显式 enum 并提供 typed helper，旧标志位清零；typed-state-guard findings=0；rustfmt/retired-surface 通过。 |
| `UI-007.2` 配置编辑异步阶段 typed 化 | 5 | `infiltrator-iced` nextest 524/524（run `3788e3b2-d0b1-42c1-a0c6-0b1cd17136c4`）；ConfigEditorState 17 项异步阶段转为显式 enum，is_loading_*/is_saving_* 清零；typed-state-guard findings=0；rustfmt/retired-surface 通过。 |
| `UI-007.1` 共享控件 transient operation enum | 2 | `infiltrator-bevy-widgets` nextest 552/552；`infiltrator-bevy-ui` 379/379；`infiltrator-contract` 379/379；TrayBadge 与 pull-to-refresh 改用 TrayBadgePhase 与 PullToRefreshPhase，旧字段/bool 搜索为零；typed-state 0、retired-surface 与 rustfmt 通过。 |
| `CORE-018.9` Core enum layout 与 Mihomo adapter Clippy 收口 | 5 | Core nextest `f3f2f0be-f482-488e-ba87-72690bff83ae`：2211 passed、9 skipped；最终 contract nextest `ac8b13fb-d2dd-4606-8eb0-f837dbde94f0`：379/379；contract/Mihomo API all-targets Clippy `-D warnings` 通过。boxed observation 的既有 JSON 形状与 round-trip 有 contract 回归；runtime-config apply 使用 typed request，stream state/transition 改为具名类型；Android/Composition/Iced/Bevy/Presentation consumers 完成 API 源码迁移。改动路径 typed-state 0、doc-governance/retired-surface/rustfmt/diff 通过。全仓 structure gate 仍被 1483 项既有 import-hygiene finding 阻断，全仓 typed-state 有 8 项未改 UI finding；未构建 UI 包/APK，用户确认暂无 ARM64 设备，实机证据保持开放。 |
| `CORE-018.8` Desktop Core composition typed input records | 3 | commit `e1d7a8b` 已推送 main；`infiltrator-desktop` nextest `ec62216c-05b4-48b3-a595-12e980861c98`：249/249；all-targets Clippy `-D warnings` 通过；typed-state scoped findings 0、doc-governance/retired-surface/rustfmt/diff 通过。Application/runtime/surface/switch composition 改用具名输入记录并保留 owner/scope/engine/failure 语义。未构建 UI 包/APK；用户确认暂无 ARM64 设备，设备验收继续留在对应任务实机条件。 |
| `BANDROID-041.3.2R.14.7` Runtime probe settings editor moves to Presentation | 2 | Core nextest `e994cb8c-3c44-4256-8dae-7b620bbb5e50`：789 passed、5 skipped；覆盖 draft validation、unavailable checkpoint recovery、write failure/retry、matching readback 与 stale receipt；Composition/Iced/Bevy imports 切至 Presentation，Application 根 crate 旧导出清零；Bevy request/field-projection 阶段改为 enum；changed-path typed-state 0、retired-surface/doc-governance/rustfmt/diff 通过。未构建 Iced/Bevy 包；ARM64 暂无设备，设备验收未声称完成。 |
| `BANDROID-041.3.2R.14.1` Profile edit session 迁至 Presentation | 3 | Core nextest `13f60e4f-c5c2-459b-b46b-1ba4fda660a6`：783 passed、5 skipped；覆盖 profile read/status、source-bound edit 与保存回执；迁移 reducers scoped typed-state findings=0，retired-surface/doc-governance/test-policy/fmt/diff 通过；全仓 typed-state 仍有 9 项既有 UI-phase findings；Iced/Bevy 包未构建，ARM64 真机无设备未验收。 |
| `BANDROID-041.3.2R.14.2` Subscription filter editor typed state 迁至 Presentation | 3 | 同批 Core nextest `13f60e4f-c5c2-459b-b46b-1ba4fda660a6`：783 passed、5 skipped；四项 reducer 行为覆盖 profile/document fence、dirty draft、invalid input、pending/receipt correlation 与失败重试；迁移 reducer scoped typed-state findings=0，retired-surface/doc-governance/test-policy/fmt/diff 通过；全仓 9 项既有 UI-phase findings 仍待收敛；未构建 UI 包、无真机验收。 |
| `BANDROID-041.3.2R.14.3` DNS leak action reducer 迁至 Presentation | 2 | 同批 Core nextest `13f60e4f-c5c2-459b-b46b-1ba4fda660a6`：783 passed、5 skipped；覆盖 stale token、重试准入、权限恢复和成功/失败终态；迁移 reducer scoped typed-state findings=0，retired-surface/doc-governance/test-policy/fmt/diff 通过；全仓 9 项既有 UI-phase findings 仍待收敛；未构建 UI 包、无真机验收。 |
| `BANDROID-041.3.2R.14.4` Doctor action reducer 迁至 Presentation | 2 | Core nextest `638ba305-6386-4995-9621-9b8fc458c2f1`：788 passed、5 skipped；覆盖 retry、stale token、成功/失败/取消；Iced Doctor busy flags 改为显式 operation enum，Bevy/Iced 旧 reducer imports 与 Application 导出清零；改动路径 typed-state/import findings=0，retired-surface/doc-governance/test-policy/fmt/diff 通过。全仓 typed-state 仍有 8 项既有 UI findings；全局结构门被 1476 项 import-hygiene 基线债务拦截；未编 UI 包，ARM64 暂无设备。 |
| `BANDROID-041.3.2R.14.5` Language choice/read phase 迁至 Presentation | 2 | 同批 Core nextest `638ba305-6386-4995-9621-9b8fc458c2f1`：788 passed、5 skipped；覆盖 unknown、read failure、pending、saved、rejected、readback mismatch 与 token exhaustion；Composition/Iced/Bevy consumers 改用 Presentation，Application 旧导出/import 清零；改动路径 typed-state/import findings=0，retired-surface/doc-governance/test-policy/fmt/diff 通过。全局结构门受 1476 项既存 import-hygiene 债务阻断；未编 UI 包，ARM64 暂无设备。 |
| `BANDROID-041.3.2R.14.6` Proxy group order editor 迁至 Presentation | 2 | 同批 Core nextest `638ba305-6386-4995-9621-9b8fc458c2f1`：788 passed、5 skipped；覆盖 inventory revision fence、重复身份、pending、拒绝/重试、成功与 stale receipt；Bevy 重复 reducer 和 Option request tuple 改为共享 editor 与 typed request enum；改动路径 typed-state/import findings=0，retired-surface/doc-governance/test-policy/fmt/diff 通过。Iced/Bevy 包级验证遵守 Core-first 门，ARM64 暂无设备；全局结构门受 1476 项既存 import-hygiene 债务阻断。 |
| `BANDROID-041.3.2R.13` 共享只读投影迁出 Application | 5 | Core nextest `efc5bc96-56fe-401d-8ff7-0a37adb77251`：783 passed、5 skipped；覆盖 Presentation/Application/Composition 与 capture-fixtures；typed-state 0、retired-surface 与 doc governance 通过；未构建 Iced/Bevy UI 包。 |
| `SLINT-003` Age 凭据 Core owner 与安全存储 | 3 | Core packages nextest `96806302-8ec2-4f14-a411-541b6df86c45`：1050 passed、0 skipped；focused profile credential nextest `aa690e5a-8b6e-48d4-a2f3-51d5c13d2cad`：5/5；typed-state findings 0、doc governance 与 retired-surface guard 通过；Slint consumer/unsupported 文案迁移另列 SLINT-014，package verification 留 CORE-UI-001 |
| `CORE-020.1` Age 加密订阅导入与刷新 | 5 | Core nextest `537298e3-c8f0-4ae1-98bf-62cf14ad32f8`：1060/1060、0 skipped；覆盖 Mihomo/sing-box 解密、首次预置密钥导入、刷新、缺 key/错 key/损坏内容无写入、Profile 删除 tombstone 与失败回滚；typed-state 0、doc-governance、retired-surface、license 与 rustfmt 通过；Slint consumer migration 为 SLINT-014，package verification 留 CORE-UI-001 |
| `CORE-020.2` Age owner readback returns exact active-profile selection | 3 | Core nextest `29bd657b-9053-426d-b976-267835be958f`：1226 passed、2 skipped，覆盖 contract、ProfileStore、Application 与 Android owner wire；Read metadata 退役 UI 提交的 selection，owner-fenced readback roundtrip 携带精确 selection/revision；typed-state guard 0、doc governance、rustfmt 与 diff check 通过；未构建 UI 包/APK，ARM64 Binder 证据留过去佛 BANDROID-040.8 实机条件（`-D`） |
| `CORE-020.3` Desktop Age operations use the current product owner | 3 | Core/Desktop host nextest `018f33d2-ca74-4d56-9ff9-32928e4fdf88`：1282/1282；DesktopProductSession binding uses the backend's same-store ProfileApplication and replaces it on successful switch/recovery, verified with fake owner stores; Age set/clear/stale selection/wrong authorization covered by ProfileStore nextest. typed-state changed paths 0, doc governance, retired-surface, rustfmt/diff passed. No Iced/Bevy/Slint package, APK or device build. Clippy debt was registered in CORE-018.9/.8; `.9` is closed and Desktop composition remains in `.8`; ARM64 Binder evidence remains with BANDROID-040.8 device conditions. |
| `BANDROID-041.3.2A.1` Application capture feature 去除 Tokio runtime edge | 1 | Application capture-fixtures nextest `2040dafc-bfc1-4e85-9408-1bdec0cee5b4`：613/613、0 skipped；default/capture normal dependency tree 均无 Tokio；typed-state 与 doc-governance 通过，retired-surface passed，rustfmt 与 diff check 通过；父项 BANDROID-041.3.2A 继续迁移 fixtures/codecs |
| `BANDROID-042.1` VPN Service 单写者有界 admission | 2 | `:core-host-tests:test --rerun-tasks`（2026-10-13）：108/108；覆盖队列饱和、销毁与清理优先级 |
| `BANDROID-042.2` 贯穿 deadline 与 owner 取消 | 2 | `:core-host-tests:test --rerun-tasks`（2026-10-13）：108/108；覆盖绝对 deadline、取消与迟到回执围栏 |
| `BANDROID-042.3` TUN readiness receipt 与 Running 发布 | 2 | Rust nextest `cc3c0959-7e30-467e-a1a5-15eca68fca48`：2673 passed、6 skipped；`:core-host-tests:test --rerun-tasks`：108/108 |
| `BANDROID-042.4` Routing activation lease 发布门 | 2 | Rust nextest `cc3c0959-7e30-467e-a1a5-15eca68fca48`：2673 passed、6 skipped；`:core-host-tests:test --rerun-tasks`：108/108；设备证据仍按对应任务实机条件开放 |
| `BANDROID-036.1` typed 独立状态与默认暂停语义 | 2 | Rust nextest `cc3c0959-7e30-467e-a1a5-15eca68fca48`：2673 passed、6 skipped；`:core-host-tests:test --rerun-tasks`：145/145（commit `b74e435`），含 snapshot projection |
| `BANDROID-029.1` 跨端独立运行状态投影 | 2 | Contract/Application nextest `9cc0b40f-843e-4d2a-95aa-8fee1270b08e`：1037/1037；Android/composition nextest `a62d1717-5d4d-4c2a-80cc-55fc803b6dad`：234/234、7 skipped；`:core-host-tests` 183/183；doc governance OK、typed-state changed-path 0、Mihomo evidence Python 10/10；Android 设备与 UI 消费者适配仍未验收，详见 `.evidence/BANDROID-029.1-runtime-status-20261013.md` |
| `BANDROID-042.7` 专用 typed VPN Stop IPC | 2 | Rust nextest `cc3c0959-7e30-467e-a1a5-15eca68fca48`：2673 passed、6 skipped；host JVM 145/145，含 stop terminal/readback receipts |
| `BANDROID-042.8` Revoke/cleanup 可恢复终态 | 3 | Rust nextest `cc3c0959-7e30-467e-a1a5-15eca68fca48`：2673 passed、6 skipped；host JVM 145/145，含 Stop/Revoke、queue rejection、resource retry 与 generation fence |
| `BANDROID-037.1` prepare 往返与通知顺序源码收口 | 2 | `:core-host-tests:test --rerun-tasks`：154/154（commit `93a7401`）；Android manifest contract guard 通过；授权成功/拒绝、通知拒绝与 owner readback 顺序回归 |
| `BANDROID-036.2` 五类 Core/VPN command typed 终态 readback | 2 | Rust nextest `cc3c0959-7e30-467e-a1a5-15eca68fca48`：2673 passed、6 skipped；`:core-host-tests:test --rerun-tasks`：156/156（commit `cbb1a87`；intent wire、authorization/start readback、typed stop receipt）；设备操作按对应任务实机条件开放 |
| `BANDROID-042.5` 单次冻结 VPN 配置事务 | 2 | Rust nextest `cc3c0959-7e30-467e-a1a5-15eca68fca48`：2673 passed、6 skipped；`:core-host-tests:test --rerun-tasks`：166/166（commit `d418689`），含 typed prepared configuration 与一次性消费 |
| `BANDROID-042.5b` TUN/DNS 设置 revision fence | 2 | Rust nextest `cc3c0959-7e30-467e-a1a5-15eca68fca48`：2673 passed、6 skipped；host JVM 166/166（commit `d418689`），覆盖写后 revision 前进与旧 frozen config 零副作用 |
| `BANDROID-003.1` Bevy flavor 接入 NativeActivity driver | 1 | 2026-10-10 两 ABI + APK 本机构建记录 |
| `BANDROID-015.1` 两 flavor Gradle 产品结构 | 2 | 2026-10-10 debug APK/单测/AndroidTest APK 编译 |
| `BANDROID-018.1` 高成本 CI tag-only 触发门禁 | 2 | YAML 六文件解析；CI trigger guard + Python 5/5；Apple deferral guard + Python 3/3；当前 `main` SHA 无 Actions run |
| `BANDROID-019.1` manifest contract guard 接入 | 1 | 结构门 + 11/11 专项正反例 + asset guard 6/6 |
| `BANDROID-021.1` 两 flavor JVM 单测本机执行 | 1 | Compose/Bevy suites 各 102/102（多次记录） |
| `BANDROID-022.1` AndroidTest APK 组装 | 1 | 2026-10-11 lease 切片记录 |
| `BANDROID-026.1` typed 滚动状态机 | 2 | widgets 256/256、UI 544/544、workspace 4483（`272d97ba`） |
| `BANDROID-038.1` identity registry 与守卫 | 2 | 定向 17/17、Python 44/44、packaging 19/19、双 APK aapt |
| `APPID-001.1` 产品身份登记与全平台守卫 | 3 | nextest 1981/1981、packaging 19/19、双 APK aapt |
| `BANDROID-040.1` 非 owner 初始化守卫 | 1 | `shared_core()` owner 检查（运行时证据在实机看板） |
| `BANDROID-040.2` typed IPC 基础契约 | 3 | protocol/request ID/session/generation/deadline Messenger |
| `BANDROID-041.1` host→UI 依赖方向整改 | 2 | host/FFI 不再依赖 bevy-ui；capture-controller 显式 feature |
| `BANDROID-041.2.1` driver feature 隔离 | 2 | `SurfaceComposition` 枚举组合；Android target cargo tree 核查 |
| `BANDROID-041.3.1` Bevy desktop composition 移出 UI crate | 2 | Host nextest `99dfb42c-b754-41a0-835f-869744314304` 1/1；UI nextest `ef586ab9-d1ab-4143-92d7-9eb10159fbdc` 699/699；ARM64 driver `cargo check` 通过；host Clippy、typed-state/doc/结构/retired-surface 门通过 |
| `BANDROID-041.3.2.1` 共享 byte formatter | 1 | `infiltrator-shared::byte_units`；旧模块删除 + 守卫 |
| `BANDROID-041.3.2.2` 共享 presentation crate | 2 | nextest/Clippy/结构门通过 |
| `BANDROID-041.3.2.3` ShellObservation 显式 enum | 2 | 2344 项受影响包回归（`f09bcff7`）；HostProtocol V6 |
| `BANDROID-042.6` 内核 spawn/API readiness 分离 | 2 | application 633/633 历史切片；设备验证在实机看板 |
| `CORE-007.1` 身份契约与桌面 scope fence | 2 | Contract/Bevy 4/4、routing 1/1、Bevy 704/704、Iced 583/583、workspace 4483 |
| `CORE-008.1` evidence lock 与 capability provider | 3 | v1.19.18/v1.19.32 SHA 锁定；未知/无版本/prerelease fail closed |
| `CORE-008.2` 双版本 direct+typed real-process smoke | 3 | run `6e8c801e`/`8f2726f5`、`d99b30f4`/`aaeac5b5` |
| `CORE-008.3` Logs operation family conformance | 3 | adapter `96f85ae3-6d29-486f-9c53-90916d8df181`/`c2ed93ff-92ea-4555-abec-4aefc3c18e37`; scope regression in nextest `6e33cf86-2b6e-4835-992b-d33157132a43`; target-scoped `logs.stream` evidence |
| `CORE-008.4` operation family capability provider registration | 3 | Core nextest `46084d73-e35a-4eb3-838e-72705eadc3e2`：3082 passed、5 skipped；application/command capability matrix、OwnerManaged authority 与 Android composed-port fakes 通过；typed-state guard 0、retired-surface guard passed |
| `CORE-008.5` operation evidence 与宿主/构建目标绑定 | 5 | Core nextest `46084d73-e35a-4eb3-838e-72705eadc3e2`：3082 passed、5 skipped；Android owner nextest `42579b22-a5c2-48e3-9108-128cd93011ce`：161/161；Core-only Android `cargo check` 对 `aarch64-linux-android` 与 `x86_64-linux-android` 均通过；目标/adapter mismatch 仍 fail closed。设备验收留在 `BANDROID-040.7-D` |
| `CORE-008.6` Mihomo DNS query 双版本真实 conformance | 3 | 两个 SHA 锁定 live Mihomo 进程均通过 `/dns/query` 与 typed `DnsQueryPort`，A `core-smoke.test` → `192.0.2.42`；v1.19.32 nextest `576168d3-53cd-4e53-a589-01cde7a39ac8`，v1.19.18 `0ad9ef53-3911-45d4-bfd2-bf69afc6bd4c`；focused capability nextest `49583c4c-a33d-4146-af4f-608a861cba95` 381/381、1 skipped；non-UI unified nextest `ca741c0f-4f37-4c8a-ba3f-63f4cf3b2f83` 3212/3212、8 skipped；evidence guard 9/9；仅 Linux amd64 / revision 1 / v1.19.18 与 v1.19.32 授权 |
| `CORE-008.7` Mihomo active Connection.Close 双版本真实 conformance | 3 | superseded: legacy unscoped gateway entry retired; current typed `ConnectionPort::observe/close` live revalidation passed on v1.19.32 `476fe72f-e109-4534-9b2b-9ac6a40fa17a` and v1.19.18 `6f034e1d-2b4b-4c7d-b716-bbda198f5185`; Core unified nextest `2f61e785-3236-4c36-915d-bac32ec15fc0` 2396 passed、4 skipped；only Linux amd64 / adapter revision 2 / exact locked versions authorized; Connection.List remains Unsupported |
| `CORE-008.8` Mihomo FakeIpCache.Flush 双版本真实 conformance | 3 | 两个 SHA 锁定 controller 均接受 scoped `ControllerFakeIpCachePort` flush 并返回匹配 scope/operation 的 typed receipt；v1.19.32 nextest `f03b17e0-1ed2-4700-b35a-617c41b2036c`，v1.19.18 `ce963eae-46ab-4d85-9e75-cc260dfbfb18`；non-UI unified nextest `ca741c0f-4f37-4c8a-ba3f-63f4cf3b2f83` 3212/3212、8 skipped；evidence guard 9/9；仅 Linux amd64 / revision 1 / 两个精确版本授权，不证明缓存内容读回为空 |
| `CORE-008.9` Mihomo rule-provider List/Update 双版本真实 conformance | 3 | v1.19.32 live HTTP fixture `ruleCount=1→2`，typed list/update/readback nextest `ce8e3858-508c-43ac-be3d-87d5a4d74925`；v1.19.18 `47ce3a57-c6d1-4042-8b31-0d24c4faed32`；wire DTO nextest `27a14c80-c7b8-4a20-9cd8-da542200abbb`；non-UI unified nextest `ca741c0f-4f37-4c8a-ba3f-63f4cf3b2f83` 3212/3212、8 skipped；仅 Linux amd64 / revision 1 / 两个精确版本授权，proxy-provider operations 仍 Unsupported |
| `CORE-008.10` Mihomo CoreGarbageCollection.Trigger 双版本 conformance | 2 | SHA-locked v1.19.18/v1.19.32 启动时 Debug real-process smoke `f8e04d65-fb62-4f79-b02c-0eb0eaed1e93` / `3b26f83a-1720-4c0c-add9-05de91c9dd7b`；统一 nextest `5613c06a-c03c-4601-9540-044446720046` 3214 passed、8 skipped；404→typed Unsupported，未声称 GC 内存效果读回 |
| `CORE-008.11` Mihomo ApplyDnsSettings 双版本真实 conformance | 3 | 双版本 real-process typed DNS transaction：v1.19.32 `9e3da32d-1cc7-464f-8ad2-19f4ff1ae231`、v1.19.18 `10ac4134-6755-491d-9663-92e652ad582b`；受控 DNS `192.0.2.53→192.0.2.42` 且原 workspace 字节恢复；domain DNS codec nextest `932048d4-c833-4f30-8942-1501ea47b497`；统一 nextest `70c5ea2a-7a78-41a3-9c3f-a89a9ca88243` 3218 passed、8 skipped |
| `CORE-008.12` PreferReload fallback 保留双重失败 cause | 2 | typed `ApplyError::ReloadAndRestartFailed` 回传 reload 与 restart 原因、且精确 rollback；focused nextest `07af92d8-5eb4-4b21-bd56-66239ef2d8d3`；统一 nextest `70c5ea2a-7a78-41a3-9c3f-a89a9ca88243` 3218 passed、8 skipped |
| `CORE-008.13` Mihomo Rules.Apply 双版本真实 conformance | 3 | v1.19.32/v1.19.18 real-process Profile smoke `9e3da32d-1cc7-464f-8ad2-19f4ff1ae231` / `10ac4134-6755-491d-9663-92e652ad582b`；typed `RuleEntry` 真实 reload 后 `/rules` 精确读回新增规则，原 workspace 字节恢复；统一 nextest `70c5ea2a-7a78-41a3-9c3f-a89a9ca88243` 3218 passed、8 skipped |
| `CORE-008.14` Mihomo `/rules` signed size wire compatibility | 2 | `/rules` 实际返回 `size=-1`；`Rule.size` 保留 signed wire 值，正数与 `-1` 单测及双版本实时 `/rules` 读回通过；统一 nextest `70c5ea2a-7a78-41a3-9c3f-a89a9ca88243` 3218 passed、8 skipped |
| `CORE-008.15` Mihomo TUN live configuration 双版本 conformance | 2 | v1.19.32/v1.19.18 真实进程通过 `RuntimeConfigurationApplication` 写入、读回并恢复 `system → mixed → system`，全程 `tun.enable=false`；nextest `266b6d65-fb2c-4b73-bd91-f119b7423028` / `c9a16fec-b62e-4c66-9673-c46ed9896922`；evidence revision 2；fake/adapter nextest `ccc890e3-18a1-435c-9862-32bcbdbb72b4` 9/9、统一 nextest `4c298ab9-2610-45bc-bb52-e9b611477f4c` 3218 passed/11 skipped |
| `CORE-008.16` Mihomo Rules.Trace 双版本真实 conformance | 3 | v1.19.32/v1.19.18 real-process nextest `f2f24a6d-d409-42a4-a23c-c7bfa68ae6a2` / `2d637629-c8b2-43f9-8984-96787c869604`；production `ProfileApplication`/Mihomo YAML codec 读取运行中 profile，live outbound observation 解析 `DOMAIN` 命中与 `CORE-SMOKE-HTTP` opaque ID；只读 trace，不覆盖反向应用/UI ingress；统一 nextest `4c298ab9-2610-45bc-bb52-e9b611477f4c` 3218 passed/11 skipped；evidence revision 2 |
| `CORE-008.17` Mihomo Profile.ApplyFeature 双版本真实 conformance | 2 | v1.19.32/v1.19.18 real-process nextest `5ae2637c-f3b0-4d5e-8025-5346b720b43d` / `9fea92e1-629e-4fd0-a571-be0cef4a7ee9`；`RuntimeConfigurationApplication` 验证 IPv6 `false → true → false` scope/revision/readback，`tun.enable=false`；统一 nextest `4c298ab9-2610-45bc-bb52-e9b611477f4c` 3218 passed/11 skipped；evidence revision 2 |
| `CORE-008.18` Mihomo subscription Import/Update/Refresh 双版本真实 conformance | 3 | v1.19.32/v1.19.18 SHA-locked nextest `ed80c380-90b8-4491-a4ab-6426c484d44c` / `6581de2d-2fcc-4046-8997-8bff8072993b`；loopback-only production `HttpSubscriptionSource`、typed reports、ETag 条件更新、同一 engine-scoped workspace 与逐阶段 Mihomo `-t`；Core/contract/API nextest `93926a2c-6f7b-4cfe-8deb-e4bdf2c07fab` 768 passed/6 skipped；evidence revision 2；详见 `.evidence/CORE-008.18-subscription-import-20261013.md` |
| `CORE-008.19` Mihomo Geo.UpdateDatabases 双版本真实 conformance | 3 | v1.19.32/v1.19.18 SHA-locked nextest `2df5d774-f075-4870-a93a-5b8576ae4d5d` / `2f37826c-78c8-4d42-826e-13c4c502df1d`；production client 触发均被接受，四个 `geox-url` 仅指向 loopback；证据只表示异步触发接受，不声称下载/安装完成；Core/contract/API nextest `93926a2c-6f7b-4cfe-8deb-e4bdf2c07fab` 768 passed/6 skipped；evidence guard self-test、10/10 Python tests、doc-governance、shell syntax 通过 |
| `CORE-009.1` ProfileDocument raw-byte CAS 主路径 | 3 | 3314/3314 多端 unified nextest |
| `CORE-009.2` WebDAV/local profile namespace 与安全迁移 | 3 | Core nextest `241bb2b6-895c-4a19-a501-68b9301e8c0f` 2822 passed、5 skipped，含 Fake WebDAV 同名隔离、unknown marker、强 ETag、copy/readback/delete 恢复；DAV/admin/CLI adjunct nextest `1cf3a659-bf6b-4001-8f34-b65a38af25d8` 178/178 |
| `CORE-009.3` 写保护与精确回滚验收 | 2 | Core nextest `241bb2b6-895c-4a19-a501-68b9301e8c0f` 2822 passed、5 skipped；覆盖 absent/empty 区分、foreign/stale 零写、逐字节 sidecar 回滚；旧 `allow_*: bool` 声明搜索无匹配 |
| `CORE-010A` engine/format/schema codec port | 3 | 3314/3314 多端 unified nextest |
| `CORE-010B.1` YAML snippet 扩展迁入 Mihomo adapter | 2 | Core nextest `241bb2b6-895c-4a19-a501-68b9301e8c0f` 2822 passed、5 skipped；旧 application YAML patch 符号搜索无匹配 |
| `CORE-010B.2` Mixin/filter sidecar codec seam | 2 | Core nextest `241bb2b6-895c-4a19-a501-68b9301e8c0f` 2822 passed、5 skipped；foreign engine/format 与 workspace CAS 行为通过，application production 源无 Mihomo codec 依赖 |
| `CORE-010B.3` ConfigurationApplication patch 退役与 Android owner codec 接入 | 3 | Core nextest `46084d73-e35a-4eb3-838e-72705eadc3e2`：3082 passed、5 skipped；codec/application/Android fake 覆盖零写与读回；旧 patch API 与 application/Android rule-workspace YAML parser 生产搜索清零 |
| `CORE-010B.4` ProtocolCodec profile YAML edits 迁入 engine codec | 3 | Core nextest `241bb2b6-895c-4a19-a501-68b9301e8c0f` 2822 passed、5 skipped；foreign/stale/invalid 零写、精确读回通过，production application 源无 YAML/Mihomo codec 直连 |
| `CORE-010B.5` CertificateAuthority profile YAML edits 迁入 engine codec | 3 | Core nextest `241bb2b6-895c-4a19-a501-68b9301e8c0f` 2822 passed、5 skipped；production application 源无 YAML parser/patch 直连，sing-box 未组合能力保持 Unsupported |
| `CORE-010B.6` Mihomo 活动配置事务与 endpoint lease 顺序 | 3 | Core/host nextest `6e33cf86-2b6e-4835-992b-d33157132a43` 795 passed、4 skipped；v1.19.32/v1.19.18 profile apply/readback/restore `626fc06a-4fa4-4af1-837c-9149c2a7d783`/`937ba089-9de1-4718-af9d-1d61563306e8`; retired API, evidence, declaration and doc guards pass |
| `CORE-011 typed 收口（probe 弹窗/回执枚举）` | 2 | run `10468606` 1/1、`67dd29f7` 600/600、`538fe595` 1/1 |
| `CORE-011/012-2A` Core-owned speedtest cancellation and completion fencing | 3 | Core nextest `46084d73-e35a-4eb3-838e-72705eadc3e2`：3082 passed、5 skipped；typed-state guard 0、retired-surface guard passed；旧 SpeedtestPort/runtime field/no-run-id cancel 搜索清零 |
| `CORE-011/012-4` 配置读取/写入窄端口 | 3 | Core nextest `46084d73-e35a-4eb3-838e-72705eadc3e2`：3082 passed、5 skipped；scope/revision/Unsupported 零写入行为通过；typed-state scoped guard 0、retired-surface guard passed；旧 `get_config/patch_config/get_rules` 生产接口/消费者搜索清零 |
| `CORE-011/012-3` 旧 proxy map/page snapshot 收口 | 2 | Core nextest `46084d73-e35a-4eb3-838e-72705eadc3e2`：3082 passed、5 skipped；Application surface reader/stale revision 行为通过；消费者只使用 `OutboundId`+inventory revision typed preference |
| `CORE-011/012-3A` RuleTracer 与旧 proxy surfaces 退役 | 3 | Application nextest `f93f2e3c-1a00-413e-b07a-94a9fd2b431b`：667/667；retired-surface guard passed；RuleTracer/Outbound consumer 相关 typed-state guard 0；旧 `ProxiesPageSnapshot`/`get_proxies`/name-keyed map production 搜索清零 |
| `CORE-011/012-1` Probe 设置与持久化排序收口 | 2 | Core nextest `46084d73-e35a-4eb3-838e-72705eadc3e2`：3082 passed、5 skipped；typed-state scoped guard 0、retired-surface guard passed；settings migration/invalid/no-settings zero-probe 行为与旧 option/snapshot/command/default 退役验证通过 |
| `CORE-011/012-5` controller Fake-IP flush typed 收口 | 2 | Core nextest `46084d73-e35a-4eb3-838e-72705eadc3e2`：3082 passed、5 skipped；旧 `RuntimeGateway` flush 与公开 `MihomoApi` 方法搜索清零；retired-surface guard passed |
| `CORE-011/012-8` Admin 公开 wire 退役收口 | 2 | Core nextest `46084d73-e35a-4eb3-838e-72705eadc3e2`：3082 passed、5 skipped，含旧路由移除、typed 选择/readback、stale/name-only 与 webhook 负测；retired-surface guard passed |
| `CORE-011/012-9` System DNS cache flush typed outcome | 2 | Core nextest `46084d73-e35a-4eb3-838e-72705eadc3e2`：3082 passed、5 skipped；typed-state guard 0、旧 `Result<bool>` 契约搜索清零 |
| `CORE-011/012-7A` standalone profile applier 与 EvidenceOpen fail-closed contract | 3 | Core-only nextest `abc9f21b-9473-4b43-bb11-477d8e6e16a9`：3005 passed、13 skipped；changed-path typed-state guard 0、adapter declaration/Mihomo evidence/doc-governance/retired-surface guards 通过；旧 `runtime_gateway::ActiveProfileReferenceApplier` 路径清零 |
| `CORE-011/012-7B.1` ResourceApplication scope/resource port injection | 2 | Core nextest `20e4db33-75dc-4357-ac6c-8631db3d0343`：3006 passed、13 skipped；Unavailable owner scope 在 resource adapter 调用前拒绝；typed-state changed-path guard 0、doc governance 与 fmt/diff 检查通过 |
| `CORE-011/012-7C.1` retire unscoped name-based proxy facade | 2 | Core nextest `8b55b401-8511-4887-a545-b75d84ff20ee`：1340 passed、9 skipped（含 `--all-targets` smoke 示例编译）；retired-surface guard 通过，`ProxyManager`/宽泛 `MihomoApi`/RuntimeGateway switch-probe/单节点 lookup 旧入口清零；changed-path typed-state 0、doc governance、fmt/diff 通过 |
| `CORE-011/012-7C.2` scope-bound rule-provider operations and old DTO retirement | 3 | Core-only nextest `f2ec4c8e-1cdf-4396-9623-214740be4974`：2,886 passed、13 skipped；Mihomo v1.19.32/v1.19.18 real-process nextest `f7ca5d21-ce14-4f04-8cc9-97577835ef8f` / `fc28b4f6-6d91-440c-9718-dfaf773c8216`；foreign/stale/Unsupported fencing、readback、retired-surface 与 evidence guards 通过 |
| `CORE-011/012-7C.3` scope-bound Geo database trigger | 2 | Core-only nextest `f2ec4c8e-1cdf-4396-9623-214740be4974`：2,886 passed、13 skipped；Mihomo v1.19.32/v1.19.18 real-process nextest `6db834c2-3faa-473a-9fcf-d3e1968795ff` / `fed020e5-c789-4dd7-8f08-44fce6991960`；trigger receipt is Accepted only, not download/install; sing-box Unsupported and guard checks pass |
| `CORE-011/012-7D.1` remove RuntimeGateway lookup from application services | 3 | Core/host nextest `88ab01ba-9802-4049-a77b-a6aeae9a5873`：1,180 passed、7 skipped；focused command dispatch nextest `f4ec7907-617d-4119-91ba-9daae77a7b71`：1/1；application production source search found no `RuntimeGateway`/`with_runtime`/`with_gateway`; fmt/diff passed |
| `CORE-011/012-7D.2` typed ports at product composition | 3 | Core/host nextest `5c2eefb1-b31c-43b7-af87-22bb4b330dab`：1329 passed、9 skipped；Android/Desktop/composition/iOS-host source composition no longer creates gateway fakes; later D4 closed the HostRuntime consumer seam |
| `CORE-011/012-7D.3` Admin and CLI typed consumer migration | 3 | Core nextest `3a2a63d7-130d-423a-8c22-93dec7774ce9`：1302 passed、7 skipped；Admin/CLI source search clean; typed owner-scope use cases pass |
| `CORE-011/012-7D.4` Iced command, subscription and admin typed port migration | 3 | Core nextest `3a2a63d7-130d-423a-8c22-93dec7774ce9`：1302 passed、7 skipped; Iced source consumes direct typed ports; UI package build remains deferred to the unified UI phase |
| `CORE-011/012-7` retire generic runtime facade and stale descriptions | 2 | Core nextest `3a2a63d7-130d-423a-8c22-93dec7774ce9`：1302 passed、7 skipped；retired-surface, adapter-declaration and doc-governance guards pass; production/public-doc search clean |
| `CORE-012A` CoreLogLevelPort 退役 JSON patch | 2 | 双版本 Profile/Outbound/RoutingIntent/CoreLogLevel real-process smoke |
| `CORE-013.1` version DTO/factory/版本目录迁移 | 3 | 1195/1195 unified、Iced 572/572 |
| `CORE-013.2` composition version seam 全量核验 | 2 | Core nextest `241bb2b6-895c-4a19-a501-68b9301e8c0f` 2822 passed、5 skipped；Desktop/Android host composition 回归通过，产品入口通过 engine-keyed factory 选择版本 |
| `CORE-013.3` EngineBackendFactory composition contract | 3 | Core nextest `241bb2b6-895c-4a19-a501-68b9301e8c0f` 2822 passed、5 skipped；未知 engine 零副作用、Android provider scope 与 Desktop runtime registry 回归通过 |
| `CORE-013.4` Mihomo native profile transaction typed port | 5 | Core nextest `241bb2b6-895c-4a19-a501-68b9301e8c0f` 2822 passed、5 skipped；Mihomo adapter fake run `fb283f19-7190-40a9-bb12-5b137958a536` 397/3；SHA-locked dual-engine provider transaction suite `a7e148c8-4bd5-48b5-9560-271d01ec88e4` 2/2 |
| `CORE-014.1` 中性层依赖守卫 + contract suite | 2 | 结构门正反例 |
| `CORE-014.2` adapter operation capability declaration contract suite | 2 | Core nextest `241bb2b6-895c-4a19-a501-68b9301e8c0f` 2822 passed、5 skipped；declaration guard 12/12；supported/evidence-open/unsupported/owner-managed 声明正反例通过 |
| `CORE-014.3` test-only Tokio attribute false-positive removal in Core boundary guard | 1 | `core-boundary-guard.py --self-test` passed; report no longer treats `#[tokio::test]` as a production ports dependency; runtime Tokio imports remain detected |
| `CORE-015.1` sing-box 官方版本与平台资产锁 | 1 | GitHub Release API v1.14.3/4 平台 SHA-256 全匹配；Linux x86_64 archive 本地复算一致；真实二进制 check/run/`/version`/cleanup 通过（适配器 nextest 另属 CORE-015.2） |
| `CORE-015.2` sing-box process/readiness/native JSON config adapter | 3 | singbox-adapter nextest 在 Core batch `241bb2b6-895c-4a19-a501-68b9301e8c0f` 通过；官方 SHA-locked v1.14.3 process smoke `c40baec6-25a9-4412-9a05-8d767ce9eed5` 1/1，含 native check/readback、start/stop、`/version` readiness |
| `CORE-015.3` sing-box typed selector/version adapter | 2 | Core batch nextest `241bb2b6-895c-4a19-a501-68b9301e8c0f` 通过；真实 selector PUT/readback/restore、mode-list observe 与 typed unsupported write smoke `c40baec6-25a9-4412-9a05-8d767ce9eed5` 1/1 |
| `CORE-015.4` Mihomo/sing-box 生产 Core backend conformance | 3 | commit `62af9c9`；focused Core fake suite `91abed5f-ac17-4006-844e-f09a590e037e` 2/2；SHA-locked production-provider real suite `a7e148c8-4bd5-48b5-9560-271d01ec88e4` 2/2；共用 EngineBackendProvider/typed transaction 与 owner engine+instance identity 验证通过 |
| `CORE-015.5` sing-box production backend owner composition | 5 | focused fake provider suite `91abed5f-ac17-4006-844e-f09a590e037e` 2/2；SHA-locked provider real suite `a7e148c8-4bd5-48b5-9560-271d01ec88e4` 2/2；Core batch nextest `241bb2b6-895c-4a19-a501-68b9301e8c0f` 与 engine-scoped store integration `4d667a0a-4b33-44e6-8003-a2c875f2011d` 219/1；产品根采用仍属 `.13/.14` |
| `CORE-015.6` typed revision-fenced owner-switch transaction | 3 | Core/application/Android/composition nextest `f9fe8fa9-ae4e-43bd-bbcd-19564d98c279` 823/823；fake 覆盖 CAS/close/bind/provider failure 与候选 abort。Desktop 生产 provider/持久 store 真实进程验收由 CORE-015.13.3 归档；Android 产品根由 CORE-015.14 收口 |
| `CORE-015.7` Android owner shutdown 到 typed terminal close | 2 | Core nextest `46084d73-e35a-4eb3-838e-72705eadc3e2`：3082 passed、5 skipped；`:core-host-tests:test` 45/45 编译生产 host 源；owner fence/terminal close/reopen 行为通过。设备 death/rebind 独立留在对应任务实机条件 |
| `CORE-015.8` typed Core start context 端到端迁移 | 2 | Core nextest `46084d73-e35a-4eb3-838e-72705eadc3e2`：3082 passed、5 skipped；Android owner nextest `42579b22-a5c2-48e3-9108-128cd93011ce`：161/161；`:core-host-tests:test` 45/45；无参 `CoreProcess::start()` 生产搜索清零 |
| `CORE-015.8.1` persistent child identity and typed process observation | 3 | Core/Android nextest `cc3c0959-7e30-467e-a1a5-15eca68fca48`：2673 passed、6 skipped；`:core-host-tests:test --rerun-tasks`：52/52，含 process record、PID reuse、unavailable 与 owner gate cases。设备 `/proc` 与 service death/restart 仍留对应任务实机条件 |
| `CORE-015.8.2` exact Managed session proof before spawn | 2 | 同一 Core/Android nextest `cc3c0959-7e30-467e-a1a5-15eca68fca48`：2673 passed、6 skipped；`:core-host-tests:test --rerun-tasks`：52/52；typed admission、schema-v2 owner proof、legacy-unverified migration 和拒绝路径通过 |
| `CORE-015.8.3` restart recovery from exact child/session/VPN evidence | 3 | 同一 Core/Android nextest `cc3c0959-7e30-467e-a1a5-15eca68fca48`：2673 passed、6 skipped；`:core-host-tests:test --rerun-tasks`：52/52；恢复顺序与 child/VPN terminal fail-closed 通过；真机 `/proc` 和 service death/rebind 仍留对应任务实机条件 |
| `CORE-015.10` engine-keyed version backend registry | 2 | Core/Desktop nextest `5d3f01f9-a244-44da-8aa4-c1ebe2524a81` 501 passed、3 skipped；fmt 通过；未注册与 identity mismatch fail closed |
| `CORE-015.11` sing-box 生产 ProfileStore namespace 与 native transaction conformance | 3 | composition/mihomo-config/singbox-adapter nextest `4d667a0a-4b33-44e6-8003-a2c875f2011d` 219 passed、1 skipped；真实 engine-scoped ConfigManager + shared active-reference CAS 与 fake native validator 验证 Core engine/instance、workspace lease、JSON/sidecar/native file 精确读回；产品根接入仍属 `.13/.14` |
| `CORE-015.12` sing-box target-bound binary version lifecycle | 5 | singbox-adapter nextest `70b81a9d-a445-40d0-b3df-6e5f8524dedf` 32 passed、1 skipped；embedded asset lock、digest/version mismatch、target/revision journal、cancel cleanup/retry、activation/rollback；Desktop/Android registry 与 VersionPort 管理式 install/publication/rollback 仍未验证 |
| `CORE-015.13.1` Desktop engine capability/profile/version registry | 3 | Core/Desktop/mihomo-config nextest `69eb981a-26ed-4ac6-be34-586ddea6826a` 943 passed、2 skipped；engine-keyed capability/evidence dispatch、shared configs root sing-box JSON store、supported-target version registration、unsupported runtime preflight fail closed |
| `CORE-015.13.2` Desktop sing-box Core runtime composition | 3 | Desktop/Composition/singbox-adapter nextest `a56c88e1-e914-4f5d-bb1e-449b571fba0d` 303 passed、1 skipped；locked sing-box v1.14.3 process smoke `9678c2a4-f87a-4141-8e54-4137512950a7` 1/1；fake dual-engine conformance `e2dac952-b747-4ee5-a74a-f76fc1971032` 2 passed、2 skipped；real Mihomo + sing-box conformance `675b7a82-65ec-40d6-8630-3324eeb89cf1` both passed、2 skipped；Desktop scoped import guard 0、typed-state guard 0、doc governance OK; desktop package/device evidence remains open |
| `CORE-015.13.3` Desktop live engine owner switch | 3 | SHA-locked smoke script；real conformance nextest `e3d72863-5863-4e50-8d76-a248842a9dbc` 2/2；durable owner-switch nextest `65ea7d9c-29ea-4c76-9cd0-d4d028cbcbb2` 1/1；Mihomo v1.19.32 → sing-box v1.14.3 → Mihomo；first candidate dispatch failure recovered using exact old-owner terminal receipt; durable reference revisions 1→2→3 |
| `CORE-015.14` Android :vpn dual-engine profile owner replacement | 5 | Core/Android nextest `cc3c0959-7e30-467e-a1a5-15eca68fca48`：2673 passed、6 skipped；`:core-host-tests:test --rerun-tasks`：52/52；typed provider dispatch、revision CAS、old-owner terminal admission、candidate failure recovery 与版本 owner IPC 通过。app UI、APK、Binder/device evidence 仍留后续任务 |
| `CORE-016.1` 双引擎/Core evidence 文档同步 | 1 | commit `8e421f6` 更新三份 tracked docs；doc-governance enforce/self-test OK；旧 conformance OPEN 当前态描述搜索无命中 |
| `CORE-017.1` typed export review lifecycle | 2 | focused ScriptExportApplication nextest `fd05f157-956e-4ffd-8410-ce2ba75d7e3f`：14/14；Core batch `ae4c8bea-3b87-4c4c-ae51-51c475aafd03`：1053 passed、1 skipped；typed-state scoped guard 0 findings |
| `CORE-018.1` 双引擎 adapter crate owner 与依赖方向 | 1 | `python3 scripts/quality/doc-governance-guard.py --mode enforce`：OK；架构、Core 后端契约、功能 owner 表、文档地图与七个新职责 crate README 已登记 |
| `CORE-018.2` 共享 host adapters 与 Mihomo 进程拆分 | 5 | `cargo nextest` run `97851d80-376a-46c0-8bf0-fb37da84ae8c`：1209 passed、4 skipped；受影响包 `cargo check --offline --locked --all-targets` 通过；旧 package/symbol 搜索清零 |
| `CORE-018.3` ProfileStore 生产 owner 中性化 | 5 | Shared store/Desktop nextest run `2ef74b72-20b5-4538-95e6-97a6e5df6c51`：435/435；Desktop `a22932a6-7057-4cfb-b09f-396382e55cc2`：243/243；Desktop fmt 通过；`ConfigManager` 旧 `ProfileStore`/workspace 生产调用搜索为零 |
| `CORE-018.5` sing-box native config adapter 独立 crate | 3 | nextest run `026f7542-10fc-41e2-9ea6-149b5cacf141`：320 passed、4 skipped；`singbox-config` 直接持有 JSON codec、native CLI check/file readback 和 revision-fenced transaction；旧 `singbox-adapter` package/path 搜索为零 |
| `CORE-018.4` sing-box process adapter 独立 crate | 3 | 同一 nextest run：1209 passed、4 skipped；`singbox-process` 命令、进程与 gate package 通过 all-target check，composition/conformance 消费者已迁移 |
| `CORE-018.6` sing-box controller API adapter 独立 crate | 3 | 同一 nextest run：1209 passed、4 skipped；adapter declaration guard 与 `singbox-api`/composition all-target check 通过 |
| `CORE-018.7` sing-box version adapter 独立 crate | 3 | 同一 nextest run：1209 passed、4 skipped；目标锁、安装/激活/回滚行为与 Desktop/Android consumers 的 all-target check 通过 |
| `CORE-019.1` Iced typed Core consumers 与 DesktopProductSession typed close | 3 | nextest `bfe19acc-ed7c-4836-a881-8f01e4250152`：Iced/Bevy/widgets/Slint 1681/1681；收口后 Iced+Slint `9b79a601-f1f9-4799-8aa4-c810dc450c74`：582/582；改动路径 typed-state guard 0；旧 rebuild result/message 与演示入口路径已退役 |
| `CORE-019.2` Bevy typed Core consumer migration | 2 | nextest `bfe19acc-ed7c-4836-a881-8f01e4250152`：1681/1681；定向 cross-surface batch `dfa80876-cfd8-4551-ad76-bfbf0575fa87`：52/52；改动路径 typed-state guard 0；profile/editor/demo fixtures 注入当前 typed contract |
| `CORE-019.3` Slint LogStreamState exhaustive projection | 1 | nextest `9b79a601-f1f9-4799-8aa4-c810dc450c74`：582/582；新增 `Unsupported`/`Cancelled` 简中与英文投影行为测试，日志状态 match 穷尽 |
| `CORE-019.4` Default Core graph excludes capture-fixture support | 1 | commit `46c822f`; Android/Composition default-feature nextest `9fc51584-c091-4c96-b44e-8d5b1d63a963`：562 passed、2 skipped；capture-fixtures nextest `3d0f4356-8800-43c4-b95f-737e1d265109`：675 passed、5 skipped；fixture-only imports are feature-gated; fmt/diff and changed-path typed-state, doc-governance, retired-surface guards pass |
| `BANDROID-004.3` Android owner-authoritative SurfaceSnapshot route | 3 | Android owner nextest `46183b72-bae1-46bf-bc96-3cbad2a48e28`：189 passed、2 skipped；UiClient 原样转发 owner JSON/revision、失败投影 FFI `NotReady` 回归通过；owner route guard self/current 与 unittest 4/4；doc governance、changed-path typed-state、retired-surface、VPN lifecycle、Bevy host boundary、manifest guard、fmt/diff check 通过。全量结构入口被既存 import hygiene 1,481 项债务提前拦截，归 CI-QUALITY-001。未运行 Gradle/UI 包或 APK/设备；Compose 轮询归 BANDROID-043.6，设备 IPC/readback 归对应任务实机条件。 |
| `BANDROID-041.3.2R.4.7` 日志原始事实与展示解析分离 | 3 | Core nextest `79c07e18-fd0f-4a97-acc5-345591f82328`：1115 passed、5 skipped；typed-state changed-path 0、doc-governance 与 retired-surface guard 通过；UI 包构建留 Core-first 阶段门 |
| `BANDROID-041.3.2R.4.8` Profile、quota 与 failure 投影归入 presentation | 3 | Core nextest `2909093c-1c71-44ce-9cda-57487a042309`：1245 passed、2 skipped；changed-path typed-state 0、doc-governance 与 retired-surface guard 通过；application 正常依赖无 presentation 边、旧投影路径清零；未构建 UI/APK，ARM64 暂无设备 |
| `BANDROID-041.3.2R.4.1` DNS leak 展示投影迁入 presentation | 2 | Core nextest `2909093c-1c71-44ce-9cda-57487a042309`：1245 passed、2 skipped；DNS leak 行为测试通过、application 旧路径/导出为零；UI package 构建遵守 Core-first 阶段门 |
| `BANDROID-041.3.2R.4.2` Fake-IP 映射投影迁入 presentation | 2 | Core nextest `2909093c-1c71-44ce-9cda-57487a042309`：1245 passed、2 skipped；mapping 搜索/缺失行为测试通过、application 旧路径/导出为零；UI package 构建遵守 Core-first 阶段门 |
| `BANDROID-041.3.2R.4.3` Sync copy 投影迁入 presentation | 1 | Core nextest `2909093c-1c71-44ce-9cda-57487a042309`：1245 passed、2 skipped；sync 状态与 copy 行为测试通过、application 旧路径/导出为零；UI package 构建遵守 Core-first 阶段门 |
| `BANDROID-041.3.2R.5.1` RoutingIntent action state 显式 enum + presentation owner | 3 | Core nextest `4c8d4c36-77bb-46ff-9907-26dcb749976a`：1060 passed、0 skipped；覆盖 generation/revision、迟到 receipt、retry、dismiss 与 invalidate 取消；application reducer/export 与旧 UI import 清零；typed-state 0、retired-surface/doc-governance/fmt 通过；未构建 Iced/Bevy UI/APK，按 Core-first 留统一 UI 验收 |
| `BANDROID-041.3.2R.4.9` DNS 诊断与设置展示投影迁入 presentation | 5 | Core nextest `443ea958-4e0d-4e02-b790-e47c274baf36`：1123/1123；DNS health/latency/settings/STUN 行为测试通过，application normal dependency 无 presentation；changed-production typed-state scope 27 文件/0 findings，doc-governance、retired-surface、fmt 与 diff 通过；Iced/Bevy package 构建留 Core-first 统一验收阶段 |
| `SLINT-002.1` Profile display name typed persistence | 3 | commit `ace465d`; Core nextest `9a3d56b1-bf75-4f5e-b7dd-b371fb0dc21e`: 1040 passed, 0 skipped; owner readback/persistence tests |
| `SLINT-002.2` Snapshot-fenced profile catalog ordering | 3 | commit `e660750`; Core nextest `9a3d56b1-bf75-4f5e-b7dd-b371fb0dc21e`: 1040 passed, 0 skipped; fmt, typed-state, doc-governance and retired-surface guards passed |
| `SLINT-002.3` Typed clear active profile selection | 3 | commit `8a46f90`; Core nextest `d61dd42f-8b79-419d-84e1-26c17dd7fdfc`: 1043 passed, 0 skipped; stale/foreign/bound-owner/idempotent receipts and CAS fence verified |
| `CORE-011/012-7B.2` scope-fenced connection and runtime stream use-cases | 3 | Core-only nextest `5a52443d-e02c-4221-94cf-95bfcfd9f0a0`: 3202 passed, 13 skipped; non-UI composition/consumer batch; Iced/Bevy/Slint package validation transferred to CORE-UI-001 |
| `CORE-011/012-7B.3` scope-bound configuration use-case injection | 3 | Core-only nextest `5a52443d-e02c-4221-94cf-95bfcfd9f0a0`: 3202 passed, 13 skipped; non-UI composition/consumer batch; Iced/Bevy/Slint package validation transferred to CORE-UI-001 |
| `CORE-011/012-6.1` scope-fenced connection identity/control and typed route | 5 | Core-only nextest `5a52443d-e02c-4221-94cf-95bfcfd9f0a0`: 3202 passed, 13 skipped; non-UI composition/consumer batch; Iced/Bevy/Slint package validation transferred to CORE-UI-001 |
| `CORE-011/012-6.2` scoped logs/traffic/connections streams | 5 | Core-only nextest `5a52443d-e02c-4221-94cf-95bfcfd9f0a0`: 3202 passed, 13 skipped; non-UI composition/consumer batch; Iced/Bevy/Slint package validation transferred to CORE-UI-001 |
| `CORE-011/012-6.3` typed resource observation and GC readback | 3 | Core-only nextest `5a52443d-e02c-4221-94cf-95bfcfd9f0a0`: 3202 passed, 13 skipped; non-UI composition/consumer batch; Iced/Bevy/Slint package validation transferred to CORE-UI-001 |
| `CORE-011/012-2` Speedtest/jitter identity and real Mihomo probe | 3 | Core-only nextest `5a52443d-e02c-4221-94cf-95bfcfd9f0a0`: 3202 passed, 13 skipped; non-UI composition/consumer batch; Iced/Bevy/Slint package validation transferred to CORE-UI-001 |
| `CORE-015.9` Host retry/rebuild on the shared Core owner | 2 | Core-only nextest `5a52443d-e02c-4221-94cf-95bfcfd9f0a0`: 3202 passed, 13 skipped; non-UI composition/consumer batch; Iced/Bevy/Slint package validation transferred to CORE-UI-001 |
| `BANDROID-043.5` V6 owner revision fence and scoped application commands | 3 | commit `28aa02a`; Rust nextest `9fc51584-c091-4c96-b44e-8d5b1d63a963`: 562 passed, 2 skipped; Compose Gradle JVM batch: 16/16; changed-path typed-state findings 0, doc-governance, retired-surface, rustfmt and diff checks passed; retired per-operation Logs/Connections UniFFI exports and verified `logs_clear`/`connection_close(s)` export search clear. No APK build or Android device acceptance; ARM64 hardware unavailable. |
> v143 在 v142 上归档 DNS display projection ownership，共 5 pt。继承的历史明细仍比累计少 1 条 / 1 pt，旧任务 ID 与证据无法还原；当前已知切片均登记，未猜填历史记录。

## 全局研判与产品交付收口门（2026-10-11）

当前不是“每次修改都无法确认”，而是过去把局部测试、源码状态和产品验收混写，且跨进程竞态/系统授权/真机输入没有统一的阶段门。最近的 VPN 复核已经从当前源码直接确认了若干实现与文档目标不一致的风险，因此先稳定任务与验收契约，再继续扩大代码改动。本文档和质量规则的更新只建立工作顺序，不关闭任何实现任务。

以下都是当前产品交付范围，按依赖顺序推进，但不能因阶段依赖而移入未来佛：

1. **Engine-neutral Core 与双适配：**按 CORE-007～015 收口身份/代次、操作能力、ProfileDocument namespace/CAS、engine codec/config adapter、typed outbound/routing/runtime、ports/factory，以及真实 Mihomo + sing-box 两种 Core。每片迁完所有生产消费者后同片退役旧接口、wire、fallback 和重复状态；两引擎按同一 conformance suite 留真实进程证据。
2. **Android 唯一 owner 与 APK 链路：**完成 Kotlin `:vpn` owner、VPNService、typed IPC、权限/生命周期、Bevy/Slint/Compose flavors 与最终 `.so`/APK identity。Compose、Bevy、Slint 只做 UI/系统能力 client；所有请求、回执和快照绑定 identity/generation/session/revision，Binder death/rebind 先 readback/adopt 再受理。
3. **桌面服务管理与所有必要页面：**Iced/Bevy/Slint 共用 `DesktopProductSession`、`CommandIntent`、`SurfaceSnapshot` 和 presentation/i18n/asset owners；完成 overview、proxy、profiles、network、connections、logs、rules/providers、traffic、settings 与 about/许可证页的共享语义和服务操作。
4. **跨平台交付与统一验证：**Windows、Linux、Android 当前目标构建和产品身份核对都在现在佛；统一收口 Core 行为、三种 Android flavor、桌面 UI 场景和 target build。签名轮换、安装器格式、3D 地球、额外性能与 lint 工具属于未来佛增强。

### 统一阶段门

- 每个改动切片先声明唯一主 owner、文件/模块范围、输入状态、typed 终态、并发/取消语义、旧接口退役点与验收证据；同一边界不得由并行实现者同时改写。跨 crate 或进程契约先合并边界设计，再实现生产端与消费者。
- Rust 测试唯一执行器是 `cargo nextest`；禁止裸 `cargo test`。局部行为用 nextest 精确包/测试过滤器；阶段收口统一跑 `cargo nextest run --workspace --no-fail-fast`，并记录运行提交/工作区状态、通过/失败/跳过数。格式、结构门、Gradle JVM、AndroidTest APK、最终 APK 与设备证据分开报告。
- 状态、请求类别、失败/取消/Unsupported、owner role、transition 和结果使用穷尽 typed enum/sealed 类型；禁止用多个可组合 `bool`、隐式字符串 action 或散落 `if/else` 形成第二状态机。副作用成功必须以同一 owner 的真实 receipt/readback 为准。
- 单测/模拟、静态 guard、Android 编译、APK 安装、模拟器行为、ARM64 真机流量分别属于不同证据层，不能彼此外推。某阶段要求的设备缺失时，相关任务记录为 `OPEN`/`INCOMPLETE` 并明确可复现缺口，不用源码推断补齐。
- Core-first 阶段先完成所有不依赖 UI 的底层工作；如果 Core 契约退役只被既有 UI 调用阻塞，可将机械式 typed API 边界迁移纳入同一 Core 切片，但不做页面、视觉或新功能改造，并在迁移批次后统一构建验收。Core 没有可独立推进的必要事项后，再进入各 UI 专属整改。
- 候选版本、APK、应用身份和截图必须绑定 commit、version、产品 ID、APK SHA-256、ABI、设备/API、窗口 dp/density/fontScale 与测试结果。不得引用旧 APK 的结果证明新源码。
- 新 typed 替代能力达到迁移条件时，旧生产消费者、公开 API、DTO/wire、测试和旧描述必须同一切片退役；唯一例外是读取历史持久化格式的显式版本迁移器。文档状态只跟随证据更新，不能替代实现门。

### 本轮质量与测试回执（2026-10-11）

- 文档层级与引用方向守卫、自测及完整结构检查已通过；本地台账不反向引用上层文档。
- HEAD `22f4e56ca79c`，验证时工作区存在大量未提交改动，所有结果绑定当时 dirty workspace，不能表述为该 commit 的干净树证据。之后冻结候选针对 Bevy 触摸滚动、显式 MotionPreference、软键盘和 GlobalStatusDot 完成修复；widgets 与 UI focused Clippy（`--all-targets -D warnings`）均通过。当前记录仍绑定 dirty workspace。
- Rust 回归执行器唯一使用 `cargo nextest`，统一 workspace 命令带 `--no-fail-fast`。最近一次完整 workspace run `272d97ba-6521-4a54-9ebb-ccfd66eb37f8` 为 4483 passed、0 failed、2 skipped；ShellObservation 切片最近一次完整受影响包 run `f09bcff7-5913-48d1-a547-9cd9957fff62` 为 2344/2344，但早于连接速率 enum 迁移。当前源码 run `016f2f7c-6319-41b9-b466-cbfc71626696` 编译并开始 2971 项 nextest，用户要求切换到 TODO 盘点时最后可见 2517 项通过，未得到最终摘要，不计作通过；`041.3.2.4` 保持 `IN_PROGRESS`。
- parity resolver：registry/discovery 均已验证，236 个 anchors、0 dangling；120 个视觉 cell 仍 `unclosed`、`aligned=0`、`complete=false`，因为没有绑定当前产物的标准/紧凑屏幕回执。workspace 测试不关闭视觉或设备验收。
- 本轮未构建 APK、未运行 Gradle/AndroidTest，也未做模拟器或 ARM64 真机验证；BANDROID 唯一 owner、VPN/TUN 真实启动、Compose/Bevy/Slint 真机输入和产品身份设备回归仍保持 OPEN。

### 跨端 UI parity 证据状态（2026-10-11）

`scripts/parity/feature_evidence.tsv` 曾有 46 个引用已删除或改名测试的 dangling anchors，现按语义替换 33 个、删除/合并 8 个冗余或错配锚点；不匹配的旧生命周期测试不再冒充代理/URI 行为证据。仍有 4 个 evidence cells 保持 `pending`：Iced 与 Bevy 的 Core rollback 应用层转发契约、Iced 原生代理搜索高亮/清除/失败恢复、Iced 运行中测速取消。下一步为这些行为补真实替代测试后再将对应 cell 标 `anchored`。最新 nextest discovery 为 4478 个 runnable IDs；resolver status 已通过、dangling 为 0，但 120 个产品 parity cell 仍未绑定两种屏幕规格的当前 APK 像素回执，`aligned=0` / `complete=false` 继续表示视觉/真机验收未完成。不得用缓存 discovery、headless 测试或历史 APK receipts 关闭 UI 任务。

## 当前执行顺序修订（按 2026-10-12 最新产品范围）

用户已明确：engine-neutral Core、Mihomo/sing-box 双适配、Kotlin VPNService 与 Android 权限、Bevy/Slint/Compose APK 链路、桌面所有必要页面和服务管理，全部属于现在佛。CORE-015 真实 sing-box 适配是必交付，不再只是“以后反证”；三套 UI 只消费共享命令/快照，必要差异留在平台 adapter。按 CORE-007～015 → `:vpn` 唯一 owner/IPC → Android flavors 与桌面页面/服务管理 → Windows/Linux/Android 构建和统一验证推进。**所有底层 Core TODO 和双后端 conformance 关闭前，禁止 UI 适配与 UI 包重编译。**

当前实现状态、依赖、验收与证据只以 [现在佛](todos/CODE_TODO.md)、[过去佛](todos/CODE_TODO_PAST.md) 和 [斗战胜佛](todos/CODE_TODO_BLOCKED.md) 为准；本页不再重复描述 Core/Android 任务。三佛迁移前的历史回执只保留在 [迁制归档](.evidence/ARCHIVE-2026-10-11-old-board.md)。Core 与双后端任务全部收口前，不进入 UI 适配或 UI 包重编译。

## 当前 Android 身份与 Gradle flavors（2026-10-10）

Compose、Bevy、Slint 三种 Android APK 都由 `:app` Gradle flavors 构建，共用 manifest、Kotlin VPN/control service、Mihomo 资产与 `:vpn` owner；Slint flavor 使用 NativeActivity，Bevy driver 只进入 Bevy flavor。三者的最终 `.so`、进程边界、IPC 与同一 snapshot 必须由 BANDROID-040/041 证据验证，源码相同或依赖树相似不能替代最终 APK 检查。Slint Android 源码已接入 typed product IPC，但三 flavor 最终重建、三端设备行为和唯一 owner 证据尚未完成。桌面 Iced/Bevy/Slint 的 Core ingress 都绑定 immutable `CoreScope`；Core freshness 只允许同一 engine/instance 代次前进。Android/桌面所有产品身份由 `product-identity.properties` 登记；Gradle 从登记表生成 app 名、notification target 与 flavor icon，CI/release 对每个最终 APK 做 aapt identity 检查。

2026-10-11 两套 x86_64 Debug APK 在 `bevy-smoke` API 35 模拟器上的有限证据：Compose 冷启动可观察到主进程与 `:vpn` owner，Core/VPN 初始均停止；显式 Core Start 后 owner 启动 `libmihomo.so`，Controller `/version` 返回 v1.19.32；Compose Profiles 底栏可切页。执行 VPN Start 曾显示通知权限提示；拒绝通知权限后继续到了系统 VPN consent。先在 Android VPN 设置执行 Forget，再显式 StartProxy，实际出现 VPN `Connection request`，选择 Cancel 后 Core/VPN 均未启动。**成功授权、TUN ready、真实流量、StopProxy/PauseKernel、Binder 重绑和新源码 APK 均未验证。**旧 APK 暴露 `reported=Running, observed=Starting`。当时的源码静态审计登记了 worker receipt 早于 TUN ready、prepare/start 配置可漂移、路由租约释放早于 Kotlin Running publication、worker-first stop 与 owner readback 冲突、revoke cleanup failure 仍可能报 Revoked 等风险，随后拆分到 BANDROID-042 跟踪。其中“路由租约早于 Running 发布”经后续源码复核为误报：当前服务先在 Held lease 下发布 Started/Failed receipt，再由内层 finally 释放；设备运行与真实流量证据仍开放。Bevy `BevyHostActivity` 与 `:vpn` owner 可启动，Core 默认停止；一次点击落在 Android 系统手势保留区并退出应用，证明该坐标不可用，但不能替代当前源布局上的 hit-target 测量。两套证据均绑定修复前 APK，不代表 ARM64 真机或完整 UI。Release 签名与 AndroidTest instrumentation 仍未验收，BANDROID-039 保持 `OPEN`。

## Android 架构现状（2026-10-10）

- **共享源码已建立，单一运行时尚未证明。** Desktop、Compose、Bevy、Slint 复用 `domain`、`contract`、`ports` 与 `application` 语义；Android 三 flavors 仍须证明运行时只由 `:vpn` 持有。
- **唯一 Rust owner 与 Android IPC 源码链部分接通。** Compose/Bevy/Slint flavors 均须经 `:vpn` typed owner IPC；Binder death/rebind、最终 APK ELF/符号和多进程 runtime 仍需验证，不能仅凭源码宣称共享 singleton。HostProtocol V6 使用显式 ShellObservation wire enum 与十进制 session token；V5 client fail closed。旧 START_CORE/STOP_CORE 与无消费者 AndroidApi/FfiApi facade 已源码收口；Compose scoped command、Bevy/Slint product command 与部分 owner operations 已接入。其余 owner surface 与设备证据见 BANDROID-004/036/040。
- **依赖方向按 BANDROID-041 分片收口。** 2026-10-14 将 Bevy desktop session/pump/command sink 与 executable entry 移至 `infiltrator-bevy-desktop-host`；Bevy UI 只保留注入入口。BANDROID-041.3.3 已移除 Android driver 对 Bevy UI `ui-application-projections` 的选择，锁定 Android target tree 与 guard 均排除 `infiltrator-application`；最终 APK ELF 与多进程运行证据仍开放，见 BANDROID-041.4.2 与对应任务实机条件。
- **实现状态与目标状态分开记。** Android 目标是 `:vpn` 唯一持有 Rust application/配置事务/Core/VPN 权威事实，Compose/Bevy/Slint 都是 client。per-app routing 与 TUN settings 已迁入 V3 typed owner operations 并退役 direct FFI exports；Profiles、WebDAV、VPN session authority 与部分 reader 尚未完整归入唯一 owner。三 flavors 的设备 IPC、Binder death/rebind、权限提示与真实流量证据仍开放。iOS 原生 surface/NetworkExtension 暂缓，不计入 57 个主工作流。
- **2026-10-11 Android Messenger surface cleanup（Rust workspace 已通过，Android 产物待验证）：**删除无产品消费者的 Core 控制、Core 子进程状态读取和 profile apply 专用 IPC forwarding；控制继续使用带 scope 的产品命令，子进程状态只由 owner-local JNI 读取，profile apply 留在 owner Profile 操作事务。VPN Builder 配置与前台状态 callback 改为 `AndroidBridge` 必须显式实现。之后统一 workspace nextest `272d97ba-6521-4a54-9ebb-ccfd66eb37f8` 为 4483 passed、0 failed、2 skipped；本轮全仓结构/import guard、文档层级守卫与 `git diff --check` 已通过。Gradle、UniFFI 生成校验、最终 APK 与设备行为待分阶段门执行。
- 本 TODO 记录 Android 平台任务拆分与执行证据。

## Kotlin 质量审查（2026-10-10）

**结论：Kotlin 已有一些正确的类型化状态与进程边界，但质量约束还没有达到 Rust 侧水平。** `AndroidProcessRole` / `VpnProcessState` 使用 sealed 类型，控制请求使用 UniFFI enum，多个 ViewModel 使用只读 `StateFlow`；这些可以作为统一模式。Android CI 已编译两种 flavor 并跑 JVM 单测，但没有 ktlint、Detekt 或 Android Lint 执行项；`scripts/android-check.sh` 实际只跑 Rust `cargo check` / Clippy，所以 workflow 的 “compile + lint” 目前不能代表 Kotlin lint 通过。

**先做架构：** 执行顺序固定为 BANDROID-040 → 041 → 043 → 042 → 044。040/041 收口唯一 Rust Core owner、IPC 与依赖方向；043 建立可注入 Kotlin host/UI seam；这些完成后再修 VPN 业务事务 042，最后强制质量门 044。

### 已确认的 Kotlin 风险

- **VPN Service 主线程调用路径已部分整改，容量与设备时序仍待验收。** 当前 `onStartCommand()` 在主线程先校验命令/授权并进入前台，再把 prepare、路由读取、Builder 建立、TUN handoff 和 worker stop 放入单写者 executor；旧的主线程同步 FFI 描述已退役。executor 队列是否有界、启动/停止/撤权竞争以及真实 Service 生命周期仍需行为与设备证据。
- **分应用路由已改为 fail-closed，并由 typed activation lease 覆盖 VPN 建立。** `loadVpnRoutingConfiguration()` 返回 typed `Ready/Failed`；读取失败、空 package、无效选中集或 `Builder.addAllowedApplication/addDisallowedApplication` 失败会在 TUN worker 启动前中断事务并走 FD/worker 清理，不再回退到全局代理。路由数据由 `:vpn` typed owner operations 唯一读写；VpnService 以专属 routing revision 取得 activation lease，再调用 `Builder.establish()`，租约覆盖 Rust 数据面启动和 Running 状态回读，期间的路由写操作返回 typed `NotReady`。剩余：Android Builder/FD 异常路径行为测试和真机多应用流量证据。
- **启动 readiness 由 Rust owner 判定。** 2026-10-11 已删除 `MihomoProcessManager.start()` 的 500ms 固定睡眠；Kotlin 只确认子进程 spawn，`CoreApplication` 使用 `ControllerReadiness` 在有限 deadline 内验证 API。现在 readiness 失败会先请求停止刚启动的进程，再发布 `Failed`；application nextest 覆盖失败回滚。仍需目标设备验证 spawn、API readiness 与退出回收时序。
- **取消和失败在 ViewModel 层被抹掉。** `runFfiCall()` 把父协程的 `CancellationException` 当普通错误返回；Overview 的 traffic/log polling 空 catch 忽略调用失败，保留旧值却没有失效状态，初次失败时 loading 也可能不结束。
- **页面离开后周期任务可能继续。** `App.kt` 以条件分支切页，多个 Screen 的默认 `viewModel()` 使用 Activity owner；Overview 在 `init` 启动多条轮询，Proxies 启动 refresh loop，页面退出组合不会自动清除 Activity 级 ViewModel。仓内各 Screen 还使用 `collectAsState()` 而非 lifecycle-aware collection。轮询应绑定可见页面/前台 lifecycle，并确认全应用同一事实只有一个 reader。
- **生命周期接线失败没有原子回滚。** `NativeHostAdapter.attach()` / `detach()` 捕获 `Throwable` 后继续/退出；中途失败会留下部分 listener 或 JNI registration，Compose `MainActivity` 也会捕获所有 `Throwable` 并继续显示页面。
- **阻塞式 IPC 成本尚未收敛。** `MihomoControlClient.transact()` 每个请求创建一个 `HandlerThread`，通过 `CountDownLatch` 最多等待 15 秒后逐次解绑；owner 侧在单个 Messenger `HandlerThread` 上 `runBlocking` 调 Rust async API。协议已有 request/session id 和超时，但需验证排队、取消、死亡重绑和高频状态读取不会堵塞 owner。
- **宿主类混装多种责任并复制配置语义。** `MihomoHost.kt` 同时放 BridgeHost、子进程管理、profile/default config 读取与凭据加密；`MihomoVpnService.kt` 同时承担 JSON schema、Builder/分应用策略、通知、生命周期与跨调用状态等待。Kotlin 还在 `config.toml` 中自己解析 profile 并生成默认 Mihomo 配置；这些应收回 Rust owner，Kotlin 保留 Android 系统副作用和窄 adapter。
- **用户错误文案绕过共享本地化。** `ui/common/FfiUtils.kt` 以英文字符串映射 `FfiErrorCode` 并生成空态文案；多个 ViewModel 另传英文 fallback。此处应由共享 failure/localization 投影给出键与参数，不能按 Kotlin 页面各自翻译。

### 实施方案与逐步清单（已迁至看板）

BANDROID-040/041/043/042/044 的逐步清单、执行态与日期回执已迁至 [todos/CODE_TODO.md](todos/CODE_TODO.md)（在制）与 [todos/CODE_TODO_PAST.md](todos/CODE_TODO_PAST.md)（已交付待验证批次）；原文全文保留在 [.evidence/ARCHIVE-2026-10-11-old-board.md](.evidence/ARCHIVE-2026-10-11-old-board.md)。保留要点：执行顺序固定 040/041 → 043 → 042 → 044；切片办结只表示该代码切片验收完成，不表示父工作流关闭；044.7 的 `typed-state-guard.py` 在全仓 88 条报告（25 bool 入参、39 bool 操作结果、11 标志驱动分支、13 多字段状态记录）清零前不得关闭质量门。

## Bevy Android 保留扩展与缺口补全（2026-10-07，当前有效）

用户要求：已有能力继续保留、继续扩展；缺失能力安排补全，先更新规范和 TODO。本节纳入既有双端/平台主线，不替换其它未完成业务；本表只登记实施任务及其状态。

规范建立时 16 项任务全部 OPEN；本节后续施工已开始，当前状态以任务表和最新验收记录为准。已有组件/代码作为复用基线保留，不因新增任务而撤掉；已有缺陷需纠正，历史 DONE/模拟器截图不能覆盖当前 Android 产品缺口。

### 保留基线与实际接线缺口

| 保留并扩展的基础 | 当前核对范围 | 本次补全方向 |
| --- | --- | --- |
| Kotlin VPN、通知、Builder 与生命周期 | Compose 宿主有 prepare/前台/TUN/停止/撤销路径；manifest 未声明 UI/VPN 分进程 | 服务无 Activity 初始化、IPC、进程所有权及真实后台/重启行为 |
| 独立 Mihomo 内核与 Rust tun2proxy | `MihomoHost` 通过 ProcessBuilder 执行改名的 `libmihomo.so`；Rust 接管 TUN FD | 复用此路线，核验 readiness、FD 回收与各分应用模式无回环；不强制改成 cgo/gomobile |
| 共享 application、命令与有界快照 | Bevy system 非阻塞读结果；全页快照通道容量 4；桌面已组合真实宿主 | Android 的命令、全页 reader/VPN 组合与跨进程终态/重绑；默认移动入口仍 NotReady |
| Winit cadence、触控与安全区消费 | 前台 Continuous、后台 500ms、遮挡 60s；已有 Insets 到根/顶部/底部布局的同步 | 真实 AppLifecycle、原生 Insets、静置事件等待与后台数据唤醒；有限等待不称完全挂起 |
| IME、文本和剪贴板相关控件 | 已处理合成 Preedit/Commit 与原生窗口 IME 字段；Android 使用 NativeActivity，移动系统剪贴板未组合 | 原生文本输入与已有状态机完整接线、ClipboardManager 的实际复制/导入/粘贴 |
| 虚拟窗口与实体池 | 算法、动态行高与 `VirtualEntityPool` 已存在；节点页仍遍历全组挂载，连接/日志没有全面回收 | 保留完整业务集合并接通实际页面/网格行槽，稳定 ID 和过期事件保护 |
| Surface 基础恢复 | 锁定 Bevy/winit 在 Android 挂起移除原生 handle、恢复重建；render 层回收并重建 surface | 产品观察/草稿/来源恢复与设备循环验证，不另造业务 swapchain owner |
| 波形、二维拓扑与粒子下钻 | CPU RGBA 到 ImageNode 路线存在；`sync_charts` 在 Update 未按变更过滤 | 缓存整改、按需动画、GPU 波形/Bloom 与后续真实地理来源的可选地球 |

2026-10-07 核对时实际执行 37 项选定 headless 测试全部通过，其余 633 项未选中；范围为 cadence/IME/gesture/list 与安全区消费，不是全仓或 Android 设备验证。命令保留供按需回归：

```bash
cargo nextest run --offline --locked -p infiltrator-bevy-ui -p infiltrator-bevy-widgets --test headless --build-jobs 4 --test-threads 4 -E 'test(cadence_tests::) | test(ime_tests::) | test(gesture_tests::) | test(list_tests::) | test(test_safe_area_insets_android_and_ios_adaptation)'
```

### 产品任务与依赖

每行只有一个主 owner；协作模块按功能地图接线，不另建 canonical 事实。所有 `DONE` 均须满足该行声明的行为和适用产物/设备证据；缺环境先完成独立工作，继续保留未验证层。

> `BANDROID-001..016` 的任务表已拆入三佛看板（现在佛 / 未来佛 / 实机证据）；原文与逐行验收条件见 [.evidence/ARCHIVE-2026-10-11-old-board.md](.evidence/ARCHIVE-2026-10-11-old-board.md)。

### 推进顺序与完成口径

2026-10-07 后续实施：原 16 项均已安排，尚无完整 Android 产品任务 DONE。Shader/GPU 波形归 013、地球归 016；新发现的实际卡片 shader 接线拆为 BANDROID-017，先实现独立的图形/缓存切片，Android 宿主与设备闸门仍保留。

先完成新增的架构阻塞项：BANDROID-040 建立 Android 唯一 Rust owner 和真实 IPC session；BANDROID-041 移除 Android host/FFI 到 Bevy UI 的反向依赖。Kotlin 质量路径按架构优先推进：BANDROID-040/041 → 043 → 042 → 044；先闭合 Core owner、IPC、模块边界，再修 VPN 业务事务，最后强制 Kotlin 静态门；不以编译通过替代行为证据。

> `BANDROID-017` 的任务行已拆入看板；原文见 [.evidence/ARCHIVE-2026-10-11-old-board.md](.evidence/ARCHIVE-2026-10-11-old-board.md)。

1. 完成 001～004 的服务进程、网络宿主、原生 Activity 和完整产品组合；009 的节点回收、013 的缓存整改与 015 的构建/预算准备可独立推进。
2. 完成 005～008 的生命周期/Insets/IME/剪贴板，扩展 010 的其它业务列表，并联合完成 011～012 的真实节能与数据唤醒。
3. 完成 014 的移动交互与适用无障碍，再由 015 对当前默认特性产物统一取证；不把库交叉检查、历史截图或选定 headless 当作设备放行。
4. 013 的 GPU/Bloom、016 的地球和 017 的卡片 shader 可独立推进图形实现与桌面 GPU 验证，保留兼容路径；完整 Android 交付仍经过宿主与功耗闸门，不以等待设备为由搁置图形组件。

关联旧项：BEVY-006/013/015、BEVY-GAP-136～141/148/149、DUAL-02-13、DUAL-03、DUAL-04、DUAL-13、DUAL-15 与 UI 体验路线图。新 ID 是实施拆分，既有 FeatureId 场景仍在统一清单验收，不创建第二份纸面 parity 状态。设备不足时保留未满足层和失败原因，不降级目标或伪标 DONE。

### 产品发行身份专项（2026-10-10）

版本唯一源已收敛到根 `Cargo.toml` `[workspace.package].version`；Slint 接入后，根 `product-identity.properties` 当前登记 Android Compose/Bevy/Slint 与 Iced/Bevy/Slint 桌面身份。Linux `.desktop` 经共享模板渲染，Deb/AppImage builder 从清单取名、二进制、图标与包 ID；Windows 安装器与 Android APK 也从同一登记表读身份。Apple target CI 按用户要求暂缓；Windows/Linux/Android 当前产品目标产物仍待统一实际构建和核对，APPID-001 保持 `IN PROGRESS`。

> `APPID-001` 的任务行已拆入看板（APPID-001.1 已归档，.2 在现在佛，.3/.4 在未来佛）；原文见 [.evidence/ARCHIVE-2026-10-11-old-board.md](.evidence/ARCHIVE-2026-10-11-old-board.md)。

Slint 接入后，`product-identity.properties` 已扩展到 Android Compose/Bevy/Slint flavors 和 Iced/Bevy/Slint 桌面产品；Windows、Linux、Android 当前 target build/identity 回执由现在佛 `APPID-001.2` 统一收口。

2026-10-10 Apple 编译暂缓：按用户要求，release workflow 删除 macOS arm64/Intel matrix 与 DMG/Tarball 构建步骤；当前自动 release 只编译 Windows、Linux、Android。当前没有 iOS target job；workspace test 仍可在 Ubuntu host 编译 `infiltrator-ios` crate，但不产出 Apple binary。`apple-ci-deferral-guard.py` 已接入结构门，覆盖 Mac runner、Darwin/iOS Rust target 的回归；`bash scripts/test.sh --guards-only` 及目标 `cargo nextest` 通过。APPID-001 仍因平台包/目标设备/签名尚未验收而未完成。

### Android 移动端 UI 基础设施专项（2026-10-10，分析报告转执行清单）

本专项起始于 Bevy 与 Compose 两种 Android 产品的触控/滚动分析；当前产品范围已扩展为 Compose、Bevy、Slint 三个 APK flavor，共用 Rust domain/contract/application 命令、状态快照、`:vpn` 唯一生命周期 owner、Kotlin VPNService、设计 token 与场景验收定义。三种 UI 是薄 adapter，各自保留必要的原生控件和输入接缝，不另建业务状态机或 Core/VPN 控制路径。用户最新要求将双引擎 Core、Android 权限/服务/APK 链路、三套 UI 必要页面、桌面页面与服务管理都列入现在佛；未来佛只留优化和锦上添花增强。操作以 Rust typed enum 顺序执行，Kotlin 不得组合本地 bool/string action 构成第二套生命周期机；Android 系统授权映射为 Rust `VpnAuthorizationResult`。Bevy 已锁定 `0.20.0`；其 `TouchInput`/Picking、`ScrollArea` 与 Android Insets 接缝存在，不等于完整 APK 输入通过。Compose 用 Material 3/`LazyColumn`；Slint 使用同一 owner snapshot 与命令契约。设计 token、i18n、响应式、安全区和业务状态优先复用；必要功能缺口不等待视觉阶段或挪入未来佛。

> `BANDROID-025..030` 的任务表已拆入看板；原文与 SHOT-OV 验收映射见 [.evidence/ARCHIVE-2026-10-11-old-board.md](.evidence/ARCHIVE-2026-10-11-old-board.md)。

#### Bevy Android 概览页实机截图审计（2026-10-10；验收映射到 BANDROID-027/028/030，不另计数）

证据来自用户提供的 Android 竖屏截图。截图未绑定 APK SHA-256、源码 commit、设备/Android 版本、窗口 dp 尺寸、density/fontScale、系统 Insets 或当前滚动位置，因此可确认截图呈现的问题，不把它冒充为当前产品的完整 L3 证据。以下细化 BANDROID-016/017、025～030、036 的现有任务，所有原状态保持不变。

**源码与产物核对：** 该次审计中，`GlobePagePlugin` 在 Overview 根上追加地图段；`LastGlobeData(None)` 被转成空 `GlobeData`，`empty_state_scene` 固定 400px 高且三个地理显示选项仍显示。全仓生产源码未找到 `GlobeDataUpdated` 发布方，只有页面接收端和 headless 测试触发端。已有地理字段不足以填入 `GeoPoint`：`PublicIpProbeSnapshot` 有城市/国家但无经纬度，`ActiveExitSnapshot.country_code` 明确来自节点标签，`destination_geo_ip` 只有内核返回的 GeoIP 代码；国家中心点属于禁止推断的坐标。审计记录的 `overview_page` 声明顺序为状态卡、RoutingIntent 控件、流量图表、指标，而用户截图的首屏顺序相反；需要检查 Add<PageRoot> 追加时序和 ScrollPosition。概览趋势图仍用 `CHART_WIDTH_PX=876`、`CHART_HEIGHT_PX=140` 的桌面截图尺寸生成纹理，`chart_scene_with_scale` 把 876px 作为最大宽度并固定高度，没有按 Android 实际排版尺寸重绘。地图文案是英文硬编码。Android 底栏虽声明 `IconId`，图标加载失败时会把 `ImageNode` 设为透明图；`android/bevy-host/build.gradle.kts` 仅注册 native driver，没有控件图标 assets source。Bevy 0.20 已支持 `InlineImage` / `InlineBox`，此处是资源打包和页面采用问题，不是引擎 API 缺失。该次审计记录的本机 Bevy universal debug APK `app-bevy-universal-debug.apk` SHA-256 为 `869be158fd67fc055156d17b99e35d15b89c370e99ac6f0e13d0d379d469c4cc`，ZIP 目录没有 `assets/icons/*`；用户截图没有包哈希，故此产物不能直接等同于截图来源。Overview 状态卡使用自绘背景且未挂 `SurfacePanel`；Globe 根卡及灰色空态画布同样只有 `BackgroundColor`，不会进入 `surface_shader::sync` 的 `With<SurfacePanel>` 查询，所以现有表面阴影材质不会给这些实体绘制阴影。即使挂载 `SurfacePanel`，审计时的 `surface_shader::sync` 仍把每张卡写死为 `ModernSurfaceElevation::Low`（偏移 2px、模糊 4px、ambient/key alpha 0.04/0.08），主视觉卡不会形成足够清楚的层级。`RouteHistory` 已有 `can_go_back/can_go_forward`，但 `content_title_row` 未使用。审计时的 Demo fixture 将 `OverviewState::Running` 映射为 `CoreLifecycle::Running`；用户截图同时显示绿色“运行中”、演示数据标记和“启动代理”。

**逐项整改方案与验收：**

1. **`SHOT-OV-01`：无地理数据时地图空态占满页面**（BANDROID-016/028/029，P1 / OPEN）。
   - **改动 owner：** `crates/infiltrator-bevy-ui/src/pages/globe.rs`；数据类型经 `infiltrator-contract`，唯一发布方经 `infiltrator-application`。
   - **实施：** (a) 将 `LastGlobeData(Option<GlobeData>)` 改为显式状态，如 `NotObserved / SourceEmpty / SourceUnavailable / Ready(GlobeData)`；禁止 `unwrap_or_default()` 把未观测折成 0/0。(b) 删除空态 `height: px(GLOBE_HEIGHT_PX)`；只有 `Ready` 且至少一个 City/Exact `GeoPoint` 时创建地图画布和 Globe/Flat/Eco 控件，其他状态显示一条紧凑的本地化说明，详情按需展开。(c) 先做地理来源盘点：现有 Public IP city/country、节点标签国家代码和 connection GeoIP codes 均不能构造经纬度；若没有真实坐标来源则保留 `SourceUnavailable` 并隐藏地图，不新增城市名转坐标、国家质心或基于延迟/旗帜的推断。(d) 如后续取得有授权且带精度/来源的真实经纬度，由 application 发布同一个 typed geo projection；UI 不读 MMDB、不访问网络。
   - **验收：** 对 NotObserved、SourceEmpty、SourceUnavailable、CountryOnly、City/Exact 五种 fixture 逐个验证；前四种不挂载 400px 画布或模式按钮，最后一种只画真实坐标/链路。Live source 缺席时 Overview 仍能正常显示状态与流量图。

2. **`SHOT-OV-02`：首屏顺序与图表主次不符合产品目标**（BANDROID-028/030，P1 / OPEN）。
   - **改动 owner：** `pages/overview.rs::overview_page`、`pages/globe.rs::on_globe_page_root_added`、`route/mount.rs`。
   - **实施：** (a) 用 `OverviewPageRoot` 的 `ScrollPosition` 和每个子段的实体/挂载日志确认截图落点来自首次挂载顺序还是历史滚动恢复；新 route identity 首次挂载时归零，返回同一 route 才恢复该 route 保存的位置。(b) 在 `pages/overview.rs::overview_page` 的 `bsn!` 静态树中声明唯一 `OverviewGeoSlot`，放在流量图和指标之后；修改 `pages/globe.rs::on_globe_page_root_added`，不再把 Geo 段 append 到任意 `PageRoot`，只向这个 slot 提交结构。(c) Compact 顺序固定为“真实 core/VPN 状态 + 一个主操作 → 流量趋势图 → 3～4 个关键指标 → 低频诊断”；Geo 只有 `Ready` 时才进入次级区。启动/停止按钮由当前 typed action availability 二选一，不能同时出现。(d) `overview.rs::{CHART_WIDTH_PX,CHART_HEIGHT_PX,chart_dims,overview_chart_scene}` 移除桌面截图的 876×140 固定纹理尺寸；布局宽度填满卡片内容区，Compact 高度由 `ResponsiveContext` 的紧凑 token 决定，桌面宽度受内容列约束。新增 `overview_restamp.rs::sync_overview_chart_extent`，在 `UiSystems::Layout` 后读取图表实体的 `ComputedNode` 和窗口 scale，写入/更新该实体的 `ChartPlate::0.width/height`；`overview_chart_spec` 与数据/回放 restamp 使用同一个最新 extent，不能再从固定 `chart_dims()` 取值。依赖 `chart.rs::sync_charts` 的 `ChartPaint` spec 比较，只于尺寸、scale、数据或主题改变时重绘，静置帧不重复上传纹理。(e) Live 图表仅读 `SurfaceSnapshot` 的实际读数；无样本显示本地化“尚未观测”短态，不调用 demo wave。
   - **验收：** Android 360dp/390dp 竖屏、横屏，桌面 1180px 视口分别采冷启动首屏；首屏顺序固定，有真实样本时先看到状态和流量趋势，无样本时无空 Geo 图挡住主图。360dp 下图表不产生 876px 固定宽纹理；旋转/窗口缩放后图像宽高跟随实际内容框，且一次尺寸变化最多触发一次新纹理，后续静置帧无重复上传。Expanded/Ultra 使用两列以上信息网格，不把 Compact 单列拉宽。

3. **`SHOT-OV-03`：导航及业务图标在 Android 截图中不可见，内容图标不足**（BANDROID-027/030，P1 / OPEN）。
   - **改动 owner：** `android/bevy-host/build.gradle.kts`、`crates/infiltrator-bevy-widgets/src/{icon.rs,asset_setup.rs}`、`scripts/quality/`。
   - **实施：** (a) 在 Bevy Android build 中把 widgets 的受支持图标资产注册到 APK，并从 `AssetServer` 的实际资源路径核实包内位置。(b) 扩展现有 APK 校验入口，从 Rust `IconId::ALL`/`icon_path()` 动态取得预期资源并检查最终 Bevy APK；不另建图标名单或功能专属 guard，Compose APK 不携带 Bevy 资源。运行期状态只报告最终 `Failed` 的加载结果，不能把 `Loading` 误报为缺失。(c) 使用仓库已有矢量资产生成 widgets 所需位图，并以 registry 作为唯一资源目录。只在有明确语义的状态或操作旁显示图标，保留本地化文字与 AccessKit label，不添加装饰性遥测图标。(d) 使用 Bevy 原生行内图像/盒子排版短标签和状态徽标。
   - **验收：** APK 校验器按 registry 核对最终 Bevy APK 的资源；删除任一已登记资源后检查失败。运行日志区分加载中、成功与最终失败；冷启动截图确认导航和业务图标可见，Compose APK 不含 Bevy 专属资源。

4. **`SHOT-OV-04`：紧凑屏顶栏和字号争夺视觉焦点**（BANDROID-027/028/030，P1 / OPEN）。
   - **改动 owner：** `shell_scene.rs::content_title_row`、`route.rs::RouteHistory`、`widgets/button.rs`、`pages/overview.rs::banner_scene`、`widgets/text.rs`、OV-08 typed window metrics bridge。
   - **实施：** (a) `RouteHistory::can_go_back/can_go_forward` 已实现，缺口是 `content_title_row` 没消费；把这两个只读结果接到按钮 disabled/hidden 投影。在 `widgets/button.rs` 提供可复用 `icon_button_scene(icon,label,...)`，组合 `ButtonVariantStyle(Ghost)`、`TouchHitbox::new(48.0)`、`IconPlate`/图像节点和本地化无障碍名称。替换 `shell_scene.rs::content_title_row` 的两个大实心 chevron pill；无历史时真实禁用对应控件。(b) 概览 RoutingIntent control 已迁入共享只读 `RoutingIntentSnapshot`，Unknown 明确显示未知而非灰化。(c) `banner_scene` 当前给状态文字无条件设 `Role::Display`；改为本地化 `Role::Heading`，另用紧凑的 `TextRole`/响应式字号映射（通过 `ResponsiveContext` 改 `TextFont`，不 remount 页面）：Compact/Medium 不启用 Display；Expanded/Ultra 只给一项主状态数值或速率启用 Display。统一映射放在 `widgets/text.rs` 或概览专属同步系统，避免每页手工乘缩放因子。(d) 消费 OV-08 新增的 typed host `density/fontScale/window dp` 数据；dp/sp 转逻辑尺寸只在 BANDROID-027 的共享 token adapter 做一次。
   - **验收：** 390dp 紧凑窗口顶栏一行可读，页面标题、状态、右侧状态 chip 不互相挤压；RouteHistory 的 back/forward enable 状态真实对应历史；1.0/1.3/1.5 fontScale 下按钮可触及、文字不裁切，点击目标保持至少 48dp。

5. **`SHOT-OV-05`：卡片、按钮和空白区域过平**（BANDROID-017/027/030，BANDROID-017 保持 IN PROGRESS）。
   - **改动 owner：** `widgets/surface.rs`、`widgets/surface_shader.rs`、`widgets/shader_fx.rs`、`widgets/button.rs`、`pages/overview.rs`。
   - **实施：** (a) 在 `infiltrator-bevy-widgets/src/surface.rs` 为 `SurfacePanel` 增加 typed `SurfaceElevation`（None/Low/Medium/High）；在 `surface_shader.rs::SurfaceShaderView` 查询它并传给 `ModernSurfaceMaterial::card`，未指定时兼容现有 Low 默认。给 `overview.rs::traffic_card_scene` 的 SurfacePanel 根显式设 Medium，其余普通卡显式/默认 Low。(b) 首页状态卡目前由 `OverviewCardState` 驱动整卡 accent/danger 填色；在 `overview.rs::banner_scene` 改为标准 `SurfacePanel` surface，在 `overview_restamp.rs::OverviewFill/reskin_overview_tokens` 停止按生命周期重刷整卡颜色，故障/运行语义改由状态点、chip、短失败文案和窄 accent 边表达，避免与 shader 的透明底材质互相覆盖。(c) `globe.rs::globe_page` 只在真实 Ready 内容存在时将地图模块外根改成一个 `SurfacePanel`；OV-01 无数据路径不再渲染 400px 灰色内板。画布如保留独立底色，用 token + hairline 分组，不伪装成有投影的卡。(d) Globe 模式按钮目前自绘 `BackgroundColor` 且未走统一按钮状态系统；重组为 `button.rs` 的 `ButtonVariantStyle` / `ButtonSizeStyle` 场景并通过 `sync_control_visuals` 更新选中、disabled、pressed。主操作 `Primary`、次操作 `Secondary`、导航/次级 `Ghost`、破坏操作 `Danger`；保证边框/前景对比和 pressed 色差，不给所有按钮叠 drop shadow。(e) 只给 Overview 唯一主 CTA（真实可执行的“启动代理”或“停止代理”）增加 `SurfaceElevation::Low`/`ElevatedControl` 标记，由 `surface_shader.rs` 扩展同一个材质 handle owner 来处理 Button surface；新增 `ControlSurfaceFill` 作为 button 视觉同步与 shader 之间的颜色事实，`sync_control_visuals` 在 variant/selected/disabled/pressed 改变时 compare-and-set，shader 按该 fill 更新材质，不与 `BackgroundColor(Color::NONE)` 互相覆盖。Pressed 时降低/移除阴影并把按钮内容下移 1px，Disabled 时不画阴影；Ghost、导航和小 chip 不加投影。(f) Flat shader 模式仍用同样 surface token、hairline border、圆角和选中 accent；主 CTA 用 1px 下边缘和 pressed 色差保留按压感，只有材质阴影关闭，信息结构和几何不变。
   - **验收：** Android 最终 APK 在 dark/light 两主题中可直接看到页面底、普通卡、主图卡和真实地图卡的层次；检查实体证明每个目标卡有 `SurfacePanel`，流量卡实际使用 Medium，状态/诊断卡使用 Low；唯一主 CTA 在 Shader 模式有可见但克制的 Low 阴影，Pressed 反馈下沉且阴影减弱，Disabled 不浮起；Flat 与 shader 路径圆角、边界、内容占位一致。Globe/模式按钮的 Default/Primary/Secondary/Ghost/disabled/pressed 态有对应色值与边界、按下操作明显反馈；无 hover 设备时不把 hover 当作验收条件；静止两秒材质和纹理无重复写入。

6. **`SHOT-OV-06`：中文壳层中夹杂完整英文地图模块**（BANDROID-030，P1 / OPEN）。
   - **改动 owner：** `pages/globe.rs`、`crates/infiltrator-shared/src/locales_table_surface_{en,zh}.rs`、相应 locale completeness tests。
   - **实施：** 增加 `globe_title`、`globe_plottable_count`、`globe_mode_globe`、`globe_mode_flat_2d`、`globe_mode_eco`、`globe_not_observed`、`globe_source_empty`、`globe_source_unavailable`、`globe_country_only` keys；计数用参数化模板，未知原因用 key，不用 `format!` 拼英文。`Text` 挂 `LocalizedText`/`TextRole`，语言切换只 restamp 文案，不重建地图纹理或滚动状态。
   - **验收：** locale completeness guard 覆盖新增 keys；同一 Android 构建分别切 zh-CN/en-US 时地图 header、控件、所有空态无混语；字体/模式/滚动保持原值。

7. **`SHOT-OV-07`：Demo 的绿色 Running 与代理操作可能被理解为真实在线**（BANDROID-004/029/036，P0 / IN PROGRESS）。
   - **改动 owner：** `pages/overview.rs::{banner_scene,status_dot_color,card_fill}`、`overview_lifecycle.rs`、`projection.rs`、`launch.rs`、`android/bevy-host/driver/Cargo.toml`。
   - **实施：** (a) banner 的状态投影加入 `OverviewOrigin`：Demo 首行显示“演示模式”，点和卡片使用中性 accent，不复用代表真实 `CoreLifecycle::Running` 的绿灯。(b) `core_control_scene` 在 Demo 中显示“仅演示”说明并禁用 Start/Stop real action；如果要演示按钮，只允许提交到 fixture-specific sink 并在文案上说明不会启动 VPN。(c) 产品 Android 编译 feature guard 禁止 `fixture-demo`，无 host composition 一律 `run_unavailable(NotReady)`；`DemoCommandSink::accepting()` 只从显式 demo entry 可达。(d) real Green/“运行中”只能在 `CoreApplication` readback 中确认 lifecycle Running 且 VPN session state符合产品文义后显示；分开显示“内核运行”和“代理/VPN 已接管流量”。
   - **验收：** Demo、missing host、core stopped、VPN unauthorized、real core running + VPN running 五态逐一验证文案/颜色/action；前四态不出现真实 green success 或可写真实控制；Android Gradle Bevy driver feature list 不含 `fixture-demo`。

8. **`SHOT-OV-08`：安全区、固定导航和滚动仍缺当前设备证据**（BANDROID-006/025/026/027/031，P0 / OPEN）。
   - **改动 owner：** 复用 `android/app/src/main/java/com/musicfrog/infiltrator/NativeHostAdapter.kt::publishInsets` 现有 listener、`RustBridge.kt`、`crates/infiltrator-android/src/jni_bridge.rs`、`crates/infiltrator-bevy-ui/src/host_insets.rs`、`ResponsiveContext`、`app.rs::sync_safe_area_insets`、`widgets/scrollarea.rs`、`gesture.rs`。不要另注册第二个 Insets listener。
   - **实施：** (a) 现有 `NativeHostAdapter.attach()` 已订阅 `WindowInsetsCompat` 并把 density/system bars/cutout/IME 通过 JNI 转成 typed `WindowInsetsInput`；沿这条桥扩展独立的 `WindowMetricsInput`，加入窗口 bounds（dp）、density、Android `fontScale`，同步改 `RustBridge.kt` 与 `infiltrator-android/src/jni_bridge.rs` 的 typed 参数。把现有 Insets listener lambda 的 `(_, insets)` 改为 `(view, insets)`，由 `publishInsets(view,insets)` 读取已布局的 `view.width/height` 并除以真实 density，同时读取 `activity.resources.configuration.fontScale`；在 `BevyHostActivity.onConfigurationChanged` 调同一个 adapter 的 `refreshWindowMetrics()`，不新增 listener。(b) `ShellPlugin` 当前以 `ResponsiveContext::new(width, 760.0)` 写死启动高度；改为从宿主首个 WindowMetrics/PrimaryWindow 高度初始化，未收到前不运行高度相关重排。`ResponsiveContext` 每次宽或高变化都更新 width/height/orientation；在响应式 token 中新增 `CompactHeight`（扣除永久 Insets 后可用高度 <480dp）映射，供概览缩短顶栏/图表并把低优先级诊断放到首屏之后或折叠，不隐藏状态和流量；窗口 metrics 未变时不重复 restamp。(c) 保留 shell topbar/bottom nav 固定、中间 `OverviewPageRoot` 单独滚动；逐条核对 `sync_safe_area_insets` 的 top/bottom 消费者，把永久 bottom inset 只给底栏一次、IME 避让只给可滚动视口一次，不重复加到页面 padding。(d) 复用 BANDROID-025 已有 MotionEvent→TouchInput→Picking→ScrollPosition 诊断字段，不另造触摸链；使用最终 APK 首帧及上下各 10 次 swipe 记录 Insets、当前 route 和 scroll offset。
   - **验收：** 回执绑定最终 Bevy APK SHA、commit、设备/Android、可用 window dp、density/fontScale/Insets、route/scroll offset；两种手机宽度、竖屏/横屏/分屏/IME 显示隐藏均无系统栏遮挡或重复留白，顶栏/底栏固定、首尾可滚动且导航不盖内容。切换窗口高或系统字号后，CompactHeight/fontScale 在一帧窗口指标更新后生效；至少 48dp 热区无裁切。无物理设备证据则保持 `OPEN`。

Google 官方移动设计参考用于落实适配、信息层级与导航形态：[Adapt layouts](https://developer.android.com/design/ui/mobile/guides/layout-and-content/adapt-layout?hl=en)（先按可用宽度、再按高度，支持 reflow/reveal/presentation change）；[Content structure](https://developer.android.com/design/ui/mobile/guides/layout-and-content/content-structure?hl=en)（按内容重要性建层级，组合图标、文字和容器）；[Layouts and navigation patterns](https://developer.android.com/design/ui/mobile/guides/layout-and-content/layout-and-nav-patterns?hl=en)（底部导航 3～5 个目的地，宽屏适配 Rail）；[Material 3 typography](https://m3.material.io/styles/typography/applying-type)（Display 保留给少量重要短文本，Headline/Title/Body/Label 建立层级）；[Window size classes](https://developer.android.com/develop/adaptive-apps/guides/use-window-size-classes?hl=en)（宽、高分开分类，按 app window 动态更新）。图表优先是本产品概览的明确产品要求；Google 资料用于约束响应式网格、图文层级、导航与字体比例，不将其误述为“Google 要求所有首页必须图表优先”。

**执行依赖：** `SHOT-OV-01` 可先独立修复无数据画布；`SHOT-OV-08` 扩展现有 BANDROID-006 窗口指标后，给 `SHOT-OV-02/04` 提供真实 dp/fontScale；`SHOT-OV-02` 在 01 后固定页面结构并改图表按实测尺寸重绘；`SHOT-OV-03` 完成图标 APK 资产合同后再做 `SHOT-OV-04` 顶栏图标按钮；`SHOT-OV-05` 在 01/02 结构稳定后收敛 surface/material 和按钮视觉；`SHOT-OV-06` 可与页面结构调整并行；`SHOT-OV-07` 是 P0，与 BANDROID-036 的 Core/VPN typed readback 一起优先处理，不能等视觉阶段结束。

#### 专项阶段门

1. **Core 双后端：**当前完成 CORE-007～015 的共享契约、旧面退役、Mihomo 与 sing-box adapter 和真实进程 conformance；双引擎共同使用 Core/application owner，不允许 sing-box 只停留在 `Unsupported`。
2. **Android 服务与系统能力：**完成 BANDROID-040/041/036/037/042/043：`:vpn` 唯一 owner、Kotlin VPNService、typed IPC、通知/系统 VPN consent、配置冻结、TUN readback、撤权恢复与 Binder 重绑。
3. **三个 Android 产品 flavor：**Compose、Bevy、Slint 共用意图、快照、语言/资源 owner 与身份表；所有必要页面、权限、输入、导航、状态和 Android APK 构建门都在现在佛。模拟器/真机实证内聚在过去佛对应任务的实机条件。
4. **桌面页面和服务管理：**Iced/Bevy/Slint 经共享 `DesktopProductSession`、Core commands 与 presentation 投影完成必要页面和服务操作；页面差异只在渲染与平台 adapter。
5. **统一交付验证：**Windows/Linux/Android target build、APK `.so`/identity、行为回归和跨 UI 同场景都属于当前必交付；视觉增强、3D 地球、额外性能优化、release signing/安装器便利属于未来佛。

本专项只把分析报告拆成实施与验收工作，不另建 `FeatureId` 或双端平权事实。Bevy 0.20 官方 API/示例以项目锁定版本对应的资料复核，报告中的版本/设备故障判断不直接视作已证实根因。

#### Compose/Bevy/Slint 表面、Android 共用宿主与架构边界（复用共享主线）

> `BANDROID-031..044` 的任务表已拆入看板；原文（含各主工作流验收条件与逐步清单链接）见 [.evidence/ARCHIVE-2026-10-11-old-board.md](.evidence/ARCHIVE-2026-10-11-old-board.md)。

三个 Android APK 的最终对照按“同一 `AndroidControlIntent`、同一 `SurfaceSnapshot`、同一成功/失败/取消语义”验收；各自布局、导航控件和输入由 Compose/Bevy/Slint 原生实现。Compose 将 scoped Core product command 与 lifecycle readback 交给 `:vpn` owner；Bevy/Slint 也经 product IPC 接入同一 owner。配置事务、Core 子进程状态与 VPN 生命周期仍只能由 owner readback；三 flavor 最终 APK `.so` 边界、唯一快照 reader、Binder/进程恢复和设备行为尚未收口，见 BANDROID-004/036/040/041 与 SLINT-001。

2026-10-10 首个实现切片：已移除 Compose `MainActivity` 注册 bridge 后自动 `coreStart()`；显式 VPN Start 路径仍会启动内核，Compose ViewModel 仍有直接 Kotlin 控制调用，两个 Rust 产品组合也尚未统一，故 036/004 保持 `IN PROGRESS`。用 Android Studio JBR 运行 `:app:testDebugUnitTest`，BUILD SUCCESSFUL（28 tasks）；通过 SDK 自带 adb 查询时当前没有已连接设备，真实 APK 首启、VPN 系统授权弹窗与流量行为尚未验收，不能标为 DONE。

2026-10-10 授权入口切片：Compose 每次用户 Start 都重新调用 `VpnService.prepare()`；如系统返回授权 Intent，授权成功后继续原来的启动动作，拒绝/取消不启动。Quick Settings tile 现在把 START/STOP 作为显式 Activity intent 送入同一 Compose 入口，不再从 TileService 绕过授权直接启动 `MihomoVpnService`；`MainActivity.onResume()` 重新核对系统授权。`POST_NOTIFICATIONS` 运行时提示、Bevy Activity 授权接缝、服务进程内核 owner、manifest 最终 APK 核验和真机弹窗仍未收口。相同 `:app:testDebugUnitTest` 再次通过（28 tasks，Kotlin compile + Rust JNI library build）。

> 以下按日期记录的是历史执行结果；其中旧 `manifest guard 0 missing` 只描述当时运行的已退役 guard，不能代替当前质量门。2026-10-11 新增的 `android-manifest-contract-guard.py` 是唯一当前 source-manifest contract guard，已接入结构门；Android 插桩版 `ManifestDeclarationsTest` 仍未由常规 workflow 执行。

2026-10-10 typed 生命周期与 IPC 切片：Compose/Bevy 的 core/VPN 操作统一为 Rust `AndroidControlIntent`，`CoreApplication` 以有序子命令执行 StartProxy/StopProxy/ResumeKernel/PauseKernel/RestartKernel；默认不因 Application、Activity、service 创建而启动 Mihomo，service 可用、内核运行与 VPN 接管分别表达。`:vpn` 的 `MihomoControlService` 持有 Android 子进程与 VPN service 操作，UI host 通过 Rust 声明的请求/回执枚举和 request-id Messenger/Binder 回执转发；VPN 授权由可见 Activity 发起并映射成 `VpnAuthorizationResult`。检查仅采用 `cargo nextest run`：4 个 targeted lifecycle/Bevy mapping 测试通过；`cargo check -p infiltrator-application -p infiltrator-bevy-ui -p infiltrator-android` 通过；当次运行的 manifest guard 输出 0 missing declarations（历史结果；当前已不在仓库质量门中）。Gradle UniFFI bindings、`:app:testDebugUnitTest`、`:app:compileDebugAndroidTestKotlin` 与 `:app:assembleDebug` 已通过。当前 adb 无连接设备；真实授权弹窗、进程 PID/存活、真流量与手势验收仍未完成，BANDROID-001/004/036/037 保持 `IN PROGRESS`。

2026-10-10 owner 状态回读切片（BANDROID-036，仍 `IN PROGRESS`）：Binder 回执现在保留 VpnService 请求、启动、前台、TUN 已建立、Running、Stopping、Stopped、Revoked、Failed、Unavailable 等 typed enum；通过 checked wire code 穿过 JNI 后，Rust `AndroidVpnServicePort::snapshot()` 按 `:vpn` owner 当前状态生成快照，不再拿 UI 进程独立 `OnceLock` 的 Idle/Stopped 缓存覆盖服务事实。Compose Overview 改读 `vpn_session_status()` 的 UniFFI typed snapshot，不再以 Activity 进程内 `VpnStateManager.vpnState` 作 VPN 状态源；未知/失败显示未知并保留错误。`cargo nextest run --offline --locked -p infiltrator-android -p infiltrator-contract -p infiltrator-platform --lib` 423/423；Gradle Compose/Bevy debug APK、两套 JVM 单测及 AndroidTest APK 组装通过。当前没有真机；owner 进程死亡/重绑、系统授权提示、真实 VPN 流量仍待设备验证，Compose core 状态和其它本地缓存也未全部迁移，036 不标完成。

2026-10-10 产品身份实施切片（APPID-001/BANDROID-038，仍 `IN PROGRESS`）：单一版本源为根 Cargo workspace；原本显式写 `0.41.0` 的 Infiltrator crates 全部继承 workspace 版本，iOS C ABI 版本和订阅 User-Agent 也从 `CARGO_PKG_VERSION` 编译生成。Iced Cargo Packager 不再覆盖版本；其产品名、Bundle ID、icon 与身份表交叉校验。根 `product-identity.properties` 登记 `android.products=compose,bevy` 与 `desktop.products=iced,bevy`，含包/Bundle ID、显示名、PNG/ICO、binary、Linux package、URL scheme 和 release prefixes。Android helper/Gradle/Bevy 临时 manifest/release tag 使用同一版本。Compose `versionCode` 41000、cargo-apk `versionCode` 16787712 从它派生。Linux launcher 模板经 `render-product-template.py` 渲染；Deb/AppImage builder 读取登记身份；macOS Info.plist 渲染 Iced/Bevy 身份并按登记 PNG 生成 ICNS；NSIS builder 从 registry 取脚本、ICO 与 workspace version，WiX 模板引用 ProductDisplayName/ProductVersion/ProductIcon。守卫扫描 Android 构建引用、桌面 matrix 二进制/artifact、Linux launcher、NSIS/WiX、macOS identity 与 Packager 元数据；未知产品/缺 mapping 有 Python 回归。`cargo nextest run -p infiltrator-domain -p infiltrator-ios -p infiltrator-iced -p infiltrator-bevy-ui -p infiltrator-android` 1981/1981、零跳过；`python3 -m unittest discover -s scripts/quality/tests -p 'test_*.py'` 37/37；`verify-packaging.py` 19/19、文档治理、当次 manifest identity/permission guards、py_compile/bash -n/git diff --check 与 Gradle identity+unit/AndroidTest compile/assemble 通过。Compose debug 和 Bevy release APK 均在版本源码变更后重建，并以 aapt 验证 package/versionCode/versionName/label/icon 匹配；SHA-256 分别 Compose `4114c132d3a7c6a726070fa640dfd1e9974954d5b4de3a478af4432b26600b44`、Bevy `25a62e3058cfed3cbf5bbd28f94771a27fc8da99cdcb1daf9bc81e71e524d0c3`。未实际执行 tag release、Linux 最终 Deb/AppImage、Windows NSIS/WiX 或 macOS bundle/ICNS runner；adb 无连接，Android 真机交互/授权也未验收，不能将 APPID-001、BANDROID-038 或总 Android goal 标为 DONE。

2026-10-10 产品身份防漏补强：每个 Android 产品显式登记 `icon_asset`，Gradle 应用显示名资源也登记在身份表；Gradle 通过 `scripts/product_identity.py` 直接读取 workspace `version.name` 与产品派生 `version_code`，不再复制 versionCode 公式。源码守卫扫描 Android application 插件模块，要求每个模块绑定已登记的 application ID、统一版本、派生 versionCode、launcher icon 与默认显示名资源；还扫描所有 `infiltrator-*` Cargo executable，未登记的新二进制必须显式注册为产品，CLI 与 UniFFI build tool 由登记表中的 `rust.non_product_binaries` 明确分类。测试覆盖未登记 Android/Rust 新程序、重复 Android app 模块及图标资产缺失/名称漂移。产品身份 guard 定向 17/17、质量 Python suite 44/44、packaging 19/19；Gradle `:app:testDebugUnitTest :app:assembleDebug` BUILD SUCCESSFUL，两个 APK aapt identity guard 通过。最新 Compose debug universal APK SHA-256 `445e4a297a94276b62a8e3652d6786dbe477d03073f86de56c3ae5224278d68c`，Bevy debug smoke APK `d751545fd1914aa44073fc9d538a488f446cfd98fb708a36eadb750020442d02`。未执行 tag release/桌面全平台最终产物、稳定 release signing 未配置，adb 当前无设备；APPID-001、BANDROID-038 与 Android 总 goal 仍保持 `IN PROGRESS`。

2026-10-10 Android process-role 与配置 IPC 切片：Kotlin 根据进程名映射 `Ui`、`VpnOwner` 或 `Unsupported`；VPN 配置通过 `ApplyVpnConfiguration` typed request 从 UI 送到 owner。新增 `VpnProcessState`，将 `StartRequested`、`Starting`、`Foreground`、`TunnelEstablished` 与 Rust callback 返回后的 `Running` 分开；Binder 启动请求最多等待 12s，只有最终 `Running` 才回给 Rust。`CoreApplication` 收到 `Starting` 会失败并回滚新启动的内核，不能把“已接受”显示成成功。Stop/revoke 从服务内改用仅停 tun2proxy worker 的 `stop_vpn_worker()`，避免 Kotlin → Rust 完整 `stop_vpn()` → Binder → 同一 Kotlin service 的递归停止；跨进程 StopProxy 等 `Stopped` readback。worker 异常退出会请求关闭 Android VPN service；Mihomo child stop 限时 3s graceful + 2s force，超限保留失败。`cargo nextest run -p infiltrator-android -p infiltrator-application` 660/660、0 skip；Gradle unit tests 20/20，`:app:assembleDebug` 成功；当次 manifest guard 输出 0 missing（历史结果），aapt identity guard、fmt/diff check 通过。Compose universal debug APK SHA-256 `55252c046edfd8ab9349a4d7381af7aee4154931f3dd744aaf13a9958215d0ca`，Bevy debug smoke APK SHA-256 `d751545fd1914aa44073fc9d538a488f446cfd98fb708a36eadb750020442d02`。启动/停止路径尚无 Android runtime 证据，adb 没有设备；`:vpn` process death 的子进程回收、真实权限弹窗和流量仍开放，BANDROID-001/036/037 与总 goal 保持进行中。

2026-10-11 per-app routing activation lease 收口：旧 `checkVpnRoutingRevision`/`compareVpnRoutingRevision` Kotlin API、对应测试和“Builder 前二次读取但仍有竞态”的旧描述全部删除。`:vpn` CoreOwner 通过 typed `AndroidVpnRoutingActivationResult` 发放 revision-fenced lease；lease 覆盖 `Builder.establish()`、TUN FD handoff、Rust worker 启动和 Running 转换，期间路由修改 typed fail closed。Rust Android/application/contract `cargo nextest` 1049/1049、零跳过；Compose/Bevy JVM 单测与 debug universal APK assemble 均成功；双 flavor aapt identity guard、`cargo fmt --check`、`git diff --check` 通过。Compose debug universal APK SHA-256 `ecf958e40ae289e45481ee1e6bd041738b0dd6e90333774dd9d0bafb1414690a`，Bevy debug universal APK SHA-256 `55c92f0f8b66a49b67aba073382fdb68a58d53a4dbc0cd322fb624a481e860ab`。Android SDK `adb devices -l` 无设备，故 Builder/FD 异常设备行为和真实多应用流量保持 OPEN；BANDROID-042.4 在该日尚未办结；后于 2026-10-13 经 Rust nextest `cc3c0959-7e30-467e-a1a5-15eca68fca48`（2673 passed、6 skipped）与隔离 Kotlin JUnit 108/108 办结，设备证据仍开放。

2026-10-11 HostProtocol V3/Core capability/Mihomo asset/模拟器复验：生命周期 snapshot 的 128 位 session token 改为十进制 JSON 字符串并升协议至 V3；旧 V2 HostProtocol 不接受，未增加 fallback。Fake `ProxyControlOperation::{Start, Stop}` 与 `LifecycleComposite` admission 已退役；Core Start 只走 `CoreLifecycle(Start)` capability，VPN 授权/启停由 typed lifecycle child 执行，StopProxy 只停 VPN。Android Gradle 改为按 `packaging/mihomo-assets.json` 对 arm64-v8a/x86_64 下载或缓存资产做 SHA-256 校验后暂存，不再从旧 vendor 路径直接复制。全 workspace `cargo nextest` 4466/4466、Compose/Bevy JVM suites 各 102/102、asset guard tests 6/6、结构检查通过；其后新 Manifest guard 的专项 11/11 与完整质量 Python tests 71/71 也通过。Gradle Compose/Bevy x86_64 Debug APK 构建成功，aapt 身份为版本 0.41.0、Compose versionCode 41000、Bevy versionCode 16787712。APK SHA-256：Compose `0e03af1f97f1d579b04a3f3f69ee7a98bdf8b4b0d8fabab9b640eb7f23f07ae4`，Bevy `c74c799a1a0a9088d767cde33ec98d1771e40fa9487d30974adcf6675b9b2737`。模拟器 Compose 冷启动显示 `:vpn` owner 与 Core/VPN 初始停止；显式 Core Start 后 `/version` 为 v1.19.32、Core readback Running、VPN service 未启动。通知权限对话框实际出现并被拒绝；随后从系统 VPN 设置 Forget 已保存授权，再显式 StartProxy 出现 `Connection request` 系统 VPN consent，Cancel 后 Core/VPN 都未启动。Bevy Activity/owner 启动且 Core 默认停止；首屏仍显示 `0 of 0 node locations` Geo Globe。模拟器 `1080x2340`、`density=440`、`navigation_mode=0`，Android `navigationBars` 与 `mandatorySystemGestures` 均占 `[0,2208][1080,2340]`；Bevy 底栏当前绘制/命中目标落在该系统保留区，`tap 430 2270` 返回 launcher，根因是安全区没有注入 driver，不是原始 Android 输入入口缺失。Compose Profiles 底栏点击成功切页。拒绝通知后的授权 Start 曾暴露 `reported=Running, observed=Starting` readback race；当时的 typed receipt 源码改动后续审计发现仍只是 worker spawn acknowledgement，不能证明 TUN ready，BANDROID-042 继续 OPEN。该次 x86_64 模拟器不代表真机、实际允许授权后的 VPN 流量或后台验收；Binder death/rebind、TUN/VPN 成功、Bevy safe-area touch/layout 与 AndroidTest instrumentation 仍 OPEN。

- 2026-10-10 身份与系统通知授权防漏补强：release APK 的 GitHub Actions artifact 分组名改为从注册产品前缀导出，并由 `android-product-identity-guard.py` 强制校验；新增负例覆盖硬编码分组名，Bevy diagnostic smoke builder/verifier 的版本、包名、名称、图标与 APK 名也由 guard 检查。Compose/Bevy 在 VPN consent 之后请求 Android 13+ 通知权限；通知拒绝仍继续显式 StartProxy，并显示本地化说明。Bevy 已授权分支现在也把 `VpnAuthorizationResult::GRANTED` 回报 Rust，再进入通知/代理启动路径。身份测试 29/29、Python 质量套件 56/56、release-tag 源码身份守卫、当次 manifest guard 输出 0 missing（历史结果）、Compose/Bevy unit tests 与 debug APK builds 均通过；两个当前 debug APK 的 aapt package/version/name/icon 检查通过，SHA-256 Compose `ccc6c553c42fb5dc314e17bbb374e267126e0a44d74b95bac800b02f4db2ba36`、Bevy `bf5954f4a92bfbd10aad6c6670c199bdf9b742c14fbfadd0e6a3f2dc7e013ab0`。`adb devices -l` 无设备，真实授权弹窗/通知抽屉/进程证据与稳定 release signing 仍未验证，APPID-001、BANDROID-037/039 与总 goal 继续未完成。


## Android CI 与验证状态

CI 工作项已拆入 [todos/CODE_TODO.md](todos/CODE_TODO.md) §五（018~024/038 的在制与回执项）与 [todos/CODE_TODO_PAST.md](todos/CODE_TODO_PAST.md)（模拟器/真机执行项）；原文见 [.evidence/ARCHIVE-2026-10-11-old-board.md](.evidence/ARCHIVE-2026-10-11-old-board.md)。

## 当前跨端工作队列映射（2026-10-10；主题映射到现有 workstream，不另计数）

- **代理与运行态：**收口模式/节点/测速读写的真实回执、未知与失败状态、request/generation 隔离；Iced/Bevy/Slint 与 Compose/Bevy/Slint Android flavors 共用语义并完成各自证据。
- **Profile 与配置：**完成文档、选项、Mixin、订阅过滤的来源绑定事务；所有 surface 保存/恢复/取消/失败重试保留原文和实际来源，快照恢复以 CAS 拒绝覆盖新编辑。
- **Rules 与 DNS：**完成规则追踪/编辑的原生交互与来源校验；DNS Hosts、查询、缓存操作保留未知/失败/空结果区别，并在共享 application owner 执行。
- **脚本工作台：**补齐 Bevy 原生编辑、运行、清理和导出审阅；Iced/Bevy 共用实例、命令结果和取消语义。未实现动作明确返回 unsupported。
- **Parity 与发行：**只用当前源码证据关闭 FeatureId；平台/包/设备状态以当前候选产物的验证回执和 parity 脚本生成的证据报告为准，历史通过记录不替代当前验收。

## Core 后端中性化专项（2026-10-10，OPEN）

**目的：**让共享层表达产品意图和真实观测；配置格式、controller DTO、原生操作和版本能力留在 engine adapter。即使暂不发行第二种 core，也先把 Mihomo 迁入该边界。最终用真实 sing-box 反证设计。

**范围：**首期一个活动 core，可安装/切换多个 engine；profile 绑定 engine。并行运行多个 core 暂不支持。跨 engine 转换显式预览、校验并报告丢失；不造覆盖所有引擎的万能配置 schema。

**当前耦合点（代码审计，均为迁移项）：**共享 crate 的 Cargo manifest 没有直接依赖 Mihomo adapter，但源码语义仍不是 engine-neutral。

- `contract/error.rs` 已从 Rust API 移除 `InfiltratorError::Mihomo`，改为 `Backend`；仍有旧 `Mihomo` Serde tag 兼容别名待按显式迁移/退役规则审计。产品命令已统一为引擎中立的 `RoutingIntent`，adapter 内部负责转换成 Mihomo wire spelling；`SelectProxyNode` 命令及 Android `proxy_select` FFI 已退役。`contract/proxies.rs` 与 `domain::Proxy` 仍暴露 Mihomo/Clash DTO，待迁移并按 adapter 边界处置。
- `ports/runtime_gateway.rs` 混合代理节点、模式设置、切换、任意 JSON patch、连接、日志与 provider 查询，需拆成窄端口和 typed operation。
- `application/configuration_application.rs`、`profile_options_application.rs` 仍直接做 YAML 解析/patch/Mixin 合并；`ProfileApplication`、`ProfileOptionsApplication` 还引用 application 内的 `mihomo_profile_options_codec`，sidecar schema 没有 engine adapter seam。`profile_document_application.rs` 的 load/save 已改走 engine/format/schema scoped codec，但 YAML snippet 与实时本地预检仍是显式 Mihomo/YAML 扩展。`domain::{config,dns,fake_ip,tun,rules,proxy_providers,sniffer,mixin,proxy_nodes,profile_aggregator,profile_converter,yaml_edit}` 含 Mihomo/Clash schema、解析或渲染事实，需迁到 Mihomo adapter 或逐项证明为共同语义。
- `contract::{tun,lan,traffic_topology,active_exit,protocol_params,protocol_params_ext,aggregator,provider_cache,rule_provider_snapshot,dialer_chain,mrs_acceleration,offline_startup}` 的具体 controller/配置事实必须按共同能力或 engine extension 分类，不能默认跨引擎通用。
- Mihomo capability 的唯一证据源为 `adapter-operation-declarations.json` 与 target-scoped `mihomo-evidence-assets.json`；当前两条 Linux x86_64 锁分别绑定 v1.19.18/v1.19.32、adapter revision 2，并登记 24 个 operation evidence ID（21 个 wire 行为与 3 个共享订阅流程）。SHA-locked 双版本 smoke 也覆盖 subscription Import/Update/Refresh 与 Geo 更新触发；Geo 证据只证明异步触发请求被接受，不证明数据下载或安装；所有能力仅授权精确版本/target/revision。完整实时记录见本行列出的 run IDs：v1.19.32 `b5961bde-fa5a-4a01-8b1f-72cf3dfd0768` / `266b6d65-fb2c-4b73-bd91-f119b7423028` / `5ae2637c-f3b0-4d5e-8025-5346b720b43d` / `f2f24a6d-d409-42a4-a23c-c7bfa68ae6a2` / `8a359cd5-8c52-40d4-b159-9f3335f777cd`；v1.19.18 `ffabc9df-cb80-48d3-92a6-cf928f154c6f` / `c9a16fec-b62e-4c66-9673-c46ed9896922` / `9fea92e1-629e-4fd0-a571-be0cef4a7ee9` / `2d637629-c8b2-43f9-8984-96787c869604` / `7fa8109d-a73c-4454-afb0-e483ecdd04a7`。能力缺少目标证据的操作仍 fail closed。
- `EngineVersionBackendFactory` 目前只创建版本端口；desktop/Android 仍有直接装配 Mihomo runtime/client 的路径，完整 process/config/runtime `EngineBackendFactory` 未落地。共享层零 Mihomo crate 依赖和编译通过都不能作为中性化验收。

**本轮切片状态（CORE-007/008/009 仍 OPEN）：**已建立 `CoreIdentity/CoreScope`、操作级能力矩阵和 `ProfileDocument` 原始文档契约；`CoreApplication.execute_scoped` 对请求做 owner scope 校验，`SurfaceReader` 整次异步读取、`SurfaceSnapshot`、`LogSession`、Iced runtime subscription message 与 Mihomo desktop log stream 已有身份/代次栅栏。Iced/Bevy 桌面发行 ingress 只使用 UI 已接受的 owner snapshot scope；没有已观察 scope 时 CoreBound 命令 fail closed，terminal ack 按穷尽的 typed transition policy 验证（`Some(session) → None` 停止转移有独立策略）。当前源码已删除公开 CoreApplication unscoped execute/dispatch/dispatch_tracked 与 Bevy/Iced legacy ingress；Core-bound desktop ingress 只接受已观察 CoreScope。CORE-007 仍因 Android Binder death/rebind、Android consumers 的 scope fence 与设备证据开放。源码检查显示 Bevy Android 与 Compose 产品 IPC 都向 `:vpn` 唯一 owner 发送 scope；Gradle flavor、Binder death/rebind、APK/设备行为证据未完成。scope/local-profile ingress 切片扩展 nextest 2679/2679 通过。随后新增的 typed outbound inventory/selection 已进入 Desktop 与 Android `:vpn` 的同一 owner composition；选择带 observed revision、验证 selectable membership 并要求写后读回。新 typed `OutboundObservationSnapshot` 在同次 Mihomo `/proxies` observation 中发布 inventory 与 engine-neutral details，inventory/detail revision 独立；偏好与 probe 使用 scoped、revision-fenced command/readback。Core contract/ports/application/Mihomo API focused nextest **1,077/1,077** 通过，覆盖 stale/non-member/Unsupported probe 零副作用、偏好成员类型校验、Mihomo 同周期 observation 与 egress history。Compose 已接入 typed details/preferences/protocol studio 与 revision-scoped preference/probe consumer，并通过 65/65 Compose JVM tests；Mihomo v1.19.32 的 per-outbound delay probe 已通过真实进程 smoke，10 ms 正向结果经本地 HTTP CONNECT 代理读回，owner capability evidence 已登记；各 UI 仍只按已接受快照中的 owner-resolved capability 状态开放操作。Bevy 代理页、详情、收藏/搜索偏好、探测、group order 与 doctor self-heal 已迁到 typed outbound inventory/details/preferences 和 capability snapshot，demo/capture 不再合成旧代理 DTO；focused Bevy nextest **21/21** 通过。Iced typed outbound consumer 已完成：focused behavior nextest 9/9，typed demo/identity/inspection/tray nextest 11/11；完整 Iced package 运行由统一收口执行。Iced 已删除自己的旧 proxy DTO 投影、domain Proxy demo source、旧选择/inspection/per-node probe message 与产品 fallback；共享旧 DTO/偏好命令仍被非 Iced 消费者使用，不能据此宣称全局接口退役。桌面/Android 的完整 scope 与 Binder 设备验收仍开放。
`LoadProfileDocument`/`LoadProfileOptions` 是带 scope 的本地 store/codec 读取，不要求未知版本的运行时 ReadFeature 证据；配置写入和运行时副作用继续 capability fail closed，未扩宽生产 Mihomo evidence。`ProfileStore/ProfileWorkspace` raw-byte typed CAS 主路径及 CORE-010A codec seam 已通过 3314/3314 多端 unified nextest；`ProfileDocumentCodecPort` 按 engine/format/schema 区分 `Inspected(Valid/Invalid)`、`Unavailable`、`Unsupported`，Mihomo YAML codec 在 `mihomo-config`，Desktop 显式注入，Bevy/Iced 消费中性 snapshot。`InfiltratorError::Backend` 与旧 serde tag 反序列化兼容另通过 contract/Android/Iced nextest 963/963；该兼容别名待按同一退役规则审计。Android `:vpn` 仍有 profile、配置、数据和操作路径未完整归入唯一 owner/codec；APK/Binder 与真机验收未完成。旧 string profile operations、engine namespace 路径/索引、凭据、backup、snapshot/restore 和 WebDAV 仍 OPEN。Core lifecycle 从 readiness 回读实际活动版本；若版本为空或失败，identity.version 保持未知，依赖活动版本的操作继续 fail closed。CORE-008 的 Mihomo lock 已登记 24 个 operation evidence ID（21 个 wire 行为与 3 个共享订阅流程，Linux x86_64 / adapter revision 2），并已通过两个版本的 Rust `OutboundPort`（observation/list/select/readback/probe）、`RoutingIntentPort`（GET/SET/readback）、subscription Import/Update/Refresh 与 Geo trigger smoke。Geo 证据只表示异步触发被接受，不证明数据库下载或安装；Android owner 设备验收和 sing-box runtime operation 仍独立开放。未登记能力继续 fail closed。

### 目标契约

1. **实例身份。**在 contract 定义 `EngineId(String)`、`CoreInstanceId`、`CoreVersion { raw, parsed? }`、`CoreGeneration` 和 `CoreIdentity { engine, instance, version, generation }`。携带于 lifecycle snapshot/event、命令请求/回执、runtime stream、日志会话、ProfileDocument 和版本安装记录；切换时退役旧 stream/watchdog，旧身份一律拒绝。
2. **能力来源。**将 `CapabilitySnapshot` 改为按 `(engine, version, HostKind, adapter_revision)` 查询的操作级矩阵。每项分别记录 engine/host/adapter 判定，再算有效结果；状态至少含 supported、read-only、experimental、unsupported、version-unavailable、host-unavailable、adapter-missing。CORE-005 的 Mihomo 数据迁成 Mihomo adapter provider，保留原测试。能力门必须覆盖 `CoreApplication` 生命周期自有命令及 `CommandApplication` 命令；启停/清理等版本无关的安全动作与需要活动版本的业务操作采用不同 typed `CoreOperation`，不能为了方便混用一个宽泛能力或绕过 gate。
3. **Profile 文档。**`ProfileDocument { profile_id, engine_id, format_id, schema_version, raw_bytes, revision }` 是存储/来源 CAS 单位。通用 `ProfileIntent` 只含定义清楚的交集；引擎原生文档与未知字段原样保留。跨引擎转换产生新目标文档和逐字段 loss report，确认前零写入。
4. **配置端口。**共享 `ProfileApplication` 管身份、CAS、事务与回执；`CoreConfigurationPort` 管 validate/read_feature/apply_feature/apply_document/rollback。禁止 application 直接调 YAML parser 或用任意 JSON patch 假装通用。将现有 DNS/TUN/rules/providers/sniffer/Mixin/script 字段逐项标 common、engine-specific、unverified；Mihomo YAML patch 留在 Mihomo adapter。
5. **运行态模型/端口。**拆 `Proxy` 为原生配置文档和 `OutboundSnapshot { engine_scoped_id, display_name, observed_fields, available_actions, revision }`。只保留验证过的共同观测，Mihomo 专属字段放 namespaced extension；缺失值不补零/false。将 `RuntimeGateway` 拆为 observation/stream、outbound control、connection control、logs 等小端口；provider、Geo、Fake-IP、GC 各自 capability-gated。
6. **产品命令。**路由策略已统一为 scoped `SetRoutingIntent { intent }`：命令先校验观察到的 `CoreScope` 与 Core capability，再由 adapter 写入并读取 typed readback；Mihomo Script 以 adapter-owned `EngineDefined` 状态读回，写入返回 typed `Unsupported`，不提供 shared selectable choice。Outbound 选择已改为带 CoreScope、collection/outbound opaque IDs 与 observed inventory revision 的 `SelectOutbound`；旧 `SelectProxyNode { group, node }` 已退役。所有结果回读真实快照；写请求被接受不等于操作成功。
7. **组合/版本。**目标 `EngineBackendFactory` 根据 `EngineId` 创建 process/config/runtime 小 ports 的组合；当前版本切片只提供 `EngineVersionBackendFactory`，不能代称整条组合根。`DesktopProductSession` 与 Android composition 不直接构造 `MihomoRuntime`。Version provider、release channel、binary manifest、安装目录、digest、回滚 journal 全按 engine 分区；单活动 core 切换必须先关闭并退役旧 session。
8. **独立运行轴。**用 typed enum 明确表达 core service/process 生命周期与代理/VPN 暴露状态，两者不得由同一 `running` bool 推导。启动宿主服务不自动启动代理；默认状态是服务可用、内核/代理保持暂停，只有明确的用户意图和所需 Android VPN 授权通过后才启用。Core 快照分别回读进程、API、配置、VPN 与流量生效状态。

### Engine-neutral Core workstreams（已迁至看板）

CORE-007..016 的 workstream 状态、`CORE-011/012` 八个退役切片与 2A/3A 已拆入 [todos/CODE_TODO.md](todos/CODE_TODO.md)、[todos/CODE_TODO_PAST.md](todos/CODE_TODO_PAST.md) 与 [todos/CODE_TODO_BLOCKED.md](todos/CODE_TODO_BLOCKED.md)；原文（含逐条证据叙述与 run ID）见 [.evidence/ARCHIVE-2026-10-11-old-board.md](.evidence/ARCHIVE-2026-10-11-old-board.md)。切片顺序要点：1 → 2 → 3；4 依赖 CORE-009/010 配置边界；5 独立按 capability 收口；6 先做 typed ConnectionId/session stream 再让 slice 3 的 topology 消费；7 最后删除 `ManagedRuntime: RuntimeGateway` 继承根；8 作为 Admin 公共 API 切片单独完成。每一片只在新契约行为 nextest、旧生产符号/文案清零及真实后端证据达到该片要求后关闭。

**本轮版本源收敛：**`scripts/capture-bevy-live.sh` 不再硬编码 Mihomo v1.19.18；它按本机 Linux target 从 `packaging/mihomo-assets.json` 选择注册 asset，并校验压缩包与解压后二进制 SHA-256。语法检查通过；本机 `vendor/mihomo-linux-amd64 -v` 报告 v1.19.32，二进制 SHA-256 与 lock 的 `6de241191ca39f9ac1cd6e3194fc9303c19055d3549c8bdb4ff1349c89173819` 一致。这只证明本机资产与锁一致，不证明旧 v1.19.18 协议假设已对齐，也不替代真实操作能力矩阵。

### 迁移顺序与真实 sing-box 验收

**旧接口退役约束：**每个新 typed contract 切片必须在生产消费者全部迁完的同一切片内删除旧公开命令、port、DTO、FFI/wire 导出与过期说明；不留双轨、兼容 fallback 或仅换名入口。历史持久化格式仅能由显式版本迁移器读取。

1. **Core first：**CoreApplication 的 unscoped execute/dispatch API 与 Bevy/Iced 兼容入口已从当前源码退役；Desktop CoreScope 行为回归通过。此前完整 workspace nextest `272d97ba-6521-4a54-9ebb-ccfd66eb37f8` 为 4483 passed、0 failed、2 skipped，覆盖此前 `1cf84762-dd12-4f1c-8b5e-e819e16d9643` 暴露的 Bevy scroll/gesture/status-dot/widget 失败。widgets/UI focused nextest 与 Bevy Clippy 均已通过；本轮结构门和文档守卫通过。Android owner/Binder death/rebind 仍缺设备证据。Mihomo v1.19.18/v1.19.32 的 evidence lock 现含 24 个 operation ID（21 个 wire 行为与 3 个共享订阅流程）；双版本 typed adapter、Profile/DNS/Rules transaction、TUN configure、Profile.ApplyFeature、Rules.Trace、subscription Import/Update/Refresh 与 Geo trigger smoke 均通过。TUN 始终关闭；Rules.Trace 只读取真实 profile/outbound facts；Geo 证据只证明异步触发接受，不证明数据下载或安装。其余未登记操作继续 fail closed。CORE-011 的 probe settings 与 speedtest/jitter 源码迁移已完成，统一 nextest/Clippy 尚未验收；application-owned surface assembler 已迁为 composition 的 `ProductSurfaceReader`，application/composition nextest `710a8609-36fc-4166-a88c-3454a5dfa01f` 为 718 passed、5 skipped；Doctor 状态已改为显式 `DoctorObservation` 并将页面投影迁入 presentation，contract/application/presentation/composition nextest `23a77ba0-fb2c-405c-8ffc-8c026771fb8c` 为 1113 passed、5 skipped。Iced/Bevy consumer package 验证仍留 Core-first 阶段门后的统一批次。proxy group order 与其它 Gateway 生产消费者仍待迁移。详见 CORE-011/012 当前清单，迁完再同片删除旧 DTO/port/wire/描述。
2. 集成矩阵：真实 Mihomo 与真实 sing-box 各锁一个 release 版本/摘要；能力 fixture 另覆盖每种 engine 至少两个版本。两种真实进程跑同一 lifecycle/config/runtime conformance suite；只比较共同意图，不强求原生字段相同。
3. 配置用例：各自 native profile 通过对应真实 binary check/apply/readback；同一份小型 common ProfileIntent 分别渲染并验证关键语义；未知字段往返；错误 engine 加载拒绝；转换取消零写入、转换报告与输出逐字段匹配。
4. 运行态用例：对声明支持的 outbound list/select、probe、connections、logs 操作检查真实请求、回执和读回；逐项验证 Mihomo-only 能力在 sing-box 上返回 Unsupported。API 路径同名不能代替语义断言。
5. 端到端证据：Linux 至少跑真实二进制集成；同一共享命令从 Iced、Bevy、Slint 桌面入口及 Compose/Bevy/Slint Android flavor 到达唯一 owner/真实 adapter，验证成功、失败、取消、Unsupported 和旧 generation。不同平台结果互不代替。

**总完成门：**真实双 core 测试报告、锁定版本和 digest、native 配置校验/应用记录、能力差异表、unsupported 负向测试、Iced/Bevy/Slint 入口与三 flavor APK 证据全部归档。mock、trait 编译通过或配置解析测试不能关闭专项。

## 历史记录口径

已完成批次、重复测试计数和过期截图检查点不再逐条堆在本台账；可追溯证据保留在 `.evidence/`、归档文档和版本历史中。旧 CORE/FUNC/PLAT/UI/QA/UP/BEVY 路线的当前状态以现行架构、功能地图、平台矩阵和测试矩阵为准。下列 task code 仅供 `doc-governance-guard.py` 校验现有文档引用，不代表仍有独立实施任务：

- BEVY: BEVY-004 BEVY-005 BEVY-006 BEVY-007 BEVY-008 BEVY-010 BEVY-012 BEVY-013 BEVY-014 BEVY-015 BEVY-016 BEVY-017 BEVY-018 BEVY-019 BEVY-020 BEVY-021 BEVY-022 BEVY-023
- BEVY: BEVY-024 BEVY-025 BEVY-026 BEVY-027 BEVY-028 BEVY-029 BEVY-030 BEVY-031 BEVY-032 BEVY-033 BEVY-034 BEVY-035 BEVY-036 BEVY-037 BEVY-038 BEVY-039 BEVY-040 BEVY-041
- BSN: BSN-001 BSN-002 BSN-003 BSN-004 BSN-005 BSN-006 BSN-007 BSN-008 BSN-009 BSN-010 BSN-011
- REM: REM-001 REM-002 REM-003 REM-004 REM-005
- BSN: BSN-001 BSN-002 BSN-003 BSN-004 BSN-005 BSN-006 BSN-007 BSN-008 BSN-009 BSN-010
- CORE: CORE-001 CORE-005 CORE-006
- ECS: ECS-007 ECS-008 ECS-009
- FUNC: FUNC-001 FUNC-002 FUNC-003 FUNC-004 FUNC-005
- GAP: GAP-001 GAP-002 GAP-003 GAP-004 GAP-005 GAP-006 GAP-007 GAP-008 GAP-009 GAP-010 GAP-011 GAP-012 GAP-013 GAP-014 GAP-015 GAP-016 GAP-017 GAP-018
- GAP: GAP-019 GAP-020 GAP-021 GAP-022 GAP-023 GAP-024 GAP-025 GAP-026 GAP-027 GAP-028 GAP-029 GAP-030 GAP-031 GAP-032 GAP-033 GAP-034 GAP-035 GAP-036
- GAP: GAP-037 GAP-038 GAP-039 GAP-040 GAP-041 GAP-042 GAP-043 GAP-044 GAP-045 GAP-046 GAP-047 GAP-048 GAP-049 GAP-050 GAP-051 GAP-052 GAP-053 GAP-054
- GAP: GAP-055 GAP-056 GAP-057 GAP-058 GAP-059 GAP-060 GAP-061 GAP-062 GAP-063 GAP-064 GAP-065 GAP-066 GAP-067 GAP-068 GAP-069 GAP-070 GAP-071 GAP-072
- GAP: GAP-073 GAP-074 GAP-075 GAP-076 GAP-077 GAP-078 GAP-079 GAP-080 GAP-081 GAP-082 GAP-083 GAP-084 GAP-085 GAP-086 GAP-087 GAP-088 GAP-089 GAP-090
- GAP: GAP-091 GAP-092 GAP-093 GAP-094 GAP-095 GAP-096 GAP-097 GAP-098 GAP-099 GAP-100 GAP-101 GAP-102 GAP-103 GAP-104 GAP-105 GAP-106 GAP-107 GAP-108
- GAP: GAP-109 GAP-110 GAP-111 GAP-112 GAP-113 GAP-114 GAP-115 GAP-116 GAP-117 GAP-118 GAP-119 GAP-120 GAP-121 GAP-122 GAP-123 GAP-124 GAP-125 GAP-126
- GAP: GAP-127 GAP-128 GAP-129 GAP-130 GAP-131 GAP-132 GAP-133 GAP-134 GAP-135 GAP-136 GAP-137 GAP-138 GAP-139 GAP-140 GAP-141 GAP-142 GAP-143 GAP-144
- GAP: GAP-145 GAP-146 GAP-147 GAP-148 GAP-149 GAP-150
- PLAT: PLAT-002 PLAT-003
- QA: QA-001 QA-004
- UI: UI-003 UI-004 UI-005 UI-006
- UP: UP-001 UP-002 UP-003 UP-005
