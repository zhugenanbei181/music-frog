# Cross-surface UI resources

> 层级：L2

All UI surfaces use `infiltrator-shared` for text lookup and visual asset
identity. A surface may adapt a canonical resource to its renderer, but it
does not own a second translation source or duplicate the source file.

`infiltrator_shared::locales::Lang` resolves stable application copy keys and
product gettext message IDs from `crates/infiltrator-shared/locales/product/`.
The Slint compiler reads the same shared `.po` catalogs that Rust surfaces
resolve at runtime. Runtime/user data such as profile names,
proxy names, logs, and controller values stays opaque and is never treated as
a translation key.

`infiltrator_shared::assets::AssetCatalog` resolves semantic asset ID,
format, and variant from `crates/infiltrator-shared/assets/asset-catalog.json`.
The catalog records SVG, PNG, ICO, and font files with their source and license
provenance. Render adapters embed or decode the selected format; they do not
redefine the asset's identity.

The resource implementation and update procedure are documented by the
[`infiltrator-shared` crate](../crates/infiltrator-shared/README.md) and its
[asset catalog guide](../crates/infiltrator-shared/assets/README.md).
