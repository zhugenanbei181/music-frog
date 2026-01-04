# 仓库指南

## 项目名称

- Mihomo-Despicable-Infiltrator

## 项目结构与模块说明

- `src-tauri/`：Tauri v2 Rust 后端（托盘主进程，管理 mihomo-rs、mihomo 内核与 Axum 服务；TUN 由 Web UI 在管理员权限下管理）。**核心**
  - `src-tauri/src/main.rs`：入口与应用生命周期，组装托盘、运行时与服务。
  - `src-tauri/src/tray.rs`：托盘菜单与交互事件（内核更新、管理员重启、自启等）。
  - `src-tauri/src/runtime.rs`：运行时启动/重启/停止，桥接 core crate。
  - `src-tauri/src/frontend.rs`：静态站点与管理界面托管。
  - `src-tauri/src/app_state.rs`：全局状态与托盘信息更新。
  - `src-tauri/src/admin_context.rs`：Admin API 上下文实现（连接 core）。
  - `src-tauri/src/core_update.rs`：内核更新流程与进度回传。
  - `src-tauri/src/autostart.rs`：Windows 计划任务自启开关。
  - `src-tauri/src/system_proxy.rs`：Windows 系统代理开关。
  - `src-tauri/src/platform.rs`：平台能力封装（管理员重启、权限检测）。
  - `src-tauri/src/paths.rs`：运行时路径/资源定位。
  - `src-tauri/src/settings.rs`：运行时设置读写与重置。
  - `src-tauri/src/factory_reset.rs`：恢复出厂设置流程（清空配置/日志/设置等）。
  - `src-tauri/src/utils.rs`：通用工具（端口解析、等待释放）。
- `crates/despicable-infiltrator-core/`：核心业务 crate（与 Tauri 解耦）。**核心**
  - `crates/despicable-infiltrator-core/src/lib.rs`：对外模块导出。
  - `crates/despicable-infiltrator-core/src/runtime.rs`：mihomo 运行时编排与生命周期。
  - `crates/despicable-infiltrator-core/src/admin_api.rs`：Axum 管理 API（订阅导入/配置切换/重启）。
  - `crates/despicable-infiltrator-core/src/servers.rs`：静态服务与管理服务封装。
  - `crates/despicable-infiltrator-core/src/profiles.rs`：配置档案读取/保存/清空逻辑。
  - `crates/despicable-infiltrator-core/src/config.rs`：配置校验（YAML/TOML）。
  - `crates/despicable-infiltrator-core/src/editor.rs`：外部编辑器探测与打开配置。
  - `crates/despicable-infiltrator-core/src/version.rs`：版本排序与展示辅助。
  - `crates/despicable-infiltrator-core/src/settings.rs`：运行时设置读取/迁移。
  - `crates/despicable-infiltrator-core/src/proxy.rs`：系统代理状态结构与格式化。
- `config-manager-ui/`：配置管理 UI 代码，基于 Vue 3 + TypeScript + Tailwind CSS。**核心**
  - `config-manager-ui/src/main.ts`：前端入口。
  - `config-manager-ui/src/App.vue`：页面布局与业务编排。
  - `config-manager-ui/src/api.ts`：Admin API 请求封装。
  - `config-manager-ui/src/types.ts`：前端类型定义。
  - `config-manager-ui/src/styles.css`：基础样式与设计变量。
  - `config-manager-ui/src/components/StatusHeader.vue`：顶部状态与刷新入口。
  - `config-manager-ui/src/components/ProfilesPanel.vue`：配置列表与筛选。
  - `config-manager-ui/src/components/ImportSubscriptionPanel.vue`：订阅导入。
  - `config-manager-ui/src/components/ImportLocalPanel.vue`：本地文件导入。
  - `config-manager-ui/src/components/EditorPanel.vue`：配置编辑器。
  - `config-manager-ui/src/components/EditorSettingsPanel.vue`：外部编辑器设置。
  - `config-manager-ui/src/components/CorePanel.vue`：内核版本管理入口。
  - `config-manager-ui/src/components/BusyOverlay.vue`：忙碌遮罩与进度提示。
  - `config-manager-ui/src/components/ToastList.vue`：临时通知。
- `zashboard/`：Mihomo Web UI 静态资源。**核心**
- `mihomo-rs/`：Tauri 后端使用的 Rust SDK，包含测试与示例。
- `mihomo.exe`：离线备用内核二进制（Windows，兼容旧名 `mihomo-windows-amd64-v3.exe`）。
- `CHANGELOG.md`：版本记录（新版本在前）。
- `USAGE_SPEC.md`：使用规范说明（命名与目录约定）。

## 构建、测试与开发命令

- `pnpm install`：安装根目录工具（Tauri CLI）。
- `pnpm dev`：启动仅托盘模式的 Tauri 应用。
- `pnpm build`：通过 Tauri 生成安装包（Windows 需要产出 `.msi`）。
- 在 `mihomo-rs/` 执行 `cargo test`：运行 Rust SDK 测试。
- 在 `src-tauri/` 执行 `cargo test`：运行后端测试（若有）。

## 编码风格与命名规范

- Rust：遵循 `rustfmt` 默认格式（4 空格缩进）；模块 `snake_case`、类型 `PascalCase`、函数 `snake_case`。
- Rust：**禁止使用 `unsafe`**。如需系统能力必须优先选用安全库；确有必要时需给出安全替代方案或完整安全性说明并记录到规范中。
- 非必要不修改二进制资源与生成物。

## 测试规范

- Rust 测试位于 `mihomo-rs/tests/`，使用标准 `cargo test`。
- 新测试名称要清晰（如 `config_manager_test.rs`），辅助代码放在 `mihomo-rs/tests/common/`。
- UI 测试由上游处理，本仓库根目录未配置 UI 测试运行器。

## 提交与 PR 规范

- 当前检出无 Git 历史，无法推断提交规范。
- 若补充 Git 历史，建议使用 Conventional Commits（如 `feat(tauri): add tray toggle`）。
- PR 应包含：简要说明、关联 issue（如有）、变更 UI 时的截图（`config-manager-ui/` 或 `zashboard/`）。
- Each bug fix must bump the patch version and update `CHANGELOG.md`, `README.md`, `USAGE_SPEC.md`.

## 安全与配置提示

- 后端默认托管 `bin/zashboard/`；如需覆盖，设置 `METACUBEXD_STATIC_DIR=/absolute/path/to/dist`。
- 管理界面默认托管 `bin/config-manager/`；如需覆盖，设置 `METACUBEXD_ADMIN_DIR=/absolute/path/to/dist`。
- 默认 mihomo 配置目录：Windows 为 `%USERPROFILE%\\.config\\mihomo-rs\\configs`，Unix 为 `~/.config/mihomo-rs/configs`。
- GeoIP 数据库文件：Windows 为 `%USERPROFILE%\.config\mihomo-rs\configs\geoip.metadb`，下载源可用 `MIHOMO_GEOIP_URL` 覆盖。（默认会尝试 GitHub 与 jsdelivr 镜像）
- 可将 `geoip.metadb` 放在内核同目录（`bin/mihomo/`）便于首次启动无网络时使用。
- Mihomo 日志文件：Windows 为 `%USERPROFILE%\.config\mihomo-rs\logs\mihomo.log`。
- 应用日志（托盘/Axum）：Windows 为 `%LOCALAPPDATA%\\com.mihomo.despicable-infiltrator\\logs\\Mihomo-Despicable-Infiltrator.log`。
- 捆绑备用内核为 `mihomo.exe`（兼容 `mihomo-windows-amd64-v3.exe`），更新需谨慎并记录。
- 运行时设置文件：Windows 为 `%APPDATA%\\com.mihomo.despicable-infiltrator\\settings.toml`（旧 settings.json 自动迁移），包含 `open_webui_on_startup`、`editor_path`、`use_bundled_core`。
- 版本管理目录：Windows 为 `%USERPROFILE%\\.config\\mihomo-rs\\versions`，默认版本记录在 `%USERPROFILE%\\.config\\mihomo-rs\\config.toml`。
- 开机自启使用计划任务，任务名 `MihomoDespicableInfiltrator`。
- 使用规范请见 `USAGE_SPEC.md`，包含默认内核命名与安装目录约定。

## 权限与提升运行（托盘）

- TUN 相关操作由 Web UI 管理，前提是应用以管理员权限运行；托盘主进程不再直接管理 TUN。
- Windows 现状：托盘菜单提供“以管理员身份重启”，重启前会正常关闭 Web 服务和 mihomo，并保留端口参数，避免端口漂移。
- Windows 说明：无法绕过 UAC 静默提权，托盘菜单提供“开机自启（计划任务）”开关。
- 跨平台备注：后续适配时，macOS 可用 launchd/SMAppService，Linux 可用 polkit/pkexec（需要额外实现）。

## 运行时功能（托盘 + 服务）

- 托盘主进程通过 `mihomo-rs` 启动 mihomo 内核，托管 Web UI 与配置管理的 Axum 静态服务，并发送运行事件。
- 托盘菜单包含：打开 Web UI、系统代理切换（Windows）、内核更新到最新稳定版、管理员重启状态、开机自启、启动时打开 Web UI。
- 内核更新使用 `mihomo-rs` VersionManager，下载稳定版并显示进度与网络状态。
- 配置管理支持订阅导入、本地文件导入、外部编辑器打开配置，并可切换已下载的内核版本。
