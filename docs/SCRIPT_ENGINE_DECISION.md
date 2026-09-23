# 脚本引擎决策记录 / Script Engine Decision Record

- 记录项：`DUAL-10-01`（QuickJS 嵌入式轻量执行沙箱）
- 状态：**当前不添加任何 JS 引擎依赖**；仓库只提供「指令 DSL + 可插拔引擎接缝」
  （接缝与能力协商记为 `shared-ready`，JS 引擎执行仍记为 `planned`）。
- 结论摘要：`rquickjs` 与 `boa_engine` 都可作为未来候选，但两者都未通过本仓库的
  「依赖准入 + 全目标静态编译 + 沙箱 ABI/熔断落点 + 迁移兼容」前置条件；在满足下方
  **触发条件**前，不把它们加入 `Cargo.toml`。本文记录的是**可复现的证据**，不是主观倾向。

> 本文没有添加任何依赖。所有命令均在 `parity/engine-seam` 工作树内执行；`cargo info`
> 只读取 crates.io 索引与元数据，不写入本仓库的 `Cargo.lock`/`Cargo.toml`。

---

## 1. 现状证据：仓库内没有 JS 引擎

### 1.1 锁文件与清单

```console
$ grep -n -iE 'rquickjs|rquickjs-core|rquickjs-sys|boa_engine|boa_ast|boa_gc|quickjs|quick-js|quickjs-rs|deno_core|rusty_v8|v8' Cargo.lock
（无输出）

$ grep -rn -iE 'rquickjs|boa_engine|quickjs' Cargo.toml crates/*/Cargo.toml
（无输出，退出码 1）
```

### 1.2 vendor 目录只有 mihomo 二进制

```console
$ ls vendor
mihomo-android-amd64
mihomo-android-arm64-v8
mihomo.exe
```

### 1.3 唯一相关命中 `js-sys` 是 wasm-bindgen 绑定，不是 JS 引擎

```console
$ grep -n -A4 'name = "js-sys"' Cargo.lock
5451:name = "js-sys"
5452-version = "0.3.104"
5453-source = "registry+https://github.com/rust-lang/crates.io-index"

$ grep -n -A4 'name = "wasm-bindgen"' Cargo.lock
10209:name = "wasm-bindgen"
10210-version = "0.2.127"
```

`js-sys` 是 wasm 目标下调用浏览器 JS 的绑定层，不能在原生桌面/移动二进制里解释脚本，
因此不构成「嵌入式 JS 引擎」。

### 1.4 运行时行为也证明它不是 JS 解释器

`crates/infiltrator-domain/src/script_engine_runtime.rs` 用字符串匹配拒绝 `while(true)`
并只识别固定指令；`crates/infiltrator-contract/src/script_sandbox.rs` 的
`ScriptEngineKind::DirectiveDsl` / `ScriptEngineCapabilities::supports_javascript_syntax = false`
是读模型里对该事实的机检断言（`engine_kind_never_claims_javascript`）。

---

## 2. 候选元数据（crates.io）

### 2.1 工具链（本仓库）

```console
$ rustc --version
rustc 1.98.1 (48a229cea 2026-09-01)

$ cat rust-toolchain.toml
[toolchain]
channel = "stable"
profile = "minimal"
components = ["rustfmt", "clippy"]
targets = [
    "aarch64-linux-android",
]

$ grep -rn 'no_std' crates/*/src/lib.rs
（无输出：全工作区是 std）
```

即：MSRV 需 ≤ 1.98.1；实际参与交叉编译的是 Android（`aarch64-linux-android`、
`x86_64-linux-android`，见 `.github/workflows/release.yml:211` 与
`.cargo/config.toml` 的 `aarch64-linux-android21-clang`）。当前工具链与 CI **没有 iOS 目标**。

### 2.2 两个主候选 + 被误称的 `quickjs-rs`

```console
$ cargo info rquickjs
rquickjs #quickjs #ecmascript #javascript #es6 #es2020
High level bindings to the QuickJS JavaScript engine
version: 0.14.0
license: MIT
rust-version: 1.87
repository: https://github.com/DelSkayn/rquickjs.git
features:
 +default             = [std]
  std                 = [rquickjs-core/std]
  39 deactivated features

$ cargo info boa_engine
boa_engine #javascript #js #compiler #lexer #parser
Boa is a Javascript lexer, parser and compiler written in Rust. Currently, it has support for some of the language.
version: 0.22.0
license: Unlicense OR MIT
rust-version: 1.91.0
repository: https://github.com/boa-dev/boa
features:
 +default          = [float16, xsum, temporal]
  ...

$ cargo info quickjs-rs
error: could not find `quickjs-rs` in registry `https://github.com/rust-lang/crates.io-index`

$ cargo info quick-js
QuickJS Javascript engine wrapper
version: 0.4.1
license: MIT
rust-version: unknown
repository: https://github.com/theduke/quickjs-rs
```

注意：任务里的 “`quickjs-rs`” 并不是 crates.io 上的 crate（真实 crate 是 `quick-js`，
底层 `libquickjs-sys`）。`quick-js` 最后发布于 **2021-03-15**，5 年未更新，故不作为候选。

### 2.3 crates.io 版本元数据（大小 / 发布 / 运行时）

```console
$ curl -s -A 'music-frog-decision-record' https://crates.io/api/v1/crates/rquickjs/0.14.0
num: 0.14.0
license: MIT
rust_version: 1.87
crate_size: 37226          # 门面 crate 源码 37KB（不含 rquickjs-sys 内的 QuickJS C 源码）
created_at: 2026-09-18T19:36:45Z

$ curl -s -A 'music-frog-decision-record' https://crates.io/api/v1/crates/boa_engine/0.22.0
num: 0.22.0
license: Unlicense OR MIT
rust_version: 1.91.0
crate_size: 944056         # 944KB 纯 Rust 源码
created_at: 2026-08-28T17:17:57Z

$ curl -s -A 'music-frog-decision-record' https://crates.io/api/v1/crates/quick-js/0.4.1
num: 0.4.1
license: MIT
rust_version: None
crate_size: 26509
created_at: 2021-03-15T14:10:57Z
```

### 2.4 依赖形态（决定交叉编译与体积）

```console
$ curl -s .../crates/rquickjs-sys/0.14.0/dependencies
cc ^1 required kind=build
bindgen ^0.72 optional kind=build
pretty_env_logger ^0.5 optional kind=build

$ curl -s .../crates/boa_engine/0.22.0/dependencies
total 77    # 含 dev；required kind=normal 的主要为 boa_*、regress、icu_normalizer、dashmap、hashbrown 等
```

- `rquickjs-sys` 的构建期依赖里有 **`cc`（必需）**：它把 QuickJS 的 C 源码按目标平台
  编译进静态库；`bindgen` 是可选项（默认关闭，使用预生成绑定）。`cc 1.4.4` 已在
  `Cargo.lock`（`grep -n 'name = "cc"' Cargo.lock` → 2302）中，说明构建工具本身可用，
  但仍需要目标平台的 C 编译器（Android NDK clang 已在 `.cargo/config.toml` 固定）。
- `boa_engine` 是纯 Rust，无 FFI/C 工具链需求；代价是 **77 个依赖**与 944KB 源码体量。

---

## 3. 约束对照

| 约束（本仓库事实） | `rquickjs` 0.14.0 | `boa_engine` 0.22.0 |
| :--- | :--- | :--- |
| 许可证准入（`THIRD-PARTY-NOTICES.md` + `license-guard.py` 白名单） | MIT ✅ | Unlicense OR MIT ✅ |
| MSRV（实测 rustc 1.98.1） | 1.87 ✅ | 1.91.0 ✅ |
| std / no_std（全工作区 std） | 默认 `std`，支持 alloc/no_std ✅ | std 形态 ✅ |
| 静态链接 | QuickJS C 源码随 crate 静态编译 ✅ | 纯 Rust 静态链接 ✅ |
| Android 交叉编译 | 需目标 C 编译器（NDK clang 已配；`cc` 已在锁文件）⚠️ 需验证 | 纯 Rust，无额外工具链 ✅ |
| 无 iOS 目标的现状 | 若未来加 iOS，需 Apple clang + QuickJS C 交叉编译 ⚠️ | 纯 Rust，加 iOS target 即可 ✅ |
| 依赖/体积 | 门面 37KB + C 源码，实际链接增量**未在本仓库实测** | 944KB 源码 + 77 依赖，实际链接增量**未实测** |
| 维护活跃度 | 0.14.0 发布于 2026-09-18，活跃 | 0.22.0 发布于 2026-08-28，活跃 |
| 沙箱 ABI / 熔断落点 | 需设计 FFI 中断回调（500ms）+ 内存配额（64MB） | 需设计中段/配额钩子，纯 Rust 较易 |
| 与 mihomo 生态脚本语义匹配 | QuickJS（ES2020）更接近内核脚本习惯 ✅ | 「部分语言支持」，语义偏离风险更高 ⚠️ |

**未实测项（诚实声明）**：本文没有把任一 crate 加进构建，因此**没有**可复现的
release 二进制体积增量、也没有 Android 实机启动/内存数据。上表体积列是 crates.io
源码大小的**测量值**，不是链接后体积；链接增量必须在真正引入时用
`cargo bloat`/APK 差分实测。

---

## 4. 推荐 / Recommendation

**推荐：当前不添加 JS 引擎。** 保持指令 DSL 为默认实现，只保留已经落地的可插拔接缝
（`ScriptEnginePort` + `ScriptEngineCapabilities`），等待触发条件满足。

若未来产品确需任意 JavaScript：

1. **首选 `rquickjs`**：它与 mihomo 生态的脚本语义（ES2020）最接近，静态编译、体积小、
   性能好；代价是引入 C 交叉编译与 FFI 沙箱边界，必须先在 Android 上验证
   `cc` + NDK 的静态链接与 page-size 参数（`.cargo/config.toml` 已设 16KB 对齐）。
2. **备选 `boa_engine`**：当「禁止 C 工具链 / 需要纯 Rust 可审计」成为硬约束（例如未来
   加 iOS 且不接受 Apple clang 交叉编译 C）时选它；接受更大的二进制与 77 依赖。
3. **不选 `quick-js`**：2021 年后未更新，且底层 `libquickjs-sys` 维护停滞。

**触发条件（满足任一即重新评审本记录）**：
- 产品明确要求用户运行任意 JavaScript（扩展脚本生态），且指令 DSL 无法表达；
- 有可复现的二进制体积预算（桌面 release / Android APK 差值）与移动实机内存数据；
- 沙箱 ABI 定稿：超时中断回调、64MB 内存配额、宿主回调（网络/文件/通知）权限边界；
- CI 纳入 iOS 或其它无 C 工具链目标，需要在 `rquickjs`/`boa_engine` 间做最终取舍。

---

## 5. 迁移计划（接缝已在位）

1. 以 **非默认特性** 引入引擎：`script-engine-quickjs`（或 `script-engine-boa`），
   `cargo build` 默认仍是不带 JS 引擎的指令 DSL。
2. 新增 `ScriptEnginePort` 适配器实现，`kind()` 返回 `ScriptEngineKind::JavascriptEngine`，
   `capabilities()` 返回 `supports_javascript_syntax = true` 与真实限额；
   `ScriptApplication::with_engine` 已可直接注入，两表面**无需改动**即可渲染。
3. 指令 DSL 继续作为默认与被禁用引擎时的回退；按 profile 维度显式 opt-in。
4. 把 `ScriptCircuitBreaker` 接到引擎中断/配额：超时用引擎中断机制，内存用引擎配额，
   失败仍走现有 typed `ScriptSandboxStatus` 与安全降级路径。
5. 在 `THIRD-PARTY-NOTICES.md` 登记所选引擎（许可证已在白名单内），并经
   `scripts/quality/license-guard.py`。
6. 矩阵 `DUAL-10-01` 行仅当真实引擎适配器在共享矩阵中被执行后，才可从 `planned`
   升为 `covered`；在此之前守卫继续反向禁止 `rquickjs`/`boa_engine`/`quickjs::`。

---

## 6. 本次已落地的接缝（供迁移复用）

- 端口：`crates/infiltrator-ports/src/script_engine.rs` —— `ScriptEnginePort`
  （`kind()` / `capabilities()` / `execute()`，同步、无 Tokio/UI 类型）。
- 默认适配器：`crates/infiltrator-application/src/script_engine_direct.rs` ——
  `DirectiveDslScriptEngine` 包真实 domain `ScriptEngine`。
- 读模型：`ScriptEngineKind`（含 `JavascriptEngine` 协商槽，当前永不产出）+
  `ScriptEngineCapabilities`（`supports_javascript_syntax`、64MB/500ms 限额）+
  `ScriptSandboxSnapshot.engine_capabilities`。
- 双端渲染：Iced `view/script_console.rs::engine_meta_rows` 与 Bevy
  `pages/profiles_script.rs` 的「引擎能力」行，均如实显示「不支持 JavaScript 语法
  （仅指令 DSL）」；注入 JS 引擎时同一代码渲染 JS 标签（测试证明不改面）。
- 守卫：`scripts/quality/scripting-sandbox-guard.py` 固化接缝标记、本文存在性，并继续
  反向断言仓库不出现任何 JS 引擎依赖。