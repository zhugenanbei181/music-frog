# 功能域地图

双端平权证据的 owner 是 `infiltrator-contract::parity::FeatureId`（场景身份与分支要求）和 `scripts/parity/`（声明、真实测试发现、像素回执校验）。两端的交互由各自 UI crate 实现；验收规则见 [UI_PARITY_AUDIT.md](UI_PARITY_AUDIT.md)。

本表按用户功能而不是按目录排列。目录是实现位置，功能域才是协作和 TODO 的归属单位；一个功能域只能有一个 Rust 逻辑 owner，Iced 与 Bevy UI 双端作为对等消费者同步呈现。

| 功能域 | 用户意图 | 当前主要 owner | UI / 宿主入口 | 重整重点 |
| --- | --- | --- | --- | --- |
| Core 生命周期 | 启动、停止、重启、状态检查、健康探测 | `infiltrator-application::CoreApplication` + `infiltrator-ports`；具体进程在 host/composition | Iced、Bevy UI、Android `MihomoHost` | CORE-001/002：统一 session、readiness、generation |
| Bevy 页面装配与实体生命周期 | 一次接线、受限 ECS、路由恢复草稿与原生控件初始化 | `pages::plugin::PageBindingsPlugin` 组合各页面插件；控件层 Observer 与调度负责原生状态 | Bevy UI 的 `PagesPlugin` 与 `WidgetsPlugin` | 页面根不持有 World/hook；不靠重挂载重置业务资源 |
| 独立桌面产品宿主 | 初始化原生端口、配置、命令服务与全页读取；退出回收宿主 | `infiltrator-desktop::product::DesktopProductSession`；命令组合 `composition::desktop_command_application` | Iced / Bevy 默认桌面入口；显式演示模式使用 fixture | 命令/读取共享引擎，启动失败不得退回演示数据 |
| Android 移动产品与进程 IPC | UI/VPN 分进程、无 Activity 初始化、命令/结果/快照绑定与重连 | 产品组合唯一 owner 为 `infiltrator-android::composition`；Android 原生 IPC adapter 承载同一 application 边界 | Compose / Bevy Activity 与 Kotlin `MihomoVpnService` | `BANDROID-001`/`BANDROID-004`；规范见 [BEVY_ANDROID_PRODUCT.md](BEVY_ANDROID_PRODUCT.md)，跨进程不共享静态状态 |
| Android VPN 生命周期与网络资源 | 授权、前台、TUN FD、独立内核、无回环、重启和撤销 | `infiltrator-application::vpn_application::VpnServiceApplication` 为语义 owner；`infiltrator-android::vpn_service` 与 Kotlin service 实现端口 | Compose / Bevy 共享 VPN 命令和通知操作 | `BANDROID-002`；保留 tun2proxy 路线，服务资源不归 Activity |
| Android 窗口与原生输入宿主 | Activity/Surface 生命周期、Insets、IME、返回与系统剪贴板 | `infiltrator-android` 原生窗口 adapter 为宿主 owner；中性事件复用既有契约，Bevy 消费者保留 | Bevy Android 原生 Activity；Compose 使用同一宿主能力与业务意图 | `BANDROID-003`/`BANDROID-005`/`BANDROID-006`/`BANDROID-007`/`BANDROID-008`/`BANDROID-014`；原生接线与合成事件分别验收 |
| Bevy 业务列表实体回收 | 完整集合的可见窗口、行槽/网格复用与稳定对象操作 | `infiltrator-bevy-widgets::list` 为几何/回收算法 owner；页面仅映射共享业务身份 | Proxies、Connections、Logs、Rules 及既有原生滚动 | `BANDROID-009`/`BANDROID-010`；不截断业务集合，Iced 等价可达性继续验收 |
| 渲染调步与后台观察唤醒 | 交互/动画/静置/后台/挂起、可见页订阅、有界结果和外部唤醒 | `contract::cadence` 为调步语义 owner；`application::surface_application` 为全页观察 owner，宿主 adapter 管事件循环唤醒 | Iced / Bevy 调步消费者；Android 服务与 UI 观察分离 | `BANDROID-011`/`BANDROID-012`；真实窗口策略与数据线程接线不能靠模式名证明 |
| 遥测图形保留与增强 | 二维波形/拓扑缓存、GPU 波形/Bloom、可选地球与真实地理来源 | `infiltrator-bevy-widgets::chart` 为绘制 owner；样本仍归既有 waveform owner，地理观测扩展归 `application::traffic_topology_application` | Iced 等价数据与操作面；Bevy 现有二维路径及可选增强 | `BANDROID-013`/`BANDROID-016`；高级效果按需启用并保留兼容回退 |
| Bevy 卡片 shader 实际接线 | 模块化 WESL、Squircle GPU 材质、尺寸/主题同步、平面回退与静态缓存 | `infiltrator-bevy-widgets::shader_fx::ModernSurfacePlugin`；受限同步与材质 owner 归 `surface_shader`，模块加载归 `shader_assets` | 桌面/Android 共用 Bevy 原生入口与既有 SurfacePanel | `BANDROID-017`；前端章程 §1.2 规定 ABI/handle/回收；插件/类型存在不代表真实渲染 |
| Bevy 图表纹理生命周期 | 波形/环图/直方图/二维拓扑原位更新、外部资产隔离、绑定与缺失修复、绘制角色退役 | `infiltrator-bevy-widgets::chart::texture` 为可写纹理 owner；各 chart 系统决定失效与绘制 | WidgetsPlugin 一次注册，真实 ImageNode 与强 handle 回收 | `BANDROID-013`；资产身份和基线回归不能替代设备 GPU 内存实测 |
| Android 产品交付与资源证据 | 默认特性 APK、双 ABI、真实控制/VPN、后台与功耗 | `scripts/build-bevy-apk.sh` 为 Bevy 产物入口，复用 Android 产品组合与验证脚本；状态唯一来源 `PLATFORM_MATRIX` | Compose / Bevy 实际包与模拟器、ARM64 真机 | `BANDROID-015`；库交叉检查不替代最终产物与设备 |
| Android CI 与自动化测试 | 编译门、清单/权限守卫、Kotlin 单元、模拟器插桩、真机证据分层 | 编译门 owner 为 `.github/workflows/android.yml` + `scripts/android-check.sh`；清单/权限 owner 为 `scripts/quality/android-manifest-guard.py`；真机 stage 归 `workflow_dispatch`/self-hosted | CI、模拟器、ARM64 真机 | `BANDROID-018`～`BANDROID-023`；库检查/无头/模拟器不得冒充设备证据 |
| 内核随包交付 | 按架构校验、打包、首次启动与更新运行副本 | 锁定资产 `packaging/mihomo-assets.json`；下载 `scripts/fetch-mihomo.py`；查找与副本 `infiltrator-desktop::bundled_kernel` | Iced / Bevy 安装包与共享宿主启动 | 双重 digest、未知资产拒绝、内容身份、损坏缓存修复 |
| Profile 与订阅 | 导入、编辑、删除、切换、订阅更新、聚合 | `infiltrator-domain::{profiles,subscription}` + `infiltrator-application::ProfileApplication`；文件/HTTP 在 core adapter | Iced、Bevy UI、Android、CLI | FUNC-001：统一 profile 命令和重建结果 |
| 配置与选项来源事务 | 一致读取两份文档、字节来源校验、文件发布与内核失败恢复 | `contract::profile_source`；字节哈希唯一 owner `domain::profile_source`；`ports::{profile_store,profile_workspace,runtime_gateway}`；持久化 `mihomo-config::profile_workspace_store`，选项 I/O `profile_option_store`；运行应用 `core::apply_workspace`，宿主 `desktop::runtime::ports`，过滤结果 `contract::subscription_filter_result`，命令输出 `contract::command_output` | 共享用例与宿主 adapter；UI 只消费来源契约 | 同目录 manager 共用进程内写入边界；不存在与空文件区分；活动/非活动身份在提交边界校验；回滚恢复原字节且不覆盖后来编辑；结果携带实际提交来源 |
| 聚合预览显示 | 实际清理计数、缺失源、区域、策略组、模板及 YAML 预览 | `aggregation_preview_projection` 消费真实 `AggregationReport`，两端共享 60 行 YAML 范围与文案折叠 | Iced 聚合模态、Bevy 聚合模态 | 不重算聚合、不合成计数；身份和用户文本保持原样，语言切换不替换草稿控件 |
| 外壳真实读数 | 导航计数、活动配置、配额与已观测速率 | `shell_readout_application` 保留真实观测并折叠；`shell_readout_projection` 唯一文案 | 全页 reader 发布 immutable `shell_readout`，Iced/Bevy 原位回放 | 未知不冒充零，读失败保留失效值，配置切换不复用另一配置配额 |
| 原生实时速率与曲线 | 未观测、真实零、读取失败、同会话保留与来源退役 | `shell_readout_application` 唯一观测折叠；`traffic_readout_projection` 统一峰值与状态文案；`traffic_waveform_application` / domain `traffic_waveform` 拥有样本窗口 | Iced Runtime/Overview 与 Bevy Overview 读取共享结果 | 新代次或 token 清除旧样本；空/单样本不回退私有历史；旧私有流消息不能覆盖组合根共享读数 |
| 模式显示文案 | 规则/全局/直连/脚本模式标签 | `proxy_mode_projection` 统一类型到共享文案 key；状态事实由 `ProxyModeApplication` 与真实 reader 提供 | 两端侧栏，Bevy 原生本地化 pill | 文案翻译不生成模式观测；语言变更保留按钮实体与无障碍身份 |
| 内核生命周期与来源文案 | 回放完整生命周期、已报告版本与演示/实时来源 | `core_status_projection` 统一生命周期与来源文案；原始事实归共享 Core 快照 | Iced Overview 与 Bevy Overview/侧栏原位回放 | 不把开始/停止中折为失败，不把未知版本补成“读取中”；语言变化保留原生实体并更新无障碍标签 |
| 模式切换终态 | 切换、等待、失败重试、关闭错误与过期回执拒绝 | `proxy_mode_actions` 唯一动作状态；`CoreApplication` 串行命令核验 `OverviewReader` 实际读回；`ProxyModeApplication` 唯一命令终态与已验证模式折叠；运行网关写入归 `RuntimeQueryApplication`；失败文案归 `proxy_mode_projection` | Iced TEA / Bevy 受限 ECS 原生错误操作面 | 请求身份与代次关联；读写隔离；健康轮询保留写入错误；不制造模式或生命周期成功；`shell-proxy-mode-control` 单独绑定真实双端证据 |
| 运行控制观测 | 模式、脚本、TUN、IPv6、LAN 安全与日志级别读取状态 | 中性 `runtime_control` 契约；`RuntimeControlApplication` 唯一保留观测与代次；设置页回放同一折叠，`ProxyModeApplication` / `SystemToggleApplication` 统一动作策略；解码归 `mihomo-api::types` | 全页 reader、双端外壳/模式与设置控件 | 字段缺失与明确 false/空/零分开；偏好成功不代表控制器成功，失败保留旧值与类别并阻断过期动作；写入缺失读回不报成功 |
| 语言偏好与本地化 | 系统/简体中文/英语选择、真实保存与失败恢复、原位文案回放 | 中性 `language` 契约、`language_choice` 状态与 `SettingsApplication` 持久 owner；翻译单一来源 `infiltrator-shared::locales` | Iced / Bevy 原生语言控件 | 保存成功才应用；过期回执和旧 reader 不覆盖新选择；不重建输入实体或丢失草稿/焦点 |
| 设置偏好事实 | 托盘与通知的真实开关、未知和失效观测 | `surface_reader` 发布实际设置，`settings_preference_projection` 唯一状态折叠；`SettingsApplication` 持久写入 | 双端设置原生控件 | 缺失不默认开启，命令使用明确布尔值；入队不冒充读回 |
| 设置诊断状态文案 | 完整性、认证、版本、服务模式、端口冲突、资源、MTU、离线预检的共享折叠 | `settings_status_projection` 消费中性观测，翻译使用共享 locales | Iced 设置与 Bevy 设置原位回放 | 缺失观测保持未知；真实零值、已应用 MTU 和实际内存上限不混淆；故障文案统一脱敏 |
| 宿主网络状态文案 | 漫游接口/网关/MTU/事件与系统代理归属/恢复状态 | `network_status_projection` 唯一语义与本地化折叠；事实仍归 `NetworkRoamingApplication` / `SystemProxyApplication`；共享 locales 提供文案 | Iced / Bevy 设置与网络诊断 | 语言变更不重建控件；未知不补零或“活跃”；保留实际错误、原始接口身份与事件详情 |
| 代理与路由 | 代理模式、代理组/节点切换、延迟测速 | `infiltrator-application::ProxyApplication` + `RuntimeGateway`；wire 实现在 `mihomo-api` | Iced Proxies、Bevy Proxies、Android Proxies | FUNC-002：共享节点身份、测速状态和排序语义 |
| 控制器观测与失败边界 | 解码真实运行态列表/单节点、保留缺失字段、拒绝失败 HTTP 回执 | domain `proxy_observation`；wire `mihomo-api::runtime_proxy`；错误映射 `runtime_gateway` | 双端共享 reader、CLI 与原生宿主 | 配置模型保持严格；不合成服务器/端口/密钥；认证/权限/参数失败与网络恢复区分 |
| 延迟呈现与来源 | 区分真实测量零、历史未判定零和未观测 | 契约 `latency_display`；唯一折叠 `latency_projection` | Iced / Bevy 原生延迟标签与颜色语义 | 历史零值不能判定超时；正延迟统计明确限定样本范围；DNS 测量零保留为真实零 |
| 内核启停与忙态 | 从真实生命周期/能力投影启停、重试、失败与待确认状态 | `core_control_projection`，共享语义 `contract::core_control`；Iced `shared_lifecycle` / Bevy `overview_lifecycle` | 两端原生启停控件 | 完整失败身份、单请求关联、真实状态回放，停止后保留宿主 |
| 脚本产品工作台与审阅导出 | 操作关联、会话隔离、连续熔断、先审阅后写入 | `ScriptApplication` / `ScriptExportApplication` 会话 owner；`script_run` / `script_export_review` 中性命令结果；`ScriptWorkbench` 共享交互状态；`script_console_projection` / `script_export_projection` 唯一展示折叠；原生 Bevy owner `profiles_script_workbench` / `profiles_script_view` / `profiles_script_scene`，控件 `multiline_editor` | Iced TEA 与 Bevy ECS 通过同一 Core 命令服务；reader 注入同一组 owner | 原生全分支与双视口像素仍需专用场景验收，不能读取其它产品进程缓存 |
| 脚本执行时间与预算 | 默认真实单调时钟；行为测试显式驱动相同转换与超时边界 | `infiltrator-domain::script_engine_runtime`；application 通过 `ScriptEnginePort` 消费结果 | 双端共享脚本快照 | 生产 500ms 预算不变；500ms/501ms 边界用虚拟时间验证 |
| 代理事实与控件身份 | 展开、排序、动态增删、原位更新节点与组 | 唯一事实/偏好折叠 `proxy_projection`，偏好状态 `proxy_preferences_application`；Bevy `proxies_identity` / `proxies_refresh` / `proxies_reconcile` | 原生代理卡片、组控件与共享 reader | 重排不改身份，删除拒绝旧操作，存活控件不重建 |
| 代理搜索与高亮 | 编辑查询、清除、空态、失败与重试 | `proxy_search_projection` 唯一匹配/文本片段折叠，`proxy_preferences_application` 管理查询；契约 `proxy_search` | Iced TEA 搜索输入 / Bevy 原生 TextField | 不改当前选择，未知零值不进入正延迟条件；保留焦点/草稿并拒绝旧回执；像素验收同时检查输入与结果可见 |
| 代理组顺序编辑 | 完整顺序草稿、上下移动、重置、应用与取消 | 中性 `proxy_group_order_editor`；真实命令 `CommandApplication::apply_group_order`，共享偏好 `proxy_preferences_application` | Iced / Bevy 独立组顺序编辑模态 | 草稿不修改共享顺序；只提交完整唯一身份列表，生产写入前核对实际内核组集合，失败保留编辑面 |
| 延迟探测参数 | 编辑 URL 与超时、校验、应用、取消和失败恢复 | 中性契约 `proxy_probe_options`，唯一验证 `proxy_probe_options_projection`，中性草稿状态 `proxy_probe_editor`，持久 owner `SettingsApplication`；实际执行 `CommandApplication` / `SpeedtestApplication` | Iced / Bevy 原生参数编辑与节点/组探测 | 草稿不替代已应用参数；所有路径校验相同范围，明确回放真实终态，不使用固定展示 URL |
| 节点地区提示与收藏 | 从名称提取地区缩写、原生收藏点击与当前状态回放 | 地区 `infiltrator-shared::country_flags`；收藏状态 `proxy_preferences_application`；Bevy `NodePinButton` 的原生 required components | 双端节点列表 | 缩写必须有词边界；地区提示不等于实测出口；收藏点击不能选择节点，删除后拒绝旧控件 |
| 代理详情检查 | 按稳定节点身份查看真实元数据、健康、延迟历史/统计和已记录出口；单节点探测 | 契约 `proxy_inspection`；唯一折叠 `proxy_inspection_projection`；探测 `CommandApplication` → `RuntimeGateway::test_delay` | Iced / Bevy 独立检查表面 | 不触发选择，不伪造分段耗时；出口附观测时间，未知明确呈现；关闭拒绝旧回执，失败保留面板供重试 |
| 自定义节点表单 | 全协议字段、URI 预览、草稿验证与取消 | `infiltrator-contract::protocol_form`；唯一字段折叠 `infiltrator-application::protocol_form` 与 `protocol_codec_application` | Iced / Bevy 独立模态 | 字段穷尽、保留失败输入、保存失败留在编辑面板 |
| 命令面板语义 | 检索、选择、执行共享命令与实时配置目录 | `infiltrator-contract::command_catalogue`；折叠 `infiltrator-application::command_palette_projection` | Iced / Bevy 全局命令面板 | 共享标题与过滤顺序；全量可达；内容变化触发重放 |
| 测速明细投影 | 查看逐节点指标、失败与宿主能力 | `infiltrator-application::speedtest_detail_projection` / `speedtest_summary_projection`；契约 `speedtest_details` | Iced 明细模态、Bevy 明细模态 | 唯一指标/摘要/历史折叠，未测丢包不生成评级或星级；等价关闭流程、双视口实证 |
| 连接详情事实 | 检查连接、复制目的主机、读取真实速率与内核归属 | `infiltrator-application::connection_rate_application::project_connection`；契约 `surface_snapshot::ConnectionSnapshot`；归属显示 `connection_detail_projection`；domain `ConnectionView` 实现 | Iced / Bevy 连接抽屉 | 端点显示与复制目标分离；未知采样与真实零值分离；耗时明细 typed Unsupported |
| 连接搜索与高亮 | 查询、原始文本高亮、结果数、空态、清空、刷新和语言/路由恢复 | `application::connection_search` / `search_text` 唯一匹配文本折叠；`ConnectionGroupingState` 缓存事实与查询；`contract::search_text::SearchTextRun` 为中性文字片段 | Iced 原生富文本与输入 / Bevy 原生 TextSpan 与输入 | 保留原始 UTF-8 与完整字形簇，元数据命中明确显示；读取不可用保留旧观测并禁用依赖当前事实的断开操作，不当作空结果 |
| 连接分组操作面 | 按进程或目标分组、搜索、刷新与返回平铺 | `application::connection_grouping::ConnectionGroupingState` 唯一缓存折叠，domain `connection_view` 定义聚合/匹配/排序语义 | Iced 分组卡片 / Bevy `connections_groups` 全列表，`connections_rows` 管理结构生命周期 | 不截断为六组摘要；完整数量与流量同义，语言变化保留实体，交互不提交控制命令 |
| 字节显示折叠 | 显示流量总量、内存和字节速率 | `infiltrator-application::byte_format::format_bytes` | Iced / Bevy 各消费表面 | 单一单位与精度；UI 直接导入定义，无转发 formatter |
| 日志滚屏与导出 | 暂停/恢复跟随、历史滚动与实际脱敏文件结果 | 滚屏状态归 `application::log_follow`；日志事实归 `log_application`，导出通过宿主端口与类型化结果闭合 | Iced / Bevy 原生滚动与导出操作面 | 锁定不停止接收事实；内容增长不误判为人工回滚；取消与失败不得报告文件已导出 |
| 控制器实时日志 | 会话隔离的日志缓冲、级别解析、故障保留、清空和级别筛选 | `application::log_application` 唯一缓冲，`log_projection` 唯一 parser，`log_search::LogSearchState` 唯一正则/字形簇高亮折叠；`log_stream` 由 composition 注入执行器与 gateway；中性级别与流状态归 `contract::logs` | Iced Runtime 共享快照 / Bevy Logs 动态行 | 代次和 token 双重隔离；失败与停止保留同代真实记录；未知不补 INFO；原生行数随实际快照增长或退役，关闭释放流驱动 |
| 日志脱敏导出 | 固定缓冲快照、确认、取消、真实保存回执与重试 | `application::log_export_application` 准备和保存 owner；`domain::log_export` 唯一脱敏字节投影；`application::log_export_actions` / `log_export_projection` 共享交互语义；`ports::log_export` 由宿主注入 | Iced Runtime / Bevy Logs 的独立原生导出模态 | 准备不写文件；确认只传快照身份；不受查看器筛选和后续日志增长影响；重启同步退役旧来源；无宿主返回 typed unsupported；原生确认、取消、待保存、失败、重试与不支持共享同一状态机；具体像素范围与平台交付只回指统一场景证据 |
| 运行态诊断 | connections、logs、traffic、memory、出口 IP | domain projection + `RuntimeGateway` / `NetworkApplication`；HTTP 在 outbound adapter | Iced Runtime、Bevy Runtime、Android Connections/Overview | FUNC-003：快照/流式数据有界，双端同语义 |
| DNS / Fake-IP / TUN | 读取、校验、保存、清缓存、VPN/TUN 控制 | domain schema + `ConfigurationApplication`；Fake-IP cache/TUN 在 typed host port | Iced DNS、Bevy DNS、Android Settings/VPN | FUNC-004：字段能力矩阵和配置事务 |
| DNS 表单展示 | 18 项中性字段标签、校验问题与排队/缺服务文案 | `application::dns_status_projection`；字段/校验事实来自 `contract::dns_form`，文案来自 shared network 本地化表 | Iced 与 Bevy 原生字段共用一次插值，Bevy 在语言变化时原位回放 | 服务缺失不登记为已提交；原始输入中的占位符不能再解释为模板 |
| DNS 观测展示 | Fake-IP 搜索、逐上游延迟、健康状态与建议操作、STUN 观测比较 | `application::dns_mapping_projection` / `dns_latency_projection` / `dns_health_projection` / `stun_projection` 唯一折叠 | Iced / Bevy 原生展示读取同一结果 | 未观测、不支持、读取不可用与搜索无匹配分开；真实零延迟不判失败，原始身份与原因只插值一次 |
| DNS 查询详情 | 名称/记录类型查询、答案/权威/附加段及失败重试 | `contract::dns_query` 中性请求/响应；`application::dns_query_application` 唯一命令与报告 owner；`ports::dns_query` 限定控制器查询能力 | Iced / Bevy 独立原生查询与结果模态 | 真实控制器响应逐字段保留；配置页失败不能遮蔽结果；取消、旧响应、不支持不伪造解析成功 |
| DNS 缓存清理 | Fake-IP 与系统缓存的确认、执行、逐目标结果与重试 | `application::dns_cache_application` 唯一报告与命令 owner，`dns_cache_actions` 中性确认状态 | Iced / Bevy 独立原生确认与结果操作面 | 取消不得调用端口；独立报告不受 DNS 配置读取故障遮蔽，保留原始失败类别 |
| Hosts 映射编辑 | 添加/编辑/删除行、取消、应用与失败重试 | 中性 `contract::dns_hosts` 严格 codec 与校验；`application::dns_hosts_editor` 草稿状态；持久 owner `ConfigurationApplication` | Iced / Bevy 原生行编辑表面 | 非空非法输入不能当清空；命令终态才更新已应用事实，读失败保留草稿与最后观测 |
| 规则追踪与反向应用 | 明确本地模拟、完整决策链、源配置身份与确认后写入 | `RuleTracerApplication` 与其来源绑定的 `statistics` 唯一 owner；`ports::rule_tracer` 原子源校验；共享 projection 唯一文案与状态折叠 | Iced Tracer 操作面 / Bevy 等价独立操作面 | reader 不触发重算或增加统计；计数与耗时核对完整配置来源，当前统计和历史报告分开发布；模拟输入不冒充实测；反向应用核对配置与原规则，取消不写入，失败保留结果 |
| 规则行与本地计数回放 | 编辑行语义、追踪计数、停用和遮蔽状态 | `rule_list_projection::draft_page` 统一完整草稿行及已有计数关联；`rule_row_projection`、`rule_statistics_projection`、`rule_statistics_inspector_projection` 与 `rule_statistics_workbench` 唯一文案、详表分页、审计目标和确认/清空状态机 | Iced 与 Bevy 原生规则行 | 不从活动连接缓存推算第二份命中统计，不把本地模拟计数称为实测流量；缺少行读数显示未观测 |
| Rules / Providers / Sniffer | 规则列表、provider 更新、Tracer 沙盒、JSON 编辑 | 列表草稿与稳定行身份归 `infiltrator-application::rule_list_editor`，源绑定提交归 `rule_list_application`，未提交向导来源归 `rule_form_binding`；语法归 `infiltrator-domain::{rules,proxy_providers,sniffer}`，其它配置读写归 `configuration_application` | Iced Rules、Bevy Rules、Android Rules | FUNC-005：完整源文档与显示窗口分开；双端修改先暂存，取消无写入，保存核对源身份并走宿主事务；重排不误改行，旧表单不写新源；区分结构化编辑与原始 JSON |
| 宿主网络服务观测展示 | VPN 生命周期/MTU、提权事务结果与 PAC 地址字节 | `application::host_network_projection` 唯一折叠；原始事实来自 VPN/privileged_network/PAC 契约 | Iced 与 Bevy 原生操作面回放同一状态、错误原因与动作门控 | 未观测 MTU 不补零；不支持和失败保留原始原因；语言变化不替换输入实体 |
| WebDAV 同步 | 保存、测试、手动同步、三向冲突处理 | `infiltrator-application::SyncApplication` + `SyncPort`；`sync-engine`/WebDAV/SQLite 在 core adapter，键级 diff 在 domain；中性 `contract::sync::SyncStatus` 与共享 `sync_projection` 拥有状态/字段文案 | Iced Sync、Bevy Sync、Android、CLI | FUNC-004：配置开启不证明连接成功；面板只回放实际冲突字段；完整单一同步生命周期和来源关联冲突命令仍须闭环 |
| Core 版本交付 | 查询、下载、校验、安装、切换、回滚 | `infiltrator-application::VersionApplication` + `VersionPort`；`mihomo-version` 只在 core/desktop adapter | Iced Settings、Bevy Settings、CLI、Admin、CI/package | CORE-006 / UP-001：版本 manifest、digest、回滚 |
| 系统集成 | 系统代理、自启动、托盘、悬浮窗、权限 | `mihomo-platform`、`infiltrator-desktop`、各宿主 | Iced tray/HUD、Bevy tray/HUD、Android VPN | PLAT-003 / UI-005：native adapter 与业务解耦 |
| 多语言与主题 | 语言、主题、错误文案、无障碍提示 | 文案唯一资源源 `infiltrator-shared::locales`；错误码 `error_codes` 只映射共享键与原始参数，各 UI 只持主题与语言回放 | Iced Theme、Bevy Theme、Android resources | UI-006：文案 key 和失败状态不分叉 |
| 原位本地化回放 | 语言变化时更新已挂载文案并保留控件身份 | 共享插值 `infiltrator-shared::i18n_interpolator::interpolate`；控件层 `infiltrator-bevy-widgets::localization`，产品偏好接线 `infiltrator-bevy-ui::localization` | 原生 `LocalizedText` 与共享语言投影 | 不重建活跃控件，不翻译用户数据，门禁覆盖双端 |
| Bevy 字体与图标初始化 | 嵌入字体注册、图标句柄和已挂载控件回放 | `infiltrator-bevy-widgets::asset_setup` 受限启动系统；资源分别归 `fonts` / `icon` | `WidgetsPlugin` 注册 `PreStartup`，先于页面和截图激活 | 生产装配不取得 World；缺少资产资源保持默认句柄，已有实体保持身份 |
| Bevy 受限 ECS 接线 | 页面投影目标、原生控件与 SDK 交互的明确访问声明 | 各页面 `query_access` 子模块与控件 owner 的 `QueryData` / `QueryFilter`；快捷键按输入、外观与通知拆分参数 | 原有 Observer / schedule 消费职责明确的 `SystemParam`，资源与业务 owner 保持原位置 | 保留所有实体筛选及可变查询互斥；不能用 lint 豁免、类型转发或 World 隐藏访问范围 |
| 测速摘要与历史文案 | 实测带宽、抖动、丢包评级、出口归属和历史顺序 | `application::speedtest_summary_projection`；详情仍归 `speedtest_detail_projection`，文案归 shared surface 本地化表 | Iced 测速卡与 Bevy 概览回放同一折叠，语言变化原位更新 | 未测值保持未知，真实零值保留，用户节点/分组/出口只插值一次，不翻译身份 |
| 规则集与 MRS 事实文案 | 来源、更新时间、刷新、ETag、缓存、MRS 状态与完整规则表达式 | `application::rule_provider_projection`、`rule_mrs_projection`、`logical_rule_projection` 唯一折叠；JSON 编辑反馈归 `contract::rule_json_feedback` 与 `application::rule_json_projection` | Iced / Bevy 各原生规则操作面 | 语言切换不加载事实、不重建输入，不翻译原始用户规则；未知、失败、空结果分别呈现 |
| 应用分流策略文案 | 全部代理、仅代理选中应用、选中应用直连及显式单应用覆盖 | `domain::app_routing::AppRoutingMode` / `AppRoutingRule` 唯一枚举；`application::routing_projection` 唯一语义标签 | Iced / Bevy 原生分流页面 | 不用相反含义的白名单/黑名单名称替代领域决策；保留实际应用与进程身份 |
| 快照历史与差异文案 | 观测摘要、修剪来源、差异摘要与行标签 | `application::snapshot_presentation`；资源来自 `infiltrator-shared` | Iced 历史与 Diff；Bevy 原位文案回放 | 未观测与成功空历史区分；用户身份不插值重解释，语言切换保留原生控件与选择 |
| 快照恢复审阅与事务 | 冻结来源与完整内容、确认、取消、权限重试和实际提交回执 | `contract::snapshot_restore`；`application::snapshot_application::restore` 为提交 owner，`snapshot_restore_workbench` 为双端状态机，`snapshot_restore_projection` 唯一文案折叠 | Iced 独立 TEA 审阅层；Bevy `pages::snapshot_restore` 受限 Observer 与调度，Diff/历史/同步入口只打开审阅 | 准备与取消零写入；审阅后快照或完整 workspace 来源变化拒绝覆盖；确认只携带实例审阅身份，成功/取消后不能重放；实际像素见统一场景证据 |
| 快照存储与比较观测 | 创建、历史、修剪、真实 Myers Diff 与恢复 | `application::snapshot_application` 为实例 owner；`command_application::snapshots` 返回 `contract::command_output` 的类型化结果；desktop runtime/命令/reader 共享 `SurfaceEngines` 的配置、配置读取与快照实例；Iced `snapshot_commands` 只提交意图 | Iced / Bevy 配置工作台与同步快照入口 | 不使用进程全局缓存；按配置身份隔离，Diff 再核对当前文档字节哈希；恢复审阅、请求终态和来源事务仍以正式场景证据验收 |
| 配置文档与 Mixin 保存 | 用户观测来源、冻结待保存草稿、实际类型化回执、丢弃与失败重试 | `application::profile_document_application` / `profile_options_application` 负责实际事务，`profile_edit_session` 为双端来源及终态状态机；存储 `ProfileWorkspacePurpose` 区分直接编辑与派生变更 | Iced TEA typed replies / Bevy 受限 `profiles_editor_transactions` Observer | 不临时构造宿主或使用 Unit 成功；提交必须核对用户原始来源，直接编辑保护与元数据更新参与同一存储写入边界；具体场景完整度以统一证据为准 |
| 配置编辑展示语义 | 保护、真实语法诊断、窗口范围、游标与应用终态 | `application::profile_editor_projection` 消费中性契约，唯一共享本地化来源 | Iced / Bevy 原生编辑器 | 契约不保存固定中文显示方法，原位回放不重置文档、焦点与草稿 |
| Mixin 工作台展示语义 | 类型化预设、流水线阶段、真实列内容与预检 | `domain::mixin_studio` 只持中性身份与真实 YAML 结果；`application::mixin_studio_projection` 唯一折叠 | Iced / Bevy Mixin 原生工作台 | 不持双轨标签；语言回放保留列与缓冲区，未知阶段不能冒充已执行 |
| 订阅观测状态文案 | 回放条件请求缓存、安全备份、过滤与真实更新计划 | `infiltrator-application::subscription_status_projection` / `profile_metadata_projection` 统一计划、重载偏好与可选流量事实；全部过滤草稿、来源和终态由 `subscription_filter_editor` 统一管理；基础/高级表单转换与校验归 `domain::filter_policy_form`，持久策略唯一定义归 `domain::profile_options::FilterSpec`；`contract::command_output` 校验读取/保存的结果类型与配置身份 | Iced 订阅设置/文档过滤与 Bevy 导入工作台/文档过滤 | Last-Modified-only 缓存不丢失，未选择与关闭分开；语言回放不重置输入草稿 |

## 表面的职责与同步演进定位

详见最高主控台账 [DUAL_SURFACE_PARITY_MASTER_PLAN.md](DUAL_SURFACE_PARITY_MASTER_PLAN.md)。

| Surface | 定位 | 应该做 | 不应该做 |
| --- | --- | --- | --- |
| Iced | 同权产品（TEA 桌面形态） | 完整桌面流程、托盘、悬浮窗、高保真运行态、内核管理 | 私自发明底层 API 语义；与 Bevy UI 产生功能分叉 |
| Bevy UI | 同权产品（ECS 自适应形态） | 与 Iced 严格同步演进；消费同一套 shared 契约；支持多模态自适应布局 | 复制 Iced 的私有状态；滞后跟随；私自修改核心配置协议 |
| Android Compose | 移动原生伴侣 | VPN/TUN、移动导航、权限和前后台生命周期 | 直接复制桌面文件/进程模型 |

## 一条功能的完成定义

功能域只有同时满足以下条件，才能从开放项移入完成项：

1. Rust owner 和单一写入口明确；
2. Iced 与 Bevy UI 双端具备等价语义和交互深度；
3. 成功、失败、取消、超时、不可用和版本不兼容有 typed 结果；
4. L1/L2 的真实测试 ID 已绑定并实际通过，L3 场景像素具备双端双视口回执；
5. 适用的目标平台、打包或真实 core smoke 已验证；
6. `USAGE_SPEC.md`、`DUAL_SURFACE_PARITY_MASTER_PLAN.md` 记录已更新。

共享工作流文案由 `infiltrator-shared/src/locales_table_workflow_{zh,en}.rs` 定义；Bevy 原位本地化由 `infiltrator-bevy-widgets/src/localization.rs` 回放；`infiltrator-bevy-ui/src/localization.rs` 只将共享产品语言偏好接入控件资源。代理地区标识与拼音匹配继续使用 `infiltrator-shared` 的唯一地区/搜索实现，不在 UI 重建词典。

诊断报告的唯一 owner 为 `application::doctor_application`，诊断/修复/初始化命令与 `ApplicationSurfaceReader` 必须持有同一实例的克隆。状态和文案折叠由 `application::doctor_projection` 负责，双端原生请求关联由 `doctor_actions` 状态模型驱动；Bevy 动态检查行按稳定 ID 原位协调。

DNS 泄漏报告的同一事实到文案、行序和状态色阶由 `application::dns_leak_projection` 唯一折叠，Iced 与 Bevy 只映射原生样式；宿主错误和探测中/失败/重试状态由共享 DNS 用例 owner 保留；独立 `SurfaceSnapshot::dns_leak` 不因配置页失败丢失。

配额文案与状态色阶唯一 owner 为 `application::subscription_quota_projection`；原始事实来自 `SubscriptionQuotaApplication`，两端不在 UI 汇总流量或补造示例值。

节点延迟历史复用 `application::proxy_inspection_projection`、`proxy_inspection_reader` 与双端原生节点检查器。Iced 的历史入口不拥有独立雷达事实或样本写入路径；配置事务状态复用 `application::profile_editor_projection::transaction` 和实际共享回执，不由 Settings 合成成功。

配置应用回执由实际运行宿主持有的 `mihomo_config::manager::ConfigManager` 实例记录；`ports::profile_store` 只暴露按配置身份读取的中性数据。contract 仅定义不可变回执，禁止全局发布/读取函数。

配置编辑文档和选项观测归 `application::profile_editor_observations`，由注入的 ProfileApplication 实例持有并在克隆间共享。独立产品不共享观测；读取票据绑定实例、配置、目标代次和各自操作序号，两个文档按同一 ProfileSourceIdentity 回放。`SurfaceSnapshot::profile_editor` 独立于列表页面发布文档、选项、各自读取状态与实际来源核验；失败保留原观测，保存门控由共享 `profile_edit_session` 决定。contract 只保留数据，旧全局发布函数退役。

活动配额的观测保留和失效归 `application::subscription_quota_application` 的产品实例 owner；纯数值派生仍归 domain，语言和色阶归共享 quota projection。读取票据在异步工作前分配；活动配置与订阅来源身份独立读取前后核验，失败只保留同配置、同订阅来源、同内核代次和会话的实际观测。来源只发布哈希，不发布订阅凭据。

产品运行态错误的中性原因定义归 `contract::error::FailureReason`，唯一语言映射归 `application::failure_projection`，文案仍归 `infiltrator-shared`。编辑器、配额与内核下载取消复用此投影；具体下载器以 `MihomoError::Canceled` 经版本端口保留取消类别与恢复策略，Iced 不再匹配错误正文。Iced 运行态操作、服务和恢复提示复用共享资源；未迁移的其它运行态错误和通知不能因结构 i18n 门通过而视为已本地化。
