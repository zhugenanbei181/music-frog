# lurker-template-bevy

> 层级：L3

Extreme-performance, ECS-driven desktop proxy client application template built on [Bevy 0.20](https://bevyengine.org/)
and the Lurker neutral core pipeline (`SurfaceReader` + `CommandSink`).

## Features

- **Data-Oriented ECS**: UI layout and interactions structured purely as entities, components, and systems.
- **Headless Testability**: Fully supports headless CI execution (`MinimalPlugins`) with deterministic frame-stepping.
- **Pipeline Integration**: Dispatches typed [`CommandIntent`] through thread-safe [`CommandSink`].

## Quick Start

```bash
cargo run
cargo nextest run
```
