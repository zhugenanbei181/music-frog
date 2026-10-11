# infiltrator-slint

> 层级：L3

MusicFrog 第三套 UI。桌面与 Android 使用同一套导入并改编的 Slint 页面；布局按窗口宽度和 Android 平台切换，产品状态与命令由现有共享 owner 提供。此 crate 只持有 Slint 组件和前端状态，桌面通过 `infiltrator-desktop` 接入共享 application，Android 通过 `infiltrator-android` 接入唯一 Android owner。

完整页面包括首页、代理、配置、网络接管、连接、日志、规则/provider、流量、应用设置、关于与许可证。可执行操作映射到共享 `CommandIntent`，数据只从 `SurfaceSnapshot` 与共享 presentation projector 投影；逐页契约和没有匹配共享 owner 的入口见 [表面契约](docs/SURFACE.md)。

Slint 及编译器 crate 固定为当前最新稳定版 `1.18.1`，并使用 `=1.18.1` 精确版本约束；稳定版依据和平台构建命令见 [平台构建说明](docs/PLATFORM_BUILD.md)。

文案、gettext catalog、SVG/PNG/ICO 与字体许可证索引由 `infiltrator-shared` 统一维护；Slint 使用同一产品 PO catalog 和共享图标/旗帜资源。跨表面资源接入与目录结构见 [`infiltrator-shared` 资源说明](../infiltrator-shared/assets/README.md)。

速率、配置名和已接受观察使用 `infiltrator-presentation` 的共享投影；owner 快照中的语言偏好驱动 Slint 运行时切换和 Rust 文案 lookup。文件选择、命名 Override、历史流量和 Android 系统 Activity 入口只有在对应平台 adapter 接入后才启用；当前界面会显示 typed unsupported 结果，不会模拟成功。
