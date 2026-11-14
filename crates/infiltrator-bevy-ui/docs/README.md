# Bevy UI 文档

> 层级：L3

本 crate 只拥有 Bevy 页面实现、通用宿主注入入口和隔离截图工具。桌面 `DesktopProductSession`、Core pump、command sink 与产品可执行入口由 [Bevy desktop host](../../infiltrator-bevy-desktop-host/README.md) 组合；Android driver 只注入 owner IPC client、窗口输入和 Insets。

- [BEVY_UI_FRONTEND.md](BEVY_UI_FRONTEND.md)：ECS、bsn 场景、shader 与资源生命周期章程。
- [BEVY_020_UI_CAPABILITIES.md](BEVY_020_UI_CAPABILITIES.md)：Bevy 0.20 UI 能力唯一权威规范（rem/em 尺寸、ThemeContext 分层主题、headless 控件优先、FixedNode、InlineBox、bsn_list!/Ready/despawn_all）；场景结构守卫（`bevy_bsn_guard.py`）、尺寸通道守卫（`bevy_rem_guard.py`）与强制化台账均以此规范为单一真源。

## 消费端架构纪律

Bevy UI 表面严格作为纯被动消费端：仅消费宿主注入的 `SurfaceSource` 并向 `UiCommandSink` 派发 typed 意图命令；禁止在 ECS System、Resource、Component 或 `bsn!` 场景中派生第二份业务真源或暂存底层未决状态。

- demo 页面矩阵运行 `bash scripts/capture-bevy-matrix.sh`；真实 Mihomo 首页运行 `bash scripts/capture-bevy-live.sh`。产品默认 feature 不包含截图 fixture；页面矩阵显式启用 `capture-tools`，live capture 显式启用 `capture-controller`。截图写入本地 `screenshots/`，默认不入库。
- crate 入口见 [README.md](../README.md)。

## 已归档 FlClash 条目的当前范围（b246 复核）

本节只记录**当前代码范围**，不改 `TODO.md` / `todos/` 的归档身份，不新建任务 ID，
也不把未实现的功能写成已交付。依据为只读侦察
`.evidence/wip/b246-flclash-bevy-gap-recon.md`「Top contradictions」与下列逐行复核；
上游行号指 pinned FlClash `be84736605527034689c95bf27828ff27d912c5e`
的 `git show <commit>:lib/... | nl -ba` 输出（`prism_ui` gitlink 内部未验证）。

- **`FLPARITY-007`（Tools IA / 四端路由闭环）**：归档裁决保留——15 个 `PageId`
  与 Bevy 15 条 `Route` 及闭环挂载确实存在
  （`crates/infiltrator-bevy-ui/src/route.rs:165`、`src/route/mount.rs:129`）。
  但它**不是** pinned v0.9.3 的动态 Tools IA：当前 `src/route/mount.rs:151`
  只把 `&UiPalette` 传给 `tools_page`，`src/pages/about.rs:330`–`:340` 是六条
  写死的 `tool_item_scene`，`:349`–`:376` 只生成图标 + 标题 + 描述，无
  `Button` / `Activate` / `RouteChanged`；`src/pages/settings_index.rs:48` 是注册表
  的静态反查，不是能力读回。上游 Tools 是 `moreToolsSelectorState` 驱动的索引
  加设置/其他分区（`lib/views/tools.dart:69`、`lib/providers/state/navigation.dart:84`）。
  **判定：归档条目按「当批 15 页路由 + 四端接线」范围成立；若把「Tools IA
  四端闭环」读成 v0.9.3 动态能力索引，则与当前 Bevy 代码矛盾。**剩余 Bevy
  UI 缺口是能力驱动的 Tools 条目/设置控件消费，见下表第 1、4 行。
- **`FLPARITY-003` / `FLPARITY-004`（实时请求历史 / DNS 查询历史 parity）**：
  Core 与 presentation 切片确实交付并归档——
  `SurfaceSnapshot.requests` / `dns_query_history`
  （`crates/infiltrator-contract/src/surface_snapshot.rs:694`、`:696`，HEAD
  同值见 `:690`、`:692`）、`project_request_page`
  （`crates/infiltrator-presentation/src/request_tracker_projection.rs:126`）
  与 `project_dns_history_page`
  （`crates/infiltrator-presentation/src/dns_query_history_projection.rs:99`）。
  但 Bevy `src/route.rs:165` 无 Requests / DNS-history / Resources 路由，
  `src/route/mount.rs:129`–`:155` 无对应场景；`rg RequestTrackerSnapshot|
  request_tracker crates/infiltrator-bevy-ui/src` 零命中，
  `DnsQueryHistorySnapshot` 在 Bevy 只以 `Default::default()` 出现在
  `src/surface_demo.rs:152`、`:290`。因此这是 **Core / presentation 范围归档**，
  不是四表面 UI parity；按四表面读则与当前代码矛盾。剩余 Bevy UI 缺口见下表
  第 2、3 行。本节不重开归档条目、不新建看板任务。
- **`SPACEUTIL-018` / b218 的 Bevy 12px 记录**：归档身份保留；那是**过期事实**
  而非新任务。HEAD 与工作树 `crates/infiltrator-bevy-ui/src/shell_scene.rs:116`
  的内容列均为 `padding: UiRect::all(rem(space::S16))`，`9c7bb6e` 之前已是
  `Val::Px(space::S16)`；`S12` 只留在胶囊/侧栏（`:246`、`:260`、`:369`、`:406`）。
  跨表面空间利用率标准 §13.1 已作 12px → 16px 的数字勘误，并钉住 Slint/Iced
  两表面像素回执的归档范围；Bevy 像素仍属 `FLPROFILES-002`
  未收口红腿，不在此宣称已消费导出值或通过空间判定。

## Current FlClash desktop IA gaps (b246 recon)

下表是 recon「Top 6 ranked」按当前 Bevy 代码重算的范围表：`Core gap` 指缺
typed owner / readback / 身份，`presentation gap` 指 Core 已发布而 Bevy 未消费，
`mixed` 指两者兼有。每行只描述**当前不存在的消费端**，不宣称实现或像素验收；
全部上游证据来自 recon 的 pinned `lib/...:LINE`，本地证据来自 `src/...:LINE`。

| # | 缺口 | 类型 | 上游证据（pinned FlClash） | 当前 Bevy 证据 |
| --- | --- | --- | --- | --- |
| 1 | `NavigationCapabilitySnapshot` 消费：隐藏 `logs`/`proxies`、active fallback 与 Tools 行仍由静态注册表绘制 | presentation gap（读回已存在；若需新 `requests`/`resources` 身份则先补 Core） | `lib/views/navigation.dart:11`、`:25`、`:56`；`lib/providers/state/navigation.dart:24`、`:38`；`lib/models/config.dart:249`；`lib/views/tools.dart:69` | `src/shell_nav.rs:348` 用 `desktop_items(...)`（b268 起本表面的行集与 Requests 行的词都由该注册表给，私有 `requests_nav_item_scene` 已退役）、`:359` 硬判 `route == Route::Overview`；`src/pages/settings_index.rs:48` 用 `pages_folded_under`；`src/surface_demo.rs:190` 把读回发布为 `PageData::unavailable`；Bevy 生产代码无 `NavigationCapabilitySnapshot` / `visible_rows` / `active_destination` 消费者。Core owner：`crates/infiltrator-contract/src/surface_snapshot.rs:735` |
| 2 | Requests / live-recent feed：**路由、导航行与 typed `RequestFeed` 已交付（b268）**，仍缺上游的 5 个 facet 过滤与 dashboard feed | presentation gap（Core 读回原已在 `surface_snapshot.rs`；缺的是 `RequestHistoryFilter` 的 facet 身份与持久化） | `lib/views/navigation.dart:34`；`lib/views/connection/requests.dart:32`、`:38`、`:89`、`:118`、`:145`、`:161`；`lib/models/config.dart:251`；`lib/views/dashboard/widgets/connections_table.dart:49`、`:61` | **已交付**：`src/route.rs` 的 `Route::Requests`、`src/pages/requests.rs:51` 的 typed `RequestFeed::{Recent,Live}`（两成员枚举，非两个 bool）、导航行由契约注册表 `desktop_items(SurfaceKind::BevyDesktop)` 发布（`SURFACE_LOCAL_REQUESTS`，表面不再自挂私有行）。**仍缺**：上游的 5 条 facet 过滤与「关闭实时」需要契约先给 `RequestHistoryFilter` 的 facet 身份，本表面不自行比对字符串猜测 |
| 3 | Resources + DNS-query-history：无资源清单/更新全部 IA，DNS 页只做配置与动作 | mixed（DNS 读回与投影已在 Core/presentation；资源清单 owner 缺失） | `lib/views/navigation.dart:40`、`:46`；`lib/views/dns_queries.dart:137`；`lib/views/resources.dart:170`、`:183`；`lib/enum/enum.dart:525`；`lib/views/tools.dart:76` | `src/route.rs:165` 只有 `Dns`；`src/route/mount.rs:146` 映射到 `dns_page`；`src/pages/dns.rs:284` 是配置/动作页；`DnsQueryHistorySnapshot` 仅在 `src/surface_demo.rs:152`、`:290` 默认值；`surface_snapshot.rs:694` 有字段但无资源清单对照物 |
| 4 | 静态 Tools hub：六条不可点的说明行，缺设置控件与能力驱动条目 | presentation gap（Tools 行与折叠页 owner 已存在） | `lib/views/tools.dart:28`、`:49`、`:64`、`:103`、`:117`、`:132`、`:145`、`:158`、`:171`、`:184`、`:197`、`:210`、`:223`、`:236`；`lib/providers/state/navigation.dart:84` | `src/pages/about.rs:283` 的 `tools_page(&UiPalette)`；`:330`–`:340` 六条写死行；`:349`–`:376` 无交互组件；`src/route/mount.rs:151` 只传 `&UiPalette` |
| 5 | 11-widget dashboard registry：只有六卡固定顺序 + 固定细节块，缺 typed 身份/编辑期/平台门/持久化 | Core gap | `lib/enum/enum.dart:450`；`lib/views/dashboard/widget_registry.dart:7`、`:22`、`:53`、`:63`；`lib/models/config.dart:167`、`:239`；`lib/views/dashboard/edit.dart:32`、`:98`、`:405`；`lib/views/dashboard/dashboard.dart:54`、`:77`、`:116`、`:215`、`:294` | `crates/infiltrator-contract/src/overview_layout.rs:32`、`:53` 只有 8 变体 / 6 卡 `DEFAULT_ORDER`；`src/pages/overview_card_order.rs:101`、`:102`、`:119` 硬建六卡；`src/pages/overview.rs:636` 追加固定细节块；无 `DashboardWidget` 等价 owner |
| 6 | Shared page-chrome model：控制条散在页身卡片，没有统一的 typed AppBar/控制带投影 | presentation gap（现有逐页 intent/投影足够，无引擎能力缺失） | `lib/views/proxies/proxies.dart:38`、`:116`、`:153`、`:159`；`lib/views/connection/requests.dart:132`–`:138`；`lib/views/profiles/profiles.dart:104`、`:107`；`lib/views/logs.dart:126`–`:128`；`lib/views/dns_queries.dart:137`、`:139`、`:142`；`lib/views/resources.dart:183`、`:185` | `src/shell_scene.rs:293`、`:309`–`:328` 单一全局标题行；`src/chrome.rs:296`、`:329`–`:340` 全局 chrome；控制落在 `src/pages/proxies/scene.rs:61`、`:67`、`:113`、`:115`、`src/pages/profiles.rs:523`–`:539`、`src/pages/logs.rs:229`、`:231`、`src/pages/connections.rs:433`、`:506`；`src/route/mount.rs:151` 的 `tools_page` 收不到快照。可对照 `crates/infiltrator-presentation/src/connection_page_chrome.rs` |

边界：本表不替代 `TODO.md` / `todos/` 台账，不生成任务 ID，不声明
standard/compact 双视口像素回执或真实交互取证；Bevy 截图 corpus 的已知缺口
仍按 `GAP-7.7` / `SPACEUTIL-022` 归档记录处理。b268 起 Bevy 随每一帧发布 `CAPTURE_CHROME`
外壳几何回执，并新增 `SPACEUTIL-024`（四项已量到的外壳几何缺陷）与 `SPACEUTIL-025`
（节点网格形态在三个表面各写一遍）两条实测缺口，见 `todos/CODE_TODO.md` §0.2。
