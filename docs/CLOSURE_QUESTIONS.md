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

## 当前状态（2026-10-20）

| # | 结论 | 等级 | 缺口 / owner |
| --- | --- | --- | --- |
| 1 | 部分 | L | 中性内核 x64/arm64 × Windows/Linux/Android/macOS **八个组合**全部通过；本机交叉复跑入口 `scripts/cross-check.sh`（zig 0.14.1 + `cargo-zigbuild`、`cargo-xwin`）与 `scripts/android-check.sh`（NDK 29.0.14206865）保持全绿；本批修掉了交叉腿实测报出的一个真实缺陷（`infiltrator-slint` 的 `clippy::missing-const-for-thread-local`，已改为 `const { Cell::new(false) }` 初始化），该腿由 `scripts/blocked/foreign-target-evidence.sh` 复跑验证；最终安装包与目标设备证据仍缺，由 CI runner 与设备矩阵取证（GAP-1.1） |
| 2 | 部分 | C（旧 commit）+ L | Windows/Linux v0.42.6 四条腿 success（旧源码）；macOS `Unsupported`（无 CI/无产物，GAP-2.2）；安装包未验证（GAP-2.1）；Windows 安装包与 macOS/iOS 目标机腿已整条迁入斗战胜佛并写明阻塞方，Linux 打包腿在现在佛 `APPID-001.3`；**本批：release 作业的签名前置改为 fail closed**——缺 secret 时在昂贵的三 flavor release 构建之前就明确失败，桌面产物不再由 debug key 背书 |
| 3 | 部分 | L | Android 源码级接通，Bevy host driver lockfile 已同步 0.50.0 解决 buildRustDriver 101 错误；**本批：Android 静态质量门成为真门**（Lint 68→0、ktlint 5046→0、Detekt 项目配置 0 条、架构红线守卫 ARCH001–007 扫描 137 点 0 违例并带 23 条负例，已接入结构门）；Android 单测 192 core host + 391 app 全绿；**release 变体改用用户持久 keystore 签名**，无 secret 时 Gradle 配置期即以缺失清单失败，debug key 不再用于任何 release 产物；APK 真实产物与真机仍缺，APK/真机腿在斗战胜佛；iOS `Unsupported`（626 行 stub，GAP-3.2）；模拟器 M1 腿可推（`PARITY-001.4`、过去佛 `BANDROID-002.1`/`002.2`） |
| 4 | 部分 | L | Iced/Bevy 深接入；Slint 已导入完整 Lumen 页面并接入共享 owner；四大 UI 表面统一经中立 `SurfaceReader` + `CommandSink` 单向管道交互（PUB-004.2/004.3 闭环）；**本批：Slint 表面拿到第一批真实像素认证**——`profiles-import-modal` 与 `profiles-delete-confirm` 两个业务在标准/紧凑双视口各有真实截图、原生 bounds、进程/窗口绑定与 SHA-256，4 张回执全部通过校验（新增 `ready` 状态，结构门与接受流程口径同步修正）；Android 三 flavor 的静态质量门已接通（Lint/ktlint/Detekt/架构守卫）；Bevy 侧本批为 widgets 共享开关几何与地球/帧报告（收口批次在途） |
| 5 | 部分 | L | 共享 ports/kernel/kits 已复用；Slint 无 kit；PUB-001.3 part 3 租约切换下沉；GAP-5.1/5.1.1、GAP-5.2、GAP-5.2.1 已完成；0.44 UI 管道（PUB-004.1～004.3、PUB-006.3）与 0.5x 四大 UI 脚手架模板（PUB-005.1～005.5）全部办结归档；presentation 纯净化与开源合规审计全绿；**本批：release 签名决策只有一个 owner**——发布作业把 base64 secret 物化成仓库外的 keystore 交给 Gradle，Gradle 只消费路径与三个凭据，斗战胜佛的 keystore 程序只校验同一份契约，三处不再各写一份签名事实 |
| 6 | 缺失 | L | 59 条业务能力中 Slint 的 scenario 锚点从 0 变为 2（`profiles-import-modal`、`profiles-delete-confirm`，双视口真实像素已认证），剩余 57 条仍 pending，其中 19 条业务在 Slint 连页面都不存在（dns/editor/filter/mixin/sync/app_routing/doctor 七类页面）；桌面三表面 354 个证据格统计为 anchored 295 / pending 59 / **missing 0**（Iced contract 59/59 + scenario 59/59；Bevy 59/59 + 59/59；Slint contract 57/59 + scenario 2/59），177 个清单格为 implemented 120 / ready 2 / pending 57，覆盖守卫 `parity-coverage-guard.py` 仍报缺口在案；**本批新增黑盒像素差判据**：以未激活基线与激活截图在交互区内做像素差，噪声地板由两次基线差测得，只写 `activated=true` 或只路由到页面的表面无法通过；Android 三 flavor 仍 0 个实际交互截图回执 |
| 7 | 部分 | L | 文件级模块化完成（<=800/600 行全绿，全仓 mod.rs 清退至 0，严格遵照 foo.rs + foo/ 模式，并在 check-structure.sh 接入 mod-rs-guard 永久禁止）；跨 UI 页面状态机仍各自实现，无共享页面模型层（GAP-7.1，收口批次在途）；隐式状态机语义审计与判决台账常驻（23 条候选逐条人工判决，未判决或 open 即门禁失败），16 处真实阶段表改写为 typed enum + typed transition；**本批：release 签名按「谁持有事实谁拥有」拆分**——密钥物化在发布作业、凭据校验在 Gradle、契约校验在斗战胜佛程序，签名配置不写第二份回退分支（debug key 回退已删除，旧路径无法再被表达） |
| 8 | 部分 | L | 已补 `--locked` 与 main 守卫门禁（仅 fmt+guards，不编译）；修复 main-gate.yml 缺失 setup-python/Pillow 导致结构守卫失败的问题；扩大 Gradle JVM 堆至 6GB 并将三大 flavor 分离独立进程构建，彻底解决打包 OOM；已交付 Android CI 套件 flavor/模拟器裁剪与 Gradle 缓存复用（GAP-8.1 办结）；**本批：三条流水线效率改动**——① release 作业在昂贵的 flavor 构建之前校验签名材料，缺 secret 秒级失败而不是构建完再失败；② Android 质量门（lint/ktlint/Detekt/架构守卫）接入结构门与本机入口 `scripts/quality/android-quality-gates.sh`，一次跑完三 flavor 并带自测；③ 截图验收改为批量模式，失败不中断整批，逐格给出 certified/uncertified 与原因，并用黑盒像素差替代自报 `activated=true`；全仓 `cargo nextest --workspace --no-fail-fast` 4650/4650 通过（15 skipped） |

## 最近一批问答（2026-10-20 隐式状态机与文档发布批次）

1. x86_64/aarch64：本批未动交叉构建腿；`scripts/cross-check.sh`（zig 0.14.1 + `cargo-zigbuild`、`cargo-xwin`）与 `scripts/android-check.sh`（NDK 29.0.14206865）保持上一批全绿结论，安装包与目标设备证据仍缺 → 部分（L）。
2. Windows/Linux/macOS：Windows 安装包（`APPID-001.4`、`GAP-2.1`）与 macOS/iOS 目标机（`GAP-2.2`、`GAP-3.2`）腿在斗战胜佛等受控环境；本批可推腿是 Bevy 尺寸通道迁移（rem 化 + 双守卫）与 Linux 打包（`APPID-001.3`）→ 部分（L）。
3. Android/iOS：模拟器 M1 腿可推（`PARITY-001.4`、过去佛 `BANDROID-002.1`/`002.2`），APK 与真机腿在斗战胜佛；iOS 仍是 host stub、显式 `Unsupported` → 部分（L）。
4. 各 UI：四大表面统一经中立 `SurfaceReader` + `CommandSink`（管道 conformance nextest `ae3c3fa9-e02c-4fed-b5ad-9e1d1db95738` 2/2）；Slint 补齐无窗口行为台，四个形状级测试文件改为驱动真实表面（41 条表面行为用例，72/72 全绿）；截图认证跑通第一格：Iced `logs-search-highlight` 双视口真实像素回执通过（`activated=true` + 原生 bounds + 进程/窗口绑定 + SHA-256），并因此抓到并修掉两个真缺陷；**Bevy 尺寸通道本批全量机器强制且存量清零**（180 处非白名单 px 全量迁 rem 通道、权威台账归零；`BEVY-REM-001`～`005` + `BEVY-BSN-007`～`010` 生效，widgets 410/410、bevy-ui headless 552/552、结构门全绿）→ 部分（L）。
5. 复用：Slint 的动作规则原先在测试里重实现（路由模式映射、网络模式序列、主题白名单、十六进制解析、连接排序），形成第二业务真源，现收拢为唯一实现 `slint::surface::action`，测试改驱动真实表面；跨 UI 页面模型层仍未建立（GAP-7.1）；**Bevy 侧同一开关几何在四个页面各写一份常量**（轨道 38×22、旋钮偏移 18），已立 `WIDGET-001.1` 收敛为 widgets 的共享 switch 控件（补 owner 结构，不在上层打补丁）→ 部分（L）。
6. 功能接入：59 业务 × 三表面 177 单元格全部登记，覆盖守卫报 anchored 292 / pending 62 / missing 0；Slint 的 59 个 scenario 仍全部 pending，Android 三 flavor 0 个交互截图回执；**Bevy 侧 ⑦ 小件收敛已机器化**（列表场景 `bsn_list!`、批量退役 `despawn_all`、胶囊 `BorderRadius::MAX`、`template_value` 硬零），本批的字号档位像素回执已闭环（1.0/1.5 两档同场景真实截图逐像素比对 46.2% 像素变化），只剩 Iced 侧命令面板圆角对齐一项跨表面对齐 → 缺失（L）。
7. 模块化：文件级达标（<=800/600 行全绿、mod.rs 清零）；状态表达红线机器化：语义审计 + 判决台账（23 条候选逐条判决，未判决或 open 即门禁失败），16 处真实阶段表改写为 typed enum + typed transition（3 处原本可表示非法组合），9 处判定为合法独立事实并记录理由；跨 UI 页面模型层仍未建立；**Bevy 场景律法 owner 已收敛**为「产品守卫 = 上游律法的严格超集，共享包门禁降为上游镜像」，同一站点不留两份台账放行；**能力规范②④的浮层存量缺口也已收口**（八个浮层 owner 补 `ThemeContext(SurfaceLevel::Floating)` 与官方 `FixedNode`，机器规则 `BEVY-BSN-011` 由标记定义自动推导 owner）、**modal 层已迁官方 `Dialog`/`ModalDialog`/`ModalDialogBarrier`/`RequestClose`**（barrier/Esc/焦点圈闭归官方，阶段为 typed `ModalStage`）、**开关几何收敛为 widgets 的唯一真源**（`infiltrator_bevy_widgets::switch` + typed `SwitchPosition`，四个页面不再各写一份常量） → 部分（L）。
8. CI：测试策略从「禁止一切 `cargo test`」精确化为「只允许 doctest 形态」（`cargo test --doc` 是 nextest 无法执行的文档示例）并覆盖未跟踪文件；新增 rustdoc 门禁（docs.rs 元数据 + 零告警 + doctest 通过）、Bevy 尺寸双守卫、隐式状态机语义审计与覆盖守卫，均接入结构门；七个中性 crate 1436 个公开项补齐真实 Rustdoc + 7 条可执行 doctest（353/353 全绿）；全仓 `cargo nextest --workspace --no-fail-fast` 4632/4632 通过 → 部分（L）。

## 最近一批问答（2026-10-20 发布签名 fail-closed 与 Android 质量门批次）

1. x86_64/aarch64：本批未动交叉构建腿；`scripts/cross-check.sh`（zig 0.14.1 + `cargo-zigbuild`、`cargo-xwin`）与 `scripts/android-check.sh`（NDK 29.0.14206865）保持上一批全绿结论，安装包与目标设备证据仍缺 → 部分（L）。
2. Windows/Linux/macOS：Windows 安装包（`APPID-001.4`、`GAP-2.1`）与 macOS/iOS 目标机（`GAP-2.2`、`GAP-3.2`）腿在斗战胜佛等受控环境；本批的可推改动是 release 作业的签名前置 fail closed（缺 secret 在昂贵构建前失败）与 Linux 打包腿（`APPID-001.3`，收口批次在途）→ 部分（L）。
3. Android/iOS：Android 静态质量门成为真门（Lint 68→0、ktlint 5046→0、Detekt 项目配置 0 条、架构红线守卫 137 点 0 违例 + 23 条负例并接入结构门），Android 单测 192 core host + 391 app 全绿；release 变体改用用户 keystore 签名、缺 secret 时 Gradle 配置期以缺失清单失败、debug key 回退已删除；APK 真实产物与真机腿仍在斗战胜佛；iOS 仍是 host stub、显式 `Unsupported` → 部分（L）。
4. 各 UI：四大表面统一经中立 `SurfaceReader` + `CommandSink`；**本批 Slint 拿到第一批真实像素认证**——`profiles-import-modal` 与 `profiles-delete-confirm` 在标准/紧凑双视口各有真实截图、原生 bounds、进程/窗口绑定与 SHA-256，4 张回执全部通过校验，并在注册表里首次出现 `ready` 状态（结构门与接受流程对 `ready` 的口径同步修正为「像素层由接受流程裁决，结构门只判 schema 与锚点」）；Android 三 flavor 的静态质量门已接通 → 部分（L）。
5. 复用：**release 签名决策只有一个 owner**——发布作业把 base64 secret 物化成仓库外 keystore 交给 Gradle，Gradle 只消费路径与三个凭据，斗战胜佛的 keystore 程序只校验同一份契约；旧的三处各写一份（Gradle 读 secret、workflow 无 secret、程序断言别的契约）已清零；跨 UI 页面模型层仍未建立（GAP-7.1，在途）→ 部分（L）。
6. 功能接入：59 业务 × 三表面 354 证据格为 anchored 295 / pending 59 / missing 0，177 清单格为 implemented 120 / ready 2 / pending 57；Slint 的 scenario 锚点由 0 变 2，剩余 57 条中 19 条连页面都不存在；**本批验收改批量并加黑盒像素差判据**：未激活基线与激活截图在交互区内做像素差、噪声地板由两次基线差测得，只写 `activated=true` 或只路由到页面的表面无法通过；Android 三 flavor 仍 0 个交互截图回执 → 缺失（L）。
7. 模块化：文件级达标（<=800/600 行全绿、mod.rs 清零）；隐式状态机语义审计 + 判决台账常驻，16 处阶段表已改 typed enum + typed transition；**本批 release 签名按「谁持有事实谁拥有」拆分**（物化在发布作业、校验在 Gradle、契约校验在斗战胜佛程序），签名配置不再有第二份回退分支，旧路径无法再被表达；跨 UI 页面模型层仍未建立 → 部分（L）。
8. CI：三条效率改动——① release 作业在 flavor 构建之前校验签名材料，缺 secret 秒级失败；② Android 质量门接入结构门与本机入口 `scripts/quality/android-quality-gates.sh`，一次跑完三 flavor 并带自测；③ 截图验收批量模式，逐格 certified/uncertified + 原因，黑盒像素差替代自报 `activated=true`；全仓 `cargo nextest --workspace --no-fail-fast` 4650/4650 通过（15 skipped）→ 部分（L）。

## 更新纪律

- 批次提交信息不需要复述八问，但本文件必须与批次同一提交更新。
- 任何一行从「部分/缺失」变成「完整」，必须同时给出对应的 `L`/`C`/`D` 证据；只有代码合并、没有证据的，保持原结论。
- 发现新缺口时先加行，再修复；禁止先删行或改结论来「过门」。