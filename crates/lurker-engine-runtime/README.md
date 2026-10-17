# lurker-engine-runtime

> 层级：L3

引擎中性的 backend provider 注册表、owner 切换协调器与高层运行时 Facade。它按 `EngineId` 把请求分派给唯一 provider，验证返回 bundle 的引擎身份，并在 provider 缺失时直接返回 typed `Unsupported`，不会调用任何 provider 或触发 adapter 副作用。

## 边界

- 只依赖 `lurker-core-contract`、`lurker-core-ports`、`async-trait`、`futures-channel`、`futures-core` 与 `futures-timer`。
- 不依赖任何 driver（`mihomo-driver`、`singbox-driver`）、产品 crate、产品 contract、传输运行时、tokio 或 UI toolkit。
- 不持有引擎专属知识：调度、注册、生命周期状态、owner 切换与状态流是唯一职责；具体 provider 与 driver 由使用者组合。
- 产品扩展 bundle（例如 `infiltrator-ports::engine_backend::ProductEngineBackendPorts`）通过泛型 `EngineBackendBundle` 接入中性注册表与协调器；高层 Facade 直接使用中性 `EngineBackendPorts`。

## API

`EngineBackendProviderRegistry<B: EngineBackendBundle>`：

- `new(providers)`：一次性注册，重复引擎返回 `EngineBackendRegistryError::DuplicateProvider`。
- `empty()`：空注册表。
- `register(provider)` / `unregister(engine_id)`：动态注册与移除；未知引擎的移除返回 `UnknownProvider`。
- `availability(engine_id)`：不调用 provider，只报告 `Registered` 或 typed `Unsupported`。
- `backend(engine_id)`：未注册引擎返回 `PortError::Unsupported { capability: CoreLifecycle, .. }` 且零 provider 调用；provider 返回的 bundle 引擎身份不符时返回 `PortError::Failed`，不会被静默接受。

`EngineSwitchCoordinator<B, J>`：

- `reserve_switch(revision, target)`：validate → dispatch/prepare → 持久化保留 → 旧 owner exact terminal → 新 selection → `BindingNew` 准入，停在延迟 apply 交接点。
- `finalize_reserved` / `cancel_reserved` / `discard_reserved`：宿主自行应用候选时使用的收口入口。
- `finalize_switch` / `switch_and_finalize`：由协调器经 host 应用候选的一次性入口。
- 保留前失败回到原 owner；旧 owner 终态之后失败只报告 `RecoveryRequired`，不伪造自动恢复。

`LurkerRuntimeBuilder` / `LurkerRuntime`：

- builder：`register_engine`、`select_engine`、`with_instance_id`、`with_switch_journal`（默认 `MemoryEngineSwitchJournal`）、`with_readiness_timeout`、`build`。
- `build` 在空注册表、缺少 selection 或 selection 未注册时 fail closed。
- runtime：`state`、`state_stream`、`start`、`stop`、`switch_engine`、`wait_ready`、`scope`、`outbound`、`routing_intent`。
- `state_stream` 对新订阅者先重放当前状态，再按顺序推送后续更新；使用 `futures-channel` 与 `Mutex<Vec<Sender>>`，不依赖 tokio。
- 同一时刻只允许一个 `start`/`stop`/`switch_engine`；并发的第二个控制调用返回 typed `RuntimeError::ControlBusy`。

## 十行用法

```rust
use std::time::Duration;
use lurker_core_contract::identity::EngineId;
use lurker_engine_runtime::facade::LurkerRuntimeBuilder;

let runtime = LurkerRuntimeBuilder::new()
    .register_engine(provider)?          // Arc<dyn EngineBackendProvider<EngineBackendPorts>>
    .select_engine(EngineId::mihomo())
    .with_readiness_timeout(Duration::from_secs(30))
    .build()?;
runtime.start().await?;
let mut states = runtime.state_stream(); // 先重放当前状态，再推送更新
while let Some(state) = states.next().await { println!("{state:?}"); }
```

宿主可额外调用 `.with_switch_journal(Arc::new(journal))` 持久化 owner 选择，或用 `.with_instance_id(instance)` 要求每个引擎 adapter 报告同一配置实例。

## 状态机

`LurkerRuntimeState` 是一个穷尽枚举：`Idle`、`Starting`、`Running`、`Switching`、`Stopping`、`Stopped`、`RecoveryRequired`、`Failed`。不存在用多个 bool / `Option` 组合表达的隐式阶段；`Running` 的 `version` 与 `Stopped` 的 terminal 是真实读回事实，不是阶段标志。

`start` 的顺序固定为：registry dispatch → require `core_identity()` → `prepare_start` → `journal.bind_owner` → `process.start()` → bounded readiness → `bind_owner_core_scope` → `Running`。`stop` 要求 exact `Stopped` terminal 且 `pid() == None` 后才发布 `Stopped`。

**Standalone owner：** Facade 始终使用 `CoreProcessStartContext::Standalone`。Android 的 `Managed` owner（VPNService session proof 与 managed child recovery evidence）留在 Android 产品组合层，不经过本 Facade 的 `start`。

## 验收

```text
cargo nextest run -p lurker-engine-runtime
cargo clippy -p lurker-engine-runtime --all-targets -- -D warnings
```
