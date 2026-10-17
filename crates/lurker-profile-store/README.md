# lurker-profile-store

> 层级：L3

Portable profile-store kernel. It owns the persistence machinery that is
identical for every backend (Mihomo / sing-box), every UI (Iced / Bevy / Slint /
Compose) and every host: layout description, durable atomic document I/O,
cross-process write fences, workspace publication and the profile-state schema.
It depends only on `lurker-core-contract` and `lurker-core-ports`, never on a
product crate, a UI toolkit, `age`, `ring`, `keyring` or `dirs`.

The crate is **not** platform-free: it is a *portable kernel*. Every filesystem
and kernel-lock primitive is expressed by
`platform::StorePlatform`, and `platform::NativeStorePlatform` is the only
implementation that contains per-OS code (`flock`/directory `fsync` on Unix,
`MoveFileExW`/`FlushFileBuffers`/named mutex on Windows). Hosts and tests inject
their own platform through `ProfileStoreCore::with_platform`; the kernel never
selects an implementation with `cfg`. The neutrality guard classifies this crate
as `PORTABLE_KERNEL`: per-OS code is allowed behind the extension point, product
/ engine-adapter / UI dependencies are not.

## Owned here

- `platform` — `StorePlatform`, the substitution boundary for reads, atomic
  writes/replacements, directory operations and the two kernel locks, plus
  `NativeStorePlatform` with the real OS implementations.
- `durable_replace` — the `NotCommitted` / `DurabilityUncertain` split of a
  same-directory replacement, served by the platform.
- `instance_lock` — kernel-mediated path-scoped locks plus the transferable
  `ThreadedInstanceLockGuard` worker hand-off.
- `document_store` — durable whole-document and sidecar bytes, optional reads,
  optional removal, staged-temporary cleanup.
- `file_lock` — the root-to-profile write fence: one process-local mutex per
  canonical directory plus the platform's cross-process engine lock.
- `layout` — `ProfileStoreLayout`, the role-named description of every directory
  and file the kernel touches. Products pin their historical literals on top of
  it; the kernel itself contains none.
- `settings` — TOML settings-document helpers (table repair, durable read/write,
  profile-row removal, typed field setters).
- `namespace` — profile-state schema marker, durable backup recovery, pure
  legacy-state merge and the stateless legacy file migration primitives.
- `store` — `ProfileStoreCore`: engine identity, layout-resolved config root /
  namespace directory / state file, the injected document codec, the injected
  platform and the one-shot namespace-initialization cell.

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
profile-state backup recovery, WTF-8 to UTF-16 path encoding and platform
substitution (`platform::tests`): an injected recording platform must receive
document writes and the profile-write fence, and an injected replacement failure
must preserve the previous document without staged temporaries.
`scripts/quality/lurker-neutrality-guard.py` and
`scripts/quality/core-boundary-guard.py` enforce the purity rules.
