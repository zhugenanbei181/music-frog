# 跨产品共享 UI 包契约

> 层级：L2

通用 UI 能力由共享仓库 `libs/zhuge-ui-kit`（单 workspace）持有，产品负责业务和宿主适配。

| package | 当前职责 |
| --- | --- |
| `zhuge-ui-core` | 文本分段、字素边界、固定/可变尺寸虚拟窗口与最宽带律（`virtual_list::max_band_rows`）、离散行分页（含页码轴与 row_budget 有界物化）、区间→单位区间的唯一投影（`value::interval_fraction`，退化答案由调用方声明）、标量线性混合的唯一算式（`easing::lerp`）与过渡时序 |
| `zhuge-ui-tokens` | 工具包无关的已解析颜色值 |
| `zhuge-ui-kit-iced` | Iced surface、badge、富文本、虚拟列表 body |
| `zhuge-ui-kit-bevy` | Bevy BSN surface（card/stat row）、官方 headless button 基础场景，以及回收池的槽位回收律（`sync_virtual_slots` + `VirtualScrollSlot` 收据守卫） |

Slint 当前复用共享 contract、application 与纯 presentation 投影；共享仓库中尚无
`zhuge-ui-kit-slint`。添加跨产品可复用的 Slint 组件前，先在共享仓库登记并建立明确的
module owner，再由产品接入；不得在本 crate 先复制通用算法或创建同义 facade。产品专属
视图、主题与宿主适配仍由各产品持有。

共享包发布于 crates.io（源码托管于私有仓库 `YellowWhiteBlackCat/zhuge-ui-kit` 单 workspace），四个包独立版本。
根 Cargo.toml 统一声明 registry 版本范围（如 `zhuge-ui-core = "0.1.7"`）；子 crate 通过 workspace 继承。
产品各自提交 Cargo.lock，保留独立发布周期；其他设备只需 registry 读取权限，无须固定的兄弟目录布局。

## 依赖方向：toolkit-neutral 契约可直达 core

`infiltrator-contract`、`infiltrator-shared` 等 toolkit-neutral 层可以直接声明
`zhuge-ui-core`：边界守卫禁用的是具体 UI/传输/后端依赖，不禁用共享纯能力 owner。
`infiltrator-contract::motion_spec` 的四条 M3 曲线属于产品身份，`CubicBezier` 的参数
求解与标量混合属于共享数值事实，因此前者留产品常量，后者直接调用
`zhuge_ui_core::easing::{CubicBezier, lerp}`，不保留第二解算器、别名或兼容 facade。
旧 `motion_spec::CubicBezier::solve` 已由 `retired-surfaces.json` 钉死；若该符号在
contract 源码或产品文档复活，retired guard 与四门共同失败。产品可见行为变化必须有
逐帧数值/像素平价证据；证据留在私有收据，不进入公开仓库。

批准的边：`contract -> core`、`shared -> core`。`zhuge-ui-tokens -> core` 当前不引入：
`ColorRgba` 的逐分量混合是颜色类型自己的面，尚未证明与单标量 `lerp` 是同一事实；
若出现真实第二权威，再按 minor 计划评估。上述选择由 core-boundary guard 的禁依赖
清单、retired-surface guard 与随 `zhuge-ui-core` 发布的四个机械门共同执行。

## 产品开发流程

1. 新增组件或修改通用行为前，先查共享包 crate README 的模块 owner。
2. 如果现有能力不足，优先扩展对应共享包，然后接入产品。禁止在产品复制同类算法、
   创建转发 facade 或以本地同义 API 隐藏共享 owner；直接 import 定义模块。
3. 先判定差异是否必要。业务、平台或无障碍确有不同才保留；纯历史习惯应统一。
   必要差异通过显式布局/颜色/搜索策略参数或产品适配表达，不按产品名分叉。
4. 产品继续拥有业务命令、筛选谓词、拼音匹配、文案、主题预设、宿主与平台 I/O。
   Iced 与 Bevy 的状态分别留在自己的工具包，不强制抽象两者没有交集的能力。
5. 改共享包同时检查 TaskForest 与 Infiltrator 的受影响消费者；不要只验证当前产品。
   同一切片迁完旧调用并删除被替换实现，保留既有业务、布局、输入和无障碍契约。

## 共享律法的机械门

共享包把每条律法的**唯一执行器**随 crate 发布：`zhuge-ui-kit-bevy` 的
`tools/check_bevy_ui.py`（BSN 唯一构造路线 + rem 唯一 authored 长度）与 `zhuge-ui-core` 的
`tools/check_band_arithmetic.py`（最宽带算术只有一个 owner）、
`tools/check_interval_normalization.py`（值进入 `0.0..=1.0` 只有一个投影）与
`tools/check_scalar_mix.py`（`A + (B - A) * t` 只有一个标量混合）与
`tools/check_easing_owner.py`（`smoothstep` 斜坡形状与「按进度解 cubic-Bézier 曲线参数」各只有一个 owner；
着色器原始字符串里的 WGSL 内建与只求值不解参数的曲线由 fixture 证明放行）。本仓库 `scripts/quality/` 保存这五份
文件的**逐字节拷贝**与各自的棘轮台账（`bevy_scene_ratchet.json`、`band_arithmetic_ratchet.json`、
`interval_normalization_ratchet.json`、`scalar_mix_ratchet.json`、`easing_owner_ratchet.json`）；
台账记录执行器的摘要与所属函数，vendored 拷贝被改动而未同步台账时，`test_*_gate.py` 立即失败，
并同时证明台账外的新增违规会咬（含同文件两处逐字相同违规各算一笔）。律法正文只有共享包 crate README 一处，产品不得另写第二套检查或第二套算术。

## 升级与验证

共享包各 crate 独立处于 `0.x`：功能与修复升 patch，破坏性更新升 minor（例如 `zhuge-ui-kit-bevy 0.3.0 → 0.4.0`）；
1.0 起按 SemVer：major 破坏、minor 兼容增加、patch 兼容修复。
公开类型、工具包版本、MSRV、feature、平台支持以及布局/焦点/滚动变化均需兼容性评估。
共享包发布要求由其仓库的 `docs/EVOLUTION.md` 持有；产品升级前阅读迁移说明。

本地联调使用共享仓库的 `scripts/local_patch.py enable/disable/status`，
只修改本机 Cargo 配置中的 `[patch.crates-io]` 与 `[patch.<git-url>]` 双源覆盖，不提交产品 path 依赖。
提交前关闭 patch，运行 `cargo metadata --format-version 1` 恢复 registry 来源的锁文件，
再用 `--locked` 检查；构建与捕获证据必须与锁文件记录的发布版本及其 checksum 一致。
私有源码认证与联调细则见共享仓库的 `docs/GIT_WORKFLOW.md`，认证信息不得入库。
包版本升级由各产品决定，不迫使所有产品同步升级；CI 接入另行安排。

验证要覆盖受影响构建、行为/几何测试、仓库门禁和真实界面。
截图必须绑定实际二进制与源码；没有目标平台证据就记录未验证，不能以桌面结果代替 Android。

本产品执行 `bash scripts/test.sh --guards-only` 和受影响 nextest（`-j 4`）；
具体原生捕获入口见 [Iced](../crates/infiltrator-iced/docs/README.md) 与
[Bevy](../crates/infiltrator-bevy-ui/docs/README.md)。

## 跨表面资源（本地化与视觉资产）

所有 UI 表面共用 `infiltrator-shared` 的文本 lookup 与视觉资产身份；surface 只适配 renderer，不持有第二份翻译源或重复源文件。

- 本地化：`infiltrator_contract::shared_copy` 是所有 UI 共享描述的 typed key 目录；`infiltrator_shared::locales::Lang` 解析稳定文案 key 与产品 gettext message ID，Slint 编译器读取同一份共享 `.po`，Android 三语言 `strings.xml` 同步登记同名 key。`shared-copy-guard.py` 强制各端接入。运行期用户数据（profile 名、代理名、日志、controller 值）保持 opaque，不作为翻译 key。
- 描述只写一次：中文文案不夹注英文译名，括号只保留配置字段、单位或协议标识；跨端共用键的简洁度由 `copy_conciseness_baseline.json` 棘轮守卫，新堆字失败、旧债只许递减。
- 视觉资产：`infiltrator_shared::assets::AssetCatalog` 从 `crates/infiltrator-shared/assets/asset-catalog.json` 解析语义 ID、格式与变体；目录记录 SVG/PNG/ICO/字体及其来源与许可证。render adapter 嵌入或解码所选格式，不重定义资产身份。
- 排版与图标唯一体系：`infiltrator_contract::design_resources` 持有闭合的 `FontWeightTier`（400/500/600）、`TextRole`→`TypefaceSpec` 与 `IconSemantic` 目录；`design-authority.json` 由其导出，Kotlin/Slint 适配文件由 `generate_design_resources.py` 与 `generate_shared_icons.py` 生成。字重只能是随包物理面，Tier A 图标只解析 canonical Lucide SVG；国旗、启动图标、通知小图标与 Slint 测速帧在 `PLATFORM_NATIVE_ICONS` 显式登记理由。`design-resource-authority.py` 校验生成物新鲜度、catalog 与字节 owner 双向闭合、四端不请求族外字重或第二套图标。
- 实现与更新流程见 [`infiltrator-shared` crate](../crates/infiltrator-shared/README.md) 与 [asset catalog guide](../crates/infiltrator-shared/assets/README.md)。
