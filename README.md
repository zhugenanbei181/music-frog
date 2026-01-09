# Mihomo Despicable Infiltrator

**Mihomo Despicable Infiltrator** 是一个基于 Tauri v2 的卑鄙潜伏者，用于管理 `mihomo` 内核。它提供系统托盘、Web UI、订阅管理、TUN 模式和自动更新功能。

## 功能

- **托盘管理**: 快速启停内核、切换系统代理、切换配置、内核更新。
- **Web UI**: 内置 `zashboard` (Metacubexd) 和 `config-manager-ui` (订阅管理)。
- **订阅系统**: 支持订阅导入、自动更新、本地文件导入、外部编辑器编辑。
- **增强特性**: TUN 模式、开机自启、管理员权限重启、GEOIP 数据库自动更新。

## 构建与运行

### 环境要求
- Node.js ≥ 18.18, pnpm ≥ 8
- Rust 1.75+

### 开发
```bash
pnpm install
pnpm dev
```

### 构建
```bash
pnpm --dir config-manager-ui build
pnpm build
```

## 目录结构
- `src-tauri/`: Tauri 后端 (Rust)。
- `crates/despicable-infiltrator-core/`: 核心业务逻辑 (Rust)。
- `mihomo-rs/`: Mihomo SDK (Rust)。
- `config-manager-ui/`: 管理界面 (Vue 3)。
- `zashboard/`: 面板静态资源。

## 详细文档
- [AGENTS.md](AGENTS.md): 开发规范。
- [USAGE_SPEC.md](USAGE_SPEC.md): 路径与命名规范。
- [CHANGELOG.md](CHANGELOG.md): 更新日志。