# Slint 表面契约

> 层级：L3

`infiltrator-slint` 是 MusicFrog 的第三套 UI。`ui/lumen-app.slint` 导出 Rust adapter 使用的 `AppWindow` 与 Lumen 组件树；Rust adapter 负责把 `SurfaceSnapshot` 投影到控件，把用户输入转成共享 `CommandIntent`，并把 owner 的 typed failure 显示出来。Slint 不创建 Core、不持有独立业务仓库，也不在 Android 侧创建第二份 application。

桌面由 `DesktopProductSession` 组合既有 application、Core 与系统 adapter。Android flavor 通过 `infiltrator-android` 的产品 IPC 提交相同 `CommandIntent`，快照由 Android 唯一 owner 发布；NativeActivity 只初始化 Slint renderer 并挂载页面。

同一组件根据可用宽度响应式切换：Android 恒用 compact 布局，桌面窗口低于 `760px × UI scale` 也切换为紧凑导航和单列内容；较宽桌面保留侧边导航和更宽的内容区。控件尺寸随页面缩放因子调整，旗帜、图标、窗口图标、字体及其 license provenance 由共享资源目录提供。布局差异只属于呈现，不改变命令语义。

页面读取 owner 快照：

- 首页投影 Core 生命周期和错误、当前 Profile、可验证的活动出口与国家旗帜、公共 IP 探测结果、连接数和实时流量。实时速率用 `infiltrator-presentation` 的共享 projector；未知、未观测和失败状态不被填成演示值。
- 代理页展示 typed outbound inventory/details，支持组内选择、搜索、排序、单节点测速和整组测速。选择与探测请求携带 owner 快照 revision，避免把过期列表提交给 Core。
- 配置页读取共享 Profile 列表，可切换、刷新、删除订阅并导入 URL。订阅 URL、更新间隔和 User-Agent 使用共享命令；只读预览从 `LoadProfileDocument` 的共享读回显示，并通过 `infiltrator-presentation::profile_preview` 结构化遮蔽凭据字段。Age 凭据读取、设置、清除与加密订阅导入经由共享 Core owner capability / `DesktopProductSession` 执行，密钥仅限输入阶段且自动擦除，不进入 UI state/log；配置重命名、本地文件导入与配置/覆盖/日志导出已接入底层命令与平台文件适配器。
- 网络页提交 Rule/Global/Direct 路由意图和 desktop 系统代理/TUN 或 Android VPN 命令。TUN 开关以宿主 `Capability` 为准；网络模式切换序列会逐条使用 owner 返回的新 Core scope。
- 连接页筛选 owner 发布的活动连接，可查看当前快照中的端点、规则、链路、开始时间和累计字节，也可请求关闭连接。owner 未发布已关闭连接历史，因此“已关闭”切换会被还原并显示原因。
- 日志页由共享 owner 提供日志流和等级过滤，关键字搜索在 Slint adapter 中作用于当前已发布窗口。规则启停使用 owner 发布的 rule index，Provider 更新提交共享 batch 命令。
- 流量页展示共享实时采样；今日/周累计历史和清理入口在 owner 未发布历史存储前显示未知值，不伪造统计。
- 应用设置写入共享语言和 theme preference；Slint 运行时根据同一个 product gettext catalog 切换界面文案。桌面自启动复用共享平台 helper；自定义强调色与启动时刷新配置写入共享 settings 契约。
- 关于/许可证页面读取随应用发布的第三方依赖许可证说明。Lumen 来源信息如实说明上游 README 的 MIT 声明以及导入 ZIP 缺少独立 LICENSE 文件。

共享快照明确发布上限的规则、活动连接和日志窗口会在对应页面显示本地化的已显示数/总数提示；本地搜索只缩小已发布窗口，不声称检索未发布内容。

配置目录排序、命名 Override 绑定、文件导入导出、流量历史清理与强调色设置已全部接入底层 owner 与 typed 命令；仍未接入特定平台能力的入口（如无二维码扫描器的宿主）会返回可见的 typed unsupported 状态，不伪装成功。用户配置名、节点名、日志正文等 runtime data 按原文呈现，不走翻译表。
