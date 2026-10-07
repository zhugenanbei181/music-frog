# Usage Specification / 使用指南

MusicFrog has two native peer desktop products, Iced and Bevy UI. Both control the same Rust product services. The earlier browser management UI has retired; the admin server provides diagnostic APIs only.
MusicFrog 提供 Iced 与 Bevy UI 两个同权原生桌面产品，均使用相同的 Rust 产品服务。旧浏览器管理界面已退役；管理服务只保留诊断 API。

## 1. Launch and core controls / 启动与内核控制

Launch the installed Iced or Bevy UI application. In Overview, use the core control to start or stop mihomo. A stopped core stays stopped until requested. A pending operation disables duplicate submission. If initialization, permissions or the controller fail, read the displayed error and retry after resolving its cause.
打开已安装的 Iced 或 Bevy UI。在概览页启动或停止 mihomo；停止状态不会自行变成已运行。操作等待期间不能重复提交。初始化、权限或控制器失败时，按显示的错误处理原因后重试。

Stopping the core keeps the application and its configuration services available. Proxy and connection observations require an available controller; unavailable data stays unavailable rather than being presented as an empty successful result.
停止内核后应用和配置服务仍可使用。节点与连接观测需要可用的控制器；不可用数据不会被当成成功的空结果。

## 2. Proxy groups and nodes / 代理组与节点

Open Proxies. Expand or collapse a group using its fold control. Select a node using its node card; opening its information control is a separate action. Search, sorting and health filtering affect presentation and keep the kernel's selected node unchanged. Missing health observations remain unknown.
进入代理页，用组折叠控件展开或收起节点。点击节点卡片进行选择；信息按钮只打开详情。搜索、排序和健康筛选只改变显示，保持内核当前选择。缺失健康观测显示未知。搜索支持名称、协议、标签、拼音/地区缩写以及 `<100` 等正延迟条件；匹配名称原位高亮，历史零值不会被当成已确认的低延迟。清除恢复列表，失败保留查询供重试。

Iced retains all sort choices and probe parameters on additional toolbar rows in narrow windows. Scroll the node area to reach remaining cards and diagnostics.
Iced 在窄窗口将完整排序选项与测速参数换行显示。滚动节点区可访问其余卡片与诊断内容。

## 3. Proxy information and a single-node probe / 节点详情与单节点测速

Click a node's information control to open its independent inspector. It shows reported protocol, server, port, cipher, UDP capability, health and latest delay. The history chart preserves controller order; a zero-valued sample has an unresolved outcome and is shown as a gap. Minimum, maximum and average RTT use positive reported samples only.
点击节点信息按钮，打开独立检查面板。面板显示已报告的协议、服务器、端口、加密方式、UDP 能力、健康与最近延迟。历史图按控制器顺序显示；历史零值保留为未判定断点，不能单独判定超时或成功。RTT 最小、最大与平均值只统计正延迟历史采样。

An egress IP is shown only when an explicit speedtest record matches the node and protocol. Its record time identifies a historical observation. It is not the server address or a claim about the current exit. Per-node DNS/TCP/TLS/TTFB stages are unavailable because the kernel does not report them; the application does not infer stages from total delay.
出口 IP 只显示与节点名称、协议匹配的明确测速记录，并附记录时间；它是历史观测，不是服务器地址或当前出口保证。内核未报告节点 DNS/TCP/TLS/TTFB 分段，因此面板明确显示不可观测，不从总延迟推算。

Use Test now to probe the inspected leaf node. A failure keeps the panel open for retry. Close or Escape dismisses it without selecting another node. Refreshes retain the inspected identity; deleting that node closes the panel, and late probe replies cannot reopen it. Scroll inside the panel to read lower metadata and history.
用“立即测速”探测当前叶节点。失败保留面板供重试；关闭或 Esc 不会选择其它节点。刷新继续检查同一节点；节点删除后面板关闭，迟到回执不能重新打开。滚动面板可查看下方元数据与历史。

## 4. Custom nodes and URI preview / 自定义节点与 URI 预览

Use Add node on Proxies. Select a protocol and edit its native form fields. Paste a supported URI to preview its parsed fields before saving. Invalid input remains in the draft with a validation error. Cancel closes the form without committing it. Saving waits for the actual persistence result; failure retains the form and draft.
在代理页选择添加节点，选择协议并编辑原生表单。粘贴支持的 URI 可先预览解析字段再保存。非法输入保留在草稿中并显示校验错误；取消不提交。保存等待真实持久化结果，失败保留表单和草稿。

## 5. Connections / 连接

Iced presents connections in Runtime; Bevy UI has a Connections page. Open a row's details to inspect that stable connection identity. Refresh and reordering keep the same connection selected; disappearance closes the drawer. Missing rates and real measured zero rates are distinct.
Iced 在运行态显示连接，Bevy UI 提供连接页。打开行详情可检查稳定的连接身份；刷新与重排保持同一连接，连接消失后关闭抽屉。缺失速率和真实观测到的零速率分别显示。

Copy host copies the actual destination host through the native clipboard. A missing clipboard service displays an unsupported/error result. Disconnect acts on the inspected connection. Close all opens an independent confirmation: cancel, Escape or navigation closes it without disconnecting; only explicit confirmation submits the destructive operation.
复制主机通过系统剪贴板复制实际目的主机；剪贴板不可用或失败时显示对应结果。断开针对当前检查的连接。断开全部先打开独立确认框：取消、Esc 或导航离开均不执行断开；只有明确确认才提交。

## 6. Speedtest details / 测速明细

Open the speedtest details control to inspect the shared per-node measurements and failures. Metrics without observations remain unknown. The detail panel does not start a probe merely by opening or scrolling it; close or Escape dismisses it.
打开测速明细，查看共享逐节点观测与失败。没有观测的指标保持未知；打开或滚动明细不会自行启动测速，关闭或 Esc 可退出。

## 7. Command palette / 命令面板

Use the command palette shortcut (Ctrl+K by default), type a query, select an available result and execute it. The list includes the shared navigation and action catalogue plus current profiles. Empty results and Escape execute nothing. Destructive actions such as closing all connections still open their confirmation surface.
使用命令面板快捷键（默认 Ctrl+K），输入查询、选择有效结果并执行。列表包含共享导航、操作和当前配置。空结果与 Esc 不执行操作；断开全部等危险操作仍进入独立确认面。

## 8. Subscription filters / 订阅过滤

Open the profile's Filter pane, or the filter workbench on Profiles. Edit include/exclude patterns, excluded protocols, rename rules and duplicate-name strategy. Advanced filter policy accepts a YAML/JSON mapping for country normalization, emoji removal, allowed/blocked ports, private-address filtering, multiplier rules, node mutation, sorting and content deduplication. Both products validate the same policy before saving and return the actual applied-node statistics.
打开配置的过滤页签，或配置页的过滤工作台。可编辑包含/排除正则、协议排除、重命名和重名去重策略。高级过滤策略接受 YAML/JSON 映射，涵盖国家标识规范化、移除表情、允许/阻止端口、私网过滤、倍率规则、节点属性修改、排序和内容去重。两端在保存前使用同一校验，并显示真实过滤统计。

For example, `{drop-private-ip: true, allowed-ports: [443], sort-by: name-asc}` keeps public nodes on port 443 and sorts their names. Use `{}` to clear advanced policies. An empty allowed-port list permits no nodes. Discard restores the last observed settings without saving. A changed source keeps the draft and requires an explicit reload; failures keep the form available for correction or retry.
例如 `{drop-private-ip: true, allowed-ports: [443], sort-by: name-asc}` 保留使用 443 端口的公网节点，并按名称排序。输入 `{}` 清除高级策略；空的允许端口列表不允许任何节点。丢弃恢复最近观测的设置，不保存。来源变化时保留草稿并要求显式重新读取；失败保留表单，可修正或重试。

## 9. Platform availability / 平台可用性

System proxy, TUN/VPN, tray, clipboard and permission features depend on the native host. Read the displayed capability and failure state rather than assuming that another platform's behavior applies. Development demo data is explicitly selected and is not a running proxy controller.
系统代理、TUN/VPN、托盘、剪贴板与权限功能取决于原生宿主。以当前显示的能力和失败状态为准，不从另一平台推断支持。开发演示数据需要显式选择，不代表正在运行的代理控制器。

The following Android sections describe the companion application's own native controls; they do not establish Bevy Android product availability.
以下 Android 章节说明原生伴侣应用控件，不代表 Bevy Android 产品已交付。

---

## 9. Android App: VPN/TUN / Android 应用：VPN/TUN

Manage VPN/TUN parameters in the **Settings > TUN** screen.
在 **Settings > TUN** 页面管理 VPN/TUN 参数。

- **MTU / MTU**: Set the MTU value.
      **MTU / MTU**: 设置 MTU 数值。
- **Auto Route / 自动路由**: Toggle default routing through the VPN.
      **Auto Route / 自动路由**: 开关默认经由 VPN 的路由。
- **Stack / 协议栈**: Select `Auto`, `system`, or `gvisor`.
      **Stack / 协议栈**: 选择 `Auto`、`system` 或 `gvisor`。
- **Strict Route / 严格路由**: Toggle strict routing behavior (Android uses route settings as available).
      **Strict Route / 严格路由**: 开关严格路由行为（Android 以现有路由设置为准）。
- **Auto Detect Interface / 自动检测网卡**: Toggle automatic outbound interface detection.
      **Auto Detect Interface / 自动检测网卡**: 开关自动检测出口网卡。
- **IPv6 / IPv6**: Enable or disable IPv6 routing for VPN.
      **IPv6 / IPv6**: 开关 VPN 的 IPv6 路由。
- **DNS Servers (one per line) / DNS Servers（每行一个）**: Enter DNS server IPs, one per line.
      **DNS Servers (one per line) / DNS Servers（每行一个）**: 逐行填写 DNS 服务器 IP。
- **Save / 保存**: Apply VPN/TUN settings.
      **Save / 保存**: 应用 VPN/TUN 设置。
- **Reload / 重新加载**: Reload current settings from the active profile.
      **Reload / 重新加载**: 从当前配置重新加载设置。

---

## 10. Android App: DNS / Android 应用：DNS

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
- **Fallback Filter GeoIP / Fallback 过滤 GeoIP**: Set `Auto`, `Enabled`, or `Disabled`.
      **Fallback Filter GeoIP / Fallback 过滤 GeoIP**: 选择 `Auto`、`启用` 或 `禁用`。
- **Fallback Filter GeoIP Code / Fallback 过滤 GeoIP 代码**: Set optional country/region code.
      **Fallback Filter GeoIP Code / Fallback 过滤 GeoIP 代码**: 设置可选国家/地区代码。
- **Fallback Filter IPCIDR / Fallback 过滤 IPCIDR**: Enter CIDR list, one per line.
      **Fallback Filter IPCIDR / Fallback 过滤 IPCIDR**: 逐行填写 CIDR 列表。
- **Fallback Filter Domain / Fallback 过滤域名**: Enter full domains, one per line.
      **Fallback Filter Domain / Fallback 过滤域名**: 逐行填写完整域名。
- **Fallback Filter Domain Suffix / Fallback 过滤域名后缀**: Enter suffix list, one per line.
      **Fallback Filter Domain Suffix / Fallback 过滤域名后缀**: 逐行填写域名后缀列表。
- **Save / 保存**: Apply DNS settings.
      **Save / 保存**: 应用 DNS 设置。
- **Reload / 重新加载**: Reload current DNS settings.
      **Reload / 重新加载**: 重新加载 DNS 设置。

---

## 11. Android App: Fake-IP / Android 应用：Fake-IP

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

## 12. Android App: Rules / Android 应用：规则

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

## 13. Android App: WebDAV Sync / Android 应用：WebDAV 同步

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

---

## 14. Android App: Profiles / Android 应用：配置管理

Manage profiles in the **Profiles** tab.
在 **Profiles** 标签页管理配置。

- **Add Profile / 添加配置**: Tap the `+` floating button, enter **Name** and **URL**, then tap **Add**.
      **Add Profile / 添加配置**: 点击右下角 `+` 浮动按钮，填写 **名称** 与 **URL** 后点击 **添加**。
- **Import Local / 本地导入**: Tap the upload floating button, choose a local file, review content, then tap **Save**.
      **Import Local / 本地导入**: 点击上传浮动按钮，选择本地文件，确认内容后点击 **保存**。
- **Edit / 编辑**: Tap row **Edit** to open profile content editor, then tap **Save**.
      **Edit / 编辑**: 点击行内 **编辑** 打开配置内容编辑器，完成后点击 **保存**。
- **Update Now / 立即更新**: Tap row **Update Now** to refresh subscription content.
      **Update Now / 立即更新**: 点击行内 **立即更新** 拉取订阅最新内容。
- **Subscription Settings / 订阅设置**: Tap row **Subscription Settings** to update URL, auto-update switch, and interval.
      **Subscription Settings / 订阅设置**: 点击行内 **订阅设置**，更新 URL、自动更新开关与间隔。
- **Delete / 删除**: Tap row **Delete**, then confirm in the dialog.
      **Delete / 删除**: 点击行内 **删除**，并在确认框中确认。

---

## 15. Android App: App Routing / Android 应用：应用路由

Manage per-app routing in **Settings > App Routing**.
在 **设置 > App Routing** 页面管理分应用路由。

- **Routing Mode / 路由模式**: Select **Proxy All Apps**, **Proxy Selected (Allowlist)**, or **Bypass Selected (Blocklist)**.
      **Routing Mode / 路由模式**: 选择 **代理所有应用**、**仅代理选中（白名单）** 或 **仅绕过选中（黑名单）**。
- **Search Apps / 搜索应用**: Use the search box to filter app list.
      **Search Apps / 搜索应用**: 使用搜索框过滤应用列表。
- **User Apps / System Apps / 用户应用 / 系统应用**: Switch tabs and toggle app switches to include/exclude apps based on mode.
      **User Apps / System Apps / 用户应用 / 系统应用**: 切换标签页并使用开关按当前模式选择应用。

---

## 16. Android App: Connections / Android 应用：连接管理

Manage runtime connections in **Settings > Connections**.
在 **设置 > 连接管理** 页面管理运行时连接。

- **Host Filter / 主机过滤** + **Process Filter / 进程过滤**: Filter active connections by host/process path.
      **Host Filter / 主机过滤** + **Process Filter / 进程过滤**: 按主机或进程路径过滤活动连接。
- **Refresh / 刷新**: Reload current active connection list.
      **Refresh / 刷新**: 重新加载当前活动连接列表。
- **Disconnect / 断开**: Disconnect a single connection from the row action.
      **Disconnect / 断开**: 通过行内操作断开单条连接。
- **Close All / 全部断开**: Disconnect all active connections at once.
      **Close All / 全部断开**: 一次性断开所有活动连接。

---

## 17. Android App: Overview Mode / Android 应用：概览模式

Switch proxy mode in the **Overview** page with **Change Mode**.
在 **概览** 页面通过 **切换模式** 修改代理模式。

- **Rule / Global / Direct / Script**: Select any mode from the dropdown and apply immediately.
      **Rule / Global / Direct / Script**: 在下拉菜单中选择模式并立即生效。

### Hosts 静态映射

在 DNS 页选择 Hosts 编辑，逐行添加、编辑或删除地址与域名。行输入先确认到草稿，应用后才写入当前配置；取消编辑不会保存。若写入失败，草稿和最后已应用映射保留，可在修复权限后重试。读取失败或配置已经变化时，应用会停用；取消草稿后重新检查当前映射。

映射写入 mihomo 顶层 `hosts`。历史误写到 `dns.hosts` 的数据会单独提示，必须选择预览迁移，再应用才会移除旧键。清空全部行并应用会删除顶层映射。

### DNS 缓存清理

DNS 页或命令面板的缓存清理会先打开确认框。取消、Esc 或离开页面均不会执行清理；确认后才请求内核 Fake-IP 映射与系统解析缓存两个目标。执行期间关闭会停用，结果按目标显示，部分失败不会冒充全部成功。权限问题修复后可在结果面板重试；不支持的目标会保留明确原因。该报告不依赖 DNS 配置页面读取成功。


## DNS 查询详情

Iced 与 Bevy DNS 页面均提供独立查询操作面。输入 DNS 名称（国际化名称使用 punycode），选择 A、AAAA、CNAME、MX、NS、TXT、SOA、PTR、SRV、CAA、HTTPS、SVCB 或 ANY，再执行查询。结果按答案、权威、附加三段查看；分页保留全部记录，并显示实际问题、DNS 状态码、响应标志、记录类型、TTL 与原始数据。

执行前取消不发起查询；进行中的查询禁止重复提交或改写草稿。读取配置失败不遮蔽独立结果。控制器失败保留上次已观测响应，明确区分旧结果与本次查询；权限和认证失败可导向设置，再手动重试。无适配器、错误响应问题或格式异常不显示伪造记录。
