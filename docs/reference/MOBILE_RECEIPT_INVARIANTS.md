# 移动端真实像素回执长效不变量（b128 独立审计）

> 层级：L3

本文是 b128 的独立只读审计交接：把四张外部 FlClash 参考图与 **b127 发布帧**逐屏对照，给出「不折不扣实现」的量化差距、可编码的验收不变量和截图时效规则。本文不写生产代码、不修改截图、不跑 `cargo`，也没有资格声明任何尚未由主收口方重跑的验证已经完成。

四张参考图是外部布局锚点，不是本仓验收证据；b127 像素只代表 16:06 发布批次，不代表 16:20 之后的活源码。参考图明载体裁切、标题重叠、emoji 与超过物理字重上限的视觉字重，均属于**反面基准**，不得照抄。

## 1. 判据与可复现测量

- 参考图：`docs/reference/flclash-mobile/flclash-mobile-{dashboard.png,proxies.jpg,profiles.jpg,tools.jpg}`；本次用 SHA-256 复核，与既有长文登记一致。
- 发布帧：`.evidence/corpora/slint/`（本地发布语料，像素不进 git，索引 `manifest.tsv` 进 git），b127 批次 `.evidence/captures/slint/20261107T080044Z_9107f001fa16_dirty_2012886/`；只取 `manifest.tsv` 中 `status=ok` 且 SHA-256 相同的 **400 × 800** 竖屏帧。
- 工具：审计环境有 Pillow / NumPy，没有 OpenCV；`/tmp/b128_audit/measure.py` 与 `/tmp/b128_audit/detail.py` 用 RGB 数组、逐行扫描、扫描线连通域、色差分类和中位数完成等价测量，报告在 `/tmp/b128_audit/measure_report.json`、`/tmp/b128_audit/detail_report.json`，联系表与裁图在同一目录。`/tmp` 文件不属于仓库回执，主收口方重采后必须重跑这些测量。
- 参考图坐标按原图像素记录，括号中给占屏宽/高百分比；b127 坐标按 400 × 800 记录，1u = 1px。参考图与发布帧的 DPR 不同，因此比较的是比例、顺序、几何分区和同屏内部关系，不把参考图绝对像素直接写成实现尺寸。
- 颜色条件采用「主题 token 的颜色」而非固定外观色：dark 的页面 / 卡片 / 强调 / 分割线实测约 `(10,11,14)` / `(29,33,38)` / `(1,104,136)` / `(38,42,48)`；light 约 `(239,240,243)` / `(255,255,255)` / `(1,104,136)` / `(213,216,218)`。像素 guard 必须从主题导出读取期望色，避免皮肤变更造成假失败；本文的实测值只锚定 b127。

### 1.1 参考图公共量化事实

| 项目 | 四屏实测 | 归一化 / 折算 | 必须实现 | 不得照抄 |
| --- | --- | --- | --- | --- |
| 页面 / 卡片颜色 | 页面约 `(239,237,240)`，卡片约 `(250,248,249)`；卡片比页面亮 11/255 | 亮卡 + 暖灰页面，四个皮肤方向一致 | 卡片层与页面层分明；底部胶囊取 contract alpha 0x99（60%）玻璃 veil，绝非实心遮盖板 | 不复制具体暖灰，本仓用主题皮肤色阶 |
| 安全边距 | 卡片 x≈48–50 到 1107（宽 1156）；页标题墨迹起笔 x≈51–52 | 左右 4.2–4.3%W；按 400W 折算约 17u | 400W 手机卡 x=16–384，宽 368u（4% / 96%） | 参考图底部内容被 dock 覆盖 |
| 卡片间距 | 24–25px；配置卡 y=228–419 / 444–635 / 660–851 / 876–1067 | 1.0–1.1%H；按 800H 折算约 8u | 每页统一 `space-mobile-card-gap`；dock 顶部空气带也是 8u，只扣一次 | 不在滚动列表底再付第二份 48u 呼吸位 |
| 圆角 | 白卡上沿起笔内缩约 18px，约 52px 收敛到直边 | 约 17.6dp、连续大圆角 | 400W 手机卡 radius≈20u；胶囊 radius=高度/2 | 不靠描边制造分层 |
| 页头 | 大标题墨迹 y≈52–142、高 63–66px；仪表盘单圆铅笔，代理「搜索+测速」胶囊 + 圆更多，配置「刷新+排序」胶囊 + 圆添加，工具无右侧动作 | 标题槽约 2.7–3.0%H；按 400W 折算标题约 22u | 四页标题左对齐，动作簇语义和数量正确，不互相重叠 | 工具页标题与「资源」组标题重叠；emoji 图标 |
| 底栏本体 | 四屏近白色连通域 x≈63–1092、高 186px；仪表盘 y=2120–2305，代理 y=2114–2299，配置 y=2093–2278，工具 y=2111–2296 | 宽 89%W、高 7.9%H；按 400W/800H 折算 356 × 63u | 悬浮圆角胶囊、四等分 tab、单一选中色 | 底栏压住仪表盘下一张卡、代理 Toast 与工具列表压到底栏附近 |
| 激活胶囊 | 参考激活段约 267 × 163px，本体上下各约 4dp | 约 90 × 55dp；全圆 | 激活态只用一个强调色，胶囊高为本体减 8u | 国旗 / emoji 作 tab 或组图标 |
| 文字与图标 | 大标题墨迹 63–66px，工具行标签约 25 × 16dp；正文一级深色，次级灰；线性图标 20–24px 无彩色底衬 | 字号 / 600 物理面近似，禁止合成 700 | 语义图标来自 canonical 资产；中文 / 拉丁 / 数值各自闭合角色 | emoji、假粗、第二套图标族、文字压在控制图形上 |
| 滚动条 | 右侧最后 20 列相对局部背景的差异 `>8` 占比为 0，JPEG 右缘众数稳定为页面色 | 无可见滚动条 | 四页全帧无垂直 / 水平滚动条 | 参考图裁切后的被压内容不能解释为「允许遮盖」 |

### 1.2 逐屏参考事实

| 屏 | 卡片顺序 / 数量 / 高度 | 行、分隔线与留白 | 标题 / 动作 | 图标语义 | 空态与反面缺陷 |
| --- | --- | --- | --- | --- | --- |
| 仪表盘 | 状态卡 y≈255–818（563px，23.8%H）→ 网络速度卡 y≈843–1377（535px，22.6%H）→ 出站模式卡 y≈1403–1779（377px，15.9%H）→ VPN 卡 y≈1805–2043（239px，10.1%H）；其后还有一张被 dock 裁切的速率卡 | 状态卡承载 7 个事实；网络速度卡内两张浅灰子卡；底部两列 label/value 左 / 右对齐。卡间 25px | 「仪表盘」+ 右上单圆铅笔 | 国旗徽章、暂停矢量、上传 / 下载、模式分叉、盾牌锁 | **必须消除**参考图 y≈2069–2190 的下一张卡被底栏压住；本仓内容止步于 dock 带上沿 |
| 代理 | 组胶囊行 y≈249–356；5 张 2 列节点卡：x=48–565 / 590–1107，y=405–602 / 627–824 / 849–1046，单卡 518 × 198px（44.8%W × 8.4%H），列间距 25px、行间距 24px；第三行只有左卡 | 每卡两行：身份 + 协议；右上角选中圆徽；测速入口在右下。日期 / 配额 chip 不额外增高 | 「代理」+ 搜索 / 测速共胶囊 + 圆形更多 + 行尾下拉 chevron | 搜索、闪电、竖三点、向下 chevron、节点国旗 / 对勾 | 大面积下半留白属于自然内容结束，不应人为补满；组 emoji 与 Toast 不可当身份图标引入 |
| 配置 | 4 张等高卡，y=228–419 / 444–635 / 660–851 / 876–1067，每张 192px（8.2%H），间距 24–25px；第二张日期 chip 与第四张选中对勾均不增高 | 两行：文件名 + 相对时间；来源 / 配额 + 选中圆徽。行尾右对齐、chip 与文字不叠 | 「配置」+ 刷新 / 排序共胶囊 + 圆形添加 | 刷新、排序、添加、选中对勾 | 配置「相对更新时间」若 owner 未发布，必须由底层补事实；UI 不得自行格式化或造第二真源。运行时长已由 Core owner 发布（typed 会话观察），UI 只渲染 |
| 工具 | 3 组白卡：资源 y≈129–352（滚动后首行被标题遮挡），设置 y≈530–1537（6 行），其他 y≈1715–2050（2 行）；设置卡每行 166–168px | 组标题槽实测约 177px（7.5%H）；分隔线 y=183/184、696/697、864/865、1032/1033、1200/1201、1368/1369 与 1883/1884，x=216–1065，线厚 2px，色 `(219,217,218)`，与卡色差 31/255（12.4%）；左端内缩 15.8% 卡宽、右端 4.0% | 「工具」在左上；无右侧动作簇 | 终端、语言气泡、画笔、列表、云、扳手、齿轮、印章、信息；行尾 chevron | **必须消除**标题与「资源」重叠、透明标题层；本仓保留 `detail` 副标题是有意差异 |

## 2. b127 发布帧逐屏对比

以下「已对齐 / 差距 / 风险」只对 b127 像素成立。16:20–16:23 的设计资源、布局 token、首页与主题 WIP，以及之后并行流的 dock typed-state WIP，均未包含在 b127 二进制中；不得用本节的任何结论代替重采验收。

### 2.1 四屏公共比对

| 检查项 | b127 实测 | 结论 | 长效风险 / 下一步 |
| --- | --- | --- | --- |
| 安全边距 / 卡片 | 页头 x=16/17；普通卡 x=17–382 或设置 / 代理卡 x=16–383；约 4%W / 92%W。浮点胶囊 x=21–378 | 参考 4.3%W 已按比例对齐 | 用像素 guard 锁住页头与卡左沿，禁止逐页写边距 |
| 页头 | 四页标题墨迹 y=16–34/35，高 18–20u；仪表盘铅笔、代理三动作、配置三动作、工具无动作；浅色代理 / 配置动作墨迹到 x≈371，深色标题与动作同槽不重叠 | 标题与动作数量 / 层级已对齐；比参考折算的 22u 略低 | 暗色代理页头动作背景 / 墨迹过暗，放大图可读性弱；应把「动作圆形至少 44u、墨迹与动作底对比度」纳入像素 guard |
| 无滚动条 | 四页最右 12 列逐列众数均为页面底色；代理 / 配置右缘模值占比 1.0；仪表盘 / 工具右缘存在卡片阴影渐变，但放大图无连续轨道或 thumb | b127 无可见滚动条 | 单靠「右缘颜色不同于页面」会把卡片投影误判为滚动条；guard 必须找 **连续 ≥8u 宽、≥30%H 的窄轨道 / thumb**，同时由静态 guard 锁住 compact `always-off` |
| dock 悬浮 | 胶囊主连通域 x=21–378、y=725–786（358 × 62u 内部纯色，描边 / 投影外溢约 x=16–384、y=724–788）；四 tab 等分，选中胶囊 x=28–110 / 115–197 / 202–284 / 289–371，y=728–783 | 几何、四等分和单一选中色已对齐 | 参考底栏本体高 63dp，b127 64u；胶囊 56u 高、上下约 4u 对齐。阴影必须留在 12u 下沿内，不得形成全幅底板 |
| 胶囊外侧只有页面底色 | 空气带 `y=716–723`、左侧 `x=0–19 / y=724–787`、右侧 `x=380–399 / y=724–787`、下方 `y=788–799` 四区，逐像素到页面色的欧氏距离均小于到卡片色的距离（100%）；卡片层与次级层像素均为 0；胶囊内 y=724–787 横切中位数 62/64 行恒为卡片色 | **用户反复强调的透明度 / 遮盖问题在 b127 已被结构 + 像素双证** | 这个结果不能靠「把 alpha 调回来」维持：typed `MobileDockBandState` 同时决定保留带与胶囊挂载、band strip 透明且只有页面 ground、胶囊填充为 contract alpha 的玻璃 veil（永不实心）、列表只扣一次带高，四者缺一都会复发 |
| 底部空隙 / 滚动到底 | `overview-mobile-end-dark` 最后一个内容到 y≈658，动作按钮到 y≈715；`settings-mobile-end-dark` 末卡到 y=715；空气带 y=716–723 无卡片层；普通首页与工具页内容也延伸到 y=715 | 滚动到底两帧无过带；仪表盘 / 工具页接近填满 | 代理 / 配置页是自然空态：`proxies-mobile-{dark,light}` 最后内容 y=340/341 到 715 有 **374–375u（46.8%H）**页面底色；`profiles-mobile-{dark,light}` 最后内容 y=198/199 到 715 有 **516–517u（64.5%H）**空白。参考对应页也留白，但配置参考是 4 卡、代理是 5 卡，b127 只有 2 张配置卡、5 张代理卡；这不是 dock 多扣空隙，而是 demo fixture 内容数量 / 密度不足 |
| 状态卡 label/value 基线 | 仪表盘 dark 墨迹带：`状态 / 运行中` y=81–93，IP y=121–135，chip y=157–166，运行时长 label y≈181–188，value y=196–205；桌面 dark 同卡 label y245–251、value y259–269 | 四层层级清楚；label 与 value 垂直分组、没有基线错位；概览卡网格四列 211u 等宽、三行卡沿全对齐（page-background 间隙 hero 696–711/923–938 = fact 696–711/923–938），手机双列 177u/177u 等宽，同构卡 meta 行同为 y646–656 | 运行时长已由 owner 发布为真实会话时长（发布帧 `03:04:49`，与参考同值）：手机竖屏 dark label 墨迹 y181–188、值 y196–205，右列「组织」值同带；桌面 dark label y245–251、值 y259–269，「出口 IP 可用」仍在 chip 行。像素 guard 应比较同卡同级 label / value 的墨迹 y 带差 ≤2u，不能只比较 CSS `font-size` |
| 青蛙 / 按钮叠加 | 仪表盘 hero 高饱和连通域 dark x=276–371、y=94–189（含 96 × 96u 圆盘），light 主体 x=279–367、y=110–174；暂停徽记 x=310–337、y=154–181 dark（light 同）；状态卡右界 x=382/383。hero / 徽记在卡外的彩色像素为 0 | b127 未出圈，暂停徽记叠在角色右下且无文字 | 圆盘只有一个 `disc-size`，控制向量由圆盘直径派生并受圆形 `clip`；重采后继续用「彩色像素不得越过状态卡 x=16–384、y=61–220」验收 |
| 分割线 | `settings-mobile-{dark,light}` 线 y=155/319/375/431/487/543/707，每行 296px 连续同色；参考工具页 2px 线、左右内缩 15.8% / 4.0% 卡宽；b127 线 x=76–371，卡 x=16–383，内缩 16.3% / 3.3%，厚度 1u | 比例和等距已对齐；1u 是 400W 下 2px @1156W 的合理折算 | 必须钉死「线属于行、边属于卡」：分隔线只从 token 派生、只测一条 1u 线，行 hover 不画第二圈描边 |
| 字体清晰度 / 字重 | 大标题墨迹 18–20u，参考折算约 22u、既有长文实测我方 display 42 × 20dp 对参考 46 × 22dp；`design-authority.json` 闭集 400/500/600，25 个 TextRole 绑到真实物理面，display 为 CJK-SemiBold 22px | b127 没有证据支持「假粗 / 700 / 分数缩放发虚」 | 可测根因是**参考标题物理字重超 600 且 DPR 不同、本仓只允许 600**：应以字号 / 字距 / 墨色拉开层级；暗色页头动作对比度低是真实可测风险。字体 family / 字重身份由 `design-authority.json` 与 `design-resource-authority.py` 静态锁住，像素只测墨迹高度、实心笔画宽度和 AA/实心比，禁止 UI 层自行换字重 |

### 2.2 逐屏差异矩阵

| 屏 | b127 帧 | 已对齐 | 仍差距 | 疑似回退 / 风险 |
| --- | --- | --- | --- | --- |
| 仪表盘 | `overview-mobile-dark/light`、`overview-mobile-end-dark`；普通帧卡带 y=61–220 / 233–365 / 378–469 / 482–551 / 562–669 / 681–715 | 状态卡、速度双子卡、三段模式、系统代理开关与多张事实卡顺序；青蛙不出圈；dock 外侧四类通道全为页面色；滚动到底末内容 y=715 | 参考只有 4 张主卡 + 1 张被压卡，b127 主卡后补了内网 IP / 配置 / 服务状态 / 内存等卡，信息量更高；运行时长已是 owner 发布的真实会话时长（`03:04:49`，与参考同值）；页头标题墨迹 19–20u 略小于参考折算 22u；参考速率数值更粗更大 | 「相对更新时间」仍是底层 owner 事实缺位，不得在 UI 格式化；「运行时长」已由 owner 发布（回执 `03:04:49`），UI 只渲染；滚动帧 y=715 与 air y=716 紧贴，若并行 dock WIP 再次双重扣带高会立刻出现空白过带 |
| 代理 | `proxies-mobile-dark/light`；组胶囊 y=72–107、节点卡 y=129–192 / 203–266 / 277–340，两列 x=17–194 / 205–382，5 张 178 × 64u | 三行两列、5 张节点、协议 / 延迟第二行、单强调色选中徽记；选中胶囊与底栏激活同色；无滚动条、无底部卡片层 | 节点卡高 64u（16%H）而参考折算约 68u，密度接近；y=341–715 的 374u 页面留白比参考图到 Toast / dock 的留白明显更长；参考有搜索 / 测速 / 更多 + 行尾下拉，b127 更按钮是三横线而非竖三点，下拉 chevron 不明显；暗色动作对比度弱 | 参考 emoji / 国旗不可照抄；若为了「填满留白」在 UI 私自增加 fixture 节点或补假卡，会制造第二业务真源。应先把 fixture 数量与参考的 5 卡契约对齐，再决定是否补真实业务内容 |
| 配置 | `profiles-mobile-dark/light`；两张卡 y=61–124、135–198，366 × 64u，间距 10–11u；首卡有配额 chip 与选中圆徽 x≈343–367 | 紧凑两行卡、日期 / 配额 chip、选中圆徽、圆角与卡间距、页头刷新 / 添加、无额外滚动条 | 参考 4 张等高 192px 卡（折算约 66u），b127 只有 2 张 64u 卡，y=200–715 形成 516u（64.5%H）空白过带；参考第一行「相对更新时间」，b127 是绝对日期 chip；参考「排序」动作在 b127 被有意移除（卡片自身可 reorder），属于已裁决差异，不应恢复第二入口 | 空白过带**不是 dock 遮盖或双扣 padding**，因为通道像素证明内容本来只到 y=198；主收口方应在底层 fixture / 投影里补齐真实配置数量与相对时间，再重采。不要在 UI 加占位卡 |
| 工具 | `settings-mobile-dark/light`、`settings-mobile-end-dark`；资源卡 y=96–215，设置卡 y=260–603（6 行 × 56u + 分隔线），其他卡 y=648–715；滚动到底末卡到 y=715 | 三组、每行 detail 副标题、chevron、分隔线 x=76–371 且等距、组标题不重叠、底栏不切卡、浅色 / 深色双皮肤同列 | 参考工具行 166–168px 折算约 56u，b127 56u 对齐；参考行无副标题，本仓保留是有意差异；参考「资源」首行被标题遮挡，b127 已消除；参考工具页无页头动作，b127 也无 | 浅色 `settings-mobile-light` 标题墨迹扫描会碎成多段（仅因浅色 AA 阈值），不能据此判「字发虚」；暗色工具页右缘渐变是卡片投影，不是滚动条 |

### 2.3 核心问题定位

用户反复感知的「底部背景透明度反复、没有长效机制」不是单一 alpha 数值，而是四个事实曾可被分别表达：① 内容带是否保留、② dock 是否挂载、③ 胶囊材质是玻璃 veil 还是实心遮盖板、④ 透明 strip 是否只允许页面 ground。任一处再次允许 `bool` 组合、`#RRGGBBAA` 常量、第二份 `padding-bottom` 或全幅底板，都会让旧截图里的分隔线 / 卡角从 tab 之间透出，或者让滚动内容被胶囊切开。长效机制必须是「typed 独占状态 + 单一几何 owner + contract 唯一 alpha 玻璃 veil（`MobileDockMaterial::VEIL_ALPHA`）+ 真实像素不变量」四层，且每条源码改动都由 guard 与下一次整批重采共同验收；只改 `scripts/quality` 或只调一个颜色都不够。

## 3. 长效不变量规格

### 3.1 静态 guard 能查的源码不变量（无需截图）

以下为**建议规格**，本审计未修改 `scripts/quality`。已有 `slint-mobile-chrome-guard.py`、`slint-surface-ladder-guard.py`、`slint-typography-role-guard.py`、`slint-viewport-tier-guard.py`、`design-resource-authority.py` 与 `receipt-freshness-guard.py` 覆盖部分条件；主收口方与 guard owner 应补齐缺口，不要复制第二套正则真源。

| ID | 静态条件 | 唯一真源 / 现有入口 | 防住什么 | 建议失败输出 |
| --- | --- | --- | --- | --- |
| `S-DOCK-STATE` | `MobileDockBandState::{Closed,Reserved}` 是唯一保存态；`transition` 穷尽匹配 intent；`draws_dock` 与 `reserves_content_band` 必须同真同假；UI 只消费 typed state；退役 `bottom-bar-present` / `bottom-bar-visible` / `hide-bottom-bar-on-scroll` 及其组合 | `crates/infiltrator-contract/src/mobile_chrome.rs`、`crates/infiltrator-presentation/src/mobile_chrome_projection.rs`、`slint-mobile-chrome-guard.py` | 内容已扣带但 dock 不画、或 dock 画了内容没扣带的第三状态 | 给出出现旧 flag 的文件和行 |
| `S-DOCK-OWNER` | `bottom-bar-band-height = dock-top-air + bottom-bar-height + bottom-bar-inset`，`bottom-bar-pill-height = bottom-bar-height - 8u`，`bottom-bar-max-width = 360u`，`bottom-bar-inset = 12u`；`AppBottomBar` 只从 theme 读几何；`AppBody` 的 `padding-bottom` 只出现一次，每个页面不得再写 dock 高度 / 底部 padding 字面量 | `theme.slint`、`app_chrome.slint`、`app_body.slint`、`lumen-app.slint`、`slint-viewport-tier-guard.py` | 双重扣带高造成空白过带；胶囊几何逐页漂移 | 输出 token 表达式差异 / 重复 padding 调用点 |
| `S-DOCK-GLASS` | strip `background: transparent`；唯一 `MobileDockPageGround` 画页面底色；胶囊 `background: LumenTheme.floating-surface`，该颜色**必须带 alpha**，且四皮肤共享 contract 的 `MobileDockMaterial::VEIL_ALPHA`（0x99 / 60%）；胶囊描边读 `LumenTheme.dock-rim-color`；禁止 dock 子树出现字面色 / `with-alpha` / `alpha` 表达；禁止 `dock-backdrop` / 全幅 plinth / `Rectangle { background: zinc-0; }` 覆盖滚动区 | `crates/infiltrator-contract/src/mobile_chrome.rs`、`theme.slint`、`app_chrome.slint`、`slint-dock-material-guard.py`、`slint-mobile-chrome-guard.py` | 材质回潮成实心遮盖条、散文与像素各说一套、透明度反复横跳 | 输出非 veil 填充 / 缺失 alpha / 未经 contract 授权的透明度表达式 |
| `S-SCROLL-POLICY` | compact `vertical/horizontal-scrollbar-policy = always-off`，桌面才允许 `as-needed`；所有 `ScrollView` 必须通过同一 body / 组件策略，禁止逐页写 `ScrollBarPolicy` 字面量 | `app_body.slint`、`slint-mobile-chrome-guard.py` | 四页右缘出现轨道 / thumb | 输出 policy 与 viewport 不一致的文件 |
| `S-DIVIDER` | `row-divider` / `row-divider-thickness` / `row-divider-inset-start` / `row-divider-inset-end` 是唯一 token；行内只用 `RowDivider` / `ListRule`；hover 只改底色，不画描边；禁止 `width: parent.width` 的满幅手绘线 | `theme.slint`、`base_widgets.slint`、`cards.slint` | 卡内多一个框、双线、线从图标 / 卡边起笔 | 输出手绘线 / 描边调用点 |
| `S-MASCOT` | `MusicFrogHero` 只暴露 `disc-size`；`clip: true` + 圆形 radius；暂停 / 播放矢量由盘径推导，盘内无 `Text`；叠加控制坐标由盘径比例推导；退役 `button-size` / `hero-scale` | `mascot.slint`、`home_compact_flow.slint`、`slint-mobile-chrome-guard.py` | 青蛙 / 按钮出圈、文字压矢量、两套尺寸 | 输出旧尺寸入口或越界表达式 |
| `S-TYPE-ASSET` | 每个 Text 只取同一 `TextRole` 的 size/weight/family；`font_weights=[400,500,600]`；无 700+/Bold、合成字重、跨 crate 相对字体路径；`design-authority.json` / Kotlin / Slint 生成物摘要一致；emoji 不进图标语义表 | `infiltrator_contract::design_resources`、`design-authority.json`、`FontFace`、`design-resource-authority.py`、`slint-typography-role-guard.py` | 假粗、字体发虚归因跑偏、第二套图标族 | 输出未登记三元组 / 字重 / 图标 ID |
| `S-RECEIPT-FRESH` | 最新全量批次必须 `scenario_count=197`；`capture-metadata.txt` 的 `source_fingerprint` 等于 `visual_receipts.py --fingerprint` 的**活源码**值；`binary_sha256` 等于归档二进制；发布 `manifest.tsv` 与运行 manifest 同 SHA / 同行集合；每个 `status=ok` PNG 存在且哈希一致；发布目录无额外 PNG；诊断批次不发布 | `receipt-freshness-guard.py`、`visual_receipts.py`、`capture-slint-matrix.sh` | 拿 b127 旧帧、旧裁图、局部诊断帧当当前结论 | 输出 recorded/current 指纹、多余 / 缺失 PNG 名单 |

### 3.2 必须真实截图回执的像素不变量

建议由 guard owner 新建 `scripts/quality/mobile-pixel-invariants.py`（仅规格，不在本切片实现）。输入：最新全量发布的 `manifest.tsv`、PNG、`capture-metadata.txt` 和主题色导出；输出 JSON `{id, scenario, pass, actual, expected}`，任一失败非零退出。先验证 source fingerprint 与像素哈希，再逐帧验证下表。所有坐标按 400 × 800 定义；若采集尺寸或 `LumenUiScale` 改变，必须由单一 scale 映射，guard 不接受临时坐标补丁。

| ID | 适用 scenario / theme | 检测区域 | 期望像素条件 | 防住的复发 |
| --- | --- | --- | --- | --- |
| `P-FRAME-ID` | 四页 × dark/light + `*-mobile-end-dark` + 三张 mascot scenario；未来重采的全量 197 行 | 整帧 400 × 800 | PNG 尺寸精确 400 × 800；SHA-256 等于 manifest；`status=ok`；manifest 所属 run 的 `source_fingerprint` 等于活源码 | 过期帧、裁图、局部诊断帧、引用 b127 旧账 |
| `P-DOCK-SIDE-GROUND` | 所有 400W 四页，dark/light/forest/amoled 只要发布 | `A=(0,716,400,724)` 空气；`L=(0,724,20,788)` / `R=(380,724,400,788)` 两侧；`B=(0,788,400,800)` 下沿 | 对每个像素，`distance(pixel,page) < distance(pixel,card)` 的比例 ≥99%；卡片层 / 次级层精确色像素为 0；允许投影使页面色产生 ≤25/255 的渐变，但不得出现连续 ≥3u 的卡片色带 | 胶囊外的全幅底板、遮盖、透明度回潮、底部奇怪空隙 |
| `P-DOCK-PILL-GLASS` | 同上 | `y=724..787`、`x=20..380`；逐行取 `x=120..280` 的非 tab 区域中位数 | 逐行中位数落在页面底色与不透明同色填充之间；与不透明填充相差 ≥6 灰阶；明显高于页面底色；禁止等于不透明填充；禁止一行内出现 ≥8 个不属于 dock / tab / 前景 token 的颜色岛；胶囊连通域 bbox 仍落在 x=16–384、y=724–788，宽 352–364u、高 60–66u；描边 / 投影不得越过 x=0 / x=399 或 y=799 | 材质回潮成实心遮盖条；tab 间透出卡片分隔线 / 文字鬼影；胶囊变成全幅 bar |
| `P-DOCK-TABS` | 四页 × 四皮肤 | `y=728..783` 四个 90u 槽；激活槽按页面依次为 x=28–110 / 115–197 / 202–284 / 289–371 | 恰好一个槽含 `accent-soft` 填充且 bbox 约 83 × 56u；其余三槽不含该填充；全部激活 / 选中徽记颜色属于同一 `root.accent` 色族，不出现第二强调色 | 选中态颜色漂移、多个 active、dock 覆盖 |
| `P-NO-SCROLLBAR` | 所有 400W mobile scenario | `x=388..399, y=52..715`；同时查 `y=700..715, x=0..399` | 找不到宽度 ≥8u、纵向连续 ≥30%H 且与局部背景差 >16 的**窄条轨道 / thumb**；也找不到高度 ≥8u、横向连续 ≥30%W 的底边轨道；右缘允许随卡片 y 变化的柔和阴影（逐行渐变、无恒定饱和 / 边框色） | 真实滚动条；同时避免把卡片投影误报 |
| `P-DOCK-BAND-ONCE` | `overview-mobile-end-dark`、`settings-mobile-end-dark`；重采后补 light end 帧 | `y=700..723`；内容 ROI `x=16..384` | 最后一处卡片 / 动作前景像素下沿 ∈ `[710,715]`；`y=716..723` 卡片层像素 = 0；从末内容下沿到 716 的页面空隙 ≤16u；禁止 724 后再出现任何卡片层像素 | 双扣带高、48u 呼吸位、内容被 dock 裁切、空白过带 |
| `P-CARD-LADDER` | 四页 × dark/light，及未来 forest/amoled mobile | 整帧 x=0..399、y=52..715；按每张卡中心列采样 | 卡片精确色比页面色亮度高（dark / amoled）或保持授权主题方向；`floating-surface` 采用 contract alpha 0x99（60%）的玻璃 veil，绝非 100% 不透明实心硬块 | 卡片沉入页面、浮层与卡片不同层、浮层变为遮盖硬块 |
| `P-CARD-GEOMETRY` | 四页 × dark/light | 卡主体连通域，限定 y<716 | 主要卡左沿 x∈[15,18]、右沿 x∈[381,385]；纵向相邻卡间页面色带 8–10u；radius 轮廓在卡角 18–22u 内收敛；dock 本体不计入卡数量 | 逐页手写安全边距 / 卡间距 / 圆角 |
| `P-DIVIDER` | `settings-mobile-*`、`settings-mobile-end-*` 及其他有列表行的 mobile 帧 | 每行 ROI `x=16..383`；线色从 theme token 读取 | 每条线恰为 1u 高，x≈76–371（内缩 16.3% / 3.3%，容差 ±3u），相邻等距 56u ±2；线色与卡色亮度差 / 色差约 12%（容差 3%）；卡内不出现第二个圆角描边闭环 | 满幅线、双线、行 hover 描边、线色过淡 / 过浓 |
| `P-PROFILES-DENSITY` | `profiles-mobile-{dark,light}`；滚动到底 scenario 如发布则同测 | `y=52..716`，卡 ROI x=16..384 | 当前 fixture 应发布 4 张等高卡；每张高 ∈[62,68]u，卡间距 8–10u；若 owner 明确变更 fixture 数量，guard 的固定期望由 contract scenario 声明驱动，而不是 UI 私自补占位；末卡下沿之后到 y=716 的连续页面色带 ≤2 × 卡高 + 空气带 | 配置页 64.5%H 空白过带；假占位卡 |
| `P-PROXIES-DENSITY` | `proxies-mobile-{dark,light}` | 节点卡 ROI x=16..384、y=120..360 | 2 列 × 5 卡，卡宽 176–180u、高 62–68u、列间距 9–12u、行间距 8–10u；第三行只有左卡；选中圆徽正好一个且在同一强调色族；节点数由 scenario 固定 fixture 驱动 | 密度退化、补第二业务真源、选中色漂移 |
| `P-HERO-CLIP` | `overview-mobile-{dark,light}`、mascot 三帧 | 状态卡 `x=16..384, y=61..220`；外部环带 `x=0..15 / 385..399` 及 `y=52..60 / 221..300` | 高饱和 hero 与暂停徽记的所有连通域 bbox 都在卡内；卡外彩色像素 = 0；徽记内无文字墨迹块；暂停 / 播放矢量两条竖线分离且不连成白块 | 青蛙出圈、按钮文字叠压、两套尺寸 |
| `P-HEADER-ACTIONS` | 四页 × dark/light | 标题 ROI `x=16..200, y=0..52`；动作 ROI `x=240..384, y=0..52` | 标题墨迹 y∈[14,38]、不进入动作 ROI；仪表盘 1 个圆动作，代理 / 配置 2+1 个语义动作，工具 0 个；动作圆直径 ≥40u；动作墨迹与动作背景的 WCAG 亮度对比 ≥3:1；禁止标题与组标题在 y=52..95 同 ROI 重叠 | 标题重叠、暗色动作看不清、动作簇数量漂移 |
| `P-TYPE-BASELINE` | 四页 × dark/light，尤其 overview 状态卡与 profiles 卡 | 同卡同级 label / value 的文字行；同事实数值行 | 同角色墨迹高度差 ≤2u；同一行 label / value 基线差 ≤2u；无 700 / 合成粗造成的 AA 灰边异常；AA/实心像素比在 dark/light 各自基线 ±20%；只读 `design-authority.json` 的 400/500/600 角色作期望 | 字体「发虚」被误修成假粗、数值基线错位 |
| `P-EMPTY-STATE` | 代理 / 配置页；其他短列表页按 scenario 声明 | 最后内容下沿到 y=716 | 连续页面底色带不得超过 scenario 声明的 `max_empty_band`；推荐配置 ≤150u、代理 ≤250u，或由真实 fixture 契约声明；dock 外侧四通道仍须满足 `P-DOCK-SIDE-GROUND` | 把 fixture / owner 缺口伪装成「参考留白」 |
| `P-NO-EMOJI` | 所有 mobile 帧 | 图标 ROI 与 dock tab ROI；静态侧由 `design-authority.json` 查语义 | 像素侧不引入未登记彩色 emoji 旗 / 火箭 / 靶心 / 城市；国旗资产若出现必须匹配 canonical 资产哈希；图标线宽 / 单色语义与生成物一致 | 参考图 emoji 被照抄、第二套图标族 |

### 3.3 验收分组与失败归属

1. **静态批次**：`S-*` guard 只检查源码与主题导出；不打开 PNG，成本秒级。任何失败不得通过放宽像素 guard 绕过。
2. **身份批次**：`P-FRAME-ID` 与 `S-RECEIPT-FRESH` 先跑；指纹不符、PNG 哈希不符、发布目录有多余 PNG 或 manifest 行数不等于 197 时，直接判整批 stale，不继续测像素，也不引用旧结论。
3. **像素批次**：四页 dark/light 八帧 + `overview-mobile-end-dark` / `settings-mobile-end-dark` + 三张 mascot 帧，一次读主题色、一次逐帧输出 JSON；forest / amoled mobile 一旦进入矩阵，按同一主题色参数扩展，不另写一套阈值。
4. **归属**：`P-DOCK-*` 失败归 dock 长效机制；`P-NO-SCROLLBAR` 失败归 viewport policy；`P-DIVIDER` 失败归共享行组件；`P-PROFILES/PROXIES-DENSITY` 与 `P-EMPTY-STATE` 失败归 demo fixture / 底层投影，不得让 UI 私自加占位；`P-TYPE-BASELINE` 失败先查契约字体角色，不得在表面调假粗。
5. **证据等级**：guard 通过与 197 帧真实像素属于本地 `L`；本审计没有跑 `cargo`、结构门、CI 或真机，不写 `C` / `D`，更不写「完整 / 已验证」。

## 4. 截图时效管理与四者对账

### 4.1 当前事实

- b127 发布目录 `20261107T080044Z_9107f001fa16_dirty_2012886`：`captured_at=2026-11-07T08:02:25Z`（本地 16:02），`capture-manifest.tsv` / 发布 `manifest.tsv` 同为 16:06:17，PNG 发布 mtime 16:06:17.52–16:06:17.69，`scenario_count=197`，`source_fingerprint=a6a6324028002882b655a36edea7694674b0367d5e231b7a5ec0e3ec98e4f49f`，`binary_sha256=f19e3629309814c868e85a8ecdbe722bc8f1da64f6449c45dcdaeaf15773e55f`。
- 16:20–16:23 有 `design_resources.rs`、`design-authority.json`、`design-type.slint`、`layout-tokens.slint`、`mascot-assets.slint`、`home_compact_flow.slint`、`home_desktop_grid.slint`、`theme.slint` WIP；其后的并行流还继续改 dock typed state / body / guard。活源码指纹在审计期间连续变化（16:35 与 17:09 两次计算彼此不同，也均不等于 b127 的 `a6a6…49f`），所以本文不冻结任何「当前指纹」hex；每次对账都以 `visual_receipts.py --fingerprint` 当时的输出为准。**b127 所有帧已失效，只能作为历史对比，不得作为当前源码验收。**
- 用户 16:28 附件 `/tmp/codex-clipboard-6597baf4-a86f-437d-9540-b82068f3875b.png` 第一次 stat 为 933 × 229、非全帧裁图；复核时文件已不存在。无论是否存在，它都晚于 16:20 WIP、无 run manifest、无 source fingerprint、无 400 × 800 全帧身份，**不得复用为任何判据**；按用户纪律应即时删除 / 不引用，也不得翻旧账。
- 参考四图是长期外部输入，不受「本仓回执过期删除」约束；过期规则只作用于 `.evidence/corpora/slint/*.png`、发布 manifest 和用户临时裁图。

### 4.2 四项对账方法

主收口方在**所有并行流交付源码后**，用以下顺序做一次对账；不要以「截图 mtime 比源码晚」单独判新鲜，也不要以「某个 PNG 看起来没变」跳过：

| 对账项 | 命令 / 来源 | 合格条件 | 失败动作 |
| --- | --- | --- | --- |
| ① `source_fingerprint` | `python3 scripts/parity/visual_receipts.py --fingerprint` 的最新输出 vs 最新全量 run 的 `capture-metadata.txt` | 两个 64 位 hex 完全相等；且这一步在所有源码写入完成后执行 | 有差异即整批 stale，等 WIP 收口后重采，不修改 metadata 造同值 |
| ② `binary_sha256` | `sha256sum` 归档 `product-build.json` 指向的 binary vs `capture-metadata.txt` / `visual_receipts.py` 的 run identity | 三者一致，binary 的 `file_identity` 在验证期间不变 | 二进制被替换 / 覆盖即重采；禁止只换 PNG |
| ③ manifest 时间 / 身份 | 运行 `capture-manifest.tsv` 与发布 `manifest.tsv` 的 SHA-256、行数、`status=ok` 行集合、每行 PNG 哈希；`capture-metadata.txt` 的 `captured_at` 与 run 目录时间 | 发布 manifest 与最新全量 run 完全相同；`scenario_count=197`；所有 `ok` PNG 存在且哈希一致；无额外 PNG；`captured_at` 不早于最后一项影响二进制的源码写入 | 任一不符则拒绝结论；由发布链整批重发并自动 sweep 旧 PNG；诊断过滤批次不得发布 |
| ④ `.evidence/corpora/slint` mtime / 用户附件 mtime | `stat` 发布目录 PNG / manifest；`stat` `/tmp/codex-clipboard-*.png` 或用户附件；与最后一次源码写入时间比较 | 发布 PNG mtime ≥ manifest 发布时刻且属于同一 run；用户附件只要不是 400 × 800 全帧、没有 run 身份，或 mtime 早于 / 晚于但无法绑定指纹，一律删除 / 不引用 | 删除过期裁图；把唯一有效批次路径写入台账；禁止把附件贴进发布目录 |

建议主收口方按顺序执行（**这些命令本审计未执行**）：

```bash
set -euo pipefail
current=$(python3 scripts/parity/visual_receipts.py --fingerprint)
run=$(python3 - <<'PY'
from pathlib import Path
import csv
root=Path('.evidence/captures/slint')
runs=[]
for metadata in root.glob('*/capture-metadata.txt'):
    facts=dict(line.split('=',1) for line in metadata.read_text().splitlines() if '=' in line)
    if facts.get('scenario_count')=='197': runs.append((metadata.parent/'capture-manifest.tsv').stat().st_mtime_ns,metadata.parent)
print(max(runs)[1])
PY
)
grep -E '^(captured_at|source_fingerprint|binary_sha256|scenario_count)=' "$run/capture-metadata.txt"
printf 'live source_fingerprint=%s\n' "$current"
sha256sum "$run/capture-manifest.tsv" .evidence/corpora/slint/manifest.tsv
stat -c '%n %y %s' "$run/capture-metadata.txt" "$run/capture-manifest.tsv" .evidence/corpora/slint/manifest.tsv .evidence/corpora/slint/*-mobile-*.png
stat -c '%n %y %s' /tmp/codex-clipboard-*.png 2>/dev/null || true
python3 scripts/quality/receipt-freshness-guard.py --mode enforce
```

指纹一致后才运行唯一一次完整采集；不要带 `INFILTRATOR_CAPTURE_SCENARIOS` 或矩阵 override，因为过滤运行只做诊断、不发布：

> **采集顺序硬规则（b152 实测教训）**：`source_fingerprint` 把 `crates/` 与 `scripts/`
> 的**全部受版本控制文件**（含判定器与各守卫）一起哈希，所以一旦某次改动落在 `crates/`
> 或 `scripts/`，本批像素回执就不再是「当前源码的像素证据」。因此：任何代码/守卫改动必须
> **在整批采集之前全部落地并跑完快门**，采集只在最后跑一次；采集结束后只允许改 `docs/`、
> `todos/`、`TODO.md`（这些不在指纹里）。唯一例外是发布本身必须随帧更新的
> `scripts/parity/screenshot_aliases.tsv` 摘要——它由发布后的像素重写，属于回执的一部分，
> 不是新的源码改动；改完必须复跑 `screenshot-identity-guard --mode enforce` 归零。
> 中途被打断（`pkill`、超时、合成器故障）的运行**不得当作发布**：它没有发布 manifest，
> 但其 `capture-metadata.txt` 里的 `scenario_count` 仍是矩阵全数，会让
> `published_run()` / `latest_run()` 把残缺批次误认成最新整批；必须先删除该运行目录再重跑。

```bash
bash scripts/capture-slint-matrix.sh
python3 scripts/quality/receipt-freshness-guard.py --mode enforce
# resolve_surface_evidence.py / visual_receipts.py 按台账既定参数传入 evidence、
# receipts、product-builds 与各 surface run；不要以无参数调用代替完整验证。
python3 scripts/quality/mobile-pixel-invariants.py --mode enforce   # 规格中的新 guard，落地前不可声称存在
bash scripts/quality/check-structure.sh
```

`resolve_surface_evidence.py` 的必选链路参数（manifest / evidence / receipts / product-builds / discovery / catalogue）与 `visual_receipts.py` 的 `--iced-run` / `--bevy-run` / `--slint-run` 必须按主收口方现有台账入口传；上述清单只表达顺序和「先身份、后像素、再结构门」的依赖，不把裸命令写成可执行验收。`mobile-pixel-invariants.py` 在本审计时**尚不存在**，不能写成已跑过或已通过。

### 4.3 长效删除 / 引用纪律

- 每次全量发布后，由 `capture-slint-matrix.sh` 先发布全部 PNG、再发布 manifest、最后 sweep 未被本批发布的 PNG；任何用户裁图 / 诊断帧 / 旧批次像素不得手工复制进 `.evidence/corpora/slint`。
- 任何文档、台账、回复引用移动端像素时，必须同时给出最新全量 run 的 `source_fingerprint` 与 manifest 时间；只写「见截图」或「上一批曾通过」无效。
- 发现活源码指纹变化：立即把旧发布帧标为历史对比，停止引用；等并行流全部落地后统一重采，禁止逐页重试或只替换几张 PNG。
- `docs/reference/flclash-mobile/` 四张外部参考图保留；`/tmp/codex-clipboard-*` 等非全帧附件随用随删，不纳入发布目录，不进入证据台账。

## 5. 给主收口方的交接结论

- **本切片只写本文与 `/tmp/b128_audit/`；`docs/reference/FLCLASH_MOBILE_REFERENCE.md` 未修改，归主收口方独占。** 若要把本文回灌为公共导航，只需在 L3 同级文档或所属包入口加一条向下 / 同级链接；不要从本文回引 L1 / L2 文档路径，也不要把本文声明成其他层级。
- b127 像素已对齐的是：四页无可见滚动条、dock 胶囊外侧 / 空气带 / 下沿全为页面底色、胶囊采用 contract alpha 玻璃 veil（非遮盖实心块）、滚动到底无双重空白带、状态卡青蛙与按钮不出圈、工具页分隔线比例、配置 / 代理卡几何和选中色。
- b127 的真实差距是：配置页只有 2 卡并形成 516u（64.5%H）空带；代理页形成 374u（46.8%H）空带；运行时长已由 owner 发布真实会话时长（回执 `03:04:49`），相对更新时间仍是底层事实缺位；暗色代理 / 配置页头动作可读性偏弱；参考标题折算 22u 与本仓 19–20u 墨迹带仍有层级差；emoji 与超 600 字重必须拒绝照抄。
- 当前 16:06 帧因源码 WIP 已失效；主收口方统一重采前，任何「已实现 / 已验证」结论都只能写「历史 b127 帧显示」或「部分」，不能写完整。dock 长效机制由另一并行流实现，本审计不碰其文件，也不替它宣告通过。
- **b181 更正（本文只写「可读性偏弱」，未定位根因）**：那条差距的根因不是页头几何或字色 token，而是 vendored `@material` 组件的明暗通道分叉——
  `material_palette.slint` 的 `scheme` 与 `elevation.slint` 的 `dark` 直接读**平台** `Palette.color_scheme`，
  而全站其余部分跟随 `LumenTheme.dark-mode`（用户主题偏好，仅 `system` 时回落平台）。
  平台为浅色、皮肤为 dark 的帧里，`IconButton` 等就画成浅色方案：`proxies-dark` 页头动作墨迹实测
  rgb(65,73,75) 落在 rgb(10,11,14) 地面上 ≈ **2.14:1**，而本表面暗色 `onSurfaceVariant` 应为 `#A5B0BC` ≈ 7.4:1。
  修复是把明暗结论交回唯一 owner（新增 `MaterialPalette.application-dark-mode`，由根窗口与 `schemes` 同时写入），
  同批实测 `proxies-dark` 页头动作对比度 **2.14:1 → 8.94:1**，且 `proxies-light` 帧逐字节不变——
  只有「应用明暗 ≠ 平台明暗」的帧（dark / amoled）受影响，故这是纠错而非整体调暗。
  本文其余结论按 b127 帧解释，仍以其自身时效规则为准。
- 建议把第 3 节 `S-*` / `P-*` ID 作为台账条目的稳定锚点：静态 guard 与像素 guard 分别验收，统一验证批次只跑一次；新增 forest / amoled mobile 或 light end 帧时扩展适用集合，而不是复制第二套阈值。
