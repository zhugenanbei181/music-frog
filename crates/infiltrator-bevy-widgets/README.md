# infiltrator-bevy-widgets

Business-agnostic Bevy UI widget layer for MusicFrog: `bsn!` scene functions
over the official unstyled [`bevy_ui_widgets`](https://crates.io/crates/bevy_ui_widgets).

## Charter law (docs/BEVY_UI_FRONTEND.md)

- **Locked bevy**: `=0.19.1`, explicit per-target feature closure, default
  features off. Same patch taskmanager locks, so a future shared extraction
  resolves one widget ABI.
- **Zero business dependencies**: bevy only — never `infiltrator-core`,
  `mihomo-*` or any sibling frontend crate. This crate is the future
  extraction candidate for cross-project shared widgets.
- **`bsn!` scene functions only**: static structure composes declaratively;
  no imperative `Node`/`Children`/`with_children` trees.
- **Tokens are the only skin authority**: colors/metrics originate in
  `theme.rs` and become bevy values only in `palette.rs`; Feathers (bevy's
  official skin system) is not adopted.
- **Observers restamp, never rebuild**: typography lands via the
  `TextRole` observer; control fills repaint via `sync_control_visuals`.

## Modules

| module | contents |
| --- | --- |
| `theme` | neutral tokens (iOS design language, dark/light), spacing/radius/metrics/type scales |
| `palette` | `UiPalette` — the single token → bevy value adapter |
| `text` | `Role`/`TextRole` + the typography stamping observer |
| `button` | `ControlVisual` + `control_fill` + `pill_scene` + repaint system (over official `Button`) |
| `surface` | `surface_scene` card chrome accepting composed child scenes |

## Verify

```bash
cargo nextest run -p infiltrator-bevy-widgets
cargo clippy -p infiltrator-bevy-widgets --all-targets
```
