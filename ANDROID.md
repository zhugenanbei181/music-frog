# Android Platform Support Planning

本文档分析当前代码库的 Android 平台兼容性，并规划移植路径。

> **核心原则**: 重构过程中必须保证 Windows/macOS/Linux Tauri 应用的完整功能，不得引入破坏性变更。
> **前置条件**: 仅当修改 Tauri 导入为新规划 crates 时，才要求通过 Tauri 构建与测试（`cargo build -p "MusicFrog-Despicable-Infiltrator"` 与 `cargo test --workspace`）。新 crates 必须开发完整后再调整 Tauri 导入；未涉及导入变更时不强制编译。变更需同步更新 `README.md`/`USAGE_SPEC.md`，并保持 Tauri 对外接口与行为兼容。
> **迁移完整性**: 兼容层移除前必须确保新 crates 功能与原 crates 对齐，且 Tauri 应用功能完整可用。

## Crate 重新规划

### 当前结构问题

```
当前 Crate 结构:
crates/
├── mihomo-rs/                    # ❌ 混合：跨平台+平台特定代码混在一起
│   ├── core/                     #    ✅ MihomoClient (HTTP API) - 跨平台
│   ├── config/                   #    ⚠️ 混合：YAML处理跨平台，keyring平台特定
│   ├── service/                  #    ❌ 进程管理 - Desktop专用
│   ├── version/                  #    ⚠️ 混合：版本API跨平台，下载Desktop专用
│   ├── proxy/                    #    ✅ ProxyManager (HTTP调用) - 跨平台
│   └── connection/               #    ✅ ConnectionManager - 跨平台
│
├── despicable-infiltrator-core/  # ⚠️ 混合：大部分跨平台，部分Desktop专用 (已移除)
│   ├── config.rs, profiles.rs    #    ✅ 跨平台
│   ├── subscription.rs           #    ⚠️ 依赖keyring
│   ├── runtime.rs                #    ❌ 依赖ServiceManager
│   ├── proxy.rs                  #    ❌ Windows注册表
│   └── editor.rs                 #    ❌ 外部编辑器
│
└── mihomo-dav-sync/              # ✅ 完全跨平台
    ├── dav-client/
    ├── state-store/
    ├── indexer/
    ├── sync-engine/
    └── platform-android/         # 预留的Android入口
```

### 目标结构

```
重构后 Crate 结构:
crates/
├── mihomo-api/                   # 新建: 纯跨平台 mihomo HTTP API 客户端
│   ├── client.rs                 # MihomoClient (从 mihomo-rs/core 提取)
│   ├── types.rs                  # API 类型定义
│   ├── proxy.rs                  # ProxyManager
│   └── connection.rs             # ConnectionManager
│
├── mihomo-config/                # 新建: 配置文件管理 (跨平台)
│   ├── manager.rs                # ConfigManager (不含 keyring)
│   ├── profile.rs                # Profile 类型
│   ├── yaml.rs                   # YAML 解析工具
│   └── subscription.rs           # 订阅逻辑 (使用 trait 抽象凭据存储)
│
├── mihomo-platform/              # 新建: 平台抽象层
│   ├── traits.rs                 # CoreController, CredentialStore, ...
│   ├── desktop/                  # Desktop 实现 (cfg 条件编译)
│   │   ├── mod.rs
│   │   ├── process.rs            # 进程管理 (从 mihomo-rs/service 提取)
│   │   ├── keyring.rs            # Keyring 凭据存储
│   │   ├── system_proxy.rs       # 系统代理 (从 core/proxy.rs 提取)
│   │   └── editor.rs             # 外部编辑器 (从 core/editor.rs 提取)
│   └── android/                  # Android 实现 (cfg 条件编译)
│       ├── mod.rs
│       ├── core_controller.rs    # JNI 调用 VPN Service
│       └── credential_store.rs   # EncryptedSharedPreferences
│
├── mihomo-version/               # 新建: 版本管理 (Desktop 专用)
│   ├── channel.rs                # 版本频道 API (可跨平台)
│   ├── download.rs               # 二进制下载 (Desktop)
│   └── manager.rs                # VersionManager
│
├── infiltrator-core/             # 重命名: 核心业务逻辑 (跨平台)
│   ├── config.rs                 # 应用配置
│   ├── profiles.rs               # 配置文件业务逻辑
│   ├── settings.rs               # 应用设置
│   ├── subscription.rs           # 订阅管理 (使用 trait)
│   ├── scheduler/                # 调度器
│   │   ├── subscription.rs
│   │   └── sync.rs               # WebDAV 同步调度
│   ├── admin_api/                # HTTP API handlers
│   └── servers.rs                # HTTP 服务器
│
├── infiltrator-desktop/          # 新建: Desktop 专用集成层
│   ├── runtime.rs                # MihomoRuntime (Desktop版)
│   ├── proxy.rs                  # 系统代理集成
│   ├── editor.rs                 # 编辑器集成
│   └── version.rs                # 版本管理集成
│
├── infiltrator-android/          # 新建: Android 专用集成层
│   ├── runtime.rs                # MihomoRuntime (Android版)
│   ├── uniffi.udl                # UniFFI 接口定义
│   └── lib.rs                    # JNI 入口
│
├── mihomo-dav-sync/              # 不变: 完全跨平台
│   ├── dav-client/
│   ├── state-store/
│   ├── indexer/
│   └── sync-engine/
│
└── (compat removed)              # 已移除 mihomo-rs 兼容层
```

### 最终 Crates 目标清单

- mihomo-api: 纯跨平台 mihomo HTTP API 客户端（client/types/proxy/connection）。
- mihomo-config: 纯跨平台配置管理（YAML/TOML），凭据通过 trait 注入。
- mihomo-platform: 平台抽象层（CoreController/CredentialStore/DataDirProvider），Desktop/Android 实现分离。
- mihomo-version: Desktop 专用版本管理（下载/切换二进制）。
- infiltrator-core: 跨平台业务逻辑（配置、订阅、调度、Admin API）。
- infiltrator-desktop: Desktop 集成层（运行时/系统代理/编辑器/版本）。
- infiltrator-android: Android 集成层（JNI/UniFFI 接口与运行时桥接）。
- mihomo-dav-sync: 跨平台 WebDAV 同步引擎（含 platform-android 入口）。

### 依赖关系图

```
                    ┌─────────────────────┐
                    │     src-tauri       │
                    │   (Tauri Desktop)   │
                    └──────────┬──────────┘
                               │
                    ┌──────────▼──────────┐
                    │ infiltrator-desktop │  ◄── Desktop 专用
                    └──────────┬──────────┘
                               │
          ┌────────────────────┼────────────────────┐
          │                    │                    │
          ▼                    ▼                    ▼
┌─────────────────┐  ┌─────────────────┐  ┌─────────────────┐
│ mihomo-platform │  │ mihomo-version  │  │ infiltrator-    │
│    (traits +    │  │   (Desktop)     │  │     core        │
│ desktop impl)   │  └─────────────────┘  │  (跨平台业务)   │
└────────┬────────┘                       └────────┬────────┘
         │                                         │
         │           ┌─────────────────────────────┤
         │           │                             │
         ▼           ▼                             ▼
┌─────────────────┐  ┌─────────────────┐  ┌─────────────────┐
│   mihomo-api    │  │  mihomo-config  │  │ mihomo-dav-sync │
│  (HTTP 客户端)  │  │  (配置管理)     │  │  (WebDAV 同步)  │
└─────────────────┘  └─────────────────┘  └─────────────────┘
         │                    │                    │
         └────────────────────┴────────────────────┘
                              │
                    ┌─────────▼─────────┐
                    │   标准库 + 跨平台  │
                    │   依赖 (tokio,    │
                    │   reqwest, sqlx)  │
                    └───────────────────┘
```

### Android 依赖路径

```
                    ┌─────────────────────┐
                    │    android/app      │
                    │  (Kotlin + Compose) │
                    └──────────┬──────────┘
                               │ JNI/UniFFI
                    ┌──────────▼──────────┐
                    │ infiltrator-android │  ◄── Android 专用
                    └──────────┬──────────┘
                               │
          ┌────────────────────┼────────────────────┐
          │                    │                    │
          ▼                    ▼                    ▼
┌─────────────────┐  ┌─────────────────┐  ┌─────────────────┐
│ mihomo-platform │  │ infiltrator-    │  │ mihomo-dav-sync │
│    (traits +    │  │     core        │  │  (WebDAV 同步)  │
│ android impl)   │  │  (跨平台业务)   │  │                 │
└─────────────────┘  └─────────────────┘  └─────────────────┘
         │                    │                    │
         ▼                    ▼                    │
┌─────────────────┐  ┌─────────────────┐           │
│   mihomo-api    │  │  mihomo-config  │           │
│  (HTTP 客户端)  │  │  (配置管理)     │           │
└─────────────────┘  └─────────────────┘           │
                                                   │
              注意: Android 不需要 mihomo-version  ─┘
                    (不下载二进制)
```

## Android 三块结构

1. **Rust NDK core**: 业务逻辑 + FFI 边界 + AndroidBridge 桥接
2. **Kotlin 原生管理界面**: UI + 生命周期 + 调用 Rust NDK API
3. **mihomo 二进制/so**: 核心运行方式（外部 APK / 内嵌 so / 纯配置模式）

## Android mihomo 二进制管理

### 问题分析

Android 上 mihomo 核心的运行方式与 Desktop 完全不同：

| 方面 | Desktop | Android |
|------|---------|---------|
| 二进制来源 | 下载 GitHub Release | 不适用 |
| 存储位置 | `~/.config/mihomo/versions/` | N/A |
| 启动方式 | `Command::new()` spawn 子进程 | VpnService API |
| 权限 | 普通用户权限 | 需要 VPN 权限 |
| 网络劫持 | 系统代理/TUN | TUN (VpnService) |

### Android 核心运行方案对比

#### 方案 A: 依赖外部 mihomo Android APK (推荐)

```
优点:
- 复用成熟的 ClashMeta Android / mihomo Android 实现
- 不需要处理复杂的 VPN 权限和 TUN 配置
- 用户可独立更新 mihomo 核心

缺点:
- 需要用户额外安装一个 APK
- 需要实现 AIDL/Intent 通信

实现:
┌─────────────────────┐      Intent/AIDL      ┌─────────────────────┐
│  Infiltrator App    │ ◄──────────────────► │   mihomo Android    │
│  (配置管理+同步)    │                       │   (VPN Service)     │
└─────────────────────┘                       └─────────────────────┘

CoreController Android 实现:
- start(): 发送 Intent 启动 mihomo VPN
- stop(): 发送 Intent 停止 VPN
- is_running(): 查询 VPN 状态
- controller_url(): 返回 mihomo API 地址 (通常 127.0.0.1:9090)
```

#### 方案 B: 嵌入 mihomo 库 (gomobile)

```
优点:
- 单一 APK，用户体验更好
- 完全控制核心版本

缺点:
- 需要用 gomobile 编译 mihomo 为 AAR
- 需要自己实现 VpnService 和 TUN 配置
- 编译复杂，维护成本高
- APK 体积增大 (~20MB)

实现:
┌─────────────────────────────────────────────┐
│              Infiltrator App                │
│  ┌─────────────────────────────────────────┐│
│  │           Kotlin UI Layer               ││
│  └─────────────────────────────────────────┘│
│  ┌─────────────────────────────────────────┐│
│  │         VpnService (Kotlin)             ││
│  │    - configureTun()                     ││
│  │    - startMihomoCore()                  ││
│  └─────────────────────────────────────────┘│
│  ┌─────────────────────────────────────────┐│
│  │       mihomo.aar (gomobile)             ││
│  │    - libgojni.so                        ││
│  └─────────────────────────────────────────┘│
│  ┌─────────────────────────────────────────┐│
│  │       Rust Core (.so via UniFFI)        ││
│  │    - infiltrator-android                ││
│  └─────────────────────────────────────────┘│
└─────────────────────────────────────────────┘

CoreController Android 实现 (嵌入式):
- start(): 调用 mihomo.aar 的 startCore() + 配置 VpnService
- stop(): 停止 VpnService + mihomo core
- is_running(): 检查 service 状态
- controller_url(): 本地 Unix socket 或 localhost
```

#### 方案 C: 纯配置管理 App (MVP)

```
优点:
- 最简单，可快速发布
- 专注于配置同步价值

缺点:
- 不控制 VPN，用户需手动操作 mihomo Android
- 体验割裂

实现:
┌─────────────────────┐                       ┌─────────────────────┐
│  Infiltrator App    │      手动导入配置      │   mihomo Android    │
│  - 配置编辑         │ ─────────────────────► │   (独立安装)        │
│  - WebDAV 同步      │                       │                     │
│  - 订阅更新         │                       │                     │
└─────────────────────┘                       └─────────────────────┘

CoreController Android 实现:
- start()/stop(): 无操作或抛出 UnsupportedOperationException
- is_running(): 尝试连接 localhost:9090 检测
- controller_url(): 用户配置或默认 127.0.0.1:9090
```

### 推荐路径

**Phase 1 (MVP)**: 方案 C - 纯配置管理

- 快速发布 Android 版本
- 验证 Rust 核心在 Android 的工作状况
- 收集用户反馈

**Phase 2**: 方案 A - 集成外部 mihomo Android

- 实现 AIDL 通信
- 提供一键启动/停止功能

**Phase 3 (可选)**: 方案 B - 嵌入式核心

- 如果用户强烈需求单一 APK
- 评估维护成本后决定

## 模块拆分详细计划

### 从 mihomo-rs 提取的模块

#### 1. mihomo-api (新建 - 完全跨平台)

**提取来源**: `mihomo-rs/src/core/`, `mihomo-rs/src/proxy/`, `mihomo-rs/src/connection/`

```rust
// crates/mihomo-api/src/lib.rs
pub mod client;      // 从 core/client.rs
pub mod types;       // 从 core/types.rs
pub mod error;       // 从 core/error.rs
pub mod proxy;       // 从 proxy/manager.rs (不含测试)
pub mod connection;  // 从 connection/

pub use client::MihomoClient;
pub use error::{MihomoError, Result};
pub use proxy::ProxyManager;
pub use connection::ConnectionManager;
```

**Cargo.toml**:

```toml
[package]
name = "mihomo-api"
version = "0.1.0"

[dependencies]
tokio = { workspace = true }
reqwest = { workspace = true }
url = { workspace = true }
serde = { workspace = true }
serde_json = "1.0"
futures-util = "0.3"
tokio-tungstenite = { version = "0.26", features = ["rustls-tls-native-roots"] }
thiserror = { workspace = true }
log = "0.4"
```

**无平台依赖，100% 跨平台。**

#### 2. mihomo-config (新建 - 完全跨平台)

**提取来源**: `mihomo-rs/src/config/` (不含 keyring 调用)

```rust
// crates/mihomo-config/src/lib.rs
pub mod manager;     // ConfigManager (重构后)
pub mod profile;     // Profile 类型
pub mod yaml;        // YAML 工具

// 使用 trait 而非直接依赖 keyring
use mihomo_platform::CredentialStore;

pub struct ConfigManager<S: CredentialStore> {
    config_dir: PathBuf,
    credential_store: S,
}
```

**关键变更**: `keyring` 调用改为通过 `CredentialStore` trait 注入。

#### 3. mihomo-platform (新建 - 平台抽象)

**职责**: 定义 trait + 提供 Desktop/Android 实现

```rust
// crates/mihomo-platform/src/lib.rs

// 公共 trait 定义
mod traits;
pub use traits::*;

// 条件编译：Desktop 实现
#[cfg(not(target_os = "android"))]
pub mod desktop;
#[cfg(not(target_os = "android"))]
pub use desktop::*;

// 条件编译：Android 实现
#[cfg(target_os = "android")]
pub mod android;
#[cfg(target_os = "android")]
pub use android::*;
```

**Desktop 实现文件结构**:

```
crates/mihomo-platform/src/desktop/
├── mod.rs
├── process.rs          # ProcessCoreController (从 mihomo-rs/service/)
├── keyring.rs          # KeyringCredentialStore
├── system_proxy.rs     # WindowsSystemProxy (从 core/proxy.rs)
└── editor.rs           # DesktopEditorLauncher (从 core/editor.rs)
```

**Android 实现文件结构**:

```
crates/mihomo-platform/src/android/
├── mod.rs
├── core_controller.rs  # AndroidCoreController (JNI 回调)
├── credential_store.rs # AndroidCredentialStore (JNI 调用)
└── jni_bridge.rs       # JNI 辅助函数
```

**Cargo.toml**:

```toml
[package]
name = "mihomo-platform"
version = "0.1.0"

[dependencies]
async-trait = { workspace = true }
thiserror = { workspace = true }
anyhow = { workspace = true }
tokio = { workspace = true }
log = "0.4"

[target.'cfg(not(target_os = "android"))'.dependencies]
# Desktop 依赖
keyring = { version = "3.6" }  # 平台 feature 在下面配置
sysinfo = "0.33"
dirs = "6.0"

[target.'cfg(windows)'.dependencies]
windows-sys = { version = "0.61", features = ["Win32_System_Threading"] }
winreg = "0.55"

[target.'cfg(target_os = "android")'.dependencies]
jni = "0.21"
```

#### 4. mihomo-version (新建 - Desktop 专用)

**提取来源**: `mihomo-rs/src/version/`

```rust
// crates/mihomo-version/src/lib.rs
pub mod channel;     // 版本频道 API
pub mod download;    // 二进制下载
pub mod manager;     // VersionManager

pub use manager::VersionManager;
pub use channel::Channel;
```

**注意**: 这个 crate 仅用于 Desktop，Android 不需要。

### 从 despicable-infiltrator-core 重构

#### infiltrator-core (重命名 + 重构)

**保留的跨平台模块**:

- `config.rs` - 应用配置
- `profiles.rs` - 配置文件业务逻辑
- `settings.rs` - 应用设置
- `subscription.rs` - 订阅管理 (重构为使用 trait)
- `scheduler/` - 调度器
- `admin_api/` - HTTP API
- `servers.rs` - HTTP 服务器

**移除的平台特定模块** (移到 `infiltrator-desktop`):

- `runtime.rs` - 包含 `ServiceManager` 调用
- `proxy.rs` - Windows 注册表操作
- `editor.rs` - 外部编辑器
- `version.rs` - 版本管理

**重构后的依赖**:

```toml
[package]
name = "infiltrator-core"

[dependencies]
mihomo-api = { path = "../mihomo-api" }
mihomo-config = { path = "../mihomo-config" }
mihomo-platform = { path = "../mihomo-platform" }  # 仅 traits
dav-client = { path = "../mihomo-dav-sync/dav-client" }
sync-engine = { path = "../mihomo-dav-sync/sync-engine" }
state-store = { path = "../mihomo-dav-sync/state-store" }
# ... 其他跨平台依赖
```

#### infiltrator-desktop (新建)

**职责**: Desktop 专用集成层

```rust
// crates/infiltrator-desktop/src/lib.rs
pub mod runtime;     // MihomoRuntime (使用 ProcessCoreController)
pub mod proxy;       // 系统代理集成
pub mod editor;      // 编辑器集成
pub mod version;     // 版本管理集成

pub use runtime::MihomoRuntime;
```

**依赖**:

```toml
[package]
name = "infiltrator-desktop"

[dependencies]
infiltrator-core = { path = "../infiltrator-core" }
mihomo-platform = { path = "../mihomo-platform" }
mihomo-version = { path = "../mihomo-version" }
# ... Desktop 特定依赖
```

#### infiltrator-android (新建)

**职责**: Android 专用集成层，预留与 Kotlin/Java 的桥接接口（UniFFI/JNI 待接入）

```rust
// crates/infiltrator-android/src/lib.rs
pub mod runtime;     // MihomoRuntime (使用 AndroidCoreController)
pub use runtime::{AndroidBridge, AndroidBridgeAdapter};
```

**依赖**:

```toml
[package]
name = "infiltrator-android"

[lib]
crate-type = ["cdylib", "rlib"]

[dependencies]
mihomo-platform = { path = "../mihomo-platform" }
mihomo-api = { path = "../mihomo-api" }
```

**桥接接口预留**:

```rust
#[async_trait]
pub trait AndroidBridge: Send + Sync {
    async fn core_start(&self) -> Result<()>;
    async fn core_stop(&self) -> Result<()>;
    async fn core_is_running(&self) -> Result<bool>;
    fn core_controller_url(&self) -> Option<String>;

    async fn credential_get(&self, service: &str, key: &str) -> Result<Option<String>>;
    async fn credential_set(&self, service: &str, key: &str, value: &str) -> Result<()>;
    async fn credential_delete(&self, service: &str, key: &str) -> Result<()>;

    fn data_dir(&self) -> Option<PathBuf>;
    fn cache_dir(&self) -> Option<PathBuf>;
}
```

**AndroidHost/Runtime 预留**:

```rust
pub struct AndroidHost {
    bridge: AndroidBridgeAdapter<Box<dyn AndroidBridge>>,
}

pub struct AndroidRuntime<B: AndroidBridge> {
    adapter: AndroidBridgeAdapter<B>,
}
```

**AndroidApi (NDK 对外 API 骨架)**:

```rust
pub struct AndroidApi<B: AndroidBridge> {
    adapter: AndroidBridgeAdapter<B>,
}

impl<B: AndroidBridge> AndroidApi<B> {
    pub fn new(bridge: B) -> Self;
    pub fn controller_url(&self) -> Option<String>;
    pub async fn core_start(&self) -> Result<()>;
    pub async fn core_stop(&self) -> Result<()>;
    pub async fn core_is_running(&self) -> bool;
    pub async fn credential_get(&self, service: &str, key: &str) -> Result<Option<String>>;
    pub async fn credential_set(&self, service: &str, key: &str, value: &str) -> Result<()>;
    pub async fn credential_delete(&self, service: &str, key: &str) -> Result<()>;
    pub fn data_dir(&self) -> Option<PathBuf>;
    pub fn cache_dir(&self) -> Option<PathBuf>;
}
```

**FFI 边界预留 (无 unsafe，仅定义接口与错误码)**:

```rust
pub enum FfiErrorCode {
    Ok = 0,
    InvalidState = 1,
    InvalidInput = 2,
    NotReady = 3,
    NotSupported = 4,
    Io = 5,
    Network = 6,
    Unknown = 255,
}

pub struct FfiStatus {
    pub code: FfiErrorCode,
    pub message: Option<String>,
}

pub struct FfiStringResult {
    pub status: FfiStatus,
    pub value: Option<String>,
}

pub struct FfiBoolResult {
    pub status: FfiStatus,
    pub value: bool,
}

pub trait FfiApi: Send + Sync {
    fn core_start(&self) -> FfiStatus;
    fn core_stop(&self) -> FfiStatus;
    fn core_is_running(&self) -> FfiBoolResult;
    fn controller_url(&self) -> FfiStringResult;

    fn credential_get(&self, service: &str, key: &str) -> FfiStringResult;
    fn credential_set(&self, service: &str, key: &str, value: &str) -> FfiStatus;
    fn credential_delete(&self, service: &str, key: &str) -> FfiStatus;

    fn data_dir(&self) -> FfiStringResult;
    fn cache_dir(&self) -> FfiStringResult;
}
```

### Kotlin 侧调用协议 (草案)

**线程模型**:

- Kotlin 侧异步调用放入 `Dispatchers.IO`，避免阻塞 UI 线程。
- Rust 侧所有 FFI 调用必须可重入，且对外暴露的接口不可 panic。
- Kotlin 侧负责在 App 生命周期时机调用 `core_start/core_stop`。

**错误码映射**:

```
Ok(0): 成功
InvalidState(1): 状态不一致/重复启动
InvalidInput(2): 入参非法/为空
NotReady(3): 运行时尚未初始化
NotSupported(4): 平台不支持/功能未实现
Io(5): 文件系统或存储失败
Network(6): 网络请求失败
Unknown(255): 未分类错误
```

**调用建议**:

1. `core_start()` → 成功后轮询 `core_is_running()` 获取运行状态。
2. `controller_url()` → 若为空视为 core 未就绪。
3. `credential_*` → 仅处理明文字符串，Kotlin 侧负责安全存储。
4. `data_dir/cache_dir` → Kotlin 侧首启时传入并缓存，不在 UI 线程调用。

**生命周期**:

- App 启动 → 目录注入 → 初始化 Bridge → 允许 UI 调用。
- App 退出/后台 → `core_stop()` → 释放资源。

## 明显差距整改规划 (多端/Android 分拆)

### 多端都需要 (Desktop + Android)

1. **DNS 专项管理**
    - 目标: DoH/DoT/系统 DNS 切换、fallback、DNS 代理、分流 DNS 规则
    - 归属: `infiltrator-core`(模型/校验/API) + UI 层
    - 最小里程碑: 配置模型落地 + 管理 API + 基础 UI
    - 进度: 配置模型 + 管理 API + UI 已完成

2. **Fake-IP 管理**
    - 目标: Fake-IP 范围、持久化、过滤名单、缓存清理入口
    - 归属: `infiltrator-core`(模型/校验/API) + UI 层
    - 最小里程碑: 配置落盘 + 管理 API
    - 进度: 配置模型 + 管理 API + 缓存清理入口 + UI 已完成

3. **规则集/规则提供者管理**
    - 目标: Rule Providers 管理、启停、更新、排序、状态查询
    - 归属: `infiltrator-core`(模型/调度/API) + UI 层
    - 最小里程碑: Providers 列表与更新 API
    - 进度: Providers 列表与更新 API 已完成，规则列表启停/排序已完成，UI 已完成

4. **TUN 高级配置**
    - 目标: 排除网段、DNS 劫持策略、FakeIP/TUN 联动策略
    - 归属: `infiltrator-core`(配置模型) + 平台层权限提示
    - 最小里程碑: 配置项落盘 + UI 开关
    - 进度: 配置模型与管理 API + UI 已完成

### 仅 Android 需要 (Android 优先)

1. **分应用代理/绕过**
   - 目标: 应用白名单/黑名单模式，按 UID 分流
   - 归属: `infiltrator-android` + Kotlin 层(应用列表/UID)
   - 最小里程碑: UID 列表与规则配置接口

2. **VPN Service 生命周期集成**
   - 目标: 权限申请、前台服务、保活、通知通道
   - 归属: Kotlin 层
   - 最小里程碑: Service 启停与权限流程

3. **Core 运行模式选择**
   - 目标: 外部 APK / 内嵌 so / 纯配置模式的策略切换
   - 归属: Kotlin 层 + `AndroidBridge` 实现
   - 最小里程碑: 选型决策 + Bridge 占位实现

### 向后兼容层

#### mihomo-rs (已移除)

**职责**: 迁移期提供向后兼容，现已移除

**说明**: 兼容期已结束，`src-tauri` 已切换到新 crates。

## 迁移执行计划

### Tauri 依赖清单 (移除兼容层前必须清零)

以下为 `src-tauri` 直接依赖旧兼容层的导入清单，迁移阶段必须逐项替换为新 crates：

- `src-tauri/src/admin_context.rs`: `despicable_infiltrator_core::{admin_api::AdminApiContext, AppSettings}`
- `src-tauri/src/app_state.rs`: `despicable_infiltrator_core::*`, `mihomo_rs::core::ProxyInfo`, `mihomo_rs::version::VersionManager`
- `src-tauri/src/core_update.rs`: `mihomo_rs::version::{channel::fetch_latest, Channel, DownloadProgress, VersionManager}`
- `src-tauri/src/frontend.rs`: `despicable_infiltrator_core::servers::{AdminServerHandle, StaticServerHandle}`, `despicable_infiltrator_core::servers`
- `src-tauri/src/factory_reset.rs`: `despicable_infiltrator_core::profiles`, `mihomo_rs::core::get_home_dir`
- `src-tauri/src/main.rs`: `despicable_infiltrator_core::SubscriptionScheduler`
- `src-tauri/src/settings.rs`: `despicable_infiltrator_core::{settings as core_settings, AppSettings}`
- `src-tauri/src/runtime.rs`: `despicable_infiltrator_core::MihomoRuntime`, `mihomo_rs::{config::ConfigManager, core::TrafficData, version::VersionManager}`
- `src-tauri/src/system_proxy.rs`: `despicable_infiltrator_core::{proxy as core_proxy, SystemProxyState}`
- `src-tauri/src/tray/menu.rs`: `despicable_infiltrator_core::profiles`, `mihomo_rs::core::ProxyInfo`, `mihomo_rs::version::VersionManager`
- `src-tauri/src/tray/handlers.rs`: `mihomo_rs::config::ConfigManager`, `despicable_infiltrator_core::profiles`, `despicable_infiltrator_core::scheduler::subscription::update_all_subscriptions`, `despicable_infiltrator_core::scheduler::sync::run_sync_tick`

**状态**: 已完成替换，`src-tauri` 旧兼容层导入清零。

### Tauri 导入替换映射 (可替换性检查)

已确认新 crates 中对应符号存在，迁移时需按下表替换并完成功能验证：

```
旧导入 (兼容层)                                  → 新导入 (目标 crate)
despicable_infiltrator_core::admin_api::*        → infiltrator_core::admin_api::*
despicable_infiltrator_core::servers::*          → infiltrator_core::servers::*
despicable_infiltrator_core::settings::*         → infiltrator_core::settings::*
despicable_infiltrator_core::profiles::*         → infiltrator_core::profiles::*
despicable_infiltrator_core::SubscriptionScheduler → infiltrator_core::SubscriptionScheduler
despicable_infiltrator_core::MihomoRuntime       → infiltrator_desktop::MihomoRuntime
despicable_infiltrator_core::SystemProxyState    → infiltrator_desktop::SystemProxyState
despicable_infiltrator_core::proxy::*            → infiltrator_desktop::proxy::*

mihomo_rs::config::ConfigManager                 → mihomo_config::ConfigManager
mihomo_rs::core::{ProxyInfo, TrafficData}        → mihomo_api::{ProxyInfo, TrafficData}
mihomo_rs::core::get_home_dir                    → mihomo_platform::get_home_dir
mihomo_rs::version::*                            → mihomo_version::*
```

**状态**: 符号级别可替换，行为级别验证待完成（迁移切换后按完整验证清单执行）。

### 行为级对齐清单 (Tauri 功能完整性)

以下为迁移前必须确认的行为级对齐点；目前均为“代码级对齐/待验证”，需在切换导入并完整验证后标记完成：

```
领域/能力                 代码级对齐 (已确认)    行为验证 (待完成)
运行时生命周期            ✅                    ☐ 启动/重启/退出/端口释放
控制接口与代理            ✅                    ☐ controller/traffic/代理切换/TUN
配置与订阅管理            ✅                    ☐ profiles/订阅更新/凭据存取
系统代理                 ✅                    ☐ Windows 注册表写入/状态读取
版本管理                 ✅                    ☐ 下载/切换/卸载/进度事件
Admin API/静态服务器      ✅                    ☐ 前端访问/路由/权限/错误处理
调度器 (订阅/WebDAV)      ✅                    ☐ 定时触发/手动触发/通知反馈
```

### Tauri 导入切换顺序建议 (仍不改动代码)

切换需一次性完成，但为降低风险，建议按以下顺序准备替换清单与评审：

1. **类型与设置基础**: `AppSettings`/`settings` 导入替换到 `infiltrator-core`。
2. **Admin API 与静态服务**: `AdminApiContext`、`servers` 导入替换到 `infiltrator-core`。
3. **运行时与系统代理**: `MihomoRuntime`/`SystemProxyState`/`proxy` 替换到 `infiltrator-desktop`。
4. **配置与订阅管理**: `ConfigManager` 替换到 `mihomo-config`。
5. **HTTP API 类型**: `ProxyInfo`/`TrafficData` 替换到 `mihomo-api`。
6. **版本管理**: `VersionManager`/`channel`/`DownloadProgress` 替换到 `mihomo-version`。
7. **路径工具**: `get_home_dir` 替换到 `mihomo-platform`。

**执行原则**: 完成以上替换后再统一构建/测试，且只在仓库根目录执行。

### Tauri 导入替换清单 (逐文件)

以下为实际替换清单（仅列出 import 层面的替换；逻辑不改）：

- `src-tauri/src/admin_context.rs`
  - `despicable_infiltrator_core::{admin_api::AdminApiContext, AppSettings}`
    → `infiltrator_core::{admin_api::AdminApiContext, AppSettings}`

- `src-tauri/src/app_state.rs`
  - `despicable_infiltrator_core::{... AppSettings, MihomoRuntime, SystemProxyState, ...}`
    → `infiltrator_core::{... AppSettings, ...}` + `infiltrator_desktop::{MihomoRuntime, SystemProxyState}`
  - `mihomo_rs::core::ProxyInfo`
    → `mihomo_api::ProxyInfo`
  - `mihomo_rs::version::VersionManager`
    → `mihomo_version::VersionManager`

- `src-tauri/src/core_update.rs`
  - `mihomo_rs::version::{channel::fetch_latest, Channel, DownloadProgress, VersionManager}`
    → `mihomo_version::{channel::fetch_latest, Channel, DownloadProgress, VersionManager}`

- `src-tauri/src/frontend.rs`
  - `despicable_infiltrator_core::servers::{AdminServerHandle, StaticServerHandle}`
    → `infiltrator_core::servers::{AdminServerHandle, StaticServerHandle}`
  - `despicable_infiltrator_core::servers as core_servers`
    → `infiltrator_core::servers as core_servers`

- `src-tauri/src/factory_reset.rs`
  - `despicable_infiltrator_core::profiles`
    → `infiltrator_core::profiles`
  - `mihomo_rs::core::get_home_dir`
    → `mihomo_platform::get_home_dir`

- `src-tauri/src/main.rs`
  - `despicable_infiltrator_core::SubscriptionScheduler`
    → `infiltrator_core::SubscriptionScheduler`

- `src-tauri/src/settings.rs`
  - `despicable_infiltrator_core::{settings as core_settings, AppSettings}`
    → `infiltrator_core::{settings as core_settings, AppSettings}`

- `src-tauri/src/runtime.rs`
  - `despicable_infiltrator_core::MihomoRuntime`
    → `infiltrator_desktop::MihomoRuntime`
  - `mihomo_rs::{config::ConfigManager, core::TrafficData, version::VersionManager}`
    → `mihomo_config::ConfigManager`, `mihomo_api::TrafficData`, `mihomo_version::VersionManager`

- `src-tauri/src/system_proxy.rs`
  - `despicable_infiltrator_core::{proxy as core_proxy, SystemProxyState}`
    → `infiltrator_desktop::{proxy as core_proxy, SystemProxyState}`

- `src-tauri/src/tray/menu.rs`
  - `despicable_infiltrator_core::profiles as core_profiles`
    → `infiltrator_core::profiles as core_profiles`
  - `mihomo_rs::core::ProxyInfo`
    → `mihomo_api::ProxyInfo`
  - `mihomo_rs::version::VersionManager`
    → `mihomo_version::VersionManager`

- `src-tauri/src/tray/handlers.rs`
  - `mihomo_rs::config::ConfigManager`
    → `mihomo_config::ConfigManager`
  - `despicable_infiltrator_core::profiles as core_profiles`
    → `infiltrator_core::profiles as core_profiles`
  - `despicable_infiltrator_core::scheduler::subscription::update_all_subscriptions`
    → `infiltrator_core::scheduler::subscription::update_all_subscriptions`
  - `despicable_infiltrator_core::scheduler::sync::run_sync_tick`
    → `infiltrator_core::scheduler::sync::run_sync_tick`

### Stage 1: 提取 mihomo-api (1-2 天)

```bash
# 1. 创建新 crate
mkdir -p crates/mihomo-api/src

# 2. 复制文件
cp crates/mihomo-rs/src/core/client.rs crates/mihomo-api/src/
cp crates/mihomo-rs/src/core/types.rs crates/mihomo-api/src/
cp crates/mihomo-rs/src/core/error.rs crates/mihomo-api/src/

# 3. 创建 lib.rs 和 Cargo.toml

# 4. 验证
cargo build -p mihomo-api
cargo test -p mihomo-api
```

**验证点**: `cargo build --workspace` 通过

### Stage 2: 提取 mihomo-platform (2-3 天)

```bash
# 1. 创建 crate 结构
mkdir -p crates/mihomo-platform/src/desktop
mkdir -p crates/mihomo-platform/src/android

# 2. 定义 traits (新文件)
# crates/mihomo-platform/src/traits.rs

# 3. 实现 Desktop 版本
# 从 mihomo-rs/service/process.rs 提取 ProcessCoreController
# 从 mihomo-rs/config/manager.rs 提取 keyring 调用

# 4. 验证
cargo build -p mihomo-platform
cargo test -p mihomo-platform
```

**验证点**: Desktop 应用正常运行

### Stage 3: 重构 mihomo-config (1-2 天)

```bash
# 1. 创建 crate
mkdir -p crates/mihomo-config/src

# 2. 将 ConfigManager 改为泛型
# ConfigManager<S: CredentialStore>

# 3. 验证
cargo build --workspace
cargo test --workspace
```

**验证点**: 订阅 URL 存储/读取正常

### Stage 4: 创建 infiltrator-core (2-3 天)

```bash
# 1. 重命名 despicable-infiltrator-core -> infiltrator-core
# 2. 移除平台特定代码到 infiltrator-desktop
# 3. 更新依赖

cargo build --workspace
```

**验证点**: `src-tauri` 编译通过，应用正常运行

### Stage 5: 创建 infiltrator-desktop (1-2 天)

```bash
# 1. 创建 crate
mkdir -p crates/infiltrator-desktop/src

# 2. 移入 runtime.rs, proxy.rs, editor.rs, version.rs

# 3. 更新 src-tauri 依赖
# Cargo.toml: infiltrator-desktop = { path = "../crates/infiltrator-desktop" }

# 4. 验证
cargo build -p "MusicFrog-Despicable-Infiltrator"
```

**验证点**: 完整功能测试通过

### Stage 6: 更新 mihomo-rs 为 re-export (1 天)

```bash
# 1. 清空 mihomo-rs/src/
# 2. 创建 re-export lib.rs
# 3. 版本号升级到 2.0.0

# 4. 验证
cargo build --workspace
```

**验证点**: 无外部 API 变更，所有调用点正常

### Stage 7: 移除兼容层 (已完成)

**前置条件**:

- [x] `src-tauri` 已完全切换到新 crates，不再依赖 `mihomo-rs` 与 `despicable-infiltrator-core`。
- [ ] 新 crates 的功能与原 crates 行为对齐，关键流程功能可用。
- [ ] Tauri 应用完整功能验证通过（详见下方清单）。

**执行步骤**:

1. 从 workspace 中移除 `crates/mihomo-rs` 与 `crates/despicable-infiltrator-core`。
2. 更新 `Cargo.toml` workspace members 与依赖树。
3. 更新文档（`README.md`/`USAGE_SPEC.md`/`CHANGELOG.md`）。

**验证点**: Tauri 完整功能测试通过，无回退需求。

### 完整验证清单

每个 Stage 完成后执行:

- [ ] `cargo build --workspace`
- [ ] `cargo test --workspace`
- [ ] `cargo build -p "MusicFrog-Despicable-Infiltrator"` (Windows)
- [ ] 手动测试:
  - [ ] 应用启动，内核正常运行
  - [ ] 系统代理切换正常
  - [ ] 配置文件切换正常
  - [ ] 订阅更新正常
  - [ ] WebDAV 同步正常
  - [ ] 托盘菜单功能正常

## 架构分层概览

```
┌─────────────────────────────────────────────────────────────────────┐
│                        应用层 (Platform Specific)                   │
├─────────────────────────────────────────────────────────────────────┤
│  src-tauri/          │  android/app/          │  (future)          │
│  Tauri v2 Desktop    │  Kotlin + Compose      │  iOS/Flutter/...   │
│  Windows/macOS/Linux │  Android               │                    │
└──────────┬───────────┴──────────┬─────────────┴────────────────────┘
           │                      │
           ▼                      ▼
┌─────────────────────────────────────────────────────────────────────┐
│                     平台桥接层 (Platform Bridges)                   │
├─────────────────────────────────────────────────────────────────────┤
│  TauriAdminContext   │  AndroidPlatformBridge │  trait 实现        │
│  (Tauri commands)    │  (JNI/UniFFI)          │                    │
└──────────┬───────────┴──────────┬─────────────┴────────────────────┘
           │                      │
           ▼                      ▼
┌─────────────────────────────────────────────────────────────────────┐
│                      核心业务层 (Platform Agnostic)                 │
├─────────────────────────────────────────────────────────────────────┤
│  despicable-infiltrator-core                                        │
│    - config.rs, profiles.rs, settings.rs, subscription.rs          │
│    - scheduler/sync.rs (WebDAV 调度)                                │
│    - admin_api/ (HTTP API handlers)                                 │
│    - servers.rs (Axum HTTP servers)                                 │
│                                                                     │
│  mihomo-dav-sync (完全跨平台)                                       │
│    - dav-client, state-store, indexer, sync-engine                 │
└──────────┬──────────────────────────────────────────────────────────┘
           │
           ▼
┌─────────────────────────────────────────────────────────────────────┐
│                      平台抽象层 (Platform Traits)                   │
├─────────────────────────────────────────────────────────────────────┤
│  mihomo-rs (需重构)                                                 │
│    - trait CoreController { start, stop, is_running, ... }         │
│    - trait CredentialStore { get, set, delete }                    │
│    - trait SystemProxyManager { enable, disable, read_state }      │
│    - trait ExternalEditorLauncher { open_file }                    │
│                                                                     │
│  平台实现:                                                          │
│    - desktop/ (ProcessController, KeyringCredentialStore, ...)     │
│    - android/ (IntentCoreController, EncryptedPrefsStore, ...)     │
└─────────────────────────────────────────────────────────────────────┘
```

## 模块兼容性矩阵

| 模块 | Android 兼容 | 阻塞因素 | 工作量 | 重构风险 |
|------|-------------|----------|--------|----------|
| `dav-client` | ✅ 完全兼容 | 无 | 0 | 无 |
| `state-store` | ✅ 完全兼容 | 无 | 0 | 无 |
| `indexer` | ✅ 完全兼容 | 无 | 0 | 无 |
| `sync-engine` | ✅ 完全兼容 | 无 | 0 | 无 |
| `despicable-infiltrator-core` | ⚠️ 部分兼容 | 见下文 | 中 | 低 |
| `mihomo-rs` | ❌ 需要重构 | 进程管理、keyring | 高 | 中 |
| `src-tauri` | ❌ 不适用 | Tauri 特定 | N/A | 无 |

## 详细分析

### 1. mihomo-dav-sync (✅ Android Ready)

四个子模块均为纯 Rust，无平台特定代码：

```
dav-client/     # HTTP/WebDAV 客户端 - reqwest 支持 Android
state-store/    # SQLite 状态存储 - sqlx 支持 Android
indexer/        # 文件扫描器 - std::fs 跨平台
sync-engine/    # 同步算法 - 纯逻辑
```

`platform-android` crate 已预留，可直接暴露 JNI/UniFFI 接口。

**无需任何修改即可用于 Android。**

### 2. despicable-infiltrator-core (⚠️ 需适配)

**平台特定代码清单：**

| 文件 | 行号 | 当前实现 | Android 适配方案 | Desktop 影响 |
|------|------|----------|------------------|--------------|
| `proxy.rs` | 2-111 | Windows 注册表操作 | 已有 `#[cfg]` 隔离，Android 不适用 | ✅ 无影响 |
| `editor.rs` | 8-335 | 调用外部编辑器 | 需抽象为 trait，Android 用 Intent | ⚠️ 需重构 |
| `version.rs` | 57-89 | 复制 bundled binary | 已有 `#[cfg(windows)]`，Android 不需要 | ✅ 无影响 |
| `runtime.rs` | 50 | 调用 `ServiceManager` | 依赖 mihomo-rs 抽象 | ⚠️ 间接影响 |
| `Cargo.toml` | 26-28 | `winreg`, `windows-sys` | 已用 `cfg(windows)` 隔离 | ✅ 无影响 |

**可直接复用的模块（无需修改）：**

- `config.rs` - 配置文件读写 (纯 YAML 操作)
- `profiles.rs` - 配置文件管理 (纯文件系统)
- `settings.rs` - 应用设置 (TOML 序列化)
- `subscription.rs` - 订阅更新 (HTTP + 文件操作)
- `scheduler/sync.rs` - WebDAV 同步调度 (依赖 sync-engine)
- `admin_api/` - HTTP API handlers (纯 Axum handlers)
- `servers.rs` - HTTP 服务器 (纯 Axum servers)

### 3. mihomo-rs (❌ 需要重构)

**当前硬编码的平台依赖：**

#### 3.1 进程管理 (`service/process.rs` 全文, `service/manager.rs` 全文)

```rust
// service/process.rs:12-67 - 当前实现
pub async fn spawn_daemon(binary: &Path, config: &Path) -> Result<u32> {
    let mut command = Command::new(binary);
    #[cfg(windows)]
    command.creation_flags(CREATE_NO_WINDOW);
    let mut child = command.spawn()?;
    // ...
}

pub fn kill_process(pid: u32) -> Result<()> {
    let mut system = System::new();
    system.process(Pid::from_u32(pid))?.kill();
}
```

**Android 问题：**

- Android 不允许应用直接执行任意二进制文件
- VPN 需通过 `VpnService` API 实现
- mihomo 需作为独立 APK 或嵌入式库运行

#### 3.2 Keyring 凭据存储 (`config/manager.rs:480-514`)

```rust
// 当前实现 - 使用 keyring crate
let entry = keyring::Entry::new(SUBSCRIPTION_SERVICE, &key)?;
entry.set_password(url)?;
```

**Cargo.toml 平台依赖 (lines 37-45)：**

```toml
[target.'cfg(windows)'.dependencies]
keyring = { version = "3.6", features = ["windows-native"] }

[target.'cfg(target_os = "macos")'.dependencies]
keyring = { version = "3.6", features = ["apple-native"] }

[target.'cfg(target_os = "linux")'.dependencies]
keyring = { version = "3.6", features = ["linux-native"] }
```

**Android 问题：**

- `keyring` crate 无 Android 支持
- Android 使用 `EncryptedSharedPreferences` 或 Android Keystore

#### 3.3 其他平台依赖

| 文件 | 依赖 | 用途 |
|------|------|------|
| `version/download.rs` | 无直接依赖 | 但下载的是 OS 特定二进制 |
| `Cargo.toml:38` | `windows-sys` | Windows 进程创建标志 |
| 各处 | `sysinfo` | 进程状态检查 (支持 Android) |

### 4. src-tauri (❌ 不移植，保持原样)

Tauri 层是 Desktop 专用的，Android 版将使用独立的 Kotlin 前端。

**关键耦合点 (需确保重构不影响)：**

| 文件 | 依赖 | 说明 |
|------|------|------|
| `runtime.rs:161` | `MihomoRuntime::bootstrap()` | 启动内核 |
| `runtime.rs:54` | `ServiceManager::start/stop()` | 控制进程 |
| `app_state.rs:88` | `Arc<MihomoRuntime>` | 运行时状态 |
| `system_proxy.rs` | `apply_system_proxy()` | 系统代理 |
| `admin_context.rs` | `AdminApiContext` trait | API 上下文 |

## 平台抽象设计

### Trait 定义

在 `mihomo-rs` 中添加 `src/platform/mod.rs`：

```rust
// crates/mihomo-rs/src/platform/mod.rs

use async_trait::async_trait;
use crate::core::Result;

/// 控制 mihomo 核心进程的生命周期
#[async_trait]
pub trait CoreController: Send + Sync {
    /// 启动 mihomo 核心
    async fn start(&self) -> Result<()>;
    
    /// 停止 mihomo 核心
    async fn stop(&self) -> Result<()>;
    
    /// 重启 mihomo 核心
    async fn restart(&self) -> Result<()> {
        self.stop().await.ok(); // 忽略停止错误
        self.start().await
    }
    
    /// 检查核心是否正在运行
    async fn is_running(&self) -> bool;
    
    /// 获取控制器 API 地址 (如 http://127.0.0.1:9090)
    fn controller_url(&self) -> Option<String>;
}

/// 安全存储凭据 (订阅 URL 等敏感信息)
#[async_trait]
pub trait CredentialStore: Send + Sync {
    /// 获取凭据
    async fn get(&self, service: &str, key: &str) -> Result<Option<String>>;
    
    /// 存储凭据
    async fn set(&self, service: &str, key: &str, value: &str) -> Result<()>;
    
    /// 删除凭据
    async fn delete(&self, service: &str, key: &str) -> Result<()>;
}

/// 系统代理管理 (仅 Desktop 平台需要)
#[async_trait]
pub trait SystemProxyManager: Send + Sync {
    /// 启用系统代理
    fn enable(&self, endpoint: &str) -> Result<()>;
    
    /// 禁用系统代理
    fn disable(&self) -> Result<()>;
    
    /// 读取当前系统代理状态
    fn read_state(&self) -> Result<(bool, Option<String>)>;
}

/// 打开外部编辑器 (仅 Desktop 平台需要)
pub trait ExternalEditorLauncher: Send + Sync {
    /// 用指定编辑器打开文件
    fn open_file(&self, file_path: &str, editor_path: Option<&str>) -> Result<()>;
}

/// 获取应用数据目录
pub trait DataDirProvider: Send + Sync {
    /// 返回应用数据目录路径
    fn data_dir(&self) -> std::path::PathBuf;
    
    /// 返回配置文件目录
    fn config_dir(&self) -> std::path::PathBuf;
    
    /// 返回日志目录
    fn log_dir(&self) -> std::path::PathBuf;
}
```

### Desktop 实现 (保持现有逻辑)

```rust
// crates/mihomo-rs/src/platform/desktop.rs

use super::*;
use crate::service::process;

/// Desktop 平台的进程控制器 (Windows/macOS/Linux)
pub struct ProcessCoreController {
    binary_path: PathBuf,
    config_path: PathBuf,
    pid_file: PathBuf,
}

#[async_trait]
impl CoreController for ProcessCoreController {
    async fn start(&self) -> Result<()> {
        // 现有 spawn_daemon 逻辑
        let pid = process::spawn_daemon(&self.binary_path, &self.config_path).await?;
        process::write_pid_file(&self.pid_file, pid).await?;
        Ok(())
    }
    
    async fn stop(&self) -> Result<()> {
        let pid = process::read_pid_file(&self.pid_file).await?;
        process::kill_process(pid)?;
        process::remove_pid_file(&self.pid_file).await?;
        Ok(())
    }
    
    async fn is_running(&self) -> bool {
        if let Ok(pid) = process::read_pid_file(&self.pid_file).await {
            process::is_process_alive(pid)
        } else {
            false
        }
    }
    
    fn controller_url(&self) -> Option<String> {
        // 从配置读取
        None
    }
}

/// Desktop 平台的凭据存储 (使用 keyring crate)
pub struct KeyringCredentialStore;

#[async_trait]
impl CredentialStore for KeyringCredentialStore {
    async fn get(&self, service: &str, key: &str) -> Result<Option<String>> {
        // 现有 keyring 逻辑
        match keyring::Entry::new(service, key) {
            Ok(entry) => entry.get_password().ok().map(Some).unwrap_or(None),
            Err(_) => Ok(None),
        }
    }
    
    async fn set(&self, service: &str, key: &str, value: &str) -> Result<()> {
        let entry = keyring::Entry::new(service, key)?;
        entry.set_password(value)?;
        Ok(())
    }
    
    async fn delete(&self, service: &str, key: &str) -> Result<()> {
        if let Ok(entry) = keyring::Entry::new(service, key) {
            entry.delete_credential().ok();
        }
        Ok(())
    }
}
```

### Android 实现 (通过 JNI/UniFFI)

```rust
// crates/mihomo-rs/src/platform/android.rs

use super::*;

/// Android 平台的核心控制器 (通过 Intent/AIDL 与 mihomo APK 通信)
pub struct AndroidCoreController {
    // JNI 回调或 AIDL 连接
}

#[async_trait]
impl CoreController for AndroidCoreController {
    async fn start(&self) -> Result<()> {
        // 发送 Intent 启动 mihomo VPN Service
        // 或通过 AIDL 调用
        todo!("通过 JNI 调用 Kotlin 层")
    }
    
    async fn stop(&self) -> Result<()> {
        todo!("通过 JNI 调用 Kotlin 层")
    }
    
    async fn is_running(&self) -> bool {
        todo!("通过 JNI 查询状态")
    }
    
    fn controller_url(&self) -> Option<String> {
        // Android 上可能是 localhost 或 Unix socket
        Some("http://127.0.0.1:9090".to_string())
    }
}

/// Android 平台的凭据存储 (使用 EncryptedSharedPreferences)
pub struct AndroidCredentialStore {
    // JNI 环境引用
}

#[async_trait]
impl CredentialStore for AndroidCredentialStore {
    async fn get(&self, service: &str, key: &str) -> Result<Option<String>> {
        // 通过 JNI 调用 EncryptedSharedPreferences.getString()
        todo!()
    }
    
    async fn set(&self, service: &str, key: &str, value: &str) -> Result<()> {
        // 通过 JNI 调用 EncryptedSharedPreferences.putString()
        todo!()
    }
    
    async fn delete(&self, service: &str, key: &str) -> Result<()> {
        // 通过 JNI 调用 EncryptedSharedPreferences.remove()
        todo!()
    }
}
```

### 条件编译配置

```rust
// crates/mihomo-rs/src/platform/mod.rs

#[cfg(not(target_os = "android"))]
pub mod desktop;
#[cfg(not(target_os = "android"))]
pub use desktop::*;

#[cfg(target_os = "android")]
pub mod android;
#[cfg(target_os = "android")]
pub use android::*;

// 通用 trait 定义
mod traits;
pub use traits::*;
```

## ServiceManager 重构方案

### 当前 ServiceManager (需保留接口兼容)

```rust
// crates/mihomo-rs/src/service/manager.rs (当前)
pub struct ServiceManager {
    binary_path: PathBuf,
    config_path: PathBuf,
    pid_file: PathBuf,
}

impl ServiceManager {
    pub fn new(binary_path: PathBuf, config_path: PathBuf) -> Self { ... }
    pub async fn start(&self) -> Result<()> { ... }
    pub async fn stop(&self) -> Result<()> { ... }
    pub async fn restart(&self) -> Result<()> { ... }
    pub async fn status(&self) -> Result<ServiceStatus> { ... }
    pub async fn is_running(&self) -> bool { ... }
}
```

### 重构后 ServiceManager (泛型 + 默认实现)

```rust
// crates/mihomo-rs/src/service/manager.rs (重构后)
use crate::platform::CoreController;

/// 泛型 ServiceManager，接受任意 CoreController 实现
pub struct ServiceManager<C: CoreController> {
    controller: C,
}

impl<C: CoreController> ServiceManager<C> {
    pub fn new(controller: C) -> Self {
        Self { controller }
    }
    
    pub async fn start(&self) -> Result<()> {
        self.controller.start().await
    }
    
    pub async fn stop(&self) -> Result<()> {
        self.controller.stop().await
    }
    
    pub async fn restart(&self) -> Result<()> {
        self.controller.restart().await
    }
    
    pub async fn is_running(&self) -> bool {
        self.controller.is_running().await
    }
    
    pub async fn status(&self) -> Result<ServiceStatus> {
        if self.controller.is_running().await {
            Ok(ServiceStatus::Running(0)) // PID 可能不适用于所有平台
        } else {
            Ok(ServiceStatus::Stopped)
        }
    }
}

// 为 Desktop 保持向后兼容的类型别名
#[cfg(not(target_os = "android"))]
pub type DefaultServiceManager = ServiceManager<crate::platform::ProcessCoreController>;

#[cfg(not(target_os = "android"))]
impl DefaultServiceManager {
    /// 保持原有构造函数签名的兼容性
    pub fn from_paths(binary_path: PathBuf, config_path: PathBuf) -> Self {
        let controller = crate::platform::ProcessCoreController::new(binary_path, config_path);
        Self::new(controller)
    }
}
```

### 迁移策略 (保持 Desktop 兼容)

**Step 1**: 添加 trait 和 Desktop 实现，不修改现有代码

```
crates/mihomo-rs/src/
├── platform/
│   ├── mod.rs          # 新增: trait 定义 + 条件导出
│   ├── traits.rs       # 新增: CoreController, CredentialStore
│   └── desktop.rs      # 新增: ProcessCoreController, KeyringCredentialStore
├── service/
│   ├── mod.rs          # 不变
│   ├── process.rs      # 不变 (被 desktop.rs 调用)
│   └── manager.rs      # 不变 (暂时)
```

**Step 2**: 为 ServiceManager 添加泛型版本，保留原有版本

```rust
// 原有 ServiceManager 重命名为 LegacyServiceManager
pub type LegacyServiceManager = ServiceManager;

// 新增泛型版本
pub struct GenericServiceManager<C: CoreController> { ... }
```

**Step 3**: 验证所有 Desktop 调用点正常工作

```rust
// src-tauri/src/runtime.rs:50 - 无需修改
let service_manager = ServiceManager::new(binary, config_path.clone());
// 等价于 ServiceManager<ProcessCoreController>::from_paths(...)
```

**Step 4**: 移除 Legacy 版本，统一使用泛型版本

## ConfigManager 中的 Keyring 重构

### 当前实现 (硬编码 keyring)

```rust
// crates/mihomo-rs/src/config/manager.rs:480-514
fn store_subscription_url(key: &str, url: &str) -> Result<()> {
    let entry = keyring::Entry::new(SUBSCRIPTION_SERVICE, &key)?;
    entry.set_password(url)?;
    Ok(())
}

fn load_subscription_url(key: &str) -> Result<Option<String>> {
    let entry = keyring::Entry::new(SUBSCRIPTION_SERVICE, &key)?;
    // ...
}
```

### 重构后 (依赖注入)

```rust
// crates/mihomo-rs/src/config/manager.rs

use crate::platform::CredentialStore;

pub struct ConfigManager<S: CredentialStore> {
    home_dir: PathBuf,
    credential_store: S,
}

impl<S: CredentialStore> ConfigManager<S> {
    pub fn new_with_store(credential_store: S) -> Result<Self> {
        let home_dir = get_home_dir()?;
        Ok(Self { home_dir, credential_store })
    }
    
    async fn store_subscription_url(&self, key: &str, url: &str) -> Result<()> {
        self.credential_store.set(SUBSCRIPTION_SERVICE, key, url).await
    }
    
    async fn load_subscription_url(&self, key: &str) -> Result<Option<String>> {
        self.credential_store.get(SUBSCRIPTION_SERVICE, key).await
    }
}

// Desktop 便捷构造函数
#[cfg(not(target_os = "android"))]
impl ConfigManager<crate::platform::KeyringCredentialStore> {
    pub fn new() -> Result<Self> {
        Self::new_with_store(crate::platform::KeyringCredentialStore)
    }
}
```

## Tauri 应用影响评估

### 调用链分析

```
src-tauri/src/runtime.rs
    └── MihomoRuntime::bootstrap() [despicable-infiltrator-core]
            └── ServiceManager::new() [mihomo-rs]
                    └── spawn_daemon() [mihomo-rs/service/process.rs]
```

### 受影响的 Tauri 文件

| 文件 | 调用点 | 影响分析 |
|------|--------|----------|
| `runtime.rs:150-207` | `bootstrap_runtime()` | 调用 `MihomoRuntime::bootstrap()` |
| `runtime.rs:81-148` | `rebuild_runtime()` | 调用 `runtime.shutdown()` |
| `app_state.rs:112-121` | `stop_runtime()` | 调用 `runtime.shutdown()` |
| `system_proxy.rs` | `apply_system_proxy()` | 直接调用 core 的 proxy 模块 |
| `admin_context.rs` | `TauriAdminContext` | 实现 `AdminApiContext` trait |

### 兼容性保证策略

1. **保持公开 API 签名不变**

   ```rust
   // MihomoRuntime::bootstrap 签名不变
   pub async fn bootstrap(
       vm: &VersionManager,
       use_bundled: bool,
       bundled_candidates: &[PathBuf],
       data_dir: &Path,
   ) -> anyhow::Result<Self>
   
   // ServiceManager::new 签名不变 (Desktop)
   pub fn new(binary_path: PathBuf, config_path: PathBuf) -> Self
   ```

2. **使用类型别名保持兼容**

   ```rust
   // 对于 Desktop 平台，ServiceManager 是具体类型的别名
   #[cfg(not(target_os = "android"))]
   pub type ServiceManager = GenericServiceManager<ProcessCoreController>;
   ```

3. **内部重构，外部无感**
   - Tauri 代码无需修改
   - 所有变更在 `mihomo-rs` 内部完成
   - 通过 feature flags 控制 Android 代码编译

### 验证清单

在每个重构阶段后必须验证：

- [ ] `cargo build -p "MusicFrog-Despicable-Infiltrator"` (Windows 构建)
- [ ] `cargo build -p "MusicFrog-Despicable-Infiltrator" --target x86_64-apple-darwin` (macOS 构建)
- [ ] `cargo build -p "MusicFrog-Despicable-Infiltrator" --target x86_64-unknown-linux-gnu` (Linux 构建)
- [ ] `cargo test --workspace` (全部测试通过)
- [ ] 手动测试：启动应用 → 内核启动 → 系统代理切换 → 退出应用

## 移植路线图

### Phase 1: 平台抽象层 (2-3 周)

**目标**: 在 `mihomo-rs` 中添加 trait 抽象，保持 Desktop 功能完整

**Week 1: Trait 定义**

```
任务:
1. 创建 crates/mihomo-rs/src/platform/mod.rs
2. 定义 CoreController, CredentialStore, DataDirProvider traits
3. 创建 crates/mihomo-rs/src/platform/desktop.rs
4. 实现 ProcessCoreController (封装现有 process.rs 逻辑)
5. 实现 KeyringCredentialStore (封装现有 keyring 逻辑)

验证:
- cargo build --workspace
- cargo test --workspace
- 现有 Tauri 应用正常运行
```

**Week 2: ServiceManager 重构**

```
任务:
1. 创建 GenericServiceManager<C: CoreController>
2. 为 Desktop 创建类型别名 ServiceManager = GenericServiceManager<ProcessCoreController>
3. 确保所有现有调用点无需修改
4. 添加单元测试

验证:
- Tauri 应用启动/停止内核正常
- 系统代理切换正常
```

**Week 3: ConfigManager 重构**

```
任务:
1. 将 ConfigManager 改为泛型 ConfigManager<S: CredentialStore>
2. 为 Desktop 保持 ConfigManager::new() 的默认实现
3. 迁移所有 keyring 调用到 CredentialStore trait
4. 添加单元测试 (使用 mock CredentialStore)

验证:
- 订阅 URL 存储/读取正常
- 删除配置文件时凭据清理正常
```

### Phase 2: Android 基础设施 (2-3 周)

**Week 4: Android 工具链**

```
任务:
1. 安装 Android NDK, cargo-ndk
2. 添加 Android target 到 rust-toolchain.toml
3. 创建 .cargo/config.toml 配置 Android linker
4. 验证 mihomo-dav-sync 可编译为 Android 库

命令:
rustup target add aarch64-linux-android armv7-linux-androideabi x86_64-linux-android
cargo ndk -t arm64-v8a build -p sync-engine
```

**Week 5: UniFFI 集成**

```
任务:
1. 添加 uniffi 依赖到 platform-android
2. 创建 platform-android/src/lib.udl (接口定义)
3. 生成 Kotlin 绑定
4. 测试 JNI 调用

文件:
crates/mihomo-dav-sync/platform-android/
├── Cargo.toml          # 添加 uniffi 依赖
├── src/
│   ├── lib.rs          # uniffi 宏生成代码
│   └── lib.udl         # 接口定义语言
└── uniffi.toml         # UniFFI 配置
```

**Week 6: Android CoreController**

```
任务:
1. 创建 crates/mihomo-rs/src/platform/android.rs
2. 实现 AndroidCoreController (JNI 回调框架)
3. 实现 AndroidCredentialStore (JNI 调用 EncryptedSharedPreferences)
4. 创建 Kotlin 端的 PlatformBridge 类

Kotlin 端:
android/app/src/main/kotlin/
└── com/example/mihomo/
    ├── PlatformBridge.kt      # Rust 回调实现
    ├── MihomoVpnService.kt    # VPN Service 包装
    └── CredentialManager.kt   # EncryptedSharedPreferences 封装
```

### Phase 3: Android 应用 (4-6 周)

**Week 7-8: 项目骨架**

```
任务:
1. 创建 android/ 目录结构
2. 配置 Gradle 构建脚本
3. 集成 Rust 编译到 Gradle
4. 实现最小可运行 App

结构:
android/
├── app/
│   ├── build.gradle.kts
│   ├── src/main/
│   │   ├── kotlin/com/example/mihomo/
│   │   ├── jniLibs/         # cargo-ndk 输出
│   │   └── AndroidManifest.xml
│   └── proguard-rules.pro
├── build.gradle.kts
├── settings.gradle.kts
└── gradle.properties
```

**Week 9-10: VPN 集成**

```
任务:
1. 实现 MihomoVpnService
2. 集成 mihomo Android 库或 APK
3. 实现 AIDL/Intent 通信
4. 测试 VPN 连接

关键 API:
- VpnService.Builder
- ParcelFileDescriptor
- TUN 设备配置
```

**Week 11-12: UI 实现**

```
任务:
1. 使用 Jetpack Compose 实现主界面
2. 配置文件列表/切换
3. WebDAV 同步设置
4. 订阅管理

复用逻辑:
- profiles.rs 的所有逻辑
- scheduler/sync.rs 的 WebDAV 同步
- subscription.rs 的订阅更新
```

## 依赖兼容性检查

### 核心依赖 Android 支持状态

| Crate | 版本 | Android 支持 | 备注 |
|-------|------|--------------|------|
| `tokio` | 1.x | ✅ 完全支持 | 避免 `io-uring` feature |
| `reqwest` | 0.12 | ✅ 需配置 | 使用 `rustls-tls` 而非 `native-tls` |
| `sqlx` | 0.8 | ✅ 完全支持 | SQLite 内置于 Android |
| `serde` | 1.x | ✅ 完全支持 | 无平台依赖 |
| `axum` | 0.8 | ✅ 完全支持 | HTTP 服务器，Android 可用 |
| `chrono` | 0.4 | ✅ 完全支持 | 时间处理 |
| `quick-xml` | 0.37 | ✅ 完全支持 | XML 解析 |
| `md5` | 0.7 | ✅ 完全支持 | 哈希计算 |
| `sysinfo` | 0.33 | ✅ 部分支持 | 进程信息在 Android 受限 |
| `keyring` | 3.6 | ❌ 不支持 | 需用 Android Keystore 替代 |
| `winreg` | 0.55 | ❌ Windows only | 已用 `cfg(windows)` 隔离 |
| `windows-sys` | 0.61 | ❌ Windows only | 已用 `cfg(windows)` 隔离 |

### Cargo.toml 配置建议

```toml
# 根 Cargo.toml - 添加 Android 条件依赖
[workspace.dependencies]
reqwest = { version = "0.12", default-features = false, features = [
    "json", "rustls-tls", "gzip", "brotli"
] }

# mihomo-rs/Cargo.toml - 平台隔离
[target.'cfg(target_os = "android")'.dependencies]
# Android 特定依赖 (JNI, 等)
jni = "0.21"

[target.'cfg(not(target_os = "android"))'.dependencies]
# Desktop 特定依赖
keyring = { version = "3.6", features = ["..."] }
```

## 文件路径适配

### 问题

Android 使用不同的文件系统路径约定：

```rust
// Desktop - 使用 dirs crate
fn get_home_dir() -> PathBuf {
    dirs::home_dir().unwrap().join(".config/mihomo")
}

// Android - 需从 Context 获取
// context.getFilesDir() -> /data/data/com.example.app/files
// context.getExternalFilesDir(null) -> /storage/emulated/0/Android/data/com.example.app/files
```

### 解决方案

状态更新: 已在 mihomo-rs 中提供 `apply_data_dir_override` 针对 `DataDirProvider` 的目录注入钩子。

通过 `DataDirProvider` trait 抽象路径获取：

```rust
// Kotlin 端初始化时传入路径
class App : Application() {
    override fun onCreate() {
        val dataDir = filesDir.absolutePath
        val cacheDir = cacheDir.absolutePath
        RustBridge.initialize(dataDir, cacheDir)
    }
}

// Rust 端接收并存储
static DATA_DIR: OnceLock<PathBuf> = OnceLock::new();

#[cfg(target_os = "android")]
pub fn initialize(data_dir: String, cache_dir: String) {
    DATA_DIR.set(PathBuf::from(data_dir)).ok();
}

#[cfg(target_os = "android")]
impl DataDirProvider for AndroidDataDirProvider {
    fn data_dir(&self) -> PathBuf {
        DATA_DIR.get().cloned().unwrap_or_default()
    }
}
```

## 测试策略

### 单元测试 (Host 平台)

现有 24 个测试在 host 平台运行，不受影响：

```bash
cargo test --workspace
```

### 集成测试 (Mock Trait)

为平台抽象添加 mock 实现：

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    struct MockCoreController {
        running: AtomicBool,
    }
    
    #[async_trait]
    impl CoreController for MockCoreController {
        async fn start(&self) -> Result<()> {
            self.running.store(true, Ordering::SeqCst);
            Ok(())
        }
        // ...
    }
    
    #[tokio::test]
    async fn test_service_manager_lifecycle() {
        let mock = MockCoreController::new();
        let sm = ServiceManager::new(mock);
        assert!(!sm.is_running().await);
        sm.start().await.unwrap();
        assert!(sm.is_running().await);
    }
}
```

### Android 集成测试

使用 Android Emulator 运行：

```bash
# 编译 Android 库
cargo ndk -t arm64-v8a build -p platform-android --release

# 运行 instrumented test (需要 Android Studio)
./gradlew connectedAndroidTest
```

## 风险评估

| 风险 | 可能性 | 影响 | 缓解措施 |
|------|--------|------|----------|
| Desktop 功能回归 | 中 | 高 | 每次重构后运行完整测试套件 |
| mihomo Android 兼容性 | 高 | 高 | 先验证 AIDL 接口，考虑备选方案 (嵌入式编译) |
| JNI 内存泄漏 | 中 | 中 | 使用 jni crate 的 safe wrapper |
| Tokio 在 Android 的性能 | 低 | 中 | 性能测试，必要时使用 current_thread runtime |
| UniFFI 生成代码问题 | 中 | 低 | 保持简单接口，充分测试 |
| SQLite 并发问题 | 低 | 低 | 使用 WAL 模式，连接池 |

## 备选方案

### 如果 mihomo Android 集成困难

**方案 B: 嵌入式 libclash**

直接将 mihomo 核心编译为 Android 库 (需 Go → C → Rust FFI):

```
优点: 完全控制，无需外部 APK
缺点: 复杂的 CGO 交叉编译，VPN 权限处理复杂
```

**方案 C: 纯配置管理应用**

只实现配置管理和 WebDAV 同步，不控制 VPN：

```
优点: 简单，可快速发布
缺点: 用户需手动安装 ClashMeta Android，体验割裂
```

## 下一步行动

### 立即执行 (Phase 1 Week 1)

1. [x] 创建目标 crates 骨架（mihomo-api/mihomo-config/mihomo-platform/mihomo-version/infiltrator-core/infiltrator-desktop/infiltrator-android）
2. [x] 迁移 mihomo-api 当前实现（client/types/error/proxy/connection）
3. [x] 迁移 mihomo-config 当前实现（manager/profile/yaml + home/port）
4. [x] 迁移 mihomo-version 当前实现（channel/download/manager + home）
5. [x] mihomo-platform 提供 trait 与 Desktop/Android 凭据存储/ProcessCoreController 实现
6. [x] infiltrator-core/infiltrator-desktop 完成文件拆分并改用新 crates
7. [x] despicable-infiltrator-core 切换为兼容 re-export
8. [x] mihomo-rs 切换为兼容 re-export（Tauri 导入未改）
9. [x] 切换 Tauri 导入到新 crates 并完成构建与测试
10. [x] 清理 despicable-infiltrator-core 旧源码，仅保留 re-export
11. [x] 移除 mihomo-rs 与 despicable-infiltrator-core 兼容层
12. [x] infiltrator-android 预留 AndroidBridge 接口占位

### 验收标准

- [x] Tauri 导入已切换到新 crates
- [x] `cargo build -p "MusicFrog-Despicable-Infiltrator"` 成功
- [x] `cargo test --workspace` 全部通过
- [ ] 手动测试 Tauri 应用功能正常

---

*最后更新: 2026-01-10*
