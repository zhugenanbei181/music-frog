# infiltrator-cli

> 层级：L3

命令行入口：解析参数、选择输出方式并调用共享 application/core 能力。

命令行为和错误语义复用 contract。Outbound 命令为本次 invocation 建立本地 `CoreApplication` owner session，并将当前运行进程接入该 session；CLI 不复制 outbound 业务规则。

## Outbound 命令

`outbound list` 显示当前 Core 的 typed inventory、opaque ID 与已观察到的当前选择；`outbound select <collection-id> <outbound-id>` 和 `outbound probe <outbound-id>` 使用该 inventory revision 创建 contract request。选择操作要求 membership 校验与写后读回；探测结果保留 `Measured`、`TimedOut`、`Unsupported`、`Failed` 状态。

每次 outbound 命令都通过 `ProcessCoreController` 和 `ControllerReadiness` 确认正在运行的 Core，再由本次 CLI invocation 的唯一 `CoreApplication` owner 生成 scope。各引擎适配器（Mihomo / sing-box adapter）从同一 owner 读取 scope，并为 inventory observation 产生 revision；CLI capability gate 只开放有精确版本证据的操作。没有可采用的运行中 Core 或版本证据时，命令 fail closed。

## 终端能力协商

CLI 在启动期协商终端能力，即在进入任何交互循环之前确定渲染层该用多少颜色、该走哪条图形协议：

- 色彩深度是 typed 档位：`truecolor` / `ansi256` / `ansi16`；上限由终端自报的能力决定，`NO_COLOR` 与 `FORCE_COLOR` 是使用者的显式约束。
- 图形协议是 typed 档位：`kitty` / `sixel` / `none`。
- 协商只读终端自报的环境信号（`TERM`、`COLORTERM`、`TERM_PROGRAM`、`KITTY_WINDOW_ID`、`XTERM_VERSION`、`WT_SESSION`），全程不读终端输入，因此既不会抢占按键，也不会因为等待终端应答而卡死。

`infiltrator terminal capabilities` 打印协商到的能力，`--json` 给出结构化视图；`infiltrator terminal quantize <rrggbb>` 把一段真彩折算到三档色彩，用来核对主题色在受限终端上的表现。
