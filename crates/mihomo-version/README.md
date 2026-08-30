# mihomo-version

## 1. Role (单一职责与定位)
处理 Mihomo 内核以及软件本身的版本获取、版本解析与更新检查逻辑。

## 2. Boundary (依赖边界与禁止耦合)
- 依赖上游: 仅依赖标准网络通信组件与版本语义解析（如 `semver`）。
- 禁止反向依赖: 禁止直接依赖平台模块或 UI 模块。
- 零跨 crate re-export 原则: 必须通过规范路径导入，禁止任意 glob 转发。

## 3. Contract (核心公开类型与对外契约)
- `VersionInfo`: 包含当前内核版本与前端应用版本信息。
- `Updater`: 负责检查、下载及替换新版本内核的控制类。
- `VersionReq`: 语义化版本约束和比对接口。

## 4. Verification (验证与测试指引)
- 运行测试（工作区全量统一入口）: `bash scripts/test.sh`
- 质量门禁: 遵守 `line-guard.py`（单文件 ≤ 800 行）与 `doc-link-guard.py`
