# Usage Specification / 使用指南

This document provides instructions for users on how to interact with the **MusicFrog Despicable Infiltrator** interface and its features.
本文档为用户提供如何使用 **MusicFrog Despicable Infiltrator** 界面及其功能的说明。

---

## 1. Tray Menu / 托盘菜单

The tray menu provides quick access to connection info and core controls.
托盘菜单提供连接信息和核心控制的快速访问。

- **Open Browser / 打开浏览器**: Opens the management Web UI in your default browser.
  **打开浏览器**: 在默认浏览器中打开管理 Web UI。
- **System Proxy / 系统代理**: Toggle the system-wide proxy on or off.
  **系统代理**: 切换全局系统代理的开启或关闭。
- **Proxy Mode / 代理模式**: Switch between `Rule`, `Global`, `Direct`, and `Script` modes.
  **代理模式**: 在 `规则`、`全局`、`直连` 和 `脚本` 模式之间切换。
- **Profile Switch / 配置切换**: Quickly switch between different proxy profiles.
  **配置切换**: 在不同的代理配置之间快速切换。
- **Advanced Settings / 高级设置**: Direct links to specific configuration panels (DNS, Tun, Rules, etc.).
  **高级设置**: 直达特定配置面板（DNS、Tun、规则等）的链接。

---

## 2. Profile Management / 配置管理

Manage your subscriptions and local configuration files.
管理您的订阅和本地配置文件。

- **Import Subscription / 导入订阅**:
  - **Name / 名称**: Enter a friendly name for the subscription.
      **名称**: 输入订阅的友好名称。
  - **URL**: Paste the subscription link.
      **URL**: 粘贴订阅链接。
  - **Import Now / 立即导入**: Downloads and saves the subscription.
      **立即导入**: 下载并保存订阅。
- **Import Local / 导入本地**:
  - **Select File / 选择文件**: Choose a `.yaml` or `.toml` file from your disk.
      **选择文件**: 从磁盘选择 `.yaml` 或 `.toml` 文件。
  - **Save / 保存**: Imports the local file into the application.
      **保存**: 将本地文件导入应用。
- **Profiles List / 配置列表**:
  - **Set Active / 设为当前**: Activate the selected profile and restart the core.
      **设为当前**: 激活所选配置并重启内核。
  - **Edit / 编辑**: Opens the built-in editor to modify the profile content.
      **编辑**: 打开内置编辑器修改配置内容。
  - **Delete / 删除**: Removes the profile from the list.
      **删除**: 从列表中移除配置。

---

## 3. Advanced Configuration / 高级配置

Fine-tune your proxy behavior through specialized panels.
通过专门的面板精细调整代理行为。

- **DNS Panel / DNS 面板**:
  - **Enable / 启用**: Toggle the built-in DNS server.
      **启用**: 切换内置 DNS 服务器。
  - **Nameservers / 上游服务器**: List of DNS servers to use.
      **上游服务器**: 要使用的 DNS 服务器列表。
- **Tun Panel / Tun 面板**:
  - **Enable / 启用**: Toggle TUN mode for transparent proxying (Requires Admin).
      **启用**: 切换 TUN 模式以实现透明代理（需要管理员权限）。
  - **Stack / 栈类型**: Select between `gVisor` or `System` network stacks.
      **栈类型**: 在 `gVisor` 或 `System` 网络栈之间选择。
- **Fake-IP Panel / Fake-IP 面板**:
  - **Fake-IP Range / 范围**: Define the IP range for Fake-IP mapping.
      **范围**: 定义 Fake-IP 映射的 IP 范围。
  - **Flush Cache / 清理缓存**: Click to clear stored Fake-IP mappings.
      **清理缓存**: 点击以清除存储的 Fake-IP 映射。
- **Rules Panel / 规则面板**:
  - **Add Rule / 添加规则**: Insert a new routing rule.
      **添加规则**: 插入一条新的路由规则。
  - **Save Rules / 保存规则**: Apply the changes to the rules list.
      **保存规则**: 将更改应用到规则列表。

---

## 4. Sync & Backup / 同步与备份

Keep your configurations in sync across multiple devices using WebDAV.
使用 WebDAV 在多个设备之间保持配置同步。

- **WebDAV Settings / WebDAV 设置**:
  - **Enable / 启用**: Toggle automatic synchronization.
      **启用**: 切换自动同步。
  - **URL / Username / Password**: Your WebDAV server credentials.
      **地址 / 用户名 / 密码**: 您的 WebDAV 服务器凭据。
  - **Sync Now / 立即同步**: Manually trigger a synchronization cycle.
      **立即同步**: 手动触发一次同步循环。
  - **Test Connection / 测试连接**: Verify if the server is accessible.
      **测试连接**: 验证服务器是否可访问。

---

## 5. Core Management / 内核管理

Manage the underlying `mihomo` binary versions.
管理底层的 `mihomo` 二进制版本。

- **Core Panel / 内核面板**:
  - **Activate / 启用**: Switch to a specific downloaded version of the core.
      **启用**: 切换到特定的已下载内核版本。
  - **Update to Stable / 更新到稳定版**: Download the latest stable core version.
      **更新到稳定版**: 下载最新的稳定版内核。
  - **Default Core / 默认内核**: Toggle between the bundled core and downloaded versions.
      **默认内核**: 在捆绑内核和下载版本之间切换。
