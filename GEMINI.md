# Mihomo Despicable Infiltrator - Project Context

## Overview

**Mihomo Despicable Infiltrator** is a Tauri v2 tray app managing the `mihomo` core.

## Architecture

- **Workspace**:
  - `src-tauri`: Backend, Tray, Axum server, OS integration (imports switched to new crates).
  - `crates/infiltrator-core`: Cross-platform business logic (config, profiles, scheduler, Admin API).
  - `crates/infiltrator-desktop`: Desktop integrations (runtime, system proxy, editor, version).
  - `crates/mihomo-api`: HTTP API client (client/types/proxy/connection).
  - `crates/mihomo-config`: Config management (YAML/TOML, credential abstraction).
  - `crates/mihomo-version`: Desktop version manager (download/install).
  - `crates/mihomo-platform`: Platform traits + Desktop/Android implementations.

## Key Concepts

- **Thread Safety**: Runtime mutations (rebuild, factory reset) are protected by `rebuild_lock` in `AppState`.
- **Modularization**: Core logic resides in `infiltrator-core`, Desktop integration in `infiltrator-desktop`, both decoupled from Tauri.
- **Panic Safety**: Strict "no panic" policy in production code.

## Build

1. 仅在仓库根目录执行构建命令，避免锁定子目录。
2. `pnpm --dir config-manager-ui build`
3. `pnpm build` (Outputs MSI on Windows)

## Planning Playbook

### Human Notes

Phase A covers cross-platform basics (DNS/Fake-IP/Rule Providers/TUN advanced). Phase B covers Android-only work (per-app proxy, VPN service, core runtime mode). Each item has a crate owner and a minimal milestone tracked in `TODO.md` and `ANDROID.md`.

### AI Prompting

Act as a planning executor: always map features to crates, define minimal milestones, and update `TODO.md`/`ANDROID.md`/`CHANGELOG.md`. Ship cross-platform items before Android-only tasks, and preserve existing Tauri behavior.
