# 使用规范说明

## 资源路径

- **默认内核**: `bin/mihomo/mihomo.exe`
- **GeoIP 库**: `%USERPROFILE%\.config\mihomo-rs\configs\geoip.metadb` (支持 `MIHOMO_GEOIP_URL` 环境变量覆盖)

## 配置文件 (Windows)

- **运行时设置**: `%APPDATA%\com.mihomo.despicable-infiltrator\settings.toml`
  - `open_webui_on_startup`: 启动时打开 Web UI
  - `editor_path`: 外部编辑器路径
  - `use_bundled_core`: 是否强制使用捆绑内核
  - `language`: 界面语言 (zh-CN / en-US)
  - `[webdav]`: WebDAV 同步配置
    - `enabled`: 是否启用同步
    - `url`: WebDAV 服务器地址
    - `username`: 用户名
    - `password`: 密码/Token
    - `sync_interval_mins`: 自动同步间隔（分钟）
    - `sync_on_startup`: 启动时自动同步
- **内核配置**: `%USERPROFILE%\.config\mihomo-rs\configs`
- **WebDAV 状态数据库**: `%USERPROFILE%\.config\mihomo-rs\sync_state.db`
- **Mihomo 日志**: `%USERPROFILE%\.config\mihomo-rs\logs\mihomo.log`
- **应用日志**: `%LOCALAPPDATA%\com.mihomo.despicable-infiltrator\logs\Mihomo-Despicable-Infiltrator.log`

## 行为规范

- **订阅存储**: 链接存入系统凭据管理器 (`Mihomo-Despicable-Infiltrator`)。
- **配置清空**: 删除所有配置并恢复默认，重置端口。
- **出厂设置**: 清除所有配置、日志、已下载内核及应用设置，重启服务。
- **开机自启**: 使用 Windows 计划任务 (`MihomoDespicableInfiltrator`)，需管理员权限。
- **WebDAV 同步**:
  - 仅同步 `configs/` 目录下的 `.yaml/.yml/.toml` 文件
  - 冲突文件保存为 `.remote-bak-{timestamp}` 便于手动处理
  - 使用 `.sync-tmp` 临时文件确保原子写入
  - 基于 ETag 机制防止并发冲突
