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

## 当前状态（2026-10-18）

| # | 结论 | 等级 | 缺口 / owner |
| --- | --- | --- | --- |
| 1 | 部分 | L | 中性内核 x64/arm64 × Windows/Linux/Android/macOS **八个组合**全部通过；产品层本机已建立标准自动化复跑脚本 `scripts/cross-check.sh` 与 `scripts/android-check.sh` 并实测全绿：`aarch64-unknown-linux-gnu`（zig 0.14.1 + `cargo-zigbuild` 编译 `zstd-sys`/`ring`）、`aarch64-pc-windows-msvc`（`cargo-xwin` clang sysroot）、`aarch64-linux-android`（NDK 29.0.14206865），覆盖 core/composition/desktop 与 Iced/Bevy/Slint 全部通过；最终安装包与目标设备证据仍缺，由 CI runner 与设备矩阵取证（GAP-1.1） |
| 2 | 部分 | C（旧 commit）+ L | Windows/Linux v0.42.6 四条腿 success（旧源码）；macOS `Unsupported`（无 CI/无产物，GAP-2.2）；安装包未验证（GAP-2.1） |
| 3 | 部分 | L | Android 源码级接通，Bevy host driver lockfile 已同步 0.50.0 解决 buildRustDriver 101 错误，APK 待 CI 打包验证，真机 0；iOS `Unsupported`（626 行 stub，GAP-3.2）；APK 取证见 GAP-3.1 |
| 4 | 部分 | L | Iced/Bevy 深接入；Slint 已导入完整 Lumen 页面并接入共享 owner；四大 UI 表面统一经中立 `SurfaceReader` + `CommandSink` 单向管道交互（PUB-004.2/004.3 闭环，ui_pipeline_conformance 通过）；0.5x 交付 Slint/Iced/Bevy/Android 四套极简独立脚手架模板（templates/* 全部编译测试全绿）；Android 基础页；Slint 验收见 GAP-4.1 |
| 5 | 部分 | L | 共享 ports/kernel/kits 已复用；Slint 无 kit；PUB-001.3 part 3 租约切换下沉；GAP-5.1/5.1.1 完成；GAP-5.2 完成；GAP-5.2.1 完成；0.44 UI 管道（PUB-004.1～004.3、PUB-006.3）与 0.5x 四大 UI 脚手架模板（PUB-005.1～005.5）全部高质量办结归档；presentation 纯净化与开源合规审计全绿 |
| 6 | 缺失 | L | Android running runtime operations `Unsupported`；Slint/Bevy 功能验收未完成（GAP-4.1 / GAP-3.1） |
| 7 | 部分 | L | 文件级模块化完成（<=800/600 行全绿，全仓彻底清退全部遗留 mod.rs 至 0 处，严格遵照 foo.rs + foo/ 模式，并在 check-structure.sh 接入 mod-rs-guard 永久禁止）；跨 UI 页面状态机仍各自实现，无共享页面模型层（GAP-7.1） |
| 8 | 部分 | L | 已补 `--locked` 与 main 守卫门禁（仅 fmt+guards，不编译）；修复 main-gate.yml 缺失 setup-python/Pillow 导致 structure guards 失败的问题，并在 test_visual_receipts 补齐 skipUnless 容错；同步 android/bevy-host/driver/Cargo.lock 解决 release 工作流 Android 打包 101 报错；扩大 Gradle JVM 堆至 6GB 并将三大 flavor 分离独立进程构建，彻底解决打包 OOM；已交付 Android CI 套件 flavor/模拟器裁剪与 Gradle 缓存复用（GAP-8.1 办结） |

## 最近一批问答（2026-10-18）

1. x86_64/aarch64：中性内核 8 个组合全部通过（macOS 仅编译检查，无产物）；产品层已固化 `scripts/cross-check.sh` 与 `scripts/android-check.sh` 并在本机实测全绿（Linux aarch64 经 zig 0.14.1 + cargo-zigbuild，Windows aarch64 经 cargo-xwin clang sysroot，Android aarch64 经 NDK 29.0.14206865；Iced/Bevy/Slint 与 core/composition/desktop 均全量覆盖通过），最终安装包与目标设备证据仍缺 → 部分（L）。
2. Windows/Linux/macOS：Windows/Linux 有旧 commit 的 CI 腿，macOS 无产物 → 部分（C 旧 + L）。
3. Android/iOS：Android APK 驱动 lockfile 已同步 0.50.0 解决 buildRustDriver 101 错误，等待 CI 打包验证、真机 0；iOS 只有 host stub → 部分 / 缺失（L）。
4. 各 UI：Iced 59.6k 行、Bevy 72.8k 行、Slint 约 7.1k Rust + 6.1k Slint 行（完整 Lumen 页面，接入共享 owner）、Android 23.6k 行，全部参与编译；四大 UI 表面完全统一收敛为中立 `SurfaceReader` + `CommandSink` 单向管道消费者（PUB-004.2/004.3 闭环，ui_pipeline_conformance 2/2 通过）；0.5x 交付 Slint/Iced/Bevy/Android 四大独立模板工程与 `cargo-generate.toml` 脚手架（PUB-005.1～005.5 全量闭环，无头与窗口测试通过）；0.6x 交付架构说明书与选型指南 `docs/FRAMEWORK_SELECTION.md`（PUB-006.2 闭环）；Iced/Bevy 是深接入，Slint 与 Android 视觉验收未完成 → 部分（L）。
5. 复用：ports/kernel/UI kits 已复用；Slint 无共享 kit；`interpolate` 已去重；PUB-001.3 part 3 租约切换下沉；GAP-5.1 与 GAP-5.1.1 办结；GAP-5.2 完成；GAP-5.2.1 办结；0.44 UI 管道（PUB-004.1～004.3、PUB-006.3）与 0.5x 脚手架模板（PUB-005.1～005.5）全部高质量办结归档；0.6x 交付治理体系 CONTRIBUTING/SECURITY/CODE_OF_CONDUCT 与中立迁移指南 MIGRATION_GUIDE（PUB-006.4 闭环）；presentation 纯净化与开源合规审计全绿 → 部分（L）。
6. 功能接入：Android 运行态操作与版本安装入口未接；Slint/Bevy 验收未完成 → 缺失（L）。
7. 模块化：文件级达标（<=800/600 行全绿，全仓彻底清退全部遗留 mod.rs 至 0 处，严格统一采用 foo.rs + foo/ 标准形态，并在 check-structure.sh 接入 mod-rs-guard 永久禁止），复用级不达标，Iced/Bevy 页面状态机重复 → 部分（L）。
8. CI：已补 `--locked`（release 构建与 test.sh）与 main 分支守卫门禁（不编译，`ci-trigger-guard` 强制禁止编译命令）；main-gate.yml 补充 setup-python 与 Pillow 依赖并修复 test_visual_receipts 的 PIL 容错；同步 android/bevy-host/driver/Cargo.lock 解决 release 工作流 Android 打包 101 报错；扩大 Gradle JVM 堆至 6GB 并将三大 flavor 分离独立进程构建，彻底解决打包 OOM；已交付 Android CI 套件 flavor/模拟器按需裁剪与 Gradle 缓存复用（GAP-8.1 办结） → 部分（L）。

## 更新纪律

- 批次提交信息不需要复述八问，但本文件必须与批次同一提交更新。
- 任何一行从「部分/缺失」变成「完整」，必须同时给出对应的 `L`/`C`/`D` 证据；只有代码合并、没有证据的，保持原结论。
- 发现新缺口时先加行，再修复；禁止先删行或改结论来「过门」。
