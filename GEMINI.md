# Mihomo Despicable Infiltrator - Project Context

## Overview
**Mihomo Despicable Infiltrator** is a Tauri v2 tray app managing the `mihomo` core.

## Architecture
- **Workspace**:
  - `src-tauri`: Backend, Tray, Axum server, OS integration.
  - `crates/despicable-infiltrator-core`: Business logic (Runtime, Admin API, Profiles).
  - `mihomo-rs`: SDK for mihomo process/API management.
- **Frontend**:
  - `config-manager-ui`: Vue 3 app for config/subscription management.
  - `zashboard`: Static assets for Metacubexd.

## Key Concepts
- **Thread Safety**: Runtime mutations (rebuild, factory reset) are protected by `rebuild_lock` in `AppState`.
- **Modularization**: Core logic resides in `despicable-infiltrator-core`, decoupled from Tauri.
- **Panic Safety**: Strict "no panic" policy in production code.

## Build
1. `pnpm --dir config-manager-ui build`
2. `pnpm build` (Outputs MSI on Windows)