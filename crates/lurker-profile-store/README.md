# lurker-profile-store

> 层级：L3

Engine-neutral profile-store kernel. It owns the persistence machinery that is
identical for every backend (Mihomo / sing-box), every platform
(Windows / Linux / macOS / Android / iOS) and every UI (Iced / Bevy / Slint /
Compose). It depends only on `lurker-core-contract` and `lurker-core-ports`,
never on a product crate, a UI toolkit, `age`, `ring`, `keyring` or `dirs`.

## Owned here

- `durable_replace` — same-directory atomic replacement and durable removal with
  an explicit `NotCommitted` / `DurabilityUncertain` split. Unix renames then
  fsyncs the parent directory; Windows uses `MoveFileExW` with
  `MOVEFILE_WRITE_THROUGH`.
- `instance_lock` — kernel-mediated path-scoped locks (`flock` on Unix, named
  mutex on Windows) plus the transferable `ThreadedInstanceLockGuard`.
- `document_store` — durable whole-document and sidecar bytes, optional reads,
  optional removal, staged-temporary cleanup.
- `file_lock` — the root-to-profile write fence: one process-local mutex per
  canonical directory plus the cross-process engine lock.
- `layout` — `ProfileStoreLayout`, the role-named description of every directory
  and file the kernel touches. Products pin their historical literals on top of
  it; the kernel itself contains none.
- `settings` — TOML settings-document helpers (table repair, durable read/write,
  profile-row removal, typed field setters).
- `namespace` — profile-state schema marker, durable backup recovery, pure
  legacy-state merge and the stateless legacy file migration primitives.
- `store` — `ProfileStoreCore`: engine identity, layout-resolved config root /
  namespace directory / state file, the injected document codec and the one-shot
  namespace-initialization cell.

## Not owned here

Credential stores, subscription URLs, Age keys and Armored Age decryption,
catalog order, display names, aggregation templates, named overrides, active
reference / owner records and reset transactions remain product-owned. The
product facade pins its historical on-disk names in
`infiltrator-profile-store`'s own layout module and keeps the public
`EngineProfileStore` API on top of this kernel.

A neutral crate may not re-export product types, so consumers import the kernel
modules directly (`lurker_profile_store::document_store`,
`lurker_profile_store::store::ProfileDocumentExtension`, and so on).

## Verification

`cargo nextest run -p lurker-profile-store` covers lease contention (single
winner, serialized read-modify-write), atomic replacement (complete value, no
temporaries, previous value preserved on failure), opaque byte round-trip,
profile-state backup recovery and WTF-8 to UTF-16 path encoding.
`scripts/quality/lurker-neutrality-guard.py` and
`scripts/quality/core-boundary-guard.py` enforce the purity rules.
