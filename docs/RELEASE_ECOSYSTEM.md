# 发布生态门禁 (Release Ecosystem Gates)

> 层级：L2

本文登记 crates.io 发布生态的四组门禁：语义化版本兼容守卫、多架构交叉构建守卫、无头沙盒混沌套件与开发者全旅程演练。四组门禁都在 `scripts/quality/check-structure.sh` 里汇总，结构门本身不编译、不联网、不启动任何内核。

## 1. SemVer 兼容守卫（PUB-006.10）

已发布的中性 crate 的公开 API 是承诺：能对 `lurker-core-contract 0.68.0` 编译的消费者必须能对下一个版本继续编译。守卫用两层实现同一件事。

| 层 | 实现 | 语义 |
| --- | --- | --- |
| 源码层（离线，始终可用） | `scripts/quality/semver-compat-guard.py --mode check` | 基线 `scripts/quality/semver-baseline.json` 记录七个中性 crate 的公开 API；移除或改签名即破坏性变更并失败，新增项记为漂移 |
| 工具层（可选） | `scripts/quality/semver-compat-guard.py --mode tool` | 装有 `cargo-semver-checks` 时按基线记录的 git 修订做类型检查分析 |

- **基线**：`scripts/quality/semver-baseline.json` 由 `--mode capture` 生成，结构门检查它覆盖全部七个 crate、版本与本次发布一致、并记录一个 40 位 git 修订。
- **严格形态**：`--mode verify` 要求基线与当前 API 完全一致，发布前用它兜住“未复审的 API”。
- **负例**：`--mode negative` 复制一个中性 crate、删掉一个公开函数，要求守卫必须拒绝这次破坏。守不住破坏的守卫不是守卫，所以负例进结构门。
- **工具层缺席**不是静默通过：守卫会明确报告工具未安装，源码层继续权威。

## 2. 多架构交叉构建守卫（PUB-006.10）

四个桌面目标三元组与两个 Android 三元组分属三个入口，`scripts/quality/cross-build-guard.py` 证明它们一个都不能丢：

| 入口 | 覆盖 |
| --- | --- |
| `.github/workflows/release.yml` | `x86_64-unknown-linux-gnu`、`aarch64-unknown-linux-gnu`、`x86_64-pc-windows-msvc`、`aarch64-pc-windows-msvc` |
| `scripts/cross-check.sh` | 两条 aarch64 桌面腿（zig + Windows 交叉工具链） |
| `scripts/android-check.sh` | `aarch64-linux-android`、`x86_64-linux-android` |

Apple 目标仍处于延后状态，守卫会在任何入口发现 Apple 三元组时报错；解禁与否由 Apple 延后守卫裁决。

## 3. 无头沙盒混沌套件（PUB-006.7）

`sandbox/` 是中性运行时的集成面：它用注入确定性故障的适配器驱动真实的 `LurkerRuntime`，并读回 typed 答案。

- **四类故障**：网络抖动、内核猝死、写栅栏争用、配置损坏。
- **确定性**：不掷随机数，不以挂钟测量决定结论；故障次数是固定预算。
- **fail-closed**：注入故障绝不产生运行时尚未观察到的成功；运行时发布精确失败阶段，并拒绝从失败态再次运行。
- **租约与资源回收**：owner 租约在精确终态后释放、写栅栏在持有者离开后可再次获取、启动过的进程都会被再次停止。
- **自愈**：宿主读到 typed 失败后可重建运行时与进程并重新到达运行态。

入口：

```bash
python3 scripts/quality/sandbox-chaos-guard.py --mode structure   # 离线审计
python3 scripts/quality/sandbox-chaos-guard.py --mode run          # cargo nextest run -p lurker-sandbox
```

## 4. 开发者全旅程演练（PUB-006.11）

`scripts/developer-journey-rehearsal.py` 在纯净克隆里重放外部开发者的旅程：克隆 → 读文档 → 脚手架生成 → 运行 → 接驱动 → 调配置，全链路预算五分钟。

- 报告模板：`scripts/developer-journey-audit-template.md`；演练把每一步的耗时、判决与证据填进模板，写到 `.evidence/journey/`。
- `--dry-run` 不克隆、不编译、不运行，但仍走完整步骤序并产出同一份报告，结构门用它离线验证演练逻辑。
- `scripts/quality/journey-rehearsal-guard.py` 保证脚本与模板描述的是同一条旅程：步骤数、顺序、五分钟预算与全部占位符。

## 5. Release 签名契约（fail closed）

Release 产物的身份只有一处来源：用户持有的持久 keystore。debug key 永远不用于 release 产物，缺 secret 时发布流程**明确失败**，不会退化成未签名或 debug 签名的产物。

需要在仓库 secrets 里配置四个值：

| secret | 含义 |
| --- | --- |
| `RELEASE_KEYSTORE_BASE64` | 持久 release keystore 的 base64 内容 |
| `RELEASE_KEYSTORE_PASSWORD` | keystore 的 store 密码 |
| `RELEASE_KEY_ALIAS` | keystore 内的 key alias |
| `RELEASE_KEY_PASSWORD` | 该 key 的密码 |

发布作业的行为：

1. 先校验四个 secret 是否齐备，缺任意一个立即以明确信息失败——**在昂贵的三 flavor release 构建之前**。
2. 把 base64 secret 物化成仓库之外（`$RUNNER_TEMP`）的私有目录里的 keystore 文件，权限 0600。
3. 把 keystore 路径与三个凭据交给 Gradle；`android/app/build.gradle.kts` 的 release 变体用专用签名配置读取它们。
4. 作业结束时（无论成功失败）删除该 keystore 文件。

Gradle 侧同样是 fail closed：release 产物的构建任务若拿不到完整材料，会在**配置期**失败并列出缺了哪一项（`RELEASE_KEYSTORE_PATH` / 密码 / alias / key 密码），不会产出未签名 APK。Debug 变体不受影响，本地 lint / ktlint / Detekt 门照常在 Debug 变体上运行。

本机演练（不需要真实 secret，只校验接线契约）：

```bash
bash scripts/blocked/release-keystore-evidence.sh --dry-run
```

拿到真实 keystore 后，同一条命令会读出证书指纹与 APK 签名者的证书摘要并比对两者：

```bash
RELEASE_KEYSTORE_BASE64=... RELEASE_KEYSTORE_PASSWORD=... RELEASE_KEY_ALIAS=... RELEASE_KEY_PASSWORD=... \
  bash scripts/blocked/release-keystore-evidence.sh --apk-a dist/app-compose-universal-release.apk
```

## 验证入口

```bash
bash scripts/quality/check-structure.sh          # 四组门禁的离线形态
cargo nextest run --locked -p lurker-sandbox     # 混沌矩阵
python3 scripts/quality/examples-guard.py --mode compile   # 用例展厅编译门
python3 scripts/quality/semver-compat-guard.py --mode tool  # SemVer 工具层
bash scripts/blocked/release-keystore-evidence.sh --dry-run # release 签名契约
```
