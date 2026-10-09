# 双端产品平权与场景验收

> 层级：L2

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

这 79 个字符串清单式 guard 已**删除**，行为验收由动态的 [`resolve_surface_evidence.py`](../scripts/parity/resolve_surface_evidence.py)（真实 `FeatureId::ALL` + `cargo nextest list` 交叉核对）与结构性 [`doc-governance-guard.py`](../scripts/quality/doc-governance-guard.py) 承担。架构、导入、BSN、行数、测试布局、打包、i18n 等结构约束继续运行；禁止把已删除脚本重新注册为行为证据。

225 项历史 `parity-ready` 标记统一降为 `legacy-unassessed`，表示旧 L1 声明待按本规范重新验收。既有测试与实现保留；旧状态和 `host-verified` 只说明原有证据范围，均不能外推为 L2/L3 平权。

首条修复场景为 `connections-close-all-confirm`：Bevy 原先通过再次点击原按钮执行，缺少独立取消表面；现使用独立确认模态，支持取消、Esc、返回导航关闭，重复原按钮不执行，关闭后的确认不能重放。Iced 保持其生产确认模态。其余场景必须继续逐条核验，不能由这一条修复推导全量端平。

进一步核对发现，Bevy 已有连接详情抽屉、测速详情模态和聚合编辑步骤；这不等于所有分支已闭环。自定义节点、聚合器与快照 Diff 已从页内卡片迁移为独立模态，原有表单/比较器与投影更新保留；这些新增表面仍需补齐完整状态机和像素验收，不能因模态存在就标为端平。规则追踪经核对在两端当前均为 Tracer Tab，不能凭示例 Token 名推断 Iced 已有独立抽屉。完整缺口由结构化清单及解析报告给出，不再维护另一份手写 Ready 表。

本规范参考本机 taskmanager 的 `docs/CROSSPLATFORM_STRATEGY.md`、`docs/ARCH.md` §8、`docs/UI_PARITY_AUDIT.md` 与 `scripts/parity/`；不沿用它的系统监视器领域模型，也不把其四端当前状态视为本项目证据。

## 原生帧与合成器截图绑定

Iced 场景就绪需经过原生 redraw 和 GPU 截图读取，构建 `view()` 不产生已渲染事实。每次取证保留 `rendered-frame.png`；合成器截图必须与该原生帧在实际交互区域内一致。解析器同时绑定原生帧路径、哈希和比较结果，阻断状态与几何已就绪但截图仍停在上一帧的回执。缺少原生帧证明或原始产物的旧回执保留历史状态并登记待复验；篡改回执、像素或产物才属于证据完整性失败。

## Bevy 开发规范与迁移验收

唯一实现章程为 [BEVY_UI_FRONTEND.md](bevy-ui/BEVY_UI_FRONTEND.md)，全仓工程底线见 [CODE_QUALITY_BASELINE.md](CODE_QUALITY_BASELINE.md)。业务、页面、控件交互和截图激活使用最小 Query/资源/Commands/Observer，禁止整个 World、DeferredWorld、独占系统、App 包装器与全局服务定位器。页面插件一次注册资源和观察者；路由只管理实体，不能借生命周期 hook 清空未提交草稿。原生初始化使用受限 `On<Insert<T>>` Observer，SDK 状态与实体退役由声明的调度和延迟提交边界闭合。

本次迁移保留既有业务能力，并撤掉对应旁路及其调用者。结构门禁只证明访问和场景构造符合章程；原生键盘/指针/IME、确认取消、失败重试、语言刷新、路由恢复、同帧退役和实际渲染仍需行为与像素证据。

## 现行证据与迭代入口

当前证据、候选构建与复验状态以 `.evidence/` 下的治理报告与正式 acceptance 报告为准；场景身份、测试锚点与像素回执的机械闭环由 `scripts/parity/` 解析，缺口索引见 [DUAL_SURFACE_GAP_LEDGER.md](DUAL_SURFACE_GAP_LEDGER.md)。历史检查点与像素只保留各自源码版本的历史范围，不构成当前验收，也不外推为完整发行覆盖。
