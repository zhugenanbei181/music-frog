# 版本记录

- Unreleased

- 0.12.3 – **UI 优化与项目重命名**：
  - 项目重命名为 MusicFrog Despicable Infiltrator，避免版权争议。
  - 所有 checkbox 开关改为现代 switch 样式。
  - PanelCard/StatusHeader 统一使用圆角阴影毛玻璃效果。

- 0.12.2 – **UI 美化与 API 兼容修复**：
  - 修复 Axum 0.8 路由参数语法（`:param` → `{param}`），解决启动 panic。
  - PanelCard/StatusHeader 统一使用 `.panel` 样式（圆角、阴影、毛玻璃效果）。
  - WebDAV 配置表单始终可见，开关仅控制功能启用状态。
  - AGENTS.md 新增依赖 API 变更记录章节，避免重复踩坑。

- 0.12.1 – **稳定性修复**：
  - 托盘“打开配置管理”在内核未就绪时仍可用，便于排错。
  - 配置管理锚点解析增加容错，避免非法 hash 阻断导航。

- 0.12.0 – **Android 迁移基础与高级配置**：
  - 新增 DNS/Fake-IP/规则集/TUN 高级配置托盘入口与 Fake-IP 缓存清理。
  - 托盘高级入口仅在内核可连接时启用（与代理规则一致）。
  - 配置管理界面支持 hash 定位直达高级面板。
  - Web UI 面板结构统一（PanelCard + 标题/底部组件），按钮密度与空态/辅助文字风格统一。
  - App.vue 业务逻辑进一步下沉到 composables，表单工具与类型约束收敛。
  - mihomo-rs 引入 platform 抽象（CoreController/CredentialStore）。
  - DataDirProvider 接口占位，为 Android 目录注入做准备。
  - 新增 `apply_data_dir_override` 钩子，允许注入 Android 数据目录。
  - 新增目标 crates 骨架并加入工作区（Desktop 优先，Android 仅占位）。
  - mihomo-api/mihomo-config/mihomo-version 完成迁移基线（从 mihomo-rs 拷贝实现）。
  - infiltrator-core/infiltrator-desktop 提供分域 re-export 过渡。
  - mihomo-rs 切换为兼容 re-export，对新 crates 进行转发。
  - mihomo-platform 补齐 ProcessCoreController 桌面实现。
  - infiltrator-core/infiltrator-desktop 完成文件拆分。
  - despicable-infiltrator-core 切换为兼容 re-export。
  - infiltrator-core 移除 mihomo-rs 依赖，改用新 crates。
  - infiltrator-desktop 移除 mihomo-rs 依赖，改用新 crates。
  - 清理 despicable-infiltrator-core 旧源码，保留兼容 re-export。
  - ServiceManager 泛型化，桌面端类型别名保持兼容。
  - ConfigManager 使用 CredentialStore 注入，订阅凭据存取解耦。
  - 新增平台抽象相关测试（GenericServiceManager、CredentialStore）。
  - 文档补充兼容层移除前置条件与迁移计划约束。
  - Tauri 导入切换到新 crates，并完成根目录构建与测试。
  - 移除 `mihomo-rs` 与 `despicable-infiltrator-core` 兼容层。
  - 新增 DNS 配置模型与管理 API（读取/更新/校验/应用）。
  - 新增 Fake-IP 配置模型与管理 API（读取/更新/清理缓存）。
  - 新增规则集与 Rule Providers 管理 API（列表/更新/启停/排序）。
  - 新增 TUN 高级配置模型与管理 API（读取/更新/校验/应用）。
  - 配置管理 UI 补齐 DNS/Fake-IP/规则集/TUN 高级配置面板。
- 0.11.0 – **迁移切换完成**：
  - Tauri 依赖切换到新 crates（infiltrator/mihomo-*）。
  - 移除旧兼容层与遗留 workspace 依赖。
  - 修复管理员上下文构建时的 panic 风险。
  - 预留 Android Bridge 接口（Core/Credential/DataDir）。
- 0.10.0 – **WebDAV 配置同步正式发布**：
  - **WebDAV 多设备同步**：完整实现跨设备配置自动同步与云端备份功能
    - `dav-client`: 基于标准 WebDAV 协议 (PROPFIND/GET/PUT/DELETE + If-Match 条件上传)
    - `state-store`: SQLite 状态数据库追踪文件哈希与 ETag
    - `sync-engine`: 三方对比算法 (本地/远端/上次状态) 智能决策上传/下载/删除
    - 冲突处理: 双向修改时自动保存远端备份为 `.remote-bak-{timestamp}`
  - **UI 完整集成**：
    - WebDAV 配置面板 (URL/用户名/密码/同步间隔/启动时同步)
    - 连接测试与手动同步按钮
    - 完整国际化支持 (zh-CN / en-US)
  - **后端调度器**：
    - 自定义同步间隔 (默认 60 分钟)
    - 启动时自动同步选项
    - HTTP API (`/admin/api/webdav/sync`, `/admin/api/webdav/test`)
  - **安全机制**：
    - 原子写入 (`.sync-tmp` 临时文件 + rename)
    - ETag 防并发冲突
    - 仅同步 `.yaml/.yml/.toml` 配置文件

- 0.9.12 – **技术栈升级与体验优化**：
  - **依赖升级**：前端构建工具升级至 Vite 8.0 Beta，Vue 3.5+，Tailwind CSS 4.0+。
  - **国际化 (i18n)**：全面支持简体中文 (zh-CN) 和 英语 (en-US)。托盘菜单和配置管理界面均已实现动态语言切换。
  - **线程安全增强**：核心运行时引入严格的并发锁机制 (`rebuild_lock`)，彻底解决内核重启、配置切换与恢复出厂设置时的竞态条件。
  - **Admin API 模块化**：重构管理接口，支持应用级设置（语言、编辑器路径等）的读取与保存。
  - **稳定性提升**：修复托盘代理节点显示限制（增加到 20 个）；清理死代码；强制 LF 换行符规范。

- 0.9.11 – 内部版本，包含上述功能的逐步集成。

- 0.9.10 – 优化托盘菜单状态同步机制：内核启动成功后立即刷新菜单，解决代理组、TUN 等功能在启动初期呈灰色不可用的问题。

- 0.9.9 – 修复右键托盘菜单闪退问题；使用原生 Windows API (`ShellExecute`) 替代 PowerShell 实现管理员重启，提升安全性与稳定性。

- 0.9.8 – 托盘菜单全面重构：采用功能分组设计，新增分隔线提升视觉清晰度；添加“关于”子菜单展示版本信息；在配置切换中集成“自动更新当前订阅”开关；修复 TUN 模式菜单在配置未启用时灰化的问题。

- 0.9.7 – 简化内核启动检测逻辑：移除显式 TCP 端口探测，完全依赖 API 客户端重试（支持最长 15s 等待），并在等待期间实时监控内核进程存活，避免因检测方式不一致或进程中途退出导致的启动误判。

- 0.9.6 – 引入智能启动重试机制：当内核启动失败（如端口被抢占或进程异常）时，自动轮换端口并重试启动（最多3次），彻底解决“端口占用”导致的启动死锁。

- 0.9.5 – 优化内核启动流程：增加对端口开放的显式等待，避免因内核启动慢导致的“控制接口未就绪”误报；增加内核进程即时存活检查，在启动失败时提供更明确的错误信息。

- 0.9.4 – 修复订阅调度器（SubscriptionScheduler）在非 Tokio 上下文中启动导致的 Critical Panic 问题。

- 0.9.3 – 新增全局 Panic 钩子，确保崩溃时弹出错误对话框而非静默退出；修复托盘菜单构建中的异步线程安全问题，提升运行稳定性。

- 0.9.2 – 托盘菜单构建增加 ID/名称容错与精简兜底，避免异常配置或节点名导致托盘初始化失败。

- 0.9.1 – 补强订阅更新之防重入/重试/批量单次重载与密钥链存储，增加系统通知与托盘手动更新入口。

- 0.8.3 – 完成 Rust 防 panic 编码规范审计，修复所有高优先级问题（`.parent().unwrap()`、直接索引访问等），业务逻辑代码不再包含裸 unwrap 调用。
- 0.8.2 – 修复编辑器路径选择对话框构建导致打包失败。
- 0.8.1 – 修复外部编辑器通过 code/cmd 启动失败，新增编辑器路径选择入口，自动探测失败回退记事本。
- 0.8.0 – 外部编辑器自动探测 VSCode 并回退记事本，支持带参数命令解析，配置管理 UI 模块化拆分，新增清空配置与托盘恢复出厂设置。
- 0.7.3 – 撤销 MSI 安装界面定制，配置切换等待增加超时与重试，避免管理页卡死。
- 0.7.2 – 配置切换/导入/保存增加锁定与重启等待反馈。
- 0.7.1 – MSI 安装定制（已撤销）。
- 0.7.0 – 订阅导入改为后台重启提升响应速度，Windows 启动不再闪 cmd 窗口，配置管理界面升级为 Vue 3 + TS + Tailwind。
- 0.6.7 – 订阅导入增加解码失败兜底（禁用自动解压后手动解码 gzip/deflate/brotli），并在导入失败时自动刷新配置列表，补充应用日志路径说明。
- 0.6.6 – 订阅导入改用字节流解码并开启压缩解码能力，避免“error decoding response body”，管理接口补全请求/响应链路日志。
- 0.6.5 – GeoIP 支持从内核同目录复制并新增镜像源兜底下载，重启前等待控制端口与代理端口释放并明确占用错误。
- 0.6.4 – GeoIP 预下载强制使用 identity 编码并校验文件大小，避免响应体解码失败导致启动中断。
- 0.6.3 – 启动前预下载 GeoIP 数据库以避免 GEOIP 规则导致启动失败，配置切换后无论成功与否刷新列表状态。
- 0.6.2 – 启动时等待控制端口就绪再上报，mihomo 输出重定向到日志文件便于排查配置问题。
- 0.6.1 – 运行时设置改为 TOML 并自动迁移旧 JSON，配置切换/保存时重启等待控制端口释放，避免 Web UI 控制端口漂移。
- 0.6.0 – 托盘/运行时/前端/设置逻辑模块化拆分，核心能力下沉到 `despicable-infiltrator-core`，移除平台 `unsafe` 调用并改用安全库封装，YAML 解析切换为纯 Rust 实现。
- 0.5.11 – 兼容默认内核文件名 `mihomo.exe` 与 `mihomo-windows-amd64-v3.exe`，补充使用规范文档并打包进安装包。
- 0.5.10 – 修复安装目录 `bin/mihomo` 下捆绑内核的查找路径，确保首次启动直接使用默认内核。
- 0.5.9 – 首次启动强制使用捆绑内核且不联网下载，下载失败时回退捆绑内核，改进资源路径解析并记录日志。
- 0.5.8 – 切换内核后托盘状态自动刷新，避免停留在“启动中/初始化中”。
- 0.5.7 – 外部编辑器报错更明确，删除配置/内核强提示确认。
- 0.5.6 – 托盘内核管理支持默认内核与版本启用/删除，配置列表加入滚动。
- 0.5.5 – 新增本地导入与外部编辑器设置，内核更新显示进度/网络状态，支持计划任务自启。
