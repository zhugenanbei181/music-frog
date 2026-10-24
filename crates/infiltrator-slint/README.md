# infiltrator-slint

> 层级：L3

MusicFrog 第三套 UI。桌面与 Android 使用同一套导入并改编的 Slint 页面；布局按窗口宽度和 Android 平台切换，产品状态与命令由现有共享 owner 提供。此 crate 只持有 Slint 组件和前端状态，桌面通过 `infiltrator-desktop` 接入共享 application，Android 通过 `infiltrator-android` 接入唯一 Android owner。

完整页面包括首页、代理、配置、网络接管、连接、日志、规则/provider、流量、应用设置、关于与许可证。可执行操作映射到共享 `CommandIntent`，数据只从 `SurfaceSnapshot` 与共享 presentation projector 投影；逐页契约和没有匹配共享 owner 的入口见 [表面契约](docs/SURFACE.md)。

Slint 及编译器 crate 固定为当前最新稳定版 `1.18.1`，并使用 `=1.18.1` 精确版本约束；稳定版依据和平台构建命令见 [平台构建说明](docs/PLATFORM_BUILD.md)。

文案、gettext catalog、SVG/PNG/ICO 与字体许可证索引由 `infiltrator-shared` 统一维护；Slint 使用同一产品 PO catalog 和共享图标/旗帜资源。跨表面资源接入与目录结构见 [`infiltrator-shared` 资源说明](../infiltrator-shared/assets/README.md)。

速率、配置名和已接受观察使用 `infiltrator-presentation` 的共享投影；owner 快照中的语言偏好驱动 Slint 运行时切换和 Rust 文案 lookup。平台特有操作由对应平台 adapter 接入，未支持操作明确返回 typed unsupported，不模拟成功。

`infiltrator_slint::surface` 是 surface 行为的唯一实现与唯一驱动入口：`surface::action` 提供 typed owner 命令构造器（surface 回调与行为测试共用同一实现），`surface::harness` 在无头软件渲染平台上注册生产回调、派发真实手势，并回报实际发出的 typed `CommandIntent`、typed `Failure` 与 owner 可见投影。行为测试不得在测试内复刻 surface 规则。

组件树遵循模块化架构，拆分为 `theme.slint`、`models.slint`、`base_widgets.slint` 与 `cards.slint`，由 `slint-syntax-guard.py` 执行行预算与语义色彩质量门禁。无感后台截图矩阵由 `scripts/capture-slint-matrix.sh` 执行，通过 `--demo` 与 `CAPTURE_READY` 契约提供像素级离线回执。

平价业务取证通过 `INFILTRATOR_SCENARIO=<feature>` 激活：该场景会真的打开对应业务在 Slint 上的界面状态，并把该界面的真实窗口几何写进 `CAPTURE_READY ... scenario=<feature> activated=true bounds=<x>,<y>,<w>,<h>`；surface 未实现的业务只会写 `activated=false`，因此截图链路不能为不存在的实现背书。场景矩阵与已发布回执见 [表面契约](docs/SURFACE.md)。

## 文档

- [SURFACE.md](docs/SURFACE.md)：前端边界、共享 owner 接入方式和当前功能范围。
- [PARITY_GAPS.md](docs/PARITY_GAPS.md)：Slint 表面业务承载页与取证矩阵台账。
- [PLATFORM_BUILD.md](docs/PLATFORM_BUILD.md)：Slint 版本依据、Windows/Linux/Android 构建入口与验证边界。
- [导入源归档](lumen-source/README.md)：原始 ZIP 文件覆盖、逐文件完整性清单与只读历史源码说明。
- [Material 组件改动记录](lumen-source/material-1.1.0/LUMEN-PATCHES.md)：vendored Slint Material 组件的产品适配和系统主题读取接口。

