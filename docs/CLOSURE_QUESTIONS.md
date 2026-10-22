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

## 当前状态（2026-10-21）

| # | 结论 | 等级 | 缺口 / owner |
| --- | --- | --- | --- |
| 1 | 部分 | L | 中性内核 x64/arm64 × Windows/Linux/Android/macOS **八个组合**全部通过；本机交叉复跑入口 `scripts/cross-check.sh`（zig 0.14.1 + `cargo-zigbuild`、`cargo-xwin`）与 `scripts/android-check.sh`（NDK 29.0.14206865）保持全绿；本批修掉了交叉腿实测报出的一个真实缺陷（`infiltrator-slint` 的 `clippy::missing-const-for-thread-local`，已改为 `const { Cell::new(false) }` 初始化），该腿由 `scripts/blocked/foreign-target-evidence.sh` 复跑验证；最终安装包与目标设备证据仍缺，由 CI runner 与设备矩阵取证（GAP-1.1） |
| 2 | 部分 | C（旧 commit）+ L | Windows/Linux v0.42.6 四条腿 success（旧源码）；macOS `Unsupported`（无 CI/无产物，GAP-2.2）；安装包未验证（GAP-2.1）；Windows 安装包与 macOS/iOS 目标机腿已整条迁入斗战胜佛并写明阻塞方，Linux 打包腿在现在佛 `APPID-001.3`；**本批：release 作业的签名前置改为 fail closed**——缺 secret 时在昂贵的三 flavor release 构建之前就明确失败，桌面产物不再由 debug key 背书 |
| 3 | 部分 | L | Android 源码级接通，Bevy host driver lockfile 已同步 0.50.0 解决 buildRustDriver 101 错误；**本批：Android 静态质量门成为真门**（Lint 68→0、ktlint 5046→0、Detekt 项目配置 0 条、架构红线守卫 ARCH001–007 扫描 137 点 0 违例并带 23 条负例，已接入结构门）；Android 单测 192 core host + 391 app 全绿；**release 变体改用用户持久 keystore 签名**，无 secret 时 Gradle 配置期即以缺失清单失败，debug key 不再用于任何 release 产物；APK 真实产物与真机仍缺，APK/真机腿在斗战胜佛；iOS `Unsupported`（626 行 stub，GAP-3.2）；模拟器 M1 腿可推（`PARITY-001.4`、过去佛 `BANDROID-002.1`/`002.2`） |
| 4 | 部分 | L | Iced/Bevy 深接入；Slint 已导入完整 Lumen 页面并接入共享 owner；四大 UI 表面统一经中立 `SurfaceReader` + `CommandSink` 单向管道交互；**本批：九个页面的状态推导有了唯一 owner**——`infiltrator-presentation` 拥有代理/概览/设置/规则/订阅/诊断/DNS/应用路由/同步九个页面模型与两个共用基座（带提交令牌的 typed 阶段、由基线推导的草稿），三端删除各自的私有推导（Iced 8+10 个阶段机与四份契约快照副本、Bevy 私有过滤排序与概览投影、Slint 私有过滤排序与路由/网络模式推导），共享层把三端已经不一致的事实一次写定；**本批：契约层的五处缺口补齐**——LAN 共享读回成为一份事实（`Option<LanSharingSnapshot>`，未观测不是关闭）、PAC 读回回环请求、规则列表过滤归共享层、应用路由「未观测」有自己的文案、闭合连接并入共享连接模型，三端的私有副本与降级分支清零搜索为空；**本批：三个表面的 demo 与捕获层真的执行了业务**（Iced 5 个 panic 与 Bevy 4 个 panic 的根因都是共享能力门没有能力矩阵，已修；另补 18 条真实交互：Iced 的设置权限/LAN/端口冲突/回滚确认/拓扑下钻/测速取消/测速历史/键盘焦点/重连遮罩/生命周期恢复/导入表单，Bevy 的应用路由规则/生命周期恢复/脚本沙箱/脚本导出评审） |
| 5 | 部分 | L | 共享 ports/kernel/kits 已复用；Slint 无 kit；PUB-001.3 part 3 租约切换下沉；GAP-5.1/5.1.1、GAP-5.2、GAP-5.2.1 已完成；0.44 UI 管道（PUB-004.1～004.3、PUB-006.3）与 0.5x 四大 UI 脚手架模板（PUB-005.1～005.5）全部办结归档；presentation 纯净化与开源合规审计全绿；**本批：三个 owner 收敛**——① release 签名决策只有一个 owner；② 圆角阶梯只有一份真相；③ 中性生态的公开 API 只有一份基线；**本批第四个 owner 收敛**：页面状态推导只有一份（九个页面模型 + 两个共用基座），三端的第二份过滤、排序、阶段机与契约快照副本已删除；**本批第五个 owner 收敛**：契约层的页面事实只有一份（LAN 共享读回、PAC 读回、规则过滤 API、应用路由未观测文案、闭合连接视图），**本批第六个 owner 收敛**：两个契约声明、应用却回 `unsupported()` 的命令（同步冲突解决、概览卡片排序）现在由真实 owner 回答，带令牌、读回与 stale 拒绝 |
| 6 | 缺失 | L | 59 条业务能力中 Slint 的 scenario 锚点从 0 变为 2（双视口真实像素已认证），**本批再增 16 条业务有了载体与表面行为测试**（DNS 4、编辑器 4、过滤与聚合 3、诊断 2、同步 2、应用路由 1），**且 18 条承载业务的交互路径全部补齐**（42 个新行为测试，失败/重试/取消路径第一次真正可达：owner 的终态此前被丢弃，共享 workbench 补上在途台账与 typed 终态），**注册表已登记这 16 条业务的情景锚点**（锚点总数 312，解析器 status=pass）；**本批：13 个 owner 端口第一次有了 fixture**（版本、端口冲突、服务模式、LAN 配置、特权网络、配置存储、订阅源、订阅导入、规则提供者与缓存、快照存储、测速历史、文档 codec、核心进程），此前每一条都让对应业务无法被证明；剩余 41 条业务在 Slint 仍无页面载体（代理、连接、规则、测速、命令面板、设置、订阅错误等）；桌面三表面 354 个证据格统计为 anchored 296 / pending 58 / **missing 0**；**本批新增黑盒像素差判据**与「编译失败的包不算通过、不可枚举的锚点算未验证」两类假通过的修正 |
| 7 | 部分 | L | 文件级模块化完成（<=800/600 行全绿，全仓 mod.rs 清退至 0，严格遵照 foo.rs + foo/ 模式，并在 check-structure.sh 接入 mod-rs-guard 永久禁止）；隐式状态机语义审计 + 判决台账常驻（22 条候选逐条人工判决，未判决或 open 即门禁失败），16 处阶段表已改 typed enum + typed transition；**本批：九个页面的状态推导有唯一 owner**——`infiltrator-presentation` 拥有九个页面模型与两个共用基座（`OperationPhase` 的提交令牌让被取代的答复不移动状态，`EditableDraft` 的「已修改」由草稿与基线推导），三端的私有阶段机、私有过滤排序与契约快照副本已删除；**本批：契约层的五处缺口与两个「声明了却回 unsupported」的命令都由真实 owner 回答**；**本批：三个 owner 收敛**（release 签名、圆角阶梯、中性 API 基线）；开关几何收敛为 widgets 的唯一真源 |
| 8 | 部分 | L | 已补 `--locked` 与 main 守卫门禁（仅 fmt+guards，不编译）；修复 main-gate.yml 缺失 setup-python/Pillow 导致结构守卫失败的问题；扩大 Gradle JVM 堆至 6GB 并将三大 flavor 分离独立进程构建，彻底解决打包 OOM；已交付 Android CI 套件 flavor/模拟器裁剪与 Gradle 缓存复用（GAP-8.1 办结）；**本批：四条流水线效率改动**——① release 作业在昂贵的 flavor 构建之前校验签名材料，缺 secret 秒级失败；② Android 质量门接入结构门与本机入口；③ 截图验收改为批量模式并用黑盒像素差替代自报 `activated=true`；④ 新增生态门禁 workflow；**本批：版本号按身份改动一次做完升到 0.51.0**（工作区版本、20 个成员 pin、lock 文件、SemVer 基线与两份文档同步，发布拓扑守卫读工作区版本而不是测试里的字面量），**本批：tag 流水线在本机修掉两个真实缺陷**——① parity 证据解析的锚点指向了一个被迁移改回的测试名，test-gate 因此跑不到那个测试而失败；② 测试门禁的构建写爆了 runner 根分区（runner 连自己的日志都写不进去），改为只保留行号表并回收镜像里用不到的工具链；`v0.51.0` run `39477624601` 的 validate、test-gate 与四条桌面构建腿全绿，Android 腿按设计因缺用户 keystore secrets fail closed，发布作业被跳过 |

## 最近一批问答（2026-10-20 第三表面补齐、页面模型与远端门禁批次）

1. x86_64/aarch64：本批未动交叉构建腿；`scripts/cross-check.sh`（zig 0.14.1 + `cargo-zigbuild`、`cargo-xwin`）与 `scripts/android-check.sh`（NDK 29.0.14206865）保持全绿结论，`cross-build-guard.py` 把四条桌面 triple、两条 aarch64 交叉腿与两条 Android 腿钉在位且拒绝 Apple 目标混入；最终安装包与目标设备证据仍缺（GAP-1.1）→ 部分（L）。
2. Windows/Linux/macOS：Windows 安装包与 macOS/iOS 目标机腿在斗战胜佛等受控环境；本批的可推改动是 Linux 打包腿（deb 与 AppImage 从登记表产出、安装-启动-卸载冒烟通过）与 release 作业的签名前置 fail closed → 部分（L）。
3. Android/iOS：Android 静态质量门成为真门（Lint 68→0、ktlint 5046→0、Detekt 0 条、架构红线守卫 137 点 0 违例 + 23 条负例），Android 单测 192 core host + 391 app 全绿；release 变体改用用户 keystore 签名、缺 secret 时 Gradle 配置期以缺失清单失败、debug key 回退已删除；**本批新增 Android 三 flavor 模拟器取证流**：回执绑定 APK 摘要、业务 id、截图与证明该步发生过的 logcat 片段，像素判决复用桌面同一把尺子；真机腿仍在斗战胜佛；iOS 仍是 host stub、显式 `Unsupported` → 部分（L）。
4. 各 UI：四大表面统一经中立 `SurfaceReader` + `CommandSink`；**本批：第三表面补上六类页面载体**（DNS、编辑器、过滤与聚合、诊断、同步、应用路由），十九条原本无载体的业务有了页面，两个「声明了但无调用点」的回调也有了真实控件；**本批：远端门禁首次全绿**——`main-gate`（`cargo fmt --all --check` + 全部结构守卫）在推送后通过；Bevy 侧模态层改用官方 `Dialog`/`ModalDialog`/`RequestClose`，业务状态保留为 typed stage → 部分（L）。
5. 复用：**四个 owner 收敛**——release 签名决策只有一个 owner；圆角阶梯只有一份真相（契约裁决三个互斥角色，重复的 `DIALOG` 档退役，widgets 镜像逐档对照合同测试）；中性 API 只有一份基线（1182 项 SemVer 基线 + 会拒绝故意破坏的负例）；**页面模型开始有唯一 owner**（`infiltrator-presentation` 拥有日志页与连接页的页面模型，Iced 的第二份日志查询状态、字符串排序键与本地排序归一化已删除）→ 部分（L）。
6. 功能接入：59 业务 × 三表面 354 证据格为 anchored 296 / pending 58 / missing 0，177 清单格为 implemented 120 / ready 2 / pending 55；**本批：16 条业务在 Slint 有了载体与表面行为测试**，但交互路径覆盖仍不完整，注册表如实记为 pending 并逐条列出已证明与仍缺的路径——只派发命令不算已认证 scenario；**四个 pending 行为锚点已全部 anchored**；Android 三 flavor 的取证流已交付，真实回执随收口批次产出 → 缺失（L）。
7. 模块化：文件级达标；隐式状态机语义审计 + 判决台账常驻，16 处阶段表已改 typed enum + typed transition；**本批：页面模型有唯一 owner 并开始收编各表面**；三个 owner 收敛（release 签名、圆角阶梯、中性 API 基线）；开关几何收敛为 widgets 的唯一真源；地球增强有自己的 typed 阶段与降级策略 → 部分（L）。
8. CI：**本批：远端门禁首次全绿**——`main-gate` 在推送后通过（格式化 + 全部结构守卫）；**本批还修掉了三个真实缺陷**：① 推送到远端才暴露出 27 个文件偏离格式化器的形状（本地提交期间门禁不跑），现在提交即格式化，远端树的形状与格式化器一致，而正在被别的会话写的文件用「只改索引里的提交形状、不动工作树」的方式修复，两边都不丢改动；② Android 取证流的测试在干净检出上因为目标目录不存在而失败，现在测试自己建目录；③ release 作业此前用 debug key 签名发布产物且不带任何签名 secret，现在缺 secret 即在昂贵的构建之前失败。生态门禁 workflow（示例编译、SemVer 类型检查、无头混沌矩阵）已登记触发策略，只有受控 `v*` tag 起 job → 部分（L）。

## 最近一批问答（2026-10-21 页面模型收编、真实交互与 0.51.0 批次）

1. x86_64/aarch64：本批未动交叉构建腿；`scripts/cross-check.sh`（zig 0.14.1 + `cargo-zigbuild`、`cargo-xwin`）与 `scripts/android-check.sh`（NDK 29.0.14206865）保持全绿结论，`cross-build-guard.py` 把四条桌面 triple、两条 aarch64 交叉腿与两条 Android 腿钉在位；最终安装包与目标设备证据仍缺（GAP-1.1）→ 部分（L）。
2. Windows/Linux/macOS：本批未动平台打包腿；Windows 安装包与 macOS/iOS 目标机腿在斗战胜佛等受控环境，Linux 打包腿（deb 与 AppImage）已交付并在过去佛待统一批次复验；本批的可推改动是版本号按身份改动一次做完升到 0.51.0（工作区版本、20 个成员 pin、lock 文件、SemVer 基线、发布拓扑与产品身份守卫同时通过）→ 部分（L）。
3. Android/iOS：Android 静态质量门是上一批的真门（Lint 68→0、ktlint 5046→0、Detekt 0 条、架构红线守卫 137 点 0 违例 + 23 条负例），Android 单测 192 core host + 391 app 全绿；release 变体用用户持久 keystore 签名、缺 secret 时 Gradle 配置期以缺失清单失败、debug key 回退已删除；本批新增的事实是 Slint 侧 owner 终态经 `execute_typed` 送达，Android host 仍只发 `CommandOutput::Unit`（带 identity 的评审类业务在 Android 上只能到「已派发」，已登记在 GAP-4.1 的待验证里）；iOS 仍是 host stub、显式 `Unsupported` → 部分（L）。
4. 各 UI：四大表面统一经中立 `SurfaceReader` + `CommandSink`；**本批：九个页面的状态推导有了唯一 owner**（`infiltrator-presentation` 的九个页面模型 + 两个共用基座），Iced/Bevy/Slint 三端都改为读共享模型并删除各自的私有推导（Iced 8+10 个阶段机与四份契约快照副本、Bevy 私有过滤排序与概览投影、Slint 私有过滤排序与路由/网络模式推导）；**本批：三个表面的 demo 与捕获层真的执行了业务**（Iced 5 个 panic 与 Bevy 4 个 panic 的根因都是共享能力门没有能力矩阵，已修；另补七类真实交互）；Bevy 侧模态层仍用官方 `Dialog`/`ModalDialog`/`RequestClose` → 部分（L）。
5. 复用：**本批第四个 owner 收敛**——页面状态推导只有一份：共享层把三端已经不一致的事实一次写定（代理过滤规则取三端并集：名称/身份/协议与别名/feature 标签/UDP/延迟/拼音；收藏恒排最前；未知延迟恒排最后；组匹配是子串或拼音；协议芯片与名称高亮各一份实现；诊断页复用 presentation 层的共享诊断动作阶段机而不再并行跑第二台阶段机），三端的第二份实现已按清零搜索确认消失 → 部分（L）。
6. 功能接入：59 业务 × 三表面 354 证据格为 anchored 296 / pending 58 / missing 0；**本批：Slint 侧 18 条承载业务的交互路径全部补齐**（42 个新行为测试，owner 的失败/重试/取消路径第一次真正可达——此前终态被丢弃、`can_retry()` 永远为假）；**本批修掉两类假通过**（编译失败的包曾被记成「通过」、不可枚举的锚点曾被记成「悬空」，现在分别是「未知」与「未验证」），黑盒像素差判据让「只路由到页面」的表面无法冒充已认证；全量像素认证与回执登记仍未跑完 → 缺失（L）。
7. 模块化：文件级达标；隐式状态机语义审计 + 判决台账常驻；**本批：九个页面模型与两个共用基座成为页面状态的唯一 owner**（`OperationPhase` 的提交令牌让被取代的答复不移动状态，`EditableDraft` 的「已修改」由草稿与基线推导而不是 flag，Iced 的 `lan_sharing_dirty`/`lan_security_dirty` 与 Bevy 的 `RulesJsonState.dirty` 随之消失）→ 部分（L）。
8. CI：**本批：版本号按身份改动一次做完升到 0.51.0**（工作区版本、20 个成员 pin、lock 文件、SemVer 基线与两份文档同步；发布拓扑守卫读工作区版本而不是测试里的字面量，版本号再升不必改测试）；**本批把一个真实缺陷修在门上**——`test_neutral_release_guard` 曾硬编码 `0.50.0`，每次发版都会把一个与版本无关的测试打红，现在它读工作区版本；远端 `main-gate`（格式化 + 全部结构守卫）在本批之前的推送上持续绿；`v0.51.0` tag 触发的 release 流水线（run `39477624601`）里 **validate、test-gate 与四条桌面构建腿全绿**，Android 腿按设计因缺用户 keystore secrets 在签名前置 fail closed，发布作业被跳过（没有产出未签名的发布件）；**本批还修掉了 tag 流水线上的两个真实缺陷**：① parity 证据解析的锚点指向了一个被迁移改回的测试名，test-gate 因此跑不到那个测试而失败，锚点现在指向真实存在的测试；② 测试门禁的构建写爆了 runner 根分区（runner 连自己的日志都写不进去），改为只保留行号表并回收镜像里用不到的工具链 → 部分（L）。

## 最近一批问答（2026-10-21 契约缺口、owner 能力与真实交互批次）

1. x86_64/aarch64：本批未动交叉构建腿；`scripts/cross-check.sh` 与 `scripts/android-check.sh` 保持全绿结论，`cross-build-guard.py` 把四条桌面 triple、两条 aarch64 交叉腿与两条 Android 腿钉在位；最终安装包与目标设备证据仍缺（GAP-1.1）→ 部分（L）。
2. Windows/Linux/macOS：本批未动平台打包腿；Windows 安装包与 macOS/iOS 目标机腿在斗战胜佛等受控环境，Linux 打包腿已交付并在过去佛待统一批次复验；本批的可推改动是版本号按身份改动一次做完升到 0.51.0（工作区版本、20 个成员 pin、lock 文件、SemVer 基线、发布拓扑与产品身份守卫同时通过）→ 部分（L）。
3. Android/iOS：Android 静态质量门是真门，Android 单测全绿；release 变体用用户持久 keystore 签名、缺 secret 时 Gradle 配置期以缺失清单失败、debug key 回退已删除；**本批新增的事实**：Android host 现在把 owner 的 typed 输出交给 Slint 表面（此前只发 `CommandOutput::Unit`，带 identity 的评审类业务在 Android 上只能到「已派发」），Android 侧因此不再丢 owner 的答复；iOS 仍是 host stub、显式 `Unsupported` → 部分（L）。
4. 各 UI：四大表面统一经中立 `SurfaceReader` + `CommandSink`；**本批：九个页面的状态推导有了唯一 owner**（九个页面模型 + 两个共用基座），**契约层的五处缺口补齐**（LAN 共享读回、PAC 读回、规则过滤 API、应用路由未观测文案、闭合连接视图），**三端删除各自的私有副本与降级分支**；**本批：18 条真实交互**（Iced 11 条、Bevy 4 条，其余为三端共用的 owner 接线）→ 部分（L）。
5. 复用：**本批第六个 owner 收敛**——两个契约声明、应用却回 `unsupported()` 的命令（同步冲突解决、概览卡片排序）现在由真实 owner 回答：冲突解决组合同步 owner 与配置 owner（保留本地丢弃远端副本 / 接受远端先写本地再丢弃副本，失败保留副本），概览卡片排序由 `OverviewLayoutApplication` 作为 owner 持有顺序、相对移动按 owner 当前状态计算、绝对排序按 revision 围栏；**本批第五个 owner 收敛**（契约层的五处页面事实）→ 部分（L）。
6. 功能接入：59 业务 × 三表面 354 证据格为 anchored 296 / pending 58 / missing 0；**本批：18 条承载业务的交互路径全部补齐**（42 个新行为测试）；**本批：16 条 Slint 业务的情景锚点已登记**（锚点 312，解析器 status=pass）；**本批：13 个 owner 端口第一次有了 fixture**（此前每一条都让对应业务无法被证明）；**本批：4 条 Bevy 业务与 11 条 Iced 业务有了真实交互**；全量像素认证与回执登记仍未跑完 → 缺失（L）。
7. 模块化：文件级达标；隐式状态机语义审计 + 判决台账常驻；**本批：九个页面模型与两个共用基座成为页面状态的唯一 owner**；**本批：契约层五处事实与两个「声明了却回 unsupported」的命令都由真实 owner 回答**；**本批：13 个端口 fixture 带 README 索引**，每个 fixture 诚实回答（失败的源真的失败、不知道的配置真的回 `unsupported()`）→ 部分（L）。
8. CI：**本批：版本号按身份改动一次做完升到 0.51.0**（发布拓扑守卫读工作区版本而不是测试里的字面量，版本号再升不必改测试）；**本批把 tag 流水线上的两个真实缺陷修在门上**——① parity 证据解析的锚点指向了一个被迁移改回的测试名，test-gate 因此跑不到那个测试而失败，锚点现在指向真实存在的测试；② 测试门禁的构建写爆了 runner 根分区（runner 连自己的日志都写不进去），改为只保留行号表并回收镜像里用不到的工具链；`v0.51.0` run `39477624601` 的 validate、test-gate 与四条桌面构建腿全绿，Android 腿按设计因缺用户 keystore secrets fail closed（没有产出未签名的发布件）→ 部分（L）。

## 最近一批问答（2026-10-22 工程约束补齐批次）

1. x86_64/aarch64：本批未动交叉构建腿，`scripts/cross-check.sh` 与 `scripts/android-check.sh` 的既有结论不变；本批新增的七道门禁都是离线结构门，不产生也不消耗任何目标架构证据 → 部分（L）。
2. Windows/Linux/macOS：本批未新增平台构建腿，安装包证据仍缺；本批只把依赖政策、工具链钉版、unsafe 可追溯、panic 边界、库层输出、构建脚本与特性矩阵变成常驻门禁 → 部分（L）。
3. Android/iOS：本批未动 Android 构建腿，APK 与真机证据仍缺；新增门禁对 Android 侧同样生效（工具链版本、依赖政策、构建脚本），但不产生设备证据 → 部分（L）。
4. 各 UI：本批未改任何 UI 表面代码；新增门禁只约束工程面，不改变各表面的接入与验收范围 → 部分（L）。
5. 复用：**本批新增的七道门禁各自只有一个真源**——依赖政策由根目录的依赖政策文件裁决并与第三方声明保持一致，Rust 版本由工具链钉版文件裁决，unsafe、panic 边界、库层输出、构建脚本与特性矩阵各有一份台账；台账与结构门互为校验，没有第二份副本或影子状态 → 部分（L）。
6. 功能接入：本批未新增业务能力，场景锚点与像素回执统计不变；本批新增的约束不产生业务证据 → 缺失（L）。
7. 模块化：**本批：五份台账成为常驻门禁**——unsafe 操作数必须与源码逐个一致、缺少安全说明的数量只能下降；每个 panic 边界必须登记它拦住的失败，任何 profile 禁止 abort；内层 crate 的标准输出清零，其它库层站点登记上限；每个构建脚本必须声明归属 crate、构建依赖、是否派生进程与是否写文件；每个特性必须分类，default 不得隐式开启捕获与测试特性 → 部分（L）。
8. CI：**本批：生态门禁新增 tag 门控的依赖政策作业**（公告、许可、禁用、来源四项一起执行），主门禁原先浮动的工具链引用改为钉住与工作区同一个发布版，工具链版本从此只有一个真源；本批的改动全部是离线门禁与配置，不触发任何编译成本 → 部分（L）。

## 更新纪律

- 批次提交信息不需要复述八问，但本文件必须与批次同一提交更新。
- 任何一行从「部分/缺失」变成「完整」，必须同时给出对应的 `L`/`C`/`D` 证据；只有代码合并、没有证据的，保持原结论。
- 发现新缺口时先加行，再修复；禁止先删行或改结论来「过门」。
