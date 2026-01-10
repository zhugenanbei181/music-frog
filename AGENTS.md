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

3. **前端规范 (Frontend)**
    * **SFC 优先**: Vue 组件必须使用 `.vue` 单文件组件与 `<script setup>`。
    * **ES6+**: TypeScript 必须使用 ES6 及以上语法，禁止旧式 `var`/构造式。
    * **类型与 API**: 前端类型集中在 `src/types.ts`，接口封装集中在 `src/api.ts`。
    * **国际化**: 新增 UI 文案必须同步 `en-US.json` 与 `zh-CN.json`。
    * **表单工具**: 表单文本解析/规整统一使用 `src/composables/useFormUtils.ts`。
    * **模块化**: 业务逻辑优先下沉到 `src/composables/`，保持 `App.vue` 轻量。
    * **UI 一致性**: 面板统一使用 `PanelCard` + `PanelHeader`/`PanelFooter`/`PanelTitle`；辅助/空态文字使用 `help-text`/`empty-text`；列表内操作按钮优先 `btn-xs`，面板底部操作使用 `btn-sm`。
    * **现代化样式**: 严禁使用原始丑陋的组件样式，必须遵循下方 UI 样式规范。

4. **架构设计 (Architecture)**
    * **模块化**: 业务逻辑下沉至 `crates/infiltrator-core`，Desktop 集成位于 `crates/infiltrator-desktop`。
    * **松耦合**: 模块间避免循环依赖，通过 Trait 或 Context 注入依赖。

5. **变更管理 (Git)**
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

## 文档撰写规范 (Documentation Standards)

所有项目文档均由 AI 代理根据核心指令自主生成与维护。

### README.md

1. **语言**: 统一使用英文 (English)。
2. **结构**:
    * **Name**: 项目名称。
    * **Tech Stack & Libraries**: 列出核心使用的后端库 (Tauri, Axum, SQLx, Reqwest, Tokio, Serde 等) 与前端库 (Vue 3, Vite, Tailwind CSS 等)。
    * **AI Codex**: 明确列出参与开发的 AI 助手（如 Gemini 2.0/2.5, Claude 3.5 Sonnet），并说明项目完全由 AI 完成。
    * **Documentation Links**: 引用 `USAGE_SPEC.md`。

### USAGE_SPEC.md (使用指南)

1. **语言**: 中英双语对照 (Bilingual: Chinese & English)。
2. **内容定位**: 面向普通用户，仅提供**已有功能**的使用说明。
3. **描述规范**: 必须结合前端页面的**按钮名称**、**面板标题**以及**UI 交互逻辑**进行描述。
4. **禁止内容**: 严禁在 `USAGE_SPEC.md` 中包含纯开发阶段的 API 文档或底层代码逻辑描述。

## 命名规范 (Naming Conventions)

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

1. **功能域前缀**: 必须包含功能模块名 (`profile-`, `webdav-`, `core-`, `mode-`, `tun-`, `dns-`, `fake-ip-`, `rules-`, `config-`)
2. **使用短横线**: 统一使用 `-` 连接单词，禁止混用 `_`
3. **动作明确**: 使用动词描述操作 (`sync`, `update`, `switch`, `delete`)
4. **避免缩写歧义**: 优先使用完整单词

## 托盘菜单 ID 约定

以下为托盘菜单关键项的已定义 ID（用于排查/维护，避免出现“看不懂是哪一项”的情况）：

* `about`：关于子菜单标题
* `about-app`：应用版本
* `about-sdk`：SDK/工作区标识
* `about-core`：核心版本
* `quit`：退出

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
* [ ] 前端组件符合 UI 样式规范（无原始 checkbox/toggle，使用 switch）

## UI 样式规范

前端必须使用统一的现代化样式，禁止使用原始丑陋的组件。

### 容器与面板

| 组件 | 样式类 | 说明 |
|------|--------|------|
| 面板卡片 | `.panel` | 圆角 `rounded-2xl`、阴影 `shadow-panel`、毛玻璃 `backdrop-blur`、半透明背景 `bg-white/85` |
| 面板边框 | `border-ink-500/10` | 统一使用低对比度边框 |

### 表单控件

| 控件 | 样式类 | 说明 |
|------|--------|------|
| 开关 | `.switch` | ✅ 必须使用滑块开关，禁止使用原始 checkbox |
| 输入框 | `.input` | 圆角 `rounded-xl`、聚焦高亮 |
| 文本域 | `.textarea` | 同 input 风格 |
| 下拉框 | `.select` | 同 input 风格 |

### 按钮

| 场景 | 样式类 | 说明 |
|------|--------|------|
| 主操作 | `.btn-primary` | 强调色背景 |
| 次要操作 | `.btn-secondary` | 边框样式 |
| 危险操作 | `.btn-danger` | 红色边框 |
| 列表内操作 | `.btn-xs` | 紧凑尺寸 |
| 面板底部操作 | `.btn-sm` | 标准尺寸 |

### 文字

| 场景 | 样式类 | 说明 |
|------|--------|------|
| 面板标题 | `.panel-title` | `text-lg font-semibold` |
| 表单标签 | `.label` | `text-sm font-semibold` |
| 辅助说明 | `.help-text` | `text-xs text-ink-500` |
| 空态提示 | `.empty-text` | `text-sm text-ink-500` |

### 禁止使用

```html
<!-- ❌ 禁止：原始 checkbox -->
<input type="checkbox" class="checkbox checkbox-primary" />

<!-- ❌ 禁止：DaisyUI toggle -->
<input type="checkbox" class="toggle toggle-primary" />

<!-- ✅ 正确：使用 switch -->
<input type="checkbox" class="switch" />

<!-- ❌ 禁止：DaisyUI card -->
<div class="card bg-base-100 shadow-xl">

<!-- ✅ 正确：使用 panel -->
<div class="panel">
```

## 依赖 API 变更记录

记录依赖库升级时的 Breaking Changes，避免重复踩坑。

### Axum 0.7 → 0.8

| 变更项 | 旧语法 | 新语法 | 说明 |
|--------|--------|--------|------|
| 路由参数 | `/users/:id` | `/users/{id}` | 路径参数必须使用花括号 |
| 通配符 | `/files/*path` | `/files/{*path}` | 通配符也需花括号包裹 |

**报错特征**: `Path segments must not start with ':'. For capture groups, use '{capture}'.`

### Tauri v1 → v2

| 变更项 | v1 | v2 | 说明 |
|--------|-----|-----|------|
| 插件前缀 | `tauri-plugin-xxx` | `tauri-plugin-xxx` (API 变更) | 需检查各插件迁移指南 |
| 窗口 API | `tauri::Window` | `tauri::WebviewWindow` | 窗口类型重命名 |
| 事件系统 | `app.emit_all()` | `app.emit()` | 事件广播 API 简化 |

### 新增变更模板

```markdown
### [依赖名] [旧版本] → [新版本]

| 变更项 | 旧语法 | 新语法 | 说明 |
|--------|--------|--------|------|
| xxx | `old` | `new` | 描述 |

**报错特征**: `错误信息关键词`
```
