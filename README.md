# Mihomo Despicable Infiltrator

Mihomo Despicable Infiltrator 是一个 Tauri v2 托盘应用，使用 `mihomo-rs` 管理 mihomo 内核，并通过内置 Axum 服务提供 Web UI 与订阅管理界面。

## 功能概览

- 托盘主进程：启动/监控 mihomo 内核、发送运行态事件。
- Web UI：内置静态服务器托管 `zashboard`（Mihomo Web UI）与 `config-manager-ui`（订阅/配置管理）。
- 配置管理：管理配置列表、导入订阅或本地文件、保存/切换/删除配置，支持外部编辑器打开。
- 配置清空：一键清空配置并恢复默认配置。
- 系统代理切换（Windows）。
- 内核更新：获取最新 Stable 版本，下载进度可视化，重启内核并清理旧版本。
- 内核版本管理：可手动切换已下载的内核版本，首次启动无版本时使用捆绑内核。
- 管理员权限重启：安全关闭服务后重启，并保持端口稳定。
- 开机自启：Windows 使用计划任务；可在托盘切换“启动时打开 Web UI”。
- 恢复出厂设置：清空配置、已下载内核、日志与运行时设置，重启为默认状态。

## 目录结构

- `src-tauri/` – Tauri v2 Rust 后端（托盘主进程 + Axum 服务）。
- `mihomo.exe` – 离线备用内核二进制（兼容旧名 `mihomo-windows-amd64-v3.exe`）。
- `mihomo-rs/`（可选）– SDK 源码，便于调试与对齐 API。
- `zashboard/` – Mihomo Web UI 静态资源。
- `config-manager-ui/` – 订阅/配置管理 UI 静态资源。
- `CHANGELOG.md` – 版本记录（新版本在前）。
- `USAGE_SPEC.md` – 使用规范说明（命名与目录约定）。

## 模块细分（开发参考）

### `src-tauri/src/`

- `main.rs`：入口与应用生命周期，组装托盘/运行时/静态服务。
- `tray.rs`：托盘菜单构建与交互事件。
- `runtime.rs`：运行时启动/重启/停止，桥接 core crate。
- `frontend.rs`：静态站点与管理界面托管。
- `app_state.rs`：全局状态与托盘状态文本更新。
- `admin_context.rs`：Admin API 上下文实现（连接 core）。
- `core_update.rs`：内核更新流程与进度上报。
- `autostart.rs`：Windows 计划任务自启开关。
- `system_proxy.rs`：Windows 系统代理开关。
- `platform.rs`：平台能力封装（管理员重启、权限检测）。
- `paths.rs`：运行时路径/资源定位。
- `settings.rs`：运行时设置读写与重置。
- `factory_reset.rs`：恢复出厂设置流程（清空配置/日志/设置等）。
- `utils.rs`：通用工具（端口解析、等待释放）。

### `crates/despicable-infiltrator-core/src/`

- `lib.rs`：对外模块导出。
- `runtime.rs`：mihomo 运行时编排与生命周期。
- `admin_api.rs`：Axum 管理 API（订阅导入/配置切换/重启）。
- `servers.rs`：静态服务与管理服务封装。
- `profiles.rs`：配置档案读取/保存/清空逻辑。
- `config.rs`：配置校验（YAML/TOML）。
- `editor.rs`：外部编辑器探测与打开配置。
- `version.rs`：版本排序与展示辅助。
- `settings.rs`：运行时设置读取/迁移。
- `proxy.rs`：系统代理状态结构与格式化。

### `config-manager-ui/src/`

- `main.ts`：前端入口。
- `App.vue`：页面布局与业务编排。
- `api.ts`：Admin API 请求封装。
- `types.ts`：前端类型定义。
- `styles.css`：基础样式与设计变量。
- `components/StatusHeader.vue`：顶部状态与刷新入口。
- `components/ProfilesPanel.vue`：配置列表与筛选。
- `components/ImportSubscriptionPanel.vue`：订阅导入。
- `components/ImportLocalPanel.vue`：本地文件导入。
- `components/EditorPanel.vue`：配置编辑器。
- `components/EditorSettingsPanel.vue`：外部编辑器设置。
- `components/CorePanel.vue`：内核版本管理入口。
- `components/BusyOverlay.vue`：忙碌遮罩与进度提示。
- `components/ToastList.vue`：临时通知。

## 环境要求

- Node.js ≥ 18.18
- pnpm ≥ 8
- Rust toolchain (1.75+ recommended) + `cargo`

## 安装依赖

```powershell
# 在仓库根目录
pnpm install            # 安装 Tauri CLI
pnpm --dir config-manager-ui install  # 安装配置管理 UI 依赖
```

## 开发流程

```powershell
# 启动仅托盘模式的 Tauri 应用
pnpm dev
```

`pnpm dev` / `tauri dev` 过程说明：

1. Tauri 后端（`src-tauri/src/main.rs`）无窗口启动，通过 `mihomo-rs` 启动内核，并同时托管 `zashboard/` 与 `config-manager-ui/` 的静态服务。
2. Web UI 默认不自动打开浏览器，可在托盘菜单中启用“启动时打开 Web UI”；托盘左键或“显示主界面”可打开。
3. 后端事件：
   - `mihomo://ready` – 内核就绪时的控制器地址与配置路径。
   - `mihomo://traffic` – 来自 `mihomo-rs` 的流量数据。
   - `mihomo://summary`、`mihomo://mode-changed`、`mihomo://error` – 托盘交互与状态事件。

## 构建/打包流程

```powershell
# 先构建配置管理 UI 静态资源
pnpm --dir config-manager-ui build

# 再打包 Tauri 安装包
pnpm build
```

Tauri 打包包含：

- Web UI 静态资源（`bin/zashboard/`）
- 配置管理 UI 静态资源（`bin/config-manager/`）
- 备用内核 `bin/mihomo/mihomo.exe`（见 `tauri.conf.json > bundle.resources`）

`mihomo-rs` 会通过 `VersionManager` 安装/更新官方版本；若失败（离线/防火墙等），回退使用捆绑的 Windows 二进制，并存放在应用数据目录。

MSI 输出路径（Windows）：

- `target/release/bundle/msi/Mihomo Despicable Infiltrator_0.8.3_x64_zh-CN.msi`

版本记录请查看 `CHANGELOG.md`（新版本在前）。
使用规范请查看 `USAGE_SPEC.md`（命名与目录约定）。

## 运行时结构与目录

- 资源打包路径：`bin/`
  - `bin/zashboard/`：Web UI 静态资源
  - `bin/config-manager/`：订阅/配置管理 UI 静态资源
  - `bin/mihomo/mihomo.exe`：备用内核
- 端口策略：
  - Web UI 默认从 `4173` 起寻找可用端口
  - 配置管理默认从 `5210` 起寻找可用端口
  - 管理员重启会携带 `--static-port` / `--admin-port`，避免端口漂移
- 权限：
  - 需要管理员权限时，托盘菜单可触发“以管理员身份重启”
  - 开机自启使用计划任务，默认关闭

## 配置与数据目录

- 运行时设置（托盘设置）：
  - Windows：`%APPDATA%\\com.mihomo.despicable-infiltrator\\settings.toml`（旧 `settings.json` 自动迁移）
  - 字段：`open_webui_on_startup`、`editor_path`、`use_bundled_core`
- 内核版本管理目录（`mihomo-rs` VersionManager）：
  - Windows：`%USERPROFILE%\\.config\\mihomo-rs\\versions\\<version>\\mihomo.exe`
  - 默认版本记录：`%USERPROFILE%\\.config\\mihomo-rs\\config.toml`
- 内核配置文件目录：
  - Windows：`%USERPROFILE%\\.config\\mihomo-rs\\configs`
- GeoIP 数据库文件：
  - Windows：`%USERPROFILE%\\.config\\mihomo-rs\\configs\\geoip.metadb`
  - 下载源可用环境变量覆盖：`MIHOMO_GEOIP_URL`（默认会尝试 GitHub 与 jsdelivr 镜像）
  - 也可将 `geoip.metadb` 放在内核同目录（`bin/mihomo/`），启动时会自动复制
- Mihomo 日志文件：
  - Windows：`%USERPROFILE%\\.config\\mihomo-rs\\logs\\mihomo.log`
  - 应用日志（托盘/Axum）：Windows：`%LOCALAPPDATA%\\com.mihomo.despicable-infiltrator\\logs\\Mihomo-Despicable-Infiltrator.log`
- 开机自启：
  - Windows 通过计划任务保存，任务名 `MihomoDespicableInfiltrator`

## Rust 后端概览

- `MihomoRuntime::bootstrap`（`src-tauri/src/main.rs`）负责串联 SDK：
  - `ConfigManager` 保证配置与控制器。
  - `VersionManager` 解析内核二进制或使用捆绑版本。
  - 启动服务并推送流量/日志到 Tauri 事件。
- UI 可调用的命令（`invoke`）：
  - `get_mihomo_summary` – 代理组、模式、控制器地址、运行状态。
  - `toggle_core_mode` – 切换 Rule/Global 模式。
  - `switch_proxy_group` – 切换代理组节点。
  - `restart_mihomo` – 重启服务并重载配置。
- 系统托盘（Tauri v2 `TrayIconBuilder`）快捷功能：
  - 打开 Web UI / 配置管理
  - 系统代理切换（Windows）
  - 内核管理（版本展示/更新）
  - 选择已下载内核版本
  - 以管理员身份重启
  - 开机自启 / 启动时打开 Web UI
  - 退出

后续 UI 可通过 `window.__TAURI__.invoke` 或监听事件与托盘保持同步，避免重复实现后端调用。

## 贡献/发布说明

- 版本号：每次发布递增小版本（例如 `0.5.2 -> 0.5.3`），同步更新 `src-tauri/Cargo.toml`、`src-tauri/tauri.conf.json`、`package.json`。
- 测试：`cargo test`（`src-tauri/`）用于编译与基础回归。
- 打包：`pnpm build` 产出 MSI（路径见上文）。
- 许可证合规：发布前请核对所用库的许可证与二次分发要求。

主要依赖与许可证（以 Cargo registry 元数据为准）：

- `tauri` – Apache-2.0 OR MIT
- `tauri-build` – Apache-2.0 OR MIT
- `tauri-plugin-log` – Apache-2.0 OR MIT
- `axum` – MIT
- `tower-http` – MIT
- `tokio` – MIT
- `reqwest` – MIT OR Apache-2.0
- `serde` / `serde_json` – MIT OR Apache-2.0
- `yaml-rust2` – MIT OR Apache-2.0
- `webbrowser` – MIT OR Apache-2.0
- `rfd` – MIT
- `is_elevated` – MIT
- `winreg` – MIT
- `mihomo-rs` – MIT (see `mihomo-rs/Cargo.toml`)

静态 UI（`zashboard/`、`config-manager-ui/`）可能包含上游资产与字体，请在发布前确认其上游许可证要求。

## 额外提示

- 静态服务器默认使用 `zashboard/`；如需覆盖，设置 `METACUBEXD_STATIC_DIR=/absolute/path/to/dist`。
- 管理界面默认使用 `config-manager-ui/`；如需覆盖，设置 `METACUBEXD_ADMIN_DIR=/absolute/path/to/dist`。
- 若自维护 mihomo 配置，请放置在 `%USERPROFILE%\\.config\\mihomo-rs\\configs`（Windows）或 `~/.config/mihomo-rs/configs`。
- 构建产物默认输出到 `target/`，已在 `.gitignore` 忽略；如需提交或归档，请自行处理。
