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

## 截图取证、测试矩阵与组件树架构

1. **后台无感实机截图取证（Capture Matrix）**：
   - 支持通过 `--demo` 命令行标志或 `INFILTRATOR_DEMO=1`、`INFILTRATOR_PAGE`、`INFILTRATOR_SKIN`、`INFILTRATOR_WINDOW_SIZE` 环境变量激活演示与截图模式；
   - 窗口挂载与稳定首帧绘制后向 `INFILTRATOR_CAPTURE_MARKER` 写入标准 `CAPTURE_READY page=<p> skin=<s>\n` 标记；
   - 配合 `scripts/capture-slint-matrix.sh` 与 `scripts/capture_slint_scenarios.tsv`，在私有虚拟 Wayland 合成器（KWin + Niri IPC）内实现无弹窗、不夺焦点、绑定二进制与源码指纹的自动化像素级取证。

2. **行为测试矩阵覆盖（49 项全部通过）**：
   - `core_and_network_mode_tests`：覆盖 Core 生命周期枚举、多命令作用域传递与错误拦截、路由模式与网络模式切换（系统代理/TUN/仅核心切换时的停机重启时序）；
   - `outbound_and_probe_tests`：覆盖代理组节点选择的 revision 强校验、单节点测速与整组批量测速（concurrency=8）意图、排序与搜索偏好分发；
   - `connections_and_logs_tests`：覆盖指定连接中断命令生成、四维连接排序、日志级别与别名过滤、发布窗口截断多语言告警；
   - `rules_and_settings_tests`：覆盖规则开关幂等分发、Rule Provider 批量刷新、语言偏好解析、主题模式白名单与十六进制强调色合法性校验及反差度计算；
   - `platform_adapter_tests` / `profile_catalog_action_tests` / `age_credential_tests` / `traffic_history_tests`：覆盖平台权限、窗口生命周期、配置管理与敏感凭据。

3. **Lumen 组件树模块化与质量规范**：
   - 彻底打破单文件巨石结构，建立清晰的分层模块（每个文件严格 $\le 600$ 行）：
     - `ui/components/theme.slint`：全局 `LumenTheme` 主题系统与语义颜色；
     - `ui/components/models.slint`：跨端结构体定义；
     - `ui/components/base_widgets.slint`：基础交互原子组件（按钮、开关、滑块、输入框等）；
     - `ui/components/cards.slint`：领域卡片与容器；
     - `ui/components/sidebar.slint`：桌面侧边导航栏；
     - `ui/components/profile_sheets.slint` 与 `profile_overlays.slint`：配置添加、操作、分享、删除与设置抽屉；
     - `ui/components/settings_sheets.slint` 与 `settings_overlays.slint`：配色、缩放、User-Agent 抽屉与网络接管切换确认弹窗；
     - `ui/components/app_chrome.slint`：移动端悬浮底栏、清空流量弹窗、通知/消息气泡与节点排序菜单；
     - `ui/components/app_body.slint`：桌面/移动端弹性主体容器与滚动锚定；
     - `ui/pages/page_*.slint` 与 `pages_*.slint`：分领域页面与路由集群；
     - `ui/lumen-app.slint`：轻量级主页面根骨架（从 3587 行解耦并缩减至 591 行）；
   - 所有触控交互控件均遵循移动端 $\ge 48\text{px} \times \text{LumenUiScale.factor}$ 最小触控热区规范；
   - 由 `scripts/quality/slint-syntax-guard.py` 门禁执行单文件 $\le 600$ 行预算与严禁硬编码状态色约束（100% 通过）。

