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
