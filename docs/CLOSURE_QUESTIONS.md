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
| 1 | 部分 | L | 中性内核 x64/arm64 × Windows/Linux/Android/macOS **八个组合**全部通过；产品层本机已建立标准自动化复跑脚本 `scripts/cross-check.sh` 与 `scripts/android-check.sh` 并实测全绿：`aarch64-unknown-linux-gnu`（zig 0.14.1 + `cargo-zigbuild` 编译 `zstd-sys`/`ring`）、`aarch64-pc-windows-msvc`（`cargo-xwin` clang sysroot）、`aarch64-linux-android`（NDK 29.0.14206865），覆盖 core/composition/desktop 与 Iced/Bevy/Slint 全部通过；最终安装包与目标设备证据仍缺，由 CI runner 与设备矩阵取证（GAP-1.1）。本批只治理 Bevy UI 编写口径文档与看板，未触及交叉构建腿 |
| 2 | 部分 | C（旧 commit）+ L | Windows/Linux v0.42.6 四条腿 success（旧源码）；macOS `Unsupported`（无 CI/无产物，GAP-2.2）；安装包未验证（GAP-2.1）；Windows 安装包与 macOS/iOS 目标机腿已整条迁入斗战胜佛并写明阻塞方，Linux 打包腿在现在佛 `APPID-001.3`。本批未触及任何平台构建腿 |
| 3 | 部分 | L | Android 源码级接通，Bevy host driver lockfile 已同步 0.50.0 解决 buildRustDriver 101 错误，APK 待 CI 打包验证，真机 0；iOS `Unsupported`（626 行 stub，GAP-3.2）；模拟器 M1 腿可推（`PARITY-001.4`、过去佛 `BANDROID-002.1`/`002.2`），APK 与真机腿在斗战胜佛。本批未触及 Android/iOS 腿 |
| 4 | 部分 | L | Iced/Bevy 深接入；Slint 已导入完整 Lumen 页面并接入共享 owner；四大 UI 表面统一经中立 `SurfaceReader` + `CommandSink` 单向管道交互（PUB-004.2/004.3 闭环，管道 conformance nextest `ae3c3fa9-e02c-4fed-b5ad-9e1d1db95738` 2/2 全绿）；**parity 注册表实测只含 iced/bevy 两表面各 118 单元格，Slint 0 单元格，4 个单元格仍 pending**；0.5x 四套模板与 0.6x 治理文档已归档；**Slint 全量审计实测：缺 dns/editor/filter/mixin/sync/app_routing/doctor 七类页面（19 条业务无载体）、只派发 42 种 `CommandIntent`、59 条业务里 2 条完整实现 / 18 条部分实现 / 39 条未实现且今天零 scenario 锚点、4 个测试文件重实现 Slint 逻辑（第二业务真源）、2 个已声明回调无调用点**；parity 注册表已补齐第三表面（177 单元格全部可解析，Slint 57 pending + 2 implemented）；补齐由现在佛 `PARITY-001.2`～`001.4` 与 `SLINT-001.1`～`001.9` 承担；**Bevy UI 编写口径已全量机器强制且存量清零**：结构层（`bsn!` 唯一入口 + ECS 访问边界）由 `scripts/quality/bevy_bsn_guard.py` 强制（BEVY-BSN-001～010 + BEVY-ECS-007～009，356 文件零违例）；尺寸层（rem/em）由产品守卫 `scripts/quality/bevy_rem_guard.py` 强制（BEVY-REM-001～005，覆盖共享包律法全部规则），180 处非白名单 px 全量迁到 rem 通道、胶囊圆角改 `BorderRadius::MAX`、一次性布局尺寸改角色常量，权威台账归零；⑦ 小件收敛已机器化（列表场景 `bsn_list!`、批量退役 `despawn_all`、胶囊 `BorderRadius::MAX`、`template_value` 硬零）；owner 收敛裁决为「产品守卫 = 上游律法严格超集、共享包门禁降为上游镜像」；**字号档位像素回执已闭环**（1.0/1.5 两档同场景真实截图逐像素比对 46.2% 像素变化、差异覆盖整帧），剩余仅 Iced 侧命令面板圆角对齐一项跨表面对齐**；**本批：Slint 补齐无窗口行为台**（`slint::surface::harness` 在真实 `AppWindow` 上注册真实回调并回报派发动作），四个形状级测试文件改为驱动真实表面（41 条表面行为用例）；**截图认证跑通第一格**：Iced `logs-search-highlight` 双视口真实像素回执通过（`activated=true` + 原生 bounds + 进程/窗口绑定 + SHA-256），并抓到修掉两个真缺陷（验收 discovery 漏 `capture-tools` features、`product_build.py` 仍指向搬走前的 Bevy 资产目录且漏了 Slint 的运行时资源根） |
| 5 | 部分 | L | 共享 ports/kernel/kits 已复用；Slint 无 kit；PUB-001.3 part 3 租约切换下沉；GAP-5.1/5.1.1 完成；GAP-5.2 完成；GAP-5.2.1 完成；0.44 UI 管道（PUB-004.1～004.3、PUB-006.3）与 0.5x 四大 UI 脚手架模板（PUB-005.1～005.5）全部高质量办结归档；presentation 纯净化与开源合规审计全绿。本批未新增共享层，只登记 Bevy UI 尺寸通道的复用缺口（`rem()` + `REM_BASE` 唯一换算入口待落地） |
| 6 | 缺失 | L | 59 条业务能力中 Slint 缺全部 scenario 锚点与 standard/compact 双视口像素回执（其中 19 条业务在 Slint 连页面都不存在，2 条已实现但缺表面测试）；桌面三表面 177 单元格已全部登记并可解析（Iced contract 59/59、scenario 58/59；Bevy 59/59 + 59/59；Slint contract 57/59、scenario 0/59），覆盖守卫 `parity-coverage-guard.py` 报 anchored 292 / pending 62 / **missing 0**，即每个业务都已指派到每个表面、缺口全部登记在案而非静默通过；Android 三 flavor 0 个实际交互截图回执；Android running runtime operations `Unsupported`；覆盖矩阵与收口条目见现在佛 §二/§三（`PARITY-001.2`～`001.4`、`PLATFORM-001.1`、`SLINT-001.1`～`001.9`）。本批新增的 Bevy UI 尺寸通道缺口属编写口径而非业务覆盖，不改变本行结论；**本批：Slint 侧 59 个 scenario 仍全部 pending（19 条业务连页面都不存在）**，Android 三 flavor 0 个交互截图回执 |
| 7 | 部分 | L | 文件级模块化完成（<=800/600 行全绿，全仓彻底清退全部遗留 mod.rs 至 0 处，严格遵照 foo.rs + foo/ 模式，并在 check-structure.sh 接入 mod-rs-guard 永久禁止）；跨 UI 页面状态机仍各自实现，无共享页面模型层（GAP-7.1）。本批把 Bevy UI 能力规范的执行状态集中进同一台账（强制化台账），收口条目落在 C 主线 §7.1；**本批把状态表达红线机器化**：新增语义审计 `implicit-state-audit` + 判决台账（23 条候选逐条人工判决，未判决或 open 即门禁失败），16 处真实阶段表改写为 typed enum + typed transition（其中 3 处原本可表示非法组合），9 处判定为合法独立事实并记录理由；Bevy 尺寸层的 rem 通道与双守卫已落地 |
| 8 | 部分 | L | 已补 `--locked` 与 main 守卫门禁（仅 fmt+guards，不编译）；修复 main-gate.yml 缺失 setup-python/Pillow 导致 structure guards 失败的问题，并在 test_visual_receipts 补齐 skipUnless 容错；同步 android/bevy-host/driver/Cargo.lock 解决 release 工作流 Android 打包 101 报错；扩大 Gradle JVM 堆至 6GB 并将三大 flavor 分离独立进程构建，彻底解决打包 OOM；已交付 Android CI 套件 flavor/模拟器裁剪与 Gradle 缓存复用（GAP-8.1 办结）。本批无 CI 变更，新增的 rem 守卫尚未存在（`BEVYCAP-001.1`）；**本批**：测试策略精确化为只允许 doctest 形态并覆盖未跟踪文件，新增 rustdoc 门禁（docs.rs 元数据 + 零告警 + doctest 通过）、Bevy 尺寸双守卫、隐式状态机语义审计与覆盖守卫，均接入结构门；全仓 `cargo nextest --workspace --no-fail-fast` 4632/4632 通过 |

## 最近一批问答（2026-10-20 隐式状态机与文档发布批次）

1. x86_64/aarch64：本批未动交叉构建腿；`scripts/cross-check.sh`（zig 0.14.1 + `cargo-zigbuild`、`cargo-xwin`）与 `scripts/android-check.sh`（NDK 29.0.14206865）保持上一批全绿结论，安装包与目标设备证据仍缺 → 部分（L）。
2. Windows/Linux/macOS：Windows 安装包（`APPID-001.4`、`GAP-2.1`）与 macOS/iOS 目标机（`GAP-2.2`、`GAP-3.2`）腿在斗战胜佛等受控环境；本批可推腿是 Bevy 尺寸通道迁移（rem 化 + 双守卫）与 Linux 打包（`APPID-001.3`）→ 部分（L）。
3. Android/iOS：模拟器 M1 腿可推（`PARITY-001.4`、过去佛 `BANDROID-002.1`/`002.2`），APK 与真机腿在斗战胜佛；iOS 仍是 host stub、显式 `Unsupported` → 部分（L）。
4. 各 UI：四大表面统一经中立 `SurfaceReader` + `CommandSink`（管道 conformance nextest `ae3c3fa9-e02c-4fed-b5ad-9e1d1db95738` 2/2）；Slint 补齐无窗口行为台，四个形状级测试文件改为驱动真实表面（41 条表面行为用例，72/72 全绿）；截图认证跑通第一格：Iced `logs-search-highlight` 双视口真实像素回执通过（`activated=true` + 原生 bounds + 进程/窗口绑定 + SHA-256），并因此抓到并修掉两个真缺陷；**Bevy 尺寸通道本批全量机器强制且存量清零**（180 处非白名单 px 全量迁 rem 通道、权威台账归零；`BEVY-REM-001`～`005` + `BEVY-BSN-007`～`010` 生效，widgets 410/410、bevy-ui headless 552/552、结构门全绿）→ 部分（L）。
5. 复用：Slint 的动作规则原先在测试里重实现（路由模式映射、网络模式序列、主题白名单、十六进制解析、连接排序），形成第二业务真源，现收拢为唯一实现 `slint::surface::action`，测试改驱动真实表面；跨 UI 页面模型层仍未建立（GAP-7.1）；**Bevy 侧同一开关几何在四个页面各写一份常量**（轨道 38×22、旋钮偏移 18），已立 `WIDGET-001.1` 收敛为 widgets 的共享 switch 控件（补 owner 结构，不在上层打补丁）→ 部分（L）。
6. 功能接入：59 业务 × 三表面 177 单元格全部登记，覆盖守卫报 anchored 292 / pending 62 / missing 0；Slint 的 59 个 scenario 仍全部 pending，Android 三 flavor 0 个交互截图回执；**Bevy 侧 ⑦ 小件收敛已机器化**（列表场景 `bsn_list!`、批量退役 `despawn_all`、胶囊 `BorderRadius::MAX`、`template_value` 硬零），本批的字号档位像素回执已闭环（1.0/1.5 两档同场景真实截图逐像素比对 46.2% 像素变化），只剩 Iced 侧命令面板圆角对齐一项跨表面对齐 → 缺失（L）。
7. 模块化：文件级达标（<=800/600 行全绿、mod.rs 清零）；状态表达红线机器化：语义审计 + 判决台账（23 条候选逐条判决，未判决或 open 即门禁失败），16 处真实阶段表改写为 typed enum + typed transition（3 处原本可表示非法组合），9 处判定为合法独立事实并记录理由；跨 UI 页面模型层仍未建立；**Bevy 场景律法 owner 已收敛**为「产品守卫 = 上游律法的严格超集，共享包门禁降为上游镜像」，同一站点不留两份台账放行；**能力规范②④的浮层存量缺口也已收口**（八个浮层 owner 补 `ThemeContext(SurfaceLevel::Floating)` 与官方 `FixedNode`，机器规则 `BEVY-BSN-011` 由标记定义自动推导 owner）、**modal 层已迁官方 `Dialog`/`ModalDialog`/`ModalDialogBarrier`/`RequestClose`**（barrier/Esc/焦点圈闭归官方，阶段为 typed `ModalStage`）、**开关几何收敛为 widgets 的唯一真源**（`infiltrator_bevy_widgets::switch` + typed `SwitchPosition`，四个页面不再各写一份常量） → 部分（L）。
8. CI：测试策略从「禁止一切 `cargo test`」精确化为「只允许 doctest 形态」（`cargo test --doc` 是 nextest 无法执行的文档示例）并覆盖未跟踪文件；新增 rustdoc 门禁（docs.rs 元数据 + 零告警 + doctest 通过）、Bevy 尺寸双守卫、隐式状态机语义审计与覆盖守卫，均接入结构门；七个中性 crate 1436 个公开项补齐真实 Rustdoc + 7 条可执行 doctest（353/353 全绿）；全仓 `cargo nextest --workspace --no-fail-fast` 4632/4632 通过 → 部分（L）。

## 更新纪律

- 批次提交信息不需要复述八问，但本文件必须与批次同一提交更新。
- 任何一行从「部分/缺失」变成「完整」，必须同时给出对应的 `L`/`C`/`D` 证据；只有代码合并、没有证据的，保持原结论。
- 发现新缺口时先加行，再修复；禁止先删行或改结论来「过门」。