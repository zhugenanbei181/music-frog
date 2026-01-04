# 使用规范说明

本文件用于约定资源命名、目录结构与配置保存位置，避免运行时不一致。

## 安装目录资源规范

- 默认内核存放目录：
  - `bin/mihomo/`
- 默认内核文件名（兼容两种）：
  - 推荐：`mihomo.exe`
  - 兼容：`mihomo-windows-amd64-v3.exe`

> 安装包内建议统一使用 `mihomo.exe`，运行时仍兼容旧命名。

## 配置与数据目录（Windows）

- 运行时设置：
  - `%APPDATA%\com.mihomo.despicable-infiltrator\settings.toml`
  - 旧 settings.json 会在读取后自动迁移为 settings.toml
  - 字段：`open_webui_on_startup`、`editor_path`、`use_bundled_core`
- 内核版本管理（`mihomo-rs`）：
  - `%USERPROFILE%\.config\mihomo-rs\versions\<version>\mihomo.exe`
  - 默认版本记录：`%USERPROFILE%\.config\mihomo-rs\config.toml`
- 内核配置文件：
  - `%USERPROFILE%\.config\mihomo-rs\configs`
- GeoIP 数据库文件：
  - `%USERPROFILE%\.config\mihomo-rs\configs\geoip.metadb`
  - 下载源可用环境变量覆盖：`MIHOMO_GEOIP_URL`（默认会尝试 GitHub 与 jsdelivr 镜像）
  - 也可将 `geoip.metadb` 放在内核同目录（`bin/mihomo/`），启动时会自动复制
- Mihomo 日志文件：
  - `%USERPROFILE%\.config\mihomo-rs\logs\mihomo.log`
  - 应用日志（托盘/Axum）：`%LOCALAPPDATA%\\com.mihomo.despicable-infiltrator\\logs\\Mihomo-Despicable-Infiltrator.log`

## 开机自启

- 计划任务方式：
  - 任务名：`MihomoDespicableInfiltrator`
  - 仅管理员权限可创建/开启
