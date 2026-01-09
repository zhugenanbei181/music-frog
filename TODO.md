# 待办事项

## ✅ 已完成

- [x] **安全性审计**: 消除代码中潜在的 panic 点（unwrap, expect, 索引越界）
- [x] **依赖升级**: 将所有 Rust 依赖项升级至最新稳定版本并解决 API 兼容性问题
- [x] **测试覆盖**: 为 `despicable-infiltrator-core` 核心模块添加初步单元测试
- [x] **国际化**: 为 Web UI 和托盘菜单添加多语言支持（zh-CN / en-US）
- [x] **WebDAV 配置同步功能 (完整实现)**:
  - [x] **P0: 核心协议与状态 (Foundation)**
    - [x] 初始化 `crates/mihomo-dav-sync` 多 Crate 工作区
    - [x] `dav-client`: WebDAV 协议客户端 (PROPFIND/GET/PUT/DELETE + If-Match 支持)
    - [x] `state-store`: SQLite 状态数据库 (文件哈希/ETag/同步时间映射)
    - [x] `indexer`: 本地文件树扫描器 (MD5 指纹计算，仅同步 .yaml/.yml/.toml)
  - [x] **P1: 同步引擎与安全 (Engine)**
    - [x] `sync-engine`: 三方对比算法 (本地/远端/上次状态)
    - [x] 原子写保护 (`.tmp` 临时文件 + `If-Match` 条件上传)
    - [x] 冲突处理 (双向修改时保存远端备份到 `.remote-bak-{timestamp}`)
  - [x] **P2: 集成与 UI (Integration)**
    - [x] 后端定时调度器 (集成到 `scheduler/sync.rs`)
    - [x] HTTP API (`/admin/api/webdav/sync` 手动同步, `/webdav/test` 连接测试)
    - [x] Vue UI 完整实现 (`SyncSettingsPanel.vue` + 国际化)
    - [x] 配置持久化 (`settings.toml` 中的 `[webdav]` 区块)
  - [x] **P3: 托盘菜单与兜底 (Robustness)**
    - [x] 托盘"同步与备份"子菜单 (状态显示/立即同步/同步设置)
    - [x] 同步结果系统通知 (成功/失败/文件数)
    - [x] 完整错误上下文与国际化错误提示
    - [x] 本地目录自动创建与网络错误兜底

## 🚧 待优化 (可选)

- [ ] **WebDAV 同步增强**:
  - [ ] 同步历史记录 UI (显示最近同步时间、操作数量)
  - [ ] 实时同步状态指示器 (进度条/同步中动画)
  - [ ] 支持排除特定文件/目录 (如 `cache/`, `*.tmp`)
  - [ ] 批量操作优化 (大量文件时的性能改进)

## 📋 计划中

- [ ] **CI/CD**: 配置 GitHub Actions 自动构建与发布
- [ ] **跨平台支持**: macOS / Linux 适配
- [ ] **性能监控**: 内核资源占用统计与可视化
- [ ] **Android 平台支持** (详见 `ANDROID.md`):
  - [ ] **Stage 0: Crate 重新规划** (1-2周)
    - [ ] 提取 `mihomo-api` (跨平台 HTTP 客户端)
    - [ ] 提取 `mihomo-platform` (平台抽象 trait + 实现)
    - [ ] 提取 `mihomo-config` (配置管理，使用 trait)
    - [ ] 创建 `infiltrator-core` (跨平台业务逻辑)
    - [ ] 创建 `infiltrator-desktop` (Desktop 集成层)
    - [ ] 更新 `mihomo-rs` 为向后兼容 re-export crate
  - [ ] **Stage 1: Android 基础设施** (2-3周)
    - [ ] 配置 Android NDK + cargo-ndk
    - [ ] 创建 `infiltrator-android` crate
    - [ ] UniFFI 集成与 Kotlin 绑定生成
    - [ ] 实现 `AndroidCoreController` (JNI)
    - [ ] 实现 `AndroidCredentialStore` (JNI)
  - [ ] **Stage 2: Android App MVP** (3-4周)
    - [ ] 创建 android/ 项目结构
    - [ ] Jetpack Compose UI 实现
    - [ ] 配置管理 + WebDAV 同步功能
    - [ ] 发布测试版本
  - [ ] **Stage 3 (可选): VPN 集成**
    - [ ] 集成外部 mihomo Android (AIDL)
    - [ ] 或嵌入 mihomo 库 (gomobile)
