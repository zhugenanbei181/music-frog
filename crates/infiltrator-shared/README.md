# infiltrator-shared

## 1. Role (单一职责与定位)
三端（Iced / Web / Android）共享的基础模型与契约层，提供全端统一的强类型错误码、多语言国际化文案与多端用户意图注册表。

## 2. Boundary (依赖边界与禁止耦合)
- 依赖上游: 标准通用工具库 (`serde`, `anyhow`, `thiserror`)。
- 禁止反向依赖: 禁止依赖项目内具体业务/平台 crate (`infiltrator-core`, `infiltrator-desktop` 等)。
- 零跨 crate re-export 原则: 必须通过规范路径导入，禁止任意 glob 转发。

## 3. Contract (核心公开类型与对外契约)
- `InfiltratorErrorCode` & `StructuredError`: 全端强类型错误码与结构化排错建议。
- `get_localized_error`: 多语言文案解析与参数动态插值。
- `IntentRegistry` & `UserIntent`: 多端用户意图与能力支持矩阵。

## 4. Verification (验证与测试指引)
- 运行测试（工作区全量统一入口）: `bash scripts/test.sh`
- 质量门禁: 遵守 `line-guard.py`（单文件 ≤ 800 行）与 `doc-link-guard.py`
