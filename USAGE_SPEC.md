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
  - `editor_path` 可填写 `code`/`Code.exe`/`.cmd`，为空则自动探测 VSCode，失败回退记事本
  - 编辑器路径可通过 UI 的选择对话框设置，仅展示可执行文件
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

## 订阅更新与通知

- 托盘菜单「配置切换」提供“立即更新所有订阅”。
- 自动更新/手动更新成功或失败会触发系统通知。
- 订阅链接默认存储在系统密钥链，配置文件保留回退副本用于密钥链丢失时恢复。
- 不同配置的订阅链接分别存储，互不影响（key 格式：`subscription:{profile}`）。
- Windows 可在“控制面板 → 凭据管理器 → Windows 凭据”中查看/删除，目标项名称为 `Mihomo-Despicable-Infiltrator`，账号名为 `subscription:{profile}`。

## 托盘菜单容错

- 菜单项名称过长会自动截断显示，避免影响托盘初始化。
- 菜单项 ID 使用稳定哈希生成，与配置或节点真实名称解耦。

## 清空配置

- 入口：配置管理界面「清空配置」。
- 行为：删除现有配置列表并生成默认配置，控制接口端口将重新分配。

## 恢复出厂设置

- 入口：托盘菜单「恢复出厂设置」。
- 行为：停止服务并清除应用设置、配置、已下载内核与日志，随后重启为默认状态。

## 开机自启

- 计划任务方式：
  - 任务名：`MihomoDespicableInfiltrator`
  - 仅管理员权限可创建/开启

- 以上选项默认不勾选。

