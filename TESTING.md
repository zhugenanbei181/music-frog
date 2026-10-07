> 双端平权验收规则见 [docs/UI_PARITY_AUDIT.md](docs/UI_PARITY_AUDIT.md)。79 个字符串型特性 guard 已从执行入口退役；结构检查统一由 `scripts/quality/check-structure.sh` 注册，动态行为证据由 `scripts/parity/` 解析。

# 测试与质量保障指南 (Testing & Quality Assurance)

本文档记录了本项目的测试策略、实战经验以及保持卓越工程质量的最佳实践。

## 验证口径

- 测试数量与真实 ID 以 `cargo nextest list --workspace --build-jobs 4 --message-format json` 为准，不用源码正则统计覆盖率。历史 2026-09-08 的 2,371 项检查点不能代表当前工作树。
- 完整 Rust 验证由 `bash scripts/test.sh` 执行，全工作区、4 个构建 job、4 个测试进程；不得靠删测试、跳过失败或替换断言获得通过。
- 全工作区 `cargo check --workspace --all-targets -j 4`、`cargo fmt --all -- --check` 和 `cargo clippy --workspace --all-targets -j 4 -- -D warnings` 必须无错误与警告。
- 结构规范由 [代码质量底线](docs/CODE_QUALITY_BASELINE.md) 负责：生产、测试与示例统一 800 行，明确导入、唯一类型定义、禁止规避。
- 导入和模块重整必须保留原测试，更新真实测试 ID 绑定并执行测试；仅发现测试不等于测试通过。
- L1/L2/L3 平权验收见 [UI 审计](docs/UI_PARITY_AUDIT.md) 与 `scripts/parity/`；真实 mihomo、移动真机、发行包、系统权限及网络副作用分别按 [平台矩阵](docs/PLATFORM_MATRIX.md) 与 [测试矩阵](docs/TEST_MATRIX.md) 验收，不能由无头测试代替。

---

## 测试安全策略

为保证测试在任意环境（含 CI 沙箱）下都能可重复运行，本仓库强制执行以下三条红线：

1. **测试绝不启动真实的 mihomo 二进制**：只有生产代码允许调用 `spawn_daemon`；
   测试一律通过 loopback HTTP mock（mockito）模拟 mihomo 的 REST/WebSocket 接口，
   或通过 trait seam（如 `mihomo_api::MihomoApi`）注入纯内存实现。
2. **测试绝不发起外网请求**：所有 HTTP mocking 仅绑定 `127.0.0.1` 回环地址；
   任何看似“需要真实网络”的行为都应退化为 loopback 验证或纯函数验证。
3. **CI 强制隔离执行**：`.github/workflows/test.yml` 先在有网环境中
   `bash scripts/test.sh --no-run` 预编译（此时 mock server 尚未启动），
   再通过 `sudo unshare -n bash -c 'ip link set lo up && bash scripts/test.sh'`
   在仅保留 loopback 接口的网络命名空间中运行全部测试；任何意外的外网请求都会
   直接失败，而不是侥幸通过。

4. **测试运行器强制统一**：仓库内完整 Rust 测试一律通过
   `bash scripts/test.sh` 运行。该入口固定使用 `cargo nextest`、工作空间全量测试、
   4 个构建 job 和 4 个测试并发槽位；CI 的 `check-test-policy.sh` 会拒绝重新引入
   原始 Cargo 测试命令。

---

## GUI 渲染测试（niri 后台捕获，本地运行）

iced 桌面端的测试分三层，全部零 mihomo 启动、零外联：

| 层 | 位置 | 运行方式 | 证明什么 |
| --- | --- | --- | --- |
| L1 无头单元/逻辑 | 各 crate `#[cfg(test)]` | `bash scripts/test.sh` | 业务逻辑、状态机、mock API |
| L2 GUI 逻辑 | `crates/infiltrator-iced/tests/{common,headless,gui}` | 已并入全量运行 | AppState update/view 管线、demo fixture 契约 |
| L3 静态页面渲染 | `scripts/capture-{iced,bevy}.sh` | 本地页面矩阵 | 页面外观；不能证明专用交互场景 |
| L3 场景验收 | `scripts/parity/accept-surface-interactions.sh` | 本地与正式发布 CI；发布使用 `--all` | 原生交互激活、区域可见性、标准/紧凑视口真实像素与完整证据绑定 |

L3 流程（参照 taskmanager 成熟实践，**全程后台、对操作者桌面零干扰**）：

1. `--demo` 模式启动真实二进制：跳过一切生产副作用（不 spawn mihomo、不改系统代理、
   不托盘、不写设置），使用 `src/demo.rs` 的 mock mihomo fixture 数据
   （7 代理组/9 节点/延迟四级/流量波形/40 日志/10 连接/15 规则/3 配置）。
2. 后台合成器栈：`kwin_wayland --virtual`（私有 XDG_RUNTIME_DIR）作不可见宿主，
   niri 嵌套其中软件渲染（`LIBGL_ALWAYS_SOFTWARE=1`），与操作者桌面完全隔离；
   被测窗口按 PID 绑定，就绪判定用标记文件（首帧 `CAPTURE_READY`），零固定 sleep。
3. 产出 `docs/screenshots/iced/*.png` 与 `manifest.tsv`（尺寸/字节/sha256 收据）；
   失败语义区分 `BLOCKED (compositor)` 与场景 `FAIL`，部分成功仍保留证据。

```bash
# 静态页面矩阵；场景与数量由 capture_iced_scenarios.tsv 定义
bash scripts/capture-iced.sh
# 单场景
INFILTRATOR_CAPTURE_SCENARIOS=proxies-dark bash scripts/capture-iced.sh
# iced 测试布局守卫（tests/{common,headless,gui} 约定）
python3 scripts/quality/test-layout-guard.py
# 全仓 Rust 行数红线（生产/测试/示例统一非注释 ≤800 行/文件）
python3 scripts/quality/line-guard.py --mode report
# 同权双端：动态解析真实 nextest 测试锚点；未闭环项明确为 pending。
python3 scripts/parity/resolve_surface_evidence.py --report-json target/parity/evidence.json
# 单场景 L1/L2/L3 验收（私有后台 Wayland、真实像素、双视口）。
bash scripts/parity/accept-surface-interactions.sh connections-close-all-confirm
# 全量闭环验收；任何缺口阻断，不能用普通测试通过代替。
bash scripts/parity/accept-surface-interactions.sh --all
```

---

## Android CI 与测试分层（BANDROID-018～023）

Android 原先没有独立的测试 CI；`.github/workflows/android.yml` 是新增的 L0/L1 编译门（PR/push 触发），`scripts/android-check.sh` 是它的 Rust 侧命令入口。分层与本仓库其它平台一致：

| 层 | 命令 | 说明 |
| --- | --- | --- |
| L0/L1 编译门 | `bash scripts/android-check.sh` | 两个 ABI 的 cargo check/clippy；不替代设备 |
| L1.5 清单/权限 | `python3 scripts/quality/android-manifest-guard.py --mode enforce` | 产品清单权限/VPN service/`foregroundServiceType`/receiver |
| L1.5 APK 权限 | `BEVY_APK_USE_SYSTEM_TOOLCHAIN=1 BEVY_APK_SKIP_EMULATOR=1 bash scripts/build-bevy-apk.sh` | 构建后经 aapt 断言实际打包权限 |
| L2 Kotlin 单元 | `( cd android && ./gradlew --no-daemon :app:testDebugUnitTest )` | JVM 逻辑（BANDROID-021） |
| L2.5 模拟器插桩 | 编译 `:app:assembleDebugAndroidTest`；模拟器跑 `:app:connectedDebugAndroidTest` | 已安装 APK 权限/service 声明、Activity/service/Insets/IME（BANDROID-022） |
| L3/L4 真机 | `workflow_dispatch`/self-hosted 运行 `scripts/android-device-evidence.sh` | VPN/后台/功耗原始报告（BANDROID-023） |

工具链版本两端统一为 min 29 / target 36 / compile 36 / NDK `29.0.14206865`（BANDROID-020）。`cargo-apk` 只能声明 `uses_permission`，完整 VPN 清单由 Gradle 宿主交付（BANDROID-003/015）。普通 PR 的编译门与守卫通过不构成设备证据。

---

## 全仓代码行数红线（line budget）

导入、拆分与反规避规范见 [docs/CODE_QUALITY_BASELINE.md](docs/CODE_QUALITY_BASELINE.md)。

每个 `.rs` 文件（生产、测试、示例）的**非注释、非空行**不得超过 800 行。注释与空行不计入——
"把代码挪进注释"属于违规而非整改。超限说明该文件承载了不止一个业务语义，
必须按业务域拆分子模块（参照 2026-08-30：`iced update/core.rs` → `update/core/*`）。

- **扫描范围**：`crates/` 下所有 Rust 文件（构建产物除外），包含 `tests/`、路径挂载测试和 examples；没有测试豁免。
- **机械检查**：`python3 scripts/quality/line-guard.py --mode enforce`，CI 强制执行；
  本地先用 `--mode report` 看违规清单。
- **拆分规范**：按业务语义切子模块（如 lifecycle / proxy / dns / tun），
  同一类型的 `impl` 块允许分布在多个文件；跨模块可见性用 `pub(crate)`/`pub(super)`
  表达；**禁止**为拆分新增跨 crate `pub use` 转发层（见 ARCHITECTURE.md 导入规范）。

---

## Bevy UI `bsn!` 场景法守卫（BEVY-004）

Shader 与 GPU handle 开发契约见 [Bevy 前端章程 §1.2](docs/BEVY_UI_FRONTEND.md)。共享结构入口同时运行 `python3 scripts/quality/bevy_shader_guard.py --mode enforce`；其规则测试归 `scripts/quality/tests/test_bevy_shader_guard.py`。资产复用/隔离/缺失恢复与 100 次回退、挂载/销毁基线回归归 widgets 的 `render_cache_tests` / `render_lifecycle_tests`，由正常 nextest 执行。WESL 链接仍须实际 GPU 捕获；CPU 资产表回收不能替代 Android 显存及长时资源报告。

两个 bevy crate（`infiltrator-bevy-widgets`、`infiltrator-bevy-ui`）的生产代码
执行 100% `bsn!` 场景法：UI 树只能在 `bsn! { … }` 场景内声明并经 `spawn_scene`
挂载；ECS 观察者使用受限参数原位更新组件。开发边界以
[BEVY_UI_FRONTEND.md](docs/BEVY_UI_FRONTEND.md) 为准。

- **扫描范围**：两个 bevy crate 的 `src/` 生产代码；`tests/` 目录与 `*_test(s).rs`
  挂载测试模块不在红线内。注释与字符串/字符字面量剥离后再扫。
- **违规项**：`bsn!` 之外出现结构原语（`Node {` / `Children [` / `Text(…)`）及
  遗留 UI bundle（`NodeBundle` 等）；`with_children`/`push_children`/
  `add_child(ren)`/`insert_children`/`replace_children`/`add_related`/
  `insert_related`（含 turbofish）手工接线与 `ChildSpawner(Commands)`/
  `RelatedSpawner(Commands)` 类型声明；任何 `.spawn(` / `.spawn_batch(` /
  `.spawn_empty(_at)` 直建实体树；结构原语的 `::default()`/`::EMPTY`/`::new`/
  `::from_*` 临时值构造（BEVY-BSN-005）；`insert(ChildOf(…))` 脱离
  `spawn_scene` 同语句链（BEVY-BSN-006）；`bsn!` 花括号不平衡。
- **运行态访问检查**：`BEVY-ECS-007/008` 检查整 `World` 类型入口与
  `App::world()` / `world_mut()` 旁路；业务、页面、控件与截图激活均受检查。
  正反例覆盖以引号结束的注释，防止词法屏蔽吞掉后续运行代码。
- **豁免**：`spawn_scene`（唯一挂载缝）、`spawn(Camera2d)` 与
  `spawn(Observer::new(…))`（相机/观察者基础设施，非 UI 树）——按首个实参文本
  前缀机械判定，无逐文件白名单；`accesskit::Node::new`（无障碍树）与
  `despawn_children`/`remove_child(ren)`（拆除路线）。
- **机械检查**：`python3 scripts/quality/bevy_bsn_guard.py --mode enforce`，
  CI 强制执行；本地先用 `--mode report` 看违规清单，`--self-test` 内嵌正反用例
  并复扫真实生产树。

---

## 卓越工程实践经验

在达成“卓越水平”测试覆盖的过程中，我们总结了以下核心经验：

### 1. 彻底的测试隔离 (Test Isolation)
- **挑战**：多个测试并行修改全局静态变量（如 `HOME_DIR_OVERRIDE`）或读写同一个配置文件，导致随机失败。
- **经验**：
    - 每个测试必须使用 `tempfile` 创建独立的临时目录。
    - 对于涉及全局状态的测试，必须在 Crate 级别引入 `TEST_LOCK`（互斥锁）。
    - 运行全量测试时使用 `bash scripts/test.sh`，固定 4 个 nextest 测试并发槽位。

### 2. 原子化操作验证 (Atomic Operations)
- **挑战**：文件下载、数据库更新等长耗时操作中途失败会导致“脏数据”残留（如空的版本目录或 `.sync-tmp` 临时文件）。
- **经验**：
    - **先写临时文件，成功后再重命名 (Rename)**：这是保证文件系统原子性的金科玉律。
    - **异常清理机制**：在测试中必须模拟 IO 失败、权限不足等异常，验证代码是否能正确触发 `rollback` 清理残留。
    - **发现 Bug**：通过该项测试，我们成功修复了同步引擎在下载失败时残留临时文件的漏洞。

### 3. 工具函数的健壮性挖掘 (Robust Utils)
- **挑战**：类似 `extract_port_from_url` 的工具函数看似简单，实则隐藏大量边界情况（如无协议头、带 Query、前缀冒号）。
- **经验**：
    - 不要信任外部库的默认解析逻辑（如 `url` crate 认为 `https:443` 端口是 `None`）。
    - 必须为工具函数提供全场景 Mock 输入测试（≥10 个用例），确保能应对各种非标的真实用户输入。

### 4. 跨语言边界的“饱和攻击” (FFI Safety)
- **挑战**：Rust 与 Java/JNI 通信时，错误码映射不一致会导致 UI 收到“Unknown Error”，难以调试。
- **经验**：
    - **穷举验证**：为 FFI 错误码编写穷举测试，确保每一个 Rust `Enum` 变体都能对应到唯一的 Ffi 代码。
    - **Mock 桥接器**：通过泛型或 Trait 注入 Mock Bridge，模拟底层（Android 系统层）抛出的各种极端异常。

### 5. 零警告原则 (Zero Warnings)
- **经验**：警告通常是代码质量下滑的开始。
    - 严禁提交包含 `unused_imports`、`dead_code` 或类型不匹配警告的代码。
    - 保持代码洁净能显著降低维护者的心智负担，使真正的逻辑错误更容易浮出水面。

---

## 核心功能测试矩阵

| 模块 | 测试重点 | 经验总结 |
| :--- | :--- | :--- |
| **src-tauri** | 状态流转、UI 映射、多语言 | 验证托盘 ID 与配置项的 1:1 精准映射是 UI 稳定的关键。 |
| **admin-api** | 路由匹配、权限保护、调度锁 | 模拟 API 报错 404/400 路径，验证后端的容错能力。 |
| **sync-engine** | 哈希碰撞、冲突算法、磁盘原子性 | 哈希校验（MD5/ETag）必须在每一步操作后进行重验。 |
| **core-logic** | 订阅解码、配置深度合并 | Gzip/Base64 解码必须具备多层 Fallback 机制。 |
| **version-mgr** | 多平台适配、安装回滚 | 预先在安装路径“占坑”是验证回滚逻辑最有效的方法。 |

---

## 如何运行测试

```bash
# 首次使用先安装固定版本的 nextest
cargo install cargo-nextest --locked --version 0.9.138

# 推荐的开发环境运行方式（强制 nextest，固定 4 并发）
bash scripts/test.sh

# 检查代码质量（必须无输出）
cargo check --workspace
```
