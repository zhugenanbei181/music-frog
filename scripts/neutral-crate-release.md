# Neutral crate release (crates.io)

> 层级：L3

The seven neutral ("Lurker") crates are published to crates.io as one ordered
release chain. The order is not a convention: it is derived from the
dependency graph and enforced by the guards listed below.

```text
lurker-core-contract -> lurker-core-ports -> lurker-engine-runtime
-> lurker-profile-store -> lurker-ui-presentation
-> mihomo-driver -> singbox-driver
```

| Entry point | Purpose |
| --- | --- |
| `scripts/quality/publish-order-guard.py` | derives the publish order from the manifests and rejects cycles, missing `version` pins and product crates that are not `publish = false` |
| `scripts/quality/neutral-release-guard.py` | release contract of the chain: presence, publishability, licence, version alignment, dry-run classification, ordered publish |
| `scripts/quality/license-guard.py` | licence whitelist of the third-party notices |
| `scripts/quality/lurker-neutrality-guard.py` | proves that no neutral crate depends on a product crate or a UI toolkit |
| `scripts/quality/ci-trigger-guard.py` | trigger, job-condition and token policy of the release workflow |
| `scripts/neutral-crate-release.sh` | local wrapper around the release guard |

## Local rehearsal

```bash
# Offline topology audit: no network, no cargo, no crates.io.
bash scripts/neutral-crate-release.sh structure

# `cargo publish --dry-run` for every crate in the enforced order.
bash scripts/neutral-crate-release.sh dry-run

# The real upload. Refuses to run without a crates.io API token.
CARGO_REGISTRY_TOKEN=... bash scripts/neutral-crate-release.sh publish
```

Every mode prints the same per-crate table: publish order, crate, version,
licence, neutral dependencies and the resulting status.

## Dry-run semantics

`cargo publish --dry-run` never uploads. It packages and verifies every crate
of the chain, in order. Its failure is classified as follows:

| Status | Meaning | Effect |
| --- | --- | --- |
| `ok` | the crate packages and verifies | — |
| `order-gate` | a neutral dependency of the crate is not on crates.io yet (`no matching package named ...`) | expected before the chain is published; not a failure |
| `already-published` | crates.io already holds this exact version | expected when the tag is re-run |
| `packaging-failure` | a genuine packaging or verification defect | fails the run and blocks the publish job |

A missing third-party dependency is never an `order-gate`: it is a real
packaging failure.

## Required repository secret

| Secret | Value |
| --- | --- |
| `CARGO_REGISTRY_TOKEN` | a crates.io API token that may publish the seven neutral crates |

The token needs the `publish-scope` capability on crates.io, plus `owner` when
the crates belong to an organisation. It is stored as a *repository* secret and
is only ever read from the environment: it never appears in the workflow, in
the job logs, in this repository or in the crate metadata. cargo picks it up
from `CARGO_REGISTRY_TOKEN`.

When the secret is absent the publish job fails loudly and nothing is
published. There is no anonymous, dry-run or debug fallback.

## CI workflow

`.github/workflows/release-neutral-crates.yml` runs the same sequence:

1. `validate-topology` — the guards above, offline.
2. `dry-run` — the ordered dry-run, whose `order-gate` result is expected and
   whose `packaging-failure` result blocks the release.
3. `publish` — the ordered upload, reachable only from a `v*` tag push and
   only with the token secret present.

A manual dispatch runs the dry-run rehearsal only: the publish job requires a
`v*` tag push, so a dispatched run can never upload. Apple targets stay
deferred, so the workflow never schedules a macOS runner.
