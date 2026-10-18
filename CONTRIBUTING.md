# 贡献指南 (Contributing to MusicFrog & Lurker)

> 层级：L1

感谢您对 MusicFrog 及其开源中立内核 Lurker 做出贡献！为了保持工程的高性能、确定性与跨平台稳定性，我们制定了以下贡献流程与质量约束。

---

## 行为准则

参与本项目即代表您同意遵守 [行为准则](CODE_OF_CONDUCT.md)。

---

## 开发工作流

### 1. 分支管理
- 本仓库遵循单一主干开发模式，所有改动均基于 `main` 分支演进；
- 提交前请确保拉取最新远程提交并保持工作区整洁。

### 2. 核心架构纪律
- **单一真源原则**：UI 表面（Iced/Bevy/Slint/Android）严格作为纯被动消费者；禁止在 UI 层沉淀第二个业务事实真源或推导逻辑；
- **强类型状态机**：多阶段操作必须使用显式 `enum` 建模合法状态迁移，严禁使用多个 `bool` 或标志位组合构建隐式状态机；
- **自底向上逐层演进**：遇到架构、契约或所有权未决时，严禁在上层修补绕行，必须自底向上从 Contract/Ports/Core 彻底裁决并闭环。

---

## 质量门禁与本地验证

提交 Pull Request 之前，必须在本机跑通以下全套门禁：

```bash
# 1. 代码格式检查
cargo fmt --check

# 2. 全量单元与契约测试
cargo nextest run --workspace

# 3. 严苛静态检查 (零警告)
cargo clippy --workspace --all-targets --locked -- -D warnings

# 4. 全仓结构与质量守卫套件 (包含 58 项守卫)
bash scripts/quality/check-structure.sh
```

---

## 安全问题报告

若发现潜在的安全漏洞，请勿公开提 issue，请参阅 [安全策略](SECURITY.md) 中的受保护渠道与维护者联系。
