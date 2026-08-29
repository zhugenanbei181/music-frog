# Iced 前端截图（demo 模式 / native render evidence）

本目录存放 infiltrator-iced 前端的**原生 iced 渲染证据**：应用以内置 demo 模式
（确定性假数据，`INFILTRATOR_DEMO=1` + `--demo`）在一个**后台合成器栈**中启动，
截图通过 niri IPC（`niri msg action screenshot-window`）完成，并绑定到精确的
app-id + PID + window-id。不使用 grim，也不截取宿主桌面。

## Background compositor stack / 后台合成器栈

采集全程在后台运行，**操作员桌面上不会出现任何窗口、不会发生焦点抢占**：

1. `kwin_wayland --virtual`（独立的私有 `XDG_RUNTIME_DIR`，清空
   `WAYLAND_DISPLAY`/`DISPLAY`）作为不可见的虚拟合成器宿主；
2. 嵌套 niri 以该宿主为父 Wayland 连接启动 —— niri 自己的 winit 窗口因此
   只存在于虚拟宿主内部，对操作员会话完全不可见；
3. demo 应用连接嵌套 niri 的私有 Wayland socket；niri 配置关闭
   hotkey-overlay（无任何合成器 UI 弹层），输出固定 `scale 1`，应用窗口
   `open-floating` 以保持 `INFILTRATOR_WINDOW_SIZE` 声明的尺寸。

kwin、niri、应用各自运行在独立进程组（`setsid`，pgid == pid 校验），
`trap EXIT/INT/TERM` 保证无论如何都不残留孤儿进程。

## How to run / 运行方式

```bash
bash scripts/capture-iced.sh                          # 全矩阵（18 场景）
bash scripts/capture-iced.sh proxies-dark             # 单个场景
bash scripts/capture-iced.sh overview-dark,sync-light # 子集
INFILTRATOR_CAPTURE_SCENARIOS=proxies-dark bash scripts/capture-iced.sh
```

- 场景矩阵：`scripts/capture_iced_scenarios.tsv`
  （`name	page	skin	window_size`），覆盖 9 个页面 × light/dark @1180x780。
- 每个场景输出 `docs/screenshots/iced/<scenario>.png`，manifest 发布为
  `docs/screenshots/iced/manifest.tsv`
  （列：`scenario page skin requested_window app_pid window_id width height bytes sha256 status`）。
- 就绪判定：应用首帧渲染后向 marker 文件追加一行
  `CAPTURE_READY page=<page> skin=<skin>`；窗口发现与截图均要求
  `app_id` 与进程 PID 同时匹配（`niri msg -j windows`），截图前重新校验
  窗口仍然存活；截图带 5 次重试预算。
- 尺寸/哈希由 PNG IHDR 头（python3 struct，无第三方依赖）与 `sha256sum`
  直接计算。niri 的 `screenshot-window` 会把合成器阴影边距一并截入
  （与参考项目 taskmanager 同栈同表现），因此 PNG 略大于窗口尺寸；
  校验要求 PNG 完整**包含**场景声明的窗口渲染区域（宽高均不小于
  `INFILTRATOR_WINDOW_SIZE`，fail-closed），嵌套输出固定 `scale 1`。
- 合成器栈（kwin 虚拟宿主 / 嵌套 niri）未能启动时脚本以
  `BLOCKED (compositor)`（退出码 3）退出；单场景失败时已成功的截图与
  manifest 行仍会发布，但整体退出码非零。每次运行的完整证据
  （niri 配置、日志、window.json、marker、原始 PNG、metadata）保留在
  `target/iced-evidence/<run_id>/`。

## Evidence rules / 证据规则

- manifest.tsv 反映**最近一次运行**；每次运行会先删除本次所含场景的旧
  PNG 再整体重写 manifest，不要手工编辑 PNG 或 manifest。
- 截图只允许来自 demo 模式的确定性数据；提交前仍需人工检查不得包含宿主
  主机的用户名、路径、窗口标题、网络信息或图像元数据。
- 更新截图时整批重新生成，禁止用旧运行的结果替换部分场景（stale evidence）。

## CI policy / CI 策略

**CI 不运行截图采集。** 捕获依赖 `kwin_wayland --virtual`、嵌套 niri 与
本机 GPU/软件渲染栈，属 local-only 工作流（与参考项目 taskmanager 的政策
一致）。CI 只验证脚本语法与 manifest 的静态一致性（如适用），不产出或
校验图片内容。
