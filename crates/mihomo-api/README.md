# mihomo-api

## 1. Role (单一职责与定位)
处理与 Mihomo 内核的 API 通信（如 RESTful HTTP 调用、WebSocket 订阅等），封装内核控制逻辑。

## 2. Boundary (依赖边界与禁止耦合)
- 依赖上游: 仅依赖标准网络库（如 `reqwest`, `tokio`）及共享的数据结构层。
- 禁止反向依赖: 禁止直接依赖上层的业务逻辑 crate (`infiltrator-core`, `infiltrator-desktop` 等)。
- 零转发导入原则: 禁止一切 re-export（`pub use` / `pub(crate) use` 转发层，含 glob），禁止 `use ... as 别名`（`as _` 匿名 trait 导入除外）；一切导入走定义模块的规范路径。由 `scripts/quality/import-guard.py` 在 CI 强制。

## 3. Contract (核心公开类型与对外契约)
- `ApiClient`: 封装 HTTP 与 WebSocket 的主要控制客户端。
- `ProxiesState`: 节点状态的数据结构映射。
- `TrafficStats`: 流量统计信息。

## 4. Verification (验证与测试指引)
- 运行测试（工作区全量统一入口）: `bash scripts/test.sh`
- 质量门禁: 遵守 `line-guard.py`（单文件 ≤ 800 行）与 `doc-link-guard.py`
