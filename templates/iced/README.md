# lurker-template-iced

> 层级：L3

Responsive, pure Elm-architecture desktop proxy client application template built with [Iced](https://iced.rs/)
and the Lurker neutral core pipeline (`SurfaceReader` + `CommandSink`).

## Features

- **Strict Elm Architecture**: Unidirectional state machine (`State` -> `Message` -> `update` -> `view`).
- **Pipeline Integration**: Asynchronous command intents dispatch through `CommandSink` via `Task::perform`.
- **Dynamic Theming**: Native support for live dark/light mode toggling.

## Quick Start

```bash
cargo run
```
