# MusicFrog Infiltrator

跨平台 mihomo 代理客户端。Rust 全栈，双 UI surface（Iced 桌面 + Bevy 统一），Android companion。

## 架构

六边形 / Clean Architecture：
- **domain** (`infiltrator-domain`): 纯领域模型，零框架依赖
- **contract** (`infiltrator-contract`): 跨端共享类型与 viewport/capability 契约
- **ports** (`infiltrator-ports`): 入站/出站端口 trait
- **application** (`infiltrator-application`): use-case 编排，不直接依赖 Tokio
- **composition** (`infiltrator-composition`): 组合根，注入具体 adapter
- **host adapters**: `infiltrator-desktop`, `infiltrator-admin`, `infiltrator-android`, `infiltrator-ios`
- **UI surfaces**: `infiltrator-iced`（主桌面）、`infiltrator-bevy-ui` + `infiltrator-bevy-widgets`（战略统一）
- **mihomo 层**: `mihomo-api`, `mihomo-config`, `mihomo-platform`, `mihomo-version`, `mihomo-dav-sync`

UI 只提交意图、读取不可变结果；Rust 是所有 mihomo 控制操作的唯一产品边界。

## 构建

```bash
# 前置：拉取 mihomo 内核二进制
bash scripts/fetch-mihomo.sh

# 编译
cargo build --workspace

# Bevy UI 专项
cargo build -p infiltrator-bevy-ui
```

工具链：`rust-toolchain.toml` → stable, components: rustfmt + clippy

## 测试

```bash
# 全量（含质量守卫脚本 + nextest）
bash scripts/test.sh

# Bevy UI 专项测试
bash scripts/test-bevy.sh

# 格式与 lint
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
```

`scripts/test.sh` 会先跑 `scripts/quality/` 下的守卫（parity / i18n / import / session / hot-reload / crash-watchdog / core-channel / kernel-integrity / kernel-rollback / controller-auth / responsive-parity），然后跑 `cargo nextest`。

测试全部使用 mock/headless，不依赖外网或真实 mihomo 进程。

## 代码规范

- **单文件 ≤800 非空行**（`line-guard` 强制）
- **禁止 re-export / pub use 转发**：从定义模块的规范路径导入（`import-guard` 强制）
- **禁止 `use ... as 别名`**：同名冲突用完整路径
- **100% i18n**：零中文裸字面量，全部走 `locales_table`（`i18n-guard` 强制）
- Application 层不直接依赖 Tokio；具体异步实现由 composition root 注入

## 文档结构

- `docs/README.md` — 文档索引与权威关系表
- `docs/DUAL_SURFACE_PARITY_MASTER_PLAN.md` — 最高主控台账（双端同步）
- `docs/ARCHITECTURE.md` — 分层边界与数据流
- `DEFECTS.md` — 差距视图（证据索引，非执行顺序）
- `TESTING.md` — 测试命令与策略
- `USAGE_SPEC.md` — 用户功能说明
- `TODO.md` — 本地工作台账（.gitignore 忽略）

一个事实只有一个权威来源，其他文档只链接不复制。

## 常用开发任务

- 新增功能：先更新 `docs/FUNCTIONAL_MAP.md` 确认 owner，再落 contract → application → host adapter → UI
- 双端新特性：Iced 与 Bevy UI 同步推进，共享 contract/application，UI 表现可为 accepted difference
- UI 的"相同"指用户意图、数据语义、失败语义；像素/布局密度/手势可以是有记录的差异
