# mihomo-rs 2.2 对标与补齐排期

对标对象：[mihomo-rs](https://github.com/DINGDANGMAOUP/mihomo-rs) `v2.2.0`（commit `cdb1489`）。
审计结论（2026-08-31）：v2.1.0 → v2.2.0 功能增量为零（仅版本号 + Homebrew 发布脚本），实质能力在 2.1.0 已定型；
其形态为 SDK + CLI（约 8.6k 行），本仓库为双端 GUI 管理器（约 68.5k 行）。
四项落后点已全部补齐，本文档记录差距判定、排期与落地状态。

## 一、差距判定（他们有、我们当时没有）

| # | 能力 | mihomo-rs 形态 | 补齐状态 |
| --- | --- | --- | --- |
| 1 | doctor 自检 + 保守自动修复门面 | `doctor run/fix/list/explain`，8 项检查、4 项修复、退出码 0/1/2、`--json` | ✅ 已落地（第 1 波 A + 第 2 波 A） |
| 2 | 零配置服务引导 | `ensure_default_config` / `ensure_external_controller` 开箱即用 | ✅ 已落地（`bootstrap.rs`，底座 mihomo-config 原已具备） |
| 3 | CLI / 脚本化管理入口 | 命名空间化 clap 命令树 + crates.io/Homebrew 发行 | ✅ 命令树已落地（`infiltrator-cli`）；crates.io/Homebrew 发行不在范围 |
| 4 | 配置目录直通云同步 | `configs-dir set/unset` + `MIHOMO_CONFIGS_DIR` 指向 iCloud/Dropbox 目录 | ✅ 已落地（`INFILTRATOR_CONFIGS_DIR` > settings `configs_dir` > 默认） |

他们领先但经评估**不采纳**的：配置目录重定向之外的云方案（我们已有更强的 WebDAV 同步链）、
crates.io/Homebrew 发行（产品形态不同）。内核下载无校验和是他们的安全短板（我们 fail-closed 校验链保持领先）。

## 二、排期与落地记录

### 第 1 波（并行）

| 任务 | 交付物 | 验收 |
| --- | --- | --- |
| A. doctor 门面 | `infiltrator-core/src/doctor/{mod,checks,fixes,pidfile}.rs` + `bootstrap.rs`；9 项检查（id 与上游对齐）、4 项保守修复、`DoctorEnv` 路径注入、`exit_code`、explain 元数据、`run_with/fix_with` 过滤器 | `nextest -p infiltrator-core` 260 绿；clippy 零警告；守卫 0 违规 |
| B. configs_dir 直通 | `AppSettings.configs_dir`（serde 向后兼容）；`mihomo-config` `resolve_configs_dir_in`、`ConfigManager::with_home_configs_dir_and_store`；`~` 展开、相对路径按显式 home 拼接 | `nextest -p mihomo-config -p infiltrator-core`；env 测试持对应测试锁串行 |

### 第 2 波（并行）

| 任务 | 交付物 | 验收 |
| --- | --- | --- |
| A. Admin API 暴露 | `GET /admin/api/doctor?only=`（含 `exit_code`）、`GET /admin/api/doctor/checks`、`GET /admin/api/doctor/checks/{id}`（404 语义）、`POST /admin/api/doctor/fix`（SSE `doctor-fix` 事件）、`POST /admin/api/bootstrap`；`AppSettingsPayload.configs_dir` 补齐 | `nextest -p infiltrator-admin` 62 绿 |
| B. CLI 命令树 | `crates/infiltrator-cli`（bin `infiltrator`）：`doctor / bootstrap / kernel / profile(含 configs-dir get/set/unset) / service / proxy / connection / sync`，`--json` 覆盖机器可读输出，doctor 退出码 0/1/2，`service start` 前自动 bootstrap | `nextest -p infiltrator-cli` 49 绿；二进制 `--help`/`doctor list --json` 实测 |

### 第 3 波（收尾）

| 任务 | 状态 |
| --- | --- |
| `profile_options::apply_saved_options_for` 跟随 configs_dir 重定向（原硬编码 `<home>/configs`）+ 回归测试 | ✅ |
| 全仓 `scripts/test.sh`、`cargo clippy --workspace --all-targets -- -D warnings`、line/import 守卫 | ✅ |
| 本文档落盘 | ✅ |

## 三、补强波次（第 2 日，六路并行）

对标交付后复查确认的三个确定性缺口，已全部补齐：

1. **configs_dir settings 字段路径全端生效**（此前仅 env 路径与 CLI/Admin 设置读写生效）：
   - core：`settings::app_config_manager[_in]` 规范工厂，接入 profiles/rules/proxy_providers/sniffer/tun/dns/fake_ip 全部构造点；其中 schema/YAML 变换归 `infiltrator-domain`，profile 文件读写由对应 `*_io` adapter 承担；doctor 检查与修复、bootstrap 跟随重定向（`DoctorEnv::config_manager` 改 async）；mihomo-config 新增 `with_home_configs_dir[_and_store]`、`config_dir()`。
   - admin：私有 `support.rs` 助手，订阅调度（含 JoinSet/启动播种）、profiles handlers 8 处、WebDAV 同步 local_root 跟随重定向；顺手修复 delete handler 清错 sidecar 目录的问题。
   - 桌面：iced 22 处构造 + 8 处手拼路径经 `configs_dir.rs` 入口；desktop runtime bootstrap 设置感知；src-tauri 5+ 处。
   - Android：`support.rs::build_config_manager` 唯一工厂，8 处构造 + 1 处手拼路径归零；webdav sync 根跟随重定向。
2. **doctor/bootstrap 全端暴露**：admin 端点 + CLI 已有；本轮新增 iced 设置页"体检与修复"卡片（体检/修复/引导，徽章列表 + 自动刷新）与 Android uniffi（`doctor_run/doctor_fix/doctor_checks/doctor_explain/bootstrap_now` + 8 个 record）；Web Admin 前端新增"诊断与修复"分区（DoctorPanel + composable，中英 i18n）。
3. **dav-sync/cli 占位 crate 删除**：确认零依赖后 `git rm`；members 清理；顺带移除 dav-sync 子树永不被读取的遗留 Cargo.lock。
4. 质量基建：doc-link-guard 排除 `node_modules`（npm install 生成的第三方 JSDoc markdown 不应门禁）。

验收（排除外部并行工作线 `infiltrator-bevy-widgets`/`infiltrator-bevy-ui` 后）：全仓 **1015/1015** 测试通过、`clippy --workspace --all-targets -D warnings` 零警告、line/import/test-layout/doc-link 四守卫 0 违规；Android `aarch64-linux-android` check 通过；webui vitest 109 用例通过 + vite build 通过。

## 四、后续跟进（非本轮范围）

1. ~~GUI 与 Android 端 ConfigManager 显式传入 configs_dir~~ ✅ 补强波次完成。
2. ~~Android uniffi 暴露 doctor~~ ✅ 补强波次完成（Kotlin UI 接线由 app 侧跟进）。
3. ~~dav-sync/cli 占位 crate 去留~~ ✅ 已删除。
4. `infiltrator-cli` 的 crates.io/Homebrew 单独发行：产品形态决策，暂缓。
5. 工作区另有并行工作线（`infiltrator-bevy-widgets`/`infiltrator-bevy-ui`，当时编译未完成），其门禁由该线自行闭环。

## 五、对齐后的能力对照速查

| 领域 | mihomo-rs 2.2 | 本仓库（当前） |
| --- | --- | --- |
| doctor | 8 检查/4 修复/退出码/`--json` | 9 检查/4 修复/退出码/`--json`，另有 DNS 健康、出口 IP、抖动/丢包/审计等深度诊断 |
| 引导 | ensure_default_config + controller 推导 | `bootstrap` 一键引导 + doctor fix + admin 端点 + CLI 命令 |
| CLI | version/config/service/proxy/connection/doctor | kernel/profile/service/proxy/connection/doctor/bootstrap/sync |
| 云同步 | configs 目录指向网盘文件夹 | 同款直通 + 自研 WebDAV 全链路（ETag 乐观锁/冲突/墓碑） |
| 订阅 | 无 | URL 校验/解析/转换/定时更新（上游为零） |
| 内核升级 | 下载无校验 | SHA256 + release 摘要 fail-closed + 能力集推导（上游为零） |
