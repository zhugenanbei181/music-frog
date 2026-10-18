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

## 四、开源独立发布、多 UI 模板与发布自动化生态（16条 / 28pt）

> 1.0 中性内核（原 4.1–4.3，PUB-001～003）已迁入[现在佛](CODE_TODO.md)作为 1.0 主工作流；本板保留 UI 呈现标准化、四套模板脚手架与开源发布自动化。

### 4.4 UI 消费端管道标准化与 Presentation 纯净化（已全部迁入现在佛推进）

### 4.5 多 UI 模板化与工程脚手架（已全部迁入现在佛推进）

### 4.6 开源合规发布、交付文档与发布自动化生态（9条 / 15pt，PUB-006.2/006.4 已迁入现在佛推进）
- [ ] PUB-006.1 [kind: infra] [priority: P1] [points: 2] [owner: CI] Crates.io 自动化发布 CI 工作流
  - 产物：GitHub Actions 发布流水线；包含发布前版本拓扑校验、Dry-run 打包、签名校验与顺序发布。
  - 依赖：PUB-001.1～PUB-003.3。
  - 验收：本地与 CI 环境下执行 dry-run 发布成功，依赖图谱无环且版本严格对齐。
- [ ] PUB-006.6 [kind: infra] [priority: P1] [points: 2] [owner: docs] Docs.rs 预览与公共 API 文档质量门
  - 产物：配置全中性 crate 的 docs.rs 渲染模拟与 API 文档门禁；七个 crate 开启 `#![warn(missing_docs)]`，公开接口提供标准 Rustdoc 注释与 doctest。
  - 依赖：PUB-006.1、PUB-006.2。
  - 验收：`cargo doc --all-features --no-deps` 零警告，intra-doc links 全部正确校验；公开 API 文档覆盖率 100% 且包含可执行 doctest。
- [ ] PUB-006.7 [kind: mechanism] [priority: P1] [points: 2] [owner: test-infra] 无头沙盒集成测试与混沌容错验证
  - 产物：针对中性运行时建立无头 CI 集成沙盒与混沌测试套件；覆盖高频网络波动、内核进程猝死拉起、锁竞争与损坏配置热恢复。
  - 依赖：PUB-003.4、PUB-006.1。
  - 验收：异常注入场景下状态机 100% fail-closed、租约安全回收、内存无泄漏且能够自愈，沙盒集成测试全部通过。
- [ ] PUB-006.8 [kind: evidence] [priority: P1] [points: 2] [owner: core/drivers] 双引擎驱动真实二进制本地/CI Smoke 取证
  - 产物：激活 `mihomo-driver` 与 `singbox-driver` 中预置的真实二进制 smoke 测试套件；实现驱动在 CI 环境下自动加载二进制并执行本地通信测试。
  - 依赖：PUB-002.1、PUB-002.3、PUB-006.1。
  - 验收：在 Linux/Windows 真实进程环境下完成 API 握手、配置写入、节点测速与流量分发冒烟验证，取得真实二进制执行证据。
- [ ] PUB-006.9 [kind: mechanism] [priority: P1] [points: 2] [owner: examples] 最小化用例展厅 (Examples Gallery)
  - 产物：在 `examples/` 目录交付 4 套独立极简用例（`minimal-cli`、`minimal-slint`、`minimal-iced`、`minimal-bevy`）；演示无需脚手架时直接引用中性 crate 的最简范式。
  - 依赖：PUB-005.1～PUB-005.3、PUB-004.2。
  - 验收：四个极简 example 工程独立 `cargo run --example` 成功，代码量均 ≤200 行，无多余外部依赖并接入 CI 编译守卫。
- [ ] PUB-006.10 [kind: infra] [priority: P1] [points: 2] [owner: CI] 多架构交叉构建自动化与 SemVer 兼容守卫
  - 产物：自动化跨架构编译流水线（涵盖 Linux/Windows/Android aarch64 与 x86_64）与 `cargo-semver-checks` 门禁集成。
  - 依赖：GAP-1.1、PUB-006.1。
  - 验收：四大目标架构交叉编译 100% 通过；SemVer 门禁能够拦截中性 crate 公开 API 的隐式破坏性变更。
- [ ] PUB-006.11 [kind: evidence] [priority: P1] [points: 1] [owner: docs/qa] 开发者全生命周期演练与干跑审计
  - 产物：在全新纯净容器中模拟外部开发者全流程旅程（克隆代码 -> 查看文档 -> 脚手架生成 -> 运行工程 -> 接入驱动 -> 调试配置）；输出端到端审计报告。
  - 依赖：PUB-005.5、PUB-006.4、PUB-006.9。
  - 验收：第三方开发者旅程零阻滞，全链路安装与交互用时 ≤5 分钟，所有体验毛刺清零并形成归档回执。
- [ ] PUB-006.12 [kind: release] [priority: P1] [points: 1] [owner: release] 1.0-RC 候选版本基线冻结与预发布
  - 产物：锁定七大中性 crate 版本为 `1.0.0-rc.1`；生成首个 release candidate 校验和清单、GPG 签名草案与预发布发布说明（Release Notes）。
  - 依赖：PUB-006.1～PUB-006.11。
  - 验收：七个中性 crate API 签名完全冻结；预发布打包 dry-run 100% 成功；0.6x 开源发布与生态体系完备收官，正式进入 0.7x 质量与安装包阶段。
- [ ] PUB-006.5 [kind: evidence] [priority: P1] [points: 1] [owner: release] 1.0.0 首发布回执
  - 产物：`v1.0.0` tag、release notes、crates.io 七个中性 crate（contract/ports/profile-store/mihomo-driver/singbox-driver/engine-runtime/ui-presentation）顺序发布回执与 docs.rs 构建链接。
  - 依赖：PUB-006.1、PUB-006.3、PUB-006.4、PUB-006.12。
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
