# 多端验收工具

> 层级：L3

本目录只实现多端验收注册表、nextest 锚点、构建身份和像素回执的解析与执行。三个产品表面共享同一份业务目录（`infiltrator-contract::parity`）：Iced、Bevy 与 Slint。

```bash
# 只检查声明结构
python3 scripts/parity/resolve_surface_evidence.py --structure-only

# 编译注册表并发现实际测试
python3 scripts/parity/resolve_surface_evidence.py --report-json target/parity/evidence.json

# 验收一个业务（单表面）
bash scripts/parity/accept-surface-interactions.sh 'registered-feature-id'

# 批量验收：多个业务、两个表面、失败不中断
bash scripts/parity/accept-surface-interactions.sh 'a,b,c' --surfaces iced,bevy

# 批量验收：注册表里的全部业务
bash scripts/parity/accept-surface-interactions.sh --all
```

取证产物与验收记录写入 `.evidence/`。交互截图使用隔离的虚拟合成器；Pillow 用于像素几何分析。普通测试入口不会把 pending 或静态页面截图解释为平权通过。

## 批量验收的判定口径

一次验收对每个 `(业务, 表面, 视口)` 单元格给出 `certified` / `uncertified` 与原因，机器可读结果写在
`.evidence/runs/<stamp>/certification.json`（逐格）与 `certification.tsv`（逐格表格）。一个业务失败不会中断整批；
批次结束时只要还有一个被承载的单元格未认证，整批仍然失败。

每个单元格要同时满足三件事：

1. **L1/L2 行为锚点**：注册表里该业务的 contract 与 scenario 锚点存在、可发现且测试通过。缺失或失败会写明
   缺哪一层、哪个测试。
2. **L3 真实像素**：私有合成器栈里真实截图，进程 PID、窗口 ID、PNG 摘要、构建清单摘要、原生 bounds 全部绑定；
   Iced 还要通过原生渲染帧比对（`rendered_frame.py`）。
3. **L3 交互真实性**：表面必须真的激活了该业务。判据是黑盒像素差：同一页面同一视口的「未激活基线」截图与
   「已激活」截图在交互 region 内必须有可测差异；两次基线截图之间的差异是噪声地板，交互差异必须压过噪声地板。
   只挂载路由、只回退到页面级观察的表面会被判为 `no demo interaction`，不会因为写了 `activated=true` 就蒙混过关。

## 表面与载体

- 注册表 `feature_evidence.tsv` 与 `cross_surface_manifest.tsv` 按 `(业务, 表面, 层级)` 记录：每个业务在 Iced、Bevy、Slint 各有一格 contract 与一格 scenario，共 59 × 3 × 2 = 354 行证据、177 行清单。
- 契约目录 `FeatureSpec::slint_page` 登记该业务在 Slint 表面的载体页面；**空值表示 Slint 尚未实现该业务**，其 scenario 格必须保持 `pending` 并写明缺失能力，resolver 会拒绝任何「无载体却已锚定」的 Slint scenario。
- 验收脚本按载体计算：没有载体页面的业务不阻塞其它表面的验收，但会作为已登记缺口打印出来。
- 视觉回执由 `visual_receipts.py` 校验，`--iced-run`/`--bevy-run`/`--slint-run` 各自绑定一个真实 capture run；缺任一视口的像素回执都不能算作平权通过。

## 本目录的组成

| 文件 | 职责 |
|------|------|
| `accept_surface_interactions.py` | 批量验收：行为锚点 + 捕获矩阵 + 逐格报告 + 注册表收口 |
| `interaction_delta.py` | 交互真实性判据：已激活截图与未激活基线的像素差与噪声地板 |
| `visual_receipts.py` | 进程绑定的像素回执构建与校验（严格与批量两种模式） |
| `resolve_surface_evidence.py` | 注册表 × 真实 discovery × 像素回执的收口解析 |
| `product_build.py` | 不可变产品构建身份（构建清单、build id、输入指纹） |
| `evidence_progress.py` | 历史已验收基线与本批次候选的分开统计 |

`resolve_surface_evidence.py` 的 `--require-surfaces` / `--accepted-surfaces` 限定一次验收覆盖哪些表面：没有
scenario 锚点的表面不能被算作「已关闭」，只能作为 `not_certifiable` 记录在验收报告里。
