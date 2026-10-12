# MusicFrog Agent 总纲

> 层级：L0

## 阅读顺序

先读 [文档地图](docs/README.md)，再读受影响包的 `README.md`、其链接文档与源码。产品说明见 [README](README.md)，使用说明见 [contract 文档](crates/infiltrator-contract/docs/README.md)。

## 强制编码规则（所有 Agent）

多阶段流程一律用显式 `enum`/sealed 状态表达互斥状态，并以 typed transition 和穷尽匹配限定合法迁移。**禁止用多个 `bool`、`Option`/null 或标志位组合藏出隐式状态机；`bool` 只表示真实二值事实。**修改后必须通过 typed-state 质量门禁；细则见 [代码质量底线](docs/CODE_QUALITY_BASELINE.md)。

Core-first 顺序同样是硬门：先完成所有底层 Core TODO（domain/contract/ports、application 与 fake/nextest、Mihomo/sing-box adapter 与 conformance、Android owner/IPC/VPNService），再做 UI 适配。该 Core 阶段全部关闭前，不实现或重编 UI 包；每个 Core 切片用 mock/nextest 验收并提交，阶段统一验证后再进入 UI。

## 工作规则

- **分支管理：** 本仓库只在 `main` 开发、提交和推送。开始操作先确认当前分支是 `main`；禁止创建或切换到其它分支、禁止另建 worktree。清理分支前先核对独有提交和未保存改动，任何已有工作都必须先安全归并到 `main`，不得用 reset 或删除分支丢弃。
- **本地任务台账（gitignored，CI 无这些文件属正常）：** 任务真源是 `TODO.md`（三佛导航、points 台账）与 `todos/` 看板：`CODE_TODO.md` 现在佛、`CODE_TODO_PAST.md` 过去佛、`CODE_TODO_FUTURE.md` 未来佛、`MACHINE_TODO.md` 实机证据。开发会话先按 `TODO.md`「三佛工作流程」的 Agent 执行循环行事：定阶段 → 现在佛按 P0→P1、InProgress→Ready→Planned 选原子条目 → 实现完成同次迁入过去佛（`Integrated`）→ 验证收口通过后办结退出并在台账登记 archived points；三佛之间如何流转按该节流转总表执行，不得在旧描述里另记第二份状态。
- 文档引用遵守单向层级：L0（本文件）→ L1（根 README、TESTING、第三方声明与文档地图）→ L2（跨 crate/surface 公共契约，含共享 UI contract）→ L3（具体 crate、Android、资源和工具实现细则）→ L4（本地 TODO 叶台账）。只允许同级或向下引用，包含 Markdown 链接及正文中的文档路径；实现细则和叶台账不得回引上位契约，公共入口负责向下导航。门禁按文件范围分配层级，校验头部声明与范围一致，并拒绝未登记 Markdown，不能靠改层级声明绕过反向引用检查。
- 新功能先查 [owner 表](docs/FUNCTIONAL_MAP.md)，按 [架构边界](docs/ARCHITECTURE.md) 修改；每项事实只留一个权威来源。
- 每个新功能切片都先盘点会被新语义取代的旧入口和旧描述；先迁完全部生产消费者，再在同一切片删除旧公开接口、命令、port、DTO、FFI/wire、兼容分支、重复状态表达与过期产品文案。以仓库搜索确认受影响范围内旧符号和旧说法已清零，并以新契约行为测试取代旧路径测试。不得因为旧入口仍能编译、暂时有调用点或“以后再清理”而保留新旧双轨、fallback、重复描述或仅换名入口。只有读取历史持久化格式时可保留旧格式，并且必须由显式版本迁移器隔离。Core 后端的具体约束见 [Core 后端契约](docs/CORE_BACKENDS.md)。
- UI 与平台规则分别见 [共享 UI 契约](crates/infiltrator-contract/docs/README.md)、[Bevy 文档](crates/infiltrator-bevy-ui/docs/README.md) 和 [Android 文档](android/docs/README.md)。**Bevy UI 新代码必须先读并遵守 [Bevy 0.20 UI 能力规范](crates/infiltrator-bevy-ui/docs/BEVY_020_UI_CAPABILITIES.md)**：rem/em 尺寸随 `RemSize` 缩放、`ThemeContext` 两段式主题、官方 headless 控件优先（TabList/Dialog/Popover/Menu）、`FixedNode` 悬浮层、`InlineBox` 行内内容、`bsn_list!`/`Ready`/`despawn_all`。
- 不支持标为 `unsupported`；没有对应证据不写“已验证”。代码规则与验证入口见 [质量底线](docs/CODE_QUALITY_BASELINE.md) 和 [TESTING.md](TESTING.md)。

- 修改 Iced/Bevy 通用组件前，先读 [跨产品共享包契约](docs/SHARED_UI_PACKAGES.md)：
  优先扩展共享 owner，保留必要差异，检查其他消费者和版本兼容性。
