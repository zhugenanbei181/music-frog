# MusicFrog 文档中心

本目录记录项目当前有效的架构、功能边界、跨平台策略和上游依赖规则。实现细节仍以受影响 crate 的源码和测试为准；本目录不承担临时开发流水。

## 阅读顺序

1. [README.md](../README.md)：产品定位、发行形态和用户可见能力。
2. [ARCHITECTURE.md](ARCHITECTURE.md)：Rust、mihomo、宿主与多个 UI 的分层边界。
3. [FUNCTIONAL_MAP.md](FUNCTIONAL_MAP.md)：按功能域查找唯一 owner、各端入口和待办编号。
4. [MIHOMO_CORE.md](MIHOMO_CORE.md)：Rust 操作 mihomo 的核心契约、生命周期和安全边界。
5. [FRONTENDS.md](FRONTENDS.md)：Iced、Tauri/Web、Android 的求同存异矩阵。
6. [PLATFORM_MATRIX.md](PLATFORM_MATRIX.md)：平台、架构、打包和验证状态。
7. [UPSTREAM.md](UPSTREAM.md)：Rust、mihomo、Web、Android 依赖的版本与升级流程。
8. [TEST_MATRIX.md](TEST_MATRIX.md)：功能域、UI、平台和真实 core 的分层回归矩阵。

## 文档与待办的权威关系

| 内容 | 唯一入口 | 规则 |
| --- | --- | --- |
| 产品当前状态 | `README.md` | 只写当前可验证事实，不写开发流水 |
| 架构和边界 | `docs/ARCHITECTURE.md` | 变更先更新边界，再改实现 |
| 功能归属 | `docs/FUNCTIONAL_MAP.md` | 一项功能只指定一个逻辑 owner |
| UI 求同存异 | `docs/FRONTENDS.md` | 每个前端必须显式选择 shared/local/accepted difference/unsupported |
| 上游依赖 | `docs/UPSTREAM.md` | 版本真相来自 manifest/lockfile/脚本，不在多个文档手抄 |
| 回归证据 | `docs/TEST_MATRIX.md` + `TESTING.md` | 测试命令和测试覆盖矩阵分开维护 |
| 工作台账 | 本地 `TODO.md` | 被 `.gitignore` 忽略，按任务 ID 与验收条件维护 |
| 缺陷视图 | `DEFECTS.md` | 只描述差距和证据，具体执行顺序回指 `TODO.md` |
| 用户使用说明 | `USAGE_SPEC.md` | 只描述已经存在或明确承诺的用户操作 |
| 测试执行规则 | `TESTING.md` | 记录命令、隔离策略和回归入口 |

## 维护规则

1. 一个事实只有一个权威来源，其他文档只链接，不复制完整表格。
2. 架构文档写稳定边界；临时方案、实验结果和未决事项写入本地 `TODO.md`。
3. TODO 只有在代码、行为测试和适用的平台/打包证据都具备时才能标记 `DONE`。
4. 上游升级必须同时检查 API/配置兼容性、锁文件、许可证、二进制校验和回滚路径。
5. UI 的“相同”指用户意图、数据语义、失败语义和可达性；像素、布局密度和手势可以是有记录的差异。
6. `ISSUE_RESOLVED.md`、`FIX_SUMMARY.md` 等历史报告不属于当前架构依据；需要复用其中结论时，先迁移为当前规则。
