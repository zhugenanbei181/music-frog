# MusicFrog Despicable Infiltrator

Cross-platform manager for mihomo-based proxy configurations. The **primary desktop client is a native Rust app** (`crates/infiltrator-iced`, built on iced 0.14 with tray-icon/muda for the system tray): profiles/subscriptions, proxy & rule management, DNS/Fake-IP editors, runtime diagnostics (connections/logs/traffic with live stream and auto refresh), proxy delay testing (single + batch), core control (stable update/download/switch), language/theme preferences, and system-tray shortcuts. The **Tauri + Vue app** (`src-tauri`) is the **legacy/secondary desktop client**, which additionally serves the Admin Web UI (Axum) for profiles, network settings, rules, advanced `rule-providers`/`proxy-providers`/`sniffer` editors, grouped tray shortcuts, and live update notifications for tray and core changes. The Android companion app provides VPN/TUN controls, per-app routing with single FFI-backed state, profile edit/import/subscription management, runtime connection management (filter + disconnect), extended DNS/TUN advanced fields (`fallback-filter`, `stack`, `auto-detect-interface`), DNS/Fake-IP/Rules management, and WebDAV sync.

## Tech Stack & Libraries

- Desktop (primary): Rust, iced 0.14, tray-icon/muda, Tokio, Reqwest, Serde
- Desktop (legacy): Tauri, Axum, SQLx, Reqwest, Tokio, Serde
- Frontend (legacy web UI): Vue 3, Vite, Tailwind CSS

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
