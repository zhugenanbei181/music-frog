# YAML 注释/锚点保真：现状、目标语义与迁移计划

apply 事务链上的配置写路径目前把用户配置经 `serde_yaml_ng` 反序列化 → 变换 →
再序列化。serde 模型里没有注释、锚点和排版，round-trip 之后手写注释的配置
会被"洗"成一份归一化文档。本文档立项修复这个数据级欠账：给出可复现的失败
证据、保真语义分级、基于 `crates/infiltrator-core/src/yaml_edit.rs`
（文本级 splice 原型，已带单测）的迁移路径，以及与 apply 事务的整合与回滚
语义论证。实现细节以 `yaml_edit.rs` 的模块文档与单测为准。

## 1. 现状失败样例（可执行证据）

输入一份带注释与锚点的最小配置：

```yaml
# 端口与模式（手写注释）
mixed-port: 7890
mode: rule   # rule / global / direct

rules:
  # 手写的兜底规则
  - &catchall MATCH,DIRECT
```

让它走过两条真实写路径（证据由
`yaml_edit::tests::characterizes_current_pipeline_fidelity_loss` 固化，可
用 `bash scripts/test.sh` 复现打印）：

**① mixin 标量覆写**（`mixin::merge_profile_with_config`，输入 +
`mode: global`），实际输出：

```yaml
mixed-port: 7890
mode: global
rules:
- MATCH,DIRECT
```

**② 规则行剔除**（`profile_options::strip_rule_lines`，剔除
`MATCH,DIRECT`），实际输出：

```yaml
mixed-port: 7890
mode: rule
rules: []
```

损失点清单（两条路径一致）：

| 损失 | 证据 | 原因 |
| --- | --- | --- |
| 文件头注释 | `# 端口与模式（手写注释）` 消失 | serde 模型无注释节点，反序列化即丢弃 |
| 行内注释 | `# rule / global / direct` 消失 | 同上 |
| 块内注释 | `# 手写的兜底规则` 消失 | 同上 |
| 锚点定义 | `&catchall` 消失 | serde 模型无锚点/别名，解析期解引为普通数据 |
| 排版漂移 | rules 子项缩进从 2 格漂移为 0 格、空行消失、`rules` 被写成 flow 风格 `[]` | 再序列化按 serde 自身风格归一化 |

补充：顶层键序（mixed-port → mode → rules）在本例中"碰巧"保留，因为
`serde_yaml_ng::Value::Mapping` 恰好是插入序实现——这是实现巧合而非契约，
serde_yaml / serde_yaml_ng 版本变更、或任何改用 `HashMap` 的中间结构都会
破坏它。注释与锚点的丢失则是确定性行为，这正是需要文本级工具的原因。

## 2. 目标语义分级

- **L1 注释保真**：独立注释行与行内注释逐字保留（含其前导/分隔空白）。
  yaml_edit 已达成（场景 a/b/c 单测）。
- **L2 键序与排版保真**：未触碰行零字节改动——键顺序、缩进宽度、引号风格、
  空行、换行风格（LF/CRLF 逐行保留）、BOM、缺失的文末换行全部原样。
  yaml_edit 已达成（round-trip 字节等同单测 + 各场景精确断言）。
- **L3 锚点全局一致**：编辑后文档中锚点/别名仍指向同一数据；涉及块移动、
  重命名、合并的变换需重写锚点引用以维持全局一致。本期只承诺
  "不触碰即不变"（锚点行原样保留、含锚点/别名的行可整体增删），锚点重写
  留待后续立项（见 §6 风险）。

## 3. yaml_edit 原型 API 与保证

`SourceDoc` 把文件建模为"物理行向量（逐行原文 + 逐行换行符）+ BOM 标记"，
不做完整 YAML 解析；编辑按缩进栈定位块边界。解析 + 渲染对任意输入字节
等同，编辑是唯一改动入口：

| API | 保证 | 失败模式（保守拒绝，不猜测） |
| --- | --- | --- |
| `parse` / `render` | 字节等同 round-trip | 多文档（第二个 `---`）、tab 缩进 → `Err` |
| `append_rule` | 新行插到 `rules` 块最后一个子项之后；块内既有注释/空行/锚点行逐字保留；缺块时文末新建（2 格缩进，沿用文档换行风格） | 块内含块标量、`rules: [flow]`、顶层序列文档、多行 rule → `Err` |
| `remove_rule` | 仅删除首个匹配子项的整物理行（含行尾注释）；相邻行不动 | 块缺失、行不在块内 → `Err` |
| `set_top_scalar` | 仅重写该行的值段；键原文、冒号后空白、行尾注释及其分隔空白逐字保留；其余字节不动 | 键缺失、键下挂嵌套块、值含未引号 `#`、多行值 → `Err` |

所有操作共享一条硬边界：`` | ``/`>` 块标量行及其缩进内容构成编辑屏障，
受影响行落在屏障内一律 `Err(BlockScalar)`；屏障外的编辑不受影响（有单测）。

## 4. 迁移路径

**先切 yaml_edit（写路径按风险从低到高）：**

1. **mixin 标量覆写**：`MixinConfig` 的标量键（mode/log-level/ipv6/
   allow-lan/mixed-port/secret/external-controller/external-ui）命中
   `set_top_scalar` 的能力域；mixin 仅含标量覆写时整条链路可保真。
2. **rules 增删**：`RuleMixin` 的 append/prepend/replace 写回与
   `strip_rule_lines` 的幂等剔除改为 `append_rule` / `remove_rule` 循环；
   文本级删除天然比"整块重建"更接近用户预期（其余规则行连缩进带注释不动）。
3. **接入点**：`profile_options::compose_content` 按操作类型分派——全部
   操作可保真时走 yaml_edit，任一操作超能力域时整体降级 serde 并打
   warn（降级是产品决策点：宁可提示用户"本次保存将丢注释"，也不静默）。

**必须留 serde 的路径（及原因）：**

- **filter 管线**（`filter::SubscriptionFilterPipeline::apply_to_yaml`）：
  它对 `proxies` 序列做结构级整编——过滤元素、重命名嵌套映射字段、去重、
  序号后缀。被操作的对象是"每项多行的映射块"，任何一次过滤都是对序列的
  大规模重排，文本级等价操作等于重写整个 `proxies` 块，保真无收益；且
  `proxies` 来自机器生成的订阅内容，不是用户注释区。
- **deep_merge**（dns/tun/sniffer/proxy-groups 表合并与 `custom-yaml`）：
  语义是"子树级覆盖"，覆盖点散布在任意深度，splice 粒度不够；这些块通常
  也无手写注释。这类路径维持现状，并在文档中标注"该写法不保真"。

**阶段划分**：阶段 1 = 原型 + 单测（本文档，已完成）；阶段 2 = compose
分派器 + mixin 标量路径切换；阶段 3 = rules 增删切换 + `strip_rule_lines`
退役；阶段 4 = L3 锚点重写立项。

## 5. 与 apply 事务的整合与回滚语义不变性

apply 事务（`apply.rs`）的五步——`validate_config`（yaml-rust2 语法校验）
→ temp 文件原子写 → reload/重启 → readiness 健康检查 → 失败回滚——全部以
**整文件字节串**为状态单位。yaml_edit 的定位只是 `new_content` 的"更聪明
的生产者"：调用方先 `SourceDoc::parse` 磁盘原文、执行 splice、`render` 出
新串，再交给原事务。

回滚语义不变性论证：

1. 事务的比较/恢复单位仍是字节串：`old_content` 取自上次落盘内容，
   回滚即 `atomic_write(old_content)`。yaml_edit 不改变这个单位，只改变
   新串的生成方式，因此 `RolledBack` / `RollbackFailed` 的判定路径与恢复
   字节完全不变。
2. yaml_edit 的输出仍通过同一道 `validate_config` 语法门；splice 拒绝的
   形状（块标量/多文档/flow）在解析期就被挡下，不会流入写盘步骤，"校验
   失败不落盘"的前置语义不变。
3. 保真带来的边际收益正好落在回滚场景：由于新串未触碰的行与旧文件逐字节
   相同，回滚后的 diff 只含真实编辑行，审计与历史快照（snapshot history）
   的可读性随之稳定——但这是性质改善，不改变事务的状态机。

## 6. 风险与不支持场景清单

**明确不支持（`yaml_edit` 保守返回 `Err`，绝不猜测）：**

- 多文档（第二个 `---`）；`...` 结束标记后接续内容的形状按多文档风险对待。
- `` | ``/`>` 块标量内部的任何编辑（含把块标量键改写为标量）。
- `rules: [flow, style]` 等内联值形态上追加块子项。
- tab 缩进（本就违反 YAML 规范）。
- 顶层是序列（非映射）的文档。
- 值文本中含未转义 `#`（注释起始符）——调用方须自行传入带引号的标量。
- 多行 plain/flow 标量、跨行的 rule 项。
- 重复顶层键：只认第一处（重复键本身是非法 YAML）。

**已知风险与缓解：**

- 新建行的"风格"（缩进取既有子项宽度，缺省 2 格；换行符沿用文档既有风格）
  是合理推断而非用户意图，文档化即可。
- 文末无换行的文件追加内容时，必须先给末行补换行符——这是 splice 成立的
  最小必要字节改动，已在单测中固定。
- 锚点行只保证"不触碰即不变"：若未来操作移动/复制含锚点的行，可能产生
  重名锚点或悬空引用，属 L3 范围，未实现前对应操作不得接入 yaml_edit。
- 保守拒绝 ≠ 自动降级：调用方（compose 分派器）必须显式决定"中止"还是
  "降级 serde 并提示丢注释"，禁止静默吞错。

**验证门槛**：`bash scripts/test.sh`（16 用例全绿）；
`cargo clippy -p infiltrator-core --all-targets -- -D warnings` 零告警；
`scripts/quality/line-guard.py`（800 行预算）零违规。
