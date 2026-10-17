# lurker-engine-runtime

> 层级：L3

引擎中性的 backend provider 注册表。它按 `EngineId` 把请求分派给唯一 provider，验证返回 bundle 的引擎身份，并在 provider 缺失时直接返回 typed `Unsupported`，不会调用任何 provider 或触发 adapter 副作用。

## 边界

- 只依赖 `lurker-core-contract`、`lurker-core-ports` 与 `async-trait`。
- 不依赖任何 driver（`mihomo-driver`、`singbox-driver`）、产品 crate、产品 contract、传输运行时或 UI toolkit。
- 不持有引擎专属知识：调度、注册、可用性检查与 bundle 身份校验是唯一职责；具体 provider 与 driver 由使用者组合。
- 产品扩展 bundle（例如 `infiltrator-ports::engine_backend::ProductEngineBackendPorts`）通过泛型 `EngineBackendBundle` 接入，中性层不知道其字段。

## API

`EngineBackendProviderRegistry<B: EngineBackendBundle>`：

- `new(providers)`：一次性注册，重复引擎返回 `EngineBackendRegistryError::DuplicateProvider`。
- `empty()`：空注册表。
- `register(provider)` / `unregister(engine_id)`：动态注册与移除；未知引擎的移除返回 `UnknownProvider`。
- `availability(engine_id)`：不调用 provider，只报告 `Registered` 或 typed `Unsupported`。
- `backend(engine_id)`：未注册引擎返回 `PortError::Unsupported { capability: CoreLifecycle, .. }` 且零 provider 调用；provider 返回的 bundle 引擎身份不符时返回 `PortError::Failed`，不会被静默接受。

## 验收

```text
cargo nextest run -p lurker-engine-runtime
```
