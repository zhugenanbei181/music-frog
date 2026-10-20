# Android 交互截图取证

> 层级：L3

本目录是 Android 三 flavor（Compose / Bevy / Slint）在模拟器上的**真实交互取证流**。
它把一次交互和它的事实绑在一起：候选 APK 的路径与 SHA-256、flavor、业务 id、步骤、
时间戳、窗口/Activity 身份、以及证明这一步真的发生过的 logcat 摘录。

**失败即封闭**：没有真正发生的步骤不产生回执；截图与上一状态没有可测差异的步骤被报告为
`no_observable_change`，而不是被算成一次交互。

## 组成

| 文件 | 职责 |
|------|------|
| `android_interaction_capture.py` | 入口：驱动一个 flavor，产出回执与报告；也负责合并三个 flavor 的回执 |
| `capture_flow.py` | 流程本体：装 APK、起飞、逐步执行、取证、判定 |
| `capture_device.py` | adb 传输层：**只接受模拟器序列号**，物理设备一律拒绝 |
| `capture_scenarios.py` | 场景表装载器（步骤 → 业务 → 动作 → 需要的证据） |
| `capture_actions.py` | 动作词汇表（typed）：`launch`、`tap_text`、`tap_nav`、`tap_anchor`、`swipe_up`、`wait_text`、`tap_dialog`、`revoke_permission` |
| `capture_anchors.py` | 原生表面的标定锚点（导航条与其它控件） |
| `capture_hierarchy.py` | 无障碍树解析与节点选择 |
| `capture_pixels.py` | 截图摘要与交互判定（判定规则复用 `scripts/parity/interaction_delta.py` 这一份真源） |
| `capture_evidence.py` | typed 证据、回执校验与回执写出 |
| `android_interaction_scenarios.tsv` | 六个步骤 × 三个 flavor 的场景表 |
| `android_interaction_anchors.tsv` | 原生表面的锚点（比例坐标 + 推导来源） |

## 步骤与业务

场景表把每一步绑到共享业务目录（`scripts/parity/feature_evidence.tsv`）里的一条业务，
守卫会校验这个绑定。

| 步骤 | 业务 | 证明什么 |
|------|------|----------|
| `startup` | `runtime-telemetry-observation` | 应用真的起来了：进程注册、Activity 恢复、首帧与起飞前的画面不同 |
| `navigation` | `shell-routing-intent-control` | 换到了另一个主区域：画面变了，Compose 还要断言标题文字 |
| `list-scroll` | `logs-scroll-lock` | 列表真的滚动了：内容区域像素差超过噪声地板 |
| `vpn-consent` | `settings-tun-permission` | 系统 VPN 同意弹窗真的出现了（logcat 里有 vpndialogs 的 Activity 记录）并被接受 |
| `permission-prompt` | `settings-permission-panel` | 通知权限弹窗真的出现了并被允许，权限状态读回为 granted |
| `vpn-terminal-state` | `settings-tun-permission` | 授权流程的终态读回（established / not-established） |

步骤之间有显式依赖：`permission-prompt` 依赖 `vpn-consent`，`vpn-consent` 依赖
`list-scroll`，依此类推。前置步骤没有被证明时，依赖它的步骤记为 `blocked`，不产生回执。

## 怎么跑

前置条件：一个**还在运行**、adb 可见的模拟器，以及三个 flavor 的候选 APK。

Gradle Managed Device 在 EXECUTOR 模式下会在测试任务结束时销毁设备，所以取证前要有一个
活着的设备。两种做法：

```bash
# A) 用钉住的系统镜像建一个 AVD 并直接起（不改任何构建文件）
SDK_ROOT="$(sed -n 's/^sdk\.dir=//p' android/local.properties | tail -1)"
API="$(python3 scripts/android-toolchain.py get android.emulator.api)"
IMAGE="$(python3 scripts/android-toolchain.py get android.emulator.image)"
echo no | "$SDK_ROOT/cmdline-tools/latest/bin/avdmanager" create avd \
  -n musicfrog-parity -k "system-images;android-$API;$IMAGE" --device pixel_6
"$SDK_ROOT/emulator/emulator" -avd musicfrog-parity -no-window -no-audio -no-boot-anim \
  -gpu swiftshader_indirect -no-snapshot &
"$SDK_ROOT/platform-tools/adb" wait-for-device

# B) 让 Gradle Managed Device 以 BOOT 动作留着设备（需要一个 init 脚本）
```

装 APK、跑三个 flavor、合并、验收：

```bash
# 三个 flavor 各跑一次（顺序无所谓，合并在最后）
python3 scripts/android/android_interaction_capture.py --flavor compose
python3 scripts/android/android_interaction_capture.py --flavor bevy
python3 scripts/android/android_interaction_capture.py --flavor slint

# 合并三个 flavor 的回执
python3 scripts/android/android_interaction_capture.py --merge .evidence/android/captures/<stamp>

# 标定辅助：打印设备事实（屏幕尺寸、密度、当前 Activity）
python3 scripts/android/android_interaction_capture.py --probe
```

退出码：`0` 该 flavor 的每一步都被证明；`1` 至少一步未被证明（已被证明的回执仍然保留）；
`2` 流程根本没跑起来。

## 设备准备（每条取证的事实）

流程在起飞前把设备摆成一个确定的状态，并把这件事写进回执的 `guest` 段：

| 事实 | 做法 | 为什么 |
|------|------|--------|
| 候选 APK 就是被取证的那个 | `pm install -r <候选 APK>`，回执记它的 SHA-256 | 不允许旧包冒充候选 |
| 飞行模式 | `cmd connectivity airplane-mode enable` | 取证期间产品不许碰网络 |
| 没有 VPN 同意 | `appops set <pkg> ACTIVATE_VPN ignore` | 同意弹窗必须真的出现 |
| 没有通知权限 | `pm revoke <pkg> POST_NOTIFICATIONS` | 通知权限弹窗必须真的出现 |

模拟器只认 `emulator-<port>` 序列号：物理设备在传输层就被拒绝，流程不可能跑到真机上。

## 产物

```
.evidence/android/captures/<stamp>/
├── compose/
│   ├── android-visual-receipts.json   # 该 flavor 的机器可读回执
│   ├── capture-manifest.tsv            # 与桌面捕获同形的清单
│   ├── receipts.tsv                    # 逐步骤摘要
│   ├── report.md / report.json         # 人类可读的步骤台账（含未证明步骤与原因）
│   └── steps/<step_id>/                # before.png / before-replicate.png / after.png / logcat.txt
├── bevy/…
├── slint/…
└── android-visual-receipts.json        # 合并后的三 flavor 回执集
```

回执集里每一条回执都带：APK 路径与 SHA-256、flavor、业务 id、步骤、时间戳、截图路径与
摘要、logcat 摘录路径与摘要、像素差测量（changed/required/noise）、无障碍断言、状态读回、
动作解析记录、Activity/窗口身份。

## 守卫

`scripts/quality/android-interaction-receipt-guard.py` 守住这条链：

* 场景表里的业务必须真的在共享业务目录里；每个步骤在三个 flavor 上必须绑同一条业务；
* 流程只接受模拟器序列号，物理设备一律拒绝；流程源码里不允许出现任何网络客户端；
* 回执必须有 APK 摘要、业务 id、logcat 证据、可测的像素差；动作没解析出来的回执不算证据；
* 一组截图全都一模一样 = 什么都没证明，整组回执被拒。

```bash
python3 scripts/quality/android-interaction-receipt-guard.py --self-test
python3 scripts/quality/android-interaction-receipt-guard.py --mode enforce
python3 scripts/quality/android-interaction-receipt-guard.py --receipts .evidence/android/captures/<stamp>/android-visual-receipts.json
```

## 模拟器能证明什么、不能证明什么

* **能证明**：应用启动、进程与 Activity 身份、主区域切换、列表滚动、系统弹窗（VPN 同意、
  通知权限）的出现与接受、授权流程的终态读回。
* **不能证明**：真机上的后台行为、真实内核流量（模拟器里没有可用的 mihomo 内核二进制，
  终态通常读回 `not-established`）、真机 GPU 合成路径。物理设备那条腿仍然是另一条证据线。
* **原生表面的弱点**：Bevy 与 Slint 在 Android 上是原生表面，不发布无障碍树，所以它们的
  导航只能靠标定锚点 + 内容区像素差来证明，路由本身读不出来。锚点没标定时，相关步骤会被
  报告为未证明，而不是猜一个坐标。
