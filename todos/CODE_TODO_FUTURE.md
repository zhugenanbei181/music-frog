# MusicFrog 工程 TODO · 未来佛（可延后的增强项）

> 层级：L4

## 0. 编写规则

- 本板只放不阻塞 Mihomo/sing-box 双后端、Kotlin VPNService/权限、Compose/Bevy/Slint 页面、桌面服务管理或 Windows/Linux/Android 基础构建的增强项。
- 必须有具体 task、owner、范围、依赖与验收；必要功能和交付链路进入[现在佛](CODE_TODO.md)，实现后待验证进入[过去佛](CODE_TODO_PAST.md)。
- 状态 `Blocked` 必须写明暂缓增强所需的具体前置；迁移时保持 task ID、owner、验收和 points。

## 一、视觉与性能增强（2条 / 4pt）

- [ ] BANDROID-016.2 [kind: mechanism] [priority: P1] [points: 3] [owner: bevy-widgets::chart] [status: Planned] 按需 3D/2.5D 地球与流量弧线
  - 产物：拖拽、节点地理标记、流量弧线；保留二维和 Eco 回退、低电量/热状态/减少动效降级。
  - 依赖：BANDROID-016.1；013 图形基础。
  - 验收：操作沿用真实节点身份；离页释放资源、静置休眠。
- [ ] BANDROID-016.3 [kind: evidence] [priority: P1] [points: 1] [owner: bevy-widgets::chart] [status: Planned] 地球桌面帧/内存报告
  - 产物：显示/关闭模式的帧/内存报告；增强关闭后资源释放核查。
  - 依赖：BANDROID-016.2。
  - 验收：报告绑定同一源码身份；等价操作面通过。

## 二、发行签名与安装包便利（5条 / 10pt）

- [ ] BANDROID-015.2 [kind: evidence] [priority: P0] [points: 2] [owner: android/app Gradle + release] [status: Blocked] release universal APK 签名/identity 校验
  - 产物：Compose/Bevy/Slint release universal APK 通过 identity/aapt 与签名校验。
  - 依赖：BANDROID-039 持久 keystore。
  - 验收：签名指纹稳定；identity/aapt 全部匹配。
- [ ] BANDROID-039.1 [kind: infra] [priority: P0] [points: 3] [owner: release.yml + build.gradle.kts] [status: Blocked] 持久 release keystore 接入
  - 产物：用户/组织持有的 release keystore 与受保护 CI secrets；无 secret 时发布 job 清晰失败，禁止静默退回 debug key。
  - 依赖：用户提供并安全配置 keystore/alias/password。
  - 验收：签名产物可复现；私钥不入仓库/日志/APK 输出。
- [ ] BANDROID-039.2 [kind: evidence] [priority: P0] [points: 1] [owner: release workflow] [status: Planned] 连续两候选签名一致
  - 产物：连续两个候选 release 签名证书指纹一致；前版安装后 `adb install -r` 升级可用。
  - 依赖：BANDROID-039.1。
  - 验收：升级验证记录（设备部分按对应任务实机条件记录）。
- [ ] APPID-001.3 [kind: evidence] [priority: P0] [points: 2] [owner: packaging/linux] [status: Planned] Linux 最终 Deb/AppImage 产物
  - 产物：Linux 最终 Deb/AppImage 构建与验收。
  - 依赖：APPID-001.2。
  - 验收：包名/图标/二进制来自登记表；安装冒烟通过。
- [ ] APPID-001.4 [kind: evidence] [priority: P0] [points: 2] [owner: packaging/windows] [status: Planned] Windows NSIS/Wix 产物
  - 产物：Windows NSIS/Wix 最终产物构建与验收。
  - 依赖：APPID-001.2。
  - 验收：版本/名称/图标来自登记表；安装冒烟通过。

## 三、质量工具增强（4条 / 9pt）

- [ ] BANDROID-044.1 [kind: infra] [priority: P1] [points: 2] [owner: android gradle] [status: Planned] Android Lint 接入
  - 产物：Compose/Bevy Debug 变体 lint task 注册并接入 PR/push workflow；不把 assemble 当 lint 结果。
  - 依赖：BANDROID-042/043 存量违规清理；当前 AGP 9.3.1/JDK 17 的 Lint crash 要等 AGP/KGP 官方兼容矩阵覆盖可修复 patch 后再启用。
  - 验收：lint 结果分项报告。
- [ ] BANDROID-044.2 [kind: infra] [priority: P1] [points: 2] [owner: android gradle + scripts/quality] [status: Planned] ktlint/Detekt 门
  - 产物：ktlint 与 Detekt 接入；清理盘点违规后强制；无永久 baseline/文件级 suppression/抬高阈值。
  - 依赖：BANDROID-044.1。
  - 验收：违规清零后强制生效；正反例可复现。
- [ ] BANDROID-044.4 [kind: infra] [priority: P1] [points: 3] [owner: scripts/quality] [status: Planned] 架构红线守卫
  - 产物：禁止生产 readiness sleep、主线程阻塞、无界 GlobalScope、空 catch、吞取消、`!!` 和多状态 Boolean wire。
  - 依赖：BANDROID-044.2。
  - 验收：窄例外有局部说明和行为回归；正反例通过。
- [ ] BANDROID-044.5 [kind: infra] [priority: P1] [points: 2] [owner: CI + 本机检查入口] [status: Planned] 质量门接线并自测
  - 产物：Android workflow 与本机入口分别运行两 flavor lint、ktlint、Detekt、文件预算/架构守卫。
  - 依赖：BANDROID-044.1~044.4。
  - 验收：正反例验证任务注册/排除/放行/拒绝；真实执行后才称质量门接通。

## 四、开源独立发布与多 UI 模板框架架构（13条 / 24pt）

> 1.0 中性内核（原 4.1–4.3，PUB-001～003）已迁入[现在佛](CODE_TODO.md)作为 1.0 主工作流；本板保留 UI 呈现标准化、四套模板脚手架与开源发布自动化。

### 4.4 UI 消费端管道标准化与 Presentation 纯净化（3条 / 6pt）
- [ ] PUB-004.1 [kind: mechanism] [priority: P1] [points: 2] [owner: presentation] [status: InProgress] 跨 UI 纯视图模型投影 `lurker-ui-presentation` 独立化
  - 产物：纯数据模型与国际化文案折叠库，零 UI 框架依赖，仅负责将底层快照转换为各表面易读的 ViewModel。
  - 裁决：`infiltrator-presentation` 82 个生产模块中只有 5 个（1,063 行）当前零契约改动；其余 77 个依赖 36k 行 `infiltrator-contract`。本条目只交付「种子 crate + `TextCatalog` 端口」，全量迁移拆为 B2 文案统一 / B3 `surface_snapshot` 契约下移，禁止在 PUB-004.1 内承诺 20.4k 行迁移。
  - 依赖：PUB-001.1。
  - 进度（2026-10-17，本地 L）：B1 完成 —— `lurker-ui-presentation` 已建（仅依赖 `lurker-core-contract`），迁入 `failure_projection`/`proxy_group_order_editor`/`routing_intent_actions`/`log_follow`/`dns_leak_actions` 与 `TextCatalog` 端口，26 个消费文件已改接；中性 crate 21 测试、六表面 1246 passed / 2 skipped。B2（45 个 i18n 模块统一 `&dyn TextCatalog`）与 B3（`PageStatus`/`PageData`/`PageId`/`SurfaceOrigin` 下移中立契约）未开始。
  - 验收：`cargo check` 零图形库依赖；单元测试覆盖各页面状态折叠，禁止私有逻辑残留。
- [ ] PUB-004.2 [kind: mechanism] [priority: P1] [points: 2] [owner: contract] 标准单向 UI 通信管道协议规范化
  - 产物：固化 `SurfaceReader`（只读快照流）与 `CommandSink`（强类型意图通道），作为模板框架的唯一前后端接口。
  - 依赖：PUB-004.1、PUB-003.3。
  - 验收：UI 表面与底层交互仅能通过快照拉取与意图投递完成，杜绝任何 UI 层直调 Core 副作用。
- [ ] PUB-004.3 [kind: validation] [priority: P1] [points: 2] [owner: UI crates] 现有全 UI 表面接入统一管道回归验证
  - 产物：Iced、Bevy、Slint 桌面表面与 Android 呈现端纯粹作为消费者接入统一管道。
  - 依赖：PUB-004.2。
  - 验收：四大 UI 表面全部通过编译与无副作用回归，无任何 UI 私有业务状态推导。

### 4.5 多 UI 模板化与工程脚手架（5条 / 11pt）
- [ ] PUB-005.1 [kind: mechanism] [priority: P1] [points: 2] [owner: templates/slint] Slint 桌面轻量模板工程规范化
  - 产物：独立的 Slint 客户端工程模板；包含轻量现代 UI 布局、页面路由与单向状态绑定，可独立打包分发。
  - 依赖：PUB-004.2。
  - 验收：独立工程直接 `cargo run` 成功，内存占用低且页面切换无卡顿。
- [ ] PUB-005.2 [kind: mechanism] [priority: P1] [points: 2] [owner: templates/iced] Iced 桌面响应式模板工程规范化
  - 产物：独立的 Iced 客户端工程模板；演示标准 Elm 架构、响应式状态管理、自定义样式与系统托盘接入。
  - 依赖：PUB-004.2。
  - 验收：独立工程支持跨平台桌面窗口、托盘事件响应与明暗主题切换。
- [ ] PUB-005.3 [kind: mechanism] [priority: P1] [points: 2] [owner: templates/bevy] Bevy 0.20 极端性能/ECS 模板工程规范化
  - 产物：独立的 Bevy 客户端工程模板；展示 ECS 架构与 Core 管道的高性能绑定、headless 测试与无头模式支持。
  - 依赖：PUB-004.2。
  - 验收：独立工程在无头环境与桌面窗口下均能稳定排帧，通过 ECS 调度验证。
- [ ] PUB-005.4 [kind: mechanism] [priority: P1] [points: 3] [owner: templates/android] Android 双引擎 VPN 移动端模板工程规范化
  - 产物：独立的 Android 工程模板；包含 Compose UI、Rust JNI 桥接、VPNService 权限与前台服务拓扑。
  - 依赖：PUB-004.2。
  - 验收：Gradle 直接组装 Debug APK 成功，安装后能正常拉起 VPN 授权并展示双引擎状态。
- [ ] PUB-005.5 [kind: infra] [priority: P1] [points: 2] [owner: cargo-generate] 交互式工程生成脚手架配置
  - 产物：配置 `cargo-generate` 规则与参数问答，支持开发者一键选择特定 UI 表面裁剪生成新项目。
  - 依赖：PUB-005.1～PUB-005.4。
  - 验收：执行 `cargo generate` 挑选单一 UI 框架后，生成的子项目立即可编译运行且无其他 UI 残留。

### 4.6 开源合规发布与交付文档（5条 / 7pt）
- [ ] PUB-006.1 [kind: infra] [priority: P1] [points: 2] [owner: CI] Crates.io 自动化发布 CI 工作流
  - 产物：GitHub Actions 发布流水线；包含发布前版本拓扑校验、Dry-run 打包、签名校验与顺序发布。
  - 依赖：PUB-001.1～PUB-003.3。
  - 验收：本地与 CI 环境下执行 dry-run 发布成功，依赖图谱无环且版本严格对齐。
- [ ] PUB-006.2 [kind: evidence] [priority: P1] [points: 1] [owner: docs] 架构设计说明书与 UI 框架选型指南
  - 产物：编写顶层架构说明、开发者接入指南、以及各 UI（Slint/Iced/Bevy/Android）在性能与开销上的选型对比。
  - 依赖：PUB-005.1～PUB-005.5。
  - 验收：文档包含快速上手代码、架构图与不同 UI 内存/包体 benchmark 详实数据。
- [ ] PUB-006.3 [kind: infra] [priority: P1] [points: 2] [owner: release] 开源合规与可发布基线
  - 产物：根 `LICENSE-MIT` + `LICENSE-APACHE`（MIT OR Apache-2.0）；每个中性 crate 的 `description`/`license`/`repository`/`README`；产品 crate 全量 `publish = false` 审计；中性 crate 独立版本拓扑（1.0.x，产品保持 0.4x）与发布顺序。
  - 依赖：PUB-001.1～PUB-003.3。
  - 验收：`cargo package --list` 无产品私有文件；每个中性 crate `cargo publish --dry-run` 通过；`cargo metadata` 证明中性 crate 不依赖任何产品 crate。
- [ ] PUB-006.4 [kind: evidence] [priority: P1] [points: 1] [owner: docs] 开源仓库治理文档
  - 产物：README（快速上手/特性/截图）、CONTRIBUTING、SECURITY、CODE_OF_CONDUCT，以及从产品仓迁移到中立内核的迁移指南。
  - 依赖：PUB-006.2、PUB-005.5。
  - 验收：文档链接全部可解析；新开发者按 README 能在干净环境跑通 example 与模板。
- [ ] PUB-006.5 [kind: evidence] [priority: P1] [points: 1] [owner: release] 1.0.0 首发布回执
  - 产物：`v1.0.0` tag、release notes、crates.io 七个中性 crate（contract/ports/profile-store/mihomo-driver/singbox-driver/engine-runtime/ui-presentation）顺序发布回执与 docs.rs 构建链接。
  - 依赖：PUB-006.1、PUB-006.3、PUB-006.4。
  - 验收：crates.io 六 crate 版本严格对齐；docs.rs 六页构建成功；回执绑定 tag 与 commit。

## 收口八问缺口（延迟项，6条 / 21pt）

- [ ] GAP-2.1 [kind: evidence] [priority: P2] [points: 2] [owner: release] Windows/Linux 安装包与系统集成验证
  - 产物：NSIS/AppImage/deb 产物与安装、托盘、TUN 回执。
  - 依赖：`GAP-3.1` 之外的发布流水线；需要 CI 或本机打包环境。
  - 验收：每个安装包有可复跑构建与安装回执；不达标项显式 Unsupported。
- [ ] GAP-2.2 [kind: evidence] [priority: P2] [points: 3] [owner: release] macOS 产物与签名
  - 产物：App bundle、签名、权限与安装运行回执。
  - 依赖：Apple target 解禁（当前由 `apple-ci-deferral-guard.py` 显式禁止）。
  - 验收：更新平台矩阵并取得目标产物证据。
- [ ] GAP-3.1 [kind: evidence] [priority: P1] [points: 3] [owner: android/release] Android APK 构建修复取证
  - 产物：cmdline-tools 重链修复后的 APK 与模拟器/真机回执。
  - 依赖：CI tag 或本机完整 SDK/Gradle。
  - 验收：APK 产物 + 启动/触摸/权限回执；否则保持 Unsupported。
- [ ] GAP-3.2 [kind: mechanism] [priority: P2] [points: 5] [owner: ios] iOS target 与应用骨架
  - 产物：`aarch64-apple-ios` 构建、应用包与 NetworkExtension 骨架。
  - 依赖：Apple target 解禁。
  - 验收：真实 target 构建 + 包结构检查；当前只有 626 行 host stub。
- [ ] GAP-4.1 [kind: evidence] [priority: P2] [points: 3] [owner: slint] Slint 功能验收与 parity
  - 产物：Slint 六个页面回调对应的功能验收与 parity cell。
  - 依赖：目标产物与交互证据。
  - 验收：parity 锚点 + 屏幕回执；否则在八问表保持「部分」。
- [ ] GAP-7.1 [kind: mechanism] [priority: P2] [points: 5] [owner: presentation] 跨 UI 页面模型去重
  - 产物：Iced/Bevy/Slint 共享的页面模型层，消灭各自实现的同义状态机。
  - 依赖：GAP-5.2 完成后评估。
  - 验收：同一页面在三个表面只保留一份状态推导；UI 只做渲染与事件投递。
