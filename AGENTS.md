# Repository Guidelines

## Project Name

- Mihomo-Despicable-Infiltrator

## 项目结构与模块说明

- `src-tauri/`：Tauri v2 Rust 后端（托盘主进程，管理 mihomo-rs、mihomo 内核与 Axum 服务；TUN 由 Web UI 在管理员权限下管理）。**核心**
- `config-manager-ui/`：订阅/配置管理 UI 代码（管理界面）。**核心**
- `zashboard/`：Mihomo Web UI 代码。**核心**
- `mihomo-rs/`：Tauri 后端使用的 Rust SDK，包含测试与示例。
- `mihomo-windows-amd64-v3.exe`：离线备用内核二进制（Windows）。

## 构建、测试与开发命令

- `pnpm install`：安装根目录工具（Tauri CLI）。
- `pnpm dev`：启动仅托盘模式的 Tauri 应用。
- `pnpm build`：通过 Tauri 生成安装包（Windows 需要产出 `.msi`）。
- 在 `mihomo-rs/` 执行 `cargo test`：运行 Rust SDK 测试。
- 在 `src-tauri/` 执行 `cargo test`：运行后端测试（若有）。

## 编码风格与命名规范

- Rust：遵循 `rustfmt` 默认格式（4 空格缩进）；模块 `snake_case`、类型 `PascalCase`、函数 `snake_case`。
- 非必要不修改二进制资源与生成物。

## 测试规范

- Rust 测试位于 `mihomo-rs/tests/`，使用标准 `cargo test`。
- 新测试名称要清晰（如 `config_manager_test.rs`），辅助代码放在 `mihomo-rs/tests/common/`。
- UI 测试由上游处理，本仓库根目录未配置 UI 测试运行器。

## 提交与 PR 规范

- 当前检出无 Git 历史，无法推断提交规范。
- 若补充 Git 历史，建议使用 Conventional Commits（如 `feat(tauri): add tray toggle`）。
- PR 应包含：简要说明、关联 issue（如有）、变更 UI 时的截图（`config-manager-ui/` 或 `zashboard/`）。

## 安全与配置提示

- 后端默认托管 `bin/zashboard/`；如需覆盖，设置 `METACUBEXD_STATIC_DIR=/absolute/path/to/dist`。
- 管理界面默认托管 `bin/config-manager/`；如需覆盖，设置 `METACUBEXD_ADMIN_DIR=/absolute/path/to/dist`。
- 默认 mihomo 配置目录：Windows 为 `%USERPROFILE%\\.config\\mihomo-rs\\configs`，Unix 为 `~/.config/mihomo-rs/configs`。
- 捆绑备用内核为 `mihomo-windows-amd64-v3.exe`，更新需谨慎并记录。

## 权限与提升运行（托盘）

- TUN 相关操作由 Web UI 管理，前提是应用以管理员权限运行；托盘主进程不再直接管理 TUN。
- Windows 现状：托盘菜单提供“以管理员身份重启”，重启前会正常关闭 Web 服务和 mihomo，并保留端口参数，避免端口漂移。
- Windows 说明：无法绕过 UAC 静默提权，托盘菜单提供“开机自启（计划任务）”开关。
- 跨平台备注：后续适配时，macOS 可用 launchd/SMAppService，Linux 可用 polkit/pkexec（需要额外实现）。

## 运行时功能（托盘 + 服务）

- 托盘主进程通过 `mihomo-rs` 启动 mihomo 内核，托管 Web UI 与配置管理的 Axum 静态服务，并发送运行事件。
- 托盘菜单包含：打开 Web UI、系统代理切换（Windows）、内核更新到最新稳定版、管理员重启状态、开机自启、启动时打开 Web UI。
- 内核更新使用 `mihomo-rs` VersionManager，下载稳定版并显示进度与网络状态，重启内核并仅保留最新版本。
- 配置管理支持订阅导入、本地文件导入、外部编辑器打开配置，并可切换已下载的内核版本。
