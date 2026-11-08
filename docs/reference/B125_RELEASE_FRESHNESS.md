# b125 发布时效说明

> 层级：L3

本文记录 b125 字体公共体系中间态与 b126 当前发布的身份边界，不另立任务看板。

## 历史发布批次（b125/b126，已被 b127 取代）

- 批次：`.evidence/captures/slint/20261107T051115Z_9107f001fa16_dirty_885864/`
- `scenario_count=197`；`CAPTURE MATRIX PASS: 197 scenario(s) in 477s`
- `source_fingerprint=db81da53d92847bd983a04ad0d3d8a9de208db5565f8ac82d786b23e85e3d78b`
- `binary_sha256=6fb87e9195c3b1ff32de7e81ff38df811ec9f99c0635d451713a8c6255898720`
- b124：`.evidence/captures/slint/20261107T030409Z_9107f001fa16_dirty_231838` 仍保留为历史发布批次，但其像素已被 b125/b126/b127 取代，不再作为当前结论判据。
- 用户附件 `/tmp/codex-clipboard-6676cfc4-ee07-497e-98f3-8ba31f439c0e.png` 是 b124 之前的旧 dock 截图，失效；按「过时截图即时删除 / 不翻旧账」纪律不作为当前判据。

## 当前发布批次（b127）

- 批次：`.evidence/captures/slint/20261107T080044Z_9107f001fa16_dirty_2012886/`
- `scenario_count=197`；`CAPTURE MATRIX PASS: 197 scenario(s) in 333s`
- `source_fingerprint=a6a6324028002882b655a36edea7694674b0367d5e231b7a5ec0e3ec98e4f49f`
- `binary_sha256=f19e3629309814c868e85a8ecdbe722bc8f1da64f6449c45dcdaeaf15773e55f`
- `receipt-freshness=0`；`screenshot-identity scanned=563 aliases=10 violations=0`
- b127 继续把卡头/服务 chip/策略 tile/导航/页头几何收敛到公共 token；服务数值完整显示，配置名截断减少，dock 与侧栏 active/inactive 统一物理字重；悬浮 dock、无滚动条、青蛙按钮叠加不变量保持。

## b128 源码已前进，b127 像素不再新鲜

- 2026-11-07 16:20 以后，字体物理面、`TextRole` 公共体系与移动 dock 长效机制的源码继续改动；`receipt-freshness-guard.py` 已把 `source_fingerprint` 对账升级为硬门，当前源码与 b127 的 `a6a6324028002882b655a36edea7694674b0367d5e231b7a5ec0e3ec98e4f49f` 不一致，因此 b127 的 197 张 PNG 仅是历史发布批次，不得再引作当前 UI 证据。
- 所有并行源码切片收口后，由主收口方统一执行 `bash scripts/capture-slint-matrix.sh`，再跑 `python3 scripts/quality/receipt-freshness-guard.py --mode enforce` 确认 `violations=0`。在此之前不写「当前截图已验证」。

## b125 中间态 / 证据边界

b125 发布口径登记 31 个 typography roles、typed role resolution、64 mixed → 0，以及 `slint-typography-role-guard`；其中 6 个角色与既有三元组完全重复，属于中间态而非当前终态。b126 已核对数量差异、合并重复三元组、清理 theme 死 token、处理非字体 UI 细节，并以 197/197 整批重采发布；b125 像素不再作当前判据。

## 证据等级

- 本地：`L`（`capture-metadata.txt` 批次身份、整批 197 帧回执、typography guard `findings=0`）。
- CI / 真机 / 目标产物：`C` / `D` 本说明未取得，不写「已提交」「已就绪」或「完整」；全仓重型验证与提交状态另按统一验证批次如实登记。
