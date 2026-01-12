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

---

## 8. Android App: VPN/TUN / Android 应用：VPN/TUN

Manage VPN/TUN parameters in the **Settings > TUN** screen.
在 **Settings > TUN** 页面管理 VPN/TUN 参数。

- **MTU / MTU**: Set the MTU value.
      **MTU / MTU**: 设置 MTU 数值。
- **Auto Route / 自动路由**: Toggle default routing through the VPN.
      **Auto Route / 自动路由**: 开关默认经由 VPN 的路由。
- **Strict Route / 严格路由**: Toggle strict routing behavior (Android uses route settings as available).
      **Strict Route / 严格路由**: 开关严格路由行为（Android 以现有路由设置为准）。
- **IPv6 / IPv6**: Enable or disable IPv6 routing for VPN.
      **IPv6 / IPv6**: 开关 VPN 的 IPv6 路由。
- **DNS Servers (one per line) / DNS Servers（每行一个）**: Enter DNS server IPs, one per line.
      **DNS Servers (one per line) / DNS Servers（每行一个）**: 逐行填写 DNS 服务器 IP。
- **Save / 保存**: Apply VPN/TUN settings.
      **Save / 保存**: 应用 VPN/TUN 设置。
- **Reload / 重新加载**: Reload current settings from the active profile.
      **Reload / 重新加载**: 从当前配置重新加载设置。

---

## 9. Android App: DNS / Android 应用：DNS

Manage DNS settings in **Settings > DNS**.
在 **Settings > DNS** 页面管理 DNS 设置。

- **Enable DNS / 启用 DNS**: Toggle DNS on or off.
      **Enable DNS / 启用 DNS**: 开关 DNS 功能。
- **IPv6 / IPv6**: Enable or disable IPv6 resolution.
      **IPv6 / IPv6**: 开关 IPv6 解析。
- **Enhanced Mode / 增强模式**: Enter `fake-ip` or `redir-host`.
      **Enhanced Mode / 增强模式**: 填写 `fake-ip` 或 `redir-host`。
- **Nameserver / 主 DNS**: Enter DNS servers, one per line.
      **Nameserver / 主 DNS**: 逐行填写 DNS 服务器。
- **Default Nameserver / 默认 DNS**: Enter default DNS servers, one per line.
      **Default Nameserver / 默认 DNS**: 逐行填写默认 DNS 服务器。
- **Fallback / 备用 DNS**: Enter fallback DNS servers, one per line.
      **Fallback / 备用 DNS**: 逐行填写备用 DNS 服务器。
- **Save / 保存**: Apply DNS settings.
      **Save / 保存**: 应用 DNS 设置。
- **Reload / 重新加载**: Reload current DNS settings.
      **Reload / 重新加载**: 重新加载 DNS 设置。

---

## 10. Android App: Fake-IP / Android 应用：Fake-IP

Manage Fake-IP settings in **Settings > Fake-IP**.
在 **Settings > Fake-IP** 页面管理 Fake-IP 设置。

- **Fake-IP Range / Fake-IP 范围**: Set the fake IP CIDR range.
      **Fake-IP Range / Fake-IP 范围**: 设置 Fake-IP 的 CIDR 范围。
- **Fake-IP Filter / Fake-IP 过滤**: Enter filter rules, one per line.
      **Fake-IP Filter / Fake-IP 过滤**: 逐行填写过滤规则。
- **Store Fake-IP / 持久化 Fake-IP**: Toggle persistence for fake IP cache.
      **Store Fake-IP / 持久化 Fake-IP**: 开关 Fake-IP 缓存持久化。
- **Save / 保存**: Apply Fake-IP settings.
      **Save / 保存**: 应用 Fake-IP 设置。
- **Reload / 重新加载**: Reload current Fake-IP settings.
      **Reload / 重新加载**: 重新加载 Fake-IP 设置。
- **Clear Cache / 清理缓存**: Clear the Fake-IP cache file.
      **Clear Cache / 清理缓存**: 清理 Fake-IP 缓存文件。

---

## 11. Android App: Rules / Android 应用：规则

Manage rules and providers in **Settings > Rules**.
在 **Settings > Rules** 页面管理规则与 Providers。

- **New Rule / 新增规则** + **Add Rule / 添加规则**: Add a new rule line.
      **New Rule / 新增规则** + **Add Rule / 添加规则**: 添加新规则行。
- **Rule Toggle / 规则开关**: Enable or disable an entry.
      **Rule Toggle / 规则开关**: 启用或禁用规则条目。
- **Remove / 删除**: Remove a rule entry.
      **Remove / 删除**: 删除规则条目。
- **Save Rules / 保存规则**: Apply rule list changes.
      **Save Rules / 保存规则**: 保存规则列表变更。
- **Rule Providers (JSON) / Providers (JSON)**: Edit providers JSON config.
      **Rule Providers (JSON) / Providers (JSON)**: 编辑 Providers 的 JSON 配置。
- **Save Providers / 保存 Providers**: Save providers configuration.
      **Save Providers / 保存 Providers**: 保存 Providers 配置。

---

## 12. Android App: WebDAV Sync / Android 应用：WebDAV 同步

Manage WebDAV sync in the **Sync** tab.
在 **Sync** 标签页管理 WebDAV 同步。

- **Enable WebDAV Sync / 启用 WebDAV 同步**: Toggle WebDAV sync on or off.
      **Enable WebDAV Sync / 启用 WebDAV 同步**: 开关 WebDAV 同步。
- **WebDAV URL / WebDAV 地址**: Set the WebDAV endpoint.
      **WebDAV URL / WebDAV 地址**: 设置 WebDAV 地址。
- **Username / 用户名** + **Password / 密码**: Configure credentials.
      **Username / 用户名** + **Password / 密码**: 配置用户名与密码。
- **Sync interval (minutes) / 同步间隔（分钟）**: Set the sync interval.
      **Sync interval (minutes) / 同步间隔（分钟）**: 设置同步间隔。
- **Sync on startup / 启动时同步**: Run sync when the app starts.
      **Sync on startup / 启动时同步**: 应用启动时触发同步。
- **Save / 保存**: Persist WebDAV settings.
      **Save / 保存**: 保存 WebDAV 设置。
- **Test / 连接测试**: Verify the WebDAV connection.
      **Test / 连接测试**: 验证 WebDAV 连接。
- **Sync Now / 立即同步**: Trigger a manual sync.
      **Sync Now / 立即同步**: 手动触发同步。
- **Reload / 重新加载**: Reload WebDAV settings.
      **Reload / 重新加载**: 重新加载 WebDAV 设置。
