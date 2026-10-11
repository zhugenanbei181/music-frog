# 双端验收工具

> 层级：L3

本目录只实现双端验收注册表、nextest 锚点、构建身份和像素回执的解析与执行。

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
