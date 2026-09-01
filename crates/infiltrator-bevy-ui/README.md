# infiltrator-bevy-ui

Bevy UI frontend shell for MusicFrog Infiltrator — the strategic unified
desktop+mobile surface. Peer to `infiltrator-iced` (desktop), Tauri/Web
(compat) and the Android Compose companion; the coexistence matrix lives in
[docs/FRONTENDS.md](../../docs/FRONTENDS.md) and the frontend charter in
[docs/BEVY_UI_FRONTEND.md](../../docs/BEVY_UI_FRONTEND.md).

## Why a second native frontend

iced is winit-desktop-only: it has no Android story, so the mobile ceiling of
this product currently sits with the separate Kotlin companion. bevy runs the
same UI tree on `aarch64-linux-android`. The widget budget is thin upstream,
but `bevy_ui_widgets` (official unstyled widgets) plus our own
`infiltrator-bevy-widgets` layer closes that gap — widgets we build, we own.

## Current state (M1 shell + first M2 page slice)

- `ShellPlugin` + `shell_scene`: window chrome, header (title, flex spacer,
  theme pill), `ContentSlot`.
- **Routing** (`route.rs`): a `RouteChanged` trigger swaps the page by
  bounded subtree replacement under the `ContentSlot` —
  `despawn_children` on the slot, then `spawn_scene` for the new page
  (despawn is recursive per the vendored bevy_ecs 0.19.1 docs; mounting
  never touches `.spawn(`). Same-route re-triggers are no-ops, so a shown
  page keeps its entity ids; `PageRoot(Route::…)` marks every mounted
  page root for assertions and nav chrome. `PagesPlugin` mounts the
  default route the moment the slot exists and takes the injectable
  projection source.
- **Overview page** (`pages/overview.rs`): the first real page — run
  state, proxy-mode pill group, upload/download rates, connection count
  and the whole-card failure projection, assembled from the widget
  layer's `surface_scene` / `pill_scene` with token-only colors.
  `projection.rs` is the pure data seam: a zero-bevy
  `OverviewProjection` (typed Running/Stopped/Unavailable tri-state),
  the `OverviewSource` trait, and the switchable `DemoOverviewSource`
  fixture. **No live mihomo-api transport yet** — the data pump is the
  next slice; until then the page renders the injected fixture.
- **Refresh seam**: the page self-registers one observer (bind hook on
  the page root, once per world). An `OverviewProjectionUpdated`
  trigger restamps text contents, inks, pill selection and the card fill
  in place — no polling, no tree rebuilds, entity ids stable (asserted
  by headless tests).
- Fonts are embedded: the widget layer registers its four OFL faces into the
  `Assets<Font>` store, so every role stamps a real face (no cosmic-text
  system-fallback dependence).
- Theme is live: the header pill flips dark ↔ light. The shell mirrors the
  mode in a `ThemeMode` resource, activation triggers the widget layer's
  `ThemeSwitch`, and the mounted tree is restamped in place — entity ids
  never change (asserted by headless tests).
- Minimal accessibility loop: the shell stamps AccessKit `AccessibilityNode`
  seeds — root as a named `Window` role, header as `Header`, the pill as a
  named `Button` — and the Linux build enables the `accesskit_unix` feature
  so the windowed winit bridge publishes them over AT-SPI. The bridge plugin
  itself lives in bevy_winit and activates only with `DefaultPlugins`
  (windowed); headless runs carry the seeds as inert components. `accesskit`
  is a direct dependency for the node/role vocabulary (bevy_a11y 0.19 no
  longer re-exports it; 0.24.1 matches the locked bevy resolution).
- Dependency whitelist: locked bevy (`=0.19.1`) + `infiltrator-bevy-widgets`
  + the accesskit vocabulary crate. Business crates are deliberately absent
  until the shared contract seam (BEVY-M2).
- `infiltrator_bevy_ui::run()` launches the windowed shell.

## Verify

The crate is a standalone workspace on purpose (empty `[workspace]`
table) — run cargo from this directory:

```bash
cargo nextest run
cargo clippy --all-targets -- -D warnings
# windowed smoke (needs a Wayland session):
cargo run
```
