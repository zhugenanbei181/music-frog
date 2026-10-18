# lurker-core-contract

> 层级：L3

Engine-neutral, platform-neutral and UI-neutral contract kernel for proxy core orchestrators.
It defines pure domain identities, lifecycle state machines, typed failures, outbound and
routing intent models, and page-level presentation contracts.

This crate depends strictly on `std` and minimal core libraries (`serde`, `url`, `anyhow`).
It never depends on any UI framework, concrete engine driver, platform API, or network runtime.

## Core Capabilities

- `identity` — engine identity (`EngineId`), versioning, and binary host provenance.
- `lifecycle` — typed, sealed lifecycle states (`CoreLifecycle`) and terminal readbacks.
- `error` — user-facing typed error codes (`ErrorCode`, `Failure`) and recovery policies.
- `routing_intent` — engine-neutral traffic routing intents (`RoutingIntent::FollowRules`, `RouteAllThroughProxy`, `BypassProxy`).
- `outbound` — typed proxy inventory observations, selection requests, and latency measurement DTOs.
- `page` — neutral UI view status models (`PageStatus`, `PageData`, `SurfaceOrigin`).
- `redact` — RFC 7235 and standard credential redaction invariants.

## Licensing

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](../../LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](../../LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.
