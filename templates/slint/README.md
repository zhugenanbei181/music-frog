# lurker-template-slint

> 层级：L3

Lightweight, modern desktop proxy client application template built with [Slint](https://slint.dev/)
and the Lurker neutral core pipeline (`SurfaceReader` + `CommandSink`).

## Features

- **Extreme Lightness**: Near-zero idle RAM footprint (< 25MB) and rapid cold-start times.
- **Unidirectional Architecture**: UI actions strictly dispatch typed [`CommandIntent`] through [`CommandSink`]; status updates bind via [`SurfaceReader`].
- **Clean Decoupling**: Completely free from internal engine adapters and private product structures.

## Quick Start

```bash
cargo run
```
