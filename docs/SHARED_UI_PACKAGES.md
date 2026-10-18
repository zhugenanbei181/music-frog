# 跨产品共享 UI 包契约

> 层级：L2

通用 UI 能力由共享仓库 `libs/zhuge-ui-kit`（单 workspace）持有，产品负责业务和宿主适配。

| package | 当前职责 |
| --- | --- |
| `zhuge-ui-core` | 文本分段、字素边界与固定/可变尺寸虚拟窗口 |
| `zhuge-ui-tokens` | 工具包无关的已解析颜色值 |
| `zhuge-ui-kit-iced` | Iced surface、badge、富文本、虚拟列表 body |
| `zhuge-ui-kit-bevy` | Bevy BSN surface、官方 headless button 基础场景 |

Slint 当前复用共享 contract、application 与纯 presentation 投影；共享仓库中尚无
`zhuge-ui-kit-slint`。添加跨产品可复用的 Slint 组件前，先在共享仓库登记并建立明确的
module owner，再由产品接入；不得在本 crate 先复制通用算法或创建同义 facade。产品专属
视图、主题与宿主适配仍由各产品持有。

权威远端是私有仓库 `YellowWhiteBlackCat/zhuge-ui-kit`，四个包独立版本。
根 Cargo.toml 统一声明 Git URL、完整 rev 和版本范围；子 crate 通过 workspace 继承。
同一产品的四个包使用同一 rev，产品各自提交 Cargo.lock，保留独立发布周期。
其他设备只需远端读取权限，无须固定的兄弟目录布局。

## 产品开发流程

1. 新增组件或修改通用行为前，先查共享包 crate README 的模块 owner。
2. 如果现有能力不足，优先扩展对应共享包，然后接入产品。禁止在产品复制同类算法、
   创建转发 facade 或以本地同义 API 隐藏共享 owner；直接 import 定义模块。
3. 先判定差异是否必要。业务、平台或无障碍确有不同才保留；纯历史习惯应统一。
   必要差异通过显式布局/颜色/搜索策略参数或产品适配表达，不按产品名分叉。
4. 产品继续拥有业务命令、筛选谓词、拼音匹配、文案、主题预设、宿主与平台 I/O。
   Iced 与 Bevy 的状态分别留在自己的工具包，不强制抽象两者没有交集的能力。
5. 改共享包同时检查 TaskForest 与 MusicFrog 的受影响消费者；不要只验证当前产品。
   同一切片迁完旧调用并删除被替换实现，保留既有业务、布局、输入和无障碍契约。

## 升级与验证

共享包当前是 `0.1.x`：同一兼容线内的功能/修复只升 patch；破坏性更新升到 `0.2`。
稳定版本按 SemVer：major 破坏、minor 兼容增加、patch 兼容修复。
公开类型、工具包版本、MSRV、feature、平台支持以及布局/焦点/滚动变化均需兼容性评估。
共享包发布要求由其仓库的 `docs/EVOLUTION.md` 持有；产品升级前阅读迁移说明。

本地联调使用共享仓库的 `scripts/local_patch.py enable/disable/status`，
只修改本机 Cargo 配置中的同 URL patch，不提交产品 path 覆盖。
提交前关闭 patch，运行 `cargo metadata --format-version 1` 恢复 Git 来源的锁文件，
再用 `--locked` 检查；构建与捕获证据必须纳入实际 Git checkout 或 patch 源码。
私有源码认证与联调细则见共享仓库的 `docs/GIT_WORKFLOW.md`，认证信息不得入库。
Git rev 升级由各产品决定，不迫使所有产品同步升级；CI 接入另行安排。

验证要覆盖受影响构建、行为/几何测试、仓库门禁和真实界面。
截图必须绑定实际二进制与源码；没有目标平台证据就记录未验证，不能以桌面结果代替 Android。

本产品执行 `bash scripts/test.sh --guards-only` 和受影响 nextest（`-j 4`）；
具体原生捕获入口见 [Iced](../crates/infiltrator-iced/docs/README.md) 与
[Bevy](../crates/infiltrator-bevy-ui/docs/README.md)。

## 跨表面资源（本地化与视觉资产）

所有 UI 表面共用 `infiltrator-shared` 的文本 lookup 与视觉资产身份；surface 只适配 renderer，不持有第二份翻译源或重复源文件。

- 本地化：`infiltrator_shared::locales::Lang` 解析稳定应用文案 key 与产品 gettext message ID，来源为 `crates/infiltrator-shared/locales/product/`；Slint 编译器读取同一份共享 `.po`。运行期用户数据（profile 名、代理名、日志、controller 值）保持 opaque，不作为翻译 key。
- 视觉资产：`infiltrator_shared::assets::AssetCatalog` 从 `crates/infiltrator-shared/assets/asset-catalog.json` 解析语义 ID、格式与变体；目录记录 SVG/PNG/ICO/字体及其来源与许可证。render adapter 嵌入或解码所选格式，不重定义资产身份。
- 实现与更新流程见 [`infiltrator-shared` crate](../crates/infiltrator-shared/README.md) 与 [asset catalog guide](../crates/infiltrator-shared/assets/README.md)。
