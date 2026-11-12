# 跨表面设计资源强制标准

> 层级：L2

本文是 Iced、Bevy、Slint、Android Compose 四类 UI 表面共享字体、字重、图标与视觉资产的唯一强制标准。规则按自底向上顺序落地：权威表 → 字节/文件 owner → 生成适配 → 被动消费 → 强制门禁。任何表面不得在页面、视图、组件或宿主层自行推导字体、字重、图标身份。

## 1. 唯一权威（Single Authority）

- `infiltrator_contract::design_resources` 是跨语言设计资源的唯一 typed 真源，持有：
  - `FontWeightTier` 闭集：`Regular=400`、`Medium=500`、`SemiBold=600`；
  - `FontFaceToken::ALL` 的 **7 个物理面**：Inter 400/500/600、JetBrains Mono 400、HarmonyOS Sans SC 400/500/600；CJK 只有一个家族名 `HarmonyOS Sans SC`，不再有 `font.CJK-Regular` 单面假设；
  - **26 个 `TextRole`** → `TypefaceSpec` 单表 `typeface(role)`，以穷尽匹配绑定 family/weight/size；具体语义允许共享物理三元组，但只有 authority 显式登记的语义组可以重复，新别名一律由生成器拒绝；
  - `CONNECTION_COLUMN_WEIGHTS` 与 `HERO_FACT_COLUMN_WEIGHTS` 是连接表与 hero 事实列的唯一布局权重，生成器把它们导出为绝对分数；
  - `IconSemantic` 闭集与 `IconTier::Canonical`；
  - `PLATFORM_NATIVE_ICONS` 显式存异表，每项必须写理由。
- `src/bin/export_design_authority.rs` 是唯一导出入口，产物为 `crates/infiltrator-contract/assets/design-authority.json`。禁止手写、局部改写或以另一份 JSON/YAML/常量替代。
- 新增字重、角色、语义图标或物理字面的唯一合法顺序：先改 contract typed 表与导出测试 → 重新导出 JSON → 重新生成所有适配物 → 迁移全部生产消费者 → 删除旧入口。禁止先加 UI 常量后补契约。

## 2. 字体与字重（求同）

- 所有 UI 表面只能请求 `TextRole`，不得请求裸字号、裸字重数字或工具包私有字体名。
- 权重只允许 `400 / 500 / 600`，必须与随包物理面一一对应。禁止 `700`、`650`、`550`、`450`、`Bold`、工具包合成假粗、可变字体插值来「近似」层级；层级差异用角色、字号、颜色或留白表达。
- `infiltrator_shared::effect_assets::FontFace` 是物理字体字节的唯一 owner。`FontFace::bytes()`、`FontFace::all_bytes()`、`FontFace::catalog_id()` 与 `FontFace::assert_catalog_alignment()` 保证 catalog 记录、文件路径与嵌入字节双向闭合。
- `crates/infiltrator-shared/assets/asset-catalog.json` 是字体身份与许可证的唯一库存。禁止在各 surface crate 内 `include_bytes!` 跨 crate 相对字体路径、复制 `.ttf/.otf` 或运行时网络取字体。
- `android/app/src/main/res/font` 是生成目录，不得 git 跟踪字体副本。Gradle `GenerateSharedFonts` 从 shared catalog 拷贝 **7 张面**，Android `AppFontFamily` 由生成器按 catalog ID 注册 400/500/600。
- Slint 的 7 个字体 `import` 由 `scripts/generate_design_resources.py` 从 authority/catalog 写入 `components/design-type.slint`；`theme.slint` 及其他 surface 文件不得再写跨 crate 字体路径，也不得把单张 CJK 面当整个家族。

### 2.1 当前裁决：单角色单物理面、CJK 三面齐备

本批裁决保持「一次 `TextRole` 请求解析为单个 `TypefaceSpec`」，不引入 surface 私有 family 链。所有 CJK 角色绑定同一个 `HarmonyOS Sans SC` 家族名，但按语义分别绑定到 `font.CJK-Regular`、`font.CJK-Medium` 或 `font.CJK-SemiBold` 三个独立 TTF；字重差异来自物理面，不来自渲染器合成假粗。Inter 三面继续作为 `FontFamilyToken::Sans` 的完整物理库存，供 typed `Sans` 角色与非本切片表面适配使用，不得把「角色暂时没有引用 Inter」写成删除这三面的理由。

只读元数据核验：`CJK-Regular.ttf`、`CJK-Medium.ttf`、`CJK-SemiBold.ttf` 的 OS/2 `usWeightClass` 分别为 400、500、600，name 16 均为 `HarmonyOS Sans SC`；三者字节互不相同，catalog 共用 `HarmonyOS-Sans-LICENSE.txt`。`design-resource-authority.py` 会在结构门读取这些元数据并比对 authority/catalog/生成器导入，新增或替换物理面时必须同步唯一真源。实测边界仍成立：100% 与 105% 缩放下的字形锐度无可测差异（实心笔画中位宽恒 2.0px），niri 输出为整数 `scale 2`，因此「字体发虚」不归因于分数号或宿主缩放；不得再引入 `Math.round(size / 1px) * 1px`、700/Bold、合成假粗或 `animate font-weight`。

上段的 b127 时效结论是**当时**的切片状态，不是本标准的当前状态：此后身份变化已按标准多次整批重采，最新发布批为 `20261109T150048Z_a2c3a2924bf6_dirty_1644937`（197/197），`receipt-freshness` 按运行绑定归零。本段保留「物理字体注册/角色消费变更必须触发整批重采」这条长效规则；具体批次号一律以最新 manifest 与 `source_fingerprint` 为准。

## 2.2 产品吉祥物（品牌资产，不是图标语义）

- `mascot.musicfrog-cutout`（`crates/infiltrator-shared/assets/mascot/musicfrog-cutout.png`）是 MusicFrog 青蛙乐手形象的**唯一**透明 RGBA 主资产，由 `asset-catalog.json` 与 `design-authority.json` 的 `mascot` 条目共同登记；任何 surface 不得另存副本、重绘或改色。主 plate 为 **1254×1254**、8-bit RGBA、真透明角与高频细节门（拒绝把 440×440 参考图放大冒充高清）；`musicfrog-hero-512.png` 只是同一 catalog ID 下由 shared catalog 以 `hero-512` variant 登记的 **512×512 预采样 hero 绘制档**，供约 132–150px 的真实 hero 盒取样，避免 10× 实时降采样把眼镜、琴弦和西装纹理变成锯齿。两者都在 shared owner，生成器只写出 Slint 的 `MascotAssets` 适配物，UI 不持有路径字面量或第二套形象。
- 律动不是 UI 私有动效：`infiltrator_contract::mascot_motion` 用显式 `MascotMotion`（Idle/Starting/Running/Stopping）与 `MotionCue` 表达互斥状态，typed transition + 穷尽匹配；`infiltrator_presentation::mascot_motion_projection` 把 owner 生命周期读回与用户意图投影成唯一 `MascotMotionSpec`（sway/pulse/note 进度/控制字形）。Slint 只被动消费该 spec，节拍与减弱动态效果由 contract 裁决，capture 帧冻结为确定性静帧。
- **许可状态是硬门**：当前形象是按用户提供的第三方插画做身份保持高分辨率重绘，原始授权未知，`MascotAssetDocument.redistributable = false`；`design-resource-authority.py` 强制主 plate ≥1024px、RGBA/透明角/真实边缘细节、512px hero 绘制档与许可状态文件存在。未取得书面授权前不得进入发行包、安装器与 Android/Windows 资源清单；状态记录见 `docs/reference/MUSICFROG_MASCOT_LICENSE.md`。

## 3. 图标（求同存异）

- 所有 UI 表面只请求 `IconSemantic` 或其生成物对应枚举，不得在业务视图里写 `Icons.*`、`Material Symbols`、文件名、字形 codepoint 或本地文件名猜测。
- `assets/ui/iced/svg/` 的 canonical Lucide SVG 是 Tier A 唯一矢量源。Iced、Bevy 白模 PNG、Slint `Image`、Android `CanonicalIcons.kt` 均从该源读取或生成；渲染格式变体由 `scripts/generate_shared_icons.py` 派生，不属于第二套图标。
- `infiltrator_shared::effects_assets::white_ink_icon` 与 `infiltrator_shared::assets::iced_icon_svg/bevy_icon_png` 是合法 adapter；任何 surface 不得另建同义图标 helper、转发 facade 或文件目录。
- 允许存异的前提是写入 `PLATFORM_NATIVE_ICONS` 且带理由。当前仅登记：国旗全彩资产、产品启动图标 OS 密度/ICO 变体、Android 通知/磁贴小图标、Slint 测速旋转帧。未登记差异一律视为违规。
- 禁止 `-dark.svg`、`-active.svg`、`-white.svg` 等颜色/状态派生文件作为独立资产入库；颜色、active、disabled 由 renderer `colorize`/tint 表达。禁止 `material-icons-extended` 或任何第二套通用图标族进入产品依赖。

### 3.1 国旗是矩形资产，只有一个几何（b177 → b185 → b189 裁决）

- 用户反馈「一般国旗都是方形 最好不要做这种圆形旗帜图案容器」共到达四次（b177 / b185 /
  b189 各一次，b193 第四次附的还是同一张图）。四次反馈的唯一现实结论：**旗帜是矩形，容器
  就是这块矩形本身**——只要组件或资产里还留着圆角，回执读到的就是奖章。
- **这条判据必须是可量的，不能靠"看着是方的"**：发布帧里这枚身份旗的墨迹盒等于该表面声
  明的旗帜盒（桌面档 `38×28`，2x 档 `76×56`），并且**顶行跨度等于盒宽**——套印圆角会在顶
  行两端各留出约一个半径的空缺（b189 实测改动前顶行只有 32/38，改动后 38/38）。测法只用
  现有发布回执：取该帧的红/白像素掩膜，比较首行跨度与盒宽即可；缺口即违规，不需要新工具。
- `flag.<code>` 的**唯一清单**是 `crates/infiltrator-shared/assets/flags/<code>.svg`，且
  **只有一个几何**：`0 0 640 480` viewBox 的 4:3 全出血矩形、直角。255 面来自 flag-icons
  （MIT，`LICENSE-FLAG-ICONS`）；`ac`/`ea`/`ta` 三面 flag-icons 未发布，是 Twemoji 原作
  （CC BY 4.0，`LICENSE-GRAPHICS`）按同一矩形重新铺板的派生件。b177 已退役的
  `variant: "circle-144"` 圆形 144px 栅格派生物（同 258 个 ID 的第二份 artwork）不得回归。
- 呈现侧同样只有一个 owner：`FlagIcon` 画 4:3 盒（无 `clip`、无 `border-radius`、
  `image-fit: contain`、高 = `size * 3 / 4`），尺寸入口名为 `size`（矩形宽）。旧 `diameter`
  是供应商给圆形容器取的名字，本仓已退役该入口——调用点再喂 `diameter` 会被当成「声明了却
  没人喂的输入」静默忽略，因此 `scripts/quality/slint-flag-shape-guard.py`（SLINT-020）把它
  钉成结构违规。
- **任何持旗容器不得声明圆角**（b185 的「不超过边长八分之一」已在 b189 收紧为零容忍，因为
  资产自己也不再带圆角），`LumenTheme.country-chip-radius` 作为退役符号不得回到 theme。
  加重即违规：容器一圆，矩形立刻变奖章。
- 索引是 owner ISO 代码与这份清单之间唯一的桥：`FlagImages.flags` 的顺序必须等于 catalog
  `flag.*` 的顺序，且 `infiltrator-slint` 的 `flag_assets::index_for_code` 必须与之一致；
  任一漂移都会让一个国家代码画出另一个国家的旗帜，守卫两侧都咬。
- `crates/infiltrator-slint/lumen-source/assets/flags/circle-png/` 里的圆形副本**保留**：
  那是逐字节保存的供应商导入快照（由 `slint-source-import-guard.py` 与
  `SOURCE_IMPORT_MANIFEST.json` 钉住），不是本仓清单，不得被当作可消费的资产。
- 上游 FlClash 根本不画圆形国旗（行内是 emoji 或两字母文字徽记），因此这条不是「存异」，
  而是把本仓自造的容器形状改回事实：旗帜归旗帜、圆徽归徽记（选中态对勾仍是圆形）。
- 清单只有一个，消费它的表面不止一个（b219 起）：`scripts/generate_flag_assets.py` 从同一
  `asset-catalog.json` 生成 `crates/infiltrator-iced/src/flag_assets_generated.rs`，Iced 的
  `view/flag_plate.rs` 因此画的是**同一批 4:3 矢量**，不再画平台 emoji 字形。Bevy 与 Android
  仍不消费这份清单（Bevy 的旗位现在只剩两字母文字，栅格板与重采另批收口；Android 只出地区
  文字）。任何新消费者只能吃同一矩形几何与同一 owner，不得另建第二套圆角或裁剪。
- **跨表面通则**：任何表面都不得把国旗放进声明了圆角或裁剪的容器里，也不得把国旗画成 Uni-
  code regional-indicator 字形——b219 的已发布 Iced 帧证明后者同样是本条反馈：`proxies-dark`
  的 HK 节点是波浪边 emoji、未识别节点是**字面圆形 🌐**，形状与可用性都押在系统 emoji 字体上。
  现状逐表面为——Slint 画共享 4:3 资产（`CountryChip` / `FlagIcon`，零圆角），Iced 画同一批
  矢量的 `flag_plate`（零圆角、无旗时是方形代码板），Bevy 旗位为 ASCII 代码文字，Android 不画
  国旗 artwork。两条都由 `scripts/quality/flag-plate-guard.py`（FLAGART-001）与 SLINT-020 咬。

## 4. 适配物生成（唯一写入者）

- `scripts/generate_design_resources.py` 是 Kotlin 字体/字重/排版/语义枚举与 Slint `DesignType` global 的唯一写入者；`scripts/generate_shared_icons.py` 是 Android canonical `ImageVector` 的唯一写入者。
- 生成物头部必须声明来源，禁止手工编辑。修改生成物必须先改 contract 或生成器；`--check` 必须在结构门通过。
- 旧手写适配物必须在新入口迁移完成的同一切片删除，不得保留新旧双轨、兼容常量或 fallback。

## 5. 强制门禁与证据（不可绕过）

`scripts/quality/design-resource-authority.py` 是资源标准的判据实现，由 `scripts/quality/check-structure.sh` 在每次结构门强制运行；`.github/workflows/main-gate.yml` 与 Android CI 必须执行同一结构门。判定项：

1. `design-authority.json` 与 contract exporter 重新生成的字节完全一致；
2. 所有 Kotlin/Slint 生成物 `--check` 完全一致；
3. `.slint font-weight` 与 Kotlin `FontWeight.*` 只出现随包闭集；
4. Rust surface 不含跨 crate 字体 `include_bytes!`，catalog 字体 ID 与 contract 字体 ID 完全一致；
5. Android `res/font` 无 git 跟踪实体，Gradle 生成器已注册；
6. Android 业务源码除 `AppIcon.kt`/`CanonicalIcons.kt` 外不出现 `Icons.`，Gradle 不含第二套图标族依赖；
7. Slint 只引用 canonical SVG；`ui/lumen/svg/` 仅允许 `PLATFORM_NATIVE_ICONS` 登记的帧；
8. `IconSemantic` 全部为 Canonical 且每个 catalog ID/文件存在；`PLATFORM_NATIVE_ICONS` 每项理由非空；
9. Android 语义枚举与 resolver 穷尽覆盖 contract 目录，不创造未登记语义。
10. 吉祥物主 plate ≥1024px 且通过透明角、alpha (0,255)、真实高频边缘细节门；同一 catalog ID 的 512px hero 绘制档由 shared owner 唯一登记，生成适配物字节一致，surface 目录无第二份青蛙 PNG；资产身份变化必须重采双视口真实像素回执。
11. 国旗清单只有一个形状（§3.1）：由 `scripts/quality/slint-flag-shape-guard.py`（SLINT-020）在结构门强制——`FlagImages.flags` 必须逐条等于 catalog `flag.*` / `source` 的顺序、`flag_assets::index_for_code` 必须与之一致、shared 目录不得再有 `circle-144` 记录或 `flags/circle-png/`、`FlagIcon` 不得 `clip`/圆角/`cover`，调用点不得喂退役的 `diameter` 入口。
12. 旗面只有一个清单、且没有任何表面画 emoji 旗（§3.1，b219）：由 `scripts/quality/flag-plate-guard.py`（FLAGART-001）在结构门强制——`crates/infiltrator-iced/src/flag_assets_generated.rs` 的代码集必须逐条等于 catalog `flag.*` / `source`、`generate_flag_assets.py --check` 必须 fresh、身份层源码（Iced `view/`、Bevy `pages/`、Slint `ui/` 与投影、presentation、shared `country_flags`）不得出现 regional-indicator 旗形或 🌐 兜底、退役入口 `node_flag_emoji` / `emoji_flag` / `flag_icon_token` / `proxy_flag_icon_token` 不得回归、`flag_plate` 必须画矢量且圆角为 0。别名表（contract `geo_region`、domain `COUNTRY_DEFS`）匹配用户节点名里的 emoji 属输入解析，不在本条范围。

验收证据最少包含：

- `L`：`cargo nextest run -p infiltrator-contract -p infiltrator-shared -p infiltrator-iced -p infiltrator-bevy-widgets -p infiltrator-slint`、定向 `cargo check --all-targets`、`design-resource-authority.py --skip-exporter`、Gradle `compileComposeDebugKotlin` 与 `testComposeDebugUnitTest`；
- `L`：结构门全绿，且 `python3 scripts/generate_design_resources.py --check` / `python3 scripts/generate_shared_icons.py --check` 全绿；
- `D`：字体、字重、图标形状或吉祥物资产发生身份变化时，必须重采 Iced/Bevy/Slint standard+compact 双视口真实像素回执，Android 三 flavor 交互截图回执由 `resolve_surface_evidence.py` 与 `visual_receipts.py` 判定；缺任一项只能写「部分」，不得宣称通过；
- `C`：tag CI 与 main-gate 工作流执行同一结构门与导出一致性检查，记录 run ID。

## 6. 违规判定

以下任一情形视为架构违规，批次必须退回：表面持有第二份字体/字重/图标真源；增加族外权重或合成假粗；保留旧图标族、第二套矢量源或颜色派生资产；手写生成物、手写 parity 常量或以自断言冒充跨端校验；未登记的平台分叉；身份变化后复用旧像素回执。

本标准的执行细则与当前实现见 [跨产品共享 UI 包契约](SHARED_UI_PACKAGES.md)、[代码质量底线](CODE_QUALITY_BASELINE.md) 与 [收口八问](CLOSURE_QUESTIONS.md)。
