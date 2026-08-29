# 功能域地图

本表按用户功能而不是按目录排列。目录是实现位置，功能域才是协作和 TODO 的归属单位；一个功能域只能有一个 Rust 逻辑 owner，多个 UI 是它的消费者。

| 功能域 | 用户意图 | 当前主要 owner | UI / 宿主入口 | 重整重点 |
| --- | --- | --- | --- | --- |
| Core 生命周期 | 启动、停止、重启、状态检查、健康探测 | `mihomo-platform` + `infiltrator-desktop::runtime` | Iced、Tauri、Android `MihomoHost` | CORE-001/002：统一 session、readiness、generation |
| Profile 与订阅 | 导入、编辑、删除、切换、订阅更新 | `infiltrator-core::profiles/subscription`、admin scheduler | Iced、Admin Web、Android | FUNC-001：统一 profile 命令和重建结果 |
| 代理与路由 | 代理模式、代理组/节点切换、延迟测速 | `mihomo-api::proxy`、`connection` | Iced Runtime、Admin Runtime、Android Proxies | FUNC-002：共享节点身份、测速状态和排序语义 |
| 运行态诊断 | connections、logs、traffic、memory、出口 IP | `mihomo-api` + `infiltrator-admin` runtime handlers | Iced Runtime、RuntimePanel、Android Connections/Overview | FUNC-003：快照/流式数据有界，三端同语义 |
| DNS / Fake-IP / TUN | 读取、校验、保存、清缓存、VPN/TUN 控制 | `infiltrator-core::{dns,fake_ip,tun}` | Iced、Admin Web、Android Settings/VPN | FUNC-004：字段能力矩阵和配置事务 |
| Rules / Providers / Sniffer | 规则列表、provider 更新、JSON 高级编辑 | `infiltrator-core::{rules,proxy_providers,sniffer}` | Iced、Admin Web、Android Rules | FUNC-005：区分结构化编辑与原始 JSON |
| WebDAV 同步 | 保存、测试、手动同步、冲突处理 | `mihomo-dav-sync/*` + admin/Android orchestration | Iced、Admin Web、Android | FUNC-004：单一同步生命周期和冲突结果 |
| Core 版本交付 | 查询、下载、校验、安装、切换、回滚 | `mihomo-version` + `scripts/fetch-mihomo.sh` | Iced、Admin Web、CI/package、Android build | CORE-006 / UP-001：版本 manifest、digest、回滚 |
| 系统集成 | 系统代理、自启动、托盘、文件选择、权限 | `mihomo-platform`、`infiltrator-desktop`、各宿主 | Iced tray、Tauri tray、Android VPN | PLATFORM-003 / UI-005：native adapter 与业务解耦 |
| 多语言与主题 | 语言、主题、错误文案、无障碍提示 | `infiltrator-shared` + 各 UI theme/locales | Iced、Tauri/Web、Android resources | UI-006：文案 key 和失败状态不分叉 |

## 当前表面的职责

| Surface | 定位 | 应该做 | 不应该做 |
| --- | --- | --- | --- |
| Iced | 主桌面客户端 | 完整桌面流程、托盘、运行态、内核管理 | 直接决定 mihomo API/配置语义 |
| Tauri + Config Manager | 兼容/次级桌面入口 | 管理 Web、深度配置编辑、已有功能兼容 | 新建一套与 Iced 不同的 use-case |
| Android Compose | 移动伴侣 | VPN/TUN、移动导航、权限和后台生命周期 | 直接复制桌面文件/进程模型 |
| mihomo dashboard dist | 外部运行态面板 | 消费 mihomo controller 提供的既有能力 | 参与 MusicFrog profile/config ownership |

## 一条功能的完成定义

功能域只有同时满足以下条件，才能从 `TODO.md` 的开放项移入完成项：

1. Rust owner 和单一写入口明确；
2. Iced、Tauri/Web、Android 的 surface decision 已登记；
3. 成功、失败、取消、超时、不可用和版本不兼容有 typed 结果；
4. 至少有不依赖真实 mihomo 进程的行为测试；
5. 适用的目标平台、打包或真实 core smoke 已验证；
6. `USAGE_SPEC.md` 和对应 UI 文档没有继续描述旧行为。
