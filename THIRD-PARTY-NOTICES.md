# THIRD-PARTY NOTICES / 第三方组件声明

This repository redistributes or vendored-builds the following third-party
content. Each item keeps its upstream license. / 本仓库包含以下第三方内容，
均遵循其上游许可协议。

## 1. mihomo core binaries (`vendor/`)

- Source: official releases of [MetaCubeX/mihomo](https://github.com/MetaCubeX/mihomo),
  pinned to **v1.19.18** (<https://github.com/MetaCubeX/mihomo/releases/tag/v1.19.18>).
- Files: `vendor/mihomo.exe` (from `mihomo-windows-amd64-v3-v1.19.18.zip`),
  `vendor/mihomo-android-amd64` and `vendor/mihomo-android-arm64-v8`
  (from the corresponding `.gz` assets). Unmodified / 未作任何修改。
- License: **GPL-3.0** — per the upstream `LICENSE` file and README
  ("This software is released under the GPL-3.0 license").
  Copyright (c) the MetaCubeX/mihomo authors.
- Note: the binaries are no longer git-tracked; restore them with
  `scripts/fetch-mihomo.sh` after a fresh clone.
- Upstream additionally requires that downstream projects not affiliated with
  MetaCubeX do not use the word "mihomo" in their project names (upstream
  README). This repo does not use it in the project name; the vendored file
  names above merely mirror the upstream release asset names. / 上游 README 另有
  约定：非 MetaCubeX 关联的下游项目名称不得包含 “mihomo”。本仓库项目名未使用该词，
  vendor 文件名仅沿用官方发布产物名称。

## 2. Retired vendored assets / 已退役的第三方资源

The zashboard dashboard build (`webui/mihomo-manager-ui/dist/`, MIT, ©
Zephyruso) and the Vue admin panel (`webui/config-manager-ui/`) were
removed in release/0.20 together with the Tauri host. Their license
notices applied to the vendored copies while they existed; nothing from
them is redistributed any longer. / zashboard 仪表盘与 Vue 管理面板已随
Tauri 宿主于 release/0.20 一并移除，本仓库不再分发其任何产物。

## 3. Scope / 范围说明

This file covers vendored binaries and prebuilt third-party assets. Rust
crates and npm packages are dependencies declared in `Cargo.toml` /
`package.json` files and are governed by their own licenses. Since BEVY-008
the bevy dependency stack added for the bevy-based desktop client is also
recorded below (sections 7–11); other Rust crates and npm packages remain
governed by their own licenses. / 本文件覆盖入库的二进制与预构建第三方资源；
Rust crate 与 npm 依赖以其各自声明文件及许可证为准。自 BEVY-008 起，bevy 桌面
客户端引入的 bevy 依赖栈另行登记于下方第 7–11 节；其余 Rust crate 与 npm 依赖
仍以其各自声明文件及许可证为准。

## 4. Fonts bundled with the iced desktop client (`crates/infiltrator-iced/assets/fonts/`)

The native iced desktop client embeds the following fonts at build time
(`include_bytes!`, loaded in `src/main.rs`; both under the **SIL Open Font
License 1.1** with the standard font-name reservation clause):

| File | Typeface / upstream | Version | License |
|---|---|---|---|
| `Inter-Regular.ttf`, `Inter-Medium.ttf`, `Inter-SemiBold.ttf` | [Inter](https://github.com/rsms/inter) by Rasmus Andersson | v4.1 | SIL OFL 1.1 |
| `JetBrainsMono-Regular.ttf` | [JetBrains Mono](https://github.com/JetBrains/JetBrainsMono) by JetBrains | v2.304 | SIL OFL 1.1 |

- Inter is the default UI face (Regular / Medium / SemiBold map to
  `font::Weight::Normal / Medium / Semibold`); JetBrains Mono is used for
  latency and throughput numerals (tabular digits) via
  `crates/infiltrator-iced/src/view/theme.rs` (`MONO`).
- Unmodified copies of the upstream release TTFs (checksums recorded at
  vendoring time in the PR that added them). The OFL 1.1 texts are available
  from the upstream repositories linked above
  (<https://github.com/rsms/inter/blob/master/LICENSE.txt>,
  <https://github.com/JetBrains/JetBrainsMono/blob/master/OFL.txt>).
- Reserved Font Names: none invoked; files keep their upstream names.

## 5. Icons bundled with the iced desktop client (`crates/infiltrator-iced/assets/icons/*.svg`)

The 28 monochrome stroke icons in `crates/infiltrator-iced/assets/icons/`
are original hand-written SVGs modeled on the **Lucide** icon style
(24x24 viewBox, `stroke-width="2"`, round caps/joins, `fill="none"`,
`stroke="currentColor"`). Lucide itself is ISC-licensed; these files are not
copies of upstream path data and are distributed with this project's license.
For attribution of inspiration: <https://lucide.dev> (ISC).

## 6. Bevy 0.19.1 engine family (`bevy` facade + sub-crates)

- Source: crates.io, upstream [bevyengine/bevy](https://github.com/bevyengine/bevy),
  pinned by `Cargo.lock` to **0.19.1**. The crates are not vendored into this
  repository; they are pulled from crates.io at build time. / 不随仓库分发，
  构建时由 cargo 自 crates.io 拉取。License 逐个实证自本机
  `~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/<name>-<version>/Cargo.toml`
  的 `license` 字段。
- Members (all at 0.19.1): `bevy` (facade), `bevy_internal`, `bevy_a11y`,
  `bevy_android`, `bevy_app`, `bevy_asset`, `bevy_asset_macros`, `bevy_camera`,
  `bevy_clipboard`, `bevy_color`, `bevy_core_pipeline`, `bevy_derive`,
  `bevy_diagnostic`, `bevy_ecs`, `bevy_ecs_macro_logic`, `bevy_ecs_macros`,
  `bevy_encase_derive`, `bevy_gizmos`, `bevy_gizmos_macros`,
  `bevy_gizmos_render`, `bevy_image`, `bevy_input`, `bevy_input_focus`,
  `bevy_light`, `bevy_log`, `bevy_macro_utils`, `bevy_material`,
  `bevy_material_macros`, `bevy_math`, `bevy_mesh`, `bevy_picking`,
  `bevy_platform`, `bevy_ptr`, `bevy_reflect`, `bevy_reflect_derive`,
  `bevy_render`, `bevy_render_macros`, `bevy_scene`, `bevy_scene_macros`,
  `bevy_shader`, `bevy_sprite`, `bevy_sprite_render`, `bevy_tasks`,
  `bevy_text`, `bevy_time`, `bevy_transform`, `bevy_ui`, `bevy_ui_render`,
  `bevy_ui_widgets`, `bevy_utils`, `bevy_window`, `bevy_winit`.
- License: **MIT OR Apache-2.0** (dual-licensed, same expression for every
  member above), © Bevy contributors — see the upstream repository for the
  full license texts. / 上游全体子 crate 统一双许可 MIT OR Apache-2.0。
- Companion crates maintained alongside Bevy (also **MIT OR Apache-2.0**):
  `variadics_please` 1.1.0 ([bevyengine/variadics_please](https://github.com/bevyengine/variadics_please),
  used by `bevy_app`/`bevy_ecs`/`bevy_render` etc.) and `web-task` 1.1.3
  ([NthTensor/web-task](https://github.com/NthTensor/web-task), wasm task
  backend of `bevy_tasks`).

## 7. wgpu 29 / naga 29 rendering stack (added by `bevy_render`)

- Source: crates.io, upstream [gfx-rs/wgpu](https://github.com/gfx-rs/wgpu)
  (all `wgpu*` and `naga` crates) and
  [bevyengine/naga_oil](https://github.com/bevyengine/naga_oil/). Pinned by
  `Cargo.lock`. / 同上，构建时自 crates.io 拉取，license 实证自本机 vendored
  源码的 `Cargo.toml`。
- Note: the iced client already brought `wgpu`/`naga` **27.x**; the entries
  below are the **29.x** set added with bevy 0.19.1. Both generations coexist
  in `Cargo.lock`. / 27.x 系 iced 原有；下述为 bevy 新增的 29.x 条目，两代并存。
- Core (all **MIT OR Apache-2.0**): `wgpu` 29.0.4, `wgpu-core` 29.0.4,
  `wgpu-hal` 29.0.4, `wgpu-types` 29.0.4, `wgpu-core-deps-apple` 29.0.4,
  `wgpu-core-deps-windows-linux-android` 29.0.4, `wgpu-naga-bridge` 29.0.4,
  `naga` 29.0.4, `naga_oil` 0.22.0.
- Transitive additions (new name or new version entry, added by the stack
  above):
  - `codespan-reporting` 0.13.1 — **Apache-2.0**
    ([brendanzab/codespan](https://github.com/brendanzab/codespan); the older
    0.12.0 entry predates bevy). Pulls `termcolor` 1.4.1 — **Unlicense OR
    MIT** ([BurntSushi/termcolor](https://github.com/BurntSushi/termcolor)).
  - `spirv` 0.4.0+sdk-1.4.341.0 — **Apache-2.0** (SPIRV-Registry headers
    binding, via `naga`).
  - `bit-set` 0.9.1 / `bit-vec` 0.9.1 — **Apache-2.0 OR MIT** (via `naga`).
  - `pp-rs` 0.2.1 — **BSD-3-Clause** (preprocessor used by `naga`).
  - `petgraph` 0.8.3 — **MIT OR Apache-2.0** (via `naga_oil` /
    `tree_magic_mini`); `weak-table` 0.3.2 — **MIT** (via `naga_oil`).
  - `gpu-allocator` 0.28.0 — **MIT OR Apache-2.0**
    ([Traverse-Research/gpu-allocator](https://github.com/Traverse-Research/gpu-allocator));
    `presser` 0.3.1, `range-alloc` 0.1.5, `raw-window-metal` 1.1.0 — **MIT OR
    Apache-2.0** (via `wgpu-hal`).
  - `objc2-metal` 0.3.2 — **Zlib OR Apache-2.0 OR MIT** (macOS Metal
    bindings, version bump via `wgpu-hal`).
  - `rectangle-pack` 0.4.2 — **MIT/Apache-2.0** (via `bevy_image`).

## 8. Text & layout stack added with `bevy_text` / `bevy_ui`

- Source: crates.io; license fields verified from the vendored registry
  checkouts as above. / 来源 crates.io，license 逐个实证自本机 vendored 源码。
- Parley family (upstream
  [linebender/parley](https://github.com/linebender/parley), used by
  `bevy_text`): `parley` 0.9.0, `parley_data` 0.9.0, `fontique` 0.9.0,
  `parlance` 0.1.0 — all **Apache-2.0 OR MIT**.
- Font parsing ([googlefonts/fontations](https://github.com/googlefonts/fontations),
  version bumps via parley/cosmic-text): `skrifa` 0.42.1, `read-fonts` 0.39.2,
  `font-types` 0.11.3 — **MIT OR Apache-2.0**; shaping via `harfrust` 0.6.2 —
  **MIT** ([harfbuzz/harfrust](https://github.com/harfbuzz/harfrust)).
- ICU4X segmentation (unicode-org/icu4x, via `parley`): `icu_segmenter` 2.3.0,
  `icu_segmenter_data` 2.3.0, `icu_locale_fallback` 2.3.0,
  `icu_locale_fallback_data` 2.3.0 — **Unicode-3.0**. Note: Unicode-3.0
  (ICU/Unicode Consortium permissive license) is *not* in the license-guard
  whitelist set; it is registered here verbatim. / Unicode-3.0 不在
  license-guard 白名单内，此处原样登记；该许可为 Unicode 联盟的宽松许可。
- Layout ([DioxusLabs/taffy](https://github.com/DioxusLabs/taffy), used by
  `bevy_ui`): `taffy` 0.10.1 — **MIT**; its dependency `grid` 1.0.1 — **MIT**.

## 9. Accessibility & clipboard stack added with `bevy_a11y` / `bevy_winit` / `bevy_clipboard`

- Source: crates.io; licenses verified from the vendored registry checkouts
  as above. / 来源 crates.io，license 逐个实证自本机 vendored 源码。
- AccessKit ([AccessKit/accesskit](https://github.com/AccessKit/accesskit)):
  `accesskit` 0.24.1, `accesskit_consumer` 0.35.0 and 0.38.0 (two versions
  coexist in `Cargo.lock`), `accesskit_macos` 0.26.3, `accesskit_windows`
  0.32.1 — all **MIT OR Apache-2.0**; `accesskit_winit` 0.32.2 —
  **Apache-2.0** (Apache only, unlike the rest of the family).
- Clipboard ([1Password/arboard](https://github.com/1Password/arboard), via
  `bevy_clipboard`): `arboard` 3.6.1 — **MIT OR Apache-2.0**; its Linux
  backend `wl-clipboard-rs` 0.9.3 — **MIT/Apache-2.0**
  ([YaLTeR/wl-clipboard-rs](https://github.com/YaLTeR/wl-clipboard-rs));
  `os_pipe` 1.2.3 — **MIT**; `tree_magic_mini` 3.2.2 — **MIT**.
- Note: `winit` itself is not a new dependency (the iced client already uses
  it); `bevy_winit` reuses the same locked version. / winit 非新增，iced 原有。

## 10. Other supporting crates added with the bevy stack

All pinned by `Cargo.lock`; licenses verified from the vendored registry
checkouts (`~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/`). /
其余随 bevy 栈入锁的中小依赖，license 均实证自本机 vendored 源码。

- Log/trace stack (via `bevy_log` / `bevy_app`): `tracing-subscriber` 0.3.23
  (**MIT**), `tracing-log` 0.2.0 (**MIT**), `matchers` 0.2.0 (**MIT**),
  `nu-ansi-term` 0.50.3 (**MIT**), `sharded-slab` 0.1.7 (**MIT**),
  `lazy_static` 1.5.0 (**MIT OR Apache-2.0**), `thread_local` 1.1.10
  (**MIT OR Apache-2.0**), `tracing-oslog` 0.3.0 (**Zlib**, macOS only),
  `tracing-wasm` 0.2.1 (**MIT OR Apache-2.0**, wasm only), `valuable` 0.1.1
  (**MIT**, via `tracing-core`), `console_error_panic_hook` 0.1.7
  (**Apache-2.0/MIT**, wasm panic hook via `bevy_app`).
- Engine utilities: `approx` 0.5.1 (**Apache-2.0**), `assert_type_match`
  0.1.1, `atomicow` 1.2.0, `critical-section` 1.2.0, `disqualified` 1.0.0,
  `fixedbitset` 0.5.7, `inventory` 0.3.24, `nonmax` 0.5.5,
  `rand_distr` 0.6.0, `ron` 0.12.2
  ([ron-rs/ron](https://github.com/ron-rs/ron), asset/scene format),
  `radsort` 0.1.1, `hexasphere` 18.0.0, `const_soft_float` 0.1.4,
  `constgebra` 0.1.4, `const-fnv1a-hash` 1.1.0 (**MIT**), `heapless` 0.9.3 +
  `hash32` 0.3.1, `send_wrapper` 0.6.0 (**MIT/Apache-2.0**),
  `stackfuture` 0.3.1 (**MIT**), `offset-allocator` 0.2.0 (**MIT**) —
  all **MIT OR Apache-2.0** unless stated otherwise.
- Shader/buffer layout: `encase` 0.12.1, `encase_derive` 0.12.1,
  `encase_derive_impl` 0.12.1
  ([teoxoy/encase](https://github.com/teoxoy/encase)) — **MIT-0** (MIT without
  the attribution paragraph; registered verbatim, not in the license-guard
  whitelist set). `const_panic` 0.2.17 (**Zlib**) and `typewit` 1.15.2
  (**Zlib**) via encase/stackfuture.
- Hashing: `blake3` 1.8.7
  ([BLAKE3-team/BLAKE3](https://github.com/BLAKE3-team/BLAKE3), via
  `bevy_asset`) — **CC0-1.0 OR Apache-2.0 OR Apache-2.0 WITH
  LLVM-exception**.
- Signals: `ctrlc` 3.5.2 (**MIT/Apache-2.0**,
  [Detegr/rust-ctrlc](https://github.com/Detegr/rust-ctrlc.git), via
  `bevy_app`) and `nix` 0.31.3 (**MIT**, version bump via `ctrlc`).
- Misc (via `derive_more-impl`): `convert_case` 0.10.0 (**MIT**),
  `unicode-xid` 0.2.6 (**MIT OR Apache-2.0**).
