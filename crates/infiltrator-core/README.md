# infiltrator-core

## 1. Role (单一职责与定位)
MusicFrog Infiltrator 的核心业务逻辑层，负责协调各底层模块（api, config, platform）并对上层 UI 和接口暴露统一的客户端状态机和生命周期。

## 2. Boundary (依赖边界与禁止耦合)
- 依赖上游: `mihomo-api`, `mihomo-config`, `mihomo-platform`, `infiltrator-shared` 等子模块。
- 禁止反向依赖: 绝对禁止依赖任何具体的前端 UI crate (`infiltrator-desktop`, `infiltrator-android`)。
- 零转发导入原则: 禁止一切 re-export（`pub use` / `pub(crate) use` 转发层，含 glob），禁止 `use ... as 别名`（`as _` 匿名 trait 导入除外）；一切导入走定义模块的规范路径。由 `scripts/quality/import-guard.py` 在 CI 强制。

## 3. Contract (核心公开类型与对外契约)
- `CoreEngine`: 管理代理内核的启动、停止与重启。
- `AppState`: 全局核心状态容器（流量、连接数、策略选中状态等）。
- `ProfileManager`: 订阅和配置文件的生命周期与持久化管理。

## 4. Verification (验证与测试指引)
- 运行测试（工作区全量统一入口）: `bash scripts/test.sh`
- 质量门禁: 遵守 `line-guard.py`（单文件 ≤ 800 行）与 `doc-link-guard.py`
