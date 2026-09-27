# MusicFrog Despicable Infiltrator

> 一只不安分的音乐青蛙，安静地穿过每一道网络边界。

**MusicFrog Despicable Infiltrator**（音乐青蛙 · 卑鄙的渗透者）是一个以 [mihomo](https://github.com/MetaCubeX/mihomo) 为数据平面与代理内核的跨平台客户端。名字是玩笑，工程不是玩笑：它把「换一个更顺手的代理客户端」做成一件能把配置、内核和运行态长期托付出去的事。

它不是某个内核的换皮界面，也不是把网络操作散落在各端拼装起来的产品。所有对 mihomo 的控制都收敛到 Rust 的唯一产品边界——内核生命周期、配置、版本、REST/WebSocket 控制，以及面向界面的业务编排。界面只提交意图、读取不可变结果。

## 内涵：我们在意什么

1. **一个事实，一个权威源。** profile、配置与运行态各有且只有一个 canonical owner，其余都是可失效的投影或缓存；同一份真相不允许在多个前端各存一份。
2. **诚实的完成度。** 不支持就明确 `unsupported`，没验证过就不写「已验证」。不用空列表、默认值或藏起来的入口假装功能已经做完；成功、失败、超时、取消、无权限、版本不兼容都必须可区分。
3. **求同存异，不伪造平价。** 「相同」指用户意图、数据语义、失败语义与可达性；像素、布局、手势可以是差异。但每个差异都要挂在同一个共享意图上，并显式记成 `accepted difference` 或 `unsupported`。
4. **对等双主干。** 桌面主客户端与战略统一端不是「先后跟随」，而是严格同步演进，共用同一套 contract 与语义。

## 它长什么样

- **桌面主客户端**：原生 Rust + Iced，系统托盘、密集多栏操作与高保真运行态。
- **战略统一端**：Bevy UI，面向桌面 + 移动 + iOS 的统一 surface。
- **移动伴侣**：Android（Compose + UniFFI），承接 VPN/TUN、分应用路由与移动生命周期。

> 早期的 Tauri + Vue Web 客户端已于 `0.20` 退役，内嵌 admin server 保留 API-only 的诊断用途。台账见 [docs/TAURI_WEBUI_RETIREMENT_LEDGER.md](docs/TAURI_WEBUI_RETIREMENT_LEDGER.md)。

## 能做什么（用户可见）

- **配置与订阅**：多渠道导入、订阅定时更新、多源聚合、YAML 编辑器与实时语法预检。
- **代理与路由**：策略组分类与切换、节点收藏 / 排序 / 检索、批量延迟测速。
- **网络**：DNS / Fake-IP、TUN 多堆栈调度、系统代理注入与自愈、端口冲突探测与避让。
- **运行态**：连接、日志、实时流量波形、内存 / CPU、链路拓扑与出口 IP 诊断。
- **内核**：多通道版本交付、SHA256 校验、秒级回滚与崩溃自愈。
- **数据**：WebDAV 三向同步与冲突处理。
- **系统集成**：托盘、悬浮窗、自启动、服务模式提权、多语言与主题。
- **Android**：VPN / TUN、分应用路由、配置编辑 / 导入 / 订阅、运行态连接管理、DNS/TUN 高级字段、WebDAV 同步。

## 文档

- [USAGE_SPEC.md](USAGE_SPEC.md) — 面向用户的功能说明与使用指南（双语）。
- [docs/README.md](docs/README.md) — 文档中心：架构、功能地图、平台矩阵与上游规则。
- [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) — 分层边界与数据流。
- [docs/DUAL_SURFACE_PARITY_MASTER_PLAN.md](docs/DUAL_SURFACE_PARITY_MASTER_PLAN.md) — 双端同步演进的最高主控台账。
- [docs/FUNCTIONAL_MAP.md](docs/FUNCTIONAL_MAP.md) — 按功能域查找唯一 owner。
- [docs/MIHOMO_CORE.md](docs/MIHOMO_CORE.md) — Rust ↔ mihomo 的生命周期、控制与发布契约。
- [docs/UPSTREAM.md](docs/UPSTREAM.md) — 上游依赖版本与升级流程。

构建、测试与代码规范属于工程侧约定，单独维护；本 README 只描述产品本身。
