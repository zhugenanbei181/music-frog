# MusicFrog Infiltrator — Agent 总纲与项目地图

> 本文件是本仓**项目地图的最高入口**：自本文件起，逐层细化到 `docs/` 地图文档与各 owner 文档。
> 人类向的产品介绍在 [README.md](README.md)。

## 1. 文档职责边界（唯一口径）

- `README.md`（根与各 crate）**面向人类**，用平实话说明“这是什么、有什么特色、怎么用、当前状态与限制”。
- README **不是项目地图、全仓导航、索引、施工契约或红线清单**；它不参与地图层级。
- 项目地图以本文件为最高入口，逐层细化到 [docs/README.md](docs/README.md) 与其登记的各 canonical 文档。
- 一个事实只有一个权威来源，其它文档只链接不复制；宿主提示文件（[.claude/CLAUDE.md](.claude/CLAUDE.md)）只保留宿主差异与读取顺序。

## 2. 身份

MusicFrog Despicable Infiltrator（音乐青蛙 · 卑鄙的渗透者）是一个以 [mihomo](https://github.com/MetaCubeX/mihomo) 为数据平面与代理内核的跨平台客户端。Rust 全栈，双 UI 同权产品（Iced + Bevy UI），另有 Android companion。

- UI 只提交意图、读取不可变结果；**Rust 是所有 mihomo 控制操作的唯一产品边界**（内核生命周期、配置、版本、REST/WebSocket 控制与业务编排）。
- 产品定位、发行形态与用户可见能力见 [README.md](README.md)；用户功能说明见 [USAGE_SPEC.md](USAGE_SPEC.md)。
- 本项目由 AI 助手协作开发与维护：核心逻辑、系统集成与功能规划，前端 UI/UX 设计与组件重构，以及代码补全、例行重构与文档维护分别由不同模型承担；所有改动以代码、行为测试和适用的平台/打包证据为准。

## 3. 阅读顺序（任务先查 docs/README.md）

1. 本文件。
2. [docs/README.md](docs/README.md)：文档索引与权威关系表；接到任务先查它的路由表与权威关系。
3. [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md)：分层边界与数据流。
4. [docs/FUNCTIONAL_MAP.md](docs/FUNCTIONAL_MAP.md)：按功能域定位唯一 owner。
5. 受影响 crate 的 `README.md`（crate 自述）及其源码与测试。

- 双端同步演进的最高主控台账：[docs/DUAL_SURFACE_PARITY_MASTER_PLAN.md](docs/DUAL_SURFACE_PARITY_MASTER_PLAN.md)。
- 差距视图（证据索引，非执行顺序）：[DEFECTS.md](DEFECTS.md)；测试命令与策略：[TESTING.md](TESTING.md)、[docs/TEST_GOVERNANCE.md](docs/TEST_GOVERNANCE.md)。

## 4. 架构骨架

六边形 / Clean Architecture，细节与数据流见 [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md)：

`domain`（纯领域模型，零框架依赖） → `contract`（跨端共享类型与 viewport/capability 契约） → `ports`（入站/出站端口 trait） → `application`（use-case 编排，不直接依赖 Tokio） → `composition`（组合根，注入具体 adapter） → host adapters（desktop / admin / android / ios）、UI surfaces（iced / bevy-ui / bevy-widgets）、mihomo 层（api / config / platform / version / dav-sync）。

## 5. 工程不变量

- **构建**：先 `bash scripts/fetch-mihomo.sh` 拉取内核二进制，再 `cargo build --workspace`；工具链由 `rust-toolchain.toml` 钉死（1.99.0 + rustfmt + clippy）。
- **测试**：`bash scripts/test.sh`（先跑 `scripts/quality/` 守卫，再跑 `cargo nextest`）；Bevy UI 专项 `bash scripts/test-bevy.sh`。全部使用 mock/headless，不依赖外网或真实 mihomo 进程。测试质量遵循零废话断言契约。
- **代码规范**：严格遵守 [docs/CODE_QUALITY_BASELINE.md](docs/CODE_QUALITY_BASELINE.md)。生产、测试、示例统一 ≤800 非注释、非空代码行；禁止内联长路径代替明确 `use`，禁止命名导入别名、re-export 与纯类型转发；按业务拆分，不压行、不提高阈值、不加白名单规避。新增 UI 文案走共享本地化；Application 异步实现由 composition 注入。
- 上述守卫与质量门的具体命令、阈值与例外以 `scripts/quality/**` 与 [TESTING.md](TESTING.md) 为准。

## 6. 工作协议

- 新增功能：先更新 [docs/FUNCTIONAL_MAP.md](docs/FUNCTIONAL_MAP.md) 确认 owner，再落 `contract → application → host adapter → UI`。
- 双端新特性：Iced 与 Bevy UI 是独立发行、同权的产品；遵循 [docs/UI_PARITY_AUDIT.md](docs/UI_PARITY_AUDIT.md) 的同异律与 L1/L2/L3 验收。模态、抽屉、向导与二次确认不可降级为 Toast 或静默执行；共享层禁止按前端身份编译；平台 typed 不支持与前端缺口分轴记录。
- Bevy UI 遵循 [docs/BEVY_UI_FRONTEND.md](docs/BEVY_UI_FRONTEND.md)：业务、页面、控件交互与截图激活系统禁止接收或透传整个 `World`；使用受限 `Query`、资源、`Commands`、Observer 与职责明确的 `SystemParam`。实体生命周期用调度和延迟提交边界解决，不以独占 World 或消音错误补洞。
- Bevy 页面插件一次注册观察者与资源，页面根不使用生命周期 hook 懒注册业务或重置草稿；控件初始化使用受限 `On<Insert<T>>` Observer。生产源码禁止 `DeferredWorld` 与 `App::world()` / `world_mut()`，字体和图标通过受限 `PreStartup` 系统初始化，具体装配边界以 Bevy UI 章程为准。
- Bevy 自有 shader 的 WESL 模块、GPU ABI、handle owner/复用/回收与验证统一遵循 [Bevy UI 章程 §1.2](docs/BEVY_UI_FRONTEND.md)。相关改动同时检验静置无上传、共享资产隔离、角色退役及有界回收；结构守卫、资产表回收和实际 GPU/Android 设备证据分别负责自身事实。
- Bevy Android 的保留扩展与缺口补全遵循 [docs/BEVY_ANDROID_PRODUCT.md](docs/BEVY_ANDROID_PRODUCT.md)：复用现有宿主、控件和共享业务，推进 UI/VPN 进程与 IPC、原生输入、业务实体回收、事件驱动节能及真实设备验收；当前平台状态与执行队列分别回指平台矩阵和本地 TODO。
- Android 目标在 PR/push 上经 `.github/workflows/android.yml`（编译门）与 `scripts/quality/android-manifest-guard.py`（清单/权限守卫）验证；Compose 与 Bevy 工具链版本统一，真机/长时证据只在显式 stage 取得（BANDROID-018～023）。
- 所有改动以代码、行为测试和适用的平台/打包证据为准；不支持的明确标 `unsupported`，未验证的不写“已验证”。
