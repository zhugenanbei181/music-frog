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
    * **模块化**: 业务逻辑下沉至 `crates/despicable-infiltrator-core`，Tauri 层仅负责 UI/OS 交互与编排。
    * **松耦合**: 模块间避免循环依赖，通过 Trait 或 Context 注入依赖。

4. **变更管理 (Git)**
    * **Commit Message**: 使用 Conventional Commits (如 `feat: add feature`, `fix: resolve bug`)。
    * **Documentation**: 功能变更必须同步更新 `USAGE_SPEC.md` 和 `README.md`。

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

- [ ] 无 `unwrap()`/`expect()` (测试代码除外)
- [ ] 无 `unsafe` 块 (FFI 封装除外)
- [ ] 菜单 ID 符合命名规范
- [ ] 国际化 key 完整 (zh-CN + en-US)
- [ ] 错误处理有上下文信息
- [ ] `cargo check --workspace` 无警告
