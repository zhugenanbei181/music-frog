# infiltrator-iced

> 层级：L3

MusicFrog 的 Iced 桌面产品。负责路由、视图、输入、托盘呈现和 UI 状态机，通过桌面组合根消费共享 application。

通用徽标、card style 和富文本来自 `zhuge-ui-kit-iced`；纯文本投影与
固定行高窗口算法归 `zhuge-ui-core`。两者同属共享仓库 `libs/zhuge-ui-kit`。
保留产品主题、字体、文案、路由和业务消息；公共能力直接从定义模块导入。
根 Cargo.toml 固定共享 Git rev；本地开发用共享仓库的 Cargo patch 工具联调。
提交前关闭 patch 并恢复 Git 锁文件，产品各自保留锁文件与平台 feature。

不要在 UI 中启动 core、读写 canonical 配置或重建业务 projection。Iced 本地说明与截图流程见 [docs/README.md](docs/README.md)。

产品默认构建不包含 demo fixture；截图构建由 `capture-tools` feature 显式启用。统一测试脚本为 Iced/Bevy 测试启用该 feature，发布产品不启用。

代理页排序使用 `OutboundSortOrder` 与 Core-owned outbound preferences；运行诊断页使用独立的 `RuntimeDelaySortOrder`。probe URL/timeout 由 Core 持有的 `RuntimeProbeSettings` 统一投影到 Iced/Bevy；旧无版本 TOML 键只在 Core 设置加载器中迁移到 schema v2。

通用 UI 功能先扩展对应 `zhuge-ui-*` owner，再在本 crate 接入；不得复制共享实现。
产品适配保留业务职责，更新共享依赖时同时验证其他消费者和可见行为。
