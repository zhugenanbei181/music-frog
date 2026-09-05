# 功能域地图

本表按用户功能而不是按目录排列。目录是实现位置，功能域才是协作和 TODO 的归属单位；一个功能域只能有一个 Rust 逻辑 owner，Iced 与 Bevy UI 双端作为对等消费者同步呈现。

| 功能域 | 用户意图 | 当前主要 owner | UI / 宿主入口 | 重整重点 |
| --- | --- | --- | --- | --- |
| Core 生命周期 | 启动、停止、重启、状态检查、健康探测 | `infiltrator-application::CoreApplication` + `infiltrator-ports`；具体进程在 host/composition | Iced、Bevy UI、Android `MihomoHost` | CORE-001/002：统一 session、readiness、generation |
| Profile 与订阅 | 导入、编辑、删除、切换、订阅更新、聚合 | `infiltrator-domain::{profiles,subscription}` + `infiltrator-application::ProfileApplication`；文件/HTTP 在 core adapter | Iced、Bevy UI、Android、CLI | FUNC-001：统一 profile 命令和重建结果 |
| 代理与路由 | 代理模式、代理组/节点切换、延迟测速 | `infiltrator-application::ProxyApplication` + `RuntimeGateway`；wire 实现在 `mihomo-api` | Iced Proxies、Bevy Proxies、Android Proxies | FUNC-002：共享节点身份、测速状态和排序语义 |
| 运行态诊断 | connections、logs、traffic、memory、出口 IP | domain projection + `RuntimeGateway` / `NetworkApplication`；HTTP 在 outbound adapter | Iced Runtime、Bevy Runtime、Android Connections/Overview | FUNC-003：快照/流式数据有界，双端同语义 |
| DNS / Fake-IP / TUN | 读取、校验、保存、清缓存、VPN/TUN 控制 | domain schema + `ConfigurationApplication`；Fake-IP cache/TUN 在 typed host port | Iced DNS、Bevy DNS、Android Settings/VPN | FUNC-004：字段能力矩阵和配置事务 |
| Rules / Providers / Sniffer | 规则列表、provider 更新、Tracer 沙盒、JSON 编辑 | `infiltrator-domain::{rules,proxy_providers,sniffer}` + `infiltrator-application::configuration_application` | Iced Rules、Bevy Rules、Android Rules | FUNC-005：区分结构化编辑与原始 JSON |
| WebDAV 同步 | 保存、测试、手动同步、三向冲突处理 | `mihomo-dav-sync/*`；application sync facade 仍在迁移，当前 admin/Android 保留 host orchestration | Iced Sync、Bevy Sync、Android | FUNC-004：单一同步生命周期和冲突结果 |
| Core 版本交付 | 查询、下载、校验、安装、切换、回滚 | `mihomo-version` + `scripts/fetch-mihomo.sh` | Iced Settings、Bevy Settings、CI/package | CORE-006 / UP-001：版本 manifest、digest、回滚 |
| 系统集成 | 系统代理、自启动、托盘、悬浮窗、权限 | `mihomo-platform`、`infiltrator-desktop`、各宿主 | Iced tray/HUD、Bevy tray/HUD、Android VPN | PLAT-003 / UI-005：native adapter 与业务解耦 |
| 多语言与主题 | 语言、主题、错误文案、无障碍提示 | `infiltrator-shared` + 各 UI theme/locales | Iced Theme、Bevy Theme、Android resources | UI-006：文案 key 和失败状态不分叉 |

## 表面的职责与同步演进定位

详见最高主控台账 [DUAL_SURFACE_PARITY_MASTER_PLAN.md](DUAL_SURFACE_PARITY_MASTER_PLAN.md)。

| Surface | 定位 | 应该做 | 不应该做 |
| --- | --- | --- | --- |
| Iced | 成熟主桌面客户端 | 完整桌面流程、托盘、悬浮窗、高保真运行态、内核管理 | 私自发明底层 API 语义；与 Bevy UI 产生功能分叉 |
| Bevy UI | 战略统一主干 surface（桌面+移动跨平台） | 与 Iced 严格同步演进；消费同一套 shared 契约；支持多模态自适应布局 | 复制 Iced 的私有状态；滞后跟随；私自修改核心配置协议 |
| Android Compose | 移动原生伴侣 | VPN/TUN、移动导航、权限和前后台生命周期 | 直接复制桌面文件/进程模型 |

## 一条功能的完成定义

功能域只有同时满足以下条件，才能从开放项移入完成项：

1. Rust owner 和单一写入口明确；
2. Iced 与 Bevy UI 双端实现与 UI 表现已 100% 对齐；
3. 成功、失败、取消、超时、不可用和版本不兼容有 typed 结果；
4. 具备双端无头行为测试（`tests/gui/iced_*` 与 `tests/headless/*`）；
5. 适用的目标平台、打包或真实 core smoke 已验证；
6. `USAGE_SPEC.md`、`DUAL_SURFACE_PARITY_MASTER_PLAN.md` 记录已更新。
