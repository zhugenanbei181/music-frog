# 上游依赖与版本演进

上游分成四条线分别跟踪：mihomo core、Rust/Cargo、Web、Android。它们的升级风险不同，不能只依赖一次 `cargo update`。

## 1. 版本真相来源

| 上游线 | 当前真相来源 | 当前观察 | 升级影响 |
| --- | --- | --- | --- |
| mihomo core | `packaging/mihomo-assets.json`；执行 `scripts/fetch-mihomo.sh` | 版本、平台资产、archive/binary SHA-256 均由锁定清单维护；未知版本或目标 fail-closed | 资产名、API、配置字段、行为、许可证、ABI、回滚 |
| Rust toolchain | `rust-toolchain.toml` | 1.99.0 + rustfmt/clippy，声明 Android target | edition、MSRV/编译器、native crate 和 CI |
| Rust libraries | 根 `Cargo.toml` + `Cargo.lock` + `src-tauri/Cargo.lock` | workspace 统一 Tokio/Reqwest/Serde 等，UI/宿主有额外 toolkit 依赖 | resolver、feature、TLS、系统库、跨平台编译 |
| Bevy UI engine | 根 `Cargo.toml`（`bevy = "=0.20.0-rc.2"`）+ `Cargo.lock` | 战略统一 surface，锁 **0.20 发布线**（上游当前为预发布构建 rc.2，GA 后只改 pin 不改代码）；与 taskmanager 同锁 | BSN 语法、feature 闭包、wgpu/naga、文本/无障碍栈、Android APK、桌面打包 |
| Web Admin | `webui/config-manager-ui/package.json` + lockfile | Vue/Vite/Tailwind/Vitest 独立构建 | API schema、浏览器行为、bundle |
| external dashboard | `webui/mihomo-manager-ui/dist` + `THIRD-PARTY-NOTICES.md` | 上游静态构建产物 | controller API、资产来源、字体和许可 |
| Android | `android/app/build.gradle.kts` + Gradle wrapper | Compose BOM、AndroidX、NDK/ABI 构建脚本 | min/target SDK、JNI/UniFFI、VPN、ABI |

文档只引用这些来源，不另外手抄版本号。变更版本时先改 manifest/lock/script，再更新说明。

## 2. mihomo 升级流程

每个候选 core release 都要经过：

1. 读取官方 release notes，确认 Windows/Android/未来 Linux/macOS 资产和架构；
2. 对比 controller endpoint、返回字段、WebSocket、代理模式和配置 schema；
3. 更新版本 manifest 和所有 archive/binary digest；
4. 运行解压、权限、安装目录、原子替换和回滚验证；
5. 使用 mock API 跑 Rust 行为矩阵，再以该版本执行一次真实 core smoke；
6. 检查 profile、DNS/Fake-IP/TUN、rules/providers/sniffer、connections 和 delay；
7. 检查 Android 两种 ABI、Windows 包资源及其他目标平台的“明确不支持”状态；
8. 更新 `THIRD-PARTY-NOTICES.md`、发布说明和 `TODO.md` 中的升级记录。

禁止把 `MIHOMO_VERSION` 临时改成任意字符串就当作可交付升级。非 pinned 版本必须有对应 digest manifest，或者明确标记为本地实验，不得进入 release。

## 3. Rust/Cargo 升级流程

- 一次只升级一个依赖族，优先使用 workspace dependency，避免各 crate 重复声明不同 feature；
- 先更新 `Cargo.toml`，再用受限的 lockfile 更新验证解析变化；
- 检查 `cargo tree -d`、native system library、TLS backend、Android NDK 和 Windows/macOS 条件分支；
- 依次运行 workspace check、clippy、Rust tests、Web tests、Android compile 和适用的 package smoke；
- toolkit 大版本升级必须单独评估窗口/渲染/输入/托盘，不和 mihomo core 升级混在同一功能任务里；
- 升级失败时保留旧 lockfile/版本路径，记录回退原因，不删除旧的兼容 fixture。

## 4. 当前需要补强的供应链问题

| ID | 风险 | 处理方向 |
| --- | --- | --- |
| UP-001 | 版本→资产→digest 已由 `packaging/mihomo-assets.json` 锁定；下载与提取均校验 | 版本升级必须更新可信 digest；未知版本与目标拒绝执行 |
| UP-002 | mihomo API/配置能力没有按 core version 固化 fixture | 建立 endpoint、字段、错误和版本能力矩阵 |
| UP-003 | Rust、Tauri、Iced、Web、Android 的升级验证入口分散 | 统一 dependency update checklist 和 CI stage |
| UP-004 | external dashboard dist 的上游版本/来源不在单独 manifest | 记录 commit/release、构建方式、字体和 license provenance |
| UP-005 | Linux/macOS core 资产与发布策略未完整表达 | 在平台矩阵中明确 bundled/download/system-core policy |

## 5. 更新后的完成条件

“依赖已升级”只表示 manifest 改了，不表示任务完成。完成必须同时有：版本来源、lockfile、兼容结论、测试结果、平台影响、许可证检查和回滚方案；mihomo 还必须有 digest 和真实 core smoke。
