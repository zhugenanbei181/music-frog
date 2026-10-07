# 双端产品平权与场景验收

Iced 和 Bevy UI 是同权、独立发行的产品。任何一端出现的新业务能力都进入双端能力并集；不存在默认主端、被动挂靠端或以 UI 框架选择业务语义的路径。平台能力轴与前端轴分开：平台确实缺能力时返回可见的 typed `Unsupported`；同一平台上前端缺入口或交互降级属于缺陷，不能登记为 accepted difference。

## 同异律

1. **语义完备**：用户意图、命令、结果、能力、失败与交互状态拥有纯中性类型。共享层不引入 toolkit 类型或按 Iced/Bevy 身份编译的分支。
2. **映射穷尽**：每端对每个共享语义完整实现，或通过能力契约展示带原因的 typed 不支持。静默丢弃、默认值、空列表、隐藏入口都不构成映射。
3. **同一事实**：同一事实到标签、状态、行序、搜索匹配和汇总的折叠只有一个 owner，归属于 domain/application/projection；UI 缓存只能回放并按 revision/generation 失效。
4. **语义平价**：同一命令的后果、失败、取消和可恢复性在两端相同。独立模态、详情抽屉、多 Tab、比较器、向导与时间轴必须有等价的可视化操作表面；Toast、静默写文件、按钮文案变化不能替代交互面板。
5. **一次折叠，多端渲染**：Iced 以 TEA 的 Model–Update–View 组织状态；Bevy 以 ECS、`bsn!` 场景树和 Observer 组织呈现。求同到语义为止，不到像素；每个布局、密度、手势差异必须对应一个共享语义。

## 三层证据不可互相替代

| 层 | 验收对象 | 合格证据 | 不能证明该层的材料 |
| --- | --- | --- | --- |
| L1 contract | 命令、结果与共享投影 | mock/headless 行为测试，精确测试 ID | 类型名、函数签名、文档状态 |
| L2 scenario | 实际控件、独立操作面与完整状态机 | 两端生产 update/observer 路径测试；精确覆盖成功、取消、关闭、错误、重试、空态、typed 不支持等适用分支 | 仅导航到页面、仅向 sink 提交命令、只测共享 DTO |
| L3 visual | 激活后的真实渲染帧 | 两端 1180×780 与 720×480 像素；激活标记、原生布局边界、交互区域像素、进程/窗口身份、源码/资源和二进制指纹、PNG SHA256、几何签名 | 静态页面图、源码截图、路由空挂、以前版本的像素 |

取消必须无副作用；危险操作必须从明确的确认面提交，并拒绝关闭后的重复确认。错误与提权引导需要可操作的恢复路径。小视口通过布局/滚动保留相同操作，不能删除复杂交互。

## 唯一事实源与机械闭环

- 场景身份与适用状态机分支：[`FeatureId::ALL`](../crates/infiltrator-contract/src/parity.rs)。当前验收全集由编译后的注册表确定，包含独立的规则列表草稿编辑场景；原 225 项能力仍保留为业务范围，不能因场景收敛丢失功能。
- 每个场景必须同时登记两端：[`cross_surface_manifest.tsv`](../scripts/parity/cross_surface_manifest.tsv)。
- 每端的 L1/L2 精确测试锚点与分支：[`feature_evidence.tsv`](../scripts/parity/feature_evidence.tsv)。`pending` 是明确缺口；`anchored` 只表明有被声明的测试，不自动等于测试通过或完整平权。
- [`resolve_surface_evidence.py`](../scripts/parity/resolve_surface_evidence.py) 从编译后的 Rust 注册表和 `cargo nextest list --message-format json` 解析事实，不读取生产源码。包名、测试二进制和完整测试名一起核对；删改测试、错误归属、漏端、漏层、重复单元和不完整分支均失败。
- [`visual_receipts.py`](../scripts/parity/visual_receipts.py) 校验实际像素和场景激活回执。标准与紧凑视口必须同时存在；两端不要求 PNG 哈希相等。schema 4 绑定独立归档的产品二进制、构建清单与运行资源，以及窗口和交互区域的像素签名。原生布局边界必须完整落在视口内，目标区域必须实际有内容；仅整页非空、路由或状态标志不能证明操作面可见。
- 证据完整性与变更影响分开。源码变化不撤销某构建曾通过的验收，也不删除累计成果；[`product_build.py`](../scripts/parity/product_build.py) 从 Cargo 实际生产 depfile 及声明的运行资源建立输入清单，相关输入变化进入复验评估，文档、测试文件和另一前端不因全仓哈希变化而抹掉该回执。共用源码可能影响两端，不能假设局部无影响。
- 新取证运行归档的可执行文件和运行资源；产物、取证记录与验收报告保存于 `.evidence/`，独立于 Cargo `target/` 缓存和 `cargo clean`。禁止把可被后续构建替换的 `target/debug` 文件当成永久证据。全仓指纹仅作旧检查点的来源追踪，不能作验收新鲜度门禁。旧 schema 3 保留历史范围；没有原始构建产物或输入清单的，登记待迁移/复验，不伪造来源绑定。
- 发布验收显式提供 `--product-builds`，将两端各选定一个构建 ID。累计已验收、因相关变更待复验、从未验收、该候选版本完整覆盖分别统计；`--require-complete` 只对选定候选产物放行，不能把不同构建的截图拼成一份发行证明。
- 完整验收使用 `--require-complete`；单场景验收使用 `--require-features <token>`。普通测试入口允许显式 pending，表示证据完整性检查通过，不能用该结果宣布平权完成。

## 审计范围与退役边界

79 个源码/文档状态字符串 guard 已从两个测试入口退役，历史脚本清单见 [`retired_source_guards.tsv`](../scripts/parity/retired_source_guards.tsv)。架构、导入、BSN、行数、测试布局、打包、i18n 等结构约束继续运行；禁止重新注册退役脚本作为行为证据。

225 项历史 `parity-ready` 标记统一降为 `legacy-unassessed`，表示旧 L1 声明待按本规范重新验收。既有测试与实现保留；旧状态和 `host-verified` 只说明原有证据范围，均不能外推为 L2/L3 平权。

首条修复场景为 `connections-close-all-confirm`：Bevy 原先通过再次点击原按钮执行，缺少独立取消表面；现使用独立确认模态，支持取消、Esc、返回导航关闭，重复原按钮不执行，关闭后的确认不能重放。Iced 保持其生产确认模态。其余场景必须继续逐条核验，不能由这一条修复推导全量端平。

进一步核对发现，Bevy 已有连接详情抽屉、测速详情模态和聚合编辑步骤；这不等于所有分支已闭环。自定义节点、聚合器与快照 Diff 已从页内卡片迁移为独立模态，原有表单/比较器与投影更新保留；这些新增表面仍需补齐完整状态机和像素验收，不能因模态存在就标为端平。规则追踪经核对在两端当前均为 Tracer Tab，不能凭示例 Token 名推断 Iced 已有独立抽屉。完整缺口由结构化清单及解析报告给出，不再维护另一份手写 Ready 表。

本规范参考本机 taskmanager 的 `docs/CROSSPLATFORM_STRATEGY.md`、`docs/ARCH.md` §8、`docs/UI_PARITY_AUDIT.md` 与 `scripts/parity/`；不沿用它的系统监视器领域模型，也不把其四端当前状态视为本项目证据。

## 原生帧与合成器截图绑定

Iced 场景就绪需经过原生 redraw 和 GPU 截图读取，构建 `view()` 不产生已渲染事实。每次取证保留 `rendered-frame.png`；合成器截图必须与该原生帧在实际交互区域内一致。解析器同时绑定原生帧路径、哈希和比较结果，阻断状态与几何已就绪但截图仍停在上一帧的回执。缺少原生帧证明或原始产物的旧回执保留历史状态并登记待复验；篡改回执、像素或产物才属于证据完整性失败。

## Bevy 开发规范与迁移验收

唯一实现章程为 [BEVY_UI_FRONTEND.md](BEVY_UI_FRONTEND.md)，全仓工程底线见 [CODE_QUALITY_BASELINE.md](CODE_QUALITY_BASELINE.md)。业务、页面、控件交互和截图激活使用最小 Query/资源/Commands/Observer，禁止整个 World、DeferredWorld、独占系统、App 包装器与全局服务定位器。页面插件一次注册资源和观察者；路由只管理实体，不能借生命周期 hook 清空未提交草稿。原生初始化使用受限 `On<Insert<T>>` Observer，SDK 状态与实体退役由声明的调度和延迟提交边界闭合。

本次迁移保留既有业务能力，并撤掉对应旁路及其调用者。结构门禁只证明访问和场景构造符合章程；原生键盘/指针/IME、确认取消、失败重试、语言刷新、路由恢复、同帧退役和实际渲染仍需行为与像素证据。

## 当前检查点

[证据治理报告](../.evidence/governance/20261007/evidence.json) 分开统计累计历史验收与显式候选构建，不再以全仓指纹把已验收成果归零。历史通过记录仅说明各自构建曾被验收，候选产物尚未选定时覆盖值为空，不能表示“从未完成”。schema 4 的产物归档、真实输入影响评估和独立候选选择已接入脚本；[恢复场景实际验收](../.evidence/runs/20261007T055320Z_1574533/acceptance.json) 已通过真实行为与双端双视口像素。本批复验范围和累计历史成果分别读取机器报告，不能外推为完整发行覆盖。

以下实施检查点按各自源码和时间保留，不能把其中的旧“当前像素为零”统计当作新的累计进度。

[独立观测与运行态 i18n 检查点](../target/parity/runtime-copy-20261007/checkpoint.json) 绑定已通过全仓回归的冻结源码。编辑器文档/选项读取状态与来源核验独立于列表页面，失败保留实际观测并阻断保存；配额只在确认同订阅来源、同代次和会话时保留旧值。类型化错误与下载取消复用共享文案，Iced update/state 纳入 i18n 结构扫描。编译注册表和真实测试发现已核验，完整场景验收与最终平台交付仍待闭环；本批次开始冻结源码补齐已实现场景的当前像素，不将测试数量作为完成率。

量化目标固定为注册表场景全集 × 两端的 L1/L2/L3 验收，以及 Iced 三平台、Bevy 五平台的最终交付；实现状态、测试绑定、累计验收、相关变更待复验和候选覆盖分别统计。当前数值读取证据治理报告与正式 acceptance 报告；历史验收计入累计成果，发行放行核对显式候选构建。

[编辑来源事务检查点](../target/parity/editor-source-20261007/checkpoint.json) 记录该历史源码版本的 3962 项全仓测试零跳过、严格 Clippy、Android arm64 无默认特性库检查与完整结构入口。文档和 Mixin 提交携带调用方实际观测的来源，返回本次提交的类型化内容；远端直接编辑保护在同一存储边界核验，凭据值不可用不解除已记录的归属。两端保存与独立丢弃使用共享来源/等待/终态状态，保留草稿与 typed Failure，拒绝旧回执；原生控件与实体退役已有相关行为复验。独立读取失效观测、完整原生读请求分支、紧凑布局与其它场景仍待收口；120 个端侧单元均无最终源码全量像素，发布继续阻断，goal active。

[配置观测与统一 i18n 检查点](../target/parity/profile-observations-20261007/checkpoint.json) 记录该源码版本的 3958 项全仓测试零跳过、严格 Clippy、Android arm64 无默认特性库检查和完整结构入口。配置文档/选项与实际应用回执已退出全局缓存，按产品实例、配置来源与读取身份隔离；同产品命令和 reader 复用一个实例。Mixin 使用完整 workspace 来源比较提交，保留过滤与规则注释/锚点。19 类错误说明与恢复建议统一进入共享语言资源，2431 个中英文键的参数集合一致。运行态业务错误、原生编辑器保存来源与独立失败状态仍待收口；当前 120 个端侧单元均未具备最终源码完整像素，不放行正式版本，goal active。

[真实业务事实检查点](../target/parity/real-facts-20261007/checkpoint.json) 记录该历史源码版本的 3950 项全仓测试零跳过、45 项专项回归、严格 Clippy、Android arm64 无默认特性库检查与完整结构质量入口。旧 UI 固定配额、伪造延迟样本及直接提交成功状态已撤掉；两端配额统一共享文案和状态投影，来源数据的零值与缺失保持区分，实际原生进度几何已验证。事务入口只回放实际回执，历史入口使用共享节点检查器及真实探测。后续配置观测与 i18n 迁移已使该检查点及之前恢复像素历史化，当前矩阵 120 个端侧单元仍待最终像素；独立失败观测、编辑器来源事务、其它场景和实际平台交付继续阻断完整发布，goal active。

[快照恢复与共享文案检查点](../target/parity/snapshot-restore-i18n-20261007/checkpoint.json) 记录该源码版本的干净重建、3945 项全仓测试零跳过、严格 Clippy、Android arm64 无默认特性库检查，以及结构质量入口与共享 i18n 零违规。恢复准备冻结完整内容和配置/选项来源；取消零写入，确认执行来源比较提交，权限失败保留同一审阅供重试。两端使用独立原生审阅层和真实类型化终态，完整历史与语言回放保留实体、选择和用户身份。[正式恢复场景验收](../target/parity/20261007T015836Z_459753/acceptance.json) 的四张标准/紧凑图已逐张目视核验，绑定实际模态几何、原生帧、源码与二进制；这批像素只证明准备好的审阅操作面，错误、重试与事务后果由行为测试证明。该源码版本的全量矩阵仍有 118 个端侧单元未闭合，恢复后的编辑器读回和实际平台交付继续 pending，goal 为 active。

[快照实例与类型化回执历史检查点](../target/parity/snapshot-owner-20261007/checkpoint.json) 记录该源码版本的 3933 项全仓测试（零跳过）、严格 Clippy、Android arm64 无默认特性库检查，以及 800 行、导入、受限 ECS 和工具回归。进程全局快照历史/Diff 缓存及跨 await 的测试锁豁免已撤；独立产品不能借用观测，克隆 reader 共享 owner。历史按配置名隔离，Diff 再核对当前文档字节哈希；另一配置的备份不退役正在查看的比较。desktop 命令与 reader 共同消费绑定 runtime 配置目录的配置、配置读取和快照实例，Iced 自建快照存储服务的路径已撤。创建、历史、Diff 和修剪必须提供类型化结果，拒绝 Unit、错配置、错保留上限、错存储身份与无效创建哈希。

当前仍有 53 处固定文案；动态解析核对 120 个真实测试锚点，但 120 个端侧单元均未闭环，没有该源码的正式交互像素回执。共享快照工作台的独立失败观测、来源/请求终态、完整历史/Diff 访问、恢复审阅/取消/比较提交与错误重试仍 pending；同步、DNS、完整脚本流程、其它场景和八种实际产品/平台交付也未完成。发布 goal 保持 active，下列检查点只保留其历史源码范围。


[编辑器与 Mixin 共享投影检查点](../target/parity/editor-projections-20261007/checkpoint.json) 记录上一源码的 3929 项全仓测试（零跳过）、严格 Clippy、Android arm64 无默认特性库检查，以及 800 行、明确导入、受限 ECS 与工具回归。保护、应用事务、视口与诊断文案下沉到共享 application；Mixin 的预设、阶段、列身份采用中性枚举，双端不再分别保存 locale key 与固定中文标签。原生回归确认语言变化保留脏缓冲区、Unicode 光标位置、焦点、保护覆盖与原实体，且不提交业务命令；实际预检失败原因在 Mixin 状态行可见，非法草稿仍为零提交。

本轮退役 36 处 Bevy 固定中文，余 53 处仍阻断完整本地化入口。动态解析验证真实注册表与 120 个测试锚点，但 120 个端侧单元仍未闭环；该批没有其源码的正式交互像素回执。编辑器来源/终态、Diff 历史与快照实例、同步会话/冲突、DNS、完整脚本流程和八种实际产品/平台交付继续 pending，发布 goal 保持 active。下列检查点与图像仅保留各自历史源码范围。


[配置与同步真实观测检查点](../target/parity/profile-sync-observations-20261007/checkpoint.json) 记录上一源码的 3925 项全仓测试（零跳过）、严格 Clippy、Android arm64 无默认特性库检查，以及 800 行、明确导入和受限 ECS 检查。WebDAV 配置开启不再冒充连接成功，冲突面只回放实际字段，历史失败与缺失大小保持显式状态；两端共用可选流量和计划折叠，缺字段不补零，零总额不冒充无限制，未观测周期不冒充 Cron。原生回归验证语言、实际输入和字段实体保留，以及冲突容器和 SDK 禁用状态。

[该源码启动图清单](../target/parity/profile-sync-observations-20261007/sync-final/manifest.tsv) 的两张标准/紧凑暗色图已目视核验，只证明此源码的启动布局与缺字段禁用外观，不计专用场景 L3。该批源码使更早的交互回执历史化；当时解析 120 个端侧单元待新像素闭环。该批退役 50 处旧文案，余 89 处阻断当时完整质量入口；同步会话与来源关联冲突命令、快照实例观测、订阅草稿来源/终态及其它业务和八种实际产品/平台交付继续保持 pending，发布 goal 为 active。下列记录只保留各自历史源码范围。

[共享观测与原位文案检查点](../target/parity/localized-observations-20261007/checkpoint.json) 记录该源码版本的 3918 项全仓测试（零跳过）、严格 Clippy、Android arm64 无默认特性库检查，以及零 World/App world 入口、零查询/参数 lint 豁免、800 行与明确导入检查。测速指标、历史、系统开关与拓扑状态采用共享折叠；未知和真实零值分开，抖动要求至少两个成功 RTT，实际丢包率独立保留。偏好开关读取真实值、提交明确布尔命令，原生状态、失效门控、无障碍与主题对比度有行为证据。契约中的旧中文开关/HUD 显示方法已撤，语言回放保留结果、模态与原生输入。

三个场景的[正式联合验收](../target/parity/20261006T225729Z_3672238/acceptance.json) 绑定 32 项行为锚点及注册表测试，双端双视口的 12 张真实交互图均已目视核验；该批次 6 个端侧单元对齐、114 个未闭环。图像证明 DNS 查询结果模态、语言选择成功态与测速详情，不外推为完整业务或平台平权。该源码版本仍有 139 处旧硬编码；DNS 来源/终态、脚本上下文与完整报告、八种产品/平台实际交付仍待闭环，发布 goal 保持 active。此前同轮图像暴露的零抖动伪观测已修复并重新取证；此前启动布局图仅保留其源码版本的历史范围。

[ECS 查询与概览视觉检查点](../target/parity/ecs-query-cleanup-20261007/checkpoint.json) 与其[旧联合验收](../target/parity/20261006T220039Z_3366268/acceptance.json) 已成为历史记录。59 处查询/参数豁免撤退、角色排除与共享设计令牌修复仍保留，旧源码图像不能替代当前验收。

[受限 ECS 与启动资产检查点](../target/parity/scoped-ecs-20261007/checkpoint.json) 记录上一源码的 3908 项测试、受限启动资产、实体/色彩保留及输入渲染查询；其[联合验收](../target/parity/20261006T212058Z_3176219/acceptance.json) 与图像已成为历史记录。

[DNS 原生输入与宿主网络观测检查点](../target/parity/network-input-20261007/checkpoint.json) 记录上一源码的 3905 项测试、18 项共享字段标签与原生缺字段修复、IME 事务及宿主状态门控。其[联合验收](../target/parity/20261006T205904Z_3079317/acceptance.json) 与旧源码图像保留为历史记录，不能替代当前验收。

[原生多行编辑与 Bevy 工作台检查点](../target/parity/native-script-20261007/checkpoint.json) 记录上一源码检查点的全仓 3901 项测试、严格 Clippy、Android arm64 无默认特性库检查、原生键盘/IME 与审阅取消/权限重试证据。该源码版本的标准暗色与紧凑浅色启动图只证明当时布局和 SDK 调度能启动，不计入脚本场景 L3。预设、完整报告下钻、导出上下文及专用像素仍待闭环，两个脚本 Token 保持 pending；228 处旧硬编码中文仍由完整质量入口阻断。DNS 输入与宿主网络投影的后续改动已使此检查点成为历史记录。此前的[脚本会话检查点](../target/parity/script-session-20261007/checkpoint.json) 与旧联合场景图像保留为历史记录，不能替代当前验收。

规则集、MRS、JSON 编辑状态、逻辑规则和应用分流文案由共享 application owner 折叠，两端从定义模块导入。应用分流直接消费领域枚举，选中应用是否代理与策略标签一致；语言回放保留实际 OS 名称、输入、焦点、选择与 IME，不触发写入。其余应用选择、添加流程与来源关联命令仍待闭环。取证发现的 Bevy 紧凑规则编号与类型重叠已修复，重复类型文案已撤；新的激活器同时核验首行关键文本独立边界，原机器通过批次已退役。列表完整虚拟滚动、行高与原生布局关系以及操作图标辨识仍需进一步视觉验收。

统计检查器的 Tab、分页、审计目标、二次确认与清空状态机由共享 `rule_statistics_workbench` 与投影拥有，两端提供原生操作面。确认摘要只折叠一次；取消不改草稿，确认按源身份和稳定行 ID 再校验，只暂存停用，显式保存才写入。来源变化保留审阅并禁用确认；同源读取失败保留旧统计并标记失效。清空命令返回来源、递增版本与实际清除数量的类型化回执，`Unit` 不能替代成功；旧终态不能替代新请求，读回先于或晚于终态到达均按提交时版本处理。原生测试覆盖权限重试、未知值、取消零写入、来源失效及语言回放；Bevy 以临时交互屏蔽同步 SDK、无障碍和色阶，保留业务禁用状态并恢复存活输入焦点。

本地追踪计数与耗时绑定实际配置名和内容哈希，成功模拟按原规则行身份记录一次；命名子规则计入实际经过的根规则，同文本行分别保留观测。当前规则统计独立于历史追踪报告发布，reader 不增加计数。清空用例与模拟共用准入边界，核对实际源和统计 scope 后才清空；旧来源请求不能清空新来源。

概览生命周期、模式和来源回放共享文案；缺少版本不能显示为读取中，忙碌状态不能补造运行错误。原生语言与主题刷新保留实体、焦点、IME 和测速 URL 草稿，URL 接通实际 SDK 输入。规则行命中数只回放共享投影，明确为本地追踪统计；来源不符或发布窗口外的行保持未观测，真实零值仍为零。两端首屏先呈现列表，取证同时要求真实业务行及保存失败恢复操作面，标题与摘要不能代替列表。完整场景状态和规则虚拟窗口的实际几何仍需继续验收。

导出原生测试覆盖实际 Core 命令、隔离宿主文件、取消零写入、保存期间门控、权限重试、来源退役和显式不支持；Iced 固定 `/tmp` 写入及假成功提示已撤掉。导出像素只覆盖实际失败与恢复面板，审阅、待保存和成功回执还需增加视觉状态证据。

遥测由共享 owner 区分未观测、真实零值与同会话失效值，新代次或 token 清除旧速率和曲线。Iced Runtime 原先的占位零值与私有传输状态旁路已撤掉；Bevy Overview 转换保留共享 waveform，空/单样本不回退旧 UI 历史。实际像素覆盖失败后的零值保留、可读原因与两种视口；首次未观测、当前零值、来源退役和语言切换分别有行为证据，仍需补充对应视觉状态。取证中发现的 Iced 错误文本零宽与 Bevy 未接通原生滚动均已修复，没有降低几何门禁。

脚本沙盒与导出已纳入专用场景注册表。原进程全局缓存已撤掉，桌面组合根为每个独立产品注入脚本与导出 owner，命令和 reader 共享同一实例。执行结果关联提交内容、操作身份和递增版本，清空报告不重置熔断保护；导出先冻结完整草稿与内容哈希，确认才调用宿主，取消后拒绝重复确认。Iced 使用原生多行编辑、共享工作台与独立审阅层；Bevy 已移除被动报告卡，接入 SDK 多行输入、实际 Core 命令终态、独立审阅层与取消/权限重试。原生事件回归与窗口调度正在验收，完整预设/导出上下文、报告下钻和双视口像素仍有缺口。现有共享矩阵不能证明完整原生交互平权。

其它场景、本地化和实际平台交付继续阻断发布；Android 的 no-default-features 库检查不证明 APK、默认脚本引擎、设备或其它平台发行产物。

## 历史证据索引

下列证据对应各自验收时的构建并保留累计成果。发行放行按显式候选构建与适用复验判断，不能仅因无关改动抹掉通过记录，也不能直接把不同构建的历史像素拼成当前发行证明。数值与失败原因读取对应机器报告。

| 范围 | 证据索引 |
| --- | --- |
| 共享规则文案与领域分流策略 | [locale-policy](../target/parity/locale-policy-20261007/checkpoint.json) |
| 原生统计检查器与确认边界 | [statistics-inspector](../target/parity/statistics-inspector-20261007/checkpoint.json) |
| 规则统计来源与清空准入 | [trace-statistics](../target/parity/trace-statistics-20261006/checkpoint.json) |
| 状态文案与规则首屏 | [overview-copy](../target/parity/overview-copy-20261006/checkpoint.json) |
| 共享遥测与联合日志验收 | [traffic-observation](../target/parity/traffic-observation-20261006/checkpoint.json) |
| 日志导出共享基础 | [log-export-base](../target/parity/log-export-base-20261006/checkpoint.json) |
| 日志搜索与锁定 | [log-follow](../target/parity/log-follow-20261006/checkpoint.json) |
| 日志搜索 | [logs-search](../target/parity/logs-search-20261006/checkpoint.json) |
| 连接搜索与分组 | [connection-search](../target/parity/connection-search-20261006/checkpoint.json) |
| 连接分组 | [connections-grouping](../target/parity/connections-grouping-20261006/checkpoint.json) |
| 完整过滤场景 | [filter-capture](../target/parity/filter-capture-20261006/checkpoint.json) |
| 配置来源事务 | [profile-source](../target/parity/profile-source-20261006/checkpoint.json) |
| 全过滤草稿 | [filter-editor](../target/parity/filter-editor-20261006/checkpoint.json) |
| 订阅状态与去重 | [subscription-copy](../target/parity/subscription-copy-20261006/checkpoint.json) |
| 本地化与词法 | [localization](../target/parity/localization-20261006/checkpoint.json) |
| 共享模式入口 | [mode-seam](../target/parity/mode-seam-20261006/checkpoint.json) |
| 模式请求与读写边界 | [mode-actions](../target/parity/mode-actions-20261006/checkpoint.json) |
| 字段存在性与设置门控 | [config-presence](../target/parity/config-presence-20261006/checkpoint.json) |
| 运行控制观测 | [runtime-observation](../target/parity/runtime-observation-20261006/checkpoint.json) |
| 模式文案 | [mode-copy](../target/parity/mode-copy-20261006/checkpoint.json) |
| 稳定规则与源绑定表单 | [stable-rules](../target/parity/stable-rules-20261006/checkpoint.json) |
| 外壳真实读数 | [shell-readout](../target/parity/shell-readout-20261006/checkpoint.json) |
| 无整仓访问与原生帧 | [no-world](../target/parity/no-world-final-20261006/checkpoint.json) |
| 日志原生导出与联合验收 | [log-export-native](../target/parity/log-export-native-20261006/checkpoint.json) |
