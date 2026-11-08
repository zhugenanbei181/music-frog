# MusicFrog 吉祥物来源与许可状态

> 层级：L3

本文件记录 `mascot.musicfrog-cutout`（`crates/infiltrator-shared/assets/mascot/musicfrog-cutout.png`）的资产来源。**它是状态记录，不是许可裁决。**

- 2026-11-06 用户在会话中提供一张 440×440 青蛙乐手插画（`/tmp/codex-clipboard-bf4e4391-cfe4-45c0-92a2-7e6df18df91c.jpg`），明确要求「抠出来」放到 hero 启动/暂停按钮中，并指出低分辨率放大后的形象粗糙。
- 本仓保存的是按该形象做身份保持重绘的 **1254×1254 透明 RGBA 主 plate**，以及同一 catalog ID 下的 **512×512 Lanczos 预采样 hero 绘制档**（`asset-catalog.json` 均登记为 `USER-PROVIDED-REFERENCE`）。主 plate 不是原图插值放大，眼镜、琴弦、羽冠与西装纹理均为真实细节；512 档只解决约 132–150px hero 盒的实时降采样走样。二者用于本机实现与像素取证。
- **原作者、原始授权条款、是否允许商业分发与二次创作均未知**。因此当前状态为：**不得随发行包对外分发，不进入 Android/Windows/安装器资源清单，不写入第三方声明**。
- 解锁条件：用户提供原始授权凭证或书面许可范围；届时由资产 owner 一次性把 `license` / `license_path` 改为正式条款，并同步第三方声明与 Android 资源生成链。
- 若无法取得授权，须以原创或明确可授权的吉祥物替换该资产族；替换只动 shared 资产 owner、catalog 与生成适配物，不动任何 UI 消费端的私有路径。

本文件遵循跨表面设计资源强制标准；同级参考：[FlClash 移动端参考](FLCLASH_MOBILE_REFERENCE.md)。
