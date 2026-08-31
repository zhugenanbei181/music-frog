# MusicFrog Despicable Infiltrator

Cross-platform manager for mihomo-based proxy configurations. The **primary desktop client is a native Rust app** (`crates/infiltrator-iced`, built on iced 0.14 with tray-icon/muda for the system tray): profiles/subscriptions, proxy & rule management, DNS/Fake-IP editors, runtime diagnostics (connections/logs/traffic with live stream and auto refresh), proxy delay testing (single + batch), core control (stable update/download/switch), language/theme preferences, and a full-featured system tray (per-group node switching, profile quick-switch/update-all/auto-update, kernel management, WebDAV sync, autostart, factory reset, info section, OS notifications and boot retry with controller-port rotation). The former **Tauri + Vue web client was retired in release/0.20** (ledger: [docs/TAURI_WEBUI_RETIREMENT_LEDGER.md](docs/TAURI_WEBUI_RETIREMENT_LEDGER.md)) and the embedded admin server continues API-only for the Doctor diagnostics loop. The Android companion app provides VPN/TUN controls, per-app routing with single FFI-backed state, profile edit/import/subscription management, runtime connection management (filter + disconnect), extended DNS/TUN advanced fields (`fallback-filter`, `stack`, `auto-detect-interface`), DNS/Fake-IP/Rules management, and WebDAV sync.

## Tech Stack & Libraries

- Desktop (primary): Rust, iced 0.14, tray-icon/muda, Tokio, Reqwest, Serde
- Desktop: Iced, Axum (embedded admin API), notify-rust (OS notifications), Reqwest, Tokio, Serde

## Packaging

Fresh clones must run `scripts/fetch-mihomo.sh` before packaging to fetch the mihomo kernel binaries.

## AI Codex

This project is fully developed and maintained by AI assistants.

- **Google Gemini 2.5 / 3.0 Flash/Pro**: Core logic, system integration, and feature planning.
- **Anthropic Claude 4.5 Sonnet**: Frontend UI/UX design and component refactoring.
- **OpenAI Codex**: Code completion, routine refactorings, and documentation upkeep.

## Documentation Links

- [USAGE_SPEC.md](USAGE_SPEC.md) - Detailed feature descriptions and usage guide (Bilingual).
- [docs/README.md](docs/README.md) - Architecture, functional map, UI split, platform matrix, and upstream policy.
- [docs/MIHOMO_CORE.md](docs/MIHOMO_CORE.md) - Rust/mihomo lifecycle, controller, configuration, and release contract.
- `TODO.md` - Local ignored workboard; task execution order is intentionally kept out of public product docs.
