# Mihomo Despicable Infiltrator - Project Context

## Project Overview

**Mihomo Despicable Infiltrator** is a specialized **Tauri v2** tray application designed to manage the `mihomo` core (a proxy client). It acts as a wrapper and manager, providing a system tray interface, process management, and a web-based configuration UI.

### Key Features
*   **Tray-based Management:** Starts/stops the mihomo core, manages system proxy (Windows), and provides quick actions via the system tray.
*   **Web UI Hosting:** Built-in Axum server hosts:
    *   `zashboard`: A static web interface for mihomo (Metacubexd).
    *   `config-manager-ui`: A custom Vue 3 application for managing subscriptions and configurations.
*   **Core Management:** Handles downloading, versioning, and switching of the `mihomo` binary via the `mihomo-rs` crate.
*   **Subscription System:** Manages subscription links securely (system keychain) with fallback mechanisms.

## Architecture

The project is a **Rust Workspace** mixed with a **Node.js/Vue** frontend environment.

### Workspace Members
1.  **`src-tauri`**: The main application entry point.
    *   **Role:** Tauri backend, system tray logic, Axum server, OS integration (autostart, proxy settings).
    *   **Tech:** Rust, Tauri v2.
2.  **`crates/despicable-infiltrator-core`**: Shared business logic.
    *   **Role:** Platform-agnostic logic for runtime management, config validation, and profile handling. Decoupled from Tauri.
    *   **Tech:** Rust.
3.  **`mihomo-rs`**: A specialized SDK crate.
    *   **Role:** Low-level management of the `mihomo` process, API interaction, and version management.
    *   **Tech:** Rust.

### Frontend Components
1.  **`config-manager-ui`**: The subscription and settings manager.
    *   **Tech:** Vue 3, Vite, TypeScript, Tailwind CSS.
    *   **Build Output:** `dist/` (packaged into `bin/config-manager`).
2.  **`zashboard`**: Static assets for the Mihomo dashboard.
    *   **Source:** External upstream (Metacubexd), stored in `zashboard/`.
    *   **Packaged into:** `bin/zashboard`.

## Development & Build

### Prerequisites
*   Node.js ≥ 18.18, pnpm ≥ 8
*   Rust toolchain (1.75+ recommended)

### Commands

| Action | Command | Context |
| :--- | :--- | :--- |
| **Install Dependencies** | `pnpm install` | Root (installs Tauri CLI) |
| **Install UI Deps** | `pnpm --dir config-manager-ui install` | Root |
| **Run Dev Mode** | `pnpm dev` | Root (Starts Tauri in tray-only mode) |
| **Build UI** | `pnpm --dir config-manager-ui build` | Root (Required before full build) |
| **Build App (MSI)** | `pnpm build` | Root (Generates `.msi` in `target/release/bundle/msi`) |
| **Test SDK** | `cargo test` | `mihomo-rs/` directory |
| **Test Backend** | `cargo test` | `src-tauri/` directory |

### Build Pipeline
1.  Build `config-manager-ui` → outputs to `config-manager-ui/dist`.
2.  Run `pnpm build` (Tauri) → bundles `zashboard`, `config-manager-ui/dist`, and `mihomo.exe`.
3.  Tauri generates the MSI installer.

## Project Conventions & Standards

**Critical**: Strictly adhere to the rules defined in `AGENTS.md`.

### 1. Rust Panic Safety (Strict)
*   **No `unwrap()` or `expect()`** in business logic (`src-tauri/`, `crates/`).
*   **Exception:** Allowed in tests (`tests/`) and examples (`examples/`).
*   Use `?` for error propagation or `ok_or_else` for explicit error handling.
*   **Forbidden**: `mutex.lock().unwrap()` (Use `.unwrap_or_else(|p| p.into_inner())` or similar safe patterns).
*   **Forbidden**: Indexing `slice[i]` (Use `.get(i)`).

### 2. Safety
*   **No `unsafe` code**. If system APIs are needed, use safe wrappers (`windows-sys`, `nix`) or justify extensively.

### 3. File & Directory Usage (`USAGE_SPEC.md`)
*   **Runtime Settings**: `%APPDATA%\com.mihomo.despicable-infiltrator\settings.toml`
*   **Mihomo Data**: `%USERPROFILE%\.config\mihomo-rs\` (configs, logs, versions).
*   **Binary Name**: Always refer to the bundled core as `mihomo.exe`.

## Directory Structure

*   `src-tauri/` - **Main Backend**. Contains `main.rs` (entry), `tray.rs`, `runtime.rs`.
*   `crates/despicable-infiltrator-core/` - **Core Logic**. `lib.rs`, `admin_api.rs`, `profiles.rs`.
*   `mihomo-rs/` - **SDK**. `src/`, `examples/`, `tests/`.
*   `config-manager-ui/` - **Frontend Source**. `src/`, `vite.config.ts`.
*   `zashboard/` - **Static Assets**.
*   `bin/` - **Runtime Resources** (Virtual path in build config, maps to physical dirs).
