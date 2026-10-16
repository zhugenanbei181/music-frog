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
