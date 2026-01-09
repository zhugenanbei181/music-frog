# Mihomo Despicable Infiltrator

**Mihomo Despicable Infiltrator** 是一个基于 Tauri v2 的卑鄙潜伏者，用于管理 `mihomo` 内核。它提供系统托盘、Web UI、订阅管理、TUN 模式和自动更新功能。

## 功能

- **托盘管理**: 快速启停内核、切换系统代理、切换配置、内核更新。
- **Web UI**: 内置 `mihomo-manager-ui` (面板) 和 `config-manager-ui` (订阅管理)。
- **订阅系统**: 支持订阅导入、自动更新、本地文件导入、外部编辑器编辑。
- **WebDAV 同步**: 多设备配置自动同步与云端备份（支持自定义间隔、启动时同步）。
- **增强特性**: TUN 模式、开机自启、管理员权限重启、GEOIP 数据库自动更新。
- **国际化**: 完整支持简体中文 (zh-CN) 和英语 (en-US)。

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
│   ├── mihomo-rs/                # Mihomo SDK (配置管理、内核控制)
│   ├── despicable-infiltrator-core/  # 核心业务逻辑 (订阅、调度、Admin API)
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
| Android | 📋 规划中 | 详见 [ANDROID.md](ANDROID.md) |

## License

MIT
