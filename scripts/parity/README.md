# 多端验收工具

> 层级：L3

本目录只实现多端验收注册表、nextest 锚点、构建身份和像素回执的解析与执行。三个产品表面共享同一份业务目录（`infiltrator-contract::parity`）：Iced、Bevy 与 Slint。

```bash
# 只检查声明结构
python3 scripts/parity/resolve_surface_evidence.py --structure-only

# 编译注册表并发现实际测试
python3 scripts/parity/resolve_surface_evidence.py --report-json target/parity/evidence.json

# 验收指定 feature 或完整候选
bash scripts/parity/accept-surface-interactions.sh '<registered-feature-id>'
bash scripts/parity/accept-surface-interactions.sh --all
```

取证产物与验收记录写入 `.evidence/`。交互截图使用隔离的虚拟合成器；Pillow 用于像素几何分析。普通测试入口不会把 pending 或静态页面截图解释为平权通过。

## 表面与载体

- 注册表 `feature_evidence.tsv` 与 `cross_surface_manifest.tsv` 按 `(业务, 表面, 层级)` 记录：每个业务在 Iced、Bevy、Slint 各有一格 contract 与一格 scenario，共 59 × 3 × 2 = 354 行证据、177 行清单。
- 契约目录 `FeatureSpec::slint_page` 登记该业务在 Slint 表面的载体页面；**空值表示 Slint 尚未实现该业务**，其 scenario 格必须保持 `pending` 并写明缺失能力，resolver 会拒绝任何「无载体却已锚定」的 Slint scenario。
- 验收脚本按载体计算：没有载体页面的业务不阻塞其它表面的验收，但会作为已登记缺口打印出来。
- 视觉回执由 `visual_receipts.py` 校验，`--iced-run`/`--bevy-run`/`--slint-run` 各自绑定一个真实 capture run；缺任一视口的像素回执都不能算作平权通过。
