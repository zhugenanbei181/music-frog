# MusicFrog 工程 TODO · 未来佛

> 层级：L4
> 范围：**验收项**——实现已交付，但验收必须在**真实环境**里取回执；在环境到位之前，它们既不是未办结的实现工作，也不能办结退出。

## 0. 本板的口径

**未来佛收"因为没有实际环境而必须阻塞的验收项"**，判据三条同时成立：

1. **实现侧已经交付**（代码、契约、快照、页面、捕获路径都已落地，代码级证据齐备）；
2. **剩下的唯一动作是取环境回执**——真机 / 受控 CI / 真实发布产物 / 真实引擎二进制 / 真实相机 / 带指针的捕获台架；
3. **在本机无法以任何替代手段伪造该回执**——一旦能用代码级证据顶替，那它就属于现在佛或过去佛，不属于这里。

与相邻两板的分工：

| 板 | 收什么 | 下一个可执行动作 |
| --- | --- | --- |
| 现在佛 | 全部可推的实现工作 | 在本机就能做完的动作 |
| 过去佛 | 已交付、等统一批次或本机可取的回执 | 统一 nextest / 守卫 / 捕获批次 |
| 斗战胜佛 | 实现工作本身要等外部环境（secrets、真机、凭证） | 环境到位后才有第一个可执行动作 |
| **未来佛** | **验收项**：实现已交付，回执要真实环境 | 环境到位后取回执，回执到手即办结 |

**流转**：环境到位 → 取回执 → 回执齐备即办结退出并登记 archived points；回执证明实现有缺口 → 迁回现在佛（不是留在本板等人看）。

## 一、板上条目

- [ ] BEVY-AUTO-NAV-001 [kind: evidence] [priority: P1] [points: 2] [owner: infiltrator-bevy-ui + infiltrator-bevy-widgets] [status: Blocked] 官方方向导航的**自动**空间搜索回执
  - 产物：在真实应用（带真实相机与真实布局）里证明 Bevy 0.20 的官方自动空间搜索能把焦点移到几何上最近的邻居。
  - 阻塞方：headless 台架无法提供目标相机——`ComputedUiTargetCamera` 的相机字段是 crate 私有，无法在无头环境构造。现有 headless 证据走的是官方**手动**导航图 `DirectionalNavigationMap::add_edge`（官方声明边的通道），它证明的是"官方通道确实被接上"，不是"自动搜索确实可用"。
  - 验收：真实运行的应用里，方向键把焦点移到几何最近邻居的像素/事件回执；不得用手动导航图的证据顶替。
  - 事实依据：2026-10-21 接线官方方向导航时实测（官方导航确实移动 `InputFocus`，但无头环境给不出目标相机）。

- [ ] PIXEL-POINTER-001 [kind: evidence] [priority: P1] [points: 2] [owner: infiltrator-bevy-ui + infiltrator-iced] [status: Blocked] 指针驱动态的像素回执（hover 填充 / tooltip / 焦点环）
  - 产物：hover 填充、导航 tooltip、焦点环跟随这三类**只在真实指针下出现**的状态，各取双视口真实像素回执。
  - 阻塞方：捕获台架不注入指针，hover/焦点态在静态截图里不可成像（代码级证据已在各自条目里记明，但代码级证据不是像素证据）。
  - 验收：带指针的捕获台架（或等价实机取证）产出的 PNG，经 `visual_receipts.py` 严格验签并被验收认证为 `ready`。
  - 事实依据：2026-10-21 壳层与焦点真源两条交付的未证明层（`BEVYUI-001` 的 #4/#18、`BEVY-020.2` 的焦点环跟随）。

- [ ] SLINT-FOCUS-RIG-001 [kind: evidence] [priority: P2] [points: 1] [owner: infiltrator-slint] [status: Blocked] Slint `shell-keyboard-focus` 的焦点像素回执
  - 产物：Slint 表面工作台动作按钮的真实焦点环像素回执，使 `shell-keyboard-focus/slint` 这一格能被认证。
  - 阻塞方：Slint 的捕获路径只驱动产品回调（`invoke_workbench_action` / `invoke_workbench_draft_edited`），不发真实键/指针事件；工作台动作按钮也还没有画焦点环。
  - 验收：带键/指针的捕获台架下产出的焦点像素回执，经严格验签并被验收认证为 `ready`。
  - 事实依据：2026-10-22 Slint 载体页交付时逐格分类的结论（该格因外部环境保持 `pending`，未伪造激活）。


- [ ] APPID-001.4 [kind: evidence] [priority: P0] [points: 2] [owner: packaging/windows] [status: Blocked] Windows NSIS/Wix 产物
  - 产物：Windows NSIS/Wix 最终产物构建与验收。
  - 依赖：APPID-001.2。
  - 验收：版本/名称/图标来自登记表；安装冒烟通过。
  - 当前证据：登记表与 builder 脚本已交付；Windows 产物未构建。
  - 阻塞方：Windows 目标机（安装与卸载冒烟必须在真实 Windows 环境执行）。
  - 解锁条件：Windows CI 或本机产物与安装冒烟回执；取得后同次迁回未来佛办结。
  - 本板停留期间：builder 与登记表已闭环，无剩余本机可执行动作。

- [ ] BANDROID-015.2 [kind: evidence] [priority: P0] [points: 2] [owner: android/app Gradle + release] [status: Blocked] release universal APK 签名/identity 校验
  - 产物：Compose/Bevy/Slint release universal APK 通过 identity/aapt 与签名校验。
  - 依赖：BANDROID-039 持久 keystore。
  - 验收：签名指纹稳定；identity/aapt 全部匹配。
  - 当前证据：Debug 变体 identity/aapt 本地通过；release 签名未验证。
  - 阻塞方：持久 release keystore（同板 BANDROID-039.1）。
  - 解锁条件：keystore 就绪后的 release universal APK 签名与 identity 回执；取得后同次迁回过去佛办结。
  - 本板停留期间：Debug 变体本地核验已闭环，无剩余本机可执行动作。

- [ ] BANDROID-018.2 [kind: evidence] [priority: P0] [points: 1] [owner: CI] [status: Blocked] tag-only CI 实际全绿回执
  - 产物：受控 `v*` tag release 与 Android workflow 的完整 run 链接和分项证据；普通 push/PR 不运行。
  - 依赖：高成本 tag-only 工作流；CI-QUALITY-001～005；CORE-UI-002（全仓绿）。
  - 验收：tag run 中 nextest、构建、Clippy、APK identity 与 Android tests 全绿并归档。
  - 当前证据：本地 5200+ nextest 与 Clippy 0 警告全绿；受控 tag 流水线定义在 `.github/workflows/` 已对齐。
  - 阻塞方：受控 tag release runner 的 **Android 腿缺用户 keystore secrets**（`RELEASE_KEYSTORE_BASE64`／`RELEASE_KEYSTORE_PASSWORD`／`RELEASE_KEY_ALIAS`／`RELEASE_KEY_PASSWORD`），签名前置因此 fail closed，发布作业被跳过；`v0.51.0` tag 已打好（run `39477624601`），test-gate 与四条桌面腿全绿，缺的就是签名材料与 crates.io 令牌。
  - 解锁条件：用户授权打受控 `v*` tag 后取得的 workflow run 回执；取得后同次迁回过去佛办结。
  - 本板停留期间：本地全量门禁已闭环，无剩余本机可执行动作。

- [ ] BANDROID-019.2 [kind: evidence] [priority: P0] [points: 1] [owner: CI] [status: Blocked] manifest/aapt 守卫 CI 回执
  - 产物：`android-manifest-contract-guard.py` 与 aapt identity 检查在 CI 实际执行并留存回执。
  - 依赖：BANDROID-018.2。
  - 验收：回执绑定 run 与 APK 哈希。
  - 当前证据：本地 manifest guard 11/11 全通。
  - 阻塞方：受控 tag release runner。
  - 解锁条件：伴随 tag release 工作流的实际 runner 回执；取得后同次迁回过去佛办结。
  - 本板停留期间：本地 guard 全通，无剩余本机可执行动作。

- [ ] BANDROID-024.1 [kind: validation] [priority: P0] [points: 2] [owner: android gradle] [status: Blocked] AGP/Kotlin/Gradle 兼容复核
  - 产物：AGP 9.3.1、KGP 2.4.21、Gradle 9.7.0、JDK 17 的组合落在官方 fully-supported 矩阵；API 37 与 AGP 9.3 上限一致；SDK/NDK/JDK pins 由 BANDROID-020.1 唯一登记。
  - 依赖：BANDROID-020.1（本条同板阻塞）。
  - 验收：官方兼容矩阵复核；Gradle 配置与 Core host JVM 测试通过；版本漂移由 toolchain checker 拒绝。
  - 当前证据：183/183；本机 toolchain checker 通过。
  - 阻塞方：受控 tag release runner（AGP 9.3.1 + JDK 17 的 Lint crash 另行登记在 BANDROID-044.1）。
  - 解锁条件：真实 tag CI 证据回执；取得后同次迁回过去佛办结。
  - 本板停留期间：本机矩阵复核与 JVM 批次已闭环，无剩余本机可执行动作。

- [ ] BANDROID-038.2 [kind: evidence] [priority: P0] [points: 1] [owner: CI + release workflow] [status: Blocked] 每个 APK 的 aapt identity CI 检查
  - 产物：tag release 对每个最终 APK 的 aapt identity 检查实际接入并留存回执；普通 push/PR 不触发构建。
  - 依赖：已交付的 identity registry 与 guard（BANDROID-038.1）。
  - 验收：负例（drift APK）在 CI 被拒；回执绑定 APK SHA。
  - 当前证据：本地 identity registry 与 guard 已交付闭环。
  - 阻塞方：受控 tag release runner。
  - 解锁条件：tag release 工作流对各 APK 的 SHA 校验回执；取得后同次迁回过去佛办结。
  - 本板停留期间：本地 identity registry 与 guard 已闭环，无剩余本机可执行动作。

- [ ] BANDROID-039.2 [kind: evidence] [priority: P0] [points: 1] [owner: release workflow] [status: Blocked] 连续两候选签名一致
  - 产物：连续两个候选 release 签名证书指纹一致；前版安装后 `adb install -r` 升级可用。
  - 依赖：BANDROID-039.1。
  - 验收：升级验证记录（设备部分按对应任务实机条件记录）。
  - 当前证据：无。
  - 阻塞方：持久 release keystore；指纹比对可在 keystore 就绪后由 CI 产出，设备升级腿另受 ARM64 真机限制。
  - 解锁条件：两候选版本的签名指纹一致回执与升级验证记录；取得后同次迁回过去佛办结。
  - 本板停留期间：无独立本机动作（前置整条被 keystore 阻塞）。
  - 交叉索引：设备升级腿见 §一 ARM64 真机取证组。

- [ ] BANDROID-040.4 [kind: validation] [priority: P0] [points: 2] [owner: android/bevy-host driver] [status: Blocked] Bevy 产品 IPC 源码路径
  - 产物：driver 注入 AndroidOwnerClient request/snapshot source；JNI→Activity→UniFFI→MihomoHost→Messenger→`:vpn` 路径接通；移除 driver 对 infiltrator-android 静态链接；双 ABI 与 universal Debug APK 构建通过 identity/aapt（2026-10-11 记录）。
  - 依赖：BANDROID-040.2。
  - 验收：fake transport scope/stale/timeout 批次回执记录。
  - 当前证据：`bd01251d-c13c-4bf7-a8d5-f316a852c7f4`；13/13。
  - 阻塞方：ARM64 Android 物理设备。
  - 解锁条件：真机运行与 IPC 记录回执；取得后同次迁回过去佛。
  - 本板停留期间：源码路径与 fake 批次已闭环，无剩余本机可执行动作。

- [ ] GAP-2.1 [kind: evidence] [priority: P2] [points: 2] [owner: release] [status: Blocked] Windows/Linux 安装包与系统集成验证
  - 产物：NSIS/AppImage/deb 产物与安装、托盘、TUN 回执。
  - 依赖：`GAP-3.1` 之外的发布流水线；需要 CI 或本机打包环境。
  - 验收：每个安装包有可复跑构建与安装回执；不达标项显式 Unsupported。
  - 当前证据：无。
  - 阻塞方：Windows 目标机（NSIS 腿）；Linux 腿由 APPID-001.3 在未来佛推进。
  - 解锁条件：Windows 安装、托盘与 TUN 回执；取得后同次迁回未来佛办结。
  - 本板停留期间：Linux 侧由 APPID-001.3 承担，本条无独立本机动作。

- [ ] GAP-2.2 [kind: evidence] [priority: P2] [points: 3] [owner: release] [status: Blocked] macOS 产物与签名
  - 产物：App bundle、签名、权限与安装运行回执。
  - 依赖：Apple target 解禁（当前由 `apple-ci-deferral-guard.py` 显式禁止）。
  - 验收：更新平台矩阵并取得目标产物证据。
  - 当前证据：无。
  - 阻塞方：macOS 目标机与 Apple target 解禁决策。
  - 解锁条件：macOS bundle、签名与安装运行回执；取得后同次迁回未来佛办结。
  - 本板停留期间：被 deferral guard 显式禁止，本条无本机动作。

- [ ] GAP-3.1 [kind: evidence] [priority: P1] [points: 3] [owner: android/release] [status: Blocked] Android APK 构建修复取证
  - 产物：cmdline-tools 重链修复后的 APK 与模拟器/真机回执。
  - 依赖：CI tag 或本机完整 SDK/Gradle。
  - 验收：APK 产物 + 启动/触摸/权限回执；否则保持 Unsupported。
  - 当前证据：本机 SDK 已含 cmdline-tools 12.0、NDK 29.0.14206865 与 platform 37.0；修复后的 APK 与模拟器回执尚未产出。
  - 阻塞方：受控 tag CI runner（Android 变体构建与 aapt 身份回执只在 tag 工作流产出）。
  - 解锁条件：tag 工作流的 APK 构建与身份回执；取得后同次迁回未来佛办结。
  - 本板停留期间：本机 toolchain 齐备但缺少 CI 回执，无剩余本机可执行动作。

- [ ] PUB-006.5 [kind: evidence] [priority: P1] [points: 1] [owner: release] [status: Blocked] 1.0.0 首发布回执
  - 产物：`v1.0.0` tag、release notes、crates.io 七个中性 crate（contract/ports/profile-store/mihomo-driver/singbox-driver/engine-runtime/ui-presentation）顺序发布回执与 docs.rs 构建链接。
  - 依赖：PUB-006.1、PUB-006.3、PUB-006.4、PUB-006.12。
  - 验收：crates.io 七 crate 版本严格对齐；docs.rs 七页构建成功；回执绑定 tag 与 commit。
  - 当前证据：无。
  - 阻塞方：crates.io 上架凭证、受控 tag 与 PUB-006.12 基线冻结。
  - 解锁条件：`v1.0.0` tag 发布回执与 docs.rs 构建链接；取得后同次迁回过去佛办结。
  - 本板停留期间：本条无独立本机动作。
