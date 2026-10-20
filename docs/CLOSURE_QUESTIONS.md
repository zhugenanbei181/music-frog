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
| 4 | 部分 | L | Iced/Bevy 深接入；Slint 已导入完整 Lumen 页面并接入共享 owner；四大 UI 表面统一经中立 `SurfaceReader` + `CommandSink` 单向管道交互；**本批：第三表面补上六类页面载体**（DNS、编辑器、过滤与聚合、诊断、同步、应用路由），十九条原本无载体的业务有了页面，两个「声明了但无调用点」的回调也有了真实控件；**本批：远端门禁首次全绿**——`main-gate`（格式化 + 全部结构守卫）在推送后通过，`cargo fmt --all --check` 与结构门都在远端机器上执行过；Bevy 侧模态层改用官方 `Dialog`/`ModalDialog`/`RequestClose`（barrier 点击外关闭、Esc 拦截、焦点圈闭归官方，业务状态保留为 typed stage） |
| 5 | 部分 | L | 共享 ports/kernel/kits 已复用；Slint 无 kit；PUB-001.3 part 3 租约切换下沉；GAP-5.1/5.1.1、GAP-5.2、GAP-5.2.1 已完成；0.44 UI 管道（PUB-004.1～004.3、PUB-006.3）与 0.5x 四大 UI 脚手架模板（PUB-005.1～005.5）全部办结归档；presentation 纯净化与开源合规审计全绿；**本批：三个 owner 收敛**——① release 签名决策只有一个 owner（发布作业物化密钥、Gradle 只消费路径与凭据、斗战胜佛程序只校验同一份契约）；② 圆角阶梯只有一份真相（契约裁决三个互斥角色，重复的 `DIALOG` 档退役，widgets 镜像阶梯逐档对照合同测试）；③ 中性生态的公开 API 只有一份基线（1182 项 SemVer 基线 + 会拒绝故意破坏的负例），七套示例与无头沙盒都只依赖中性 crate |
| 6 | 缺失 | L | 59 条业务能力中 Slint 的 scenario 锚点从 0 变为 2（双视口真实像素已认证），**本批再增 16 条业务有了载体与表面行为测试**（DNS 4、编辑器 4、过滤与聚合 3、诊断 2、同步 2、应用路由 1），但**交互路径覆盖仍不完整**，因此注册表如实记为 `pending` 并逐条列出已证明与仍缺的路径——只派发命令不算已认证 scenario；剩余 41 条业务在 Slint 仍无页面载体（代理、连接、规则、测速、命令面板、设置、订阅错误等）；桌面三表面 354 个证据格统计为 anchored 296 / pending 58 / **missing 0**（Iced 59/59 + 59/59；Bevy 59/59 + 59/59；Slint contract 58/59、scenario 2/59），177 个清单格为 implemented 120 / ready 2 / pending 55；**本批新增黑盒像素差判据**：以未激活基线与激活截图在交互区内做像素差，噪声地板由两次基线差测得，只写 `activated=true` 或只路由到页面的表面无法通过；四个 pending 行为锚点已全部 anchored；Android 三 flavor 的取证流已交付（真实回执随收口批次产出） |
| 7 | 部分 | L | 文件级模块化完成（<=800/600 行全绿，全仓 mod.rs 清退至 0，严格遵照 foo.rs + foo/ 模式，并在 check-structure.sh 接入 mod-rs-guard 永久禁止）；隐式状态机语义审计 + 判决台账常驻（22 条候选逐条人工判决，未判决或 open 即门禁失败），16 处阶段表已改 typed enum + typed transition；**本批：页面模型开始有唯一 owner**——`infiltrator-presentation` 拥有日志页与连接页的页面模型（typed 页面状态、typed 等级过滤、typed 搜索结局、唯一的排序选项表），Iced 的第二份日志查询状态、字符串排序键与本地排序归一化已删除，Bevy 与 Slint 的迁移按精确工单推进中；**本批：三个 owner 收敛**（release 签名按「谁持有事实谁拥有」拆分、圆角阶梯一份真相、中性 API 一份基线）；开关几何收敛为 widgets 的唯一真源；地球增强有自己的 typed 阶段与降级策略 |
| 8 | 部分 | L | 已补 `--locked` 与 main 守卫门禁（仅 fmt+guards，不编译）；修复 main-gate.yml 缺失 setup-python/Pillow 导致结构守卫失败的问题；扩大 Gradle JVM 堆至 6GB 并将三大 flavor 分离独立进程构建，彻底解决打包 OOM；已交付 Android CI 套件 flavor/模拟器裁剪与 Gradle 缓存复用（GAP-8.1 办结）；**本批：四条流水线效率改动**——① release 作业在昂贵的 flavor 构建之前校验签名材料，缺 secret 秒级失败；② Android 质量门（lint/ktlint/Detekt/架构守卫）接入结构门与本机入口 `scripts/quality/android-quality-gates.sh`，一次跑完三 flavor 并带自测；③ 截图验收改为批量模式，失败不中断整批，逐格给出 certified/uncertified 与原因，并用黑盒像素差替代自报 `activated=true`；④ 新增生态门禁 workflow（示例编译、SemVer 类型检查、无头混沌矩阵），触发策略已登记，只有受控 `v*` tag 才起 job；全仓 `cargo nextest --workspace --no-fail-fast` **5372/5372 通过（15 skipped）**、`bash scripts/quality/check-structure.sh` **exit 0** |

## 最近一批问答（2026-10-20 第三表面补齐、页面模型与远端门禁批次）

1. x86_64/aarch64：本批未动交叉构建腿；`scripts/cross-check.sh`（zig 0.14.1 + `cargo-zigbuild`、`cargo-xwin`）与 `scripts/android-check.sh`（NDK 29.0.14206865）保持全绿结论，`cross-build-guard.py` 把四条桌面 triple、两条 aarch64 交叉腿与两条 Android 腿钉在位且拒绝 Apple 目标混入；最终安装包与目标设备证据仍缺（GAP-1.1）→ 部分（L）。
2. Windows/Linux/macOS：Windows 安装包与 macOS/iOS 目标机腿在斗战胜佛等受控环境；本批的可推改动是 Linux 打包腿（deb 与 AppImage 从登记表产出、安装-启动-卸载冒烟通过）与 release 作业的签名前置 fail closed → 部分（L）。
3. Android/iOS：Android 静态质量门成为真门（Lint 68→0、ktlint 5046→0、Detekt 0 条、架构红线守卫 137 点 0 违例 + 23 条负例），Android 单测 192 core host + 391 app 全绿；release 变体改用用户 keystore 签名、缺 secret 时 Gradle 配置期以缺失清单失败、debug key 回退已删除；**本批新增 Android 三 flavor 模拟器取证流**：回执绑定 APK 摘要、业务 id、截图与证明该步发生过的 logcat 片段，像素判决复用桌面同一把尺子；真机腿仍在斗战胜佛；iOS 仍是 host stub、显式 `Unsupported` → 部分（L）。
4. 各 UI：四大表面统一经中立 `SurfaceReader` + `CommandSink`；**本批：第三表面补上六类页面载体**（DNS、编辑器、过滤与聚合、诊断、同步、应用路由），十九条原本无载体的业务有了页面，两个「声明了但无调用点」的回调也有了真实控件；**本批：远端门禁首次全绿**——`main-gate`（`cargo fmt --all --check` + 全部结构守卫）在推送后通过；Bevy 侧模态层改用官方 `Dialog`/`ModalDialog`/`RequestClose`，业务状态保留为 typed stage → 部分（L）。
5. 复用：**四个 owner 收敛**——release 签名决策只有一个 owner；圆角阶梯只有一份真相（契约裁决三个互斥角色，重复的 `DIALOG` 档退役，widgets 镜像逐档对照合同测试）；中性 API 只有一份基线（1182 项 SemVer 基线 + 会拒绝故意破坏的负例）；**页面模型开始有唯一 owner**（`infiltrator-presentation` 拥有日志页与连接页的页面模型，Iced 的第二份日志查询状态、字符串排序键与本地排序归一化已删除）→ 部分（L）。
6. 功能接入：59 业务 × 三表面 354 证据格为 anchored 296 / pending 58 / missing 0，177 清单格为 implemented 120 / ready 2 / pending 55；**本批：16 条业务在 Slint 有了载体与表面行为测试**，但交互路径覆盖仍不完整，注册表如实记为 pending 并逐条列出已证明与仍缺的路径——只派发命令不算已认证 scenario；**四个 pending 行为锚点已全部 anchored**；Android 三 flavor 的取证流已交付，真实回执随收口批次产出 → 缺失（L）。
7. 模块化：文件级达标；隐式状态机语义审计 + 判决台账常驻，16 处阶段表已改 typed enum + typed transition；**本批：页面模型有唯一 owner 并开始收编各表面**；三个 owner 收敛（release 签名、圆角阶梯、中性 API 基线）；开关几何收敛为 widgets 的唯一真源；地球增强有自己的 typed 阶段与降级策略 → 部分（L）。
8. CI：**本批：远端门禁首次全绿**——`main-gate` 在推送后通过（格式化 + 全部结构守卫）；**本批还修掉了三个真实缺陷**：① 推送到远端才暴露出 27 个文件偏离格式化器的形状（本地提交期间门禁不跑），现在提交即格式化，远端树的形状与格式化器一致，而正在被别的会话写的文件用「只改索引里的提交形状、不动工作树」的方式修复，两边都不丢改动；② Android 取证流的测试在干净检出上因为目标目录不存在而失败，现在测试自己建目录；③ release 作业此前用 debug key 签名发布产物且不带任何签名 secret，现在缺 secret 即在昂贵的构建之前失败。生态门禁 workflow（示例编译、SemVer 类型检查、无头混沌矩阵）已登记触发策略，只有受控 `v*` tag 起 job → 部分（L）。

## 更新纪律

- 批次提交信息不需要复述八问，但本文件必须与批次同一提交更新。
- 任何一行从「部分/缺失」变成「完整」，必须同时给出对应的 `L`/`C`/`D` 证据；只有代码合并、没有证据的，保持原结论。
- 发现新缺口时先加行，再修复；禁止先删行或改结论来「过门」。