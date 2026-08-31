# infiltrator-desktop

## 1. Role (单一职责与定位)
MusicFrog Infiltrator 的桌面宿主集成与系统特权层，负责系统原生代理（Windows/Linux/macOS）、开机自启动、系统托盘解耦与 TUN Service 提权服务。

## 2. Boundary (依赖边界与禁止耦合)
- 依赖上游: `infiltrator-core`, `mihomo-platform`, `infiltrator-shared`
- 禁止反向依赖: 作为桌面支撑层，禁止被 `infiltrator-core` 或 `infiltrator-android` 依赖。
- 零转发导入原则: 禁止一切 re-export（`pub use` / `pub(crate) use` 转发层，含 glob），禁止 `use ... as 别名`（`as _` 匿名 trait 导入除外）；一切导入走定义模块的规范路径。由 `scripts/quality/import-guard.py` 在 CI 强制。

## 3. Contract (核心公开类型与对外契约)
- `SystemProxy`: 跨平台系统代理设置与 `ProxyOverride` 旁路列表。
- `TunServiceManager`: TUN 模式免特权 Service 服务控制。
- `AutostartManager`: 跨平台开机自启动管理。
- `TrayEventDispatcher`: 托盘菜单事件与窗口生命周期解耦。

## 4. Verification (验证与测试指引)
- 运行测试（工作区全量统一入口）: `bash scripts/test.sh`
- 质量门禁: 遵守 `line-guard.py`（单文件 ≤ 800 行）与 `doc-link-guard.py`
