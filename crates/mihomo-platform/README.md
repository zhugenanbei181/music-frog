# mihomo-platform

## 1. Role (单一职责与定位)
封装针对各个操作系统平台（Linux, Windows, macOS, Android）的底层差异，包括 TUN 权限管理、系统代理设置及服务自启等。

## 2. Boundary (依赖边界与禁止耦合)
- 依赖上游: `libc`, `winapi`, 平台相关的底层 FFI 绑定。
- 禁止反向依赖: 禁止依赖业务逻辑模块如 `infiltrator-core` 或 `mihomo-api`。
- 零转发导入原则: 禁止一切 re-export（`pub use` / `pub(crate) use` 转发层，含 glob），禁止 `use ... as 别名`（`as _` 匿名 trait 导入除外）；一切导入走定义模块的规范路径。由 `scripts/quality/import-guard.py` 在 CI 强制。

## 3. Contract (核心公开类型与对外契约)
- `SystemProxy`: 操作系统级代理配置接口。
- `TunManager`: TUN 虚拟网卡生命周期管理与权限校验。
- `ServiceController`: 系统级后台服务管理。

## 4. Verification (验证与测试指引)
- 运行测试（工作区全量统一入口）: `bash scripts/test.sh`
- 质量门禁: 遵守 `line-guard.py`（单文件 ≤ 800 行）与 `doc-link-guard.py`
