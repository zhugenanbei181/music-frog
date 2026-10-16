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

## 四、开源独立发布与多 UI 模板框架架构（22条 / 49pt）

### 4.1 基础契约与中性端口解耦（3条 / 7pt）
- [ ] PUB-001.1 [kind: mechanism] [priority: P1] [points: 2] [owner: contract] 抽取零外部重依赖的通用 `proxy-core-contract`
  - 产物：独立契约 crate，收敛 EngineId、CoreLifecycle、Outbound/Group 模型、RoutingIntent、CoreScope 与 Failure；完全剔除应用私有字段。
  - 依赖：当前双后端 Core 契约稳定。
  - 验收：仅保留 serde 等轻量依赖；`cargo check -p proxy-core-contract` 独立通过，无任何产品私有符号。
- [ ] PUB-001.2 [kind: mechanism] [priority: P1] [points: 2] [owner: ports] 抽取中性端口 `proxy-core-ports`
  - 产物：独立端口 crate，定义 CoreProcess、CoreReadiness、OutboundPort、RoutingIntentPort 与 ProfileDocumentCodecPort 等 Trait。
  - 依赖：PUB-001.1。
  - 验收：纯 Trait 定义，不引入任何 Tokio runtime、channel 或 UI 类型，多引擎端口可独立实现。
- [ ] PUB-001.3 [kind: mechanism] [priority: P1] [points: 3] [owner: profile-store] 抽象通用 ProfileStore 与租约并发接口
  - 产物：解耦产品私有 Age 凭据与硬编码路径，提取基于 CAS 与版本租约锁的通用 `proxy-profile-store` 存储与事务接口。
  - 依赖：PUB-001.1、PUB-001.2。
  - 验收：单测覆盖活动租约锁竞争、并发 CAS 冲突与安全原子替换，无业务私有路径泄露。

### 4.2 双引擎专属驱动 crate 纯净化与打包（5条 / 11pt）
- [ ] PUB-002.1 [kind: mechanism] [priority: P1] [points: 3] [owner: mihomo-api + mihomo-process] Mihomo 进程与 API 客户端整合为独立 `mihomo-rs`
  - 产物：输出独立 `mihomo-rs` crate；内聚 HTTP REST、WS 流式日志/连接、进程探测、PID 与精确终态管理；通过 feature 按需裁剪。
  - 依赖：PUB-001.1、PUB-001.2。
  - 验收：提供无侵入纯客户端使用方式，在无上层包装下通过真实与 mock 进程/API 单元测试。
- [ ] PUB-002.2 [kind: mechanism] [priority: P1] [points: 2] [owner: mihomo-config] Mihomo YAML 配置 Codec 与校验独立化
  - 产物：独立的配置处理模块，支持内存 AST 解析、配置合法性校验及 Mihomo 原生 dry-run 检查。
  - 依赖：PUB-001.1、PUB-001.2。
  - 验收：纯内存解析通过标准与畸形 YAML 校验，输出精确的 typed DocumentValidation 结果。
- [ ] PUB-002.3 [kind: mechanism] [priority: P1] [points: 3] [owner: singbox-api + singbox-process] sing-box 进程与 Clash API 客户端整合为独立 `singbox-rs`
  - 产物：输出独立 `singbox-rs` crate；实现 Clash API 通信、进程启动/停止、child process 守护与信号回收。
  - 依赖：PUB-001.1、PUB-001.2。
  - 验收：独立拉起 sing-box 二进制、探测 readiness、通过 Clash API 切换 Selector 节点并安全退出。
- [ ] PUB-002.4 [kind: mechanism] [priority: P1] [points: 2] [owner: singbox-config] sing-box JSON 配置 Codec 与校验独立化
  - 产物：支持 sing-box 原生 JSON 配置模型、schema 校验及原生二进制 `check` 命令事务适配。
  - 依赖：PUB-001.1、PUB-001.2。
  - 验收：单测覆盖合法 JSON 接受与非法规则/端口拦截，返回结构化诊断信息。
- [ ] PUB-002.5 [kind: evidence] [priority: P1] [points: 1] [owner: mihomo-* + singbox-*] 双引擎独立 crate 最小可用 example 与单测
  - 产物：为 `mihomo-rs` 与 `singbox-rs` 各编写独立的 `examples/`（命令行拉起与节点查询），补齐独立 nextest 覆盖。
  - 依赖：PUB-002.1～PUB-002.4。
  - 验收：在完全脱离本仓库其他包的环境下，`cargo run --example` 成功演示内核生命周期。

### 4.3 统一双引擎调度器与高层 Facade SDK（4条 / 11pt）
- [ ] PUB-003.1 [kind: mechanism] [priority: P1] [points: 3] [owner: composition::factory] 双引擎工厂注册器 `EngineBackendFactoryRegistry` 泛型化
  - 产物：抽象动态引擎注册机制，解耦编译期硬编码，允许应用自由注入单引擎或双引擎 adapter。
  - 依赖：PUB-001.2、PUB-002.1、PUB-002.3。
  - 验收：注册器支持动态挂载/卸载引擎，对未注册引擎强类型 fail-closed 拦截并拒绝操作。
- [ ] PUB-003.2 [kind: mechanism] [priority: P1] [points: 3] [owner: application::owner_switch] 双引擎原子切换事务抽象为通用的 `EngineSwitchCoordinator`
  - 产物：将基于 CAS 租约与 Generation/Revision 围栏的无缝切换抽象为独立通用组件；支持切换失败时的原主自动恢复。
  - 依赖：PUB-003.1。
  - 验收：nextest 覆盖正常切换与注入故障回滚，证明新旧进程生命周期严格互斥且终态可查。
- [ ] PUB-003.3 [kind: mechanism] [priority: P1] [points: 3] [owner: runtime facade] 开发者友好的高层 Facade SDK `proxy-engine-runtime`
  - 产物：提供开箱即用的 Builder 模式 API（如 `ProxyRuntimeBuilder`）；隐藏复杂的 Token/Generation 传递细节，对外暴露直观的异步操控入口。
  - 依赖：PUB-003.2。
  - 验收：第三方开发者在 10 行以内完成初始化并监听状态流，单测与文档示例验证通过。
- [ ] PUB-003.4 [kind: evidence] [priority: P1] [points: 2] [owner: test-support] 双引擎 Conformance 跨平台验证测试套件
  - 产物：抽取通用的适配器一致性测试套件；规范进程状态机、配置应用、节点读写、错误回读的测试断言。
  - 依赖：PUB-003.1～PUB-003.3。
  - 验收：Conformance 覆盖 Mihomo 与 sing-box 真实与 fake 适配器，第三方适配器跑通即可证明兼容。

### 4.4 UI 消费端管道标准化与 Presentation 纯净化（3条 / 6pt）
- [ ] PUB-004.1 [kind: mechanism] [priority: P1] [points: 2] [owner: presentation] 跨 UI 纯视图模型投影 `proxy-ui-presentation` 独立化
  - 产物：纯数据模型与国际化文案折叠库，零 UI 框架依赖，仅负责将底层快照转换为各表面易读的 ViewModel。
  - 依赖：PUB-001.1。
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

### 4.6 开源合规发布与交付文档（2条 / 3pt）
- [ ] PUB-006.1 [kind: infra] [priority: P1] [points: 2] [owner: CI] Crates.io 自动化发布 CI 工作流
  - 产物：GitHub Actions 发布流水线；包含发布前版本拓扑校验、Dry-run 打包、签名校验与顺序发布。
  - 依赖：PUB-001.1～PUB-003.3。
  - 验收：本地与 CI 环境下执行 dry-run 发布成功，依赖图谱无环且版本严格对齐。
- [ ] PUB-006.2 [kind: evidence] [priority: P1] [points: 1] [owner: docs] 架构设计说明书与 UI 框架选型指南
  - 产物：编写顶层架构说明、开发者接入指南、以及各 UI（Slint/Iced/Bevy/Android）在性能与开销上的选型对比。
  - 依赖：PUB-005.1～PUB-005.5。
  - 验收：文档包含快速上手代码、架构图与不同 UI 内存/包体 benchmark 详实数据。
