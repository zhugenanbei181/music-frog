# infiltrator-shared

> 层级：L3

共享错误码、本地化资源、跨格式视觉资源和通用呈现工具。用户业务事实与 projection 归 domain/application；本 crate 只提供跨产品共用的基础资源。

`byte_units::format_bytes` 提供 Iced、Bevy 和 application 投影共用的字节数显示规则；新表面应复用该实现，不在 feature 或页面中复制单位阈值。

`locales::Lang` 是 UI 无关的文案接口。现有稳定文案键与产品 gettext message ID 共用它；编译型界面和 Rust 渲染界面从 `locales/product/translations/` 读取同一份 `.po`。新增键或源文案时，在本 crate 更新资源并确保每个受支持语言都有条目；用户配置名、节点名和日志正文按原文呈现。

`assets::AssetCatalog` 的使用与格式布局见 [资源说明](assets/README.md)；数据清单 `assets/asset-catalog.json` 为 SVG、PNG、ICO 和字体登记稳定 ID、格式变体及许可证来源。Iced SVG、Bevy 位图、Lumen 图标与圆形旗帜、桌面 ICO 和 Android/各桌面产品 PNG 共用此资源 owner；renderer adapter 只负责把对应格式交给自己的图像控件。
