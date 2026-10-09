# Dual-surface evidence closure

The Rust `infiltrator-contract::parity::FeatureId::ALL` registry owns interaction
identities and required paths. The compiled `parity_catalogue` example exports
that authority as JSON. No Python script reconstructs Rust types with regex.

`cross_surface_manifest.tsv` has exactly one cell for each feature and peer.
`feature_evidence.tsv` has exactly one L1 (`contract`) and L2 (`scenario`) cell
for each peer. Test IDs use `<nextest-binary-id>::<full-test-name>`, with multiple
anchors separated by `;`. Scenario paths use commas. Assign IDs from actual
nextest discovery, then inspect the tests to ensure their assertions prove the
named paths. Discovery itself cannot establish that a test is meaningful or passed.

`pending` records a gap, `implemented` records implemented UI awaiting full
closure, and `ready` is a delivery declaration requiring all three layers.
Neither `pending` nor test discovery becomes a Ready claim. Both peers and both
viewports must pass to accept a feature. Host unsupported uses the existing
capability contract; a missing UI is a pending defect, never an exception.

```bash
# Schema + paired-cell checks only; does not discover tests or accept parity.
python3 scripts/parity/resolve_surface_evidence.py --structure-only
# Compile the Rust authority, discover actual workspace tests, resolve anchors.
python3 scripts/parity/resolve_surface_evidence.py --report-json target/parity/evidence.json
# Capture and accept one implemented interaction, driving production controls.
bash scripts/parity/accept-surface-interactions.sh connections-close-all-confirm
# Full parity acceptance: any missing L1/L2/L3 evidence fails closed.
bash scripts/parity/accept-surface-interactions.sh --all
```

The capture driver uses private virtual KWin + nested niri, writes durable evidence to `.evidence/`,
and never uses the operator's compositor. Pillow is required for pixel geometry.
The existing page capture matrices keep their static-page role; an interaction
row is added only after a real activator exists. Unknown/unimplemented tokens
fail instead of falling back to Overview. Visual receipts bind the activated
control, immutable product build ID, archived executable/runtime resources, app
PID, window ID, PNG SHA256, exact viewport, edge bounds and measured luminance.
Schema 4 verifies the captured build rather than the mutable latest binary or a
repository-wide source hash. Pixel, marker, geometry and archive tampering remain
integrity failures; working-tree changes produce a separate dependency-impact
review and never erase a passed baseline.

`product_build.py` records the actual Cargo production depfile inputs and declared
runtime assets. Documentation, test changes and a different frontend are not
blanket pixel invalidations. Shared inputs can affect both peers. Capture runs
execute the archived binary and Bevy loads its archived assets.

Release acceptance requires `--product-builds <selection.json>` containing one
concrete build ID for each peer. `--require-complete` rejects unspecified or mixed
candidates. The capture driver writes this selection automatically. Baseline
coverage and selected-release coverage are different reports. Schema 3 evidence
keeps its historical scope; a missing original executable cannot be reconstructed
by assigning the latest build's identity.

`retired_source_guards.tsv` records the 79 historical feature/text guards that
were deleted from `scripts/quality/` (replaced by this dynamic evidence system).
`check-test-policy.sh` rejects their re-registration. Shared structural checks
are registered once in `scripts/quality/check-structure.sh`; test workflows use
the two canonical runner scripts. Python negative tests cover dangling IDs,
wrong ownership, missing peers/layers, partial paths and missing/stale pixels.

Acceptance is separate from ordinary mock/headless tests. `scripts/test.sh` and
`scripts/test-bevy.sh` validate declared evidence integrity and run the actual
tests; their reports expose unclosed cells. A successful ordinary test run is
not a full-parity receipt. The canonical product rules are in
[`docs/UI_PARITY_AUDIT.md`](../../docs/UI_PARITY_AUDIT.md).

Captured binaries, runtime assets, run manifests and acceptance records live outside Cargo `target/`, so `cargo clean` removes build caches without deleting acceptance history. CI uploads `.evidence/` explicitly, including hidden files. Legacy `target/parity` reports remain indexed as historical records.

To inspect actual dependency changes for an archived build:

```bash
python3 scripts/parity/product_build.py --assess .evidence/builds/<build-id>/build.json
```
