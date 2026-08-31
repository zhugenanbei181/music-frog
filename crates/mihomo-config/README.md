# mihomo-config

## 1. Role (单一职责与定位)
负责解析、生成、校验和管理 Mihomo 代理的 YAML 配置文件。

## 2. Boundary (依赖边界与禁止耦合)
- 依赖上游: `serde`, `serde_yaml` 以及基础工具库。
- 禁止反向依赖: 禁止依赖任何具体的平台实现库和 UI 层代码。
- 零转发导入原则: 禁止一切 re-export（`pub use` / `pub(crate) use` 转发层，含 glob），禁止 `use ... as 别名`（`as _` 匿名 trait 导入除外）；一切导入走定义模块的规范路径。由 `scripts/quality/import-guard.py` 在 CI 强制。

## 3. Contract (核心公开类型与对外契约)
- `ConfigBuilder`: 用于编程方式构建 Mihomo 配置的构建器。
- `ProxyGroup`: 策略组的抽象数据类型。
- `RuleSet`: 路由规则的数据表达。

## 4. Verification (验证与测试指引)
- 运行测试（工作区全量统一入口）: `bash scripts/test.sh`
- 质量门禁: 遵守 `line-guard.py`（单文件 ≤ 800 行）与 `doc-link-guard.py`
