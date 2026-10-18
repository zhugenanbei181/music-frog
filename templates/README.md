# Lurker Multi-UI Application Templates

> 层级：L3

Ready-to-use, decoupled proxy client application templates powered by the neutral Lurker core pipeline
([`SurfaceReader`] + [`CommandSink`]).

## Available Framework Templates

| Framework | Target Platform | Memory Footprint | Architecture | Template Directory |
|---|---|---|---|---|
| **Slint** | Desktop (Linux/Windows/macOS) | < 25 MB | Declarative / Native Reactive | [`templates/slint`](slint/) |
| **Iced** | Desktop (Linux/Windows/macOS) | < 45 MB | Strict Elm Architecture | [`templates/iced`](iced/) |
| **Bevy 0.20** | Desktop (Linux/Windows) | High (GPU Render) | Data-Oriented ECS / Headless | [`templates/bevy`](bevy/) |
| **Android** | Mobile (Android API 26+) | < 35 MB | Jetpack Compose + VpnService | [`templates/android`](android/) |

## Scaffold Generation with `cargo-generate`

Generate a fresh project using the interactive prompt:

```bash
cargo generate --path templates --name my-proxy-client
```

Select your desired UI framework (Slint / Iced / Bevy / Android) to automatically prune unrelated toolkits.
