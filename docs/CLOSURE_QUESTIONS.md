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

## 当前状态（2026-10-17）

| # | 结论 | 等级 | 缺口 / owner |
| --- | --- | --- | --- |
| 1 | 部分 | L | 中性内核 x64/arm64 × Windows/Linux/Android/macOS **八个组合**全部 `cargo check --locked` 通过；产品层仍只有 x86_64-linux，aarch64 产品包未验证（GAP-1.1） |
| 2 | 部分 | C（旧 commit）+ L | Windows/Linux v0.42.6 四条腿 success（旧源码）；macOS `Unsupported`（无 CI/无产物，GAP-2.2）；安装包未验证（GAP-2.1） |
| 3 | 部分 | L | Android 源码级接通，APK 在 v0.42.6 失败且修复未取证，真机 0；iOS `Unsupported`（626 行 stub，GAP-3.2）；APK 取证见 GAP-3.1 |
| 4 | 部分 | L | Iced/Bevy 深接入；Slint 紧凑接入（约 5k 行含测试，6 组页面回调 + Lumen 投影；无共享 kit，功能验收未完成）；Android 基础页；Slint 验收见 GAP-4.1 |
| 5 | 部分 | L | 共享 ports/kernel/kits 已复用；Slint 无 kit；77/82 presentation 模块仍产品耦合（GAP-5.1 / GAP-5.2） |
| 6 | 缺失 | L | Android running runtime operations `Unsupported`；Slint/Bevy 功能验收未完成（GAP-4.1 / GAP-3.1） |
| 7 | 部分 | L | 文件级模块化完成；跨 UI 页面状态机仍各自实现，无共享页面模型层（GAP-7.1） |
| 8 | 部分 | L | 已补 `--locked` 与 main 守卫门禁（仅 fmt+guards，不编译）；Android 模拟器套件仍不可裁剪 |

## 最近一批问答（2026-10-17）

1. x86_64/aarch64：中性内核 8 个组合（Windows/Linux/macOS/Android × x64/arm64）全部 `cargo check --locked` 通过（macOS 仅编译检查，无产物）；产品层仍只有 x86_64-linux → 部分（L）。
2. Windows/Linux/macOS：Windows/Linux 有旧 commit 的 CI 腿，macOS 无产物 → 部分（C 旧 + L）。
3. Android/iOS：Android APK 无当前证据、真机 0；iOS 只有 host stub → 部分 / 缺失（L）。
4. 各 UI：Iced 59.6k 行、Bevy 72.8k 行、Slint 约 5k 行（含测试，6 组页面回调 + Lumen 投影）、Android 23.6k 行，全部参与编译；Iced/Bevy 是深接入，Slint 与 Android 功能验收未完成 → 部分（L）。
5. 复用：ports/kernel/UI kits 已复用；Slint 无共享 kit；`interpolate` 已去重；presentation 仍 5/82 中立 → 部分（L）。
6. 功能接入：Android 运行态操作与版本安装入口未接；Slint/Bevy 验收未完成 → 缺失（L）。
7. 模块化：文件级达标，复用级不达标，Iced/Bevy 页面状态机重复 → 部分（L）。
8. CI：已补 `--locked`（release 构建与 test.sh）与 main 分支守卫门禁（不编译，`ci-trigger-guard` 强制禁止编译命令）；Android 套件仍不可裁剪 → 部分（L）。

## 更新纪律

- 批次提交信息不需要复述八问，但本文件必须与批次同一提交更新。
- 任何一行从「部分/缺失」变成「完整」，必须同时给出对应的 `L`/`C`/`D` 证据；只有代码合并、没有证据的，保持原结论。
- 发现新缺口时先加行，再修复；禁止先删行或改结论来「过门」。
