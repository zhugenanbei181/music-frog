# Tauri/WebUI 退役台账（0.20 移除基线）

> 本文是移除 Tauri 桌面宿主与 WebUI 管理面板前的完整功能台账，双职能：
> **iced 0.20 按图索骥补齐**（下文「iced 对照矩阵」中所有 PARTIAL/MISSING 项），
> **bevyui 0.30 参考实现规格**（能力面 + admin REST 契约 + 三端差异规则）。
> 台账形成于 release/0.20 分支创建时点，移除执行记录见文末。

## 0. 版本路线图（本文随移除落地而更新）

| 版本 | 主题 | 端形态 |
| --- | --- | --- |
| **0.20** | iced 全面成熟：托盘全功能 + 全部管理功能；Tauri/WebUI 全面下课 | Android 原生（Compose+UniFFI）、跨平台 Desktop（Iced）、bevyui 起步（仅目录，不入构建面） |
| **0.30** | bevyui 成熟，Android + Desktop + iOS 大一统（三端同一 Rust UI 内核） | Bevy UI 三端 + 各平台宿主薄壳 |
| **1.0** | 终局：三端 parity、发布门禁自动化、生态化 | — |

## 1. src-tauri 宿主功能台账

**架构事实**：src-tauri 无任何 Tauri IPC 命令（无 `#[tauri::command]`）、无窗口
（`tauri.conf.json` `windows: []`）。它是以 Tauri 为 app-shell 的「托盘 + 双内嵌
HTTP 服务器 + 浏览器 UI」宿主：静态服务器（Zashboard，默认 24173）+ Admin API
服务器（默认 25210），UI 即系统浏览器。

### 1.1 托盘能力（`src-tauri/src/tray/`，菜单 ID → 行为）

| 分组 | 能力 |
| --- | --- |
| 信息区 | 静态端口/控制器/admin URL、admin 权限状态、内核版本/已装/状态/网络、同步状态、关于（应用/SDK/内核） |
| 入口 | 打开 Zashboard（左键同）、打开管理面板（含 `#dns`/`#fake-ip`/`#rules`/`#tun` 锚点直达） |
| 内核子菜单 | 切回内置内核、按版本切换（rebuild）、按版本卸载（确认）、更新到最新 Stable（托盘文本实时显示速度/ETA，完成后清理旧版本） |
| 设置子菜单 | 开机自启（注册表 `MihomoDespicableInfiltrator`，启用需管理员）、「启动时打开代理页」开关、TUN 开关（需管理员，5s 轮询外部变更刷新菜单） |
| 同步子菜单 | 立即同步（OS 通知结果）、同步设置入口 |
| 系统代理 | 开关（指向运行时 `http_proxy_endpoint()`） |
| 高级子菜单 | DNS / Fake-IP / 规则 / TUN 设置直达（浏览器锚点）、Fake-IP 缓存清理 |
| 模式子菜单 | rule / global / direct / **script** |
| 配置子菜单 | 多 profile 快切（rebuild）、全部更新订阅（OS 通知开始/逐个成功/失败原因/汇总）、单 profile 自动更新开关 |
| 节点子菜单 | 每组前 5 组 × 每组 20 节点，含延迟标签，溢出折叠子菜单 |
| 管理 | 提权重启（`ShellExecuteW runas`，保留 `--static-port`/`--admin-port`）、恢复出厂（确认对话框 → 停机 → 关自启 → 清 settings/log/mihomo home/profiles → 重建）、退出（完整 shutdown：调度器 + 前端 + 运行时 + 系统代理关闭） |
| 动态刷新 | 设置变更/运行时引导后全量重建菜单；`EVENT_PROFILES_CHANGED`/`CORE_CHANGED`/`TUN_CHANGED` 触发局部刷新；菜单构建失败降级为「错误 + 打开 + 退出」 |

### 1.2 宿主系统集成

| 能力 | 实现 |
| --- | --- |
| OS 通知 | `tauri-plugin-notification`：订阅更新开始/成功/失败原因/汇总、WebDAV 同步成功/失败原因 |
| 启动引导 | 3 次重试，失败轮换 external-controller 端口；就绪等待 15s；rebuild 流程等端口释放（5s）并重放系统代理 |
| 内核更新 | 仅 Stable，30s 检查/600s 下载超时，托盘进度文本，完成后 `set_default` + rebuild + 卸载其余版本；内置候选 `vendor/mihomo.exe` 等 |
| 恢复出厂 | 额外清理：`<app_data>/logs` 目录、mihomo home 目录（iced 现版缺这两项） |
| 提权 | `is_elevated` 检测 + `ShellExecuteW("runas")` 重启（保留端口参数）；rfd 错误/确认对话框；原生文件选择器 |
| 单实例 | `tauri-plugin-single-instance`，第二实例仅重开浏览器 |
| 崩溃 | panic → log + 原生错误对话框 |

### 1.3 Tauri 独有逻辑（共享 crate 之外）

浏览器即 UI 模型（静态托管 + 锚点 + 开机打开开关）、全功能托盘本体、
引导重试/换端口循环、Stable-only 内核更新带托盘进度、OS 通知、
`ShellExecuteW` 提权、独立注册表名 `MihomoDespicableInfiltrator`。

## 2. WebUI 面板能力台账

### 2.1 webui/mihomo-manager-ui（Zashboard，上游 vendored dist）

外部 mihomo 仪表盘（连接/日志/代理/规则），仅由 src-tauri 静态服务器提供，
iced 从未引用。随宿主整体退役；THIRD-PARTY-NOTICES 的 zashboard/字体条目同步移除。

### 2.2 webui/config-manager-ui（Vue 3 管理面板）

页面：profiles / webdav / network(#dns,#fake-ip) / core(#tun) / runtime / rules /
diagnostics(Doctor)；SSE 实时事件（`GET events`）。消费的 admin API 面
（`/admin/api/*`，服务端在 `crates/infiltrator-admin/src/admin_api.rs:50-171`，**保留**）：

- 能力探测 `capabilities`；设置 `GET/POST settings`
- Profiles：列表/详情/切换/URL 导入/保存/删除/订阅设置/立即更新/清空/编辑器打开
- Editor：`GET/POST editor`、`editor/pick`（宿主原生选择器）
- 内核：`core/versions`、`core/latest-stable`、`core/download`、`core/update-stable`、`core/activate`
- 运行时：`runtime/start|stop|status`、`rebuild/status`、connections(删除/单个)、
  traffic、memory、ip、proxies、delay test(单个/全部)、`runtime/logs[?level=]`
- 模式/节点：`proxies`、`proxy/mode`、`proxy/select`
- 配置面：dns / fake-ip(/flush) / rule-providers / proxy-providers / sniffer / rules / tun
- WebDAV：`webdav/sync`、`webdav/test`
- Doctor：`doctor[?only=]`、`doctor/checks[/{id}]`、`doctor/fix`、`bootstrap`

**该 API 面整体保留**：iced 内嵌 admin server（Doctor 面板 loopback 依赖 +
托盘/远端管理语义），亦是 0.30 bevyui 的候选后端契约。

## 3. iced 对照矩阵（0.20 补齐 backlog 的直接来源）

| Tauri 能力 | iced 状态 | 证据 / 差距 |
| --- | --- | --- |
| 系统代理开停 + 停机关代理 | ✅ COVERED | `update/ui.rs`、`admin_server.rs` |
| 开机自启（注册表） | ✅ COVERED | 注册表名 `MusicFrogInfiltrator` |
| 内核下载/更新/切换/卸载 | ✅ COVERED（更强：Beta 通道、可取消进度） | `update/core/kernels.rs` |
| 恢复出厂 | ✅ COVERED（0.20 补齐：log/、崩溃日志、keyring 订阅凭证、云同步重定向目录；修复先删 settings 后解析 configs 的顺序 bug） | `infiltrator-core/src/factory_reset.rs`、`update/core/kernels.rs` |
| 订阅调度更新 | ✅ COVERED（tick + next_update） | `subscription.rs` |
| WebDAV 同步 | ✅ COVERED（更强：冲突备份 + 逐键合并） | `update/profile/sync*` |
| 编辑器集成（选择/打开 profile） | ✅ COVERED | `admin_server.rs` |
| 单实例 / 崩溃日志 | ✅ COVERED | `lib.rs` |
| TUN 开关 | ✅ COVERED（多 TUN service 安装） | `runtime_config.rs` |
| profile 管理面 | ✅ COVERED（更强：快照/过滤/Mixin） | `update/profile*` |
| Admin Web UI 托管 | ♻️ **本版退役**：admin server 转 API-only（Doctor 依赖保留） | 本次移除 |
| **系统托盘** | ✅ COVERED（0.20 全功能化：模式(4 含 script 门控)、分组节点子菜单(5 组×20 节点+延迟+溢出)、profile 子菜单(快切+全部更新+自动更新开关)、内核子菜单(设默认/卸载/检查更新/取消下载/Fake-IP 清理)、同步子菜单(状态/上/下/取消/设置)、自启、信息区(模式/状态/控制器/管理端口/内核版本)、恢复出厂；本地化 tooltip；ksni 复合 checkmark key；muda 整树重建） | `tray/menu.rs`、`tray/spec.rs`、`tray/ksni_backend.rs`、`tray/native.rs` |
| **OS 系统通知** | ✅ COVERED（0.20：notify-rust 4.12 z/zbus；订阅自动更新成功/失败、WebDAV 周期同步完成/失败、内核启动/重建/resync 错误；设置开关 `notifications_enabled` 五处接线 + admin API parity；无 D-Bus 静默降级） | `infiltrator-iced/src/notify.rs` |
| **script 模式** | ✅ COVERED（0.20：托盘第 4 模式项 + 侧栏 4 段 + 概览 label；按内核 `GET /configs` 回报的顶层 `script:` 块门控，无块时拒绝切换并提示；配置注入复用 Mixin custom-yaml，不建 JS 引擎） | `update/core/runtime_config.rs`、`view/sidebar.rs`、`tray/menu.rs` |
| **引导重试 + 控制端口轮换** | ✅ COVERED（0.20：`bootstrap_with_retry` 3 次尝试 + `rotate_external_controller` 轮换 + 端口释放等待；`BootError.tried` 聚合已试端口；启动/rebuild 双路径接入） | `infiltrator-desktop/src/boot.rs`、`update/core/lifecycle.rs`、`update/core/rebuild.rs` |
| Zashboard 托管 + 开机打开 | ❌退役 | 随 WebUI 退役，不补 |
| 外部 TUN 状态 5s 轮询 | ❌ MISSING（可选） |  |

### 0.20 iced 必办（按优先级）

~~以下 1–4 已于 0.20 全部落地（见上表状态列），5 为伴随验证：~~

1. ~~托盘全功能化（对齐 §1.1 除浏览器入口外全部项）。~~ ✅
2. ~~OS 系统通知（订阅/WebDAV 事件；跨平台抽象）。~~ ✅
3. ~~恢复出厂补 log 目录 + mihomo home 清理。~~ ✅
4. ~~引导重试 + 控制端口轮换；script 模式入口。~~ ✅
5. ~~Admin server 转 API-only 后回归 Doctor/托盘语义。~~ ✅

## 4. bevyui 0.30 参考实现规格要点

- **能力基线**：§1–§3 台账即参考实现规格；admin REST 面（§2.2）可作为 bevyui
  远程/嵌入后端契约，或直接走 `infiltrator-shared::intent_registry` 统一意图层。
- **三端差异规则**（沿用 docs/TEST_MATRIX.md §3）：命令语义/结果/revision 必须
  三端一致；布局、手势、导航、托盘形态、平台权限流程允许原生差异，但需
  `implemented / accepted difference / unsupported(reason)` 登记。
- **宿主薄壳**：Android 宿主（现有 MihomoHost.kt 模式）、desktop 宿主
  （托盘/系统代理/自启 = 现 `infiltrator-desktop` trait）、iOS 宿主待建。
- **0.20 分支中的起点目录**：`crates/infiltrator-bevy-widgets`、
  `crates/infiltrator-bevy-ui`（保留、不入 0.20 构建面；编译修复归 0.30 线）。

## 5. 移除执行记录（release/0.20）

- 删除：`src-tauri/`、`webui/`（mihomo-manager-ui + config-manager-ui）、
  根 `package.json`/`pnpm-lock.yaml`（仅 Tauri CLI 用途）、
  `.github/workflows/release-msi.yml`（纯 Tauri MSI 管线）。
- 清理引用：workspace members（src-tauri、bevy 两 crate 出面）、
  `line-guard.py`/`import-guard.py` SCAN_ROOTS、`test.yml` 的
  `libwebkit2gtk` Tauri 依赖、`scripts/fetch-mihomo.sh` 注释、
  iced `Cargo.toml` packager resources 的 webui 行、
  `admin_server.rs` 静态托管解析（转 API-only）、托盘/设置页的
  「在浏览器打开管理面板」入口、`AppSettings.open_webui_on_startup` 字段、
  README/docs 中 Tauri/WebUI 表述、THIRD-PARTY-NOTICES 的 zashboard 条目、
  `.gitignore` 的 webui/src-tauri 条目。
- 文档未动的历史记录：FIX_SUMMARY.md、ISSUE_RESOLVED.md、iced_todo.md
  （历史事实文件，保留原文）。
