# lurker-core-ports

> 层级：L3

Engine-neutral, transport-free interface ports shared across all proxy core drivers and hosts.
It abstracts the outbound boundaries for process control, core readiness, outbound proxy selection,
routing intent, and configuration codecs.

This crate depends only on `lurker-core-contract`, `async-trait`, and `url`. It contains no concrete
networking, OS process spawning, or UI framework dependencies.

## Defined Ports

- `process` — `CoreProcess`: abstract process lifecycle supervision and signal management.
- `readiness` — `CoreReadiness`: observation of live controller endpoints and health.
- `outbound` — `OutboundPort`: proxy inventory querying, node selection, and latency probing.
- `routing_intent` — `RoutingIntentPort`: observing and applying global/rule/direct routing modes.
- `profile_document_codec` — `ProfileDocumentCodecPort`: document validation, parsing diagnostics, and snippet splicing.
- `engine_backend` — `EngineBackendProvider` and `EngineBackendPorts`: engine capability registry and bundle wiring.
- `error` — `PortError`: typed adapter error categories protecting application invariants.

## Licensing

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](../../LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](../../LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.
