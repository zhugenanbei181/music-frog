# Mihomo Despicable Infiltrator

**Mihomo Despicable Infiltrator** 是一个基于 Tauri v2 的卑鄙潜伏者，用于管理 `mihomo` 内核。它提供系统托盘、Web UI、订阅管理、TUN 模式和自动更新功能。

## 功能

- **托盘管理**: 快速启停内核、切换系统代理、切换配置、内核更新。
- **Web UI**: 内置 `mihomo-manager-ui` (面板) 和 `config-manager-ui` (订阅管理)。
- **订阅系统**: 支持订阅导入、自动更新、本地文件导入、外部编辑器编辑。
- **WebDAV 同步**: 多设备配置自动同步与云端备份（支持自定义间隔、启动时同步）。
- **增强特性**: TUN 模式、开机自启、管理员权限重启、GEOIP 数据库自动更新。
- **国际化**: 完整支持简体中文 (zh-CN) 和英语 (en-US)。
- **平台抽象**: CoreController/CredentialStore/DataDirProvider 抽象完成，增加 `apply_data_dir_override` 目录注入钩子。

## 研发规范

- Android 化推进仅在新 crates 内开发完成后，才允许调整 Tauri 导入。
- 未涉及 Tauri 导入变更时，不强制执行 Tauri 构建与测试。
- 一旦修改 Tauri 导入为新规划 crates，必须通过 Tauri 构建与测试（`cargo build -p "Mihomo-Despicable-Infiltrator"` 与 `cargo test --workspace`）。
- 构建与测试命令仅在仓库根目录执行，避免锁定子目录。
- 兼容层移除前必须保证新 crates 功能与原 crates 对齐，且 Tauri 应用功能完整可用。
- 变更需同步更新 `README.md` 与 `USAGE_SPEC.md`。
- 保持 Tauri 对外接口与行为兼容，避免破坏性改动。

## 规划流程

### 人话

整改分两阶段：Phase A 优先多端共用能力（DNS/Fake-IP/规则集/TUN 高级配置），Phase B 再做 Android 专项（分应用代理/VPN Service/Core 运行模式）。每项都有“归属 crate + 最小里程碑”，写入 `TODO.md` 与 `ANDROID.md`。

### AI 提示词工程

你是规划执行代理。输出必须包含：功能拆分→归属 crate→最小里程碑→文档落点（`TODO.md`/`ANDROID.md`/`CHANGELOG.md`）。先多端后 Android，持续更新文档状态，保持 Tauri 行为不变。

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
pnpm --dir webui/config-manager-ui build
pnpm build
```

## 目录结构

```
qiguai/
├── src-tauri/                    # Tauri 后端 (Rust)
├── crates/
│   ├── mihomo-api/               # Mihomo HTTP API 客户端 (跨平台)
│   ├── mihomo-config/            # 配置管理 (跨平台)
│   ├── mihomo-platform/          # 平台抽象 (Desktop/Android)
│   ├── mihomo-version/           # 版本管理 (Desktop)
│   ├── infiltrator-core/         # 跨平台业务逻辑
│   ├── infiltrator-desktop/      # Desktop 集成层
│   ├── infiltrator-android/      # Android 集成层
│   └── mihomo-dav-sync/          # WebDAV 配置同步引擎
│       ├── dav-client/           # WebDAV 协议客户端
│       ├── state-store/          # SQLite 状态管理
│       ├── indexer/              # 本地文件扫描器
│       ├── sync-engine/          # 三方对比同步引擎
│       └── platform-android/     # Android 平台入口 (预留)
├── webui/
│   ├── mihomo-manager-ui/        # 面板界面 (静态资源)
│   └── config-manager-ui/        # 配置管理界面 (Vue 3 + TypeScript)
├── vendor/                       # 第三方二进制
│   └── mihomo.exe                # Mihomo 内核 (Windows)
├── AGENTS.md                     # 开发规范
├── ANDROID.md                    # Android 平台规划
├── USAGE_SPEC.md                 # 路径与命名规范
├── CHANGELOG.md                  # 更新日志
└── TODO.md                       # 待办事项
```

## 最终 Crates 目标

- mihomo-api: 纯跨平台 mihomo HTTP API 客户端（client/types/proxy/connection）。
- mihomo-config: 纯跨平台配置管理（YAML/TOML），凭据通过 trait 注入。
- mihomo-platform: 平台抽象层（CoreController/CredentialStore/DataDirProvider），Desktop/Android 实现分离。
- mihomo-version: Desktop 专用版本管理（下载/切换二进制）。
- infiltrator-core: 跨平台业务逻辑（配置、订阅、调度、Admin API）。
- infiltrator-desktop: Desktop 集成层（运行时/系统代理/编辑器/版本）。
- infiltrator-android: Android 集成层（JNI/UniFFI 接口与运行时桥接）。
- mihomo-dav-sync: 跨平台 WebDAV 同步引擎（含 platform-android 入口）。

## 迁移现状

- mihomo-api/mihomo-config/mihomo-version 已从 mihomo-rs 拷贝当前实现，作为迁移基线。
- mihomo-platform 提供 trait + Desktop/Android 占位实现。
- infiltrator-core/infiltrator-desktop 已完成文件拆分，并改用新 crates（不再依赖 mihomo-rs）。
- despicable-infiltrator-core/mihomo-rs 兼容层已移除。
- Tauri 导入已切换到新 crates，并完成根目录构建与测试。

## 兼容层移除计划

在满足“新 crates 功能对齐 + Tauri 应用完整验证”的前提下：

1. `src-tauri` 完成新 crates 导入切换。
2. 根目录构建与测试通过，并完成手动功能验证。
3. 移除兼容层后，同步更新文档并完成回归验证。

## 详细文档

- [AGENTS.md](AGENTS.md): 开发规范与代码标准。
- [ANDROID.md](ANDROID.md): Android 平台支持规划与 Crate 重构计划。
- [USAGE_SPEC.md](USAGE_SPEC.md): 路径与命名规范。
- [CHANGELOG.md](CHANGELOG.md): 更新日志。
- [TODO.md](TODO.md): 待办事项与开发路线图。

## 平台支持

| 平台 | 状态 | 说明 |
|------|------|------|
| Windows | ✅ 已支持 | 完整功能 |
| macOS | 🚧 计划中 | 需适配系统代理 |
| Linux | 🚧 计划中 | 需适配系统代理 |
| Android | 📋 规划中 | 已完成平台抽象基础，详见 [ANDROID.md](ANDROID.md) |

## License

MIT

