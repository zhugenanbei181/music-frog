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

## 2. zashboard dashboard build (`webui/mihomo-manager-ui/dist/`)

- `webui/mihomo-manager-ui/dist/` is a **pre-built distribution of
  [zashboard](https://github.com/Zephyruso/zashboard)** (a dashboard for
  Clash/mihomo-type proxies), vendored as build output, with one local
  modification: the proprietary-licensed fonts documented in section 3 have
  been stripped from it (see below). / 该目录为第三方项目 zashboard 的构建产物，
  唯一本地改动为移除了第 3 节所述的专有许可字体。
- License: **MIT**, © Zephyruso and contributors — see the upstream repository
  for the full license text.
- This is not original source of this project; it is a third-party build
  artifact. / 非本项目的原创源码。

## 3. Fonts bundled inside that zashboard build (licensing consideration / 字体授权注意事项)

The vendored dist originates from zashboard's default build, which bundles
subsetted fonts from the `subsetted-fonts` npm package plus zashboard's own
assets. **On 2026-08-29 the proprietary-licensed fonts were stripped from the
vendored dist** — the MiSans/PingFang font files and their `@font-face`
stylesheets were removed. Text now renders with system fonts, and emoji fall
back to the remaining bundled OFL fonts. / 已于 2026-08-29 从入库 dist 中移除
专有许可字体（删除字体文件及其 `@font-face` 样式表）；文字回退为系统字体渲染，
表情回退到保留的开源字体。

Remaining bundled fonts (all permissively licensed / 保留的字体均为宽松许可):

| Font | Origin | License |
| --- | --- | --- |
| SarasaUiSC-Regular (subsets) | [Sarasa Gothic](https://github.com/be5invis/Sarasa-Gothic) | SIL OFL 1.1 |
| Fira Sans (via @fontsource) | Mozilla / Carrois Apostrophe | SIL OFL 1.1 |
| NotoColorEmoji-flagsonly.ttf | [Noto Emoji](https://github.com/googlefonts/noto-emoji) | SIL OFL 1.1 |
| TwemojiMozilla-flags.woff2 | Twemoji (Mozilla flags build) | Twemoji graphics: CC-BY 4.0 |

Removed on 2026-08-29 and no longer redistributed (已移除，不再随仓库分发):

- MiSans-VF (subsets) — Xiaomi MiSans, Xiaomi's proprietary MiSans font
  agreement (free to use under Xiaomi's terms, but not an open-source license);
- PingFangSC-Regular (subsets) — Apple PingFang SC (macOS system font), Apple
  proprietary; redistribution outside Apple platforms is a licensing violation.

Inert references: the minified CSS/JS still contains font-family *name* stacks
and settings-UI labels naming MiSans/PingFang. These name references load no
files and are harmless — browsers skip undefined families and use the fallback
stack (`NotoEmoji`/`Twemoji`, then `system-ui`). / 压缩产物中残留的字体名称
声明不加载任何文件，浏览器会跳过缺失字体并回退到保留字体与系统字体。

Note: if this dist is ever re-vendored or rebuilt from upstream, use
zashboard's **`dist-no-fonts`** release variant ("No fonts included, uses
system fonts only") — or an equivalent font-excluding build — so the
proprietary fonts are not reintroduced. / 若日后重新引入或重建上游 dist，请选用
上游的 `dist-no-fonts` 变体（或等效的排除字体构建），以免专有字体再次入库。

## 4. Scope / 范围说明

This file covers vendored binaries and prebuilt third-party assets only.
Rust crates and npm packages are dependencies declared in `Cargo.toml` /
`package.json` files and are governed by their own licenses. / 本文件仅覆盖
入库的二进制与预构建第三方资源；Rust crate 与 npm 依赖以其各自声明文件及许可证为准。

## 5. Fonts bundled with the iced desktop client (`crates/infiltrator-iced/assets/fonts/`)

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

## 6. Icons bundled with the iced desktop client (`crates/infiltrator-iced/assets/icons/*.svg`)

The 28 monochrome stroke icons in `crates/infiltrator-iced/assets/icons/`
are original hand-written SVGs modeled on the **Lucide** icon style
(24x24 viewBox, `stroke-width="2"`, round caps/joins, `fill="none"`,
`stroke="currentColor"`). Lucide itself is ISC-licensed; these files are not
copies of upstream path data and are distributed with this project's license.
For attribution of inspiration: <https://lucide.dev> (ISC).
