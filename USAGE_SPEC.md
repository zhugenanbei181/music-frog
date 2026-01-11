# Usage Specification / 使用指南

This document provides instructions for users on how to interact with the **MusicFrog Despicable Infiltrator** interface and its features.
本文档为用户提供如何使用 **MusicFrog Despicable Infiltrator** 界面及其功能的说明。

---

## 1. Tray Menu / 托盘菜单

The tray menu provides quick access to connection info, proxy/config pages, and core controls.
托盘菜单提供连接信息、代理/配置页面与核心控制的快速访问。

- **Open Proxy Page / 打开代理页**: Opens the proxy Web UI in your default browser.
  **打开代理页**: 在默认浏览器中打开代理 Web UI。
- **Open Config Page / 打开配置页**: Opens the configuration page (Admin Web UI) in your default browser.
  **打开配置页**: 在默认浏览器中打开配置页面（管理 Web UI）。
- **System Proxy / 系统代理**: Toggle the system-wide proxy on or off.
  **系统代理**: 切换全局系统代理的开启或关闭。
- **Proxy Mode / 代理模式**: Switch between `Rule`, `Global`, `Direct`, and `Script` modes.
  **代理模式**: 在 `规则`、`全局`、`直连` 和 `脚本` 模式之间切换。
- **Profile Switch / 配置切换**: Quickly switch between different proxy profiles.
  **配置切换**: 在不同的代理配置之间快速切换。
- **Advanced Settings / 高级设置**: Direct links to specific configuration panels (DNS, Tun, Rules, etc.).
  **高级设置**: 直达特定配置面板（DNS、Tun、规则等）的链接。

---

## 2. Admin UI Navigation / 管理界面导航

The Admin Web UI uses a left-side **Sections** menu and a fixed top status header.
管理 Web UI 使用左侧 **导航** 菜单与顶部固定状态栏。

- **Status Header / 顶部状态栏**:
  - **Language / 语言**: Select Follow System / 简体中文 / English.
      **Language / 语言**: 选择 跟随系统 / 简体中文 / English。
  - **Theme / 深色模式**: Select Follow System / Light / Dark.
      **Theme / 深色模式**: 选择 跟随系统 / 浅色 / 深色。
  - **Refresh Status / 刷新状态**: Reload status and panel data.
      **Refresh Status / 刷新状态**: 刷新状态与各面板数据。
  - **Refresh Updates / 刷新更新**: Appears when background changes are detected; click to reload the latest data.
      **Refresh Updates / 刷新更新**: 检测到后台变更时出现，点击后刷新最新数据。
- **Sections / 导航分组**:
  - **Profiles & Imports / 配置管理**: Profile list, subscription import, local import, editor, and external editor.
      **Profiles & Imports / 配置管理**: 配置列表、订阅导入、本地导入、编辑器与外部编辑器。
  - **WebDAV Sync / WebDAV 同步**: WebDAV configuration, test, and manual sync.
      **WebDAV Sync / WebDAV 同步**: WebDAV 配置、连接测试与手动同步。
  - **DNS & Fake-IP / DNS / Fake-IP**: DNS settings and Fake-IP cache control.
      **DNS & Fake-IP / DNS / Fake-IP**: DNS 设置与 Fake-IP 缓存控制。
  - **Core & TUN / 内核与 TUN**: Core version switching and TUN advanced settings.
      **Core & TUN / 内核与 TUN**: 内核版本切换与 TUN 高级设置。
  - **Rules & Providers / 规则与 Providers**: Rule providers and rule list management.
      **Rules & Providers / 规则与 Providers**: Rule Providers 与规则列表管理。

---

## 3. Profiles & Imports / 配置管理

Manage profiles, imports, editing, and synchronization in the **Profiles & Imports** section.
在 **Profiles & Imports**（配置管理）分组中完成配置管理、导入、编辑与同步。

- **Profiles / 现有配置**:
  - **Refresh / 刷新**: Reload profile list and status.
      **Refresh / 刷新**: 重新加载配置列表与状态。
  - **Clear Configs / 清空配置**: Clear all configs and restore defaults.
      **Clear Configs / 清空配置**: 清空配置并恢复默认。
  - **Set Active / 设为当前**: Activate the selected profile and restart the core.
      **Set Active / 设为当前**: 激活所选配置并重启内核。
  - **Edit / 编辑**: Open the built-in **Config Editor** with the selected profile.
      **Edit / 编辑**: 打开内置 **Config Editor** 进行编辑。
  - **External Edit / 外部编辑**: Launch the external editor for the selected profile.
      **External Edit / 外部编辑**: 用外部编辑器打开所选配置。
  - **Delete / 删除**: Remove the selected profile.
      **Delete / 删除**: 删除所选配置。
  - **Update Now / 立即更新** + **Save Settings / 保存订阅设置**: Update subscription and save its schedule.
      **Update Now / 立即更新** + **Save Settings / 保存订阅设置**: 立即更新订阅并保存更新计划。
- **Import Subscription / 通过订阅导入**:
  - **Name / 配置名称** + **Subscription URL / 订阅链接**: Fill in the subscription metadata.
      **Name / 配置名称** + **Subscription URL / 订阅链接**: 填写订阅名称与 URL。
  - **Set as active after import / 导入后设为当前配置** + **Import Now / 立即导入**:
    Import the subscription and optionally activate it (core restarts in background).
      **Set as active after import / 导入后设为当前配置** + **Import Now / 立即导入**:
        导入订阅并可选自动激活（内核后台重启）。
- **Import Local File / 从本地文件导入**:
  - **Select File / 选择文件**: Choose a `.yaml` or `.toml` file from disk.
      **Select File / 选择文件**: 从磁盘选择 `.yaml` 或 `.toml`。
  - **Save Local Config / 保存本地配置**: Import and save the local file.
      **Save Local Config / 保存本地配置**: 导入并保存本地文件。
- **Config Editor / 配置编辑器**:
  - **Save Config / 保存配置**: Save the content as a profile, optionally activate it.
      **Save Config / 保存配置**: 保存内容为配置，并可选激活。
  - **External Edit / 外部编辑**: Open the current profile in external editor.
      **External Edit / 外部编辑**: 用外部编辑器打开当前配置。
- **External Editor / 外部编辑器设置**:
  - **Select Path / 选择路径**: Pick the external editor executable.
      **Select Path / 选择路径**: 选择外部编辑器路径。
  - **Save / 保存** + **Reset / 恢复默认**: Save or reset editor settings.
      **Save / 保存** + **Reset / 恢复默认**: 保存或重置外部编辑器设置。

---

## 4. WebDAV Sync / WebDAV 同步

Manage sync settings, tests, and manual sync in the **WebDAV Sync** section.
在 **WebDAV Sync** 分组中管理同步设置、连接测试与手动同步。

- **Test Connection / 连接测试**: Verify the WebDAV server.
    **Test Connection / 连接测试**: 验证 WebDAV 服务可用性。
- **Sync Now / 立即同步**: Start a manual synchronization.
    **Sync Now / 立即同步**: 手动触发同步。
- **Save / 保存**: Persist sync settings.
    **Save / 保存**: 保存同步设置。

---

## 5. DNS & Fake-IP / DNS 与 Fake-IP

Tune network behavior in the **DNS & Fake-IP** section.
在 **DNS & Fake-IP** 分组中调整网络行为。

- **DNS Settings / DNS 设置**:
  - **Save / 保存**: Apply DNS settings.
      **Save / 保存**: 应用 DNS 设置。
  - **Refresh / 刷新**: Reload DNS settings.
      **Refresh / 刷新**: 重新加载 DNS 设置。
- **Fake-IP / Fake-IP**:
  - **Save / 保存**: Apply Fake-IP settings.
      **Save / 保存**: 应用 Fake-IP 设置。
  - **Refresh / 刷新**: Reload Fake-IP settings.
      **Refresh / 刷新**: 重新加载 Fake-IP 设置。
  - **Flush Cache / 清理缓存**: Clear the Fake-IP cache.
      **Flush Cache / 清理缓存**: 清理 Fake-IP 缓存。

---

## 6. Core & TUN / 内核与 TUN

Manage core versions and advanced TUN settings in the **Core & TUN** section.
在 **Core & TUN** 分组中管理内核版本与 TUN 高级设置。

- **Core Version / 内核版本**:
  - **Refresh / 刷新**: Reload available core versions.
      **Refresh / 刷新**: 重新加载可用内核版本。
  - **Use / 启用**: Switch to the selected core version.
      **Use / 启用**: 切换到所选内核版本。
- **TUN Advanced / TUN 高级设置**:
  - **Save / 保存**: Apply TUN settings.
      **Save / 保存**: 应用 TUN 设置。
  - **Refresh / 刷新**: Reload TUN settings.
      **Refresh / 刷新**: 重新加载 TUN 设置。

---

## 7. Rules & Providers / 规则与 Providers

Manage rule providers and rules in the **Rules & Providers** section.
在 **Rules & Providers** 分组中管理 Rule Providers 与规则列表。

- **Rule Providers (JSON) / 规则提供者 (JSON)**:
  - **Save Providers / 保存 Providers**: Save the JSON providers configuration.
      **Save Providers / 保存 Providers**: 保存 Providers JSON 配置。
- **Rules / 规则列表**:
  - **Add Rule / 新增规则**: Insert a new rule line.
      **Add Rule / 新增规则**: 添加新规则行。
  - **Save Rules / 保存规则**: Apply rule list changes.
      **Save Rules / 保存规则**: 保存规则列表更改。
- **Rule list scroll / 规则列表滚动**: The rules list scrolls inside the panel to handle large sets.
      **Rule list scroll / 规则列表滚动**: 规则列表在面板内滚动，以适配大量规则。
