# 收口八问（每个批次必须回答）

> 层级：L2

本文是批次收口的硬约束，不是一次性检查表。**每次提交批次之前**必须：

1. 更新下方「当前状态」表的每一行（结论 / 证据等级 / 缺口 owner）；
2. 在「最近一批问答」中逐条回答八问，八条都不能留空，不能写「同上」；
3. 证据等级只允许：`L` = 本地（cargo/nextest/guard，需可复跑）、`C` = CI run、`D` = 真机或目标产物、`N/A` = 该维度不适用（必须写明原因）。

证据不足时结论只能写「部分 / 缺失」，禁止写「已完成 / 就绪」。`scripts/quality/closure-questions-guard.py` 会拒绝缺项、空答案和非法等级。

## 八问与合格证据

| # | 问题 | 合格证据 |
| --- | --- | --- |
| 1 | x86_64 与 aarch64 是否都支持？ | 每个架构至少一条可复跑的交叉 `cargo check`（L）或目标产物（C/D） |
| 2 | Windows / Linux / macOS 是否都支持？ | 每个平台有构建腿（C）或安装包（D）；不支持的必须显式写 `Unsupported` 并登记原因 |
| 3 | Android / iOS 是否都支持？ | APK/应用包产物（C/D）或显式 `Unsupported` |
| 4 | 各个 UI 是否都接入了？ | 每个 UI 的编译证据 + 功能验收范围；薄壳必须写「最小接入」 |
| 5 | 该复用的是否都复用了？ | 共享 owner 清单；已知重复实现必须清零或立项 |
| 6 | 各处界面的功能是否都接入了？ | parity cell 或平台矩阵状态；`Unsupported` 必须显式列出 |
| 7 | 各处界面是否模块化了？ | 文件/模块分割 + 「无跨 UI 私有业务真源」的检查结果 |
| 8 | CI 打包是否最高效？ | 缓存、`--locked`、触发策略、单批耗时；不高效处必须立项 |

## 当前状态（2026-10-19）

| # | 结论 | 等级 | 缺口 / owner |
| --- | --- | --- | --- |
| 1 | 部分 | L | 中性内核 x64/arm64 × Windows/Linux/Android/macOS **八个组合**全部通过；产品层本机已建立标准自动化复跑脚本 `scripts/cross-check.sh` 与 `scripts/android-check.sh` 并实测全绿：`aarch64-unknown-linux-gnu`（zig 0.14.1 + `cargo-zigbuild` 编译 `zstd-sys`/`ring`）、`aarch64-pc-windows-msvc`（`cargo-xwin` clang sysroot）、`aarch64-linux-android`（NDK 29.0.14206865），覆盖 core/composition/desktop 与 Iced/Bevy/Slint 全部通过；最终安装包与目标设备证据仍缺，由 CI runner 与设备矩阵取证（GAP-1.1） |
| 2 | 部分 | C（旧 commit）+ L | Windows/Linux v0.42.6 四条腿 success（旧源码）；macOS `Unsupported`（无 CI/无产物，GAP-2.2）；安装包未验证（GAP-2.1）；Windows 安装包与 macOS/iOS 目标机腿已整条迁入斗战胜佛并写明阻塞方，Linux 打包腿在现在佛 `APPID-001.3` |
| 3 | 部分 | L | Android 源码级接通，Bevy host driver lockfile 已同步 0.50.0 解决 buildRustDriver 101 错误，APK 待 CI 打包验证，真机 0；iOS `Unsupported`（626 行 stub，GAP-3.2）；模拟器 M1 腿可推（`PARITY-001.4`、过去佛 `BANDROID-002.1`/`002.2`），APK 与真机腿在斗战胜佛 |
| 4 | 部分 | L | Iced/Bevy 深接入；Slint 已导入完整 Lumen 页面并接入共享 owner；四大 UI 表面统一经中立 `SurfaceReader` + `CommandSink` 单向管道交互（PUB-004.2/004.3 闭环，管道 conformance nextest `ae3c3fa9-e02c-4fed-b5ad-9e1d1db95738` 2/2 全绿）；**parity 注册表实测只含 iced/bevy 两表面各 118 单元格，Slint 0 单元格，4 个单元格仍 pending**；0.5x 四套模板与 0.6x 治理文档已归档；**Slint 全量审计实测：缺 dns/editor/filter/mixin/sync/app_routing/doctor 七类页面（19 条业务无载体）、只派发 42 种 `CommandIntent`、59 条业务里 2 条完整实现 / 18 条部分实现 / 39 条未实现且今天零 scenario 锚点、4 个测试文件重实现 Slint 逻辑（第二业务真源）、2 个已声明回调无调用点**；parity 注册表已补齐第三表面（177 单元格全部可解析，Slint 57 pending + 2 implemented）；补齐由现在佛 `PARITY-001.2`～`001.4` 与 `SLINT-001.1`～`001.9` 承担 |
| 5 | 部分 | L | 共享 ports/kernel/kits 已复用；Slint 无 kit；PUB-001.3 part 3 租约切换下沉；GAP-5.1/5.1.1 完成；GAP-5.2 完成；GAP-5.2.1 完成；0.44 UI 管道（PUB-004.1～004.3、PUB-006.3）与 0.5x 四大 UI 脚手架模板（PUB-005.1～005.5）全部高质量办结归档；presentation 纯净化与开源合规审计全绿 |
| 6 | 缺失 | L | 59 条业务能力中 Slint 缺全部 scenario 锚点与 standard/compact 双视口像素回执（其中 19 条业务在 Slint 连页面都不存在，2 条已实现但缺表面测试）；桌面三表面 177 单元格已全部登记并可解析（Iced contract 59/59、scenario 58/59；Bevy 59/59 + 59/59；Slint contract 57/59、scenario 0/59），覆盖守卫 `parity-coverage-guard.py` 报 anchored 292 / pending 62 / **missing 0**，即每个业务都已指派到每个表面、缺口全部登记在案而非静默通过；Android 三 flavor 0 个实际交互截图回执；Android running runtime operations `Unsupported`；覆盖矩阵与收口条目见现在佛 §二/§三（`PARITY-001.2`～`001.4`、`PLATFORM-001.1`、`SLINT-001.1`～`001.9`） |
| 7 | 部分 | L | 文件级模块化完成（<=800/600 行全绿，全仓彻底清退全部遗留 mod.rs 至 0 处，严格遵照 foo.rs + foo/ 模式，并在 check-structure.sh 接入 mod-rs-guard 永久禁止）；跨 UI 页面状态机仍各自实现，无共享页面模型层（GAP-7.1） |
| 8 | 部分 | L | 已补 `--locked` 与 main 守卫门禁（仅 fmt+guards，不编译）；修复 main-gate.yml 缺失 setup-python/Pillow 导致 structure guards 失败的问题，并在 test_visual_receipts 补齐 skipUnless 容错；同步 android/bevy-host/driver/Cargo.lock 解决 release 工作流 Android 打包 101 报错；扩大 Gradle JVM 堆至 6GB 并将三大 flavor 分离独立进程构建，彻底解决打包 OOM；已交付 Android CI 套件 flavor/模拟器裁剪与 Gradle 缓存复用（GAP-8.1 办结） |

## 最近一批问答（2026-10-19）

1. x86_64/aarch64：本批只治理台账与看板，未触及交叉构建腿；`scripts/cross-check.sh`（zig 0.14.1 + `cargo-zigbuild`、`cargo-xwin`）与 `scripts/android-check.sh`（NDK 29.0.14206865）保持上一批全绿结论，安装包与目标设备证据仍缺 → 部分（L）。
2. Windows/Linux/macOS：Windows 安装包（`APPID-001.4`、`GAP-2.1`）与 macOS/iOS 目标机（`GAP-2.2`、`GAP-3.2`）腿已整条迁入斗战胜佛并写明阻塞方与解锁条件；可推腿为 Linux 打包（`APPID-001.3`）与交叉检查（`PLATFORM-001.1`），均登记在现在佛 → 部分（L）。
3. Android/iOS：模拟器 M1 腿在现在佛 `PARITY-001.4` 与过去佛 `BANDROID-002.1`/`002.2` 可推，APK 构建与真机腿在斗战胜佛（`GAP-3.1` 与 ARM64 真机组）；iOS 仍是 626 行 host stub、显式 `Unsupported` → 部分（L）。
4. 各 UI：四大表面统一收敛为中立 `SurfaceReader` + `CommandSink` 单向管道消费者，管道 conformance nextest `ae3c3fa9-e02c-4fed-b5ad-9e1d1db95738` 2/2 全绿；实测 parity 注册表只登记 iced/bevy 各 118 单元格、Slint 0 单元格、4 个 pending（`core-version-rollback-confirm` ×2、`proxies-search-highlight`、`speedtest-progress-cancel`）；进一步全量审计 Slint 表面：页面词汇只有 Home/Proxy/Config/Settings/Network/Connections/Logs/Rules/Traffic/AppSettings/About，**缺 dns/editor/filter/mixin/sync/app_routing/doctor 七类页面（19 条业务无载体）**；只派发 42 种 `CommandIntent`；59 条业务 **2 条完整实现 / 18 条部分实现 / 39 条未实现**，今天**没有任何一条可以合法标记为 Slint scenario 锚点**；4 个测试文件在用例里重实现 Slint 逻辑（路由模式映射、网络模式序列、主题白名单、十六进制解析、连接排序），构成仓库禁止的第二业务真源；两个已声明回调 `refresh-profile`、`refresh-home-egress` 无调用点；本批已把 parity 注册表补齐到三表面（177 单元格全部可解析，Slint 57 pending + 2 implemented，契约目录新增 `slint_page` 载体判定），并立 `SLINT-001.1`～`001.9` 与 `PARITY-001.2`～`001.4`；**本机私有合成器（`kwin_wayland --virtual` + 嵌套 `niri`）已实测可起栈、应用能写 `CAPTURE_READY` 并落 PNG/manifest**，逐业务截图认证在本机可执行，第一批为 Iced demo 已实现交互的 21 条业务 × 双视口 = 42 格 → 部分（L）。
5. 复用：ports/kernel/UI kits 已复用；Slint 无共享 kit；`interpolate` 已去重；PUB-001.3 part 3 租约切换下沉；GAP-5.1/5.1.1/5.2/5.2.1 办结；0.44 UI 管道（PUB-004.1～004.3、PUB-006.3）与 0.5x 模板（PUB-005.1～005.5）办结归档；0.6x 治理体系（PUB-006.2/006.4）办结归档；presentation 纯净化与开源合规审计全绿 → 部分（L）。
6. 功能接入：把 59 条业务能力按九个业务域量化到「桌面三 UI 锚点 + 双视口像素回执 + Android 三 flavor 截图」三列，缺口为 Slint 全部缺失、桌面 118 个待像素回执单元格、Android 0 个 flavor 截图，并立 `PARITY-001.2`～`001.4`、`PLATFORM-001.1`/`001.2` 与 `SLINT-001.1`～`001.9`（含 19 条无载体业务的七类缺失页面、headless 行为台、形状级测试的单一真源整改与两个未接线回调）承担；Android 运行态操作与版本安装入口仍未接 → 缺失（L）。
7. 模块化：文件级达标（<=800/600 行全绿，遗留 mod.rs 清零至 0，mod-rs-guard 永久禁止），复用级不达标，Iced/Bevy 页面状态机重复，`GAP-7.1` 已在现在佛并写明分页下沉计划 → 部分（L）。
8. CI：本批无 CI 变更；现状保持 `--locked`（release 构建与 test.sh）、main 分支守卫（`ci-trigger-guard` 禁止编译命令）、main-gate 补 setup-python 与 Pillow、driver lockfile 同步 0.50.0、Gradle JVM 6GB 与三 flavor 独立进程构建、Android CI 套件裁剪与缓存复用（GAP-8.1 已办结）；0.60 档发布流水线已由 `PUB-006.1` 交付：`.github/workflows/release-neutral-crates.yml` 三段式（拓扑校验 → dry-run → 顺序发布，tag-only、dispatch 仅演练）、`scripts/quality/neutral-release-guard.py` 守卫与 `scripts/neutral-crate-release.sh` 本地入口，本机结构审计七行全 ready、self-test 与 24 条新单测全绿、无 token 时发布模式硬拒绝；真实 tag run 与 crates.io 上架回执仍缺 → 部分（L）。

## 更新纪律

- 批次提交信息不需要复述八问，但本文件必须与批次同一提交更新。
- 任何一行从「部分/缺失」变成「完整」，必须同时给出对应的 `L`/`C`/`D` 证据；只有代码合并、没有证据的，保持原结论。
- 发现新缺口时先加行，再修复；禁止先删行或改结论来「过门」。
