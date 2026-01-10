# AI 代理规范指南

本文档定义了开发本项目的核心规范。所有 AI 代理必须严格遵守。

## 核心原则

1. **安全优先 (Safety First)**
    * **No Unsafe**: 严禁在业务逻辑或平台封装中使用 `unsafe` 代码。
    * **No Panic**: 严禁在非测试代码中使用 `unwrap()`、`expect()` 或直接索引访问。必须使用 `?`、`ok_or_else` 或 `get()` 并显式处理错误。
    * **Thread Safety**: 全局状态修改必须使用 `Mutex` 或 `RwLock` 保护。涉及运行时重启、重置等长耗时操作，必须使用 `rebuild_lock` 防止竞态条件。

2. **代码风格 (Style)**
    * **Rust**: 遵循标准格式 (4空格缩进)。模块名 `snake_case`，类型名 `PascalCase`。
    * **Conciseness**: 代码力求简洁高效，避免冗余。
    * **No Warnings**: 提交前必须确保 `cargo check --workspace` 无警告。

3. **架构设计 (Architecture)**
    * **模块化**: 业务逻辑下沉至 `crates/infiltrator-core`，Desktop 集成位于 `crates/infiltrator-desktop`。
    * **松耦合**: 模块间避免循环依赖，通过 Trait 或 Context 注入依赖。

4. **变更管理 (Git)**
    * **Tauri 变更**: 仅在新 crates 完整后再改动 Tauri 导入；改动导入时必须通过 Tauri 构建与测试，未改动导入不强制编译。
    * **迁移完整性**: 新 crates 必须覆盖原 crates 的功能与行为，且 Tauri 应用功能完整可用后才允许移除兼容层。
    * **构建位置**: 仅在仓库根目录执行构建/测试命令，避免锁定子目录。
    * **Commit Message**: 使用 Conventional Commits (如 `feat: add feature`, `fix: resolve bug`)。
    * **Documentation**: 功能变更必须同步更新 `USAGE_SPEC.md` 和 `README.md`。
    * **Markdown 规范化**: 文档格式必须通过 `pnpm dlx markdownlint-cli2 "**/*.md" --fix` 修复，配置统一使用 `.markdownlint-cli2.jsonc`、`.markdownlint.json`、`.markdownlintignore`。

## 规划流程

### 人话

把差距整改分为两阶段：Phase A 做多端共用能力（DNS/Fake-IP/规则集/TUN 高级配置），Phase B 做 Android 专项（分应用代理/VPN Service/Core 运行模式）。每一项都明确“归属 crate + 最小里程碑”，并同步到 `TODO.md` 与 `ANDROID.md`。

### AI 提示词工程

你是规划执行代理。输出必须包含：功能拆分→归属 crate→最小里程碑→文档落点（`TODO.md`/`ANDROID.md`/`CHANGELOG.md`）。优先多端能力，再做 Android 专项；每一步更新文档状态并保持 Tauri 行为不变。

## 命名规范

### 菜单 ID 命名

托盘菜单和 UI 事件 ID 必须遵循以下规范，避免歧义：

```
格式: <功能域>-<动作>[-<子对象>]

示例:
  ✅ profile-switch-xxx     配置切换
  ✅ profile-update-all     更新所有订阅
  ✅ webdav-sync-now        WebDAV 立即同步
  ✅ webdav-sync-settings   WebDAV 同步设置
  ✅ core-update            内核更新
  ✅ core-use-v1.2.3        启用指定版本内核
  
  ❌ sync_now               歧义：是订阅同步还是 WebDAV 同步？
  ❌ sync-now               歧义：缺少功能域前缀
  ❌ update                 歧义：更新什么？
```

**规则**:

1. **功能域前缀**: 必须包含功能模块名 (`profile-`, `webdav-`, `core-`, `mode-`, `tun-`)
2. **使用短横线**: 统一使用 `-` 连接单词，禁止混用 `_`
3. **动作明确**: 使用动词描述操作 (`sync`, `update`, `switch`, `delete`)
4. **避免缩写歧义**: 优先使用完整单词

## 错误处理范式

```rust
// ❌ 禁止
let val = option.unwrap();
let item = vec[0];
mutex.lock().unwrap();

// ✅ 推荐
let val = option.ok_or_else(|| anyhow!("value missing"))?;
let item = vec.get(0).cloned().unwrap_or_default();
let guard = mutex.lock().unwrap_or_else(|p| p.into_inner());
```

## 线程安全范式

```rust
// 涉及运行时重启的操作必须加锁
pub async fn critical_section(state: &AppState) -> anyhow::Result<()> {
    let _guard = state.rebuild_lock.lock().await;
    // ... 执行操作 ...
    Ok(())
}
```

## 代码审查清单

提交前必须检查：

* [ ] 无 `unwrap()`/`expect()` (测试代码除外)
* [ ] 无 `unsafe` 块 (FFI 封装除外)
* [ ] 菜单 ID 符合命名规范
* [ ] 国际化 key 完整 (zh-CN + en-US)
* [ ] 错误处理有上下文信息
* [ ] `cargo check --workspace` 无警告
* [ ] `pnpm dlx markdownlint-cli2 "**/*.md"` 无报错
