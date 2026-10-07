# infiltrator-bevy-widgets

MusicFrog 的 Bevy 原生控件层，基于官方未加皮肤的 `bevy_ui_widgets` 提供按钮、文本输入、卡片、导航、图表和移动手势组件。界面结构由 `bsn!` 场景组合，视觉来自共享主题令牌，输入与绘制状态通过 ECS 原位同步。

控件层不依赖 mihomo 控制器、产品页面或宿主服务。共享文案与语言资源负责文本、无障碍标签及占位提示更新；切换语言保留控件实体、用户草稿、焦点和 IME 状态。产品语言偏好由使用该库的前端接入。

该库与 Bevy UI 产品共用 workspace 和锁定的 Bevy 版本。无头行为验证使用 `cargo nextest run -p infiltrator-bevy-widgets`，静态检查使用 `cargo clippy -p infiltrator-bevy-widgets --all-targets`。这些检查覆盖控件行为，不代表产品所有场景及平台已经验收。

原生宿主可安装 `ModernSurfacePlugin` 为通用卡片启用 GPU Squircle 材质，使用模块化 WESL；`SurfaceShaderMode` 控制 shader/平面回退。布局与主题更新复用材质，静态帧不改写资产。波形、环图、直方图及二维拓扑各自持有可写纹理，绑定和资产丢失时恢复，销毁或移除绘制角色时释放引用。临时零尺寸保留卡片的一个材质缓存，显式平面回退释放它。无头组合默认保留原有平面呈现。
