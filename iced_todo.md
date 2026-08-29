# Iced 旧待办兼容索引

Iced 已经从“迁移项目”变成主桌面 surface；本文件不再维护独立路线图，也不再把视觉 polish 排在 mihomo 控制平面之前。

原阶段十（视觉现代化）已于本轮落地，事实记录如下：

- 设计系统：`src/view/theme.rs` 令牌（亮/暗双主题、iOS 蓝 accent、间距/圆角/阴影、延迟色阶）；
- 图标：`src/view/svg_icons.rs` + `assets/icons/*.svg`（28 个 Lucide 风格单色 SVG，主题色染色）；
- 组件库：`src/view/components.rs`（卡片/徽章/芯片/iOS 开关/分段控件/延迟徽章/统计卡/空状态）；
- 字体：内嵌 Inter（Regular/Medium/SemiBold）+ JetBrains Mono（OFL，见 THIRD-PARTY-NOTICES）；
- 页面：sidebar/overview/proxies 按 Clash Party 语言重设计，其余七页完成一致性打磨；
- 仍开放：骨架屏加载占位、更细粒度的微交互动效（悬停缩放/涟漪）。

Iced 的架构边界、与 Tauri/Web 和 Android 的求同存异规则见 [docs/FRONTENDS.md](docs/FRONTENDS.md)；功能任务按域维护在本地 `TODO.md`。
